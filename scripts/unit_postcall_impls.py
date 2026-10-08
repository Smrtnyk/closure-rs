#!/usr/bin/env python3
"""Language-neutral post-call-state equality (FORMAT.md "Post-call state: neutral equality").

Lists every container `impl` that occurs in any record's testFieldsAfter (and in the testFields
entries a descriptor checks through testFieldsAfterAlso) and classifies it as ORDERED or UNORDERED.
--check exits 1 when an impl occurs that the table does not classify, so the neutral equality rule
can never silently meet an unknown container.  Also offers neutral_equal(a, b) for a non-Java
harness to reuse as the reference implementation of the rule.
"""
import gzip, json, os, sys, collections

from paths import ROOT  # the main checkout (scripts/paths.py)
REC = f"{ROOT}/corpus/unit/records"

# Iteration order is insertion order, sequence order or sorted order: independent of hash codes.
ORDERED = {
    "java.util.ArrayList", "java.util.ArrayDeque", "java.util.LinkedHashSet", "java.util.LinkedHashMap",
    "java.util.TreeMap", "java.util.TreeSet", "java.util.Collections$EmptyList",
    "com.google.common.collect.ImmutableEnumSet",  # enum ordinal order
    "java.util.Collections$UnmodifiableRandomAccessList",
    "com.google.common.collect.RegularImmutableList", "com.google.common.collect.SingletonImmutableList",
    "com.google.common.collect.RegularImmutableSet", "com.google.common.collect.SingletonImmutableSet",
    "com.google.common.collect.RegularImmutableMap", "com.google.common.collect.RegularImmutableBiMap",
    "com.google.common.collect.SingletonImmutableBiMap",
    "com.google.common.collect.StandardTable$Row", "com.google.common.collect.StandardTable$RowMap",
    "com.google.common.collect.LinkedHashMultimap$ValueSet",
    "com.google.protobuf.LazyStringArrayList", "com.google.protobuf.ProtobufArrayList",
}
# Iteration order depends on Java hash codes (or on which multimap backs the view): compare unordered.
UNORDERED = {
    "java.util.HashMap", "java.util.HashSet",
    "com.google.common.collect.HashBiMap", "com.google.common.collect.HashBiMap$Inverse",
    "com.google.common.collect.AbstractMapBasedMultimap$WrappedSet",
    "com.google.common.collect.AbstractMapBasedMultimap$AsMap",
    # D-017 item 2 (FORMAT.md "Neutral encodings"): a Guava Table is {"table": [[row, column, value],
    # ...], "impl": ...}; HashBasedTable iterates rows and columns in hash order.
    "com.google.common.collect.HashBasedTable",
}
ORDERED |= {"com.google.common.collect.TreeBasedTable"}  # sorted rows and columns


def canon(v):
    return json.dumps(neutral(v), sort_keys=True, separators=(",", ":"))


def neutral(v):
    """Projection used by the neutral equality: drops `impl`, sorts unordered containers."""
    if isinstance(v, list):
        return [neutral(x) for x in v]
    if not isinstance(v, dict):
        return v
    impl = v.get("impl")
    if "list" in v:
        return {"list": [neutral(x) for x in v["list"]]}
    if "set" in v:
        items = [neutral(x) for x in v["set"]]
        if impl in UNORDERED:
            items = sorted(items, key=lambda x: json.dumps(x, sort_keys=True))
        return {"set": items}
    if "map" in v:
        ents = [[neutral(k), neutral(x)] for k, x in v["map"]]
        if impl in UNORDERED:
            ents = sorted(ents, key=lambda e: json.dumps(e[0], sort_keys=True))
        return {"map": ents}
    if "table" in v:
        cells = [[neutral(r), neutral(c), neutral(x)] for r, c, x in v["table"]]
        if impl in UNORDERED:
            cells = sorted(cells, key=lambda e: json.dumps(e[:2], sort_keys=True))
        return {"table": cells}
    return {k: neutral(x) for k, x in v.items()}


def neutral_equal(a, b):
    return canon(a) == canon(b)


def main():
    seen = collections.Counter()

    def walk(v):
        if isinstance(v, dict):
            if "impl" in v:
                seen[v["impl"]] += 1
            for x in v.values():
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)

    for fn in sorted(os.listdir(REC)):
        with gzip.open(os.path.join(REC, fn), "rt", encoding="utf-8") as fh:
            for line in fh:
                r = json.loads(line)
                if r.get("testFieldsAfter"):
                    walk(r["testFieldsAfter"])
                    walk(r.get("testFields"))
    unknown = sorted(k for k in seen if k not in ORDERED and k not in UNORDERED)
    for k, n in seen.most_common():
        print(f"{n:8d} {'ORDERED' if k in ORDERED else 'UNORDERED' if k in UNORDERED else 'UNKNOWN'} {k}")
    print("unknown impls:", unknown)
    if "--check" in sys.argv and unknown:
        sys.exit(1)


if __name__ == "__main__":
    main()
