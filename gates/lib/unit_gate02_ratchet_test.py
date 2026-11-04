#!/usr/bin/env python3
"""Unit tests for the gate 0.2 ratchet (gates/lib/unit_gate02_ratchet.py).

  python3 gates/lib/unit_gate02_ratchet_test.py
"""

import copy
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import unit_gate02_ratchet as R  # noqa: E402

HARD_OK = [("replay failures", True, "")]


def unit(rate, n=100, faults=("below90",), kind="group"):
    return {"kind": kind, "rate": rate, "changeExpecting": n, "failed": round(rate * n), "faults": list(faults)}


def state():
    s = R.empty_state()
    s["strictPassing"] = ["b", "c"]
    s["a"]["unclassifiedMethods"] = {"InferJSDocInfoTest#testA": "explicit unclassified entry"}
    s["a"]["excluded"] = {"AstValidatorTest#testX@0": "descriptor-marked", "AstValidatorTest#testX@1": "descriptor-marked"}
    s["e"]["unrepresentable"] = {"AstValidatorTest#testX@0": "descriptor-marked"}
    s["d"]["failingUnits"] = {"{Normalize, TypeCheck}": unit(0.5, 1000), "Small": unit(0.5, 10, kind="pass")}
    return s


CORPUS = {"records": {"AstValidatorTest#testX@0", "AstValidatorTest#testX@1"},
          "methods": {"InferJSDocInfoTest#testA"}, "dUnits": {"{Normalize, TypeCheck}", "Small"}}


def kinds(res):
    return sorted((f["kind"], f["key"]) for f in res["failures"])


class KeyTest(unittest.TestCase):
    def test_record_key(self):
        self.assertEqual(R.record_key("C", "testA", 0, 0), "C#testA@0")
        self.assertEqual(R.record_key("C", "doTest[a #1]", 2, 1), "C#doTest[a #1]@2~1")

    def test_method_key(self):
        self.assertEqual(R.method_key("com.google.javascript.jscomp.FooTest$Inner", "testA"), "FooTest$Inner#testA")
        self.assertEqual(R.method_key("a.FooTest", "doTest[x #0]"), "FooTest#doTest")


class CheckTest(unittest.TestCase):
    def test_same_state_holds(self):
        res = R.check(state(), state(), HARD_OK, False, CORPUS)
        self.assertTrue(res["holds"])
        self.assertEqual((res["resolved"], res["gone"]), ([], []))

    def test_stale_and_hard(self):
        res = R.check(state(), state(), [("replay failures", False, "1 counted record fails")], True, CORPUS)
        self.assertEqual(kinds(res), [("hard", "replay failures"), ("stale", "stamp")])

    def test_criterion_lost(self):
        cur = state()
        cur["strictPassing"] = ["b"]
        self.assertEqual(kinds(R.check(state(), cur, HARD_OK, False, CORPUS)), [("criterion", "(c)")])

    def test_new_keys_fail(self):
        cur = state()
        cur["a"]["unclassifiedMethods"]["InferJSDocInfoTest#testNew"] = "no postcall entry"
        cur["a"]["excluded"]["X#t@0"] = "post-call not captured"
        cur["a"]["postStateUnclassified"]["X#t@0"] = "testFieldsAfter not compared"
        cur["e"]["unrepresentable"]["X#t@0"] = "post-call not captured"
        res = R.check(state(), cur, HARD_OK, False, CORPUS)
        self.assertEqual(kinds(res), [("excluded", "X#t@0"), ("postStateUnclassified", "X#t@0"),
                                      ("unclassifiedMethods", "InferJSDocInfoTest#testNew"),
                                      ("unrepresentable", "X#t@0")])
        self.assertIn("unclassified post-call-assertion method not in the baseline: InferJSDocInfoTest#testNew",
                      res["failures"][0]["detail"])

    def test_removed_keys_are_resolved_or_gone(self):
        cur = state()
        del cur["a"]["excluded"]["AstValidatorTest#testX@1"]  # still recorded: resolved
        del cur["a"]["unclassifiedMethods"]["InferJSDocInfoTest#testA"]
        corpus = dict(CORPUS, methods=set())  # the method is no longer recorded: gone
        res = R.check(state(), cur, HARD_OK, False, corpus)
        self.assertTrue(res["holds"])
        self.assertEqual(res["resolved"], ["excluded AstValidatorTest#testX@1"])
        self.assertEqual(res["gone"], ["unclassifiedMethods InferJSDocInfoTest#testA"])

    def test_d_new_unit_and_new_fault(self):
        cur = state()
        cur["d"]["failingUnits"]["{A, B}"] = unit(0.95, faults=("loadedPassClassNotRewritten",))
        cur["d"]["failingUnits"]["Small"] = unit(0.5, 10, faults=("below90", "agentErrors"), kind="pass")
        self.assertEqual(kinds(R.check(state(), cur, HARD_OK, False, CORPUS)), [("dUnit", "Small"), ("dUnit", "{A, B}")])

    def test_d_rate_tolerance(self):
        self.assertEqual(R.d_tolerance(1000), 0.005)
        self.assertEqual(R.d_tolerance(10), 0.1)
        cur = state()
        cur["d"]["failingUnits"]["{Normalize, TypeCheck}"] = unit(0.496, 1000)  # -0.4 pp: within
        cur["d"]["failingUnits"]["Small"] = unit(0.4, 10, kind="pass")  # one record of ten: within
        res = R.check(state(), cur, HARD_OK, False, CORPUS)
        self.assertTrue(res["holds"])
        self.assertEqual(len(res["notes"]), 2)
        cur["d"]["failingUnits"]["{Normalize, TypeCheck}"] = unit(0.494, 1000)  # -0.6 pp
        cur["d"]["failingUnits"]["Small"] = unit(0.3, 10, kind="pass")  # two records
        self.assertEqual(kinds(R.check(state(), cur, HARD_OK, False, CORPUS)),
                         [("dRate", "Small"), ("dRate", "{Normalize, TypeCheck}")])

    def test_d_unit_resolved_or_gone(self):
        cur = state()
        del cur["d"]["failingUnits"]["Small"]
        del cur["d"]["failingUnits"]["{Normalize, TypeCheck}"]
        res = R.check(state(), cur, HARD_OK, False, dict(CORPUS, dUnits={"Small"}))
        self.assertEqual((res["resolved"], res["gone"]), (["failingUnits Small"], ["failingUnits {Normalize, TypeCheck}"]))


