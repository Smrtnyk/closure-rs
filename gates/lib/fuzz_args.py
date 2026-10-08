#!/usr/bin/env python3
"""Line-oriented argv service for the Rust fuzz driver (fuzz/driver).

The fuzzer must build compiler argv exactly as every D2 gate does, so it asks
gates/lib/case_args.py instead of re-implementing profiles and precedence in Rust.

stdin, one JSON object per line:
  {"op": "profiles"}                                  -> {"profiles": [names...]}
  {"id": "...", "inputs": [repo-rel .js, ...], "profile": "advanced", "out_dir": "build/..."}
                                                      -> {"args": [...], "profile": "..."}
Each answer is one JSON line on stdout; errors are {"error": "..."}.  A chunk profile on a
single-input case is answered with {"error": ...}; the driver only picks applicable profiles.
"""
from __future__ import annotations

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import case_args  # noqa: E402


def main():
    os.chdir(case_args.REPO)
    profiles = case_args.load_profiles()
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            q = json.loads(line)
            if q.get("op") == "profiles":
                ans = {"profiles": list(profiles["profiles"].keys()),
                       "multi_input_only": [k for k, v in profiles["profiles"].items()
                                            if v.get("applies_to") == "multi_input"]}
            else:
                case = {"id": q["id"], "inputs": q["inputs"], "externs": q.get("externs", []),
                        "shims": [], "extra_flags": q.get("extra_flags", []),
                        "profiles": [q["profile"]]}
                args = case_args.compiler_args(case, q["profile"], out_dir=q["out_dir"],
                                               profiles=profiles)
                ans = {"args": args, "profile": q["profile"]}
        except Exception as e:  # report, keep serving
            ans = {"error": f"{type(e).__name__}: {e}"}
        sys.stdout.write(json.dumps(ans) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
