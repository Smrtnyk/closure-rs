"""D2 differential runner core: run a Rust CLI on every visible D2 (case, profile) pair and
compare stdout, stderr, exit code and every output file (JS chunks, source maps) byte for byte
with the golden Java results.  See gates/README_d2_rust.md.

Used by gates/d2_rust.py (CLI) and gates/lib/d2_rust_test.py (unit tests).
"""

from __future__ import annotations

import base64
import collections
import concurrent.futures
import datetime
import hashlib
import json
import os
import random
import re
import shutil
import signal
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import case_args  # noqa: E402
import run_reference  # noqa: E402

CHECKOUT = case_args.REPO  # the checkout these gates live in (main checkout or a worktree)
CASES_FILE = "corpus/d2/cases.jsonl"  # the final visible corpus (corpus/d2/FORMAT.md)
GOLDEN_TAG = run_reference.REF_TAG
GOLDEN_ROOT = run_reference.GOLDEN_ROOT  # repo-relative, under the data root
DEFAULT_TIMEOUT_S = run_reference.DEFAULT_TIMEOUT_S
RATCHET_SCHEMA = 1
CONTEXT_BYTES = 40
MAX_EXAMPLES = 5

# Primary cause of a failing pair: the first entry in this order that applies.
CAUSE_ORDER = ("harness_error", "spawn_error", "timeout", "crash", "no_output", "exit_code",
               "missing_file", "extra_file", "stderr", "stdout", "js_output", "sourcemap")

_JSC_KEY = re.compile(rb"\[(JSC_[A-Z0-9_]+)\]")


class HarnessError(RuntimeError):
    """The runner itself cannot do its job (not a compiler mismatch)."""


# ---------------------------------------------------------------------------------------
# Locations


