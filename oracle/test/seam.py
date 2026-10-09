#!/usr/bin/env python3
"""Seam measurement (oracle/PROTOCOL.md, "Seam caveats"): compile(argv) vs optimize_from_typedast(checks_to_typedast(argv)).

Usage: python3 oracle/test/seam.py [--n 320] [--servers 3] [--isolate request] [--seed 20261006]
Resumable: one result file per pair under build/oracle/seam/results/<idx>.json.
"""
import argparse
import base64
import hashlib
import json
import os
import random
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))
from case_args import ROOT, load_cases, input_bytes, case_args  # noqa: E402
from oracle_client import Oracle  # noqa: E402

OUT = os.path.join(ROOT, "build/oracle/seam")
METHOD = "input_order"


def opt_args(args, input_order=None):
    """argv for the optimize step: same flags, but dependency management off (TypedAST inputs
    are incompatible with sorting/pruning: Compiler.initOptions throws otherwise).

    method "argv": keep the --js files in argv order (v1).
    method "input_order": list --js files in the order the checks run's compiler held them after
    dependency sorting/pruning (checks response "input_order"), dropping pruned files (v2)."""
    out = [a for a in args if not a.startswith("--entry_point=") and
           not a.startswith("--dependency_mode=")]
    if input_order is not None:
        js = [a[len("--js="):] for a in out if a.startswith("--js=")]
        keep = [n for n in input_order if n in js]
        out = [a for a in out if not a.startswith("--js=")]
        ext = [a for a in out if a.startswith("--externs=")]
        rest = [a for a in out if not a.startswith("--externs=")]
        out = ext + ["--js=" + n for n in keep] + rest
    out.append("--dependency_mode=NONE")
    return out


def h(b64s):
    if b64s is None:
        return None
    return hashlib.sha256(base64.b64decode(b64s)).hexdigest()


def select(n, seed, max_bytes):
    cases = load_cases()
    pairs = []
    for c in cases:
        sz = input_bytes(c)
        if sz is None or sz > max_bytes:
            continue
        for p in ("simple", "advanced"):
            if p in c["profiles"]:
                pairs.append((c["id"], p))
    rnd = random.Random(seed)
    rnd.shuffle(pairs)
    byid = {c["id"]: c for c in cases}
    return [(i, byid[cid], p) for i, (cid, p) in enumerate(pairs[:n])]


def run_pair(o, idx, case, prof):
    args = case_args(case, prof)
    t0 = time.time()
    r1 = o.request({"op": "compile", "args": args})
    r2 = o.request({"op": "checks_to_typedast", "args": args})
    rec = {"idx": idx, "case": case["id"], "profile": prof, "args": args, "method": METHOD,
           "compile": {k: r1.get(k) for k in ("ok", "exit_code", "error")},
           "checks": {k: r2.get(k) for k in ("ok", "exit_code", "error")}}
    rec["compile"]["stdout_sha"] = h(r1.get("stdout_b64"))
    rec["compile"]["stderr_sha"] = h(r1.get("stderr_b64"))
    files = {}
    if not r2.get("ok") or not r2.get("typedast_b64"):
        rec["class"] = "checks_no_typedast"
        files["checks.stderr"] = r2.get("stderr_b64")
        files["compile.stdout"] = r1.get("stdout_b64")
        files["compile.stderr"] = r1.get("stderr_b64")
    else:
        oa = opt_args(args, r2.get("input_order") if METHOD == "input_order" else None)
        r3 = o.request({"op": "optimize_from_typedast", "args": oa,
                        "typedast_b64": r2["typedast_b64"]})
        rec["opt_args"] = oa
        rec["optimize"] = {k: r3.get(k) for k in ("ok", "exit_code", "error")}
        rec["optimize"]["stdout_sha"] = h(r3.get("stdout_b64"))
        rec["optimize"]["stderr_sha"] = h(r3.get("stderr_b64"))
        rec["typedast_bytes"] = len(base64.b64decode(r2["typedast_b64"]))
        same_out = rec["compile"]["stdout_sha"] == rec["optimize"]["stdout_sha"]
        same_rc = r1.get("exit_code") == r3.get("exit_code")
        same_err = rec["compile"]["stderr_sha"] == rec["optimize"]["stderr_sha"]
        rec["identical_stdout"] = same_out
        rec["identical_exit"] = same_rc
        rec["identical_stderr"] = same_err
        if not (same_out and same_rc):
            rec["class"] = "differs"
            files["compile.stdout"] = r1.get("stdout_b64")
            files["compile.stderr"] = r1.get("stderr_b64")
            files["checks.stderr"] = r2.get("stderr_b64")
            files["optimize.stdout"] = r3.get("stdout_b64")
            files["optimize.stderr"] = r3.get("stderr_b64")
        else:
            rec["class"] = "identical" if same_err else "identical_stdout_stderr_differs"
            if not same_err:
                files["compile.stderr"] = r1.get("stderr_b64")
                files["checks.stderr"] = r2.get("stderr_b64")
                files["optimize.stderr"] = r3.get("stderr_b64")
    rec["seconds"] = round(time.time() - t0, 2)
    if files:
        d = os.path.join(OUT, "diffs", str(idx))
        os.makedirs(d, exist_ok=True)
        for k, v in files.items():
            if v is not None:
                with open(os.path.join(d, k), "wb") as f:
                    f.write(base64.b64decode(v))
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=320)
    ap.add_argument("--servers", type=int, default=3)
    ap.add_argument("--isolate", default="request")
    ap.add_argument("--seed", type=int, default=20261006)
    ap.add_argument("--max-bytes", type=int, default=300_000)
    ap.add_argument("--xmx", default="2g")
    ap.add_argument("--method", default="input_order", choices=["argv", "input_order"])
    a = ap.parse_args()
    global OUT, METHOD
    METHOD = a.method
    if a.method == "input_order":
        OUT = os.path.join(OUT, "v2")
    os.makedirs(os.path.join(OUT, "results"), exist_ok=True)
    work = [w for w in select(a.n, a.seed, a.max_bytes)
            if not os.path.exists(os.path.join(OUT, "results", f"{w[0]}.json"))]
    print(f"{len(work)} pairs to run", flush=True)
    lock = threading.Lock()

    def worker():
        o = Oracle(isolate=a.isolate, xmx=a.xmx)
        try:
            while True:
                with lock:
                    if not work:
                        return
                    idx, case, prof = work.pop(0)
                try:
                    rec = run_pair(o, idx, case, prof)
                except Exception as e:  # server death: record and restart the server
                    rec = {"idx": idx, "case": case["id"], "profile": prof,
                           "class": "oracle_failure", "error": repr(e)}
                    o.close()
                    o = Oracle(isolate=a.isolate, xmx=a.xmx)
                with open(os.path.join(OUT, "results", f"{idx}.json"), "w") as f:
                    json.dump(rec, f)
                print(idx, rec["class"], rec.get("seconds"), flush=True)
        finally:
            o.close()

    ts = [threading.Thread(target=worker) for _ in range(a.servers)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    print("done", flush=True)


if __name__ == "__main__":
    main()
