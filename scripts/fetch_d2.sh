#!/usr/bin/env bash
# closure-rs: fetch the D2 real-world differential corpus (docs/PORTING.md §4.4) and verify it.
#
#   scripts/fetch_d2.sh [--cache-dir DIR] [--source S]... [--group G]... [--check] [--jobs N]
#                       [--list-groups]
#
# Reads corpus/d2/sources.lock.json and re-creates every input in corpus-cache/d2/ (or DIR):
#   - npm packages and whole-program tarballs from registry.npmjs.org: the tarball's sha256, npm
#     integrity (sha512) and shasum (sha1) are checked, then it is unpacked (first path component
#     stripped into <dir>/package/, regular files only; nothing is installed or executed);
#   - Closure Library, test262 and whole-program test files from raw.githubusercontent.com at the
#     pinned commits;
#   - closure-self inputs are NOT downloaded: they are verified in place in the pinned reference
#     checkout (reference/closure-compiler, scripts/fetch_reference.sh), like the committed shims
#     under corpus/d2/shims/.
# Every file listed in the lock (or in the selected groups) must end up with its recorded sha256;
# the script exits 1 otherwise. Files already present with the right hash are not downloaded
# again, so the script is resumable. --check downloads nothing and only verifies.
#
# Lock paths are repo-relative and start with "corpus-cache/d2/"; with --cache-dir that prefix is
# mapped to DIR (used to prove the script into a fresh directory). Compilation and the golden
# results always use corpus-cache/d2/, because the input paths are part of the compiler argv.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 - "$ROOT" "$@" <<'PY'
import argparse, base64, concurrent.futures as cf, hashlib, io, json, os, subprocess, sys, tarfile
import time, urllib.parse, urllib.request

ROOT = sys.argv[1]
ap = argparse.ArgumentParser(prog="scripts/fetch_d2.sh")
ap.add_argument("--cache-dir", default="corpus-cache/d2")
ap.add_argument("--source", action="append", default=[])
ap.add_argument("--group", action="append", default=[])
ap.add_argument("--check", action="store_true", help="verify only, download nothing")
ap.add_argument("--jobs", type=int, default=12)
ap.add_argument("--list-groups", action="store_true")
a = ap.parse_args(sys.argv[2:])

LOCK_PATH = os.path.join(ROOT, "corpus/d2/sources.lock.json")
with open(LOCK_PATH, encoding="utf-8") as f:
    lock = json.load(f)
ALLOWED = set(lock["allowed_hosts"])
PREFIX = "corpus-cache/d2/"
cache = a.cache_dir.rstrip("/")
cache_abs = cache if os.path.isabs(cache) else os.path.join(ROOT, cache)

if a.list_groups:
    for g, v in lock["groups"].items():
        print(f"{g}\t{','.join(v['sources'])}\t{len(v['cases'])} cases\t{len(v['files'])} files")
    sys.exit(0)


def local(p):
    """Where a lock path lives on disk."""
    if p.startswith(PREFIX):
        return os.path.join(cache_abs, p[len(PREFIX):])
    return os.path.join(ROOT, p)


def sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for blk in iter(lambda: f.read(1 << 20), b""):
            h.update(blk)
    return h.hexdigest()


def ok(p, want):
    lp = local(p)
    return os.path.isfile(lp) and sha256_file(lp) == want


def get(url):
    host = urllib.parse.urlparse(url).hostname
    if host not in ALLOWED:
        raise RuntimeError(f"refusing to download from {host}: {url}")
    last = None
    for attempt in range(1, 5):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "closure-rs-fetch_d2/1"})
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001 - retried, then re-raised
            last = e
            time.sleep(1.5 * attempt)
    raise RuntimeError(f"{url}: {last!r}")