def find_data_root(override: str | None = None, checkout: str = CHECKOUT) -> str:
    """The checkout that holds corpus-cache/ (golden store, inputs), reference/, tools/, build/.

    Order: --data-root, $CLOSURE_RS_DATA_ROOT, the gates checkout itself, the main checkout of
    the git repository (parent of the common git dir) when the gates checkout is a worktree.
    """
    cand = override or os.environ.get("CLOSURE_RS_DATA_ROOT")
    if cand:
        cand = os.path.abspath(cand)
        if not os.path.isdir(os.path.join(cand, GOLDEN_ROOT)):
            raise HarnessError(f"data root {cand} has no {GOLDEN_ROOT}")
        return cand
    if os.path.isdir(os.path.join(checkout, GOLDEN_ROOT)):
        return checkout
    try:
        common = subprocess.run(
            ["git", "-C", checkout, "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as e:
        raise HarnessError(f"cannot locate the data root from {checkout}: {e}") from e
    root = os.path.dirname(common)
    if os.path.isdir(os.path.join(root, GOLDEN_ROOT)):
        return root
    raise HarnessError(f"no {GOLDEN_ROOT} in {checkout} or {root}; pass --data-root")


def make_run_root(out: str, data_root: str, checkout: str = CHECKOUT) -> str:
    """<out>/root: cwd of every binary run.  The case argv is repo-relative; inputs resolve
    through symlinks into the data root (corpus-cache/, reference/) and the gates checkout
    (corpus/), while build/golden-tmp/ (the out_dirs named in argv) is private to this run."""
    root = os.path.join(out, "root")
    os.makedirs(os.path.join(root, "build", "golden-tmp"), exist_ok=True)
    links = (("corpus-cache", os.path.join(data_root, "corpus-cache")),
             ("reference", os.path.join(data_root, "reference")),
             ("corpus", os.path.join(checkout, "corpus")))
    for name, target in links:
        link = os.path.join(root, name)
        if os.path.islink(link):
            if os.readlink(link) == target:
                continue
            os.remove(link)
        os.symlink(target, link)
    return root


def default_out(prefix: str = "") -> str:
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return os.path.join(CHECKOUT, "build", "d2-rust", f"{prefix}{stamp}")


def sha256_file(path: str) -> str | None:
    try:
        h = hashlib.sha256()
        with open(path, "rb") as f:
            for blk in iter(lambda: f.read(1 << 20), b""):
                h.update(blk)
        return h.hexdigest()
    except OSError:
        return None


# ---------------------------------------------------------------------------------------
# Pairs, filters, sampling


def load_pairs(checkout: str = CHECKOUT) -> tuple[list[dict], dict]:
    """All visible (case, profile) pairs in corpus order: [{"case", "profile", "source"}]."""
    profiles = case_args.load_profiles()
    pairs = []
    for c in case_args.load_cases(CASES_FILE):
        for p in case_args.case_profiles(c, profiles):
            pairs.append({"case": c, "profile": p, "source": c.get("source")})
    return pairs, profiles


def profile_order(profiles: dict) -> list[str]:
    return list(profiles["profiles"].keys())


def has_module_flags(case: dict) -> bool:
    return any(case_args.flag_key(f) in ("--module_resolution", "--process_common_js_modules")
               for f in case.get("extra_flags", []))


def stratum(pair: dict) -> tuple[str, str, bool]:
    return (pair["profile"], pair["source"] or "", has_module_flags(pair["case"]))


def stratified_sample(pairs: list[dict], n: int, seed: int, min_per_group: int = 1) -> list[dict]:
    """Deterministic sample of about n pairs, stratified by (profile, source, module flags):
    every stratum present gets at least min(min_per_group, its size) pairs, the rest is
    proportional.  The result keeps corpus order."""
    groups: dict[tuple, list[int]] = collections.defaultdict(list)
    for i, p in enumerate(pairs):
        groups[stratum(p)].append(i)
    total = len(pairs)
    rng = random.Random(seed)
    chosen: set[int] = set()
    for key in sorted(groups, key=lambda k: (k[0], k[1], k[2])):
        idx = groups[key]
        quota = max(min_per_group, int(round(n * len(idx) / total)) if total else 0)
        quota = min(quota, len(idx))
        chosen.update(rng.sample(idx, quota))
    return [pairs[i] for i in sorted(chosen)]


def apply_filter(pairs: list[dict], flt: dict) -> list[dict]:
    out = pairs
    if flt.get("profiles"):
        keep = set(flt["profiles"])
        out = [p for p in out if p["profile"] in keep]
    if flt.get("sources"):
        keep = set(flt["sources"])
        out = [p for p in out if p["source"] in keep]
    if flt.get("only_passing"):
        # the gate's ratchet check: only the pairs a baseline lists as passing
        keep = {(cid, prof) for prof, ids in flt["only_passing"].items() for cid in ids}
        out = [p for p in out if (p["case"]["id"], p["profile"]) in keep]
    if flt.get("case_regex"):
        rx = re.compile(flt["case_regex"])
        out = [p for p in out if rx.search(p["case"]["id"])]
    if flt.get("sample"):
        out = stratified_sample(out, flt["sample"], flt.get("seed") or 0,
                                flt.get("min_per_group") or 1)
    if flt.get("limit"):
        out = out[: flt["limit"]]
    return out


def normalize_filter(flt: dict) -> dict:
    """Canonical form stored in ratchet.json ({} = complete run)."""
    out = {}
    for k in ("profiles", "sources"):
        if flt.get(k):
            out[k] = sorted(set(flt[k]))
    for k in ("case_regex", "sample", "limit"):
        if flt.get(k):
            out[k] = flt[k]
    if flt.get("only_passing"):
        out["only_passing"] = True
    if flt.get("sample"):
        out["seed"] = flt.get("seed") or 0
        out["min_per_group"] = flt.get("min_per_group") or 1
    return out


# ---------------------------------------------------------------------------------------
# Golden results


def decode_text(v) -> bytes:
    """Golden text: a str (UTF-8) or {"base64": ...} (run_reference.enc)."""
    if isinstance(v, dict):
        return base64.b64decode(v["base64"])
    return v.encode("utf-8")


def golden_path(data_root: str, case_id: str, profile: str) -> str:
    return os.path.join(data_root, GOLDEN_ROOT, case_id, f"{profile}.json")


def load_golden(data_root: str, case_id: str, profile: str) -> dict:
    with open(golden_path(data_root, case_id, profile), encoding="utf-8") as f:
        return json.load(f)


# ---------------------------------------------------------------------------------------
# Comparison


def _escape(b: bytes) -> str:
    return b.decode("utf-8", "backslashreplace").encode("unicode_escape").decode("ascii")


def line_col(data: bytes, offset: int) -> tuple[int, int]:
    """1-based line and column (in bytes) of `offset` in `data`."""
    line = data.count(b"\n", 0, offset) + 1
    col = offset - (data.rfind(b"\n", 0, offset) + 1) + 1
    return line, col


def first_diff(expected: bytes, actual: bytes) -> dict | None:
    """None if identical, else where and how the bytes first differ."""
    if expected == actual:
        return None
    n = min(len(expected), len(actual))
    off = n
    for i in range(n):
        if expected[i] != actual[i]:
            off = i
            break
    line, col = line_col(expected, off)
    lo = max(0, off - CONTEXT_BYTES)
    return {
        "offset": off,
        "line": line,
        "col": col,
        "expected_len": len(expected),
        "actual_len": len(actual),
        "expected_context": _escape(expected[lo: off + CONTEXT_BYTES]),
        "actual_context": _escape(actual[lo: off + CONTEXT_BYTES]),
    }


def first_diff_line(expected: bytes, actual: bytes) -> tuple[bytes, bytes]:
    el, al = expected.split(b"\n"), actual.split(b"\n")
    for i in range(max(len(el), len(al))):
        e = el[i] if i < len(el) else b""
        a = al[i] if i < len(al) else b""
        if e != a:
            return e, a
    return b"", b""


def jsc_key(line: bytes) -> str | None:
    m = _JSC_KEY.search(line)
    return m.group(1).decode("ascii") if m else None


def compare(golden: dict, exit_code: int | None, stdout: bytes, stderr: bytes,
            outputs: dict[str, bytes]) -> tuple[list[dict], dict]:
    """Byte-for-byte comparison.  Returns (diffs, facts); diffs == [] means identical."""
    exp_stdout = decode_text(golden["stdout"])
    exp_stderr = decode_text(golden["stderr"])
    exp_out = {k: decode_text(v) for k, v in golden["outputs"].items()}
    diffs: list[dict] = []
    if exit_code != golden["exit_code"]:
        diffs.append({"what": "exit_code", "expected": golden["exit_code"], "actual": exit_code})
    for name, exp, act in (("stdout", exp_stdout, stdout), ("stderr", exp_stderr, stderr)):
        d = first_diff(exp, act)
        if d:
            diffs.append({"what": name, **d})
    for fn in sorted(set(exp_out) | set(outputs)):
        if fn not in outputs:
            diffs.append({"what": "missing_file", "file": fn, "expected_len": len(exp_out[fn])})
        elif fn not in exp_out:
            diffs.append({"what": "extra_file", "file": fn, "actual_len": len(outputs[fn])})
        else:
            d = first_diff(exp_out[fn], outputs[fn])
            if d:
                diffs.append({"what": "file", "file": fn, **d})
    maps = [fn for fn in exp_out if fn.endswith(".map")]
    facts = {
        "maps_total": len(maps),
        "maps_match": bool(maps) and all(outputs.get(fn) == exp_out[fn] for fn in maps),
        "golden_has_output": bool(exp_stdout or exp_stderr or exp_out),
        "exp_stderr": exp_stderr,
    }
    return diffs, facts


def classify(diffs: list[dict], *, timed_out: bool = False, returncode: int | None = None,
             stdout: bytes = b"", stderr: bytes = b"", outputs: dict | None = None,
             facts: dict | None = None, harness_error: str | None = None,
             spawn_error: str | None = None) -> tuple[str | None, str | None]:
    """(cause, sub_key) of a failing pair, first match in CAUSE_ORDER; (None, None) = pass."""
    outputs = outputs or {}
    facts = facts or {}
    if harness_error:
        return "harness_error", harness_error[:120]
    if spawn_error:
        return "spawn_error", spawn_error[:120]
    if timed_out:
        return "timeout", None
    if not diffs:
        return None, None
    exp_stderr = facts.get("exp_stderr", b"")
    if returncode is not None and returncode < 0:
        return "crash", f"signal {-returncode}"
    if b"panicked at" in stderr and b"panicked at" not in exp_stderr:
        return "crash", "panic"
    if not stdout and not stderr and not outputs and facts.get("golden_has_output"):
        return "no_output", None
    by = collections.defaultdict(list)
    for d in diffs:
        by[d["what"]].append(d)
    if by["exit_code"]:
        d = by["exit_code"][0]
        return "exit_code", f"{d['expected']} -> {d['actual']}"
    if by["missing_file"]:
        return "missing_file", by["missing_file"][0]["file"]
    if by["extra_file"]:
        return "extra_file", by["extra_file"][0]["file"]
    if by["stderr"]:
        e, a = first_diff_line(exp_stderr, stderr)
        return "stderr", f"{jsc_key(e) or '-'} | {jsc_key(a) or '-'}"
    if by["stdout"]:
        return "stdout", None
    js = [d for d in by["file"] if not d["file"].endswith(".map")]
    if js:
        return "js_output", js[0]["file"]
    if by["file"]:
        return "sourcemap", by["file"][0]["file"]
    return "harness_error", "unclassified difference"


# ---------------------------------------------------------------------------------------
# Running one pair


def _collect_outputs(out_abs: str) -> dict[str, bytes]:
    outputs = {}
    for root, _dirs, files in os.walk(out_abs):
        for fn in files:
            p = os.path.join(root, fn)
            rel = os.path.relpath(p, out_abs).replace(os.sep, "/")
            with open(p, "rb") as f:
                outputs[rel] = f.read()
    return dict(sorted(outputs.items()))


def _remove_out_dir(out_abs: str) -> None:
    shutil.rmtree(out_abs, ignore_errors=True)
    try:
        os.rmdir(os.path.dirname(out_abs))  # the <case> dir, once its last profile is done
    except OSError:
        pass


def run_pair(pair: dict, *, binary: str, run_root: str, data_root: str, profiles: dict,
             timeout_s: float, keep_dir: str | None = None) -> dict:
    case = pair["case"]
    cid, profile = case["id"], pair["profile"]
    res = {"case": cid, "profile": profile, "source": pair["source"], "pass": False,
           "cause": None, "sub": None, "timed_out": False, "exit_code": None,
           "expected_exit_code": None, "wall_ms": None, "maps_total": 0, "maps_match": False,
           "diffs": []}
    try:
        out_dir = case_args.default_out_dir(cid, profile, profiles)
        cargs = case_args.compiler_args(case, profile, out_dir, profiles)
        golden = load_golden(data_root, cid, profile)
        if golden.get("compiler_args") != cargs:
            raise HarnessError("argv differs from the golden compiler_args")
        if golden.get("reference_jar_sha256") != run_reference.JAR_SHA256:
            raise HarnessError("golden result is not from the pinned reference jar")
    except (OSError, ValueError, KeyError, case_args.CaseProfileError, HarnessError) as e:
        res["cause"], res["sub"] = classify([], harness_error=f"{type(e).__name__}: {e}")
        return res
    res["expected_exit_code"] = golden["exit_code"]

    out_abs = os.path.join(run_root, out_dir)
    shutil.rmtree(out_abs, ignore_errors=True)
    os.makedirs(out_abs, exist_ok=True)
    t0 = time.monotonic()
    try:
        proc = subprocess.Popen([binary, *cargs], cwd=run_root, env=run_reference.child_env(),
                                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, start_new_session=True)
    except OSError as e:
        _remove_out_dir(out_abs)
        res["cause"], res["sub"] = classify([], spawn_error=f"{type(e).__name__}: {e.strerror}")
        return res
    timed_out = False
    try:
        stdout, stderr = proc.communicate(timeout=timeout_s)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        stdout, stderr = proc.communicate()
    res["wall_ms"] = int(round((time.monotonic() - t0) * 1000))
    res["exit_code"] = proc.returncode
    res["timed_out"] = timed_out
    outputs = _collect_outputs(out_abs)

    diffs, facts = compare(golden, proc.returncode, stdout, stderr, outputs)
    cause, sub = classify(diffs, timed_out=timed_out, returncode=proc.returncode,
                          stdout=stdout, stderr=stderr, outputs=outputs, facts=facts)
    res["pass"] = cause is None
    res["cause"], res["sub"] = cause, sub
    res["diffs"] = diffs
    res["maps_total"], res["maps_match"] = facts["maps_total"], facts["maps_match"]

    if keep_dir and not res["pass"]:
        dest = os.path.join(keep_dir, cid, profile)
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        shutil.move(out_abs, dest)
        with open(os.path.join(dest, "_stdout"), "wb") as f:
            f.write(stdout)
        with open(os.path.join(dest, "_stderr"), "wb") as f:
            f.write(stderr)
        with open(os.path.join(dest, "_argv.json"), "w", encoding="utf-8") as f:
            json.dump({"argv": [binary, *cargs], "cwd": run_root, "exit_code": proc.returncode},
                      f, indent=1)
    _remove_out_dir(out_abs)
    return res


# ---------------------------------------------------------------------------------------
# Running many pairs


def run(binary: str, *, flt: dict | None = None, jobs: int | None = None,
        timeout_s: float = DEFAULT_TIMEOUT_S, out: str | None = None,
        data_root: str | None = None, keep_failing: bool = False,
        progress_every: int = 500, quiet: bool = False) -> dict:
    """Run the binary on the (filtered) visible corpus; write results.jsonl, report.json,
    report.md and ratchet.json into `out`.  Returns the report dict."""
    flt = flt or {}
    binary = os.path.abspath(binary)
    data_root = find_data_root(data_root)
    out = os.path.abspath(out or default_out())
    os.makedirs(out, exist_ok=True)
    run_root = make_run_root(out, data_root)
    keep_dir = os.path.join(out, "failing") if keep_failing else None
    jobs = jobs or int(os.environ.get("D2_JOBS") or 0) or min(8, os.cpu_count() or 1)

    all_pairs, profiles = load_pairs()
    pairs = apply_filter(all_pairs, flt)
    nflt = normalize_filter(flt)
    if not quiet:
        print(f"d2_rust: {len(pairs)} pairs ({'complete' if not nflt else nflt}), "
              f"jobs={jobs}, timeout={timeout_s}s, binary={binary}", file=sys.stderr)

    results: list[dict | None] = [None] * len(pairs)
    lock = threading.Lock()
    done = [0, 0]
    t0 = time.monotonic()

    def work(i: int) -> None:
        r = run_pair(pairs[i], binary=binary, run_root=run_root, data_root=data_root,
                     profiles=profiles, timeout_s=timeout_s, keep_dir=keep_dir)
        results[i] = r
        with lock:
            done[0] += 1
            done[1] += r["pass"]
            if not quiet and (done[0] % progress_every == 0 or done[0] == len(pairs)):
                print(f"d2_rust: {done[0]}/{len(pairs)} done, {done[1]} pass, "
                      f"{time.monotonic() - t0:.0f}s", file=sys.stderr)

    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as ex:
        for f in [ex.submit(work, i) for i in range(len(pairs))]:
            f.result()
    elapsed = time.monotonic() - t0

    with open(os.path.join(out, "results.jsonl"), "w", encoding="utf-8") as f:
        for r in results:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    meta = {
        "binary": binary,
        "binary_sha256": sha256_file(binary),
        "data_root": data_root,
        "checkout": CHECKOUT,
        "golden": GOLDEN_TAG,
        "corpus_sha256": sha256_file(os.path.join(CHECKOUT, CASES_FILE)),
        "filter": nflt,
        "complete": not nflt,
        "jobs": jobs,
        "timeout_s": timeout_s,
        "elapsed_s": round(elapsed, 1),
        "finished_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "out": out,
    }
    report = build_report(results, profile_order(profiles), meta)
    ratchet = build_ratchet(results, profile_order(profiles), meta)
    with open(os.path.join(out, "report.json"), "w", encoding="utf-8") as f:
        json.dump(report, f, ensure_ascii=False, indent=1)
        f.write("\n")
    with open(os.path.join(out, "report.md"), "w", encoding="utf-8") as f:
        f.write(render_markdown(report))
    with open(os.path.join(out, "ratchet.json"), "w", encoding="utf-8") as f:
        json.dump(ratchet, f, indent=1)
        f.write("\n")
    shutil.rmtree(os.path.join(run_root, "build"), ignore_errors=True)
    return report


# ---------------------------------------------------------------------------------------
# Report and ratchet


def _pct(a: int, b: int) -> str:
    return f"{100.0 * a / b:.2f}%" if b else "-"


def build_report(results: list[dict], order: list[str], meta: dict) -> dict:
    per_profile = {p: {"pairs": 0, "pass": 0} for p in order}
    per_source: dict[str, dict] = {}
    maps = {"pairs": 0, "match": 0}
    causes: dict[str, dict] = {}
    for r in results:
        pp = per_profile.setdefault(r["profile"], {"pairs": 0, "pass": 0})
        ps = per_source.setdefault(r["source"] or "", {"pairs": 0, "pass": 0})
        pp["pairs"] += 1
        ps["pairs"] += 1
        pp["pass"] += r["pass"]
        ps["pass"] += r["pass"]
        if r["maps_total"]:
            maps["pairs"] += 1
            maps["match"] += bool(r["maps_match"])
        if r["pass"]:
            continue
        c = causes.setdefault(r["cause"], {"count": 0, "subs": {}})
        c["count"] += 1
        s = c["subs"].setdefault(r["sub"] or "", {"count": 0, "examples": []})
        s["count"] += 1
        if len(s["examples"]) < MAX_EXAMPLES:
            s["examples"].append({"case": r["case"], "profile": r["profile"],
                                  "diffs": r["diffs"][:3]})
    for c in causes.values():
        c["subs"] = dict(sorted(c["subs"].items(), key=lambda kv: (-kv[1]["count"], kv[0])))
    causes = {k: causes[k] for k in CAUSE_ORDER if k in causes}
    total = {"pairs": len(results), "pass": sum(r["pass"] for r in results)}
    return {"meta": meta, "total": total, "per_profile": per_profile,
            "per_source": dict(sorted(per_source.items())), "sourcemaps": maps,
            "causes": causes}


def _diff_line(d: dict) -> str:
    what = d["what"] if d["what"] != "file" else f"file {d['file']}"
    if d["what"] == "exit_code":
        return f"exit code: expected {d['expected']}, actual {d['actual']}"
    if d["what"] in ("missing_file", "extra_file"):
        return f"{d['what']} {d['file']}"
    return (f"{what}: first diff at byte {d['offset']} (line {d['line']}, col {d['col']}); "
            f"len {d['expected_len']} vs {d['actual_len']}\n"
            f"      expected: `{d['expected_context']}`\n"
            f"      actual:   `{d['actual_context']}`")


def render_markdown(rep: dict) -> str:
    m = rep["meta"]
    lines = ["# D2 differential run (Rust CLI vs golden Java)", ""]
    lines.append(f"- binary: `{m['binary']}` (sha256 `{m['binary_sha256']}`)")
    lines.append(f"- golden: `{m['golden']}` in `{m['data_root']}`; corpus sha256 "
                 f"`{m['corpus_sha256']}`")
    lines.append(f"- pairs: {'complete corpus' if m['complete'] else 'filtered ' + json.dumps(m['filter'])}; "
                 f"jobs {m['jobs']}, timeout {m['timeout_s']} s, elapsed {m['elapsed_s']} s; "
                 f"finished {m['finished_utc']}")
    t = rep["total"]
    lines += ["", f"**Total: {t['pass']} / {t['pairs']} pass ({_pct(t['pass'], t['pairs'])})**", "",
              "## Per profile", "", "| Profile | Pairs | Pass | Fail | Pass % |", "|---|---|---|---|---|"]
    for p, v in rep["per_profile"].items():
        lines.append(f"| {p} | {v['pairs']} | {v['pass']} | {v['pairs'] - v['pass']} | "
                     f"{_pct(v['pass'], v['pairs'])} |")
    lines += ["", "## Per source", "", "| Source | Pairs | Pass | Pass % |", "|---|---|---|---|"]
    for s, v in rep["per_source"].items():
        lines.append(f"| {s} | {v['pairs']} | {v['pass']} | {_pct(v['pass'], v['pairs'])} |")
    sm = rep["sourcemaps"]
    lines += ["", f"**Source maps (D5):** {sm['match']} / {sm['pairs']} pairs with a golden `.map` "
              f"output have every `.map` file identical ({_pct(sm['match'], sm['pairs'])}).", "",
              "## Failures by cause", "",
              "Primary cause = first match in: " + ", ".join(f"`{c}`" for c in CAUSE_ORDER) + ".", ""]
    if not rep["causes"]:
        lines.append("None.")
    for cause, c in rep["causes"].items():
        lines += [f"### {cause}: {c['count']}", ""]
        for sub, s in list(c["subs"].items())[:15]:
            lines.append(f"- **{sub or '(all)'}**: {s['count']}")
            for ex in s["examples"]:
                lines.append(f"  - `{ex['case']}` x `{ex['profile']}`")
                for d in ex["diffs"]:
                    lines.append(f"    - {_diff_line(d)}")
        if len(c["subs"]) > 15:
            lines.append(f"- ... {len(c['subs']) - 15} more sub-groups (report.json)")
        lines.append("")
    return "\n".join(lines) + "\n"


def build_ratchet(results: list[dict], order: list[str], meta: dict) -> dict:
    counts = {p: {"pass": 0, "total": 0} for p in order}
    passing = {p: [] for p in order}
    failing = {p: [] for p in order}
    for r in results:
        counts.setdefault(r["profile"], {"pass": 0, "total": 0})
        counts[r["profile"]]["total"] += 1
        if r["pass"]:
            counts[r["profile"]]["pass"] += 1
            passing.setdefault(r["profile"], []).append(r["case"])
        else:
            failing.setdefault(r["profile"], []).append(r["case"])
    return {
        "schema": RATCHET_SCHEMA,
        "golden": meta["golden"],
        "corpus_sha256": meta["corpus_sha256"],
        "complete": meta["complete"],
        "filter": meta["filter"],
        "binary_sha256": meta["binary_sha256"],
        "counts": counts,
        "total": {"pass": sum(v["pass"] for v in counts.values()),
                  "total": sum(v["total"] for v in counts.values())},
        "passing": {p: sorted(v) for p, v in passing.items()},
        "failing": {p: sorted(v) for p, v in failing.items()},
    }


def check_ratchet(base: dict, cur: dict) -> tuple[int, list[str]]:
    """The ratchet: ratchet.json only goes up.  Returns (exit code, messages):
    0 = no regression, 1 = regression, 2 = not comparable."""
    msgs: list[str] = []
    if base.get("golden") != cur.get("golden"):
        return 2, [f"not comparable: golden {base.get('golden')} vs {cur.get('golden')}"]
    if base.get("filter", {}) != cur.get("filter", {}) or base.get("complete") != cur.get("complete"):
        return 2, [f"not comparable: filter {json.dumps(base.get('filter'))} vs "
                   f"{json.dumps(cur.get('filter'))}"]
    if base.get("corpus_sha256") != cur.get("corpus_sha256"):
        msgs.append("note: corpus/d2/cases.jsonl changed between the two runs")
    code = 0
    profiles = list(base.get("counts", {})) + [p for p in cur.get("counts", {})
                                               if p not in base.get("counts", {})]
    for p in profiles:
        b = base.get("counts", {}).get(p, {"pass": 0, "total": 0})
        c = cur.get("counts", {}).get(p, {"pass": 0, "total": 0})
        delta = c["pass"] - b["pass"]
        tag = "REGRESSION" if delta < 0 else ("up" if delta > 0 else "same")
        msgs.append(f"{p}: {b['pass']}/{b['total']} -> {c['pass']}/{c['total']} "
                    f"({delta:+d}, {tag})")
        if delta < 0:
            code = 1
    lost, gone = [], []
    for p, ids in base.get("passing", {}).items():
        now_pass = set(cur.get("passing", {}).get(p, []))
        now_fail = set(cur.get("failing", {}).get(p, []))
        for cid in ids:
            if cid in now_pass:
                continue
            (lost if cid in now_fail else gone).append(f"{cid} x {p}")
    if lost:
        code = 1
        msgs.append(f"REGRESSION: {len(lost)} previously passing pair(s) now fail:")
        msgs += [f"  {x}" for x in lost[:200]]
        if len(lost) > 200:
            msgs.append(f"  ... {len(lost) - 200} more")
    if gone:
        msgs.append(f"note: {len(gone)} previously passing pair(s) are no longer in the corpus:")
        msgs += [f"  {x}" for x in gone[:50]]
    bt, ct = base.get("total", {}), cur.get("total", {})
    msgs.append(f"total: {bt.get('pass')}/{bt.get('total')} -> {ct.get('pass')}/{ct.get('total')}")
    msgs.append("ratchet: REGRESSION" if code else "ratchet: OK (no regression)")
    return code, msgs


def check_passing_set(base: dict, cur: dict) -> tuple[int, list[str]]:
    """The gate's ratchet check on an --only-passing run: every pair the baseline lists as passing
    must still pass. 0 = OK, 1 = regression, 2 = not comparable. (Raising the baseline needs a
    complete run.)"""
    if base.get("golden") != cur.get("golden"):
        return 2, [f"not comparable: golden {base.get('golden')} vs {cur.get('golden')}"]
    lost, missing = [], []
    for p, ids in base.get("passing", {}).items():
        now_pass = set(cur.get("passing", {}).get(p, []))
        now_fail = set(cur.get("failing", {}).get(p, []))
        for cid in ids:
            if cid in now_pass:
                continue
            (lost if cid in now_fail else missing).append(f"{cid} x {p}")
    n = sum(len(v) for v in base.get("passing", {}).values())
    msgs = [f"ratchet (passing set): {n - len(lost) - len(missing)}/{n} previously passing pairs still pass"]
    if missing:
        msgs.append(f"note: {len(missing)} previously passing pair(s) are no longer in the corpus")
    if lost:
        msgs.append(f"REGRESSION: {len(lost)} previously passing pair(s) now fail:")
        msgs += [f"  {x}" for x in lost[:200]]
        msgs.append("ratchet: REGRESSION")
        return 1, msgs
    msgs.append("ratchet: OK (no regression; passing-set check)")
    return 0, msgs


def load_json(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def summary_lines(rep: dict) -> list[str]:
    out = []
    for p, v in rep["per_profile"].items():
        if v["pairs"]:
            out.append(f"  {p:16s} {v['pass']:6d} / {v['pairs']:6d}  {_pct(v['pass'], v['pairs'])}")
    t = rep["total"]
    out.append(f"  {'TOTAL':16s} {t['pass']:6d} / {t['pairs']:6d}  {_pct(t['pass'], t['pairs'])}")
    sm = rep["sourcemaps"]
    out.append(f"  source maps (D5): {sm['match']} / {sm['pairs']}")
    hc = rep["causes"].get("harness_error", {}).get("count", 0)
    if hc:
        out.append(f"  HARNESS ERRORS: {hc}")
    return out

