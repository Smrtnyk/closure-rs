#!/usr/bin/env python3
"""Derive the effective expected-side pipeline of every compiler_test_case record.

The recorder's `comparison.normalizeExpected` is `normalizeEnabled || normalizeExpectedOutputEnabled`
(oracle/patches/0002-recording-hooks.patch), which is NOT the condition under which Java normalizes
the expected AST. Java's parseExpectedJs (CompilerTestCase.java:1599-1646, pinned bb8c8e7) applies,
in this order, on a fresh compiler initialised with defaultExternsInputs and getOptions():
  1. RemoveCastNodes                      if replaceTypesWithColors || multistageCompilation
  2. GatherModuleMetadata(BROWSER) then ProcessClosurePrimitives
                                          if closurePassEnabled && closurePassEnabledForExpected && !hasErrors
  3. ClosureRewriteModule then ScopedAliases   if rewriteClosureCode
  4. ProcessClosureProvidesAndRequires(false)  if rewriteClosureProvides && closurePassEnabledForExpected && !hasErrors
  5. rewriteEsModules then transpileToEs5 if transpileEnabled && !hasErrors
     else Normalize.createNormalizeForOptimizations
                                          if normalizeEnabled && normalizeExpectedOutputEnabled && !hasErrors
`hasErrors` is the expected-side compiler's error state at that point (dynamic, not a flag).
The flags below are the static parts, computed from harness.fields with harness.fieldsAfterGetOptions
applied (the values getOptions() leaves, which is when parseExpectedJs reads them).

Output: corpus/unit/derived/expected_pipeline.jsonl.gz, one line per compiler_test_case record, in
record-file order (files sorted by name): {file, index, class, method, call, flags...}. `file` is the
record file's base name without .jsonl.gz and `index` the 0-based line number in it.  --check re-derives and exits 1 if the committed file differs.
"""
import gzip, json, os, sys

from paths import ROOT  # the main checkout (scripts/paths.py)
REC = f"{ROOT}/corpus/unit/records"
OUT = f"{ROOT}/corpus/unit/derived/expected_pipeline.jsonl.gz"


def flags(rec):
    f = dict(rec["harness"]["fields"])
    f.update(rec["harness"].get("fieldsAfterGetOptions") or {})
    b = lambda k: f.get(k) is True
    transpile = b("transpileEnabled")
    return {
        "removeCastsExpected": b("replaceTypesWithColors") or b("multistageCompilation"),
        "closurePassForExpected": b("closurePassEnabled") and b("closurePassEnabledForExpected"),
        "closureRewriteModuleForExpected": b("rewriteClosureCode"),
        "closureProvidesForExpected": b("rewriteClosureProvides") and b("closurePassEnabledForExpected"),
        "transpileExpected": transpile,
        "normalizeExpected": (not transpile) and b("normalizeEnabled") and b("normalizeExpectedOutputEnabled"),
        "transpileNormalizes": transpile and b("normalizeEnabled"),
    }


def derive():
    lines = []
    for fn in sorted(os.listdir(REC)):
        if not fn.endswith(".jsonl.gz"):
            continue
        with gzip.open(os.path.join(REC, fn), "rt", encoding="utf-8") as fh:
            for i, line in enumerate(fh):
                r = json.loads(line)
                if r.get("kind") != "compiler_test_case":
                    continue
                d = {"file": fn[:-len(".jsonl.gz")], "index": i, "class": r["class"], "method": r["method"], "call": r["call"]}
                d.update(flags(r))
                lines.append(json.dumps(d, sort_keys=False, separators=(",", ":")))
    return ("\n".join(lines) + "\n").encode()


def main():
    data = derive()
    if "--check" in sys.argv:
        with gzip.open(OUT, "rb") as fh:
            ok = fh.read() == data
        print("expected_pipeline", "fresh" if ok else "STALE")
        sys.exit(0 if ok else 1)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "wb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", mtime=0, filename="") as fh:
            fh.write(data)
    print("wrote", OUT, data.count(b"\n"), "records")


if __name__ == "__main__":
    main()
