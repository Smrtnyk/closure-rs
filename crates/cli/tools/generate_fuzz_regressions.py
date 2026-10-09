#!/usr/bin/env python3
"""Capture the Java reference bytes for the CLI regression cases of differential-fuzzing fixes.

Each case is a directory tests/data/fuzz_regressions/<name>/ holding `case.json`
({"argv": [...], "origin": "..."}) and the input files the argv names (relative paths). This
script runs the pinned reference jar on it in the golden environment (oracle/PROTOCOL.md:
env -i PATH HOME LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC, stdin empty, cwd = a scratch copy of the
case) and writes expected/stdout, expected/stderr, expected/files/<every file the run created>
and the exit code into case.json. crates/cli/tests/fuzz_regression_test.rs replays the binary
the same way and compares byte for byte.

  python3 crates/cli/tools/generate_fuzz_regressions.py [name ...]   (default: every case)
"""
import json, os, shutil, subprocess, sys, tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
import paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout (jar and JDK live there)

ROOT = Path(paths.ROOT)
JAVA = ROOT / 'tools/jdk-21/bin/java'
JAR = Path(paths.REF_JAR)
CASES = Path(__file__).resolve().parents[1] / 'tests/data/fuzz_regressions'
ENV = {'PATH': '/usr/bin:/bin', 'HOME': os.environ.get('HOME', '/tmp'), 'LANG': 'C.UTF-8',
       'LC_ALL': 'C.UTF-8', 'TZ': 'UTC'}


def inputs(case: Path):
    return sorted(p for p in case.rglob('*') if p.is_file() and p.name != 'case.json'
                  and 'expected' not in p.relative_to(case).parts)


def capture(case: Path):
    meta = json.loads((case / 'case.json').read_text())
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        before = set()
        for p in inputs(case):
            dst = tmp / p.relative_to(case)
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(p, dst)
            before.add(dst.relative_to(tmp).as_posix())
        r = subprocess.run([str(JAVA), '-Xmx2g', '-jar', str(JAR), *meta['argv']], input=b'',
                           capture_output=True, cwd=tmp, env=ENV, timeout=300)
        exp = case / 'expected'
        shutil.rmtree(exp, ignore_errors=True)
        (exp / 'files').mkdir(parents=True)
        (exp / 'stdout').write_bytes(r.stdout)
        (exp / 'stderr').write_bytes(r.stderr)
        made = []
        for p in sorted(tmp.rglob('*')):
            rel = p.relative_to(tmp).as_posix()
            if p.is_file() and rel not in before:
                (exp / 'files' / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(p, exp / 'files' / rel)
                made.append(rel)
    meta['exit_code'] = r.returncode
    meta['outputs'] = made
    (case / 'case.json').write_text(json.dumps(meta, indent=1) + '\n')
    print(f'{case.name}: exit {r.returncode}, {len(made)} output file(s)')


def main():
    names = sys.argv[1:] or sorted(p.name for p in CASES.iterdir() if (p / 'case.json').exists())
    for n in names:
        capture(CASES / n)


if __name__ == '__main__':
    main()
