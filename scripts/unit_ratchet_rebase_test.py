#!/usr/bin/env python3
"""Tests of scripts/unit_ratchet_rebase.py on small record fixtures: python3 scripts/unit_ratchet_rebase_test.py"""
import contextlib
import gzip
import io
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import unit_ratchet_rebase as rb  # noqa: E402


def write_records(d, cls, keys):
    os.makedirs(d, exist_ok=True)
    with gzip.open(os.path.join(d, f"{cls}.jsonl.gz"), "wt", encoding="utf-8") as f:
        for method, call in keys:
            f.write(json.dumps({"class": "x." + cls, "method": method, "call": call}) + "\n")


def write_ratchet(path, passing):
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"schema": "closure-rs/unit-ratchet/1", "passing": passing}, f)


class RebaseTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = self.tmp.name
        self.old, self.new = os.path.join(t, "old"), os.path.join(t, "new")
        # Old: A = [m1#0, m1#1, m2#0], B = [x#0, x#0 (a repeated key), y#0].
        write_records(self.old, "A", [("m1", 0), ("m1", 1), ("m2", 0)])
        write_records(self.old, "B", [("x", 0), ("x", 0), ("y", 0)])
        # New: A gains m0#0 in front (indexes shift), loses m2; B unchanged; C is new.
        write_records(self.new, "A", [("m0", 0), ("m1", 0), ("m1", 1)])
        write_records(self.new, "B", [("x", 0), ("x", 0), ("y", 0)])
        write_records(self.new, "C", [("z", 0)])

    def tearDown(self):
        self.tmp.cleanup()

    def test_mapping_lost_and_new(self):
        s = rb.rebase(["A[0]", "A[1]", "A[2]", "B[1]"], self.old, self.new)
        self.assertEqual(s["mapped"], 3)
        self.assertEqual(s["moved"], 2)  # A[0] -> A[1], A[1] -> A[2]; B[1] stays
        self.assertEqual([x["id"] for x in s["lost"]], ["A[2]"])
        self.assertEqual(s["lost"][0]["method"], "m2")
        self.assertEqual(s["new"], ["A[0]", "C[0]"])
        self.assertNotIn("regressed", s)

    def test_against_new_ratchet(self):
        s = rb.rebase(["A[0]", "A[1]", "B[1]"], self.old, self.new, ["A[1]", "B[1]", "C[0]"])
        self.assertEqual(s["regressed"], [{"id": "A[1]", "newId": "A[2]"}])
        self.assertEqual(s["gained"], ["C[0]"])
        self.assertEqual(s["stillPassing"], 2)

    def test_repeated_keys_pair_up_in_file_order(self):
        s = rb.rebase(["B[0]"], self.old, self.new, ["B[1]"])
        self.assertEqual(s["regressed"], [{"id": "B[0]", "newId": "B[0]"}])

    def test_cli_exit_codes(self):
        t = self.tmp.name
        old_r, new_r = os.path.join(t, "old.json"), os.path.join(t, "new.json")
        write_ratchet(old_r, ["A[0]", "A[1]"])
        write_ratchet(new_r, ["A[1]", "A[2]"])
        args = ["--old-ratchet", old_r, "--old-records", self.old, "--new-records", self.new]
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(rb.main(args + ["--new-ratchet", new_r]), 0)
        self.assertEqual(json.loads(out.getvalue())["stillPassing"], 2)
        write_ratchet(old_r, ["A[2]"])
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(rb.main(args), 1)  # A[2] (m2) is lost
            self.assertEqual(rb.main(args + ["--allow-lost"]), 0)
        write_ratchet(new_r, ["A[1]"])
        write_ratchet(old_r, ["A[1]"])  # old A[1] = m1#1 = new A[2], which no longer passes
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(rb.main(args + ["--new-ratchet", new_r]), 1)


if __name__ == "__main__":
    unittest.main()
