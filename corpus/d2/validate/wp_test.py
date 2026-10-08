#!/usr/bin/env python3
"""Run every whole-program case's test_command against the golden (Java) output of each of
its profiles.  Resumable: a pair whose result file exists in build/validate/wp/results/ is
skipped.  chunk-profile outputs (chunks2, chunks3) are concatenated in chunk order (c0.js, "\n", c1.js[, "\n", c2.js]) into one file,
which is how a page would load the two chunks as consecutive classic scripts.
"""
import concurrent.futures as cf
import json, os, re, subprocess, sys, time, base64

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))  # corpus/d2/validate/<file>
sys.path.insert(0, os.path.join(REPO, "gates/lib"))
import case_args  # noqa: E402
import run_reference as rr  # noqa: E402

WP = os.path.join(REPO, "build/validate/wp")
NODE = os.environ.get("NODE", "node")


def text(v):
    if isinstance(v, str):
        return v
    return base64.b64decode(v["base64"]).decode("utf-8")


def one(case, prof):
    res_path = os.path.join(WP, "results", f"{case['id']}__{prof}.json")
    if os.path.exists(res_path):
        with open(res_path) as f:
            return json.load(f)
    with open(os.path.join(REPO, rr.result_path(case["id"], prof)), encoding="utf-8") as f:
        g = json.load(f)
    outs = g["outputs"]
    res = {"id": case["id"], "profile": prof, "java_exit_code": g["exit_code"]}
    if g["exit_code"] != 0 or not outs:
        res.update(status="no_output", passed=False,
                   detail=f"Java exit {g['exit_code']}, outputs={sorted(outs)}")
    else:
        if prof == "chunks2" or "out.js" not in outs:
            names = sorted(outs)  # c0.js, c1.js
            code = "\n".join(text(outs[n]) for n in names)
            res["concatenated"] = names
        else:
            code = text(outs["out.js"])
        out_file = os.path.join(WP, "out", prof, f"{case['id']}.js")
        os.makedirs(os.path.dirname(out_file), exist_ok=True)
        with open(out_file, "w", encoding="utf-8") as f:
            f.write(code)
        rel = os.path.relpath(out_file, REPO)
        argv = [a if a != "{output}" else rel for a in case["test_command"].split(" ")]
        assert argv[0] == "node"
        argv[0] = NODE
        t0 = time.monotonic()
        try:
            p = subprocess.run(argv, cwd=REPO, capture_output=True, text=True, timeout=600,
                               stdin=subprocess.DEVNULL)
            so, se, ec = p.stdout, p.stderr, p.returncode
        except subprocess.TimeoutExpired as e:
            so, se, ec = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or ""), "", "timeout"
        m = re.search(r"(?m)^# tests (\d+), pass (\d+), fail (\d+), skip (\d+)$", so or "")
        passed = ec == 0 and bool(re.search(r"(?m)^# PASS$", so or ""))
        fails = [l for l in (so or "").split("\n") if l.startswith("not ok")][:15]
        res.update(status="pass" if passed else "fail", passed=passed, exit=ec,
                   tests=[int(x) for x in m.groups()] if m else None,
                   first_failures=fails,
                   tail=[l for l in ((so or "") + (se or "")).split("\n") if l.strip()][-12:],
                   wall_ms=int((time.monotonic() - t0) * 1000), test_argv=argv[1:])
    os.makedirs(os.path.dirname(res_path), exist_ok=True)
    with open(res_path + ".tmp", "w") as f:
        json.dump(res, f, indent=1)
    os.replace(res_path + ".tmp", res_path)
    return res


def main():
    profiles = case_args.load_profiles()
    cases = [c for c in case_args.load_cases() if c["source"] == "whole-program"]
    pairs = [(c, p) for c in cases for p in case_args.case_profiles(c, profiles)]
    with cf.ThreadPoolExecutor(int(sys.argv[1]) if len(sys.argv) > 1 else 8) as ex:
        results = list(ex.map(lambda cp: one(*cp), pairs))
    summ = {f"{r['id']}/{r['profile']}": r["status"] for r in results}
    with open(os.path.join(WP, "summary.json"), "w") as f:
        json.dump(summ, f, indent=1, sort_keys=True)
    n = sum(r["passed"] for r in results)
    print(f"{n}/{len(results)} pairs pass")
    for r in results:
        if not r["passed"]:
            print("FAIL", r["id"], r["profile"], r.get("exit"), r.get("tests"), r.get("detail", ""))


if __name__ == "__main__":
    main()
