#!/usr/bin/env python3
"""Writes the NoopAgent TRACE-mode scope file (D-017 item 8, HARNESS.md "Pass trace"): one FQCN per
line, every in-scope src top-level class (docs/PORTING.md §2 predicate `in_scope_src` of gates/lib/unit_gate02.py
over the pristine reference src/). The agent instruments only entry points (CompilerPass /
HotSwapCompilerPass / OptimizeCalls.CallGraphCompilerPass / peephole / NodeTraversal callback methods)
declared in these classes and their nested classes.
Usage: scripts/unit_trace_scope.py [OUT]  (default build/unit/trace/scope.txt)"""
import os, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "gates", "lib"))
import unit_gate02 as g  # noqa: E402

out = sys.argv[1] if len(sys.argv) > 1 else f"{g.ROOT}/build/unit/trace/scope.txt"
os.makedirs(os.path.dirname(out), exist_ok=True)
names = [c for c in g.src_top_classes() if g.in_scope_src(c)]
tmp = out + ".tmp"
with open(tmp, "w") as f:
    f.write("".join(n + "\n" for n in names))
os.replace(tmp, out)
print(f"{out}: {len(names)} in-scope src top-level classes")