class UpdateTest(unittest.TestCase):
    def test_shrink_only(self):
        base, cur = state(), state()
        del cur["a"]["excluded"]["AstValidatorTest#testX@1"]
        cur["strictPassing"] = ["b", "c", "e"]
        cur["d"]["failingUnits"]["{Normalize, TypeCheck}"] = unit(0.6, 1000)  # improved
        cur["d"]["failingUnits"]["Small"] = unit(0.4, 10, kind="pass")  # dropped within the tolerance
        res = R.check(base, cur, HARD_OK, False, CORPUS)
        new, changes = R.update(base, cur, res, grow=False)
        self.assertEqual(new["strictPassing"], ["b", "c", "e"])
        self.assertEqual(list(new["a"]["excluded"]), ["AstValidatorTest#testX@0"])
        self.assertEqual(new["d"]["failingUnits"]["{Normalize, TypeCheck}"]["rate"], 0.6)
        self.assertEqual(new["d"]["failingUnits"]["Small"]["rate"], 0.5)  # kept: never lowered without grow
        self.assertIn("- excluded AstValidatorTest#testX@1", changes)
        self.assertIn("+ strictPassing (e)", changes)

    def test_refused_while_failing_then_grow(self):
        base, cur = state(), state()
        cur["a"]["unclassifiedMethods"]["InferJSDocInfoTest#testNew"] = "no postcall entry"
        cur["strictPassing"] = ["b"]
        res = R.check(base, cur, HARD_OK, False, CORPUS)
        new, reasons = R.update(base, cur, res, grow=False)
        self.assertIsNone(new)
        self.assertTrue(any("--grow" in r for r in reasons))
        new, changes = R.update(base, cur, res, grow=True)
        self.assertIn("InferJSDocInfoTest#testNew", new["a"]["unclassifiedMethods"])
        self.assertEqual(new["strictPassing"], ["b"])
        self.assertIn("+ unclassifiedMethods InferJSDocInfoTest#testNew (no postcall entry)", changes)
        self.assertIn("- strictPassing (c): no longer required to pass", changes)

    def test_never_baselines_hard_or_stale(self):
        res = R.check(state(), state(), [("rule 6", False, "violated")], False, CORPUS)
        self.assertIsNone(R.update(state(), state(), res, grow=True)[0])
        res = R.check(state(), state(), HARD_OK, True, CORPUS)
        self.assertIsNone(R.update(state(), state(), res, grow=True)[0])

    def test_bootstrap_and_round_trip(self):
        cur = state()
        res = R.check(R.empty_state(), cur, HARD_OK, False, CORPUS)
        self.assertFalse(res["holds"])
        new, _ = R.update(None, cur, res, grow=True)
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "b.json")
            R.write(p, R.document(new, 25000, "v1"))
            b = R.load(p)
        self.assertEqual(b["summary"]["excluded"], 2)
        self.assertTrue(R.check(b, copy.deepcopy(cur), HARD_OK, False, CORPUS)["holds"])

    def test_load_rejects_other_schema(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "b.json")
            with open(p, "w") as f:
                f.write('{"schema": "x"}')
            with self.assertRaises(ValueError):
                R.load(p)
            self.assertIsNone(R.load(os.path.join(d, "missing.json")))


if __name__ == "__main__":
    unittest.main()
