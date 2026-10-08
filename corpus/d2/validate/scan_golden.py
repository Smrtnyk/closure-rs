#!/usr/bin/env python3
"""Scan every golden result of the D2 candidates into a compact per-pair table.

Writes build/validate/scan.jsonl: one record per (case, profile) with exit code, crash
flags, output emptiness, diagnostic counts and keys.  Read-only on the golden store.
"""
import json, os, re, sys
from multiprocessing import Pool

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))  # corpus/d2/validate/<file>
sys.path.insert(0, os.path.join(REPO, "gates/lib"))
import case_args  # noqa: E402
import run_reference as rr  # noqa: E402

DIAG_RE = re.compile(r"(?m)^.*?: (ERROR|WARNING) - \[(\w+)\] ")
SUMMARY_RE = re.compile(r"(?m)^(\d+) error\(s\), (\d+) warning\(s\)(?:, [\d.]+% typed)?$")
ICE_RE = re.compile(r"INTERNAL COMPILER ERROR")
EXC_RE = re.compile(r"(?m)^(?:Exception in thread|Caused by: |\s+at (?:com|java|jdk)\.)"
                    r"|java\.lang\.\w+(?:Error|Exception)")
EXC_NAME_RE = re.compile(r"(?m)^(?:Exception in thread \"[^\"]*\" )?((?:java|com)\.[\w.$]+(?:Exception|Error))")
FRAME_RE = re.compile(r"(?m)^\s+at (com\.google\.javascript\.[\w.$]+)\(")


def text(v):
    if isinstance(v, str):
        return v
    if isinstance(v, dict) and "base64" in v:
        import base64
        return base64.b64decode(v["base64"]).decode("utf-8", "replace")
    return ""


def scan(item):
    cid, src, prof = item
    p = os.path.join(REPO, rr.result_path(cid, prof))
    if not os.path.exists(p):
        return {"id": cid, "source": src, "profile": prof, "missing": True}
    with open(p, encoding="utf-8") as f:
        r = json.load(f)
    se = text(r.get("stderr", ""))
    so = text(r.get("stdout", ""))
    diags = DIAG_RE.findall(se)
    m = SUMMARY_RE.findall(se)
    outs = r.get("outputs", {})
    out_bytes = {k: len(text(v).encode("utf-8")) for k, v in outs.items()}
    out_empty = all(text(v).strip() == "" for v in outs.values())
    ice = bool(ICE_RE.search(se))
    exc = bool(EXC_RE.search(se))
    exc_name = None
    frame = None
    if ice or exc or r["exit_code"] == 254:
        mm = EXC_NAME_RE.search(se)
        exc_name = mm.group(1) if mm else None
        fm = FRAME_RE.search(se)
        frame = fm.group(1) if fm else None
    keys = {}
    for lvl, k in diags:
        keys[f"{lvl}:{k}"] = keys.get(f"{lvl}:{k}", 0) + 1
    return {
        "id": cid, "source": src, "profile": prof,
        "runner_version": r.get("runner_version"),
        "exit_code": r["exit_code"], "timed_out": r["timed_out"],
        "ice": ice, "java_exception": exc, "exc_name": exc_name, "exc_frame": frame,
        "stdout_len": len(so), "stderr_len": len(se),
        "summary": [int(m[-1][0]), int(m[-1][1])] if m else None,
        "n_error_lines": sum(1 for l, _ in diags if l == "ERROR"),
        "n_warning_lines": sum(1 for l, _ in diags if l == "WARNING"),
        "diag_keys": keys,
        "outputs": out_bytes, "output_empty": out_empty,
        "wall_ms": r.get("wall_ms"),
    }


def main():
    profiles = case_args.load_profiles()
    cases = case_args.load_cases()
    items = [(c["id"], c["source"], p) for c in cases for p in case_args.case_profiles(c, profiles)]
    out = os.path.join(REPO, "build/validate/scan.jsonl")
    with Pool(int(sys.argv[1]) if len(sys.argv) > 1 else 12) as pool, open(out + ".tmp", "w") as f:
        for rec in pool.imap(scan, items, chunksize=32):
            f.write(json.dumps(rec, sort_keys=True) + "\n")
    os.replace(out + ".tmp", out)
    print(f"scanned {len(items)} pairs -> {out}")


if __name__ == "__main__":
    main()
