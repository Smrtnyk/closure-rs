"""Interim D2 case -> CommandLineRunner argv builder used by the oracle's own measurements.

gates/lib/case_args.py did not exist when this was written (no corpus/d2/profiles.json yet
either), so this module constructs argv directly from corpus/d2/FORMAT.md. When the gates
library lands, the oracle measurements should switch to it.

Order: --externs for each extern, then --js for each input, then --js for each shim, then the
case's extra_flags, then the profile flags. Output goes to stdout (no --js_output_file) except
for the sourcemap profile, which writes under build/oracle/tmp.
"""
import json
import glob
import os

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

PROFILES = {
    "ws": ["--compilation_level=WHITESPACE_ONLY"],
    "simple": ["--compilation_level=SIMPLE"],
    "advanced": ["--compilation_level=ADVANCED"],
    "advanced_strict": ["--compilation_level=ADVANCED", "--jscomp_error=*",
                        "--use_types_for_optimization"],
    "lang_es5": ["--compilation_level=SIMPLE", "--language_out=ECMASCRIPT5"],
    "lang_es2015": ["--compilation_level=SIMPLE", "--language_out=ECMASCRIPT_2015"],
    "lang_next": ["--compilation_level=SIMPLE", "--language_out=ECMASCRIPT_NEXT"],
    "pretty": ["--compilation_level=SIMPLE", "--formatting=PRETTY_PRINT"],
}


def load_cases():
    cases = []
    for f in sorted(glob.glob(os.path.join(ROOT, "corpus/d2/candidates/*.jsonl"))):
        with open(f) as fh:
            for line in fh:
                line = line.strip()
                if line:
                    cases.append(json.loads(line))
    return cases


def input_bytes(case):
    n = 0
    for p in case["inputs"] + case["externs"] + case["shims"]:
        try:
            n += os.path.getsize(os.path.join(ROOT, p))
        except OSError:
            return None
    return n


def case_args(case, profile):
    a = []
    for e in case["externs"]:
        a.append("--externs=" + e)
    for i in case["inputs"]:
        a.append("--js=" + i)
    for s in case["shims"]:
        a.append("--js=" + s)
    a += list(case["extra_flags"])
    a += PROFILES[profile]
    return a
