#!/usr/bin/env python3
"""Syncs closure-rs to a new upstream Closure Compiler release: the mechanical part of
docs/PORTING.md §9, as one resumable command.

  python3 scripts/upstream_sync.py vYYYYMMDD [--from STEP | --only STEP[,STEP...]] [--dry-run]
          [--base TAG] [--work DIR] [--jobs N] [--unit-classes A,B] [--d2-sample N]
  python3 scripts/upstream_sync.py --list

Runs the steps below in order on the checkout it lives in (also a git worktree). Every step first
checks whether its result is already there and then only says so, so a run can be repeated or
resumed (`--from STEP`) at any point, and `--only` runs single steps. `--dry-run` runs only the
read-only checks and prints what each step would do. The shared state (reference checkouts, jars,
oracle builds, the D2 golden store) lives in the main checkout, as everywhere else
(scripts/paths.py); a reference that exists already is only verified, never rebuilt or moved. The
sync's own outputs (resolved versions, worklists, npm packages, the recording) go to the work
directory, default build/upstream-sync/<tag>/ of the main checkout.

The last step, `switch-default`, never runs on its own: run it with `--only switch-default` once
the port is verified. It moves the registry's default row to the new reference and rewrites the
places that name the default reference (README, NOTICE, docs, the npm wrapper's comments...).

At the end the command prints what is left: the judgement steps (porting the Java changes,
triaging the worklists) and the verification.
"""
from __future__ import annotations

import argparse
import difflib
import filecmp
import gzip
import hashlib
import io
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import urllib.error
import urllib.request
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paths  # noqa: E402  ROOT, the registry of this checkout

CHECKOUT = os.path.dirname(HERE)  # the checkout this script lives in: tracked files go here
ROOT = paths.ROOT  # the main checkout: reference/, build/, tools/, corpus-cache/
REGISTRY = paths.REGISTRY
UPSTREAM = "https://github.com/google/closure-compiler.git"
NPM_REGISTRY = "https://registry.npmjs.org"
JAVA = os.path.join(ROOT, "tools", "jdk-21", "bin")
FIXED_ENV = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TZ": "UTC"}
UNIT_JARS = ("unit_support_deploy.jar", "unit_all_tests.jar", "compiler_tests_lib.jar")


class StepError(Exception):
    pass


def log(msg=""):
    print(msg, flush=True)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def rel(path):
    """A path for messages: relative to the checkout or the main checkout when under one."""
    for base, prefix in ((CHECKOUT, ""), (ROOT, "<main>/")):
        if path == base or path.startswith(base + os.sep):
            return prefix + (os.path.relpath(path, base) if path != base else ".")
    return path


