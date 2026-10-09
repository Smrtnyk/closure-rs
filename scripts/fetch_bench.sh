#!/usr/bin/env bash
# closure-rs: fetch the D7 benchmark projects (bench/projects.json) into bench-cache/ and verify them.
#
#   scripts/fetch_bench.sh [--cache-dir DIR] [--project P]... [--check] [--print-hashes]
#
# Every source of every project is fetched at its pinned commit:
#   - kind "git": a shallow (--depth 1), blobless, sparse fetch of exactly the pinned commit, of only
#     the source's "paths" (git sparse-checkout patterns); the files are copied into
#     bench-cache/<dest>/ and the temporary clone is removed (no .git in bench-cache);
#   - kind "npm-tarball": the release tarball is downloaded, its sha256 and npm integrity (sha512)
#     are checked, and only the listed "paths" (regular files) are extracted into
#     bench-cache/<dest>/. Nothing is installed or executed.
#   - kind "closure-map": a bundle with an input source map, made from already fetched files
#     ("inputs", paths relative to the repository root) by the pinned Java reference compiler
#     (the jar of the D2 golden pipeline, never a downloaded tool): for each input,
#     WHITESPACE_ONLY + PRETTY_PRINT with --create_source_map and --source_map_include_content
#     (and --jscomp_off=moduleLoad: each input is re-emitted alone, its imports stay as they are)
#     writes bench-cache/<dest>/<input basename> and <input basename>.map. The output is
#     deterministic; its tree hash is pinned like a fetched tree's.
# Then the tree hash of bench-cache/<dest>/ is checked against "tree_sha256": sha256 over the
# sorted lines "<path relative to dest>\0<sha256 of the file>\n" of every regular file in it.
#
# Idempotent: a destination whose tree hash already matches is left alone; a wrong or partial one
# is deleted and fetched again. --check fetches nothing and only verifies. --print-hashes prints
# the hashes found (used to pin a new source) instead of failing on a mismatch.
# Exit code: 0 = everything verified, 1 = a hash mismatch or fetch failure.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 -I - "$ROOT" "$@" <<'PY'
import argparse, base64, hashlib, io, json, os, shutil, subprocess, sys, tarfile, tempfile
import urllib.request

ROOT = sys.argv[1]
ap = argparse.ArgumentParser(prog="scripts/fetch_bench.sh")
ap.add_argument("--cache-dir", default=os.path.join(ROOT, "bench-cache"))
ap.add_argument("--project", action="append", default=[])
ap.add_argument("--check", action="store_true", help="verify only, fetch nothing")
ap.add_argument("--print-hashes", action="store_true")
a = ap.parse_args(sys.argv[2:])

with open(os.path.join(ROOT, "bench/projects.json"), encoding="utf-8") as f:
    spec = json.load(f)
cache = os.path.abspath(a.cache_dir)
names = [p["name"] for p in spec["projects"]]
for p in a.project:
    if p not in names:
        sys.exit(f"unknown project {p!r}; known: {', '.join(names)}")


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for blk in iter(lambda: f.read(1 << 20), b""):
            h.update(blk)
    return h.hexdigest()


def tree_hash(d):
    """None if d is missing; raises on a symlink or special file (never produced by a fetch)."""
    if not os.path.isdir(d):
        return None
    lines = []
    for dp, dns, fns in os.walk(d):
        dns.sort()
        for fn in fns:
            p = os.path.join(dp, fn)
            if os.path.islink(p) or not os.path.isfile(p):
                raise RuntimeError(f"not a regular file: {p}")
            rel = os.path.relpath(p, d).replace(os.sep, "/")
            lines.append(f"{rel}\0{sha256_file(p)}\n")
    lines.sort()
    return hashlib.sha256("".join(lines).encode()).hexdigest()


def git(*args, cwd=None):
    subprocess.run(["git", *args], cwd=cwd, check=True, stdin=subprocess.DEVNULL,
                   env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})


