#!/usr/bin/env python3
"""closure-rs: check that the binary of every platform writes the same bytes.

  python3 scripts/platform_outputs.py run --bin BIN --args DIR [--out DIR]
  python3 scripts/platform_outputs.py compare DIR DIR...

run: compiles a fixed set of inputs with BIN, from the repository root and with relative paths
(forward slashes on every system), and writes everything each compile produces to --out
(default build/platform-outputs/out): <name>.js and its source map <name>.js.map, and the
compiler's stdout, stderr and exit code (<name>.stdout, <name>.stderr, <name>.exit). The
compiles are a small smoke input (also with --json_streams), and every DIR/*.args file (one
compiler flag per line; `scripts/run_bench.py --job REGEX --write-args DIR` writes them for the
benchmark projects of bench/projects.json), its --js_output_file replaced, with a few extra
flag variants. The output folder has the same relative path on every platform because source
maps and diagnostics name it. Exit code 1 when a compile does not exit 0 or DIR has no .args file.

compare: byte-compares the files of every DIR with those of the first one (each DIR is one
platform's --out folder) and exits 1 on any missing, extra or different file. A difference is a
porting bug: the compiler's output must not depend on the system it runs on.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

SMOKE = ('function greet(name){var unused=1;return "Hello, "+name}\n'
         'window["greet"]=greet;\nconsole.log(greet("world"));\n')

# Extra compiles: (name of an .args file without extension, suffix, extra flags).
# scripts/run_bench.py --variants also compiles these (and the --json_streams compile below) with
# Java and compares the results (.github/workflows/java-comparison.yml).
VARIANTS = [
    ('lodash-es-ADVANCED', 'es5', ['--language_out=ECMASCRIPT5']),
    ('lodash-es-SIMPLE', 'pretty', ['--formatting=PRETTY_PRINT', '--source_map_include_content']),
    ('three-ADVANCED', 'es5', ['--language_out=ECMASCRIPT5', '--isolation_mode=IIFE']),
]


def rel(path):
    return os.path.relpath(path, ROOT).replace(os.sep, '/')


def compile_one(binary, out, name, flags, stdin=None):
    """Runs one compile, writes its stdout, stderr and exit code to `out`; returns the exit code.
    The flags go in a --flagfile (an input list can exceed the length of a Windows command line)."""
    work = os.path.join(ROOT, 'build', 'platform-outputs', 'flags')
    os.makedirs(work, exist_ok=True)
    flagfile = os.path.join(work, f'{name}.args')
    with open(flagfile, 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(flags) + '\n')
    p = subprocess.run([binary, f'--flagfile={rel(flagfile)}'], cwd=ROOT, capture_output=True,
                       input=stdin if stdin is not None else b'')
    for ext, data in (('stdout', p.stdout), ('stderr', p.stderr),
                      ('exit', f'{p.returncode}\n'.encode())):
        with open(os.path.join(out, f'{name}.{ext}'), 'wb') as f:
            f.write(data)
    size = sum(os.path.getsize(os.path.join(out, f)) for f in os.listdir(out)
               if f.startswith(f'{name}.'))
    print(f'{name}: exit {p.returncode}, {size} bytes', flush=True)
    return p.returncode


def run(a):
    binary = os.path.abspath(a.bin)
    out = os.path.abspath(a.out)
    args_files = sorted(f for f in os.listdir(a.args) if f.endswith('.args')) \
        if os.path.isdir(a.args) else []
    if not args_files:
        sys.exit(f'platform_outputs.py: no .args file in {a.args}')
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    inputs = os.path.join(ROOT, 'build', 'platform-outputs', 'in')
    os.makedirs(inputs, exist_ok=True)
    smoke = os.path.join(inputs, 'smoke.js')
    with open(smoke, 'w', encoding='utf-8', newline='\n') as f:
        f.write(SMOKE)

    def to_files(name):
        js = f'{rel(out)}/{name}.js'
        return [f'--js_output_file={js}', f'--create_source_map={js}.map']

    # (name, flags, stdin)
    jobs = [('smoke-ADVANCED', ['--compilation_level=ADVANCED', f'--js={rel(smoke)}',
                                *to_files('smoke-ADVANCED')], None),
            # --json_streams=BOTH: the inputs come as JSON on stdin, the output and its source map
            # go as JSON to stdout
            ('smoke-json-streams', ['--compilation_level=ADVANCED', '--json_streams=BOTH',
                                    '--create_source_map=%outname%.map'],
             json.dumps([{'path': rel(smoke), 'src': SMOKE}]).encode())]
    for f in args_files:
        name = f[:-len('.args')]
        with open(os.path.join(a.args, f), encoding='utf-8') as fh:
            flags = [line for line in fh.read().splitlines()
                     if line and not line.startswith('--js_output_file=')]
        jobs.append((name, flags + to_files(name), None))
        jobs += [(f'{name}-{suffix}', flags + extra + to_files(f'{name}-{suffix}'), None)
                 for base, suffix, extra in VARIANTS if base == name]
    failed = [name for name, flags, stdin in jobs if compile_one(binary, out, name, flags, stdin)]
    print(f'{len(jobs)} compiles, outputs in {rel(out)}/')
    if failed:
        sys.exit(f'platform_outputs.py: compiles that did not exit 0: {", ".join(failed)}')


def files_of(d):
    found = {}
    for dirpath, _, names in os.walk(d):
        for n in names:
            path = os.path.join(dirpath, n)
            found[os.path.relpath(path, d).replace(os.sep, '/')] = path
    return found


def first_difference(a, b):
    n = min(len(a), len(b))
    i = next((i for i in range(n) if a[i] != b[i]), n)
    line = a.count(b'\n', 0, i) + 1
    return (f'first difference at byte {i} (line {line}): '
            f'{a[max(0, i - 40):i + 40]!r} vs {b[max(0, i - 40):i + 40]!r}; '
            f'sizes {len(a)} and {len(b)}')


def compare(a):
    dirs = a.dirs
    base_name, base = dirs[0], files_of(dirs[0])
    if not base:
        sys.exit(f'platform_outputs.py: {base_name} is empty')
    problems = []
    for d in dirs[1:]:
        other = files_of(d)
        for f in sorted(base.keys() - other.keys()):
            problems.append(f'{f}: in {base_name}, missing in {d}')
        for f in sorted(other.keys() - base.keys()):
            problems.append(f'{f}: in {d}, missing in {base_name}')
        for f in sorted(base.keys() & other.keys()):
            with open(base[f], 'rb') as x, open(other[f], 'rb') as y:
                bx, by = x.read(), y.read()
            if bx != by:
                problems.append(f'{f}: {base_name} and {d} differ; {first_difference(bx, by)}')
    total = sum(os.path.getsize(p) for p in base.values())
    print(f'{len(base)} files ({total} bytes) compared across {len(dirs)} platforms: '
          + ', '.join(dirs))
    if problems:
        print('\n'.join(problems))
        sys.exit(f'platform_outputs.py: {len(problems)} differences between platforms')
    print('identical on every platform')


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='cmd', required=True)
    r = sub.add_parser('run')
    r.add_argument('--bin', required=True)
    r.add_argument('--args', required=True)
    r.add_argument('--out', default=os.path.join(ROOT, 'build', 'platform-outputs', 'out'))
    c = sub.add_parser('compare')
    c.add_argument('dirs', nargs='+')
    a = ap.parse_args()
    if a.cmd == 'run':
        run(a)
    else:
        if len(a.dirs) < 2:
            sys.exit('platform_outputs.py: compare needs the outputs of at least two platforms')
        compare(a)


if __name__ == '__main__':
    main()
