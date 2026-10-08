#!/usr/bin/env python3
"""Generates corpus/unit/REPLAY.md (docs/PORTING.md §4.2) from the descriptors and helpers.

For every class the descriptors construct (`new`) it lists the constructor arities used and the
descriptors that use them; then every static call, instance method name, private field written by
`withFields`, lambda interface, enum constant, class literal and helper (with the source lines its
header cites). Static: it reads only corpus/unit/descriptors and oracle/replay/helpers.
Overloads are listed by arity; the Java signature replay resolved at run time (DSL.md "Overload
resolution") comes from corpus/unit/replay_signatures.tsv (--collect-signatures).
"""
import collections
import sys
import json
import os
import re

from paths import ROOT  # the main checkout (scripts/paths.py)
DESC = f"{ROOT}/corpus/unit/descriptors"
HELP = f"{ROOT}/oracle/replay/helpers"


def walk(x, desc, acc):
    if isinstance(x, dict):
        if "new" in x:
            acc["new"][x["new"]][len(x.get("args", []))].add(desc)
        if "static" in x:
            acc["static"][(x["static"], x["method"], len(x.get("args", [])))].add(desc)
        if "call" in x and "method" in x:
            acc["call"][(x["method"], len(x.get("args", [])))].add(desc)
        if "withFields" in x:
            for f in x.get("fields", {}):
                acc["withFields"][f].add(desc)
        if "getField" in x:
            acc["getField"][x["name"]].add(desc)
        if "lambda" in x and "iface" in x:
            acc["lambda"][(x["iface"], len(x["lambda"]))].add(desc)
        if "enum" in x and "name" in x and isinstance(x["enum"], str):
            acc["enum"][(x["enum"], x["name"])].add(desc)
        if "class" in x and isinstance(x["class"], str) and len(x) == 1:
            acc["classLit"][x["class"]].add(desc)
        if "helper" in x:
            acc["helper"][(x.get("package", "com.google.javascript.jscomp"), x["helper"])].add(desc)
        if "mutationPoint" in x:
            acc["mutationPoint"][x["mutationPoint"]].add(desc)
        for v in x.values():
            walk(v, desc, acc)
    elif isinstance(x, list):
        for v in x:
            walk(v, desc, acc)


def helper_header(pkg, holder):
    p = f"{HELP}/{pkg.replace('.', '/')}/{holder}.java"
    if not os.path.exists(p):
        return p, 0, ""
    with open(p, encoding="utf-8") as f:
        txt = f.read()
    m = re.match(r"/\*(.*?)\*/", txt, re.S)
    cites = sorted(set(re.findall(r"(test/\S+\.java)[^\n]*?lines? ([\d\-, ]+)", m.group(1) if m else "")))
    return os.path.relpath(p, ROOT), txt.count("\n"), "; ".join(f"{a} lines {b.strip()}" for a, b in cites)


SIG_DIR = f"{ROOT}/build/gate02/replay"
SIG_TSV = f"{ROOT}/corpus/unit/replay_signatures.tsv"