def write_atomic(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = f"{path}.tmp.{os.getpid()}"
    with open(tmp, "wb") as f:
        f.write(data)
    os.chmod(tmp, 0o644)
    os.replace(tmp, path)


def safe_extract(tgz, dest):
    """Unpack an npm tarball into dest/package/, stripping the first path component. Only regular
    files and directories; links, devices and paths escaping dest are skipped."""
    real = os.path.realpath(dest)
    n = 0
    with tarfile.open(fileobj=io.BytesIO(tgz), mode="r:gz") as tf:
        for m in tf.getmembers():
            parts = [p for p in m.name.replace("\\", "/").split("/")[1:] if p not in ("", ".")]
            if not parts or ".." in parts:
                continue
            target = os.path.join(dest, "package", *parts)
            if not os.path.realpath(target).startswith(real + os.sep):
                continue
            if m.isdir():
                os.makedirs(target, exist_ok=True)
            elif m.isfile():
                f = tf.extractfile(m)
                write_atomic(target, f.read() if f else b"")
                n += 1
    return n


# ------------------------------------------------------------------ selection
files = lock["files"]
if a.group:
    unknown = [g for g in a.group if g not in lock["groups"]]
    if unknown:
        sys.exit(f"unknown group(s): {unknown} (see --list-groups)")
sel = set()
tarballs = set()
if a.group or a.source:
    for g, v in lock["groups"].items():
        if g in a.group or set(v["sources"]) & set(a.source):
            sel.update(v["files"])
            tarballs.update(v["tarballs"])
    for p, e in files.items():  # source-wide extras (e.g. files no case references directly)
        if e["source"] in a.source:
            sel.add(p)
else:
    sel = set(files)
for p in sel:
    fe = files[p]["fetch"]
    if "tarball" in fe:
        tarballs.add(fe["tarball"])
    if "tarball_file" in fe:
        tarballs.add(fe["tarball_file"])
print(f"[fetch_d2] cache={cache} files={len(sel)} tarballs={len(tarballs)} "
      f"mode={'check' if a.check else 'fetch'}", flush=True)

# ------------------------------------------------------------------ download
errors = []


def fetch_tarball(key):
    t = lock["tarballs"][key]
    mine = [p for p, e in files.items() if e["fetch"].get("tarball") == key or e["fetch"].get("tarball_file") == key]
    if all(ok(p, files[p]["sha256"]) for p in mine):
        return f"cached {key}"
    tgz = get(t["url"])
    if sha256_bytes(tgz) != t["sha256"]:
        raise RuntimeError(f"{key}: tarball sha256 mismatch")
    algo, b64 = t["integrity"].split("-", 1)
    if base64.b64encode(hashlib.new(algo, tgz).digest()).decode() != b64:
        raise RuntimeError(f"{key}: npm integrity mismatch")
    if t.get("shasum") and hashlib.sha1(tgz).hexdigest() != t["shasum"]:
        raise RuntimeError(f"{key}: npm shasum mismatch")
    d = local(t["dir"])
    n = safe_extract(tgz, d)
    if t.get("keep_tarball_as"):
        write_atomic(local(t["keep_tarball_as"]), tgz)
    return f"fetched {key} ({len(tgz)} bytes, {n} files)"


def fetch_file(p):
    e = files[p]
    if ok(p, e["sha256"]):
        return None
    data = get(e["fetch"]["url"])
    if sha256_bytes(data) != e["sha256"]:
        raise RuntimeError(f"{p}: sha256 mismatch after download")
    write_atomic(local(p), data)
    return p


if not a.check:
    url_files = sorted(p for p in sel if "url" in files[p]["fetch"])
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(fetch_tarball, k): k for k in sorted(tarballs)}
        futs.update({ex.submit(fetch_file, p): p for p in url_files})
        done = 0
        for fu in cf.as_completed(futs):
            done += 1
            try:
                fu.result()
            except Exception as e:  # noqa: BLE001
                errors.append(f"{futs[fu]}: {e}")
            if done % 500 == 0:
                print(f"[fetch_d2] {done}/{len(futs)}", flush=True)

# ------------------------------------------------------------------ verify
bad = []
by_src = {}
for p in sorted(sel):
    e = files[p]
    s = by_src.setdefault(e["source"], [0, 0])
    s[0] += 1
    lp = local(p)
    if not os.path.isfile(lp):
        bad.append(f"missing  {p}")
    elif sha256_file(lp) != e["sha256"]:
        bad.append(f"MISMATCH {p}")
    else:
        s[1] += 1
cases_path = os.path.join(ROOT, lock["cases"]["path"])
if not a.group and not a.source and sha256_file(cases_path) != lock["cases"]["sha256"]:
    bad.append(f"MISMATCH {lock['cases']['path']} (cases.jsonl differs from the lock)")
ref = lock["sources"]["closure-self"]
if any(files[p]["source"] == "closure-self" for p in sel):
    try:
        head = subprocess.run(["git", "-C", os.path.join(ROOT, ref["checkout"]), "rev-parse", "HEAD"],
                              capture_output=True, text=True, check=True).stdout.strip()
        if head != ref["commit"]:
            bad.append(f"reference checkout at {head}, lock pins {ref['commit']}")
    except (OSError, subprocess.CalledProcessError) as e:
        print(f"[fetch_d2] note: could not read the reference checkout's commit ({e}); "
              "closure-self files are still verified by sha256", flush=True)
for s, (n, good) in sorted(by_src.items()):
    print(f"[fetch_d2] {s}: {good}/{n} files verified", flush=True)
for x in errors + bad:
    print("[fetch_d2] ERROR " + x, flush=True)
print(f"[fetch_d2] {'OK' if not errors and not bad else 'FAILED'}: {len(sel) - len(bad)}/{len(sel)} files "
      f"match sources.lock.json", flush=True)
sys.exit(1 if errors or bad else 0)
PY