class Sync:
    def __init__(self, a):
        self.tag = a.tag
        self.dry = a.dry_run
        self.jobs = a.jobs
        self.unit_classes = [c for c in (a.unit_classes or "").split(",") if c]
        self.d2_sample = a.d2_sample
        self.base_arg = a.base
        self.work = os.path.abspath(a.work or os.path.join(ROOT, "build", "upstream-sync", a.tag))
        self.state_path = os.path.join(self.work, "sync.json")
        self.state = None
        if os.path.exists(self.state_path):
            with open(self.state_path, encoding="utf-8") as f:
                self.state = json.load(f)
            if self.state.get("tag") != self.tag:
                raise SystemExit(f"{self.state_path} belongs to {self.state.get('tag')}, not {self.tag}")
        self.notes = []  # (step, text): things the person running the sync must look at

    # ---- helpers -------------------------------------------------------------------------------
    def note(self, step, text):
        self.notes.append((step, text))
        log(f"  NOTE: {text}")

    def env(self, **extra):
        e = dict(os.environ)
        e["CLOSURE_RS_REF"] = self.tag
        e.update({k: v for k, v in extra.items() if v is not None})
        return e

    def run(self, cmd, *, cwd=CHECKOUT, env=None, write=True, check=True, capture=False):
        """Runs cmd (a list). write=True: the command changes something, so --dry-run only prints it."""
        shown = " ".join(shlex.quote(c) for c in cmd)
        if write and self.dry:
            log(f"  would run: {shown}")
            return subprocess.CompletedProcess(cmd, 0, "", "")
        log(f"  $ {shown}")
        p = subprocess.run(cmd, cwd=cwd, env=env or self.env(), text=True,
                           stdout=subprocess.PIPE if capture else None,
                           stderr=subprocess.PIPE if capture else None)
        if check and p.returncode != 0:
            if capture:
                sys.stderr.write((p.stdout or "")[-4000:] + (p.stderr or "")[-4000:])
            raise StepError(f"exit {p.returncode}: {shown}")
        return p

    def git(self, *args, cwd=CHECKOUT, check=True):
        p = subprocess.run(["git", *args], cwd=cwd, text=True, capture_output=True)
        if check and p.returncode != 0:
            raise StepError(f"git {' '.join(args)}: {p.stderr.strip()}")
        return p.stdout

    def diffstat(self, *pathspecs):
        out = self.git("status", "--short", "--", *pathspecs).rstrip()
        log("  tracked changes: " + (("\n    " + out.replace("\n", "\n    ")) if out else "none"))
        return out

    def registry(self):
        return paths.load_registry(REGISTRY)

    def ref(self, tag=None):
        refs, _ = self.registry()
        tag = tag or self.tag
        if tag not in refs:
            raise StepError(f"{tag} is not in scripts/references.tsv (step registry)")
        return refs[tag]

    def need_state(self):
        if self.state is None:
            raise StepError("the tag is not resolved yet (step resolve)")
        return self.state

    def write_state(self):
        if self.dry:
            return
        os.makedirs(self.work, exist_ok=True)
        with open(self.state_path + ".tmp", "w", encoding="utf-8") as f:
            json.dump(self.state, f, indent=1)
            f.write("\n")
        os.replace(self.state_path + ".tmp", self.state_path)

    def abs(self, rel_path):
        return os.path.join(ROOT, rel_path)

    def set_registry_sha(self, sha):
        lines = open(REGISTRY, encoding="utf-8").read().split("\n")
        for i, line in enumerate(lines):
            cols = line.split("\t")
            if not line.startswith("#") and len(cols) >= 7 and cols[0] == self.tag:
                cols[2] = sha
                lines[i] = "\t".join(cols)
        if self.dry:
            log(f"  would pin jar_sha256 {sha} for {self.tag} in scripts/references.tsv")
            return
        with open(REGISTRY, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
        log(f"  pinned jar_sha256 {sha} for {self.tag} in scripts/references.tsv")

    def link_shared(self):
        """In a worktree, the gitignored shared state is reached through symlinks to the main
        checkout (.gitignore allows them): the D2 golden runner and the validation read
        corpus-cache/, reference/ and tools/ relative to the checkout."""
        if os.path.realpath(CHECKOUT) == os.path.realpath(ROOT):
            return
        for d in ("tools", "reference", "corpus-cache"):
            p = os.path.join(CHECKOUT, d)
            if not os.path.lexists(p):
                if self.dry:
                    log(f"  would link {d} -> {rel(os.path.join(ROOT, d))}")
                else:
                    os.symlink(os.path.join(ROOT, d), p)
                    log(f"  linked {d} -> {rel(os.path.join(ROOT, d))}")

    def upstream_repo(self):
        """A bare scratch repository with both pinned commits (for the porting worklist)."""
        st = self.need_state()
        repo = os.path.join(self.work, "upstream.git")
        if not os.path.isdir(repo):
            os.makedirs(repo)
            self.git("init", "-q", "--bare", cwd=repo)
        for tag, commit in ((st["base"], st["base_commit"]), (self.tag, st["commit"])):
            if self.git("cat-file", "-t", commit, cwd=repo, check=False).strip() == "commit":
                continue
            src = None
            try:
                src = self.abs(self.ref(tag).src)
            except StepError:
                pass
            if src and os.path.isdir(os.path.join(src, ".git")) and \
                    self.git("rev-parse", "HEAD", cwd=src, check=False).strip() == commit:
                self.git("fetch", "-q", "--depth", "1", src, commit, cwd=repo)
            else:
                self.git("fetch", "-q", "--depth", "1", UPSTREAM, commit, cwd=repo)
        return repo

    # ---- steps ---------------------------------------------------------------------------------
    def step_resolve(self):
        """Resolves the tag to its commit and the google-closure-compiler npm version."""
        m = re.fullmatch(r"v(\d{8})", self.tag)
        if not m:
            raise StepError(f"{self.tag}: an upstream release tag is vYYYYMMDD")
        if self.state:
            st = self.state
            log(f"  already resolved ({rel(self.state_path)}): {self.tag} = {st['commit']}, "
                f"npm google-closure-compiler@{st['npm_version']}, base {st['base']}")
            return "done"
        out = self.git("ls-remote", UPSTREAM, f"refs/tags/{self.tag}", f"refs/tags/{self.tag}^{{}}")
        refs = dict(reversed(line.split("\t")) for line in out.splitlines() if "\t" in line)
        commit = refs.get(f"refs/tags/{self.tag}^{{}}") or refs.get(f"refs/tags/{self.tag}")
        if not commit:
            raise StepError(f"no tag {self.tag} in {UPSTREAM}")
        npm_version = f"{m.group(1)}.0.0"
        meta = npm_meta("google-closure-compiler", npm_version)
        if meta is None:
            raise StepError(f"google-closure-compiler@{npm_version} is not on npm: syncs target npm "
                            "releases only (docs/PORTING.md §9)")
        refs_, default = self.registry()
        base = self.base_arg or default
        if base == self.tag:  # re-running a sync that is already the default: diff to the row before
            tags = list(refs_)
            base = tags[tags.index(self.tag) - 1] if self.tag in tags and tags.index(self.tag) > 0 else base
        if base not in refs_:
            raise StepError(f"base reference {base} is not in scripts/references.tsv")
        if base == self.tag:
            raise StepError("the base reference is the target; pass --base TAG")
        self.state = {"tag": self.tag, "commit": commit, "npm_version": npm_version,
                      "base": base, "base_commit": refs_[base].commit}
        log(f"  {self.tag} = {commit}; npm google-closure-compiler@{npm_version} is published; "
            f"base reference {base} ({refs_[base].commit[:9]})")
        self.write_state()
        return "ran"

    def step_registry(self):
        """Adds the reference's row to scripts/references.tsv (the default row is not touched)."""
        st = self.need_state()
        refs, _ = self.registry()
        if self.tag in refs:
            if refs[self.tag].commit != st["commit"]:
                raise StepError(f"scripts/references.tsv has {self.tag} at {refs[self.tag].commit}, "
                                f"not {st['commit']}")
            log(f"  {self.tag} is in scripts/references.tsv")
            return "done"
        t = self.tag
        row = [t, st["commit"], "-", f"reference/closure-compiler-{t}",
               f"reference/closure-compiler-{t}-recording", f"build/reference-{t}/closure-compiler.jar",
               f"build/oracle-{t}/oracle.jar"]
        used = {p for r in refs.values() for p in (r.src, r.recording_ws, r.jar, r.oracle_jar)}
        clash = [p for p in row[3:] if p in used]
        if clash:
            raise StepError(f"paths already used by another registry row: {clash}")
        for p in row[3:]:
            if os.path.lexists(self.abs(p)):
                log(f"  {p} exists already: the later steps verify and keep it")
        lines = open(REGISTRY, encoding="utf-8").read().split("\n")
        at = next(i for i, line in enumerate(lines) if line.startswith("default\t"))
        lines.insert(at, "\t".join(row))
        if self.dry:
            log("  would add: " + "\t".join(row))
            return "ran"
        with open(REGISTRY, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
        log("  added: " + " ".join(row))
        return "ran"

    def step_fetch(self):
        """Fetches the reference checkout (scripts/fetch_reference.sh)."""
        r = self.ref()
        src = self.abs(r.src)
        if os.path.isdir(os.path.join(src, ".git")) and \
                self.git("rev-parse", "HEAD", cwd=src, check=False).strip() == r.commit:
            log(f"  {rel(src)} is at {r.commit[:9]}")
            return "done"
        self.run(["bash", "scripts/fetch_reference.sh"])
        return "ran"

    def step_jar(self):
        """Builds the reference uberjar with Bazel (oracle/REFERENCE.md), installs it read-only and
        pins its sha256 in the registry; an existing jar is only hashed."""
        r = self.ref()
        jar = self.abs(r.jar)
        if os.path.exists(jar):
            sha = sha256_file(jar)
            if r.jar_sha256 == sha:
                log(f"  {rel(jar)}: sha256 {sha[:16]}..., as pinned")
                return "done"
            if r.jar_sha256 != "-":
                raise StepError(f"{rel(jar)} has sha256 {sha}, the registry pins {r.jar_sha256}")
            self.set_registry_sha(sha)
            return "ran"
        src = self.abs(r.src)
        st = self.need_state()
        base_src = self.abs(self.ref(st["base"]).src)
        lock = os.path.join(src, "MODULE.bazel.lock")
        base_lock = os.path.join(base_src, "MODULE.bazel.lock")
        if not os.path.exists(lock) and os.path.exists(base_lock):
            if filecmp.cmp(os.path.join(src, "MODULE.bazel"), os.path.join(base_src, "MODULE.bazel"),
                           shallow=False):
                if self.dry:
                    log(f"  would copy {rel(base_lock)} (MODULE.bazel is unchanged)")
                else:
                    shutil.copyfile(base_lock, lock)
                    log(f"  copied {rel(base_lock)} (MODULE.bazel is unchanged)")
            else:
                self.note("jar", "MODULE.bazel changed upstream: Bazel resolves the modules afresh and "
                                 "writes a new MODULE.bazel.lock")
        build = ". \"$1/tools/env.sh\" && cd \"$2\" && bazelisk build " \
                "--local_resources=memory=HOST_RAM*0.3 --local_resources=cpu=HOST_CPUS*0.75 " \
                "//:compiler_uberjar_deploy.jar; rc=$?; bazelisk shutdown; exit $rc"
        self.run(["bash", "-c", build, "_", ROOT, src])
        if self.dry:
            log(f"  would install bazel-bin/compiler_uberjar_deploy.jar as {rel(jar)} (mode 0555) "
                "and pin its sha256")
            return "ran"
        os.makedirs(os.path.dirname(jar), exist_ok=True)
        shutil.copyfile(os.path.join(src, "bazel-bin", "compiler_uberjar_deploy.jar"), jar + ".tmp")
        os.chmod(jar + ".tmp", 0o555)
        os.replace(jar + ".tmp", jar)
        sha = sha256_file(jar)
        with open(jar + ".sha256", "w") as f:
            f.write(f"{sha}  {os.path.basename(jar)}\n")
        log(f"  installed {rel(jar)} ({sha})")
        self.set_registry_sha(sha)
        self.note("jar", "the jar's bytes depend on the host zlib (oracle/REFERENCE.md); the "
                         "java-comparison workflow must rebuild the same sha256")
        return "ran"

    def step_npm_check(self):
        """Compares the npm release with the reference: the package's JS wrapper against the base
        release's, and the compiler.jar of google-closure-compiler-java against the reference jar."""
        st = self.need_state()
        out = os.path.join(self.work, "npm-check.txt")
        if os.path.exists(out):
            log(f"  {rel(out)} exists:")
            log("    " + open(out, encoding="utf-8").read().rstrip().replace("\n", "\n    "))
            return "done"
        if self.dry:
            log(f"  would download google-closure-compiler(-java)@{st['npm_version']} into "
                f"{rel(os.path.join(self.work, 'npm'))} and write {rel(out)}")
            return "ran"
        jar = self.abs(self.ref().jar)
        if not os.path.exists(jar):
            raise StepError(f"{rel(jar)} is missing (step jar)")
        report = []
        new = npm_unpack("google-closure-compiler", st["npm_version"], os.path.join(self.work, "npm"))
        m = re.fullmatch(r"v(\d{8})", st["base"])
        if m:
            old_version = f"{m.group(1)}.0.0"
            old = npm_unpack("google-closure-compiler", old_version, os.path.join(self.work, "npm"))
            changed, diff = wrapper_diff(old, new)
            report.append(f"google-closure-compiler {old_version} -> {st['npm_version']}: "
                          + (f"{len(changed)} wrapper file(s) changed: {', '.join(changed)}"
                             if changed else "the JS wrapper is unchanged"))
            report += [f"  package.json {k}: {v}" for k, v in package_json_diff(old, new)]
            with open(os.path.join(self.work, "npm-wrapper.diff"), "w", encoding="utf-8") as f:
                f.write(diff)
            if changed:
                self.note("npm-check", f"the npm wrapper changed upstream: {rel(os.path.join(self.work, 'npm-wrapper.diff'))} "
                                       "(npm/closure-rs follows it; its comments cite line numbers)")
        else:
            report.append(f"base {st['base']} is not an npm release: wrapper not compared")
        java = npm_unpack("google-closure-compiler-java", st["npm_version"], os.path.join(self.work, "npm"))
        differ, only_npm, only_ref = jar_compare(os.path.join(java, "compiler.jar"), jar)
        report.append(f"google-closure-compiler-java@{st['npm_version']} compiler.jar vs {self.ref().jar}: "
                      f"{len(differ)} entries differ, {len(only_npm)} only in npm, {len(only_ref)} only in the reference")
        report += [f"  differs: {n}" for n in differ[:50]] + [f"  only npm: {n}" for n in only_npm[:50]] \
            + [f"  only reference: {n}" for n in only_ref[:50]]
        with open(out, "w", encoding="utf-8") as f:
            f.write("\n".join(report) + "\n")
        log("  " + "\n  ".join(report))
        if only_npm or only_ref or len(differ) > 2:
            self.note("npm-check", "the npm compiler.jar differs from the reference jar beyond the version "
                                   f"stamp: {rel(out)}")
        return "ran"

    def step_oracle(self):
        """Builds the oracle jar against the reference jar (oracle/build.sh); an existing one is kept."""
        r = self.ref()
        oracle = self.abs(r.oracle_jar)
        if os.path.exists(oracle):
            log(f"  {rel(oracle)} exists (not rebuilt; oracle/build.sh rebuilds it)")
            return "done"
        if r.jar_sha256 == "-" and not self.dry:
            raise StepError("the reference jar is not pinned (step jar)")
        self.run(["bash", "oracle/build.sh"])
        return "ran"

    def step_resources(self):
        """Regenerates the jar resources (crates/resources/tools/sync_from_jar.py) and the Java-side
        resource fixtures (crates/resources/tools/GenResourceFixtures.java)."""
        r = self.ref()
        jar, src = self.abs(r.jar), self.abs(r.src)
        self.run([sys.executable, "crates/resources/tools/sync_from_jar.py", jar, src])
        gen = os.path.join(self.work, "resource-fixtures")
        cls, out = os.path.join(gen, "classes"), os.path.join(gen, "out")
        fixtures = os.path.join(CHECKOUT, "crates", "resources", "tests", "fixtures")
        if self.dry:
            log(f"  would run GenResourceFixtures against {rel(jar)} and copy its output to "
                f"{rel(fixtures)}")
        else:
            shutil.rmtree(gen, ignore_errors=True)
            os.makedirs(cls)
            self.run([os.path.join(JAVA, "javac"), "-nowarn", "-encoding", "UTF-8", "-cp", jar, "-d", cls,
                      os.path.join(CHECKOUT, "crates/resources/tools/GenResourceFixtures.java")])
            self.run([os.path.join(JAVA, "java"), "-cp", f"{jar}:{cls}", "GenResourceFixtures", out],
                     cwd=gen, env=dict(FIXED_ENV, HOME=os.environ.get("HOME", "/")))
            for name in sorted(os.listdir(out)):
                dst = os.path.join(fixtures, name)
                if not os.path.exists(dst) or not filecmp.cmp(os.path.join(out, name), dst, shallow=False):
                    shutil.copyfile(os.path.join(out, name), dst)
                    log(f"  updated {rel(dst)}")
        if not self.diffstat("crates/resources"):
            return "ran (no changes)"
        self.note("resources", "the jar resources changed: check the polyfill / runtime library "
                               "changes against the porting worklist")
        return "ran"

    def step_flags(self):
        """Regenerates scope/flags.txt (scope/gen_flags.py) and the npm types (scripts/gen_npm_types.mjs)."""
        self.run([sys.executable, "scope/gen_flags.py"])
        self.run(["node", "scripts/gen_npm_types.mjs"], check=False)
        if not self.diffstat("scope/flags.txt", "npm/closure-rs/index.d.ts", "npm/closure-rs/index.d.cts"):
            return "ran (no changes)"
        self.note("flags", "CommandLineRunner's flags changed: port them (crates/cli OPTIONS); "
                           "gen_npm_types.mjs stops while the two flag lists disagree")
        return "ran"

    def step_license_headers(self):
        """Refreshes the license headers to the new reference (scripts/license_headers.py)."""
        p = self.run([sys.executable, "scripts/license_headers.py", "--check"], write=False, check=False,
                     capture=True)
        last = [line for line in ((p.stdout or "") + (p.stderr or "")).splitlines() if line.startswith(("full check", "manifest check"))]
        if p.returncode == 0 and last and last[0].startswith("full check"):
            log(f"  {last[0]}: the headers are current")
            return "done"
        self.run([sys.executable, "scripts/license_headers.py", "--apply"])
        self.diffstat("LICENSES", "crates")
        return "ran"

    def step_worklist(self):
        """Writes the porting worklist: the upstream commits and every file changed between the base
        and the new commit, with the Rust files ported from it (LICENSES/headers.tsv) and the unit
        record files of the test classes."""
        st = self.need_state()
        if self.dry:
            log(f"  would write {rel(os.path.join(self.work, 'porting-worklist.md'))} "
                f"({st['base']}..{self.tag})")
            return "ran"
        repo = self.upstream_repo()
        base, new = st["base_commit"], st["commit"]
        commits = []
        if re.fullmatch(r"v\d{8}", st["base"]):
            p = subprocess.run(["git", "fetch", "-q", f"--shallow-exclude={st['base']}", UPSTREAM,
                                f"refs/tags/{self.tag}:refs/tags/{self.tag}"], cwd=repo,
                               capture_output=True, text=True)
            if p.returncode == 0:
                commits = self.git("log", "--format=%h %as %s", "--reverse", new, f"^{base}", cwd=repo,
                                   check=False).splitlines()
        numstat = self.git("diff", "--numstat", "-M", base, new, cwd=repo)
        status = dict(line.split("\t", 1)[::-1] for line in
                      self.git("diff", "--name-status", "--no-renames", base, new, cwd=repo).splitlines())
        with open(os.path.join(self.work, "upstream.diff"), "w", encoding="utf-8") as f:
            f.write(self.git("diff", "-M", base, new, cwd=repo))
        origins = {}
        with open(os.path.join(CHECKOUT, "LICENSES", "headers.tsv"), encoding="utf-8") as f:
            for line in f:
                if line.startswith("#"):
                    continue
                cols = line.rstrip("\n").split("\t")
                for o in cols[3].split("; ") if len(cols) > 3 else []:
                    if o.startswith("closure:"):
                        origins.setdefault(o[len("closure:"):], []).append(cols[0])
        records = os.path.join(CHECKOUT, "corpus", "unit", "records")
        groups = {"src": [], "test": [], "resources": [], "other": []}
        for line in numstat.splitlines():
            add, dele, path = line.split("\t", 2)
            if " => " in path:  # rename: a/{b => c}/d or a => b
                m = re.match(r"(.*)\{(.*) => (.*)\}(.*)", path)
                old_p, path = (m.group(1) + m.group(2) + m.group(4), m.group(1) + m.group(3) + m.group(4)) \
                    if m else path.split(" => ")
                kind = f"renamed from {old_p.replace('//', '/')}"
                path = path.replace("//", "/")
            else:
                kind = {"A": "added", "D": "deleted", "M": "modified"}.get(status.get(path, "M")[:1], "modified")
            e = {"path": path, "kind": kind, "added": add, "deleted": dele,
                 "rust": sorted(origins.get(path, []))}
            if path.startswith(("externs/", "src/com/google/javascript/jscomp/js/")):
                groups["resources"].append(e)
            elif path.startswith("src/"):
                groups["src"].append(e)
            elif path.startswith("test/"):
                cls = os.path.basename(path)[:-len(".java")] if path.endswith("Test.java") else None
                if cls and has_records(os.path.join(records, cls + ".jsonl.gz")):
                    e["records"] = f"corpus/unit/records/{cls}.jsonl.gz"
                groups["test"].append(e)
            else:
                groups["other"].append(e)
        wl = {"tag": self.tag, "commit": new, "base": st["base"], "base_commit": base,
              "commits": commits, **groups}
        with open(os.path.join(self.work, "porting-worklist.json"), "w", encoding="utf-8") as f:
            json.dump(wl, f, indent=1)
            f.write("\n")
        md = [f"# Porting worklist {st['base']} -> {self.tag}", "",
              f"Upstream {base[:9]}..{new[:9]}; the full diff is upstream.diff next to this file.", ""]
        if commits:
            md += [f"## Commits ({len(commits)})", ""] + [f"- {c}" for c in commits] + [""]
        titles = {"src": "Java sources (port these; Rust files from LICENSES/headers.tsv)",
                  "test": "Tests (new or changed cases: unit records, Rust test ports)",
                  "resources": "Runtime libraries and externs (step resources syncs them from the jar)",
                  "other": "Build and other files"}
        for g, title in titles.items():
            md += [f"## {title}: {len(groups[g])}", ""]
            for e in groups[g]:
                extra = []
                if g == "src":
                    extra.append(", ".join(e["rust"]) if e["rust"] else "no Rust file ports it")
                if e.get("rust") and g != "src":
                    extra.append(", ".join(e["rust"]))
                if e.get("records"):
                    extra.append(e["records"])
                md.append(f"- `{e['path']}` ({e['kind']}, +{e['added']} -{e['deleted']})"
                          + (": " + "; ".join(extra) if extra else ""))
            md.append("")
        with open(os.path.join(self.work, "porting-worklist.md"), "w", encoding="utf-8") as f:
            f.write("\n".join(md))
        log(f"  {len(commits)} upstream commits; {len(groups['src'])} source, {len(groups['test'])} test, "
            f"{len(groups['resources'])} resource and {len(groups['other'])} other files changed")
        unmapped = [e["path"] for e in groups["src"] if not e["rust"]]
        if unmapped:
            log(f"  source files no Rust file names as its origin: {', '.join(unmapped)}")
        log(f"  wrote {rel(os.path.join(self.work, 'porting-worklist.md'))} (and .json, upstream.diff)")
        return "ran"

    def step_unit_ws(self):
        """Rebuilds the recording workspace: the reference plus oracle/patches
        (scripts/unit_make_recording_ws.sh); a current one is kept."""
        r = self.ref()
        ws = self.abs(r.recording_ws)
        if os.path.isdir(os.path.join(ws, ".git")) and \
                self.git("rev-parse", "HEAD", cwd=ws, check=False).strip() == r.commit:
            pdir = os.path.join(CHECKOUT, "oracle", "patches")
            applied = all(subprocess.run(["git", "apply", "--check", "-R", os.path.join(pdir, p)], cwd=ws,
                                         capture_output=True).returncode == 0
                          for p in sorted(os.listdir(pdir)) if p.endswith(".patch"))
            if applied:
                log(f"  {rel(ws)} is at {r.commit[:9]} with oracle/patches applied")
                return "done"
            log(f"  {rel(ws)} does not carry this checkout's oracle/patches: rebuilding it")
        self.run(["bash", "scripts/unit_make_recording_ws.sh"])
        return "ran"

    def unit_dirs(self):
        u = os.path.join(self.work, "unit")
        return {"jars": os.path.join(u, "jars"), "records": os.path.join(u, "records"),
                "recording": os.path.join(u, "recording"), "base": os.path.join(u, "records-base"),
                "classes": os.path.join(u, "all_test_classes.txt"), "run": os.path.join(u, "run_classes.txt"),
                "rust": os.path.join(u, "rust")}

    def step_unit_jars(self):
        """Builds the recording jars with Bazel (scripts/unit_bazel_build.sh) into the work directory."""
        d = self.unit_dirs()
        if all(os.path.exists(os.path.join(d["jars"], j)) for j in UNIT_JARS):
            log(f"  {rel(d['jars'])} holds the recording jars")
            return "done"
        ws = self.abs(self.ref().recording_ws)
        self.run(["bash", "scripts/unit_bazel_build.sh"], env=self.env(UNIT_JARS_DIR=d["jars"]))
        self.run(["bash", "-c", ". \"$1/tools/env.sh\" && cd \"$2\" && bazelisk shutdown", "_", ROOT, ws])
        return "ran"

    def test_classes(self):
        ws = self.abs(self.ref().recording_ws)
        out = []
        for dp, _dirs, files in os.walk(os.path.join(ws, "test")):
            for fn in files:
                if fn.endswith("Test.java"):
                    p = os.path.relpath(os.path.join(dp, fn), os.path.join(ws, "test"))
                    out.append(p[:-len(".java")].replace(os.sep, "."))
        return sorted(out)

    def run_classes(self, all_classes):
        if not self.unit_classes:
            return all_classes
        by_name = {c.rsplit(".", 1)[-1]: c for c in all_classes}
        missing = [c for c in self.unit_classes if c not in by_name and c not in all_classes]
        if missing:
            raise StepError(f"--unit-classes: no such test class {missing}")
        return [by_name.get(c, c) for c in self.unit_classes]

    def step_unit_record(self):
        """Records the unit corpus against the new reference (scripts/unit_record_all.sh) into the
        work directory; resumable per class."""
        d = self.unit_dirs()
        classes = self.test_classes()
        run = self.run_classes(classes)
        stats = os.path.join(d["recording"], "stats")
        todo = [c for c in run if not os.path.exists(os.path.join(stats, c.rsplit(".", 1)[-1] + ".json"))]
        if not todo:
            log(f"  all {len(run)} classes are recorded ({rel(d['records'])})")
            return "done"
        if not all(os.path.exists(os.path.join(d["jars"], j)) for j in UNIT_JARS) and not self.dry:
            raise StepError(f"the recording jars are missing in {rel(d['jars'])} (step unit-jars)")
        log(f"  {len(todo)} of {len(run)} classes to record")
        if not self.dry:
            os.makedirs(os.path.dirname(d["classes"]), exist_ok=True)
            with open(d["classes"], "w") as f:
                f.write("".join(c + "\n" for c in classes))
            with open(d["run"], "w") as f:
                f.write("".join(c + "\n" for c in run))
        env = self.env(UNIT_JARS_DIR=d["jars"], UNIT_RECORDS_DIR=d["records"], UNIT_RECORDING_DIR=d["recording"],
                       UNIT_RECORD_PARALLEL=str(self.jobs) if self.jobs else None)
        self.run(["bash", "scripts/unit_record_all.sh", d["run"]], env=env)
        if self.dry:
            return "ran"
        crashed = [c for c in run if os.path.exists(os.path.join(stats, c.rsplit(".", 1)[-1] + ".crash.json"))]
        left = [c for c in run if not os.path.exists(os.path.join(stats, c.rsplit(".", 1)[-1] + ".json"))]
        if crashed:
            self.note("unit-record", f"{len(crashed)} recording JVMs exited non-zero: "
                                     f"{', '.join(c.rsplit('.', 1)[-1] for c in crashed[:20])} "
                                     f"(logs in {rel(os.path.join(d['recording'], 'logs'))})")
        if left:
            raise StepError(f"{len(left)} classes have no stats file; re-run --from unit-record to retry them")
        return "ran"

    def step_unit_install(self):
        """Copies the new records into corpus/unit/records (the old ones are kept in the work
        directory) and regenerates what derives from them."""
        d = self.unit_dirs()
        dst = os.path.join(CHECKOUT, "corpus", "unit", "records")
        classes = self.test_classes()
        run = self.run_classes(classes)
        stats = os.path.join(d["recording"], "stats")
        names = [c.rsplit(".", 1)[-1] for c in run]
        if not self.dry and [n for n in names if not os.path.exists(os.path.join(stats, n + ".json"))]:
            raise StepError("the recording is not complete (step unit-record)")
        if not os.path.isdir(d["base"]):
            if self.dry:
                log(f"  would keep the current records in {rel(d['base'])}")
            else:
                shutil.copytree(dst, d["base"] + ".tmp", dirs_exist_ok=True)
                os.replace(d["base"] + ".tmp", d["base"])
                log(f"  kept the current records in {rel(d['base'])}")
        changed = []
        if not os.path.isdir(d["records"]):
            if not self.dry:
                raise StepError(f"no recording in {rel(d['records'])} (step unit-record)")
            log(f"  would copy the records of {rel(d['records'])} once they are recorded")
            names = []
        names_now = set(names)
        if not self.unit_classes and names:  # a complete recording: classes gone upstream lose their records
            all_names = {c.rsplit(".", 1)[-1] for c in classes}
            names_now |= {f[:-len(".jsonl.gz")] for f in os.listdir(dst)
                          if f.endswith(".jsonl.gz") and f[:-len(".jsonl.gz")] not in all_names}
        for n in sorted(names_now):
            s, t = os.path.join(d["records"], n + ".jsonl.gz"), os.path.join(dst, n + ".jsonl.gz")
            if os.path.exists(s):
                if not os.path.exists(t) or not filecmp.cmp(s, t, shallow=False):
                    changed.append(("updated" if os.path.exists(t) else "added", n))
                    if not self.dry:
                        shutil.copyfile(s, t)
            elif os.path.exists(t):
                changed.append(("removed", n))
                if not self.dry:
                    os.remove(t)
        verb = "would change" if self.dry else "changed"
        log(f"  {verb} {len(changed)} record files" + "".join(f"\n    {k} {n}" for k, n in changed))
        self.run([sys.executable, "scripts/unit_derive_expected_pipeline.py"])
        self.run([sys.executable, "scripts/unit_postcall_impls.py", "--check"], write=False, capture=True)
        self.run([sys.executable, "scripts/unit_rust_unit_tests.py", "--check"], write=False)
        self.run(["bash", "scripts/unit_options_defaults.sh"])
        if self.diffstat("corpus/unit"):
            new_cls = [n for k, n in changed if k != "removed" and has_records(os.path.join(dst, n + ".jsonl.gz"))]
            missing = [n for n in new_cls if not os.path.exists(
                os.path.join(CHECKOUT, "corpus", "unit", "descriptors", n + ".json"))]
            if missing:
                self.note("unit-install", f"record files of classes without a descriptor: {', '.join(missing)}")
        return "done" if not changed else "ran"

    def step_unit_worklist(self):
        """Replays the records with the Rust runner and rebases the unit ratchet across the
        re-recording (scripts/unit_ratchet_rebase.py): the regression worklist."""
        d = self.unit_dirs()
        out = os.path.join(self.work, "unit-worklist.json")
        rust = d["rust"]
        report, records_out = os.path.join(rust, "report.json"), os.path.join(rust, "records.jsonl")
        if os.path.exists(out):
            log(f"  {rel(out)} exists (delete it to replay again)")
            print_unit_worklist(json.load(open(out, encoding="utf-8")))
            return "done"
        if self.dry:
            log(f"  would replay the records with the Rust runner and write {rel(out)}")
            return "ran"
        records = os.path.join(CHECKOUT, "corpus", "unit", "records")
        base = d["base"] if os.path.isdir(d["base"]) else records
        os.makedirs(rust, exist_ok=True)
        names = [c.rsplit(".", 1)[-1] for c in self.run_classes(self.test_classes())] if self.unit_classes else None
        sel = ["--classes", ",".join(names)] if names else ["--all"]
        env = self.env(CARGO_BUILD_JOBS=os.environ.get("CARGO_BUILD_JOBS", "2"))
        p = self.run(["cargo", "run", "--release", "-q", "-p", "closure-testing", "--bin", "unit_replay", "--",
                      *sel, "--report", report, "--records-out", records_out], env=env, check=False)
        if p.returncode not in (0, 1):
            raise StepError(f"the unit-record runner exited {p.returncode}")
        status = {}
        with open(records_out, encoding="utf-8") as f:
            for line in f:
                if line.strip():
                    r = json.loads(line)
                    status[f"{r['class']}[{r['index']}]"] = (r["status"], (r.get("why") or "")[:300])
        import unit_ratchet_rebase as urr  # noqa: E402  (scripts/)
        old_passing = urr.load_passing(os.path.join(CHECKOUT, "gates", "unit_ratchet.json"))
        new_passing = [rid for rid, (s, _) in status.items() if s == "pass"]
        if names:  # the other classes were not re-recorded: their records and results stand
            keep = set(names)
            new_passing += [rid for rid in old_passing if urr.parse_id(rid)[0] not in keep]
        s = urr.rebase(old_passing, base, records, new_passing)
        for item in s.get("regressed", []):
            item["status"], item["why"] = status.get(item["newId"], ("not replayed", ""))
        s["newRecords"] = [{"id": rid, "status": status.get(rid, ("not replayed", ""))[0],
                            "why": status.get(rid, ("", ""))[1]} for rid in s.pop("new")]
        s["replayed"] = {"classes": names or "all", "records": len(status),
                         "pass": sum(1 for v in status.values() if v[0] == "pass"),
                         "fail": sum(1 for v in status.values() if v[0] not in ("pass", "unported")),
                         "unported": sum(1 for v in status.values() if v[0] == "unported")}
        with open(out, "w", encoding="utf-8") as f:
            json.dump(s, f, indent=1)
            f.write("\n")
        print_unit_worklist(s)
        log(f"  wrote {rel(out)}; per-record results in {rel(records_out)}")
        return "ran"

    def step_d2_golden(self):
        """Runs the reference jar over the D2 corpus (gates/lib/golden_all.py): the golden results
        of the new reference, next to the older references' ones; resumable."""
        self.link_shared()
        if self.ref().jar_sha256 == "-":
            if self.dry:
                log("  the jar is not pinned yet: golden_all.py would run after step jar")
                return "ran"
            raise StepError("the reference jar is not pinned (step jar)")
        p = self.run([sys.executable, "gates/lib/golden_all.py", "--summary-only"], write=False, capture=True)
        summ = json.loads(p.stdout)
        if summ.get("missing") == 0:
            log(f"  golden store {self.ref().golden_tag}: all {summ['pairs_total']} pairs present")
            return "done"
        log(f"  golden store {self.ref().golden_tag}: {summ.get('missing')} of {summ.get('pairs_total')} pairs missing")
        self.run([sys.executable, "gates/lib/golden_all.py"] + (["--jobs", str(self.jobs)] if self.jobs else []))
        return "ran"

    def step_d2_validate(self):
        """Rebuilds the final D2 corpus from the new golden results: corpus/d2/profiles.json's
        reference, then corpus/d2/validate/scan_golden.py, wp_test.py and finalize.py."""
        self.link_shared()
        r = self.ref()
        prof = os.path.join(CHECKOUT, "corpus", "d2", "profiles.json")
        text = open(prof, encoding="utf-8").read()
        want = {"commit": r.commit, "jar": r.jar, "jar_sha256": r.jar_sha256}
        m = re.search(r'"reference": \{[^}]*\}', text)
        cur = json.loads("{" + m.group(0) + "}")["reference"]
        if cur != want:
            block = m.group(0)
            for k, v in want.items():
                block = re.sub(rf'("{k}": )"[^"]*"', lambda mm: f'{mm.group(1)}"{v}"', block)
            if self.dry:
                log(f"  would set the reference of {rel(prof)} to {self.tag}")
            else:
                with open(prof, "w", encoding="utf-8") as f:
                    f.write(text.replace(m.group(0), block))
                if json.load(open(prof, encoding="utf-8"))["reference"] != want:
                    raise StepError(f"could not update {rel(prof)}")
                log(f"  set the reference of {rel(prof)} to {self.tag}")
        val = os.path.join(CHECKOUT, "build", "validate")
        marker = os.path.join(val, "golden_tag")
        if not self.dry:
            # wp_test.py keeps its results across runs; they belong to one golden store
            if not os.path.exists(marker) or open(marker).read().strip() != r.golden_tag:
                shutil.rmtree(os.path.join(val, "wp"), ignore_errors=True)
            os.makedirs(val, exist_ok=True)
            with open(marker, "w") as f:
                f.write(r.golden_tag + "\n")
        jobs = [str(self.jobs)] if self.jobs else []
        self.run([sys.executable, "corpus/d2/validate/scan_golden.py", *jobs])
        # node resolves the corpus-cache symlink of a worktree; the harnesses expect the link path
        self.run([sys.executable, "corpus/d2/validate/wp_test.py", *jobs],
                 env=self.env(NODE_OPTIONS="--preserve-symlinks --preserve-symlinks-main"))
        self.run([sys.executable, "corpus/d2/validate/finalize.py"])
        if not self.diffstat("corpus/d2"):
            return "ran (no changes)"
        self.note("d2-validate", "the final D2 corpus changed: corpus/d2/JAVA_FAILURES.md and "
                                 "WHOLE_PROGRAM.md say which pairs the new Java fails or now passes")
        return "ran"

    def step_d2_worklist(self):
        """Runs the release binary over the D2 corpus against the new golden results
        (gates/d2_rust.py run) and rebases the D2 ratchet (check --rebase): the regression worklist."""
        self.link_shared()
        out = os.path.join(self.work, "d2-rust")
        wl = os.path.join(self.work, "d2-worklist.txt")
        if os.path.exists(wl):
            log(f"  {rel(wl)} exists (delete it and {rel(out)} to run again):")
            log("    " + open(wl, encoding="utf-8").read().rstrip().replace("\n", "\n    "))
            return "done"
        target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(CHECKOUT, "target")
        binary = os.path.join(target, "release", "closure-rs")
        self.run(["cargo", "build", "--release", "-q", "--bin", "closure-rs"],
                 env=self.env(CARGO_BUILD_JOBS=os.environ.get("CARGO_BUILD_JOBS", "4")))
        sample = ["--sample", str(self.d2_sample), "--seed", "1"] if self.d2_sample else []
        jobs = ["--jobs", str(self.jobs)] if self.jobs else []
        if not self.dry:
            shutil.rmtree(out, ignore_errors=True)
        p = self.run([sys.executable, "gates/d2_rust.py", "run", "--bin", binary, "--out", out, *sample, *jobs],
                     check=False, capture=True)
        if self.dry:
            log(f"  would run gates/d2_rust.py check --rebase and write {rel(wl)}")
            return "ran"
        if p.returncode != 0:
            sys.stderr.write((p.stdout or "")[-3000:] + (p.stderr or "")[-3000:])
            raise StepError(f"gates/d2_rust.py run exited {p.returncode}")
        lines = [line for line in p.stdout.splitlines() if line.strip()][-25:]
        if self.d2_sample:
            lines.append(f"(--d2-sample {self.d2_sample}: a sample is not comparable with the baseline, no rebase)")
        else:
            c = self.run([sys.executable, "gates/d2_rust.py", "check", "--rebase", "--baseline",
                          os.path.join(CHECKOUT, "gates", "d2_ratchet.json"), "--current",
                          os.path.join(out, "ratchet.json")], check=False, capture=True)
            lines += (c.stdout or "").splitlines()
            if c.returncode == 1:
                self.note("d2-worklist", f"previously passing D2 pairs fail against the new reference: {rel(wl)}")
            elif c.returncode != 0:
                raise StepError(f"gates/d2_rust.py check --rebase exited {c.returncode}: {c.stdout[-2000:]}")
        with open(wl, "w", encoding="utf-8") as f:
            f.write("\n".join(lines) + "\n")
        log("  " + "\n  ".join(lines))
        return "ran"

    def step_switch_default(self):
        """Makes the new reference the default (only with --only switch-default, once the port is
        verified): the registry's default row and every text that names the default reference."""
        refs, default = self.registry()
        r = self.ref()
        if default == self.tag:
            log(f"  {self.tag} is the registry's default")
            return "done"
        if r.jar_sha256 == "-":
            raise StepError("the reference jar is not pinned (step jar)")
        old = refs[default]
        tags = list(refs)
        older = refs[tags[tags.index(default) - 1]] if tags.index(default) > 0 else None
        dates = (ref_date(self.abs(old.src)), ref_date(self.abs(r.src)))
        changed = []
        for path in doc_files():
            full = os.path.join(CHECKOUT, path)
            text = open(full, encoding="utf-8").read()
            new = rewrite_doc(path, text, old, r, older, self.need_state()["npm_version"], dates)
            if new != text:
                changed.append(path)
                if not self.dry:
                    with open(full, "w", encoding="utf-8") as f:
                        f.write(new)
        lines = open(REGISTRY, encoding="utf-8").read().split("\n")
        lines = [f"default\t{self.tag}" if line.startswith("default\t") else line for line in lines]
        if not self.dry:
            with open(REGISTRY, "w", encoding="utf-8") as f:
                f.write("\n".join(lines))
        verb = "would rewrite" if self.dry else "rewrote"
        log(f"  {verb} {len(changed)} files: {', '.join(changed)}")
        log(f"  {'would set' if self.dry else 'set'} the registry's default to {self.tag}")
        self.run(["node", "scripts/gen_npm_types.mjs"])
        self.note("switch-default", "read the diff: prose that names the old reference other than by "
                                    "its tag, commit, npm version or jar sha256 is not rewritten")
        return "ran"


def has_records(path):
    if not os.path.exists(path):
        return False
    with gzip.open(path, "rb") as f:
        return bool(f.read(1))


# ---- npm ----------------------------------------------------------------------------------------
def npm_meta(name, version):
    try:
        with urllib.request.urlopen(f"{NPM_REGISTRY}/{name}/{version}", timeout=60) as resp:
            return json.load(resp)
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise


def npm_unpack(name, version, into):
    """Downloads name@version (integrity-checked) into its own directory; returns the package dir.
    Only regular files are extracted, nothing is run."""
    dest = os.path.join(into, f"{name}-{version}")
    if os.path.isdir(os.path.join(dest, "package")):
        return os.path.join(dest, "package")
    meta = npm_meta(name, version)
    if meta is None:
        raise StepError(f"{name}@{version} is not on npm")
    with urllib.request.urlopen(meta["dist"]["tarball"], timeout=300) as resp:
        data = resp.read()
    algo, _, want = meta["dist"]["integrity"].partition("-")
    import base64
    if base64.b64encode(hashlib.new(algo, data).digest()).decode() != want:
        raise StepError(f"{name}@{version}: integrity mismatch")
    tmp = dest + ".tmp"
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp)
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as t:
        for m in t.getmembers():
            parts = m.name.split("/")
            if not m.isfile() or m.name.startswith("/") or ".." in parts:
                continue
            target = os.path.join(tmp, "package", *parts[1:])
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, "wb") as f:
                f.write(t.extractfile(m).read())
    os.replace(tmp, dest)
    return os.path.join(dest, "package")


