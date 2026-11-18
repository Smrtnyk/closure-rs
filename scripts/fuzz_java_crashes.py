#!/usr/bin/env python3
"""Replays the Java crashes a fuzz run kept under two reference jars.

A program on which Java crashes (exit 254 or an internal compiler error) is not compared
(docs/PORTING.md §4.4); the fuzz driver keeps it as build/fuzz/java-crashes/<seed>-<index>-<profile>/
(its .js files and origin.txt). After a fuzz run against a new reference, this script runs each
kept program through the CLI of the previous and of the new reference jar, with its profile and its
exact flag list (gates/lib/fuzz_gate03.py `kept_crash`: origin.txt's second line), and reports per
program whether it crashes under both jars, only under one, or under neither:

  python3 scripts/fuzz_java_crashes.py [DIR] [--seed SEED] [--old JAR|TAG] [--new JAR|TAG]
                                       [--jobs N] [--xmx SIZE] [--timeout S] [--json FILE]

DIR is a directory of kept crashes (default build/fuzz/java-crashes of this checkout) or one kept
crash; --seed keeps the programs of runs with that seed. --old and --new take a reference tag of
scripts/references.tsv or a jar path; the defaults are the registry row before the selected
reference and the selected reference ($CLOSURE_RS_REF, else the registry's default). Each run uses
the argv of gates/lib/case_args.py, the JVM flags and environment of the D2 golden runs
(gates/lib/run_reference.py, without the CDS archive) and a copy of the program under
build/fuzz/java-crash-replay/, with cwd = this checkout.

Exit 0 when every program crashes under both jars (the crashes are Java's own, unchanged by the
new reference), 1 when one does not (a crash the new reference added or fixed, or one that does
not reproduce: listed), 2 on bad input.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)  # this checkout: argv paths are relative to it
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "gates", "lib"))
import paths  # noqa: E402  the registry and ROOT (tools/, build/)
import case_args  # noqa: E402
import fuzz_gate03  # noqa: E402  kept_crash, is_java_crash, exception_lines
import run_reference  # noqa: E402  jvm_flags, child_env

JAVA = os.path.join(paths.ROOT, "tools", "jdk-21", "bin", "java")
SCRATCH = os.path.join(REPO, "build", "fuzz", "java-crash-replay")
VERDICTS = ("both", "only old", "only new", "neither")


def die(msg):
    print(f"fuzz_java_crashes.py: {msg}", file=sys.stderr)
    sys.exit(2)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def resolve_jar(spec):
    """(label, absolute jar path) for a registry tag or a jar path."""
    refs, _ = paths.load_registry()
    if spec in refs:
        r = refs[spec]
        jar = os.path.join(paths.ROOT, r.jar)
        if not os.path.isfile(jar):
            die(f"{spec}: no jar at {jar}")
        if r.jar_sha256 != "-" and sha256_file(jar) != r.jar_sha256:
            die(f"{spec}: {jar} does not have the registry's sha256")
        return spec, jar
    if os.path.isfile(spec):
        return spec, os.path.abspath(spec)
    die(f"{spec} is neither a tag of scripts/references.tsv nor a jar")


def previous_tag(tag):
    tags = list(paths.load_registry()[0])
    i = tags.index(tag)
    if i == 0:
        die(f"no reference before {tag} in scripts/references.tsv; pass --old")
    return tags[i - 1]


def name_key(name):
    return [int(t) if t.isdigit() else t for t in re.split(r"(\d+)", name)]


def programs(d, seed):
    """The kept crash directories under d (or d itself), in seed and index order."""
    if os.path.isfile(os.path.join(d, "origin.txt")):
        return [d]
    if not os.path.isdir(d):
        die(f"no directory {d}")
    return [os.path.join(d, n) for n in sorted(os.listdir(d), key=name_key)
            if os.path.isfile(os.path.join(d, n, "origin.txt")) and (seed is None or n.startswith(f"{seed}-"))]


def run_one(prog, kept, jar, a, profiles):
    """One CLI run of a kept program: {"exit", "crash", "exception"} (or "timeout" / "error")."""
    os.makedirs(SCRATCH, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
        rel = os.path.relpath(tmp, REPO)
        for f in kept["files"]:
            shutil.copyfile(os.path.join(prog, f), os.path.join(tmp, f))
        os.makedirs(os.path.join(tmp, "out"))
        case = {"id": os.path.basename(prog), "inputs": [f"{rel}/{f}" for f in kept["files"]], "externs": [],
                "shims": [], "extra_flags": kept["extra_flags"], "profiles": [kept["profile"]]}
        try:
            args = case_args.compiler_args(case, kept["profile"], out_dir=f"{rel}/out", profiles=profiles)
        except case_args.CaseProfileError as e:
            return {"exit": None, "crash": False, "error": str(e)}
        cmd = [JAVA, *run_reference.jvm_flags(a.xmx, use_cds=False), "-jar", jar, *args]
        try:
            r = subprocess.run(cmd, cwd=REPO, env=run_reference.child_env(), stdin=subprocess.DEVNULL,
                               capture_output=True, timeout=a.timeout)
        except subprocess.TimeoutExpired:
            return {"exit": None, "crash": False, "timeout": True}
    err = r.stderr.decode("utf-8", "replace")
    crash = fuzz_gate03.is_java_crash(r.returncode, err)
    out = {"exit": r.returncode, "crash": crash}
    if crash:
        out["exception"] = fuzz_gate03.exception_class(fuzz_gate03.exception_lines(err))
    return out


def replay(prog, jars, a, profiles):
    try:
        kept = fuzz_gate03.kept_crash(prog)
    except (OSError, ValueError) as e:
        return {"dir": os.path.basename(prog), "error": str(e), "verdict": "neither"}
    row = {"dir": os.path.basename(prog), "origin": kept["origin"], "profile": kept["profile"],
           "extra_flags": kept["extra_flags"]}
    for side, (_, jar) in jars.items():
        row[side] = run_one(prog, kept, jar, a, profiles)
    old, new = row["old"]["crash"], row["new"]["crash"]
    row["verdict"] = VERDICTS[0 if old and new else 1 if old else 2 if new else 3]
    return row


def outcome(r):
    if r.get("timeout"):
        return "timeout"
    if "error" in r:
        return "error"
    return str(r["exit"])


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir", nargs="?", default=os.path.join(REPO, fuzz_gate03.CRASH_DIR),
                    help="kept Java crashes, or one of them (default build/fuzz/java-crashes)")
    ap.add_argument("--seed", help="only the programs of runs with this seed (<seed>-*)")
    ap.add_argument("--old", help="tag or jar (default: the registry row before the selected reference)")
    ap.add_argument("--new", help="tag or jar (default: the selected reference)")
    ap.add_argument("--jobs", type=int, default=4, help="programs replayed in parallel (default 4)")
    ap.add_argument("--xmx", default=run_reference.DEFAULT_XMX, help="JVM heap (default %(default)s)")
    ap.add_argument("--timeout", type=int, default=run_reference.DEFAULT_TIMEOUT_S,
                    help="seconds per run (default %(default)s)")
    ap.add_argument("--json", metavar="FILE", help="also write every result to FILE")
    a = ap.parse_args()
    sel = paths.REF_TAG
    jars = {"old": resolve_jar(a.old or previous_tag(sel)), "new": resolve_jar(a.new or sel)}
    progs = programs(a.dir, a.seed)
    if not progs:
        die(f"no kept Java crashes in {a.dir}" + (f" for seed {a.seed}" if a.seed else ""))
    profiles = case_args.load_profiles()
    print(f"replaying {len(progs)} kept Java crashes from {os.path.relpath(a.dir, REPO)}"
          + (f" (seed {a.seed})" if a.seed else ""))
    for side, (label, jar) in jars.items():
        print(f"  {side}: {label} ({jar})")
    with ThreadPoolExecutor(max(1, a.jobs)) as ex:
        rows = list(ex.map(lambda p: replay(p, jars, a, profiles), progs))
    try:
        os.rmdir(SCRATCH)
    except OSError:
        pass
    for r in rows:
        if "error" in r:
            print(f"  {'neither':9} {r['dir']}: {r['error']}")
            continue
        exc = r["new"].get("exception") or r["old"].get("exception") or ""
        print(f"  {r['verdict']:9} {r['dir']:32} exit {outcome(r['old'])}/{outcome(r['new'])}  {exc}"
              + "".join(f"  {s}: {r[s]['error']}" for s in ("old", "new") if "error" in r[s]))
    counts = {v: sum(1 for r in rows if r["verdict"] == v) for v in VERDICTS}
    print(f"{len(rows)} programs: {counts['both']} crash under both jars, {counts['only old']} only under old "
          f"({jars['old'][0]}), {counts['only new']} only under new ({jars['new'][0]}), {counts['neither']} under neither")
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump({"old": jars["old"][0], "new": jars["new"][0], "counts": counts, "programs": rows}, f, indent=1)
            f.write("\n")
    return 0 if counts["both"] == len(rows) else 1


if __name__ == "__main__":
    sys.exit(main())