def collect_signatures():
    """Replay (`ReplayMain --sig-out`, run by gate 0.2's replay phase) logs the Java
    signature DSL.md "Overload resolution" resolved at every `new`/`static`/`call`/`helper` site.
    Collected into corpus/unit/replay_signatures.tsv (descriptor, lookup, declaring class, signature,
    [widened]); kept as is when no fresh .sig.txt files exist."""
    files = sorted(f for f in os.listdir(SIG_DIR) if f.endswith(".sig.txt")) if os.path.isdir(SIG_DIR) else []
    if not files:
        return
    lines = set()
    for fn in files:
        with open(os.path.join(SIG_DIR, fn), encoding="utf-8") as fh:
            lines.update(x.rstrip("\n") for x in fh if x.strip())
    with open(SIG_TSV, "w", encoding="utf-8") as f:
        f.write("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
        for x in sorted(lines):
            f.write(x + "\n")


def read_signatures():
    if not os.path.exists(SIG_TSV):
        return []
    with open(SIG_TSV, encoding="utf-8") as fh:
        rows = [x.rstrip("\n").split("\t") for x in fh][1:]
    return rows


def sig_note():
    rows = read_signatures()
    if not rows:
        return ("Calls are listed by name and arity. The exact Java signature chosen by DSL.md "
                "\"Overload resolution\" is not listed: corpus/unit/replay_signatures.tsv is missing.")
    return (f"**Resolved signatures:** the section \"Resolved call signatures\" lists, per descriptor, the "
            f"exact Java signature (declaring class, parameter types; `<init>` = constructor) that DSL.md \"Overload "
            f"resolution\" chose at every call site the replay exercised ({len(rows)} rows, logged by `ReplayMain --sig-out` "
            f"during gate 0.2's replay phase, also in `corpus/unit/replay_signatures.tsv`). The lookup column is the class "
            f"searched (for `call`, the receiver's runtime class) and the member name. Primitive parameter types (`int`, "
            f"`short`, `byte`, `float`, ...) give the Java numeric type each argument is converted to.")


def sig_section():
    rows = read_signatures()
    if not rows:
        return []
    L = ["", "## Resolved call signatures", "", "| descriptor | lookup | resolved signature |", "|---|---|---|"]
    for r in rows:
        d, lookup, decl, sig = r[0], r[1], r[2], r[3]
        wid = " (int widening)" if len(r) > 4 and r[4] == "widened" else ""
        L.append(f"| {d} | `{lookup}` | `{decl}.{sig}`{wid} |")
    return L


def main():
    if "--collect-signatures" in sys.argv:
        collect_signatures()
    acc = collections.defaultdict(lambda: collections.defaultdict(set))
    acc["new"] = collections.defaultdict(lambda: collections.defaultdict(set))
    cmaps = {}
    names = sorted(fn[:-5] for fn in os.listdir(DESC) if fn.endswith(".json"))
    for n in names:
        with open(f"{DESC}/{n}.json") as f:
            d = json.load(f)
        walk(d.get("cases", []), n, acc)
        if d.get("classMap"):
            cmaps[n] = d["classMap"]
    L = ["# Replay manifest (docs/PORTING.md §4.2)", "",
         "Generated by `scripts/unit_replay_manifest.py` from `corpus/unit/descriptors/` and `oracle/replay/helpers/`; "
         "do not edit by hand. It lists everything a replay harness must provide to rebuild the processors: the pass "
         "classes and their constructor arities, static and instance calls, private fields written with `withFields`, "
         "functional interfaces, enum constants, class literals, `classMap` substitutions and verbatim helpers. "
         "Semantics: DSL.md. Harness pipeline: HARNESS.md.", "",
         sig_note(), "",
         f"Descriptors: {len(names)}.", "", "## Constructed classes (`new`)", "",
         "| class | arities | descriptors |", "|---|---|---|"]
    for c in sorted(acc["new"]):
        ar = acc["new"][c]
        ds = sorted(set().union(*ar.values()))
        L.append(f"| `{c}` | {', '.join(str(a) for a in sorted(ar))} | {len(ds)}: {', '.join(ds[:6])}{' …' if len(ds) > 6 else ''} |")
    for title, key, fmt in (("Static calls", "static", lambda k: f"`{k[0]}.{k[1]}` / {k[2]}"),
                            ("Instance calls (method / arity)", "call", lambda k: f"`{k[0]}` / {k[1]}"),
                            ("Private fields written by `withFields`", "withFields", lambda k: f"`{k}`"),
                            ("Fields read by `getField`", "getField", lambda k: f"`{k}`"),
                            ("Lambda interfaces (iface / parameters)", "lambda", lambda k: f"`{k[0]}` / {k[1]}"),
                            ("Enum constants", "enum", lambda k: f"`{k[0]}.{k[1]}`"),
                            ("Class literals", "classLit", lambda k: f"`{k}`"),
                            ("Mutation points", "mutationPoint", lambda k: f"`{k}`")):
        L += ["", f"## {title}", "", "| item | descriptors |", "|---|---|"]
        for k in sorted(acc[key]):
            ds = sorted(acc[key][k])
            L.append(f"| {fmt(k)} | {len(ds)}: {', '.join(ds[:6])}{' …' if len(ds) > 6 else ''} |")
    L += ["", "## Helpers", "", "| helper | file | lines | cited test source | descriptors |", "|---|---|---:|---|---|"]
    tot = 0
    for (pkg, h) in sorted(acc["helper"]):
        holder = h.split(".")[0]
        path, n, cites = helper_header(pkg, holder)
        tot += n
        ds = sorted(acc["helper"][(pkg, h)])
        L.append(f"| `{h}` | `{path}` | {n} | {cites or '(see file header)'} | {', '.join(ds[:4])} |")
    L += ["", "## classMap substitutions", "", "| descriptor | recorded class | helper class |", "|---|---|---|"]
    for n, cm in sorted(cmaps.items()):
        for k, v in sorted(cm.items()):
            L.append(f"| {n} | `{k}` | `{v}` |")
    L += sig_section()
    with open(f"{ROOT}/corpus/unit/REPLAY.md", "w") as f:
        f.write("\n".join(L) + "\n")
    print("REPLAY.md:", len(acc["new"]), "classes,", len(acc["helper"]), "helper refs")


if __name__ == "__main__":
    main()