def package_files(d):
    out = {}
    for dp, _dirs, files in os.walk(d):
        for fn in files:
            p = os.path.relpath(os.path.join(dp, fn), d)
            if p != "package.json":
                out[p] = os.path.join(dp, fn)
    return out


def wrapper_diff(old, new):
    a, b = package_files(old), package_files(new)
    changed, diff = [], []
    for p in sorted(set(a) | set(b)):
        ta = open(a[p], encoding="utf-8", errors="replace").read().splitlines(True) if p in a else []
        tb = open(b[p], encoding="utf-8", errors="replace").read().splitlines(True) if p in b else []
        if ta != tb:
            changed.append(p)
            diff += difflib.unified_diff(ta, tb, f"old/{p}", f"new/{p}")
    return changed, "".join(diff)


def package_json_diff(old, new):
    a = json.load(open(os.path.join(old, "package.json"), encoding="utf-8"))
    b = json.load(open(os.path.join(new, "package.json"), encoding="utf-8"))
    out = []
    for k in sorted(set(a) | set(b)):
        if k == "version" or a.get(k) == b.get(k):
            continue
        out.append((k, f"{json.dumps(a.get(k))} -> {json.dumps(b.get(k))}"))
    return out


def jar_compare(npm_jar, ref_jar):
    with zipfile.ZipFile(npm_jar) as za, zipfile.ZipFile(ref_jar) as zb:
        ia = {i.filename: i for i in za.infolist()}
        ib = {i.filename: i for i in zb.infolist()}
        differ = [n for n in sorted(set(ia) & set(ib))
                  if (ia[n].CRC, ia[n].file_size) != (ib[n].CRC, ib[n].file_size)]
    return differ, sorted(set(ia) - set(ib)), sorted(set(ib) - set(ia))


