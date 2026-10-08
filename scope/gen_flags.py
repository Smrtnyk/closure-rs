#!/usr/bin/env python3
"""Generate scope/flags.txt from the pinned CommandLineRunner (docs/PORTING.md §2 "Scope", D-016 item 4).

    python3 scope/gen_flags.py            write scope/flags.txt
    python3 scope/gen_flags.py --check    exit 1 if scope/flags.txt differs from what the source gives
    python3 scope/gen_flags.py --stdout   print instead of writing

Every `@Option(...)` in `CommandLineRunner.Flags` becomes one row: the flag, its aliases, its
type (from the annotated field, or `boolean` when the handler is BooleanOptionHandler), the
field's default, whether it is hidden, the usage text, the scope verdict and the reason for it.

Scope rules (docs/PORTING.md §2, applied in this order; the first match decides):
  1. usage text says "DO NOT USE" (case-insensitive)            -> out
  2. usage text says "experimental" (case-insensitive)          -> out
  3. the flag belongs to an out-of-scope area: refactoring, lint, instrumentation, ant, debugger,
     J2CL, Polymer, Chrome. Membership is decided by AREA_RULES below: a regular expression over
     the flag name (and aliases), each with the evidence that ties it to the area (the package or
     the pass it configures). A rule that matches no flag is still listed in the header, with
     its match count, so a missing exclusion is visible.
  4. otherwise                                                  -> in

The generator never adds an exclusion that docs/PORTING.md §2 does not name (an added exclusion
needs a decision in DECISIONS.md). The output is deterministic: rows are in source order,
whitespace in usage text is collapsed, and the header records the source's sha256.
"""
from __future__ import annotations

import hashlib
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
CLR = "reference/closure-compiler/src/com/google/javascript/jscomp/CommandLineRunner.java"
OUT = "scope/flags.txt"

# (area, regex over the flag name and its aliases, evidence). docs/PORTING.md §2 lists these areas.
AREA_RULES = [
    ("refactoring", r"refactor",
     "configures jscomp/refactoring/"),
    ("lint", r"(^|_)lint(_|$)",
     "configures the standalone linter (jscomp/lint/, Linter, LintPassConfig)"),
    ("instrumentation", r"instrument",
     "configures code instrumentation (jscomp/instrumentation/: CoverageInstrumentationPass, "
     "ProductionCoverageInstrumentationCallback)"),
    ("ant", r"(^|_)ant(_|$)",
     "configures the Ant task (jscomp/ant/)"),
    ("debugger", r"debugger",
     "configures jscomp/debugger/"),
    ("J2CL", r"j2cl",
     "configures the J2CL passes (J2clPass, J2clAssertRemovalPass, ...)"),
    ("Polymer", r"polymer",
     "configures the Polymer passes (PolymerPass and friends)"),
    ("Chrome", r"chrome",
     "configures the Chrome-specific passes (ChromePass)"),
]

COLUMNS = ["scope", "flag", "aliases", "type", "default", "hidden", "category", "reason", "usage"]


def _java_string_concat(expr: str) -> str:
    """Concatenate the Java string literals of `"a" + "b"` (escapes \\" \\\\ \\n \\t)."""
    out = []
    for m in re.finditer(r'"((?:[^"\\]|\\.)*)"', expr):
        s = m.group(1)
        s = re.sub(r"\\(.)", lambda e: {"n": "\n", "t": "\t"}.get(e.group(1), e.group(1)), s)
        out.append(s)
    return "".join(out)


def _balanced(src: str, i: int) -> int:
    """Index just past the ')' that closes the '(' at src[i], skipping string literals."""
    depth, j, n = 0, i, len(src)
    while j < n:
        c = src[j]
        if c == '"':
            j += 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    raise ValueError("unbalanced @Option(")


def _attrs(body: str) -> dict[str, str]:
    """Split `k = v, k2 = v2` at top level (commas outside strings, braces and parens)."""
    parts, cur, depth, j, n = [], [], 0, 0, len(body)
    while j < n:
        c = body[j]
        if c == '"':
            k = j + 1
            while k < n and body[k] != '"':
                k += 2 if body[k] == "\\" else 1
            cur.append(body[j:k + 1])
            j = k + 1
            continue
        if c in "({":
            depth += 1
        elif c in ")}":
            depth -= 1
        if c == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(c)
        j += 1
    parts.append("".join(cur))
    out = {}
    for p in parts:
        # Drop // comments inside the annotation (e.g. "// no usage").
        p = re.sub(r"//[^\n]*", "", p).strip()
        if "=" in p:
            k, v = p.split("=", 1)
            out[k.strip()] = v.strip()
    return out


