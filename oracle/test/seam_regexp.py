"""SEAM.md D7 experiment: does Compiler.hasRegExpGlobalReferences explain the
simple-10 (whole-program-minimist-1.2.8, simple) seam divergence?
Usage: python3 oracle/test/seam_regexp.py  (1 oracle server in golden_env(), about 1 minute).
Variants differ only in one appended line of a copy of the case's shim."""
import sys, os, json, copy, hashlib, base64, re
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "scripts"))
from paths import ROOT  # noqa: E402  the main checkout (scripts/paths.py)
sys.path.insert(0, ROOT + "/oracle"); sys.path.insert(0, ROOT + "/oracle/test"); sys.path.insert(0, ROOT + "/gates/lib")
os.chdir(ROOT)
import seam3, oracle_client as oc
seam3.OUT = ROOT + "/build/oracle/seam/rx"
base = next(json.loads(l) for l in open("corpus/d2/cases.jsonl") if json.loads(l)["id"] == "whole-program-minimist-1.2.8")
shim = open(base["shims"][0]).read()
variants = {"control-copy": "", "neutral-line": "globalThis['rxprobe'] = 1;\n",
            "regexp-global-ref": "globalThis['rxprobe'] = RegExp.$1;\n"}
o = oc.Oracle(xmx="2g")
for prof in ("simple", "advanced"):
    for name, extra in variants.items():
        # Same directory depth as corpus/d2/shims/<id>/: the shim requires "../../../../corpus-cache/...".
        d = f"build/oracle/rx-{name}/x"; os.makedirs(d, exist_ok=True)
        open(f"{d}/entry.js", "w").write(shim + extra)
        c = copy.deepcopy(base); c["id"] = f"rx-{name}"; c["shims"] = [f"{d}/entry.js"]
        c["extra_flags"] = [f for f in c["extra_flags"] if not f.startswith("--entry_point")] + [f"--entry_point={d}/entry"]
        rec = seam3.run_pair(o, f"rx-{prof}-{name}", c, prof)
        cf, pf = rec["compile"]["files"], rec.get("optimize", {}).get("files", {})
        print(f"{prof:8s} {name:18s} class={rec['class']:28s} compile out.js={str(cf.get('out.js'))[:12]} seam out.js={str(pf.get('out.js'))[:12]} exit {rec['compile']['exit_code']}/{rec.get('optimize',{}).get('exit_code')}")
o.close()
