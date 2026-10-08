#!/usr/bin/env python3
"""Server identity: >=300 compile requests through ONE server (seeded shuffled order) vs one fresh
JVM per request (`Main compile ARGV`), byte-compared (stdout, stderr, exit code). Every 5th
request is also run with `java -jar closure-compiler.jar ARGV` (Gate 0.1 spot check).

Usage: python3 oracle/test/identity.py [--isolate none|request] [--n 300] [--cli-par 3]
                                     [--out build/oracle/identity]
Resumable: fresh-JVM results are cached per request under <out>/cli/.
Every JVM (server, fresh oracle JVM, java -jar) runs in oracle_client.golden_env(); the server's
ready-line env is checked against GOLDEN_JVM_ENV (oracle/PROTOCOL.md, "Environment").
"""
import argparse
import hashlib
import json
import os
import random
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))
from case_args import ROOT, load_cases, input_bytes, case_args  # noqa: E402
from oracle_client import Oracle, cli_compile, JAVA, b64, golden_env  # noqa: E402

OUT = os.path.join(ROOT, "build/oracle/identity")
JAR = os.path.join(ROOT, "build/reference/closure-compiler.jar")


def sha(b):
    return hashlib.sha256(b).hexdigest()


def select(n, seed, max_bytes):
    cases = load_cases()
    pairs = []
    for c in cases:
        sz = input_bytes(c)
        if sz is None or sz > max_bytes:
            continue
        for p in ("ws", "simple", "advanced", "pretty", "lang_es5"):
            if p in c["profiles"]:
                pairs.append((c, p))
    rnd = random.Random(seed)
    rnd.shuffle(pairs)
    return [(i, c, p) for i, (c, p) in enumerate(pairs[:n])]


def cli_one(item):
    idx, case, prof = item
    path = os.path.join(OUT, "cli", f"{idx}.json")
    if os.path.exists(path):
        with open(path) as f:
            return json.load(f)
    args = case_args(case, prof)
    rc, out, err = cli_compile(args)
    rec = {"idx": idx, "case": case["id"], "profile": prof, "args": args, "rc": rc,
           "stdout_sha": sha(out), "stderr_sha": sha(err), "stdout_len": len(out)}
    if idx % 5 == 0:
        p = subprocess.run([JAVA, "-Xmx2g", "-jar", JAR, *args], cwd=ROOT, env=golden_env(),
                           stdin=subprocess.DEVNULL, capture_output=True, timeout=600)
        rec["jar"] = {"rc": p.returncode, "stdout_sha": sha(p.stdout), "stderr_sha": sha(p.stderr)}
        if rec["jar"] != {"rc": rc, "stdout_sha": rec["stdout_sha"], "stderr_sha": rec["stderr_sha"]}:
            d = os.path.join(OUT, "jar_diffs", str(idx))
            os.makedirs(d, exist_ok=True)
            for name, data in (("jar.stdout", p.stdout), ("jar.stderr", p.stderr),
                               ("oracle.stdout", out), ("oracle.stderr", err)):
                with open(os.path.join(d, name), "wb") as f:
                    f.write(data)
    with open(path + ".tmp", "w") as f:
        json.dump(rec, f)
    os.replace(path + ".tmp", path)
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=300)
    ap.add_argument("--seed", type=int, default=20261006)
    ap.add_argument("--isolate", default="none")
    ap.add_argument("--cli-par", type=int, default=3)
    ap.add_argument("--max-bytes", type=int, default=300_000)
    ap.add_argument("--skip-cli", action="store_true")
    ap.add_argument("--out", default="build/oracle/identity",
                    help="run directory (repo-relative); a cli/ cache from another environment "
                         "must not be reused")
    a = ap.parse_args()
    global OUT
    OUT = os.path.join(ROOT, a.out)
    os.makedirs(os.path.join(OUT, "cli"), exist_ok=True)
    items = select(a.n, a.seed, a.max_bytes)

    # Phase B first (one server, shuffled order); runs while the CLI pool works.
    order = list(items)
    random.Random(a.seed + 1).shuffle(order)
    server_path = os.path.join(OUT, f"server-{a.isolate}.jsonl")
    with ThreadPoolExecutor(max_workers=a.cli_par) as pool:
        futs = None if a.skip_cli else [pool.submit(cli_one, it) for it in items]
        t0 = time.time()
        server = {}
        with Oracle(isolate=a.isolate) as o, open(server_path, "w") as sf:
            server_jvm_env = o.env
            for k, (idx, case, prof) in enumerate(order):
                ts = time.time()
                r = o.request({"op": "compile", "args": case_args(case, prof)})
                rec = {"idx": idx, "pos": k, "ok": r.get("ok"), "rc": r.get("exit_code"),
                       "stdout_sha": sha(b64(r["stdout_b64"])) if r.get("ok") else None,
                       "stderr_sha": sha(b64(r["stderr_b64"])) if r.get("ok") else None,
                       "seconds": round(time.time() - ts, 3), "error": r.get("error")}
                server[idx] = rec
                if r.get("ok"):
                    with open(os.path.join(OUT, "cli", f"server-{a.isolate}-{idx}.stderr"), "wb") as f:
                        f.write(b64(r["stderr_b64"]))
                sf.write(json.dumps(rec) + "\n")
                sf.flush()
        server_secs = time.time() - t0
        cli = {it[0]: cli_one(it) for it in items} if not futs else {f.result()["idx"]: f.result() for f in futs}

    same = 0
    diffs = []
    jar_checked = jar_same = 0
    for idx, c in cli.items():
        s = server[idx]
        if (s["rc"], s["stdout_sha"], s["stderr_sha"]) == (c["rc"], c["stdout_sha"], c["stderr_sha"]):
            same += 1
        else:
            diffs.append({"idx": idx, "case": c["case"], "profile": c["profile"], "pos": s["pos"],
                          "cli_rc": c["rc"], "server_rc": s["rc"],
                          "stdout_same": s["stdout_sha"] == c["stdout_sha"],
                          "stderr_same": s["stderr_sha"] == c["stderr_sha"], "error": s["error"]})
        if "jar" in c:
            jar_checked += 1
            if c["jar"] == {"rc": c["rc"], "stdout_sha": c["stdout_sha"], "stderr_sha": c["stderr_sha"]}:
                jar_same += 1
    summary = {"isolate": a.isolate, "n": len(items), "identical": same, "differing": len(diffs),
               "server_seconds": round(server_secs, 1), "jar_checked": jar_checked,
               "jar_identical": jar_same, "diffs": diffs,
               # PROTOCOL.md "Environment": every JVM here ran in golden_env().
               "process_env": {k: v for k, v in golden_env().items() if k != "HOME"},
               "server_jvm_env": locals().get("server_jvm_env")}
    with open(os.path.join(OUT, f"summary-{a.isolate}.json"), "w") as f:
        json.dump(summary, f, indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != "diffs"}))
    for d in diffs[:20]:
        print(d)


if __name__ == "__main__":
    main()