# ---- switch-default -------------------------------------------------------------------------------
# Files that name the default reference. Not rewritten: crates/** (scripts/license_headers.py
# writes the headers), LICENSES/, the generated corpus files (finalize.py, the recorder), the
# ratchets, the Java sources that cite the commit they were written against (oracle/), the
# decision record, and the release files that carry closure-rs' own version
# (npm/config.json, npm/closure-rs/package.json, .github/ISSUE_TEMPLATE/), which a release changes.
DOC_EXCLUDE = re.compile(r"^(crates/|LICENSES/|fuzz/|DECISIONS\.md$|corpus/unit/(records|derived|descriptors|postcall)/"
                         r"|corpus/d2/(?!PROFILES\.md$)|gates/[^/]*\.json$|oracle/(src|replay|patches)/"
                         r"|npm/config\.json$|npm/closure-rs/package\.json$|\.github/ISSUE_TEMPLATE/"
                         r"|scripts/references\.tsv$|npm/closure-rs/index\.d\.c?ts$)")
DOC_SUFFIX = (".md", ".sh", ".py", ".mjs", ".js", ".yml", ".json", ".java", ".txt")


def doc_files():
    out = subprocess.run(["git", "ls-files"], cwd=CHECKOUT, capture_output=True, text=True, check=True).stdout
    return [p for p in out.splitlines()
            if (p.endswith(DOC_SUFFIX) or p == "NOTICE") and not DOC_EXCLUDE.match(p)
            and os.path.isfile(os.path.join(CHECKOUT, p))]


