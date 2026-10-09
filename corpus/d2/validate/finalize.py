#!/usr/bin/env python3
"""D2 corpus validation (docs/PORTING.md §4.4): turn the D2 candidates + golden results into the final corpus.

Inputs (read-only): corpus/d2/candidates/*.jsonl, corpus/d2/candidates/*.lock.json,
build/validate/scan.jsonl (scan_golden.py), build/validate/wp/results/*.json (wp_test.py).
Writes: corpus/d2/cases.jsonl, corpus/d2/sources.lock.json, corpus/d2/STATS.md (corpus statistics,
not committed), corpus/d2/JAVA_FAILURES.md, corpus/d2/WHOLE_PROGRAM.md, build/validate/drops.json.
Does not run Java.
"""
from __future__ import annotations

import base64
import collections
import hashlib
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))  # corpus/d2/validate/<file>
sys.path.insert(0, os.path.join(REPO, "gates/lib"))
import case_args  # noqa: E402
import run_reference as rr  # noqa: E402

D2 = os.path.join(REPO, "corpus/d2")
CAND = os.path.join(D2, "candidates")
VAL = os.path.join(REPO, "build/validate")
CL_RAW = "https://raw.githubusercontent.com/google/closure-library/{commit}/"
SOURCES = ["npm", "closure-library", "test262", "closure-self", "whole-program"]
ADV_TRIVIAL_PROFILES = ("advanced", "advanced_strict")


def jl(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(l) for l in f if l.strip()]