def fetch_git(src, dest):
    tmp = tempfile.mkdtemp(prefix=".fetch-", dir=cache)
    try:
        git("init", "-q", tmp)
        git("remote", "add", "origin", src["repo"], cwd=tmp)
        git("config", "core.sparseCheckout", "true", cwd=tmp)
        # non-cone patterns, anchored at the repository root
        pats = "".join(f"/{p}\n" for p in src["paths"])
        os.makedirs(os.path.join(tmp, ".git", "info"), exist_ok=True)
        with open(os.path.join(tmp, ".git", "info", "sparse-checkout"), "w") as f:
            f.write(pats)
        git("fetch", "-q", "--depth", "1", "--filter=blob:none", "--no-tags", "origin",
            src["commit"], cwd=tmp)
        git("-c", "advice.detachedHead=false", "checkout", "-q", "FETCH_HEAD", cwd=tmp)
        head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=tmp, check=True,
                              capture_output=True, text=True).stdout.strip()
        if head != src["commit"]:
            raise RuntimeError(f"{src['name']}: fetched {head}, pinned {src['commit']}")
        shutil.rmtree(os.path.join(tmp, ".git"))
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        os.rename(tmp, dest)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def generate_closure_map(src, dest):
    sys.path.insert(0, os.path.join(ROOT, "scripts"))
    import run_bench  # noqa: E402  the Java reference: data root, JDK, jar
    droot = run_bench.data_root(None)
    java = os.path.join(droot, run_bench.JAVA_REL)
    jar = os.path.join(droot, run_bench.JAR_REL)
    if run_bench.sha256_file(jar) != run_bench.JAR_SHA256:
        raise RuntimeError(f"{jar}: not the pinned reference jar")
    for rel in src["inputs"]:
        if not os.path.isfile(os.path.join(ROOT, rel)):
            raise RuntimeError(f"{rel} is missing: fetch the project it comes from first")
    os.makedirs(dest)
    out_dir = os.path.relpath(dest, ROOT)
    try:
        for rel in src["inputs"]:
            out = f"{out_dir}/{os.path.basename(rel)}"
            subprocess.run([java, "-jar", jar, "--compilation_level=WHITESPACE_ONLY",
                            "--language_in=ECMASCRIPT_NEXT", "--language_out=NO_TRANSPILE",
                            "--formatting=PRETTY_PRINT", "--jscomp_off=moduleLoad",
                            f"--js={rel}", f"--js_output_file={out}",
                            f"--create_source_map={out}.map", "--source_map_include_content"],
                           cwd=ROOT, check=True, stdin=subprocess.DEVNULL,
                           stdout=subprocess.DEVNULL, env=run_bench.child_env())
    except BaseException:
        shutil.rmtree(dest, ignore_errors=True)
        raise


def fetch_tarball(src, dest):
    dl = os.path.join(cache, ".downloads")
    os.makedirs(dl, exist_ok=True)
    tgz = os.path.join(dl, os.path.basename(src["url"]))
    if not (os.path.isfile(tgz) and sha256_file(tgz) == src["tarball_sha256"]):
        with urllib.request.urlopen(src["url"], timeout=120) as r:
            data = r.read()
        with open(tgz, "wb") as f:
            f.write(data)
    data = open(tgz, "rb").read()
    got = hashlib.sha256(data).hexdigest()
    alg, want = src["integrity"].split("-", 1)
    integ = base64.b64encode(hashlib.new(alg, data).digest()).decode()
    if integ != want:
        raise RuntimeError(f"{src['name']}: npm integrity mismatch ({alg})")
    if a.print_hashes:
        print(f"{src['name']}\ttarball_sha256\t{got}")
    elif got != src["tarball_sha256"]:
        raise RuntimeError(f"{src['name']}: tarball sha256 {got}, pinned {src['tarball_sha256']}")
    tmp = tempfile.mkdtemp(prefix=".fetch-", dir=cache)
    try:
        wanted = set(src["paths"])
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tf:
            for m in tf.getmembers():
                rel = m.name.split("/", 1)[1] if "/" in m.name else ""
                if rel not in wanted or not m.isreg():
                    continue
                wanted.discard(rel)
                out = os.path.join(tmp, *rel.split("/"))
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with open(out, "wb") as f:
                    f.write(tf.extractfile(m).read())
        if wanted:
            raise RuntimeError(f"{src['name']}: not in the tarball: {sorted(wanted)}")
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        os.rename(tmp, dest)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


os.makedirs(cache, exist_ok=True)
bad = 0
for proj in spec["projects"]:
    if a.project and proj["name"] not in a.project:
        continue
    for src in proj["sources"]:
        dest = os.path.join(cache, src["dest"])
        label = f"{proj['name']}/{src['name']}@{src['ref']}"
        try:
            have = tree_hash(dest)
            if have is not None and have == src["tree_sha256"] and not a.print_hashes:
                print(f"ok      {label}")
                continue
            if a.check:
                print(f"BAD     {label}: {'missing' if have is None else 'tree hash ' + have}")
                bad += 1
                continue
            shutil.rmtree(dest, ignore_errors=True)
            {"git": fetch_git, "npm-tarball": fetch_tarball,
             "closure-map": generate_closure_map}[src["kind"]](src, dest)
            have = tree_hash(dest)
            if a.print_hashes:
                print(f"{src['name']}\ttree_sha256\t{have}")
            elif have != src["tree_sha256"]:
                print(f"BAD     {label}: tree hash {have}, pinned {src['tree_sha256']}")
                bad += 1
            else:
                print(f"fetched {label}")
        except (OSError, RuntimeError, subprocess.CalledProcessError) as e:
            print(f"FAILED  {label}: {e}", file=sys.stderr)
            bad += 1
sys.exit(1 if bad else 0)
PY