def ref_date(src):
    return subprocess.run(["git", "log", "-1", "--format=%cs"], cwd=src, capture_output=True, text=True,
                          check=True).stdout.strip()


def doc_mapping(old, new, older):
    """[(regex, replacement)] applied in one pass: the old reference's tag, commit (full, 9 and 7
    digits), jar sha256 (full, 8 digits, golden tag ref-<8>) and npm version in the forms the
    docs use (google-closure-compiler@V, its npm page, ^V, V.x.y, V.1.0-exp, `V`). Other versions, such as a
    benchmark's closure-rs version, stay."""
    o, n = old.tag[1:], new.tag[1:]
    hx = "0-9a-f"
    return [(re.escape(old.commit), new.commit), (re.escape(old.jar_sha256), new.jar_sha256),
            (rf"(?<![{hx}]){re.escape(old.commit[:9])}(?![{hx}])", new.commit[:9]),
            (rf"(?<![{hx}]){re.escape(old.commit[:7])}(?![{hx}])", new.commit[:7]),
            (rf"(?<![{hx}]){re.escape(old.jar_sha256[:8])}(?![{hx}])", new.jar_sha256[:8]),
            (rf"\bv{o}\b", f"v{n}"),
            (rf"(?<=google-closure-compiler@){o}(?=\.0\.0)", n),
            (rf"(?<=google-closure-compiler ){o}(?=\.0\.0)", n),
            (rf"(?<=google-closure-compiler/v/){o}(?=\.0\.0)", n),
            (rf"(?<=\^){o}(?=\.0\.0)", n), (rf"\b{o}(?=\.x\.y|\.1\.0-exp)", n), (rf"`{o}`", f"`{n}`")]


