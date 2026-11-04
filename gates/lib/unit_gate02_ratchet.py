#!/usr/bin/env python3
"""Gate 0.2 ratchet: the report phase of gates/gate_0_2.sh compares the state behind the strict
criteria (a)-(e) with the committed baseline gates/gate_0_2_ratchet.json. The strict verdict is not
changed; the ratchet says whether the corpus got worse than the baseline, and what got worse.

State (`state` below; the baseline holds the same sections):
  strictPassing                the criteria among (a)-(e) that pass
  a.unclassifiedMethods        post-call-assertion methods without a valid classification
                               (method key -> status)
  a.postStateUnclassified      records whose replay post state is unclassified (record key -> reason)
  a.excluded                   records excluded from (a): descriptor-marked, post-call not captured,
                               no verifiable passTrace (record key -> reason)
  e.unrepresentable            records counted in (e) (record key -> kinds)
  d.failingUnits               (d) passes and groups that fail (label -> rate, counts, faults)
Record key `Class#method@call`, with `~n` appended when n >= 1 earlier records of the file have the
same method and call: the identity scripts/unit_ratchet_rebase.py maps records by, so it survives a
re-recording. Method key `Binary#method`: the record class's binary simple name (Outer$Inner) and
the test method without its parameters.

The ratchet fails on: results that do not belong to the current inputs (stamp); a failing check of
(a) or (d) that is never baselined (HARD); a criterion of strictPassing that fails; a key in one of the
identity sections that the baseline lacks; a (d) unit that fails without being in the baseline, or
fails in a way it did not (a new fault); a baselined unit below 90% whose rate drops by more than
`d_tolerance`. Keys and units of the baseline that no longer fail are listed as resolved (still in the
corpus) or gone (no longer recorded or measured); they are never failures.

`update` rewrites the baseline: by default it only shrinks (drops resolved and gone entries, raises
(d) rates that improved, adds criteria that now pass) and refuses while the ratchet fails; with grow
the baseline becomes the current state and every addition is listed.
"""
import json
import os

SCHEMA = "closure-rs/gate02-ratchet/1"
RECORD_KEY = ("Class#method@call: Class = record file stem under corpus/unit/records/, method and call = "
              "the record's fields; ~n is appended when n >= 1 earlier records of the file have the same "
              "method and call (the key scripts/unit_ratchet_rebase.py maps records by)")
METHOD_KEY = "Binary#method: the record class's binary simple name (Outer$Inner) and the test method without parameters"
# (section, subsection, what a key names, which corpus universe tells resolved from gone)
SETS = (("a", "unclassifiedMethods", "unclassified post-call-assertion method", "methods"),
        ("a", "postStateUnclassified", "record with unclassified post state", "records"),
        ("a", "excluded", "record excluded from (a)", "records"),
        ("e", "unrepresentable", "record counted in (e)", "records"))
# (d) rate tolerance for a baselined unit below 90%: half a percentage point, or one change-expecting
# record where one record is worth more (units under 200 change-expecting records). Mutation replays
# are deterministic, so unchanged records reproduce a rate exactly; it moves only when upstream adds or
# changes records of the unit. The tolerance absorbs one new surviving record in a small unit; a
# mutation that stops reaching a pass moves its rate by far more. `update` never lowers a baselined
# rate without grow, so drops within the tolerance do not add up over syncs.
D_TOLERANCE_PP = 0.005
EPS = 1e-6


TOLERANCE_TEXT = f"max({D_TOLERANCE_PP * 100:g} percentage points, one change-expecting record)"


def d_tolerance(change_expecting):
    return max(D_TOLERANCE_PP, 1.0 / change_expecting) if change_expecting else D_TOLERANCE_PP


def record_key(cls, method, call, n):
    return f"{cls}#{method}@{call}" + (f"~{n}" if n else "")


def method_key(record_class, method):
    return f"{record_class.rsplit('.', 1)[-1]}#{method.split('[')[0]}"


def d_faults(row):
    """The (d) conditions a unit row of the strict report fails (empty = the unit passes)."""
    if row["ok"]:
        return []
    f = []
    if row["rate"] < 0.90:
        f.append("below90")
    if not row["agentRewroteEntryPoints"]:
        f.append("noEntryPointRewritten")
    if row["agentErrors"]:
        f.append("agentErrors")
    if row["agentLoadedPassClassesNotRewritten"]:
        f.append("loadedPassClassNotRewritten")
    return f


