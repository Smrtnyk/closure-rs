#!/usr/bin/env python3
"""Gate 0.1 (docs/PORTING.md §4.1): the oracle server's `compile` result must be byte-identical to
the golden uberjar result for EVERY (case, profile) in corpus/d2/cases.jsonl.

Usage:
  python3 gates/lib/gate01.py run    [--seed N] [--servers 8] [--xmx 1536m] [--oom-xmx 3g]
                                     [--heap-budget-gb 14] [--reserve-gb 3.0]
                                     [--limit N] [--timeout 600] [--fresh] [--redo-failed]
  python3 gates/lib/gate01.py report

run
  * Pairs = every case in corpus/d2/cases.jsonl x its `profiles` list (authoritative for the
    final corpus, see corpus/d2/FORMAT.md).  argv = case_args.compiler_args(case, profile)
    with the default out_dir, i.e. exactly the argv of the golden run (checked against the
    golden `compiler_args`); cwd = repo root; env = run_reference.child_env().
  * All pairs are put in ONE seeded shuffled order (random.Random(seed)).  --servers
    long-lived `closurers.oracle.Main server --isolate=none` JVMs pull the next pair from that
    order (dynamic queue).  Each server's actual request sequence is logged in
    build/gate01/servers/, so any static-state leak can be replayed in order.
  * Oracle jar snapshot: the run copies build/oracle/oracle.jar to
    build/gate01/oracle-<sha12>.jar and every server runs from that copy, so a concurrent
    oracle rebuild cannot mix two oracle versions into one run.  If build/oracle/oracle.jar
    no longer matches the run's sha256, the previous run (results, mismatches, server logs,
    run.json) is moved to build/gate01/archive/<sha12>-<time>/ and a new run starts (same
    seed unless --seed is given).
  * Memory: servers * xmx + (oom_xmx - xmx) must be <= --heap-budget-gb (default 14 GB, the
    host limit), else the run refuses to start.  Before each request the worker waits while
    MemAvailable < --reserve-gb.  Each server's peak RSS (VmHWM) is recorded.
  * OutOfMemoryError: if a response's stderr (or oracle error) contains
    `java.lang.OutOfMemoryError` and the golden stderr does not, the heap of that server was
    too small for the request (a resource limit, not compiler behaviour).  The server is
    killed (an OOM can leave static state half-initialised), the SAME request is retried once
    on a fresh server with -Xmx<oom_xmx> (the golden run's heap), and the server then
    restarts at -Xmx<xmx>.  Both attempts are kept in the result file and listed in the
    report.  A second OOM is a failure.
  * Comparison against corpus-cache/d2/_golden/ref-4ef5a893/<case>/<profile>.json, byte for
    byte: stdout, stderr, exit code, and the set + content of every file in the out_dir (the
    golden runner collects outputs by walking the out_dir; so does this gate).  The oracle's
    reported `output_files` must also agree with the files on disk.  NO normalization.
  * Resumable: a pair is skipped iff its result file build/gate01/results/<case>/<profile>.json
    exists AND was produced by this gate version, this oracle sha256, the same argv, and the
    same golden file (size + mtime fast check; `report` re-verifies golden sha256).  The seed is
    fixed in build/gate01/run.json on the first run.  --fresh archives the current run and
    starts over; --redo-failed re-runs pairs whose result is a server failure / gate error
    (never a compiler-output mismatch); the earlier attempt is kept in `previous`.
  * The out_dir is build/golden-tmp/<case>/<profile> (the argv must be identical to the golden
    run's, and the path is observable in source maps / chunk prefixes).  It is created empty
    before each request and removed afterwards, exactly as run_reference.py does.

report
  * Writes gates/reports/gate_0_1.json and gates/reports/gate_0_1.md from the result files.
  * Re-verifies that each result is current: oracle sha256 == run's == current
    build/oracle/oracle.jar, argv == golden argv, golden file sha256 == the one compared.
  * Exit 0 only if every pair was compared, is current, and is identical; 1 on any mismatch;
    2 if pairs are missing or stale.
"""

from __future__ import annotations

import argparse
import base64
import collections
import difflib
import fcntl
import glob
import hashlib
import json
import os
import queue
import random
import resource
import shutil
import signal
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import case_args  # noqa: E402
import run_reference as rr  # noqa: E402

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__)))), "oracle"))
import oracle_client  # noqa: E402  (GOLDEN_JVM_ENV / check_jvm_env, PROTOCOL.md "Environment")

REPO = case_args.REPO
CASES = "corpus/d2/cases.jsonl"
GOLDEN_ROOT = rr.GOLDEN_ROOT
DEFAULT_WORK = "build/gate01"
# GATE01_WORK selects another run directory (e.g. an independent confirmation run with its own
# seed: GATE01_WORK=build/gate01/confirm-s<seed>); its report then goes to
# gates/reports/gate_0_1.<basename>.{md,json} unless GATE01_REPORT (a path stem) says otherwise.
WORK = os.environ.get("GATE01_WORK", DEFAULT_WORK).rstrip("/")
RESULTS = f"{WORK}/results"
MISMATCH = f"{WORK}/mismatch"
SERVER_LOGS = f"{WORK}/servers"
ARCHIVE = f"{WORK}/archive"
RUN_STATE = f"{WORK}/run.json"
REPORT_STEM = os.environ.get("GATE01_REPORT") or (
    "gates/reports/gate_0_1" if WORK == DEFAULT_WORK
    else f"gates/reports/gate_0_1.{os.path.basename(WORK)}")
REPORT_JSON = REPORT_STEM + ".json"
REPORT_MD = REPORT_STEM + ".md"
CONFIRM_GLOB = "build/gate01/confirm-*"  # independent full runs, summarised by the main report
RUN_LOCK = "build/golden-tmp/.gate01.lock"  # all runs share the golden out_dirs: one at a time
ORACLE_JAR = rr.paths.REF.oracle_jar  # repo-relative; build/oracle-v20261006/oracle.jar for the default reference
DEFAULT_SEED = 20261006
GATE_VERSION = 3  # v3: every server's ready-line JVM env must equal oracle_client.GOLDEN_JVM_ENV
OOM_MARK = b"java.lang.OutOfMemoryError"


def p(rel: str) -> str:
    return os.path.join(REPO, rel)