def jload(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def text(v):
    if isinstance(v, str):
        return v
    return base64.b64decode(v["base64"]).decode("utf-8", "replace")


def crash_detail(cid, prof):
    r = jload(os.path.join(REPO, rr.result_path(cid, prof)))
    se = text(r["stderr"])
    m = re.search(r"(?m)^(?:Exception in thread \"[^\"]*\" )?((?:java|com)\.[\w.$]+(?:Exception|Error))(?::\s*(.*))?$", se)
    exc = m.group(1) if m else "?"
    msg = (m.group(2) or "").strip() if m else ""
    if "INTERNAL COMPILER ERROR" in se:
        after = se.split("Please report this problem.", 1)[-1].strip().split("\n")
        msg = after[0].strip() if after else msg
        exc = "INTERNAL COMPILER ERROR (" + exc + ")"
    skip = ("Compiler.throwInternalError", "NodeTraversal.", "Preconditions", "AstValidator$1",
            "CompilerInput", "PhaseOptimizer", "Compiler.", "CombinedCompilerPass")
    frames = re.findall(r"(?m)^\s+at com\.google\.javascript\.jscomp\.([\w.$]+)\(", se)
    where = next((f for f in frames if not f.startswith(skip)), frames[0] if frames else "?")
    # The innermost "Caused by" (if any) usually names the real failing pass.
    causes = re.findall(r"(?m)^Caused by: ([\w.$]+(?:Exception|Error))(?::\s*(.*))?$", se)
    if causes:
        cexc, cmsg = causes[-1]
        tail = se[se.rfind("Caused by: "):]
        cframes = re.findall(r"(?m)^\s+at com\.google\.javascript\.jscomp\.([\w.$]+)\(", tail)
        cwhere = next((f for f in cframes if not f.startswith(skip)), cframes[0] if cframes else where)
        return exc, msg, where, f"{cexc}: {cmsg.strip()}"[:200], cwhere
    return exc, msg[:200], where, None, None


def main():
    profiles = case_args.load_profiles()
    cands = case_args.load_cases()
    scan = {(r["id"], r["profile"]): r for r in jl(os.path.join(VAL, "scan.jsonl"))}
    t262 = jload(os.path.join(CAND, "test262.lock.json"))
    meta = t262["case_meta"]
    wp_res = {}
    for fn in os.listdir(os.path.join(VAL, "wp/results")):
        r = jload(os.path.join(VAL, "wp/results", fn))
        wp_res[(r["id"], r["profile"])] = r

    # ---------------------------------------------------------------- drop decisions
    drops = []  # {id, profile, reason, ...}
    final = []
    dropped_cases = []
    for c in cands:
        allp = case_args.case_profiles(c, profiles)
        kept = []
        # A case whose committed or cached files are missing cannot be reproduced; its golden
        # results (e.g. JSC_READ_ERROR on a deleted shim) are not reference behaviour.
        missing_files = [f for f in c["inputs"] + c.get("externs", []) + c.get("shims", [])
                         if not os.path.exists(os.path.join(REPO, f))]
        for p in allp:
            r = scan[(c["id"], p)]
            if r.get("missing"):
                raise SystemExit(f"missing golden result {c['id']}/{p}")
            reason = None
            if missing_files and not (r["ice"] or r["java_exception"] or r["exit_code"] == 254):
                reason = "case_files_missing"
            elif r["timed_out"]:
                reason = "java_timeout"
            elif r["ice"] or r["java_exception"] or r["exit_code"] == 254:
                reason = "java_internal_compiler_error" if r["ice"] else "java_uncaught_exception"
            elif (p in ADV_TRIVIAL_PROFILES and r["output_empty"] and r["n_error_lines"] == 0
                  and r["n_warning_lines"] == 0 and r["summary"] in (None, [0, 0])
                  and r["stderr_len"] == 0 and r["stdout_len"] == 0):
                reason = "trivial_advanced_empty_output_no_diagnostics"
            if reason:
                drops.append({"id": c["id"], "source": c["source"], "profile": p, "reason": reason,
                              "exit_code": r["exit_code"]})
            else:
                kept.append(p)
        if not kept:
            dropped_cases.append(c["id"])
            continue
        nc = dict(c)
        nc["profiles"] = kept  # candidate order, then auto-applied profiles (the full matrix) listed explicitly
        tags = []
        if c["source"] == "test262":
            m = meta[c["id"]]
            expects_parse = not (m["negative"] and m["negative"].get("phase") == "parse")
            ws = scan[(c["id"], "ws")]  # ws is in every test262 case and never crashed
            errs = {k.split(":", 1)[1] for k in ws["diag_keys"] if k.startswith("ERROR:")}
            if expects_parse and "JSC_PARSE_ERROR" in errs:
                tags.append("test262:java-parser-rejects-valid")
            elif expects_parse and errs:
                tags.append("test262:java-errors-on-valid")
            if not expects_parse and not errs:
                tags.append("test262:java-accepts-invalid")
            if m["negative"]:
                tags.append(f"test262:negative-{m['negative'].get('phase')}")
        if len(kept) < len(allp):
            tags.append("java-crash-profiles-dropped")
        nc["tags"] = tags
        if c["source"] == "whole-program":
            tp = [p for p in kept if wp_res[(c["id"], p)]["passed"]]
            nc["test_profiles"] = tp
            if not tp:
                nc["test_command"] = None
        else:
            nc["test_profiles"] = None
        final.append(nc)
    final.sort(key=lambda c: c["id"])
    ids = [c["id"] for c in final]
    assert len(ids) == len(set(ids)), "duplicate case ids"

    # ---------------------------------------------------------------- lock merge
    files = {}  # path -> {sha256, bytes, source, fetch}
    tarballs = {}  # key -> {url, sha256, integrity, shasum?, dir, extract}

    def add(path, sha, source, fetch, nbytes=None):
        old = files.get(path)
        if old and old["sha256"] != sha:
            raise SystemExit(f"conflicting sha256 for {path}")
        if old:
            return
        e = {"sha256": sha, "source": source, "fetch": fetch}
        if nbytes is not None:
            e["bytes"] = nbytes
        files[path] = e

    cl = jload(os.path.join(CAND, "closure-library.lock.json"))
    cl_raw = CL_RAW.format(commit=cl["commit"])
    cl_prefix = cl["cache_dir"].rstrip("/") + "/"
    for p, e in cl["files"].items():
        add(p, e["sha256"], "closure-library", {"url": cl_raw + p[len(cl_prefix):]}, e.get("bytes"))
    for lic in [cl["license"]] + list(cl["license"].get("exceptions", {}).values()):
        p = lic["license_file"]
        add(p, lic["license_file_sha256"], "closure-library", {"url": cl_raw + p[len(cl_prefix):]})
    for p, sha in cl["shims"].items():
        add(p, sha, "closure-library", {"in_repo": True})

    cs = jload(os.path.join(CAND, "closure-self.lock.json"))
    for p, e in cs["files"].items():
        add(p, e["sha256"], "closure-self", {"in_repo": True}, e.get("bytes"))

    npm = jload(os.path.join(CAND, "npm.lock.json"))
    for key, pk in npm["packages"].items():
        tarballs[f"npm:{key}"] = {"url": pk["url"], "sha256": pk["sha256"], "integrity": pk["integrity"],
                                  "shasum": pk.get("shasum"), "dir": pk["dir"],
                                  "extract": "strip first path component into <dir>/package/ (regular files only)",
                                  "license": pk.get("license")}
        for p, sha in pk["files"].items():
            add(p, sha, "npm", {"tarball": f"npm:{key}"})
        if pk.get("license_file"):
            add(pk["license_file"], pk["license_file_sha256"], "npm", {"tarball": f"npm:{key}"})

    tl = t262["source"]
    for p, e in t262["files"].items():
        add(p, e["sha256"], "test262", {"url": e["url"]}, e.get("bytes"))
    lic_rel = tl["license_file"][len(tl["cache_dir"].rstrip("/")) + 1:]
    add(tl["license_file"], tl["license_file_sha256"], "test262",
        {"url": f"https://raw.githubusercontent.com/tc39/test262/{tl['commit']}/{lic_rel}"})

    wp = jload(os.path.join(CAND, "whole-program.lock.json"))
    wp_dirs = {}
    for cid, wc in wp["cases"].items():
        d = "corpus-cache/d2/whole-program/" + wc["group"].split(":", 1)[1]
        wp_dirs[cid] = d
        if wc["npm"]:
            tgz = d + "/" + os.path.basename(wc["npm"]["tarball"])
            tarballs[f"whole-program:{cid}"] = {"url": wc["npm"]["tarball"], "sha256": wc["npm"]["sha256"],
                                                "integrity": wc["npm"]["integrity"], "dir": d,
                                                "keep_tarball_as": tgz,
                                                "extract": "strip first path component into <dir>/package/ (regular files only)",
                                                "license": wc["license"]}
    for p, sha in wp["files"].items():
        cid = next(k for k, d in wp_dirs.items() if p.startswith(d + "/"))
        wc = wp["cases"][cid]
        d = wp_dirs[cid]
        rest = p[len(d) + 1:]
        if rest.startswith("repo/"):
            fetch = {"url": wc["repository"]["raw_base"] + rest[len("repo/"):]}
        elif rest.startswith("package/"):
            fetch = {"tarball": f"whole-program:{cid}"}
        elif rest.endswith(".tgz"):
            fetch = {"tarball_file": f"whole-program:{cid}"}
        else:
            raise SystemExit(f"unknown whole-program file {p}")
        add(p, sha, "whole-program", fetch)
    for p, sha in wp["generated"].items():
        add(p, sha, "whole-program", {"in_repo": True})

    # Shims, externs and inputs referenced by kept cases must all be covered; npm shims are not
    # in any candidate lock, so they are hashed here (committed files).
    dropped_set = set(dropped_cases)
    for c in final:
        for p in c["inputs"] + c["externs"] + c["shims"]:
            if p not in files:
                if p.startswith("corpus/d2/shims/"):
                    add(p, sha256_file(os.path.join(REPO, p)), c["source"], {"in_repo": True})
                else:
                    raise SystemExit(f"{c['id']}: {p} is not covered by any candidate lock")
    # Committed shim files of kept cases that are not compiler inputs (run.mjs, harness.mjs...).
    for c in final:
        d = f"corpus/d2/shims/{c['id']}"
        if os.path.isdir(os.path.join(REPO, d)):
            for fn in sorted(os.listdir(os.path.join(REPO, d))):
                p = f"{d}/{fn}"
                if p not in files:
                    add(p, sha256_file(os.path.join(REPO, p)), c["source"], {"in_repo": True})
    # Drop entries that only belong to dropped cases (their shims are deleted).
    for cid in dropped_set:
        for p in [p for p in files if p.startswith(f"corpus/d2/shims/{cid}/")]:
            del files[p]

    # Verify the whole lock against the current cache / repo, and fill in byte counts.
    bad = []
    for p, e in files.items():
        ap = os.path.join(REPO, p)
        if not os.path.exists(ap):
            bad.append(f"missing {p}")
            continue
        got = sha256_file(ap)
        if got != e["sha256"]:
            bad.append(f"sha256 {p}: {got} != {e['sha256']}")
        e.setdefault("bytes", os.path.getsize(ap))
    if bad:
        print("\n".join(bad[:50]))
        raise SystemExit(f"{len(bad)} lock problems")

    # Groups: which files a group needs (fetch_d2.sh --group).
    groups = collections.defaultdict(lambda: {"sources": set(), "cases": [], "files": set(), "tarballs": set()})
    for c in final:
        g = groups[c["group"]]
        g["sources"].add(c["source"])
        g["cases"].append(c["id"])
        for p in c["inputs"] + c["externs"] + c["shims"]:
            g["files"].add(p)
        if c["source"] == "npm":
            key = "npm:" + c["group"].split(":", 1)[1]
            g["tarballs"].add(key)
            for p, e in files.items():
                if e["fetch"].get("tarball") == key:
                    g["files"].add(p)
        elif c["source"] == "whole-program":
            d = wp_dirs[c["id"]]
            for p in files:
                if p.startswith(d + "/") or p.startswith(f"corpus/d2/shims/{c['id']}/"):
                    g["files"].add(p)
            if f"whole-program:{c['id']}" in tarballs:
                g["tarballs"].add(f"whole-program:{c['id']}")
        elif c["source"] == "test262":
            g["files"].add(tl["license_file"])
        elif c["source"] == "closure-library":
            g["files"].add(cl["license"]["license_file"])
            for pref, lic in cl["license"].get("exceptions", {}).items():
                if any(p.startswith(pref) for p in c["inputs"]):
                    g["files"].add(lic["license_file"])
        d = f"corpus/d2/shims/{c['id']}/"
        for p in files:
            if p.startswith(d):
                g["files"].add(p)
        for p in list(g["files"]):
            if files[p]["fetch"].get("tarball"):
                g["tarballs"].add(files[p]["fetch"]["tarball"])
    used_tarballs = set().union(*(g["tarballs"] for g in groups.values()))

    lock = {
        "schema": 1,
        "doc": "corpus/d2/FORMAT.md; fetched and verified by scripts/fetch_d2.sh",
        "generated_by": "corpus/d2/validate/finalize.py (D2 corpus validation)",
        "reference": profiles["reference"],
        "golden": {"root": rr.GOLDEN_ROOT, "jar_sha256": rr.JAR_SHA256},
        "cases": {"path": "corpus/d2/cases.jsonl", "count": len(final),
                  "sha256": None},
        "allowed_hosts": ["registry.npmjs.org", "raw.githubusercontent.com"],
        "sources": {
            "closure-library": {"kind": "git raw files", "repository": cl["git_url"], "tag": cl["tag"],
                                "commit": cl["commit"], "raw_base": cl_raw, "cache_dir": cl["cache_dir"],
                                "license": cl["license"], "candidate_lock": "corpus/d2/candidates/closure-library.lock.json"},
            "closure-self": {"kind": "reference checkout (verified, not downloaded)", **cs["reference"],
                             "candidate_lock": "corpus/d2/candidates/closure-self.lock.json"},
            "npm": {"kind": "npm registry tarballs", "registry": npm["registry"],
                    "verification": npm["verification"], "allowed_licenses": npm["allowed_licenses"],
                    "candidate_lock": "corpus/d2/candidates/npm.lock.json"},
            "test262": {"kind": "git raw files", "repository": tl["repo"], "commit": tl["commit"],
                        "license": tl["license"], "license_file": tl["license_file"],
                        "cache_dir": tl["cache_dir"], "candidate_lock": "corpus/d2/candidates/test262.lock.json"},
            "whole-program": {"kind": "npm registry tarballs + git raw files (tests)",
                              "cases": {k: {kk: vv for kk, vv in v.items()} for k, v in wp["cases"].items()
                                        if k not in dropped_set},
                              "verification_original": wp["verification"],
                              "candidate_lock": "corpus/d2/candidates/whole-program.lock.json"},
        },
        "tarballs": {k: tarballs[k] for k in sorted(tarballs) if k in used_tarballs},
        "groups": {g: {"sources": sorted(v["sources"]), "cases": sorted(v["cases"]),
                       "tarballs": sorted(v["tarballs"]), "files": sorted(v["files"])}
                   for g, v in sorted(groups.items())},
        "files": dict(sorted(files.items())),
    }

    # ---------------------------------------------------------------- write corpus files
    cases_path = os.path.join(D2, "cases.jsonl")
    with open(cases_path, "w", encoding="utf-8") as f:
        for c in final:
            f.write(json.dumps(c, ensure_ascii=False) + "\n")
    lock["cases"]["sha256"] = sha256_file(cases_path)
    with open(os.path.join(D2, "sources.lock.json"), "w", encoding="utf-8") as f:
        json.dump(lock, f, indent=1, ensure_ascii=False)
        f.write("\n")
    with open(os.path.join(VAL, "drops.json"), "w") as f:
        json.dump({"pairs": drops, "cases": dropped_cases}, f, indent=1)

    # Sanity: case_args accepts every kept pair; report auto-apply re-additions.
    readded = []
    for c in final:
        for p in c["profiles"]:
            case_args.compiler_args(c, p, profiles=profiles)
        extra = [p for p in case_args.case_profiles(c, profiles) if p not in c["profiles"]]
        if extra:
            readded.append((c["id"], extra))

    write_reports(cands, final, drops, dropped_cases, scan, files, wp_res, meta, readded, lock)
    print(json.dumps({"cases": len(final), "dropped_cases": dropped_cases, "dropped_pairs": len(drops),
                      "kept_pairs": sum(len(c["profiles"]) for c in final),
                      "auto_apply_readded": readded,
                      "lock_files": len(files), "groups": len(lock["groups"])}, indent=1))


def md_table(headers, rows):
    out = ["| " + " | ".join(headers) + " |", "|" + "|".join("---" for _ in headers) + "|"]
    for r in rows:
        out.append("| " + " | ".join(str(x) for x in r) + " |")
    return "\n".join(out)


def write_reports(cands, final, drops, dropped_cases, scan, files, wp_res, meta, readded, lock):
    profiles_order = list(case_args.load_profiles()["profiles"])
    src_order = SOURCES
    by_src = collections.defaultdict(list)
    for c in final:
        by_src[c["source"]].append(c)
    cand_by_src = collections.Counter(c["source"] for c in cands)
    kept_pairs = [(c, p) for c in final for p in c["profiles"]]

    def case_bytes(c):
        return sum(files[p]["bytes"] for p in c["inputs"] + c["externs"] + c["shims"])

    # ---------------- STATS.md
    L = []
    L.append("# D2 corpus statistics\n")
    L.append("Generated by `corpus/d2/validate/finalize.py` from the candidates, the golden results in "
             f"`{rr.GOLDEN_ROOT}/` (reference jar `{rr.JAR_SHA256[:12]}…`) and the whole-program test runs. "
             "Do not edit by hand.\n")
    L.append("## Totals\n")
    tot_pairs = sum(len(case_args.case_profiles(c)) for c in cands)
    L.append(md_table(["", "Candidates", "Final"], [
        ["Cases", len(cands), len(final)],
        ["(case, profile) pairs", tot_pairs, len(kept_pairs)],
        ["Groups (cases of one origin)", len({c['group'] for c in cands}), len({c['group'] for c in final})],
    ]))
    L.append("")
    L.append("Requirement: at least 2,000 cases with every source represented: "
             f"**{'met' if len(final) >= 2000 and all(by_src[s] for s in src_order) else 'NOT met'}** "
             f"({len(final)} cases, {sum(1 for s in src_order if by_src[s])} of {len(src_order)} sources).\n")
    # Profile matrix (docs/PORTING.md §4.4): every case x every profile that applies to it (profiles.json "matrix").
    allprof = case_args.load_profiles()
    chunk_profiles = [p for p, sp in allprof["profiles"].items() if sp.get("output") == "chunks"]
    applicable = sum(sum(1 for p in allprof["profiles"] if case_args.profile_applies(allprof, c, p)) for c in cands)
    by_design_missing = applicable - tot_pairs
    L.append("Profile matrix (docs/PORTING.md §4.4, `profiles.json` \"matrix\"): every candidate case runs under every profile that "
             f"applies to it: {len(allprof['profiles']) - len(chunk_profiles)} single-output profiles for every case, and "
             f"{len(chunk_profiles)} chunk configurations ({', '.join('`' + p + '`' for p in chunk_profiles)}) for every case "
             f"with at least 2 `--js` sources. Applicable matrix: {applicable:,} pairs; omitted by design: "
             f"{by_design_missing:,}; removed by the drop rules (Java crash or timeout, trivial ADVANCED, missing case files; see Drop reasons): "
             f"{tot_pairs - len(kept_pairs):,}; final: {len(kept_pairs):,}. "
             f"**{'met' if by_design_missing == 0 and len(chunk_profiles) >= 2 else 'NOT met'}** "
             "apart from the drop rules.\n")
    L.append("## Per source\n")
    rows = []
    for s in src_order:
        cs = by_src[s]
        uniq = set()
        for c in cs:
            uniq.update(c["inputs"])
        rows.append([s, cand_by_src[s], len(cs), len({c['group'] for c in cs}),
                     sum(len(c["profiles"]) for c in cs),
                     f"{sum(case_bytes(c) for c in cs):,}", f"{sum(files[p]['bytes'] for p in uniq):,}"])
    rows.append(["**total**", len(cands), len(final), len({c['group'] for c in final}), len(kept_pairs),
                 f"{sum(case_bytes(c) for c in final):,}",
                 f"{sum(files[p]['bytes'] for p in set().union(*[set(c['inputs']) for c in final])):,}"])
    L.append(md_table(["Source", "Candidate cases", "Final cases", "Groups", "Pairs",
                       "Bytes (sum over cases: inputs+externs+shims)", "Bytes (unique input files)"], rows))
    L.append("")
    L.append("## Per profile (final pairs)\n")
    rows = []
    for p in profiles_order:
        ps = [(c, q) for c, q in kept_pairs if q == p]
        ex0 = sum(1 for c, q in ps if scan[(c["id"], q)]["exit_code"] == 0)
        per = collections.Counter(c["source"] for c, _ in ps)
        rows.append([p, len(ps), ex0, len(ps) - ex0] + [per.get(s, 0) for s in src_order])
    L.append(md_table(["Profile", "Pairs", "Exit 0", "Exit ≠ 0 (diagnostic parity)"] + src_order, rows))
    L.append("")
    L.append("## Per license\n")
    lic = collections.Counter(c["license"] for c in final)
    lic_src = collections.defaultdict(collections.Counter)
    for c in final:
        lic_src[c["license"]][c["source"]] += 1
    L.append(md_table(["License", "Cases"] + src_order,
                      [[k, v] + [lic_src[k].get(s, 0) for s in src_order] for k, v in lic.most_common()]))
    L.append("")
    L.append("## Drop reasons\n")
    dr = collections.Counter(d["reason"] for d in drops)
    rows = [[k, v, len({d['id'] for d in drops if d['reason'] == k})] for k, v in dr.most_common()]
    rows.append(["trivial_advanced_empty_output_no_diagnostics", 0, 0] if "trivial_advanced_empty_output_no_diagnostics" not in dr else None)
    rows.append(["java_timeout", 0, 0] if "java_timeout" not in dr else None)
    L.append(md_table(["Reason", "Pairs dropped", "Cases affected"], [r for r in rows if r]))
    L.append("")
    L.append(f"Cases dropped because no profile remained: {len(dropped_cases)} "
             f"({', '.join('`' + c + '`' for c in dropped_cases) or 'none'}). Details: `JAVA_FAILURES.md`.\n")
    dsrc = collections.Counter((d["source"], d["profile"]) for d in drops)
    L.append("Dropped pairs by source and profile:\n")
    L.append(md_table(["Source"] + profiles_order,
                      [[s] + [dsrc.get((s, p), 0) for p in profiles_order] for s in src_order]))
    L.append("")
    n_trivial = sum(1 for d in drops if d["reason"] == "trivial_advanced_empty_output_no_diagnostics")
    L.append("The trivial-ADVANCED rule (empty output and zero diagnostics) applies to `advanced` and "
             f"`advanced_strict`. It matched {n_trivial} pair(s).")
    for cp in [p for p, sp in case_args.load_profiles()["profiles"].items() if sp.get("output") == "chunks"]:
        L.append(f"For reference, `{cp}` (also ADVANCED) has "
                 f"{sum(1 for (cid, p), r in scan.items() if p == cp and r['output_empty'] and r['exit_code'] == 0 and r['n_error_lines'] + r['n_warning_lines'] == 0)} "
                 "pairs with empty output and zero diagnostics, and "
                 f"{sum(1 for (cid, p), r in scan.items() if p == cp and r['output_empty'] and r['exit_code'] == 0)} "
                 "with empty output and exit 0; they were kept.")
    L.append("")
    if readded:
        L.append("**Auto-apply caveat.** Every profile is auto-applied to candidate cases by "
                 f"`case_args.case_profiles()`. For {len(readded)} final case/profile pairs an auto-applied pair was "
                 "dropped (Java crash), and `case_profiles()` returns a final case's list unchanged, so it is not "
                 "added back: " + ", ".join(f"`{c}` ({', '.join(p)})" for c, p in readded) +
                 ". Final `profiles` lists are explicit.\n")
    L.append("## Exit-code distribution (final pairs)\n")
    ec = collections.Counter(scan[(c["id"], p)]["exit_code"] for c, p in kept_pairs)
    big = [(k, v) for k, v in sorted(ec.items()) if k in (0, 1, 2, 3, 4, 5, 9, 10, 127)]
    other = sum(v for k, v in ec.items() if k not in (0, 1, 2, 3, 4, 5, 9, 10, 127))
    L.append(md_table(["Exit code", "Pairs"], [[k, v] for k, v in big] + [["other (6–124)", other]]))
    L.append("")
    L.append("Exit code = number of errors, capped at 127 (`AbstractCommandLineRunner`). Pairs with a non-zero "
             "exit have no output file; they are kept for diagnostic parity.\n")
    L.append("Per source and exit class:\n")
    rows = []
    for s in src_order:
        ps = [(c, p) for c, p in kept_pairs if c["source"] == s]
        e = collections.Counter(scan[(c["id"], p)]["exit_code"] for c, p in ps)
        rows.append([s, len(ps), e.get(0, 0), sum(v for k, v in e.items() if 1 <= k <= 126), e.get(127, 0)])
    L.append(md_table(["Source", "Pairs", "Exit 0", "Exit 1–126", "Exit 127 (capped)"], rows))
    L.append("")
    L.append("## Tags\n")
    tg = collections.Counter(t for c in final for t in c["tags"])
    L.append(md_table(["Tag", "Cases"], sorted(tg.items())))
    L.append("")
    L.append("- `test262:java-parser-rejects-valid`: test262 expects the file to parse (not a negative parse "
             "test), but Java reports `ERROR - [JSC_PARSE_ERROR]` (under `ws`, and the same under every profile). "
             "Kept for diagnostic parity.\n"
             "- `test262:java-errors-on-valid`: expected to parse, Java parses it but reports another error "
             "(for example `JSC_VAR_ARGUMENTS_SHADOWED_ERROR`).\n"
             "- `test262:java-accepts-invalid`: a negative parse test that Java compiles with no error.\n"
             "- `test262:negative-<phase>`: the test262 `negative` phase.\n"
             "- `java-crash-profiles-dropped`: at least one profile was dropped because Java crashed.\n")
    L.append("## Whole-program cases\n")
    wpc = by_src["whole-program"]
    L.append(f"{len(wpc)} cases; {sum(1 for c in wpc if c['test_profiles'])} have a test suite that passes on "
             f"Java's output under at least one profile; {sum(len(c['test_profiles']) for c in wpc)} of "
             f"{sum(len(c['profiles']) for c in wpc)} (case, profile) pairs carry a test requirement. "
             "See `WHOLE_PROGRAM.md`.\n")
    with open(os.path.join(D2, "STATS.md"), "w") as f:
        f.write("\n".join(L))

    # ---------------- JAVA_FAILURES.md
    L = ["# Java reference failures in the D2 candidates\n",
         "These (case, profile) pairs were **dropped** from the D2 oracle set because the pinned reference "
         f"(`{rr.JAR_SHA256[:12]}…`, commit `{rr.paths.REF_COMMIT[:7]}`) crashed with an internal compiler error or an "
         "uncaught exception (exit code 254). No run timed out. They are interesting reference behaviour, "
         "but not oracle cases: a faithful port would have to reproduce a Java crash. The golden-run report re-ran two "
         "of them (lottie-web cjs / ws and html_test_vectors / simple) and got byte-identical results, so the crashes are repeatable.\n",
         "Generated by `corpus/d2/validate/finalize.py`. The golden results stay in "
         f"`{rr.GOLDEN_ROOT}/<case>/<profile>.json`.\n"]
    details = []
    for d in drops:
        if d["reason"].startswith("java_"):
            exc, msg, where, cause, cwhere = crash_detail(d["id"], d["profile"])
            details.append((d, exc, msg, where, cause, cwhere))
    L.append("## Summary by failure\n")
    grp = collections.defaultdict(list)
    sample = {}
    for d, exc, msg, where, cause, cwhere in details:
        key = (exc, where if not cwhere else cwhere)
        grp[key].append(d)
        m = re.sub(r"\s+\[.*$", "", (cause or msg)).strip()
        sample.setdefault(key, (m[:110], d["id"], d["profile"]))
    rows = []
    for key, ds in sorted(grp.items(), key=lambda kv: (-len(kv[1]), kv[0])):
        exc, where = key
        m, sid, sprof = sample[key]
        rows.append([len(ds), len({d['id'] for d in ds}),
                     ", ".join(sorted({d['profile'] for d in ds}, key=lambda p: list(case_args.load_profiles()['profiles']).index(p))),
                     f"`{exc}`", f"`{where}`", m.replace("|", "\\|") + f" (e.g. `{sid}`/{sprof})"])
    L.append(md_table(["Pairs", "Cases", "Profiles", "Exception", "First jscomp frame (pass)", "Example message"], rows))
    L.append("")
    L.append(f"Total: {len(details)} pairs in {len({d['id'] for d, *_ in details})} cases. "
             f"Cases dropped entirely (no profile left): {', '.join('`' + c + '`' for c in dropped_cases) or 'none'}.\n")
    L.append("## All dropped pairs\n")
    rows = []
    for d, exc, msg, where, cause, cwhere in sorted(details, key=lambda x: (x[0]["id"], x[0]["profile"])):
        rows.append([f"`{d['id']}`", d["profile"], d["reason"].replace("java_", ""), f"`{(cwhere or where)}`"])
    L.append(md_table(["Case", "Profile", "Kind", "Frame"], rows))
    L.append("")
    with open(os.path.join(D2, "JAVA_FAILURES.md"), "w") as f:
        f.write("\n".join(L))

    # ---------------- WHOLE_PROGRAM.md
    L = ["# Whole-program cases: library tests on Java's output\n",
         "Each whole-program case's `test_command` (`node corpus/d2/shims/<id>/run.mjs {output}`) was run "
         "against the golden Java output of every profile of the case (`corpus/d2/validate/wp_test.py`, "
         f"node {wp_node_version()}). For a chunk profile (`chunks2`, `chunks3`), `{{output}}` is the chunk outputs "
         "concatenated in chunk order with a newline between them (`c0.js`, `c1.js`[, `c2.js`]), i.e. the chunks "
         "loaded as consecutive scripts.\n",
         "A case's `test_profiles` lists the profiles whose Java output passes the library's own tests. "
         "Only those profiles carry the test requirement (D2: Rust output must pass too). Profiles whose "
         "Java output fails keep the byte-for-byte output comparison but have no test requirement.\n"]
    wpc = sorted(by_src["whole-program"], key=lambda c: c["id"])
    rows = []
    for c in wpc:
        cells = []
        for p in c["profiles"]:
            r = wp_res[(c["id"], p)]
            t = r.get("tests")
            cells.append(f"{p}: {'pass' if r['passed'] else 'FAIL'}" + (f" ({t[1]}/{t[0]})" if t else ""))
        rows.append([f"`{c['id']}`", ", ".join(cells), ", ".join(c["test_profiles"]) or "none"])
    L.append(md_table(["Case", "Result per profile (passed/tests)", "test_profiles"], rows))
    L.append("")
    fails = [(c, p) for c in wpc for p in c["profiles"] if not wp_res[(c["id"], p)]["passed"]]
    L.append(f"## Failures ({len(fails)} pairs)\n")
    if not fails:
        L.append("None.\n")
    for c, p in fails:
        r = wp_res[(c["id"], p)]
        L.append(f"### `{c['id']}` / `{p}`\n")
        L.append(f"- Java exit code {r['java_exit_code']}; test exit {r.get('exit')}; tests (total, pass, fail, skip): {r.get('tests')}.")
        if r.get("detail"):
            L.append(f"- {r['detail']}")
        for l in r.get("first_failures", []):
            L.append(f"- `{l}`")
        msg = next((l.strip(" #") for l in r.get("tail", []) if "Error" in l and " at " not in l), None)
        if msg:
            L.append(f"- First error: `{msg}`")
        L.append("")
    if any(c["id"] == "whole-program-acorn-8.19.0" for c, _ in fails):
        acorn_fail = sorted(p for c, p in fails if c["id"] == "whole-program-acorn-8.19.0")
        L.append(f"acorn: failing profiles {', '.join('`' + p + '`' for p in acorn_fail)}. Under ADVANCED "
                 "(`advanced`, `chunks2`) the compiled parser throws "
                 "`TypeError: this is not a constructor` on the first `parse()` call, so both test groups fail. "
                 "This is the reference's ADVANCED output, not a harness problem.\n")
    n_ok = sum(1 for c in wpc if c["test_profiles"])
    L.append(f"## Count\n\n{n_ok} of {len(wpc)} whole programs keep a test requirement on at least one profile "
             f"(the corpus requires 10 or more).\n")
    with open(os.path.join(D2, "WHOLE_PROGRAM.md"), "w") as f:
        f.write("\n".join(L))


def wp_node_version():
    import subprocess
    try:
        return subprocess.run(["node", "--version"], capture_output=True, text=True).stdout.strip()
    except OSError:
        return "?"


if __name__ == "__main__":
    main()