def parse(src: str) -> list[dict]:
    rows = []
    for m in re.finditer(r"@Option\s*\(", src):
        start = m.end() - 1
        end = _balanced(src, start)
        a = _attrs(src[start + 1:end - 1])
        # The declaration after the annotation: a field (`private T name = v;` / `private T name;`).
        decl_m = re.match(r"\s*(?:private|public|protected)\s+(?P<decl>[^;]*);", src[end:], re.S)
        if not decl_m:
            raise SystemExit(f"cannot parse the declaration after @Option at offset {m.start()}")
        decl = " ".join(decl_m.group("decl").split())
        dm = re.match(r"(?P<type>.+?)\s+(?P<field>\w+)\s*(?:=\s*(?P<default>.*))?$", decl)
        if not dm:
            raise SystemExit(f"cannot parse declaration {decl!r}")
        ftype = re.sub(r"@Nullable\s+", "", dm.group("type")).strip()
        default = (dm.group("default") or "").strip()
        handler = a.get("handler", "")
        if "BooleanOptionHandler" in handler:
            typ = "boolean"
        else:
            typ = ftype.replace("CompilerOptions.", "").replace("ModuleLoader.", "") \
                .replace("SourceMap.", "")
        aliases = re.findall(r'"([^"]+)"', a.get("aliases", ""))
        rows.append({
            "flag": _java_string_concat(a["name"]),
            "aliases": aliases,
            "type": typ,
            "field": dm.group("field"),
            "default": default.replace("CompilerOptions.", "") or "(none)",
            "hidden": a.get("hidden", "false").strip() == "true",
            "usage": " ".join(_java_string_concat(a.get("usage", "")).split()),
        })
    return rows


def categories(src: str) -> dict[str, str]:
    """The help categories (Flags.categories), flag name without dashes -> category."""
    m = re.search(r"ImmutableMultimap<String, String> categories\s*=(.*?)\.build\(\);", src, re.S)
    out = {}
    if m:
        for cm in re.finditer(r'\.putAll\(\s*"([^"]+)"\s*,\s*ImmutableList\.of\((.*?)\)\)', m.group(1), re.S):
            for n in re.findall(r'"([^"]+)"', cm.group(2)):
                out[n] = cm.group(1)
    return out


def classify(row: dict, rule_hits: dict[str, int]) -> tuple[str, str]:
    u = row["usage"]
    if re.search(r"do not use", u, re.I):
        return "out", "docs/PORTING.md §2: usage says \"DO NOT USE\""
    if re.search(r"experimental", u, re.I):
        return "out", "docs/PORTING.md §2: usage says \"experimental\""
    names = [row["flag"]] + row["aliases"]
    for area, rx, evidence in AREA_RULES:
        if any(re.search(rx, n.lstrip("-"), re.I) for n in names):
            rule_hits[area] += 1
            return "out", f"docs/PORTING.md §2: {area} area; {evidence}"
    return "in", "no docs/PORTING.md §2 exclusion applies"


def render() -> str:
    path = os.path.join(REPO, CLR)
    with open(path, "rb") as f:
        raw = f.read()
    src = raw.decode("utf-8")
    rows = parse(src)
    cats = categories(src)
    rule_hits = {area: 0 for area, _, _ in AREA_RULES}
    for r in rows:
        r["scope"], r["reason"] = classify(r, rule_hits)
        r["category"] = cats.get(r["flag"].lstrip("-"), "(uncategorized)")
    n_in = sum(1 for r in rows if r["scope"] == "in")
    hdr = [
        "# scope/flags.txt: CommandLineRunner flags and their docs/PORTING.md §2 scope.",
        "# GENERATED by scope/gen_flags.py; do not edit by hand (see scope/README.md).",
        f"# source: {CLR}",
        f"# source sha256: {hashlib.sha256(raw).hexdigest()}",
        f"# flags: {len(rows)} @Option fields; in scope: {n_in}; out of scope: {len(rows) - n_in}",
        "# rules (first match decides): usage says \"DO NOT USE\" -> out; usage says \"experimental\" -> out;",
        "#   area rule matches the flag name or an alias -> out; otherwise in.",
        "# area rules (area: regex over the name without dashes, flags matched):",
    ]
    for area, rx, _ in AREA_RULES:
        hdr.append(f"#   {area}: /{rx}/i, {rule_hits[area]} flag(s)")
    hdr += [
        "# columns (tab-separated): " + ", ".join(COLUMNS),
        "#   aliases: comma-separated or '-'; type: the annotated field's Java type ('boolean' for",
        "#   BooleanOptionHandler); default: the field initializer; hidden: yes/no; category: the --help",
        "#   category (Flags.categories); usage: the usage text, whitespace collapsed ('' when none).",
    ]
    lines = []
    for r in rows:
        vals = {"scope": r["scope"], "flag": r["flag"], "aliases": ",".join(r["aliases"]) or "-",
                "type": r["type"], "default": r["default"], "hidden": "yes" if r["hidden"] else "no",
                "category": r["category"], "reason": r["reason"], "usage": r["usage"]}
        for k, v in vals.items():
            if "\t" in v or "\n" in v:
                raise SystemExit(f"tab or newline in {k} of {r['flag']}")
        lines.append("\t".join(vals[c] for c in COLUMNS))
    return "\n".join(hdr) + "\n" + "\n".join(lines) + "\n"


def load(path: str = os.path.join(REPO, OUT)) -> list[dict]:
    """Rows of scope/flags.txt as dicts (for gates and tests)."""
    out = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            out.append(dict(zip(COLUMNS, line.rstrip("\n").split("\t"))))
    return out


def main():
    text = render()
    if "--stdout" in sys.argv:
        sys.stdout.write(text)
        return
    dst = os.path.join(REPO, OUT)
    if "--check" in sys.argv:
        cur = open(dst, encoding="utf-8").read() if os.path.exists(dst) else ""
        if cur != text:
            print(f"{OUT} is out of date; run python3 scope/gen_flags.py", file=sys.stderr)
            sys.exit(1)
        print(f"{OUT} is up to date")
        return
    with open(dst, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