def sha256(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def file_sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for blk in iter(lambda: f.read(1 << 20), b""):
            h.update(blk)
    return h.hexdigest()


def mem_available_kb() -> int:
    with open("/proc/meminfo") as f:
        for line in f:
            if line.startswith("MemAvailable:"):
                return int(line.split()[1])
    return 0


def proc_status_kb(pid: int, key: str) -> int | None:
    try:
        with open(f"/proc/{pid}/status") as f:
            for line in f:
                if line.startswith(key + ":"):
                    return int(line.split()[1])
    except OSError:
        pass
    return None


def parse_size_gb(s: str) -> float:
    s = s.strip().lower()
    mult = {"g": 1.0, "m": 1 / 1024, "k": 1 / (1024 * 1024)}
    if s[-1] in mult:
        return float(s[:-1]) * mult[s[-1]]
    return float(s) / (1024 ** 3)


def load_pairs() -> tuple[list[dict], list[tuple[dict, str]]]:
    cases = case_args.load_cases(CASES)
    pairs = [(c, prof) for c in cases for prof in c["profiles"]]
    return cases, pairs


def shuffled(pairs, seed: int):
    order = list(pairs)
    random.Random(seed).shuffle(order)
    return order


def result_path(case_id: str, profile: str) -> str:
    return p(f"{RESULTS}/{case_id}/{profile}.json")


def golden_path(case_id: str, profile: str) -> str:
    return p(f"{GOLDEN_ROOT}/{case_id}/{profile}.json")


def golden_stamp(case_id: str, profile: str) -> dict:
    st = os.stat(golden_path(case_id, profile))
    return {"size": st.st_size, "mtime_ns": st.st_mtime_ns}


def dec(v) -> bytes:
    """Golden text field -> bytes (str is UTF-8 text, {"base64": ...} is raw bytes)."""
    if isinstance(v, dict):
        return base64.b64decode(v["base64"])
    return v.encode("utf-8")


def snapshot_jar_rel(sha: str) -> str:
    return f"{WORK}/oracle-{sha[:12]}.jar"


def server_jvm_flags(xmx: str) -> list[str]:
    # Heap cap and GC as the golden run (rr.jvm_flags uses SerialGC), no perf data, JVM
    # logging off so nothing but protocol lines reach the server's stdout.  The JIT is left at
    # the default (tiered C1+C2): this is how the oracle server is used afterwards, and the
    # golden run's C1-only setting was chosen for cold-start speed, not behaviour.  Heap free
    # ratios only make SerialGC give memory back between large requests (little free RAM).
    # The heap size is lower than the golden's 3g to fit the host's heap budget; an
    # OutOfMemoryError is retried at the golden heap size (see module docstring).
    # -XX:+ExitOnOutOfMemoryError makes any OOM unambiguous (the JVM exits with status 3 and
    # prints "Terminating due to java.lang.OutOfMemoryError" on its VM output stream, which
    # -XX:+DisplayVMOutputToStderr sends to the server's stderr log instead of the protocol
    # stdout) instead of surfacing as an arbitrary wrapped exception.  Neither flag changes
    # anything unless the JVM itself prints or runs out of memory; the response's stdout/stderr
    # are the in-memory capture streams, not the process streams.
    return [f"-Xmx{xmx}", "-Xms128m", "-XX:+UseSerialGC", "-XX:MinHeapFreeRatio=10",
            "-XX:MaxHeapFreeRatio=30", "-XX:-UsePerfData", "-XX:+ExitOnOutOfMemoryError",
            "-XX:+DisplayVMOutputToStderr", "-Xlog:disable", "-Xlog:all=off"]


def collect_outputs(out_abs: str) -> dict[str, bytes]:
    outputs = {}
    for root, _dirs, files in os.walk(out_abs):
        for fn in files:
            fp = os.path.join(root, fn)
            rel = os.path.relpath(fp, out_abs).replace(os.sep, "/")
            with open(fp, "rb") as f:
                outputs[rel] = f.read()
    return dict(sorted(outputs.items()))


def first_diff(a: bytes, b: bytes) -> int:
    n = min(len(a), len(b))
    i = 0
    step = 4096
    while i < n and a[i:i + step] == b[i:i + step]:
        i += step
    while i < n and a[i] == b[i]:
        i += 1
    return i


def minimal_diff(golden: bytes, oracle: bytes, ctx: int = 160) -> dict:
    """First differing byte offset with context, plus a short unified line diff."""
    off = first_diff(golden, oracle)
    lo = max(0, off - ctx)
    d = {"golden_len": len(golden), "oracle_len": len(oracle), "first_diff_offset": off,
         "golden_sha256": sha256(golden), "oracle_sha256": sha256(oracle),
         "golden_context": golden[lo:off + ctx].decode("utf-8", "backslashreplace"),
         "oracle_context": oracle[lo:off + ctx].decode("utf-8", "backslashreplace")}
    gl = golden.decode("utf-8", "backslashreplace").splitlines()
    ol = oracle.decode("utf-8", "backslashreplace").splitlines()
    if len(gl) + len(ol) < 200000:
        ud = list(difflib.unified_diff(gl, ol, "golden", "oracle", n=1, lineterm=""))
        clipped = [ln if len(ln) <= 400 else ln[:400] + "...[clipped]" for ln in ud[:60]]
        d["unified_diff_head"] = clipped
        d["unified_diff_lines"] = len(ud)
    return d


class Server:
    def __init__(self, idx: int, xmx: str, segment: str, jar_rel: str):
        self.idx = idx
        self.xmx = xmx
        self.cur_xmx = xmx
        self.segment = segment
        self.jar_rel = jar_rel
        self.starts = 0
        self.proc = None
        self.pos = 0
        self.peaks: list[dict] = []  # one entry per JVM start: {start_no, xmx, vmhwm_kb}
        os.makedirs(p(SERVER_LOGS), exist_ok=True)
        self.seq_log = open(p(f"{SERVER_LOGS}/{segment}-s{idx}.jsonl"), "a", encoding="utf-8")
        self.start()

    def log(self, obj: dict):
        self.seq_log.write(json.dumps(obj) + "\n")
        self.seq_log.flush()

    def start(self, xmx: str | None = None):
        self.cur_xmx = xmx or self.xmx
        self.starts += 1
        self.pos = 0
        cp = f"{self.jar_rel}:{rr.JAR}"
        argv = [rr.JAVA, *server_jvm_flags(self.cur_xmx), "-cp", cp, "closurers.oracle.Main",
                "server", "--isolate=none"]
        err = open(p(f"{SERVER_LOGS}/{self.segment}-s{self.idx}.stderr"), "ab")
        self.proc = subprocess.Popen(argv, cwd=REPO, env=rr.child_env(), stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=err, start_new_session=True)
        err.close()
        hello = self.proc.stdout.readline()
        h = json.loads(hello)
        if not h.get("ready") or h.get("isolate") != "none":
            raise RuntimeError(f"server {self.idx} bad hello: {hello!r}")
        # The oracle's output depends on the JVM's locale/encodings/time zone; refuse to
        # compare anything produced outside the golden environment.
        try:
            oracle_client.check_jvm_env(h.get("env"))
        except oracle_client.OracleEnvMismatch:
            self.kill()
            raise
        self.log({"event": "start", "start_no": self.starts, "pid": self.proc.pid,
                  "xmx": self.cur_xmx, "argv": argv, "jvm_env": h.get("env")})

    def record_peak(self):
        hwm = proc_status_kb(self.proc.pid, "VmHWM") if self.proc else None
        self.peaks.append({"start_no": self.starts, "xmx": self.cur_xmx, "vmhwm_kb": hwm,
                           "requests": self.pos})
        self.log({"event": "peak", "start_no": self.starts, "xmx": self.cur_xmx,
                  "vmhwm_kb": hwm, "requests": self.pos})

    def kill(self):
        try:
            os.killpg(self.proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        try:
            self.proc.wait(timeout=30)
        except Exception:  # noqa: BLE001
            pass

    def restart(self, xmx: str | None = None, reason: str = ""):
        self.record_peak()
        self.log({"event": "restart", "reason": reason, "next_xmx": xmx or self.xmx})
        self.kill()
        self.start(xmx)

    def request(self, req: dict, timeout_s: int) -> dict:
        """Send one request; on death/timeout returns {"_server_failure": ...} and restarts."""
        killed = threading.Event()

        def _kill():
            killed.set()
            self.kill()

        timer = threading.Timer(timeout_s, _kill)
        timer.start()
        try:
            self.proc.stdin.write((json.dumps(req) + "\n").encode("utf-8"))
            self.proc.stdin.flush()
            line = self.proc.stdout.readline()
        except (BrokenPipeError, OSError):
            line = b""
        finally:
            timer.cancel()
        self.pos += 1
        if not line:
            try:
                rc = self.proc.wait(timeout=30)
            except Exception:  # noqa: BLE001
                rc = self.proc.poll()
            tail = ""
            try:
                with open(p(f"{SERVER_LOGS}/{self.segment}-s{self.idx}.stderr"), "rb") as f:
                    f.seek(0, os.SEEK_END)
                    f.seek(max(0, f.tell() - 2000))
                    tail = f.read().decode("utf-8", "backslashreplace")
            except OSError:
                pass
            self.log({"event": "died", "timeout": killed.is_set(), "returncode": rc,
                      "stderr_tail": tail[-500:]})
            self.restart(reason="died")
            return {"_server_failure": "timeout" if killed.is_set() else f"died rc={rc}",
                    "_rc": None if killed.is_set() else rc, "_stderr_tail": tail[-500:]}
        return json.loads(line)

    def close(self):
        self.record_peak()
        try:
            self.proc.stdin.close()
            self.proc.wait(timeout=60)
        except Exception:  # noqa: BLE001
            self.kill()
        self.seq_log.close()


def compare(golden: dict, resp: dict, disk: dict[str, bytes], out_dir: str) -> list[dict]:
    diffs = []
    if resp.get("_server_failure"):
        tail = resp.get("_stderr_tail", "").strip()
        return [{"field": "server", "detail": resp["_server_failure"]
                 + (f"; server stderr tail: {tail[-300:]}" if tail else "")}]
    if not resp.get("ok"):
        return [{"field": "oracle_error", "detail": resp.get("error", "")[:4000]}]
    if golden.get("timed_out"):
        diffs.append({"field": "golden_timed_out", "detail": "golden run timed out"})
    if resp["exit_code"] != golden["exit_code"]:
        diffs.append({"field": "exit_code", "golden": golden["exit_code"],
                      "oracle": resp["exit_code"]})
    for name in ("stdout", "stderr"):
        g = dec(golden[name])
        o = base64.b64decode(resp[f"{name}_b64"])
        if g != o:
            diffs.append({"field": name, **minimal_diff(g, o)})
    gout = {k: dec(v) for k, v in golden["outputs"].items()}
    for k in sorted(set(gout) | set(disk)):
        if k not in disk:
            diffs.append({"field": f"output:{k}", "detail": "missing in oracle run",
                          "golden_len": len(gout[k])})
        elif k not in gout:
            diffs.append({"field": f"output:{k}", "detail": "extra file in oracle run",
                          "oracle_len": len(disk[k])})
        elif gout[k] != disk[k]:
            diffs.append({"field": f"output:{k}", **minimal_diff(gout[k], disk[k])})
    # The oracle's own output_files report must agree with what is on disk.
    prefix = out_dir.rstrip("/") + "/"
    for fname, b64 in resp.get("output_files", {}).items():
        rel = fname[len(prefix):] if fname.startswith(prefix) else None
        content = base64.b64decode(b64)
        if rel is None:
            diffs.append({"field": "output_files_report", "detail": f"file outside out_dir: {fname}"})
        elif disk.get(rel) != content:
            diffs.append({"field": "output_files_report",
                          "detail": f"reported content of {fname} != file on disk"})
    return diffs


def is_oom(resp: dict, golden_stderr: bytes) -> bool:
    """The response shows an OutOfMemoryError that the golden run did not have."""
    if OOM_MARK in golden_stderr:
        return False
    if resp.get("_server_failure"):
        # -XX:+ExitOnOutOfMemoryError: status 3 + the JVM's "Terminating due to ..." line.
        return resp.get("_rc") == 3 and OOM_MARK.decode() in resp.get("_stderr_tail", "")
    if not resp.get("ok"):
        return OOM_MARK.decode() in resp.get("error", "")
    return OOM_MARK in base64.b64decode(resp.get("stderr_b64", ""))


def is_retryable_failure(rec: dict) -> bool:
    """Server death/timeout, oracle error or gate exception -- never a compiler-output diff."""
    return any(d["field"] in ("server", "oracle_error", "gate_error") for d in rec["diffs"])


def save_mismatch(case_id: str, profile: str, resp: dict, disk: dict[str, bytes]):
    d = p(f"{MISMATCH}/{case_id}/{profile}")
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "response.json"), "w") as f:
        json.dump(resp, f)
    if resp.get("ok"):
        for name in ("stdout", "stderr"):
            with open(os.path.join(d, f"oracle.{name}"), "wb") as f:
                f.write(base64.b64decode(resp[f"{name}_b64"]))
    for k, v in disk.items():
        fp = os.path.join(d, "outputs", k)
        os.makedirs(os.path.dirname(fp), exist_ok=True)
        with open(fp, "wb") as f:
            f.write(v)