def rewrite_doc(path, text, old, new, older, npm_version, dates):
    mapping = doc_mapping(old, new, older)
    if path in ("NOTICE", "docs/PORTING.md"):  # the commit date next to the commit
        mapping.append((rf"\({re.escape(dates[0])}(?=[;)])", f"({dates[1]}"))
    rx = re.compile("|".join(f"({p})" for p, _ in mapping))
    # A line that names an older reference's golden store as the example of an older store keeps
    # an older example: the old reference's.
    rot = None
    if older and older.jar_sha256 != "-":
        rot = re.compile(rf"(?<![0-9a-f]){re.escape(older.jar_sha256[:8])}(?![0-9a-f])")

    def sub(m):
        return mapping[m.lastindex - 1][1]

    out, inserted = [], False
    for line in text.split("\n"):
        if path == "oracle/REFERENCE.md" and re.match(r"^\| `[^`]+`( \(default\))? \| ", line):
            # the table of every reference keeps its rows; the new one goes first
            if not inserted:
                out.append(f"| `{new.tag}` (default) | release `{new.tag}`, `{new.commit}`, npm `{npm_version}` | "
                           f"`{new.src}` | `{new.jar}`, `{new.jar_sha256}` | `{new.oracle_jar}` |")
                inserted = True
            out.append(line.replace(" (default) |", " |", 1))
            continue
        line = rx.sub(sub, line)
        if rot and "older" in line:
            line = rot.sub(old.jar_sha256[:8], line)
        out.append(line)
    return "\n".join(out)


