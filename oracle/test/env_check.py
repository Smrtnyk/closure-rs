"""PROTOCOL.md "Environment": the oracle matches golden only in golden_env().
Usage: python3 oracle/test/env_check.py  (run from the repository root; 2 oracle servers in turn)."""
import sys, json, base64, os, shutil
sys.path.insert(0, "oracle"); sys.path.insert(0, "gates/lib")
import oracle_client as oc, case_args as ca, run_reference as rr
cid, prof = "closure-self-ext-es3", "advanced_strict"
case = next(json.loads(l) for l in open("corpus/d2/cases.jsonl") if json.loads(l)["id"] == cid)
args = ca.compiler_args(case, prof)
g = json.load(open(rr.result_path(cid, prof)))
assert g["compiler_args"] == args
gerr = g["stderr"] if isinstance(g["stderr"], str) else base64.b64decode(g["stderr"]["base64"]).decode()
for name, env, chk in [("golden_env (default)", None, True),
                       ("de_DE.UTF-8", oc.golden_env() | {"LANG": "de_DE.UTF-8", "LC_ALL": "de_DE.UTF-8"}, False)]:
    out = os.path.join(ca.REPO, ca.default_out_dir(cid, prof)); shutil.rmtree(out, ignore_errors=True); os.makedirs(out)
    o = oc.Oracle(env=env, check_env=chk)
    try:
        oc.check_jvm_env(o.env); verdict = "env check passes"
    except oc.OracleEnvMismatch as e:
        verdict = "env check FAILS (Oracle() would refuse)"
    r = o.request({"op": "compile", "args": args}); o.close()
    err = oc.b64(r["stderr_b64"]).decode()
    print(f"{name}: exit {r['exit_code']} golden {g['exit_code']} stderr==golden {err == gerr} | {verdict} | tail: {err.strip().splitlines()[-1]}")
