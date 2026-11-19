#!/usr/bin/env python3
"""Summarizes the log of a `cargo test --no-fail-fast` run: every failing test with its panic
message, the build errors, and the totals.

  python3 gates/lib/cargo_test_summary.py LOG [--lines N]

gates/ci.sh runs `cargo test` (when cargo-nextest is not installed) with --no-fail-fast, so one run
reports every failing test binary instead of stopping at the first, writes the whole output to a
log and prints this summary of it. Per failing test it prints the test binary (cargo's
"rerun" argument), the test's name and its panic message (the first N lines from "panicked at",
default 12), or the end of its output when it did not panic. A build error is printed as rustc
reported it. The last line gives the totals. The exit status is 0 when the log shows no failing
test and no build error, 1 otherwise.
"""
from __future__ import annotations

import argparse
import re
import sys

RUNNING = re.compile(r"^\s+Running (\S+)")
DOCTESTS = re.compile(r"^\s+Doc-tests (\S+)")
RESULT = re.compile(r"^test result: (\w+)\. (\d+) passed; (\d+) failed")
BLOCK = re.compile(r"^---- (.+) stdout ----$")
RERUN = re.compile(r"^error: (?:test|doctest) failed, to rerun pass `(.+)`$")


def failure_text(name: str, block: list[str], limit: int) -> list[str]:
    """The panic message of one test's captured output: the panic of the test's own thread (the
    output also holds the panics of threads the test caught), else the last one; or the end of the
    output when nothing panicked."""
    panics = [i for i, line in enumerate(block) if " panicked at " in line]
    if panics:
        own = [i for i in panics if block[i].startswith(f"thread '{name}' ")]
        i = (own or panics)[-1]
        out = []
        for msg in block[i:]:
            if msg.startswith("note: run with `RUST_BACKTRACE") or (out and " panicked at " in msg):
                break
            out.append(msg)
        while out and not out[-1].strip():
            out.pop()
        return out[:limit] + ([f"... ({len(out) - limit} more lines in the log)"] if len(out) > limit else [])
    tail = [line for line in block if line.strip()]
    return tail[-limit:]


def summarize(lines: list[str], limit: int) -> tuple[list[str], int, int, int, int]:
    """(report lines, targets, passed, failed tests, build errors)."""
    out, targets, passed, failed, build_errors = [], 0, 0, 0, 0
    target = None
    pending = []  # (test name, failure text) of the current target, printed at its rerun line
    block = None  # (name, lines) of the "---- name stdout ----" section being read
    in_failures = False
    error_block = None  # a rustc error being copied
    for line in lines:
        if error_block is not None:
            if line.strip() == "":
                out.extend(error_block)
                out.append("")
                error_block = None
            else:
                error_block.append(line)
            continue
        m = RUNNING.match(line) or DOCTESTS.match(line)
        if m:
            target, in_failures, block = m.group(1), False, None
            targets += 1
            continue
        if line.startswith("error: could not compile"):
            out.extend([line, ""])
            continue
        if line.startswith("error[") or (line.startswith("error: ") and not RERUN.match(line)
                                         and not re.match(r"^error: \d+ targets? failed:", line)):
            build_errors += 1
            error_block = [line]
            continue
        if line == "failures:":
            if block:
                pending.append((block[0], failure_text(block[0], block[1], limit)))
                block = None
            in_failures = True
            continue
        m = BLOCK.match(line)
        if m and in_failures:
            if block:
                pending.append((block[0], failure_text(block[0], block[1], limit)))
            block = (m.group(1), [])
            continue
        if block is not None:
            block[1].append(line)
            continue
        m = RESULT.match(line)
        if m:
            passed += int(m.group(2))
            failed += int(m.group(3))
            in_failures = False
            continue
        m = RERUN.match(line)
        if m:
            for name, text in pending:
                out.append(f"FAILED {m.group(1)}: {name}")
                out.extend("    " + t for t in text)
            pending = []
    if error_block:
        out.extend(error_block)
    for name, text in pending:  # a target whose rerun line is missing (an interrupted run)
        out.append(f"FAILED {target}: {name}")
        out.extend("    " + t for t in text)
    return out, targets, passed, failed, build_errors


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("log", help="the output of cargo test (stdout and stderr)")
    ap.add_argument("--lines", type=int, default=12, help="lines of each failure message (default 12)")
    a = ap.parse_args()
    with open(a.log, encoding="utf-8", errors="replace") as f:
        lines = f.read().splitlines()
    out, targets, passed, failed, build_errors = summarize(lines, a.lines)
    for line in out:
        print(line)
    total = f"cargo test: {targets} test targets, {passed} tests passed, {failed} failed"
    if build_errors:
        total += f", {build_errors} build errors"
    print(total)
    return 0 if not failed and not build_errors else 1


if __name__ == "__main__":
    sys.exit(main())