def print_unit_worklist(s):
    rp = s.get("replayed", {})
    log(f"  replayed {rp.get('records')} records ({rp.get('classes') if rp.get('classes') == 'all' else len(rp.get('classes', []))} "
        f"classes): {rp.get('pass')} pass, {rp.get('fail')} fail, {rp.get('unported')} unported")
    log(f"  old ratchet: {s['oldPassing']} passing; {s['mapped']} still recorded, {s.get('stillPassing')} still pass; "
        f"{len(s['lost'])} lost (no longer recorded), {len(s.get('regressed', []))} regressed")
    new = s.get("newRecords", [])
    log(f"  {len(new)} new records: {sum(1 for x in new if x['status'] == 'pass')} pass, "
        f"{sum(1 for x in new if x['status'] == 'unported')} unported, "
        f"{sum(1 for x in new if x['status'] not in ('pass', 'unported'))} fail")
    for x in s.get("regressed", [])[:30]:
        log(f"    regressed {x['id']} -> {x['newId']}: {x['status']} {x['why'][:120]}")
    for x in s["lost"][:30]:
        log(f"    lost {x['id']} {x['method']}")


STEPS = [
    ("resolve", Sync.step_resolve), ("registry", Sync.step_registry), ("fetch", Sync.step_fetch),
    ("jar", Sync.step_jar), ("npm-check", Sync.step_npm_check), ("oracle", Sync.step_oracle),
    ("resources", Sync.step_resources), ("flags", Sync.step_flags),
    ("license-headers", Sync.step_license_headers), ("worklist", Sync.step_worklist),
    ("unit-ws", Sync.step_unit_ws), ("unit-jars", Sync.step_unit_jars), ("unit-record", Sync.step_unit_record),
    ("unit-install", Sync.step_unit_install), ("unit-worklist", Sync.step_unit_worklist),
    ("d2-golden", Sync.step_d2_golden), ("d2-validate", Sync.step_d2_validate),
    ("d2-worklist", Sync.step_d2_worklist), ("switch-default", Sync.step_switch_default),
]
EXPLICIT_ONLY = {"switch-default"}