def d_entry(row):
    return {"kind": row["kind"], "rate": round(row["rate"], 6), "changeExpecting": row["changeExpectingRecords"],
            "failed": row["changeExpectingFailed"], "faults": d_faults(row)}


def empty_state():
    return {"strictPassing": [], "a": {"unclassifiedMethods": {}, "postStateUnclassified": {}, "excluded": {}},
            "d": {"failingUnits": {}}, "e": {"unrepresentable": {}}}


def load(path):
    """The baseline at path, or None when there is none. Raises ValueError for a malformed file."""
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as f:
        b = json.load(f)
    if b.get("schema") != SCHEMA:
        raise ValueError(f"{path}: schema {b.get('schema')!r}, expected {SCHEMA!r}")
    for sec, sub, _, _ in SETS + (("d", "failingUnits", None, None),):
        if not isinstance((b.get(sec) or {}).get(sub), dict):
            raise ValueError(f"{path}: {sec}.{sub} must be an object")
    if not isinstance(b.get("strictPassing"), list):
        raise ValueError(f"{path}: strictPassing must be a list")
    return b


def check(base, cur, hard, stale, corpus):
    """base, cur: states; hard: [(name, ok, detail)]; stale: the phase results do not belong to the
    current inputs; corpus: {"records": record keys, "methods": method keys, "dUnits": measured (d) unit
    labels} of the current corpus. Returns {holds, failures [{kind, key, detail}], resolved, gone, notes}."""
    fails, resolved, gone, notes = [], [], [], []

    def fail(kind, key, detail):
        fails.append({"kind": kind, "key": key, "detail": detail})

    if stale:
        fail("stale", "stamp", "the phase results do not belong to the current inputs (stamp changed since prep): "
                               "re-run the phases")
    for name, ok, detail in hard:
        if not ok:
            fail("hard", name, detail)
    for k in base["strictPassing"]:
        if k not in cur["strictPassing"]:
            fail("criterion", f"({k})", f"criterion ({k}) passes in the baseline and fails now")
    for k in cur["strictPassing"]:
        if k not in base["strictPassing"]:
            notes.append(f"criterion ({k}) passes now; the baseline does not require it yet")
    for sec, sub, what, universe in SETS:
        b, c = base[sec][sub], cur[sec][sub]
        for key in sorted(set(c) - set(b)):
            fail(sub, key, f"{what} not in the baseline: {key} ({c[key]})")
        for key in sorted(set(b) - set(c)):
            (resolved if key in corpus[universe] else gone).append(f"{sub} {key}")
    bd, cd = base["d"]["failingUnits"], cur["d"]["failingUnits"]
    for label, u in sorted(cd.items()):
        rate = f"{u['rate']:.1%} of {u['changeExpecting']} change-expecting records"
        if label not in bd:
            fail("dUnit", label, f"(d) {u['kind']} {label} fails and is not in the baseline: {', '.join(u['faults'])}, {rate}")
            continue
        old = bd[label]
        new_faults = [f for f in u["faults"] if f not in old["faults"]]
        if new_faults:
            fail("dUnit", label, f"(d) {u['kind']} {label}: new fault {', '.join(new_faults)} (baseline "
                                 f"{', '.join(old['faults'])}), {rate}")
        if "below90" in u["faults"] and "below90" in old["faults"]:
            tol = d_tolerance(u["changeExpecting"])
            if u["rate"] < old["rate"] - tol - EPS:
                fail("dRate", label, f"(d) {u['kind']} {label}: rate {old['rate']:.2%} -> {u['rate']:.2%} of "
                                     f"{u['changeExpecting']} change-expecting records, more than the tolerance "
                                     f"{tol:.2%} below the baseline")
            elif u["rate"] < old["rate"] - EPS:
                notes.append(f"(d) {label}: rate {old['rate']:.2%} -> {u['rate']:.2%}, within the tolerance {tol:.2%}")
            elif u["rate"] > old["rate"] + EPS:
                notes.append(f"(d) {label}: rate {old['rate']:.2%} -> {u['rate']:.2%} (improved)")
    for label in sorted(set(bd) - set(cd)):
        (resolved if label in corpus["dUnits"] else gone).append(f"failingUnits {label}")
    return {"holds": not fails, "failures": fails, "resolved": resolved, "gone": gone, "notes": notes}


