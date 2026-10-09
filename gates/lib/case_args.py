"""Shared argv construction for D2 cases.

This module is the single source of truth for "which compiler flags does case C run
with under profile P".  The golden reference runner (run_reference.py), the full golden
driver (golden_all.py), the oracle comparison and every later D2 gate must call
`compiler_args()` / `case_profiles()` from here instead of rebuilding argv themselves.

All paths are repo-relative; every command is meant to run with cwd = repository root.
See corpus/d2/PROFILES.md for the precedence rule and the chunk split rule.
"""

from __future__ import annotations

import glob
import json
import math
import os
import re
import sys
from typing import Iterable

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(REPO, "scripts"))
import paths  # noqa: E402  the reference (scripts/paths.py)
PROFILES_JSON = "corpus/d2/profiles.json"
CANDIDATES_GLOB = "corpus/d2/candidates/*.jsonl"
# Absolute: the selected reference checkout (docs/PORTING.md §9).
CLR_SOURCE = os.path.join(paths.REF_SRC, "src/com/google/javascript/jscomp/CommandLineRunner.java")

# Flags that only the runner may set (they decide where output goes / which files are
# compiled).  A case's extra_flags must never contain them.
RUNNER_OWNED = frozenset(
    {"--js", "--js_output_file", "--chunk", "--chunk_output_path_prefix", "--externs",
     "--create_source_map", "--flagfile", "--json_streams"}
)


class CaseProfileError(ValueError):
    """The (case, profile) pair cannot be turned into a valid argv."""


def repo_path(rel: str) -> str:
    return os.path.join(REPO, rel)


def load_profiles(path: str = PROFILES_JSON) -> dict:
    with open(repo_path(path), encoding="utf-8") as f:
        return json.load(f)