def write_json_atomic(path: str, obj, indent=1):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = f"{path}.tmp.{os.getpid()}.{threading.get_ident()}"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(obj, f, ensure_ascii=False, indent=indent)
        f.write("\n")
    os.replace(tmp, path)


def archive_run(reason: str) -> str | None:
    """Move the current run (results, mismatches, server logs, run.json) under archive/."""
    if not any(os.path.exists(p(x)) for x in (RESULTS, MISMATCH, SERVER_LOGS, RUN_STATE)):
        return None
    old_sha = "unknown"
    if os.path.exists(p(RUN_STATE)):
        try:
            with open(p(RUN_STATE)) as f:
                old_sha = (json.load(f).get("oracle_jar_sha256") or "unknown")[:12]
        except (OSError, ValueError):
            pass
    dest = f"{ARCHIVE}/{old_sha}-{time.strftime('%Y%m%dT%H%M%S')}"
    os.makedirs(p(dest), exist_ok=True)
    for x in (RESULTS, MISMATCH, SERVER_LOGS, RUN_STATE):
        if os.path.exists(p(x)):
            shutil.move(p(x), p(f"{dest}/{os.path.basename(x)}"))
    with open(p(f"{dest}/ARCHIVED.txt"), "w") as f:
        f.write(f"archived {time.strftime('%Y-%m-%dT%H:%M:%S%z')}: {reason}\n")
    print(f"[gate01] archived previous run to {dest} ({reason})", flush=True)
    return dest


def result_is_current(rec: dict, oracle_sha: str, cargs: list[str], gstamp: dict) -> bool:
    return (rec.get("gate_version") == GATE_VERSION
            and rec.get("oracle_jar_sha256") == oracle_sha
            and rec.get("argv") == cargs
            and rec.get("golden_stamp") == gstamp)


