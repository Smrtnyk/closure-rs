#!/usr/bin/env python3
"""Unit tests for the pure logic of the D2 runner (gates/lib/d2_rust_core.py).

  python3 gates/lib/d2_rust_test.py
"""

import base64
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d2_rust_core as core  # noqa: E402


def golden(stdout="", stderr="", exit_code=0, outputs=None):
    return {"stdout": stdout, "stderr": stderr, "exit_code": exit_code, "outputs": outputs or {}}


class FirstDiffTest(unittest.TestCase):
    def test_identical(self):
        self.assertIsNone(core.first_diff(b"abc", b"abc"))

    def test_offset_line_col(self):
        d = core.first_diff(b"ab\ncdef\n", b"ab\ncdXf\n")
        self.assertEqual((d["offset"], d["line"], d["col"]), (5, 2, 3))
        self.assertEqual((d["expected_len"], d["actual_len"]), (8, 8))

    def test_prefix(self):
        d = core.first_diff(b"abc", b"abcdef")
        self.assertEqual(d["offset"], 3)
        d = core.first_diff(b"abcdef", b"")
        self.assertEqual((d["offset"], d["line"], d["col"]), (0, 1, 1))

    def test_context_escaped(self):
        d = core.first_diff(b"a\nb\xff", b"a\nc")
        self.assertIn("\\n", d["expected_context"])
        self.assertIn("\\xff", d["expected_context"])


class DecodeTest(unittest.TestCase):
    def test_str_and_base64(self):
        self.assertEqual(core.decode_text("hé"), "hé".encode("utf-8"))
        raw = b"\xed\xa0\x80x"
        self.assertEqual(core.decode_text({"base64": base64.b64encode(raw).decode()}), raw)


class ClassifyTest(unittest.TestCase):
    def cls(self, g, exit_code=0, stdout=b"", stderr=b"", outputs=None, **kw):
        outputs = outputs or {}
        diffs, facts = core.compare(g, exit_code, stdout, stderr, outputs)
        return core.classify(diffs, returncode=exit_code, stdout=stdout, stderr=stderr,
                             outputs=outputs, facts=facts, **kw)

    def test_pass(self):
        g = golden(outputs={"out.js": "x;\n"})
        self.assertEqual(self.cls(g, outputs={"out.js": b"x;\n"}), (None, None))

    def test_order(self):
        g = golden(stderr="a.js:1:0: ERROR - [JSC_FOO] m\n", exit_code=1)
        self.assertEqual(self.cls(g, harness_error="boom")[0], "harness_error")
        self.assertEqual(self.cls(g, spawn_error="ENOENT")[0], "spawn_error")
        self.assertEqual(self.cls(g, timed_out=True)[0], "timeout")
        self.assertEqual(self.cls(g, exit_code=-9), ("crash", "signal 9"))
        self.assertEqual(self.cls(g, exit_code=101, stderr=b"thread 'main' panicked at x")[0],
                         "crash")
        self.assertEqual(self.cls(g, exit_code=0)[0], "no_output")
        self.assertEqual(self.cls(g, exit_code=0, stdout=b"x"), ("exit_code", "1 -> 0"))
        self.assertEqual(self.cls(g, exit_code=1, stderr=b"a.js:1:0: ERROR - [JSC_BAR] m\n"),
                         ("stderr", "JSC_FOO | JSC_BAR"))
        self.assertEqual(self.cls(g, exit_code=1, stderr=g["stderr"].encode(), stdout=b"z"),
                         ("stdout", None))

    def test_files(self):
        g = golden(outputs={"out.js": "x;\n", "out.js.map": "{}"})
        self.assertEqual(self.cls(g, outputs={"out.js": b"x;\n"}), ("missing_file", "out.js.map"))
        self.assertEqual(self.cls(g, outputs={"out.js": b"x;\n", "out.js.map": b"{}", "y": b""}),
                         ("extra_file", "y"))
        self.assertEqual(self.cls(g, outputs={"out.js": b"y;\n", "out.js.map": b"{ }"}),
                         ("js_output", "out.js"))
        self.assertEqual(self.cls(g, outputs={"out.js": b"x;\n", "out.js.map": b"{ }"}),
                         ("sourcemap", "out.js.map"))

    def test_sourcemap_facts(self):
        g = golden(outputs={"out.js": "x;\n", "out.js.map": "{}"})
        _d, facts = core.compare(g, 0, b"", b"", {"out.js": b"y", "out.js.map": b"{}"})
        self.assertEqual((facts["maps_total"], facts["maps_match"]), (1, True))


def fake_pairs():
    pairs = []
    for src in ("npm", "test262", "closure-library"):
        for i in range(40):
            mod = src == "npm" and i % 4 == 0
            case = {"id": f"{src}-{i:03d}", "source": src,
                    "extra_flags": ["--module_resolution=NODE"] if mod else []}
            for prof in ("ws", "simple", "chunks2"):
                pairs.append({"case": case, "profile": prof, "source": src})
    return pairs


