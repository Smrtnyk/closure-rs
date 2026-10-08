#!/usr/bin/env python3
"""Seam measurement v3: compile(argv) vs optimize_from_typedast(checks_to_typedast(argv)),
on every D2 profile, comparing exit code, stdout, stderr AND every output file.

What v3 carries across the seam (v2 carried only a flat input_order):
  * per-input SourceKind after dependency management (checks response "chunks"/"inputs"):
    inputs that are WEAK in the checks run (goog.requireType-only deps) are sent to
    optimize_from_typedast in "weak_inputs", so stage 2 marks them WEAK and
    RemoveWeakSources empties them, as in the full compile;
  * per-chunk membership after dependency management (checks response "chunks"): the
    optimize argv's --js order and --chunk=name:count:deps flags are rebuilt from it.

Argv comes from gates/lib/case_args.compiler_args (the golden argv), with the out_dir moved
under <out>/tmp/ so it never collides with golden or Gate 0.1 runs.

Usage: python3 oracle/test/seam3.py [--per-profile 12] [--servers 6] [--seed 20261006]
                                   [--out build/oracle/seam/v3]
Resumable: one result file per pair under <out>/results/<profile>-<k>.json.
Every oracle JVM runs in oracle_client.golden_env() and is checked against GOLDEN_JVM_ENV
(oracle/PROTOCOL.md, "Environment"); each result stores the server's ready-line env as
"jvm_env".
"""
import argparse
import base64
import hashlib
import json
import os
import random
import shutil
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "gates", "lib"))
import case_args  # noqa: E402
from oracle_client import Oracle  # noqa: E402

OUT = os.path.join(ROOT, "build/oracle/seam/v3")
SPECIAL_CHUNKS = ("$strong$", "$weak$")


def h(b64s):
    if b64s is None:
        return None
    return hashlib.sha256(base64.b64decode(b64s)).hexdigest()


def opt_args(args, checks):
    """Optimize argv from the checks response. Returns (argv, weak_inputs).

    Chunk fill files ("<chunk>$fillFile", added by Compiler.fillEmptyChunks to chunks that
    dependency management emptied) are kept as --js entries; the request's "fill_inputs"
    makes the oracle create them as empty in-memory inputs (a root chunk may not be empty)."""
    drop = ("--entry_point=", "--dependency_mode=", "--js=", "--chunk=")
    rest = [a for a in args if not a.startswith(drop)]
    ext = [a for a in rest if a.startswith("--externs=")]
    rest = [a for a in rest if not a.startswith("--externs=")]
    chunks = checks.get("chunks") or []
    weak = [i["name"] for c in chunks for i in c["inputs"]
            if i["kind"] == "WEAK" and not i["fill_file"]]
    user = [c for c in chunks if c["name"] not in SPECIAL_CHUNKS]
    js, cflags = [], []
    if user:
        for c in user:
            names = [i["name"] for i in c["inputs"] if i["kind"] != "WEAK"]
            js += names
            cflags.append([c["name"], len(names), [d for d in c["deps"] if d not in SPECIAL_CHUNKS]])
        # Weak inputs go last, counted in the last chunk; JSChunkGraph moves sources marked
        # WEAK into the $weak$ chunk (JSChunkGraph.moveMarkedWeakSources).
        js += weak
        cflags[-1][1] += len(weak)
        cfl = [f"--chunk={n}:{k}" + (":" + ",".join(d) if d else "") for n, k, d in cflags]
    else:
        js = [i["name"] for i in (checks.get("inputs") or [])
              if not case_args_is_fill(i["name"])]
        cfl = []
    return ext + ["--js=" + n for n in js] + cfl + rest + ["--dependency_mode=NONE"], weak


def case_args_is_fill(name):
    return name.endswith("$fillFile")


def clean(d):
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d, exist_ok=True)


def files_of(r, out_dir):
    """{basename: sha} of the response's output files under out_dir."""
    out = {}
    for k, v in (r.get("output_files") or {}).items():
        out[os.path.relpath(k, out_dir) if k.startswith(out_dir) else k] = h(v)
    return out