def load_cases(pattern: str = CANDIDATES_GLOB) -> list[dict]:
    """All cases, in file order (files sorted by name)."""
    cases = []
    for fn in sorted(glob.glob(repo_path(pattern))):
        with open(fn, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    cases.append(json.loads(line))
    return cases


def flag_key(flag: str) -> str:
    """'--language_in=UNSTABLE' -> '--language_in'; '--process_common_js_modules' -> itself."""
    return flag.split("=", 1)[0]


def js_sources(case: dict) -> list[str]:
    """The --js files of a case, in command-line order: inputs first, then shims."""
    return list(case.get("inputs", [])) + list(case.get("shims", []))


def profile_applies(profiles: dict, case: dict, name: str) -> bool:
    spec = profiles["profiles"][name]
    rule = spec.get("applies_to", "all")
    if rule == "all":
        return True
    if rule == "multi_input":
        return len(js_sources(case)) >= 2
    raise CaseProfileError(f"unknown applies_to rule {rule!r} for profile {name}")


def is_final_case(case: dict) -> bool:
    """True for a line of the final corpus/d2/cases.jsonl (see corpus/d2/FORMAT.md)."""
    return "tags" in case and "test_profiles" in case


def case_profiles(case: dict, profiles: dict | None = None) -> list[str]:
    """The profiles a case runs under.

    A case from the final corpus (corpus/d2/cases.jsonl, written by corpus/d2/validate/finalize.py;
    it carries the "tags" and "test_profiles" keys) already has its explicit, authoritative
    list: auto-applied profiles that were kept are in it, and pairs dropped because Java crashed
    are not. That list is returned unchanged.

    A candidate case (corpus/d2/candidates/*.jsonl) gets its own list, then auto-applied
    profiles (profiles.json "auto_apply": true), in profiles.json order, for every one that
    applies to it and that it does not already list.
    """
    if is_final_case(case):
        return list(case["profiles"])
    profiles = profiles or load_profiles()
    out = list(case["profiles"])
    for name, spec in profiles["profiles"].items():
        if spec.get("auto_apply") and name not in out and profile_applies(profiles, case, name):
            out.append(name)
    return out


def default_out_dir(case_id: str, profile: str, profiles: dict | None = None) -> str:
    """Repo-relative output directory for one (case, profile) run.

    It is a pure function of (case, profile) so the argv - and the output path that
    leaks into e.g. the source map's "file" field - is identical on every run.
    """
    profiles = profiles or load_profiles()
    return profiles["out_dir_template"].format(case_id=case_id, profile=profile)


def _expand(flags: Iterable[str], out_dir: str) -> list[str]:
    return [f.replace("{out_dir}", out_dir) for f in flags]


def _chunk_flags(spec: dict, sources: list[str]) -> list[str]:
    """--chunk flags for a chunk profile (corpus/d2/PROFILES.md, "Chunk profiles").

    ceil_half (chunks2): c0 gets the first ceil(n/2) --js sources, c1 (deps c0) the rest.
    fanout_thirds (chunks3): c0 gets the first k0 = ceil(n/3), c1 (deps c0) the next
      k1 = ceil((n-k0)/2), c2 (deps c0 only) the remaining n-k0-k1, which is 0 for n = 2
      (AbstractCommandLineRunner.JsChunkSpec.create: "We will allow chunks of zero input").
    """
    ch = spec["chunks"]
    names = ch["names"]
    split = ch["split"]
    n = len(sources)
    if n < 2:
        raise CaseProfileError("chunk profile needs at least 2 --js sources")
    if split == "ceil_half" and len(names) == 2:
        first = math.ceil(n / 2)
        return [f"--chunk={names[0]}:{first}", f"--chunk={names[1]}:{n - first}:{names[0]}"]
    if split == "fanout_thirds" and len(names) == 3:
        k0 = math.ceil(n / 3)
        k1 = math.ceil((n - k0) / 2)
        k2 = n - k0 - k1
        return [f"--chunk={names[0]}:{k0}", f"--chunk={names[1]}:{k1}:{names[0]}",
                f"--chunk={names[2]}:{k2}:{names[0]}"]
    raise CaseProfileError(f"unsupported chunk split {split!r} with {len(names)} chunks")


def compiler_args(case: dict, profile: str, out_dir: str | None = None,
                  profiles: dict | None = None) -> list[str]:
    """The exact compiler argv (everything after `java ... -jar closure-compiler.jar`).

    Order: profile flags, case extra_flags (precedence rule applied), --externs, --js,
    --chunk (chunk profiles), then the output flag.
    """
    profiles = profiles or load_profiles()
    if profile not in profiles["profiles"]:
        raise CaseProfileError(f"unknown profile {profile!r}")
    spec = profiles["profiles"][profile]
    if not profile_applies(profiles, case, profile):
        raise CaseProfileError(f"profile {profile} does not apply to case {case['id']}")
    if out_dir is None:
        out_dir = default_out_dir(case["id"], profile, profiles)
    out_dir = out_dir.rstrip("/")
    if os.path.isabs(out_dir):
        raise CaseProfileError("out_dir must be repo-relative")

    multi = set(profiles["multi_valued_flags"])
    locked = set(spec.get("locked", []))
    prof_flags = _expand(spec["flags"], out_dir)
    case_flags = list(case.get("extra_flags", []))

    case_single_keys = set()
    for f in case_flags:
        k = flag_key(f)
        if k in RUNNER_OWNED:
            raise CaseProfileError(f"case {case['id']} sets runner-owned flag {k}")
        if k not in multi:
            case_single_keys.add(k)
    for k in sorted(case_single_keys & locked):
        raise CaseProfileError(f"case {case['id']} overrides {k}, which profile {profile} locks")
    # Precedence: for single-valued flags the case wins; the profile's occurrence is dropped
    # so every single-valued key appears exactly once in argv.
    prof_flags = [f for f in prof_flags if flag_key(f) not in case_single_keys]

    args = prof_flags + case_flags
    args += [f"--externs={e}" for e in case.get("externs", [])]
    sources = js_sources(case)
    args += [f"--js={s}" for s in sources]
    out = spec["output"]
    if out == "single":
        args.append(f"--js_output_file={out_dir}/out.js")
    elif out == "chunks":
        args += _chunk_flags(spec, sources)
        args.append(f"--chunk_output_path_prefix={out_dir}/")
    else:
        raise CaseProfileError(f"unknown output kind {out!r}")
    return args


# ---------------------------------------------------------------------------------------
# Verification against the pinned CommandLineRunner source.

_OPT_RE = re.compile(r"@Option\s*\((?P<body>.*?)\)\s*(?:private|public|protected)[^;=]*?"
                     r"(?P<type>[A-Za-z_<>.,?@ \[\]]+?)\s+(?P<field>\w+)\s*[;=]", re.S)


def parse_runner_options(path: str = CLR_SOURCE) -> dict[str, dict]:
    """{flag name or alias: {"name", "list": bool, "field"}} from CommandLineRunner.java."""
    with open(repo_path(path), encoding="utf-8") as f:
        src = f.read()
    opts: dict[str, dict] = {}
    for m in _OPT_RE.finditer(src):
        body = m.group("body")
        nm = re.search(r'name\s*=\s*"([^"]+)"', body)
        if not nm:
            continue
        aliases = re.findall(r'"(-[^"]+)"', (re.search(r"aliases\s*=\s*\{([^}]*)\}", body) or
                                            re.match("()", "")).group(1))
        is_list = "List<" in m.group("type")
        info = {"name": nm.group(1), "list": is_list, "field": m.group("field")}
        for n in [nm.group(1)] + aliases:
            opts[n] = info
    return opts


def verify(profiles: dict | None = None, cases: list[dict] | None = None) -> list[str]:
    """Problems found (empty list = OK): unknown flags, wrong multi-valued list,
    precedence conflicts, profiles that do not exist."""
    profiles = profiles or load_profiles()
    cases = cases if cases is not None else load_cases()
    opts = parse_runner_options()
    problems = []
    declared_multi = set(profiles["multi_valued_flags"])
    actual_multi = {k for k, v in opts.items() if v["list"]}
    for k in sorted(declared_multi - actual_multi):
        problems.append(f"multi_valued_flags lists {k}, which is not a List option")
    seen = set()
    for name, spec in profiles["profiles"].items():
        for f in spec["flags"] + ["--js", "--externs", "--js_output_file", "--chunk",
                                  "--chunk_output_path_prefix"]:
            seen.add(flag_key(f))
    for c in cases:
        for f in c.get("extra_flags", []):
            seen.add(flag_key(f))
        for p in c["profiles"]:
            if p not in profiles["profiles"]:
                problems.append(f"case {c['id']} uses unknown profile {p}")
        for p in case_profiles(c, profiles):
            try:
                compiler_args(c, p, profiles=profiles)
            except CaseProfileError as e:
                problems.append(f"{c['id']} x {p}: {e}")
    for k in sorted(seen):
        if k not in opts:
            problems.append(f"flag {k} is not defined in CommandLineRunner")
        elif opts[k]["list"] and k not in declared_multi:
            problems.append(f"flag {k} is a List option but missing from multi_valued_flags")
    return problems


if __name__ == "__main__":
    import sys
    probs = verify()
    for p in probs:
        print("PROBLEM:", p)
    print("verify:", "OK" if not probs else f"{len(probs)} problem(s)")
    sys.exit(1 if probs else 0)
