#!/usr/bin/env python3
"""Run one D2 (case, profile) through the pinned reference uberjar and store the result.

Usage:
  python3 gates/lib/run_reference.py --case <case-id> --profile <profile> [--force]
         [--result PATH] [--keep-out] [--timeout 180] [--xmx 3g]

The compiler argv comes from case_args.compiler_args() (shared with every later gate).
The process runs with cwd = repository root, repo-relative paths only, and a minimal fixed
environment.  The result is written atomically to
  corpus-cache/d2/_golden/<golden tag>/<case-id>/<profile>.json
(golden tag ref-<first 8 hex of the jar sha256>, ref-cfa8886f for the default reference; the jar
and its sha256 are the scripts/references.tsv row $CLOSURE_RS_REF, docs/PORTING.md §9)
as {args, compiler_args, exit_code, timed_out, stdout, stderr, outputs, wall_ms,
peak_rss_kb, ...}.  Text that is not valid UTF-8 is stored as {"base64": "..."}.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "scripts"))
import case_args  # noqa: E402
import paths  # noqa: E402  the reference (scripts/paths.py)

REPO = case_args.REPO
RUNNER_VERSION = 2  # v2: peak_rss_kb measured via rss_spawn.py (v1 was polluted by driver RSS)
JAVA = "tools/jdk-21/bin/java"
JAR = paths.REF.jar  # repo-relative
# None while the reference's jar sha256 is not pinned ("-" in the registry); check_jar() refuses.
JAR_SHA256 = None if paths.REF.jar_sha256 == "-" else paths.REF.jar_sha256
REF_TAG = "ref-" + JAR_SHA256[:8] if JAR_SHA256 else f"ref-unpinned-{paths.REF.tag}"
GOLDEN_ROOT = f"corpus-cache/d2/_golden/{REF_TAG}"
# A dynamic AppCDS archive belongs to one jar: keyed by its sha256.
CDS_ARCHIVE = f"build/golden-tmp/cds-{REF_TAG[4:]}.jsa"
DEFAULT_TIMEOUT_S = 180
DEFAULT_XMX = "3g"

# JVM flags that do not change compiler behaviour: heap cap, GC choice, C1-only JIT
# (measured: ~3-6x less CPU and lower RSS, also faster wall time on the largest case), no
# perf-data file,
# no unified-logging output (keeps CDS/GC notices out of stdout/stderr), and a dynamic
# AppCDS archive for faster startup.  The archive is created once by ensure_cds_archive().
def jvm_flags(xmx: str = DEFAULT_XMX, use_cds: bool = True) -> list[str]:
    flags = [f"-Xmx{xmx}", "-XX:+UseSerialGC", "-XX:TieredStopAtLevel=1", "-XX:-UsePerfData",
             "-Xlog:disable", "-Xlog:all=off"]
    if use_cds:
        flags += ["-Xshare:auto", f"-XX:SharedArchiveFile={CDS_ARCHIVE}"]
    return flags


# Minimal, fixed environment: nothing inherited (JAVA_TOOL_OPTIONS etc. would print
# "Picked up ..." to stderr), UTF-8 locale, UTC.
def child_env() -> dict[str, str]:
    return {
        "PATH": "/usr/bin:/bin",
        "HOME": os.environ.get("HOME", "/tmp"),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "TZ": "UTC",
    }


_jar_checked = False


def check_jar() -> None:
    global _jar_checked
    if _jar_checked:
        return
    if JAR_SHA256 is None:
        raise SystemExit(f"reference {paths.REF.tag} has no pinned jar sha256 in scripts/references.tsv")
    h = hashlib.sha256()
    with open(os.path.join(REPO, JAR), "rb") as f:
        for blk in iter(lambda: f.read(1 << 20), b""):
            h.update(blk)
    if h.hexdigest() != JAR_SHA256:
        raise SystemExit(f"reference jar sha256 mismatch: {h.hexdigest()}")
    _jar_checked = True


def ensure_cds_archive(xmx: str = DEFAULT_XMX) -> None:
    """Create the dynamic AppCDS archive with one small throwaway compile (not recorded)."""
    path = os.path.join(REPO, CDS_ARCHIVE)
    if os.path.exists(path) and os.path.getsize(path) > 0:
        return
    os.makedirs(os.path.dirname(path), exist_ok=True)
    warm = "build/golden-tmp/_cds_warmup"
    os.makedirs(os.path.join(REPO, warm), exist_ok=True)
    src = os.path.join(REPO, warm, "in.js")
    with open(src, "w") as f:
        f.write("function f(a){return a+1}\nwindow['x']=f(2);\n")
    argv = [JAVA, *jvm_flags(xmx, use_cds=False), "-XX:+AutoCreateSharedArchive", f"-XX:SharedArchiveFile={CDS_ARCHIVE}",
            "-jar", JAR, "--compilation_level=ADVANCED", f"--js={warm}/in.js",
            f"--js_output_file={warm}/out.js"]
    subprocess.run(argv, cwd=REPO, env=child_env(), stdin=subprocess.DEVNULL,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=300)
    shutil.rmtree(os.path.join(REPO, warm), ignore_errors=True)


def enc(data: bytes):
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return {"base64": base64.b64encode(data).decode("ascii")}


def result_path(case_id: str, profile: str) -> str:
    return os.path.join(GOLDEN_ROOT, case_id, f"{profile}.json")


def _collect_outputs(out_dir_abs: str) -> dict:
    outputs = {}
    for root, _dirs, files in os.walk(out_dir_abs):
        for fn in files:
            p = os.path.join(root, fn)
            rel = os.path.relpath(p, out_dir_abs).replace(os.sep, "/")
            with open(p, "rb") as f:
                outputs[rel] = enc(f.read())
    return dict(sorted(outputs.items()))


def run_one(case: dict, profile: str, *, result_file: str | None = None,
            timeout_s: int = DEFAULT_TIMEOUT_S, xmx: str = DEFAULT_XMX,
            keep_out: bool = False, profiles: dict | None = None) -> dict:
    """Run one (case, profile) and write its result JSON. Returns the result dict."""
    check_jar()
    profiles = profiles or case_args.load_profiles()
    out_dir = case_args.default_out_dir(case["id"], profile, profiles)
    cargs = case_args.compiler_args(case, profile, out_dir, profiles)
    argv = [JAVA, *jvm_flags(xmx, os.path.exists(os.path.join(REPO, CDS_ARCHIVE))),
            "-jar", JAR, *cargs]

    out_abs = os.path.join(REPO, out_dir)
    shutil.rmtree(out_abs, ignore_errors=True)
    os.makedirs(out_abs, exist_ok=True)
    std_dir = os.path.join(REPO, "build/golden-tmp/_std", case["id"])
    os.makedirs(std_dir, exist_ok=True)
    so_path = os.path.join(std_dir, f"{profile}.stdout")
    se_path = os.path.join(std_dir, f"{profile}.stderr")

    timed_out = threading.Event()
    with open(so_path, "wb") as so, open(se_path, "wb") as se:
        t0 = time.monotonic()
        # Spawn via rss_spawn.py so wait4's maxrss is the JVM's own peak, not this
        # (possibly large) driver's high-water RSS inherited at execve().
        rss_file = os.path.join(std_dir, f"{profile}.rss")
        if os.path.exists(rss_file):
            os.remove(rss_file)
        spawn = [sys.executable, "-I", "-S", os.path.join(os.path.dirname(os.path.abspath(__file__)), "rss_spawn.py"),
                 rss_file, "--", *argv]
        proc = subprocess.Popen(spawn, cwd=REPO, env=child_env(), stdin=subprocess.DEVNULL,
                                stdout=so, stderr=se, start_new_session=True)

        def _kill():
            timed_out.set()
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

        timer = threading.Timer(timeout_s, _kill)
        timer.start()
        try:
            _pid, status, ru = os.wait4(proc.pid, 0)
        finally:
            timer.cancel()
        wall_ms = int(round((time.monotonic() - t0) * 1000))
        proc.returncode = os.waitstatus_to_exitcode(status)  # already reaped by wait4

    try:
        with open(rss_file) as f:
            peak_rss_kb = int(f.read().strip())
        os.remove(rss_file)
    except (OSError, ValueError):
        peak_rss_kb = None  # wrapper killed (timeout) before it could report
    with open(so_path, "rb") as f:
        stdout = f.read()
    with open(se_path, "rb") as f:
        stderr = f.read()
    outputs = _collect_outputs(out_abs)
    if not keep_out:
        shutil.rmtree(out_abs, ignore_errors=True)
        os.remove(so_path)
        os.remove(se_path)
        try:
            os.rmdir(os.path.dirname(out_abs))
            os.rmdir(std_dir)
        except OSError:
            pass

    res = {
        "runner_version": RUNNER_VERSION,
        "reference_jar_sha256": JAR_SHA256,
        "case_id": case["id"],
        "source": case.get("source"),
        "profile": profile,
        "cwd": ".",
        "env": child_env() | {"HOME": "<inherited>"},
        "args": argv,
        "compiler_args": cargs,
        "out_dir": out_dir,
        "timeout_s": timeout_s,
        "timed_out": timed_out.is_set(),
        "exit_code": proc.returncode,
        "stdout": enc(stdout),
        "stderr": enc(stderr),
        "outputs": outputs,
        "wall_ms": wall_ms,
        "peak_rss_kb": peak_rss_kb,
        "cpu_ms": int(round((ru.ru_utime + ru.ru_stime) * 1000)),
    }
    dest = os.path.join(REPO, result_file or result_path(case["id"], profile))
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    tmp = f"{dest}.tmp.{os.getpid()}.{threading.get_ident()}"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(res, f, ensure_ascii=False, indent=1)
        f.write("\n")
    os.replace(tmp, dest)
    return res


def find_case(case_id: str) -> dict:
    for c in case_args.load_cases():
        if c["id"] == case_id:
            return c
    raise SystemExit(f"unknown case {case_id}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--case", required=True)
    ap.add_argument("--profile", required=True)
    ap.add_argument("--result", help="repo-relative result path (default: golden store)")
    ap.add_argument("--force", action="store_true", help="re-run even if the result exists")
    ap.add_argument("--keep-out", action="store_true", help="keep build/golden-tmp output dir")
    ap.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_S)
    ap.add_argument("--xmx", default=DEFAULT_XMX)
    a = ap.parse_args()
    case = find_case(a.case)
    dest = a.result or result_path(a.case, a.profile)
    if os.path.exists(os.path.join(REPO, dest)) and not a.force:
        print(f"exists: {dest}")
        return 0
    ensure_cds_archive(a.xmx)
    r = run_one(case, a.profile, result_file=dest, timeout_s=a.timeout, xmx=a.xmx,
                keep_out=a.keep_out)
    print(json.dumps({k: r[k] for k in ("case_id", "profile", "exit_code", "timed_out",
                                         "wall_ms", "peak_rss_kb")}))
    print(f"wrote {dest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