LEFT = """\
Left for a person or agent (docs/PORTING.md §9, §7):
  1. Port the Java changes: {work}/porting-worklist.md lists every changed upstream file with the
     Rust files ported from it, and upstream.diff holds the diff. New or changed upstream tests:
     unit records (descriptors for new record classes), Rust test ports for non-harness classes.
  2. Triage the regression worklists: {work}/unit-worklist.json (unit records) and
     {work}/d2-worklist.txt (D2 pairs) list what the new reference breaks before porting.
  3. Re-run after porting: --only license-headers (headers of newly ported files), gates/ci.sh,
     gates/unit_rust.sh --update and gates/d2_rust.py (new ratchets), gates/gate_0_1.sh with
     CLOSURE_RS_REF={tag} (the oracle against the jar), gates/gate_0_2.sh, targeted probes and a
     differential fuzz run against the new reference.
  4. When everything matches: --only switch-default (read its diff), then the counts the docs
     quote (records in corpus/unit/RECORDING.md) and the pull request."""


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("tag", nargs="?", help="the upstream release tag, vYYYYMMDD")
    ap.add_argument("--list", action="store_true", help="list the steps and exit")
    ap.add_argument("--from", dest="from_step", metavar="STEP", help="start at STEP")
    ap.add_argument("--only", metavar="STEP[,STEP]", help="run only these steps")
    ap.add_argument("--dry-run", action="store_true", help="run only the read-only checks")
    ap.add_argument("--base", help="the reference to diff against (default: the registry's default)")
    ap.add_argument("--work", help="work directory (default <main checkout>/build/upstream-sync/<tag>)")
    ap.add_argument("--jobs", type=int, help="parallel jobs for the recording and the D2 runs")
    ap.add_argument("--unit-classes", metavar="A,B", help="record, install and replay only these test "
                                                          "classes (a partial run, for testing)")
    ap.add_argument("--d2-sample", type=int, metavar="N", help="d2-worklist on a sample of N pairs (no rebase)")
    a = ap.parse_args()
    names = [n for n, _ in STEPS]
    if a.list:
        for n, f in STEPS:
            doc = " ".join(f.__doc__.split())
            log(f"{n:16} {doc}" + (" [explicit only]" if n in EXPLICIT_ONLY else ""))
        return 0
    if not a.tag:
        ap.error("the tag is required")
    if a.only:
        sel = a.only.split(",")
        bad = [s for s in sel if s not in names]
        if bad:
            ap.error(f"unknown step(s) {bad}; steps: {', '.join(names)}")
    else:
        if a.from_step and a.from_step not in names:
            ap.error(f"unknown step {a.from_step}; steps: {', '.join(names)}")
        start = names.index(a.from_step) if a.from_step else 0
        sel = [n for n in names[start:] if n not in EXPLICIT_ONLY]
    s = Sync(a)
    log(f"upstream sync {a.tag}: checkout {CHECKOUT}, main checkout {ROOT}, work {s.work}"
        + (" (dry run)" if a.dry_run else ""))
    results = []
    for n, f in STEPS:
        if n not in sel:
            continue
        log(f"== {n}: {' '.join(f.__doc__.split())}")
        try:
            results.append((n, f(s)))
        except StepError as e:
            log(f"  FAILED: {e}")
            results.append((n, "FAILED"))
            break
    log("\n== summary")
    for n, r in results:
        log(f"  {n:16} {'already done' if r == 'done' else 'would run' if a.dry_run and r.startswith('ran') else r}")
    for step, text in s.notes:
        log(f"  note ({step}): {text}")
    log("")
    log(LEFT.format(work=rel(s.work), tag=a.tag))
    return 1 if results and results[-1][1] == "FAILED" else 0


if __name__ == "__main__":
    sys.exit(main())