def cmd_run(a) -> int:
    rr.check_jar()
    if not os.path.exists(p(ORACLE_JAR)):
        print(f"missing {ORACLE_JAR}; run oracle/build.sh", file=sys.stderr)
        return 2
    xmx_gb, oom_gb = parse_size_gb(a.xmx), parse_size_gb(a.oom_xmx)
    heap_worst = a.servers * xmx_gb + max(0.0, oom_gb - xmx_gb)
    if heap_worst > a.heap_budget_gb + 1e-9:
        print(f"heap budget exceeded: {a.servers} x {a.xmx} + OOM retry ({a.oom_xmx}) = "
              f"{heap_worst:.1f} GB > {a.heap_budget_gb} GB", file=sys.stderr)
        return 2
    # Every run (main or confirmation) writes the same build/golden-tmp/<case>/<profile>
    # out_dirs (the argv must equal the golden one), so two runs may never overlap.
    os.makedirs(os.path.dirname(p(RUN_LOCK)), exist_ok=True)
    lock_fd = os.open(p(RUN_LOCK), os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print(f"another gate01 run holds {RUN_LOCK}; refusing to start", file=sys.stderr)
        return 2
    os.ftruncate(lock_fd, 0)
    os.write(lock_fd, f"{os.getpid()} {WORK}\n".encode())
    a._lock_fd = lock_fd  # keep it open (and locked) until the process exits
    profiles = case_args.load_profiles()
    cases, pairs = load_pairs()
    problems = case_args.verify(profiles, cases)
    if problems:
        for pr in problems:
            print("PROBLEM:", pr)
        return 2

    oracle_sha = file_sha256(p(ORACLE_JAR))
    state = {}
    if os.path.exists(p(RUN_STATE)):
        with open(p(RUN_STATE)) as f:
            state = json.load(f)
    if a.fresh:
        archive_run("--fresh")
        state = {}
    elif state and (state.get("gate_version") != GATE_VERSION
                    or state.get("oracle_jar_sha256") != oracle_sha):
        archive_run(f"run was gate_version={state.get('gate_version')} oracle "
                    f"{str(state.get('oracle_jar_sha256'))[:12]}; now gate_version="
                    f"{GATE_VERSION} oracle {oracle_sha[:12]}")
        state = {"seed": state.get("seed")} if a.seed is None else {}
    elif state and a.seed is not None and a.seed != state.get("seed"):
        print(f"run.json has seed {state['seed']}; use it or --fresh", file=sys.stderr)
        return 2
    seed = state.get("seed") or (a.seed if a.seed is not None else DEFAULT_SEED)

    # Snapshot the oracle jar so a concurrent rebuild cannot change it under the servers.
    jar_rel = snapshot_jar_rel(oracle_sha)
    if not os.path.exists(p(jar_rel)) or file_sha256(p(jar_rel)) != oracle_sha:
        os.makedirs(p(WORK), exist_ok=True)
        shutil.copyfile(p(ORACLE_JAR), p(jar_rel) + ".tmp")
        os.replace(p(jar_rel) + ".tmp", p(jar_rel))
    if file_sha256(p(jar_rel)) != oracle_sha:
        print("oracle.jar changed while it was being snapshotted; retry", file=sys.stderr)
        return 2

    state.update({"gate_version": GATE_VERSION, "seed": seed, "cases_file": CASES,
                  "cases_sha256": file_sha256(p(CASES)), "pairs_total": len(pairs),
                  "reference_jar_sha256": rr.JAR_SHA256, "oracle_jar_sha256": oracle_sha,
                  "oracle_jar_snapshot": jar_rel, "golden_root": GOLDEN_ROOT})
    state.setdefault("segments", [])

    order = shuffled(pairs, seed)
    index = {(c["id"], prof): i for i, (c, prof) in enumerate(order)}
    todo = []
    stale = 0
    for c, prof in order:
        rp = result_path(c["id"], prof)
        if os.path.exists(rp):
            with open(rp, encoding="utf-8") as f:
                rec = json.load(f)
            out_dir = case_args.default_out_dir(c["id"], prof, profiles)
            cargs = case_args.compiler_args(c, prof, out_dir, profiles)
            if result_is_current(rec, oracle_sha, cargs, golden_stamp(c["id"], prof)):
                if not (a.redo_failed and not rec["match"] and is_retryable_failure(rec)):
                    continue
            else:
                stale += 1
        todo.append((c, prof))
    if a.limit:
        todo = todo[: a.limit]
    segment = time.strftime("%Y%m%dT%H%M%S")
    print(f"[gate01] seed={seed} pairs={len(pairs)} todo={len(todo)} (stale results redone: "
          f"{stale}) servers={a.servers} xmx={a.xmx} oom_xmx={a.oom_xmx} heap_worst="
          f"{heap_worst:.1f}GB oracle={oracle_sha[:12]} segment={segment} pid={os.getpid()}",
          flush=True)
    if not todo:  # nothing to resume; do not record an empty segment
        write_json_atomic(p(RUN_STATE), state)
        return 0
    seg = {"segment": segment, "servers": a.servers, "xmx": a.xmx, "oom_xmx": a.oom_xmx,
           "heap_budget_gb": a.heap_budget_gb, "heap_worst_case_gb": round(heap_worst, 2),
           "jvm_flags": server_jvm_flags(a.xmx), "todo": len(todo), "start": time.time(),
           "host_mem_available_kb_at_start": mem_available_kb(),
           "host_loadavg_at_start": os.getloadavg()}
    state["segments"].append(seg)
    write_json_atomic(p(RUN_STATE), state)

    q: queue.Queue = queue.Queue()
    for item in todo:
        q.put(item)
    lock = threading.Lock()
    start_gate = threading.Lock()
    dir_lock = threading.Lock()
    counts = collections.Counter()
    done = [0]
    oom_retries = [0]
    t0 = time.monotonic()
    reserve_kb = int(a.reserve_gb * 1024 * 1024)
    stop = threading.Event()
    servers: list[Server] = []
    min_mem = [mem_available_kb()]
    oom_slot = threading.Semaphore(1)

    def process(srv: Server, c: dict, prof: str) -> dict:
        out_dir = case_args.default_out_dir(c["id"], prof, profiles)
        cargs = case_args.compiler_args(c, prof, out_dir, profiles)
        out_abs = p(out_dir)
        gpath = golden_path(c["id"], prof)
        gstamp = golden_stamp(c["id"], prof)
        with open(gpath, "rb") as f:
            graw = f.read()
        golden = json.loads(graw)
        gsha = sha256(graw)
        del graw
        golden_stderr = dec(golden["stderr"])
        attempts = []
        held = [False]

        def take_oom_slot():
            oom_slot.acquire()
            held[0] = True

        try:
            resp, disk = _attempts(srv, c, prof, cargs, out_abs, golden_stderr, attempts,
                                   take_oom_slot)
        finally:
            try:
                if srv.cur_xmx != srv.xmx:  # back to the normal heap after an OOM retry
                    srv.restart(None, reason="end of OOM retry")
            finally:
                if held[0]:
                    oom_slot.release()
        return _finish(srv, c, prof, out_dir, cargs, golden, gsha, gstamp, resp, disk,
                       attempts)

    def _attempts(srv, c, prof, cargs, out_abs, golden_stderr, attempts, take_oom_slot):
        while True:
            with dir_lock:
                shutil.rmtree(out_abs, ignore_errors=True)
                os.makedirs(out_abs, exist_ok=True)
            req = {"op": "compile", "args": cargs, "id": index[(c["id"], prof)]}
            pos, starts, xmx_now = srv.pos, srv.starts, srv.cur_xmx
            ts = time.monotonic()
            resp = srv.request(req, a.timeout)
            secs = time.monotonic() - ts
            disk = collect_outputs(out_abs)
            with dir_lock:
                shutil.rmtree(out_abs, ignore_errors=True)
                try:
                    os.rmdir(os.path.dirname(out_abs))
                except OSError:
                    pass
            att = {"server_start_no": starts, "server_pos": pos, "xmx": xmx_now,
                   "seconds": round(secs, 3)}
            srv.log({"pos": pos, "start_no": starts, "case": c["id"], "profile": prof,
                     "xmx": xmx_now, "seconds": att["seconds"]})
            if is_oom(resp, golden_stderr) and xmx_now != a.oom_xmx and not attempts:
                att["oom"] = True
                att["exit_code"] = resp.get("exit_code")
                att["evidence"] = (resp.get("_server_failure", "") + " "
                                   + resp.get("_stderr_tail", "")[-200:]
                                   + resp.get("error", "")[:200]).strip()
                attempts.append(att)
                with lock:
                    oom_retries[0] += 1
                print(f"[gate01] OOM at -Xmx{xmx_now}: {c['id']}/{prof}; retrying on a fresh "
                      f"-Xmx{a.oom_xmx} server", flush=True)
                # Only one server at a time may run at the OOM heap, so the heap budget
                # (servers * xmx + (oom_xmx - xmx)) is a real bound.
                take_oom_slot()
                srv.restart(a.oom_xmx, reason=f"OOM on {c['id']}/{prof}")
                continue
            attempts.append(att)
            return resp, disk

    def _finish(srv, c, prof, out_dir, cargs, golden, gsha, gstamp, resp, disk, attempts):
        if golden["compiler_args"] != cargs:
            diffs = [{"field": "argv", "detail": "golden compiler_args != case_args argv"}]
        else:
            diffs = compare(golden, resp, disk, out_dir)
        match = not diffs
        if not match:
            save_mismatch(c["id"], prof, resp, disk)
        final = attempts[-1]
        return {"gate_version": GATE_VERSION, "case_id": c["id"], "source": c["source"],
                "profile": prof, "match": match, "diffs": diffs, "argv": cargs,
                "oracle_jar_sha256": oracle_sha, "golden_sha256": gsha, "golden_stamp": gstamp,
                "order_index": index[(c["id"], prof)], "segment": segment, "server": srv.idx,
                "server_start_no": final["server_start_no"], "server_pos": final["server_pos"],
                "xmx": final["xmx"], "seconds": final["seconds"],
                "attempts": attempts if len(attempts) > 1 else None,
                "golden_wall_ms": golden.get("wall_ms"),
                "exit_code": resp.get("exit_code"), "golden_exit_code": golden["exit_code"],
                "oracle_sha256": None if not resp.get("ok") else {
                    "stdout": sha256(base64.b64decode(resp["stdout_b64"])),
                    "stderr": sha256(base64.b64decode(resp["stderr_b64"])),
                    "outputs": {k: sha256(v) for k, v in disk.items()}}}

    def worker(idx: int):
        try:
            srv = Server(idx, a.xmx, segment, jar_rel)
        except Exception as e:  # noqa: BLE001
            print(f"[gate01] server {idx} failed to start: {e!r}", flush=True)
            return
        with lock:
            servers.append(srv)
        try:
            while not stop.is_set():
                try:
                    c, prof = q.get_nowait()
                except queue.Empty:
                    return
                with start_gate:
                    waited = 0
                    while mem_available_kb() < reserve_kb and waited < 900:
                        time.sleep(2)
                        waited += 2
                rp = result_path(c["id"], prof)
                previous = None
                if os.path.exists(rp):
                    with open(rp, encoding="utf-8") as f:
                        prev = json.load(f)
                    previous = (prev.get("previous") or []) + [
                        {k: prev.get(k) for k in ("segment", "server", "server_pos", "match",
                                                  "diffs", "oracle_jar_sha256",
                                                  "gate_version")}]
                try:
                    rec = process(srv, c, prof)
                except Exception as e:  # noqa: BLE001 -- record, never lose a pair silently
                    import traceback
                    rec = {"gate_version": GATE_VERSION, "case_id": c["id"],
                           "source": c["source"], "profile": prof, "match": False,
                           "diffs": [{"field": "gate_error",
                                      "detail": traceback.format_exc()[-4000:]}],
                           "argv": None, "oracle_jar_sha256": oracle_sha,
                           "golden_stamp": None, "order_index": index[(c["id"], prof)],
                           "segment": segment, "server": idx, "server_start_no": srv.starts,
                           "server_pos": srv.pos, "xmx": srv.cur_xmx, "seconds": 0.0,
                           "exit_code": None, "golden_exit_code": None}
                    print(f"[gate01] GATE ERROR {c['id']}/{prof}: {e!r}", flush=True)
                    try:
                        srv.restart(None, reason=f"gate error {e!r}")
                    except Exception:  # noqa: BLE001
                        pass
                if previous:
                    rec["previous"] = previous
                write_json_atomic(rp, rec)
                srv.log({"pos": rec["server_pos"], "start_no": rec["server_start_no"],
                         "case": c["id"], "profile": prof, "match": rec["match"]})
                with lock:
                    done[0] += 1
                    m = mem_available_kb()
                    min_mem[0] = min(min_mem[0], m)
                    counts["identical" if rec["match"] else "MISMATCH"] += 1
                    if not rec["match"]:
                        print(f"[gate01] MISMATCH {c['id']}/{prof}: "
                              f"{[d['field'] for d in rec['diffs']]}", flush=True)
                    if done[0] % 250 == 0 or done[0] == len(todo):
                        el = time.monotonic() - t0
                        eta = el / done[0] * (len(todo) - done[0])
                        print(f"[gate01] {done[0]}/{len(todo)} elapsed={el:.0f}s eta={eta:.0f}s "
                              f"memavail={m // 1024}MB load={os.getloadavg()[0]:.0f} "
                              f"{dict(counts)}", flush=True)
        finally:
            srv.close()

    threads = [threading.Thread(target=worker, args=(i,), daemon=True) for i in range(a.servers)]
    for t in threads:
        t.start()
        time.sleep(1.0)  # stagger JVM starts
    try:
        for t in threads:
            t.join()
    except KeyboardInterrupt:
        stop.set()
        for t in threads:
            t.join()
    el = time.monotonic() - t0
    seg.update({"end": time.time(), "wall_s": round(el, 1), "done": done[0],
                "results": dict(counts), "oom_retries": oom_retries[0],
                "host_mem_available_kb_min": min_mem[0],
                "host_loadavg_at_end": os.getloadavg(),
                "driver_peak_rss_kb": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                "server_jvms": [{"server": s.idx, **pk} for s in sorted(servers, key=lambda s: s.idx)
                                for pk in s.peaks]})
    write_json_atomic(p(RUN_STATE), state)
    print(f"[gate01] segment done in {el:.0f}s: {dict(counts)} oom_retries={oom_retries[0]}",
          flush=True)
    if done[0] < len(todo):
        print(f"[gate01] INCOMPLETE: {len(todo) - done[0]} pairs not processed", flush=True)
        return 2
    return 0


def golden_driver_runs() -> list[dict]:
    """Every golden_all.py driver run whose end-of-run summary is in build/logs/*.log.

    golden_all.py prints its `_summary.json` (with `last_driver_run`: todo, jobs, wall_s) at the
    end of each run, and _summary.json itself keeps only the last one, so the logs are the
    record of the earlier runs.  Deduplicated on (todo, jobs, wall_s)."""
    import re
    pat = re.compile(r'"last_driver_run":\s*\{\s*"todo":\s*(\d+),\s*"jobs":\s*(\d+),'
                     r'\s*"jobs_reason":\s*"[^"]*",\s*"wall_s":\s*([0-9.]+)')
    runs, seen = [], set()
    for lp in sorted(glob.glob(p("build/logs/*.log"))):
        try:
            with open(lp, encoding="utf-8", errors="replace") as f:
                txt = f.read()
        except OSError:
            continue
        for m in pat.finditer(txt):
            key = (int(m.group(1)), int(m.group(2)), float(m.group(3)))
            if key not in seen:
                seen.add(key)
                runs.append({"pairs": key[0], "jobs": key[1], "wall_s": key[2],
                             "log": os.path.relpath(lp, REPO),
                             "log_mtime": os.path.getmtime(lp)})
    runs.sort(key=lambda r: r["log_mtime"])
    for r in runs:
        r["log_mtime"] = time.strftime("%Y-%m-%dT%H:%M:%S%z", time.localtime(r["log_mtime"]))
    return runs


def confirmation_reports() -> list[dict]:
    """Summaries of the independent confirmation runs (GATE01_WORK=build/gate01/confirm-*)."""
    out = []
    if WORK != DEFAULT_WORK:
        return out
    for d in sorted(glob.glob(p(CONFIRM_GLOB))):
        name = os.path.basename(d)
        rp = p(f"gates/reports/gate_0_1.{name}.json")
        if not os.path.exists(rp):
            out.append({"work": os.path.relpath(d, REPO), "report": None,
                        "status": "NO REPORT (running or never finished)"})
            continue
        with open(rp, encoding="utf-8") as f:
            r = json.load(f)
        out.append({"work": os.path.relpath(d, REPO), "report": os.path.relpath(rp, REPO),
                    **{k: r.get(k) for k in ("status", "seed", "pairs_total", "compared",
                                             "identical", "mismatching", "missing", "stale",
                                             "oracle_jar_sha256", "cases_sha256",
                                             "generated_at")},
                    "gate_wall_s": r.get("timing", {}).get("gate_wall_s"),
                    "segments": [{k: sg.get(k) for k in ("segment", "servers", "xmx",
                                                         "heap_worst_case_gb", "wall_s",
                                                         "finished")}
                                 for sg in r.get("segments", [])]})
    return out


def md_escape_block(s: str) -> str:
    return s.replace("```", "`​``")


def cmd_report(_a) -> int:
    profiles = case_args.load_profiles()
    cases, pairs = load_pairs()
    state = {}
    if os.path.exists(p(RUN_STATE)):
        with open(p(RUN_STATE)) as f:
            state = json.load(f)
    seed = state.get("seed")
    run_oracle_sha = state.get("oracle_jar_sha256")
    cur_oracle_sha = file_sha256(p(ORACLE_JAR)) if os.path.exists(p(ORACLE_JAR)) else None
    cur_cases_sha = file_sha256(p(CASES))
    missing, mism, recs, stale = [], [], [], []
    retries = []
    reruns = []
    per_source = collections.defaultdict(collections.Counter)
    per_profile = collections.defaultdict(collections.Counter)
    req_secs = 0.0
    golden_ms = 0
    servers_seen = set()
    # Per-segment tallies from the result files themselves, so a segment that never wrote its
    # end record (e.g. killed by a host reboot) is still reported with what it produced.
    seg_tally = collections.defaultdict(collections.Counter)
    seg_servers = collections.defaultdict(set)
    seg_last: dict[str, float] = {}
    for c, prof in pairs:
        rp = result_path(c["id"], prof)
        if not os.path.exists(rp):
            missing.append(f"{c['id']}/{prof}")
            continue
        with open(rp, encoding="utf-8") as f:
            r = json.load(f)
        # Is this result current?  (gate version, oracle jar, argv, golden bytes)
        why = []
        if r.get("gate_version") != GATE_VERSION:
            why.append(f"gate_version {r.get('gate_version')}")
        if r.get("oracle_jar_sha256") != run_oracle_sha or run_oracle_sha != cur_oracle_sha:
            why.append("oracle jar")
        out_dir = case_args.default_out_dir(c["id"], prof, profiles)
        if r.get("argv") is not None and r.get("argv") != case_args.compiler_args(
                c, prof, out_dir, profiles):
            why.append("argv")
        gp = golden_path(c["id"], prof)
        if r.get("golden_sha256") is not None and (
                not os.path.exists(gp) or file_sha256(gp) != r["golden_sha256"]):
            why.append("golden file changed")
        if why:
            stale.append(f"{c['id']}/{prof} ({', '.join(why)})")
            continue
        recs.append(r)
        golden_ms += r.get("golden_wall_ms") or 0
        req_secs += r["seconds"]
        servers_seen.add((r["segment"], r["server"], r.get("server_start_no")))
        seg_last[r["segment"]] = max(seg_last.get(r["segment"], 0.0), os.stat(rp).st_mtime)
        seg_tally[r["segment"]]["identical" if r["match"] else "mismatch"] += 1
        seg_tally[r["segment"]]["oom_retries"] += 1 if r.get("attempts") else 0
        seg_servers[r["segment"]].add((r["server"], r.get("server_start_no")))
        if r.get("attempts"):
            retries.append(r)
        if r.get("previous"):
            reruns.append(r)
        k = "identical" if r["match"] else "mismatch"
        per_source[c["source"]][k] += 1
        per_profile[prof][k] += 1
        if not r["match"]:
            mism.append(r)
    segs = state.get("segments", [])
    for s in segs:
        t = seg_tally.get(s["segment"], collections.Counter())
        s["current_results"] = {"identical": t["identical"], "mismatch": t["mismatch"],
                                "oom_retries": t["oom_retries"],
                                "server_processes": len(seg_servers.get(s["segment"], ()))}
        s["finished"] = "wall_s" in s
        if not s["finished"] and s["segment"] in seg_last and s.get("start"):
            # Interrupted (no end record): measured from its start to its last result file.
            s["wall_s_start_to_last_result"] = round(seg_last[s["segment"]] - s["start"], 1)
            s["last_result_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z",
                                                time.localtime(seg_last[s["segment"]]))
    wall_total = sum(s.get("wall_s", s.get("wall_s_start_to_last_result", 0)) for s in segs)
    gruns = golden_driver_runs()
    confirms = confirmation_reports()
    confirm_mism = [c for c in confirms if (c.get("mismatching") or 0) > 0]
    gsum = {}
    gs_path = p(f"{GOLDEN_ROOT}/_summary.json")
    if os.path.exists(gs_path):
        with open(gs_path) as f:
            gsum = json.load(f)
    gdrv = gsum.get("last_driver_run", {})
    identical = len(recs) - len(mism)
    passed = (not missing and not mism and not stale and len(recs) == len(pairs)
              and not confirm_mism)
    status = "PASS" if passed else ("FAIL" if (mism or confirm_mism) else "INCOMPLETE")
    jvms = [j for s in segs for j in s.get("server_jvms", [])]
    hwms = [j["vmhwm_kb"] for j in jvms if j.get("vmhwm_kb")]
    rep = {
        "gate": "0.1",
        "spec": "docs/PORTING.md §4.1: oracle compile byte-identical to the uberjar across the D2 corpus",
        "status": status,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "gate_version": GATE_VERSION,
        "cases_file": CASES, "cases": len(cases), "pairs_total": len(pairs),
        "compared": len(recs), "identical": identical, "mismatching": len(mism),
        "missing": len(missing), "missing_examples": missing[:100],
        "stale": len(stale), "stale_examples": stale[:100],
        "seed": seed, "order": "random.Random(seed).shuffle(all pairs in cases.jsonl order); "
                               "servers pull from that order; per-server sequences in "
                               f"{SERVER_LOGS}/",
        "reference_jar_sha256": rr.JAR_SHA256,
        "oracle_jar_sha256": run_oracle_sha,
        "oracle_jar_sha256_current": cur_oracle_sha,
        "oracle_jar_snapshot": state.get("oracle_jar_snapshot"),
        "cases_sha256": state.get("cases_sha256"),
        "cases_sha256_current": cur_cases_sha,
        "golden_root": GOLDEN_ROOT,
        "golden_files_verified": "sha256 of every compared golden file re-checked by this report",
        "compared_fields": ["exit_code", "stdout", "stderr", "every file in out_dir (names + bytes)",
                            "oracle output_files report == files on disk"],
        "normalizations": [],
        "oom_retries": [{"case_id": r["case_id"], "profile": r["profile"],
                         "attempts": r["attempts"], "match": r["match"]} for r in retries],
        "reruns": [{"case_id": r["case_id"], "profile": r["profile"], "match": r["match"],
                    "previous": r["previous"]} for r in reruns],
        "segments": segs,
        # Finished segments record every server JVM (incl. restarts); an unfinished one only
        # through the (segment, server, start_no) of its results.
        "server_jvm_processes": len(jvms) + sum(
            s["current_results"]["server_processes"] for s in segs if not s["finished"])
        if jvms else len(servers_seen),
        "memory": {
            "heap_budget_gb": max((s.get("heap_budget_gb") or 0) for s in segs) if segs else None,
            "heap_worst_case_gb": max((s.get("heap_worst_case_gb") or 0) for s in segs)
            if segs else None,
            "server_vmhwm_kb_max": max(hwms) if hwms else None,
            "server_vmhwm_kb_median": sorted(hwms)[len(hwms) // 2] if hwms else None,
            "host_mem_available_kb_min": min((s.get("host_mem_available_kb_min") or 1 << 62)
                                             for s in segs) if segs else None,
        },
        "confirmation_runs": confirms,
        "timing": {
            "gate_wall_s": round(wall_total, 1),
            "gate_wall_note": "sum over segments of the segment wall; a segment without an end "
                              "record (interrupted) counts from its start to its last result "
                              "file (mtime)",
            "golden_driver_runs": gruns,
            "golden_driver_wall_s_total": round(sum(g["wall_s"] for g in gruns), 1),
            "golden_driver_pairs_total": sum(g["pairs"] for g in gruns),
            "gate_sum_request_s": round(req_secs, 1),
            "golden_sum_run_wall_s_for_compared_pairs": round(golden_ms / 1000, 1),
            "golden_driver_wall_s": gdrv.get("wall_s"),
            "golden_driver_jobs": gdrv.get("jobs"),
            "golden_driver_pairs": gdrv.get("todo"),
            "note": "golden: one fresh `java -jar` per pair (C1-only JIT, AppCDS, -Xmx3g), "
                    "with the number of parallel JVMs listed per driver run, on a host with "
                    "load average 70-100 (driver runs are taken from the golden_all.py "
                    "end-of-run summaries in build/logs/*.log); gate: long-lived oracle servers "
                    "(see segments for count, heap and host load). Both sides ran next to "
                    "other workloads, so the wall times are indicative, not a benchmark. "
                    "Per-request times include the driver's JSON/base64 handling.",
        },
        "per_source": {k: dict(v) for k, v in sorted(per_source.items())},
        "per_profile": {k: dict(v) for k, v in sorted(per_profile.items())},
        "mismatches": [{"case_id": r["case_id"], "profile": r["profile"],
                        "server": r["server"], "segment": r["segment"],
                        "server_start_no": r.get("server_start_no"),
                        "server_pos": r["server_pos"], "order_index": r["order_index"],
                        "diffs": r["diffs"]} for r in mism],
    }
    write_json_atomic(p(REPORT_JSON), rep)

    t = rep["timing"]
    mem = rep["memory"]
    L = []
    L.append("# Gate 0.1 report: oracle server vs golden uberjar\n")
    L.append(f"**Status: {rep['status']}** ({identical} / {len(pairs)} pairs identical, "
             f"{len(mism)} mismatching, {len(missing)} not compared, {len(stale)} stale)\n")
    L.append("Generated by `gates/gate_0_1.sh` (`gates/lib/gate01.py report`, gate version "
             f"{GATE_VERSION}) at {rep['generated_at']}.\n")
    L.append("## What was compared\n")
    L.append(f"- Corpus: `{CASES}` (sha256 `{rep['cases_sha256']}`), {len(cases)} cases, "
             f"{len(pairs)} (case, profile) pairs (each case's `profiles` list).")
    L.append(f"- Golden: `{GOLDEN_ROOT}/<case>/<profile>.json`, one `java -jar` of the reference "
             f"uberjar (sha256 `{rr.JAR_SHA256}`) per pair. The report re-hashes every golden "
             "file and checks it is the one the gate compared against.")
    L.append(f"- Oracle: `{ORACLE_JAR}` (sha256 `{rep['oracle_jar_sha256']}`; current file "
             f"`{cur_oracle_sha}`), run from the snapshot `{rep['oracle_jar_snapshot']}` as "
             "`closurers.oracle.Main server --isolate=none`, `op: compile`.")
    L.append("- argv: `gates/lib/case_args.py` `compiler_args(case, profile)` with the default "
             "out_dir, cwd = repository root; the gate also checks it equals the golden "
             "`compiler_args`. Server environment = the golden child environment "
             "(`run_reference.child_env()`); every server start checks the JVM-derived locale, "
             "encodings and time zone in its ready line against `oracle_client.GOLDEN_JVM_ENV` "
             "and aborts on a difference (see `oracle/PROTOCOL.md`, Environment).")
    L.append("- Fields, byte for byte: exit code, stdout, stderr, the set of files in the out_dir "
             "and each file's bytes; plus the oracle's own `output_files` report must equal the "
             "files on disk.")
    L.append("- Heap: the servers run with a smaller `-Xmx` than the golden `-Xmx3g` to fit the "
             "host heap budget (see segments). Compiler output does not depend on the heap "
             "size: the only reads of JVM memory figures in the reference `src/` are "
             "`Platform.freeMemory/totalMemory` (used by `PerformanceTracker`, i.e. "
             "`--tracer_mode`) and `JvmMetrics`, which no profile enables. A request that hits "
             "`OutOfMemoryError` is retried at `-Xmx3g`, and a request that ended in a server "
             "failure / oracle error may be re-run by `--redo-failed`; every such request is "
             "listed below with all its attempts.")
    L.append("- **Normalizations: none.** All bytes are compared raw.\n")
    L.append("## Order and servers\n")
    L.append(f"- Seed **{seed}**: `random.Random({seed}).shuffle(pairs)`; the servers pull from "
             "this one shuffled queue, so every server sees an unrelated mix of cases and "
             "profiles. Each server's exact request sequence (position, case, profile) is in "
             f"`{SERVER_LOGS}/<segment>-s<N>.jsonl` for replay.")
    for s in segs:
        if not s["finished"]:
            cr = s["current_results"]
            L.append(f"- Segment `{s['segment']}`: {s.get('servers')} servers at "
                     f"`-Xmx{s.get('xmx')}` (OOM retry at `-Xmx{s.get('oom_xmx')}`, worst-case "
                     f"heap {s.get('heap_worst_case_gb')} GB of a {s.get('heap_budget_gb')} GB "
                     f"budget), JVM flags `{' '.join(s.get('jvm_flags', []))}`, host load "
                     f"{[round(x, 1) for x in s.get('host_loadavg_at_start', [])]} at start. "
                     f"**Interrupted**: it never wrote its end record (the process was killed, "
                     f"e.g. by a host reboot), so its server peak RSS and host-memory minimum "
                     f"are unknown; its wall time is measured from its start to its last result "
                     f"file ({s.get('wall_s_start_to_last_result', '?')} s, last result at "
                     f"{s.get('last_result_at', '?')}). Current results it "
                     f"produced (each one complete and individually verified by this report): "
                     f"{cr['identical']} identical, {cr['mismatch']} mismatching, "
                     f"{cr['oom_retries']} OOM retries, from {cr['server_processes']} server "
                     f"processes; the next segment resumed with the remaining pairs.")
            continue
        L.append(f"- Segment `{s['segment']}`: {s.get('servers')} servers at "
                 f"`-Xmx{s.get('xmx')}` (OOM retry at `-Xmx{s.get('oom_xmx')}`, worst-case heap "
                 f"{s.get('heap_worst_case_gb')} GB of a {s.get('heap_budget_gb')} GB budget), "
                 f"JVM flags `{' '.join(s.get('jvm_flags', []))}`, {s.get('done', '?')} / "
                 f"{s.get('todo')} pairs, wall {s.get('wall_s', '?')} s, results "
                 f"{s.get('results', {})}, OOM retries {s.get('oom_retries', '?')}, host load "
                 f"{[round(x, 1) for x in s.get('host_loadavg_at_start', [])]} at start.")
    L.append(f"- Server JVM processes (incl. restarts): {rep['server_jvm_processes']}.")
    if any(not s["finished"] for s in segs):
        L.append("- Peak RSS and lowest MemAvailable below cover finished segments only.")
    L.append("")
    L.append("## Memory\n")
    for sg in segs:
        L.append(f"- Segment `{sg['segment']}`: heap budget {sg.get('heap_budget_gb')} GB, "
                 f"worst-case committed heap of the pool {sg.get('heap_worst_case_gb')} GB "
                 f"({sg.get('servers')} x {sg.get('xmx')} + one server at a time at "
                 f"{sg.get('oom_xmx')} for an OOM retry)"
                 + ("" if (sg.get("heap_budget_gb") or 0) <= 14.0 else
                    "; **above the default 14 GB host limit**: the budget was raised explicitly "
                    "with `--heap-budget-gb` by whoever launched that segment")
                 + ".")
    if mem["server_vmhwm_kb_max"]:
        L.append(f"- Server peak RSS (VmHWM): max {mem['server_vmhwm_kb_max'] // 1024} MB, "
                 f"median {mem['server_vmhwm_kb_median'] // 1024} MB.")
    if mem["host_mem_available_kb_min"] and mem["host_mem_available_kb_min"] < 1 << 61:
        L.append(f"- Lowest host MemAvailable seen between requests: "
                 f"{mem['host_mem_available_kb_min'] // 1024} MB.")
    L.append("")
    L.append("## OutOfMemoryError retries\n")
    if not retries:
        L.append("None: no request hit an OutOfMemoryError at the server heap size.\n")
    else:
        L.append("Each request below hit `java.lang.OutOfMemoryError` at the server heap size "
                 "(the golden run did not) and was retried once on a fresh server at the golden "
                 "heap size. Justification: heap size is a resource limit of the host, not "
                 "compiler behaviour; the server is restarted because an OOM can leave static "
                 "state half-initialised.\n")
        for r in retries:
            L.append(f"- `{r['case_id']}` / `{r['profile']}`: attempts {r['attempts']}, final "
                     f"{'identical' if r['match'] else 'MISMATCH'}")
        L.append("")
    L.append("## Re-runs of failed requests (`--redo-failed`)\n")
    if not reruns:
        L.append("None.\n")
    else:
        L.append("These pairs first ended in a server failure / oracle error (never a "
                 "compiler-output difference) and were re-run by a later `--redo-failed` "
                 "segment. The earlier attempts are kept in the result file (`previous`):\n")
        for r in reruns:
            prev = "; ".join(
                f"segment {x.get('segment')} server {x.get('server')} pos {x.get('server_pos')}: "
                + ", ".join(f"{d['field']}={str(d.get('detail', ''))[:120]!r}"
                            for d in (x.get('diffs') or []))
                for x in r["previous"])
            L.append(f"- `{r['case_id']}` / `{r['profile']}`: earlier: {prev}. Now: "
                     f"{'identical' if r['match'] else 'MISMATCH'}"
                     + (f" (attempts {r['attempts']})" if r.get("attempts") else ""))
        L.append("")
    L.append("## Timing\n")
    L.append("| | Wall time |")
    L.append("|---|---|")
    for sg in segs:
        w = sg.get("wall_s", sg.get("wall_s_start_to_last_result"))
        n = sg.get("done") if sg["finished"] else (sg["current_results"]["identical"]
                                                   + sg["current_results"]["mismatch"])
        rate = f", {n / w:.1f} pairs/s" if w and n else ""
        L.append(f"| Gate segment `{sg['segment']}` ({sg.get('servers')} servers, {n} pairs"
                 f"{rate}){'' if sg['finished'] else ', interrupted: start to last result'} | "
                 f"{w} s |")
    L.append(f"| **Gate total** (all segments, {len(recs)} pairs) | **{t['gate_wall_s']} s** |")
    L.append(f"| Gate, sum of per-request times | {t['gate_sum_request_s']} s |")
    for g in t["golden_driver_runs"]:
        L.append(f"| Golden driver run (`golden_all.py`, {g['jobs']} JVMs, {g['pairs']} pairs, "
                 f"{g['pairs'] / g['wall_s']:.1f} pairs/s; `{g['log']}`) | {g['wall_s']} s |")
    if t["golden_driver_runs"]:
        L.append(f"| **Golden total** (all driver runs found in `build/logs/`, "
                 f"{t['golden_driver_pairs_total']} pairs, incl. pairs later dropped from "
                 f"`cases.jsonl`) | "
                 f"**{t['golden_driver_wall_s_total']} s** |")
    else:
        L.append(f"| Golden last driver run (`golden_all.py`, {t['golden_driver_jobs']} JVMs, "
                 f"{t['golden_driver_pairs']} pairs) | {t['golden_driver_wall_s']} s |")
    L.append(f"| Golden, sum of per-run walls for the {len(recs)} compared pairs | "
             f"{t['golden_sum_run_wall_s_for_compared_pairs']} s |")
    if t["gate_sum_request_s"]:
        L.append(f"| Ratio golden per-run sum / gate per-request sum | "
                 f"{t['golden_sum_run_wall_s_for_compared_pairs'] / t['gate_sum_request_s']:.1f}x |")
    L.append("")
    L.append(t["note"] + "\n")
    if WORK == DEFAULT_WORK:
        L.append("## Independent confirmation runs\n")
        if not confirms:
            L.append("None (`GATE01_WORK=build/gate01/confirm-s<seed> gates/gate_0_1.sh --seed "
                     "<seed>` runs one; its mismatches would fail this gate).\n")
        else:
            L.append("Full re-runs of every pair from scratch in their own work directory "
                     "(`GATE01_WORK=build/gate01/confirm-s<seed>`), with another seed, so every "
                     "server sees a different request sequence. Any mismatch there fails this "
                     "gate.\n")
            L.append("| Run | Seed | Status | Identical / pairs | Mismatching | Wall | Report |")
            L.append("|---|---|---|---|---|---|---|")
            for cr in confirms:
                L.append(f"| `{cr['work']}` | {cr.get('seed')} | {cr.get('status')} | "
                         f"{cr.get('identical')} / {cr.get('pairs_total')} | "
                         f"{cr.get('mismatching')} | {cr.get('gate_wall_s')} s | "
                         f"`{cr.get('report')}` |")
            L.append("")
    L.append("## Per source\n")
    L.append("| Source | identical | mismatch |")
    L.append("|---|---|---|")
    for k, v in rep["per_source"].items():
        L.append(f"| {k} | {v.get('identical', 0)} | {v.get('mismatch', 0)} |")
    L.append("\n## Per profile\n")
    L.append("| Profile | identical | mismatch |")
    L.append("|---|---|---|")
    for k, v in rep["per_profile"].items():
        L.append(f"| {k} | {v.get('identical', 0)} | {v.get('mismatch', 0)} |")
    L.append("\n## Mismatches\n")
    if not mism:
        L.append("None.\n")
    for r in mism:
        L.append(f"### `{r['case_id']}` / `{r['profile']}`\n")
        L.append(f"server {r['server']} (segment {r['segment']}, JVM start "
                 f"{r.get('server_start_no')}), request position {r['server_pos']} on that JVM, "
                 f"order index {r['order_index']}.\n")
        for d in r["diffs"]:
            L.append(f"- **{d['field']}**: " + ", ".join(
                f"{k}={d[k]}" for k in ("detail", "golden", "oracle", "golden_len", "oracle_len",
                                       "first_diff_offset") if k in d))
            if "unified_diff_head" in d and d["unified_diff_head"]:
                L.append("```diff\n" + md_escape_block("\n".join(d["unified_diff_head"][:40]))
                         + "\n```")
            elif "golden_context" in d:
                L.append("golden: ```" + md_escape_block(d["golden_context"]) + "```  ")
                L.append("oracle: ```" + md_escape_block(d["oracle_context"]) + "```")
        L.append("")
    if stale:
        L.append("## Stale results\n")
        L.append(f"{len(stale)} results were produced against a different oracle jar, gate "
                 "version, argv or golden file and do not count (re-run `gates/gate_0_1.sh`). "
                 "First ones: " + ", ".join(f"`{m}`" for m in stale[:20]) + "\n")
    if missing:
        L.append("## Not compared\n")
        L.append(f"{len(missing)} pairs have no result yet (run `gates/gate_0_1.sh` to resume). "
                 "First ones: " + ", ".join(f"`{m}`" for m in missing[:20]) + "\n")
    os.makedirs(os.path.dirname(p(REPORT_MD)), exist_ok=True)
    with open(p(REPORT_MD) + ".tmp", "w", encoding="utf-8") as f:
        f.write("\n".join(L) + "\n")
    os.replace(p(REPORT_MD) + ".tmp", p(REPORT_MD))
    print(json.dumps({k: rep[k] for k in ("status", "pairs_total", "compared", "identical",
                                          "mismatching", "missing", "stale", "seed")}))
    return 0 if passed else (1 if mism else 2)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--seed", type=int)
    r.add_argument("--servers", type=int, default=8)
    r.add_argument("--xmx", default="1536m")
    r.add_argument("--oom-xmx", default=rr.DEFAULT_XMX,
                   help="heap for the one retry after an OutOfMemoryError (golden: 3g)")
    r.add_argument("--heap-budget-gb", type=float, default=14.0)
    r.add_argument("--reserve-gb", type=float, default=3.0)
    r.add_argument("--timeout", type=int, default=600)
    r.add_argument("--limit", type=int)
    r.add_argument("--fresh", action="store_true")
    r.add_argument("--redo-failed", action="store_true")
    sub.add_parser("report")
    a = ap.parse_args()
    return cmd_run(a) if a.cmd == "run" else cmd_report(a)


if __name__ == "__main__":
    sys.exit(main())