def run_pair(o, key, case, prof):
    out_dir = os.path.join(os.path.relpath(OUT, ROOT), "tmp", key)
    args = case_args.compiler_args(case, prof, out_dir=out_dir)
    t0 = time.time()
    clean(os.path.join(ROOT, out_dir))
    r1 = o.request({"op": "compile", "args": args})
    clean(os.path.join(ROOT, out_dir))
    r2 = o.request({"op": "checks_to_typedast", "args": args})
    rec = {"key": key, "case": case["id"], "profile": prof, "args": args,
           "compile": {"ok": r1.get("ok"), "exit_code": r1.get("exit_code"),
                       "stdout_sha": h(r1.get("stdout_b64")), "stderr_sha": h(r1.get("stderr_b64")),
                       "files": files_of(r1, out_dir)},
           "checks": {"ok": r2.get("ok"), "exit_code": r2.get("exit_code"),
                      "chunks": r2.get("chunks")}}
    diffs = {"compile.stdout": r1.get("stdout_b64"), "compile.stderr": r1.get("stderr_b64")}
    for k, v in (r1.get("output_files") or {}).items():
        diffs["compile." + os.path.basename(k)] = v
    if not r2.get("ok") or not r2.get("typedast_b64"):
        rec["class"] = "checks_no_typedast"
        diffs["checks.stderr"] = r2.get("stderr_b64")
    else:
        oa, weak = opt_args(args, r2)
        clean(os.path.join(ROOT, out_dir))
        fill = [a[len("--js="):] for a in oa if a.startswith("--js=") and a.endswith("$fillFile")]
        r3 = o.request({"op": "optimize_from_typedast", "args": oa, "weak_inputs": weak,
                        "fill_inputs": fill,
                        "typedast_b64": r2["typedast_b64"]})
        rec["opt_args"], rec["weak_inputs"] = oa, weak
        rec["optimize"] = {"ok": r3.get("ok"), "exit_code": r3.get("exit_code"),
                           "error": (r3.get("error") or "")[:2000] or None,
                           "stdout_sha": h(r3.get("stdout_b64")),
                           "stderr_sha": h(r3.get("stderr_b64")), "files": files_of(r3, out_dir)}
        c, p = rec["compile"], rec["optimize"]
        rec["identical_exit"] = c["exit_code"] == p["exit_code"]
        rec["identical_stdout"] = c["stdout_sha"] == p["stdout_sha"]
        rec["identical_files"] = c["files"] == p["files"]
        rec["identical_stderr"] = c["stderr_sha"] == p["stderr_sha"]
        rec["differing_files"] = sorted(k for k in set(c["files"]) | set(p["files"])
                                        if c["files"].get(k) != p["files"].get(k))
        if rec["identical_exit"] and rec["identical_stdout"] and rec["identical_files"]:
            rec["class"] = "identical" if rec["identical_stderr"] else "identical_js_stderr_differs"
        else:
            rec["class"] = "differs"
        diffs["checks.stderr"] = r2.get("stderr_b64")
        diffs["optimize.stdout"] = r3.get("stdout_b64")
        diffs["optimize.stderr"] = r3.get("stderr_b64")
        for k, v in (r3.get("output_files") or {}).items():
            diffs["optimize." + os.path.basename(k)] = v
    shutil.rmtree(os.path.join(ROOT, out_dir), ignore_errors=True)
    rec["seconds"] = round(time.time() - t0, 2)
    if rec["class"] != "identical":
        d = os.path.join(OUT, "diffs", key)
        os.makedirs(d, exist_ok=True)
        for k, v in diffs.items():
            if v is not None:
                with open(os.path.join(d, k), "wb") as f:
                    f.write(base64.b64decode(v))
    return rec


def select(per_profile, seed, max_bytes, only=None):
    cases = [json.loads(l) for l in open(os.path.join(ROOT, "corpus/d2/cases.jsonl"))]
    profiles = case_args.load_profiles()
    by_prof = {}
    for c in cases:
        try:
            sz = sum(os.path.getsize(os.path.join(ROOT, f))
                     for f in c["inputs"] + c["shims"] + c["externs"])
        except OSError:
            continue
        if sz > max_bytes:
            continue
        for p in case_args.case_profiles(c, profiles):
            by_prof.setdefault(p, []).append(c)
    rnd = random.Random(seed)
    work = []
    for p in sorted(by_prof):
        if only and p not in only:
            continue
        cs = list(by_prof[p])
        rnd.shuffle(cs)
        for k, c in enumerate(cs[:per_profile]):
            work.append((f"{p}-{k}", c, p))
    return work


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--per-profile", type=int, default=12)
    ap.add_argument("--servers", type=int, default=6)
    ap.add_argument("--seed", type=int, default=20261006)
    ap.add_argument("--max-bytes", type=int, default=400_000)
    ap.add_argument("--xmx", default="2g")
    ap.add_argument("--profiles", default="")
    ap.add_argument("--case", default="", help="run one case id under --profiles only")
    ap.add_argument("--out", default="build/oracle/seam/v3",
                    help="run directory (repo-relative); results/, diffs/ and tmp/ go under it")
    a = ap.parse_args()
    global OUT
    OUT = os.path.join(ROOT, a.out)
    os.makedirs(os.path.join(OUT, "results"), exist_ok=True)
    only = [p for p in a.profiles.split(",") if p]
    if a.case:
        c = next(json.loads(l) for l in open(os.path.join(ROOT, "corpus/d2/cases.jsonl"))
                 if json.loads(l)["id"] == a.case)
        work = [(f"case-{c['id']}-{p}", c, p) for p in only]
    else:
        work = select(a.per_profile, a.seed, a.max_bytes, only)
    work = [w for w in work if not os.path.exists(os.path.join(OUT, "results", f"{w[0]}.json"))]
    print(f"{len(work)} pairs to run", flush=True)
    lock = threading.Lock()
    os.chdir(ROOT)

    def worker():
        o = Oracle(isolate="none", xmx=a.xmx)
        try:
            while True:
                with lock:
                    if not work:
                        return
                    key, case, prof = work.pop(0)
                try:
                    rec = run_pair(o, key, case, prof)
                    rec["jvm_env"] = o.env  # PROTOCOL.md "Environment" (golden_env, checked)
                except Exception as e:  # server death: record and restart the server
                    rec = {"key": key, "case": case["id"], "profile": prof,
                           "class": "oracle_failure", "error": repr(e)}
                    o.close()
                    o = Oracle(isolate="none", xmx=a.xmx)
                with open(os.path.join(OUT, "results", f"{key}.json"), "w") as f:
                    json.dump(rec, f)
                print(key, rec["class"], rec.get("seconds"), rec.get("differing_files", ""),
                      flush=True)
        finally:
            o.close()

    ts = [threading.Thread(target=worker) for _ in range(max(1, a.servers))]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    print("done", flush=True)


if __name__ == "__main__":
    main()