def update(base, cur, result, grow):
    """The new baseline and the list of changes (lines starting + added, - removed, ~ changed), or
    (None, reasons) when the update is refused. result = check(base or empty_state(), cur, ...)."""
    blocking = [f for f in result["failures"] if f["kind"] in ("stale", "hard")]
    if blocking:
        return None, [f"{f['kind']} {f['key']}: {f['detail']}" for f in blocking] + [
            "these are never baselined: fix them (or re-run the phases) first"]
    if result["failures"] and not grow:
        return None, [f["detail"] for f in result["failures"]] + [
            "the ratchet fails: fix these, or accept them into the baseline with --grow"]
    base = base or empty_state()
    new = empty_state()
    changes = []
    if grow:
        new["strictPassing"] = sorted(cur["strictPassing"])
    else:
        new["strictPassing"] = sorted(set(base["strictPassing"]) | set(cur["strictPassing"]))
    for k in sorted(set(new["strictPassing"]) - set(base["strictPassing"])):
        changes.append(f"+ strictPassing ({k})")
    for k in sorted(set(base["strictPassing"]) - set(new["strictPassing"])):
        changes.append(f"- strictPassing ({k}): no longer required to pass")
    for sec, sub, _, _ in SETS:
        b, c = base[sec][sub], cur[sec][sub]
        keep = set(c) if grow else set(c) & set(b)
        new[sec][sub] = {k: c[k] for k in sorted(keep)}
        changes += [f"+ {sub} {k} ({c[k]})" for k in sorted(keep - set(b))]
        changes += [f"- {sub} {k}" for k in sorted(set(b) - keep)]
    bd, cd = base["d"]["failingUnits"], cur["d"]["failingUnits"]
    nd = {}
    for label in sorted(cd if grow else set(cd) & set(bd)):
        u = cd[label]
        old = bd.get(label)
        if old is None:
            nd[label] = u
            changes.append(f"+ failingUnits {label} ({', '.join(u['faults'])}, rate {u['rate']:.2%})")
        elif grow or u["rate"] >= old["rate"] - EPS:
            nd[label] = u
            if abs(u["rate"] - old["rate"]) > EPS or u["faults"] != old["faults"]:
                changes.append(f"~ failingUnits {label}: rate {old['rate']:.2%} -> {u['rate']:.2%}, "
                               f"faults {', '.join(old['faults'])} -> {', '.join(u['faults'])}")
        else:
            # A drop within the tolerance keeps the baselined rate (and the counts it came from).
            nd[label] = dict(old, faults=u["faults"])
            if u["faults"] != old["faults"]:
                changes.append(f"~ failingUnits {label}: faults {', '.join(old['faults'])} -> {', '.join(u['faults'])}")
    changes += [f"- failingUnits {k}" for k in sorted(set(bd) - set(nd))]
    new["d"]["failingUnits"] = nd
    return new, changes


def document(state, records, reference):
    """The baseline file's content for state."""
    a, e = state["a"], state["e"]
    return {"schema": SCHEMA, "about": "gates/gate_0_2.sh --ratchet (gates/lib/unit_gate02_ratchet.py)",
            "recordKey": RECORD_KEY, "methodKey": METHOD_KEY, "reference": reference,
            "dTolerance": f"a baselined (d) unit below 90% may lose at most {TOLERANCE_TEXT} of its rate",
            "summary": {"records": records, "unclassifiedMethods": len(a["unclassifiedMethods"]),
                        "postStateUnclassified": len(a["postStateUnclassified"]), "excluded": len(a["excluded"]),
                        "excludedFraction": round(len(a["excluded"]) / records, 6) if records else 0.0,
                        "unrepresentable": len(e["unrepresentable"]),
                        "unrepresentableFraction": round(len(e["unrepresentable"]) / records, 6) if records else 0.0,
                        "dFailingUnits": len(state["d"]["failingUnits"])},
            **state}


def write(path, doc):
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=1, sort_keys=True)
        f.write("\n")
    os.replace(tmp, path)