class SampleTest(unittest.TestCase):
    def test_deterministic_and_covering(self):
        pairs = fake_pairs()
        a = core.stratified_sample(pairs, 20, seed=5, min_per_group=2)
        b = core.stratified_sample(pairs, 20, seed=5, min_per_group=2)
        self.assertEqual([(p["case"]["id"], p["profile"]) for p in a],
                         [(p["case"]["id"], p["profile"]) for p in b])
        strata = {core.stratum(p) for p in pairs}
        got = {}
        for p in a:
            got[core.stratum(p)] = got.get(core.stratum(p), 0) + 1
        self.assertTrue(all(got.get(s, 0) >= 2 for s in strata))
        order = [pairs.index(p) for p in a]
        self.assertEqual(order, sorted(order))

    def test_filter_normalization(self):
        self.assertEqual(core.normalize_filter({"profiles": None, "limit": None}), {})
        self.assertEqual(core.normalize_filter({"profiles": ["b", "a", "a"]}),
                         {"profiles": ["a", "b"]})


def ratchet(passing, failing, flt=None, golden_tag="ref-x"):
    results = [{"case": c, "profile": p, "pass": True} for p, cs in passing.items() for c in cs]
    results += [{"case": c, "profile": p, "pass": False} for p, cs in failing.items() for c in cs]
    meta = {"golden": golden_tag, "corpus_sha256": "s", "complete": not flt, "filter": flt or {},
            "binary_sha256": None}
    return core.build_ratchet(results, ["ws", "simple"], meta)


class RatchetTest(unittest.TestCase):
    def test_counts(self):
        r = ratchet({"ws": ["a", "b"]}, {"ws": ["c"], "simple": ["a"]})
        self.assertEqual(r["counts"]["ws"], {"pass": 2, "total": 3})
        self.assertEqual(r["counts"]["simple"], {"pass": 0, "total": 1})
        self.assertEqual(r["total"], {"pass": 2, "total": 4})

    def test_same_and_improvement(self):
        base = ratchet({"ws": ["a"]}, {"ws": ["b"]})
        self.assertEqual(core.check_ratchet(base, base)[0], 0)
        self.assertEqual(core.check_ratchet(base, ratchet({"ws": ["a", "b"]}, {}))[0], 0)

    def test_pair_regression_with_same_count(self):
        base = ratchet({"ws": ["a"]}, {"ws": ["b"]})
        cur = ratchet({"ws": ["b"]}, {"ws": ["a"]})
        code, msgs = core.check_ratchet(base, cur)
        self.assertEqual(code, 1)
        self.assertTrue(any("a x ws" in m for m in msgs))

    def test_count_regression(self):
        base = ratchet({"ws": ["a", "b"]}, {})
        self.assertEqual(core.check_ratchet(base, ratchet({"ws": ["a"]}, {"ws": ["b"]}))[0], 1)

    def test_pair_gone_is_not_regression(self):
        base = ratchet({"ws": ["a", "b"]}, {})
        cur = ratchet({"ws": ["a", "c"]}, {})
        code, msgs = core.check_ratchet(base, cur)
        self.assertEqual(code, 0)
        self.assertTrue(any("no longer in the corpus" in m for m in msgs))

    def test_not_comparable(self):
        base = ratchet({"ws": ["a"]}, {})
        self.assertEqual(core.check_ratchet(base, ratchet({"ws": ["a"]}, {}, flt={"limit": 1}))[0], 2)
        self.assertEqual(core.check_ratchet(base, ratchet({"ws": ["a"]}, {}, golden_tag="ref-y"))[0], 2)


class RatchetRebaseTest(unittest.TestCase):
    def test_golden_change_allowed_and_pairs_kept(self):
        base = ratchet({"ws": ["a", "b"]}, {"ws": ["c"]})
        cur = ratchet({"ws": ["a", "b", "c"], "simple": ["d"]}, {}, golden_tag="ref-y")
        self.assertEqual(core.check_ratchet(base, cur)[0], 2)
        code, msgs = core.check_ratchet_rebase(base, cur)
        self.assertEqual(code, 0)
        self.assertTrue(any("golden ref-x -> ref-y" in m for m in msgs))
        self.assertTrue(any("2 newly passing (1 pairs new to the corpus)" in m for m in msgs))

    def test_lost_pair_fails_even_with_higher_count(self):
        base = ratchet({"ws": ["a"]}, {"ws": ["b", "c"]})
        cur = ratchet({"ws": ["b", "c"]}, {"ws": ["a"]}, golden_tag="ref-y")
        code, msgs = core.check_ratchet_rebase(base, cur)
        self.assertEqual(code, 1)
        self.assertIn("  a x ws", msgs)

    def test_same_case_other_profile_is_another_pair(self):
        base = ratchet({"ws": ["a"]}, {"simple": ["a"]})
        cur = ratchet({"simple": ["a"]}, {"ws": ["a"]}, golden_tag="ref-y")
        self.assertEqual(core.check_ratchet_rebase(base, cur)[0], 1)

    def test_gone_pair_is_listed_not_lost(self):
        base = ratchet({"ws": ["a", "b"]}, {})
        code, msgs = core.check_ratchet_rebase(base, ratchet({"ws": ["a"]}, {}, golden_tag="ref-y"))
        self.assertEqual(code, 0)
        self.assertTrue(any("no longer in the corpus" in m for m in msgs))

    def test_filter_change_not_comparable(self):
        base = ratchet({"ws": ["a"]}, {})
        cur = ratchet({"ws": ["a"]}, {}, flt={"limit": 1}, golden_tag="ref-y")
        self.assertEqual(core.check_ratchet_rebase(base, cur)[0], 2)


if __name__ == "__main__":
    unittest.main()
