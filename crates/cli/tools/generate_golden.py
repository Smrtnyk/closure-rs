#!/usr/bin/env python3
"""Writes the CLI goldens (crates/cli/tests/data/cli_golden/) from the reference jar.

  python3 crates/cli/tools/generate_golden.py [--jobs N] [--work DIR] [--check]

The cases are argument vectors that end before any compilation (help texts, flag parse errors,
setup errors...): the standard cases of every flag of the reference's CommandLineRunner, read
from the jar (CliMetadata.java; a boolean flag gets `--flag false --help`, `--flag=false --help` and
`--flag strange.js --help`, an enum flag its first value and an invalid one...), and fixed cases.
Every case runs through the reference jar (scripts/paths.py: $CLOSURE_RS_REF, else the
registry's default) with the pinned JDK, in the fixture directory, with a fixed environment and an
empty stdin. cases.json lists them (argv, exit code and the files holding stdout and stderr,
named by their sha256); output files no case uses any more are deleted. The script then lists the
cases added and removed and the cases whose output changed. --check writes nothing and exits 1
when the fixture differs. The step cli-fixtures of scripts/upstream_sync.py runs it.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import reference_tools as rt  # noqa: E402

FIXTURE = rt.FIXTURES / "cli_golden"
OUTPUT_FILE = re.compile(r"^[0-9a-f]{20}\.(stdout|stderr)$")
# Inputs some cases read (flag files, an output wrapper), written into the fixture directory.
FILES = {'flags-valid.txt': '﻿--warning_level="VERBOSE" --jscomp_off="checkTypes" --help',
         'flags-quoted.txt': '--define="TEXT=hello world" --help',
         'flags-nested.txt': '--flagfile flags-valid.txt',
         'flags-invalid.txt': '--warning_level invalid',
         'at-valid.txt': '--warning_level\nverbose\n--help\n',
         'at-equals.txt': '--warning_level=verbose\r\n--help\r',
         'at-invalid.txt': '--warning_level=invalid\n',
         'wrapper.txt': 'before\n%output%\nafter'}
VALID = {'browserFeaturesetYear': '2018', 'compilationLevel': 'WHITESPACE_ONLY', 'instrumentForCoverageOption': 'NONE',
         'sourceMapLocationMapping': 'src|dest', 'sourceMapInputs': 'foo.js|foo.map', 'outputWrapper': '(%output%)',
         'outputWrapperFile': 'wrapper.txt', 'flagFiles': 'flags-valid.txt',
         'browserResolverPrefixReplacements': 'old=new', 'chunk': 'base:0', 'chunkWrapper': 'base:%s'}


def golden_cases(metadata: dict) -> list[list[str]]:
    """The argument vectors, in order: the standard cases of every flag of `metadata` (CliMetadata's
    dump), then the fixed ones."""
    cases = []

    def add(argv):
        if argv not in cases:
            cases.append(argv)

    add(['--help']); add(['--help_markdown']); add(['--version']); add(['--version', 'a.js']); add(['a.js', '--version'])
    for row in metadata['flags']:
        name = row['name']; field = row['field']; typ = row['type']; handler = row['handler']
        if handler == 'BooleanOptionHandler': value = 'false'
        elif 'enum' in row: value = row['enum'][0]
        elif typ in ['int', 'java.lang.Integer']: value = VALID.get(field, '1')
        elif 'java.util.Map' in typ: value = 'old=new'
        else: value = VALID.get(field, 'value')
        args = [name, value, '--help']
        if field == 'compilationLevel' and value == 'BUNDLE': args[0:0] = ['--js_output_file', 'bundle.js']
        add(args); add([name + '=' + value, '--help'])
        if handler == 'BooleanOptionHandler': add([name, 'strange.js', '--help'])
        elif 'enum' in row or typ in ['int', 'java.lang.Integer']:
            add([name, 'not-a-value']); add([name + '=not-a-value'])
        elif typ != "boolean": add([name])
        for alias in row['aliases']:
            add([alias, value, '--help']); add([alias + '=' + value, '--help'])
    for value in ['true', 'on', 'yes', '1', 'false', 'off', 'no', '0', 'TRUE', 'YES', 'False', 'OFF', 'maybe', '']:
        add(['--debug', value, '--help']); add(['--debug=' + value, '--help'])
    for value in ['simple', 'SIMPLE_OPTIMIZATIONS', 'Whitespace', 'whitespace_only', 'advanced', 'ADVANCED_OPTIMIZATIONS', 'TRANSPILE_ONLY']:
        add(['-O', value, '--help'])
    for argv in [['--no_such_option'], ['--help', '--unknown'], ['--unknown', '--version'], ['--'], ['-'],
                 ['--flagfile', 'missing-flags.txt'], ['--flagfile', 'flags-nested.txt'], ['--flagfile', 'flags-invalid.txt'],
                 ['--flagfile', 'flags-quoted.txt'], ['@at-valid.txt'], ['@at-equals.txt'], ['@at-invalid.txt'], ['@missing-at.txt'],
                 ['--output_wrapper', 'invalid'], ['--output_wrapper_file', 'missing-wrapper.txt'],
                 ['--output_wrapper', '%output%', '--isolation_mode', 'IIFE'], ['--compilation_level', 'BUNDLE'],
                 ['--compilation_level', 'ADVANCED', '--renaming', 'false'], ['--dependency_mode', 'PRUNE'],
                 ['--dependency_mode', 'NONE', '--entry_point', 'goog:main'], ['--dependency_mode', 'SORT_ONLY', '--entry_point', 'main'],
                 ['--source_map_input', 'bad'], ['--source_map_location_mapping', 'bad'],
                 ['--formatting', 'pretty-print', '--help'], ['--warning_level', 'vErBoSe', '--help'],
                 ['--debug=true', '--version'], ['--version', '--help'], ['--help', '--version'],
                 ['--warning_level="VERBOSE"', '--help'], ['--warning_level=\'QUIET"', '--help'],
                 ['--js=\'foo.js\'', '--help'], ['--js', '"foo.js"', '--help'],
                 ['--jscomp_error', '"checkTypes"', '--help'], ['--jscomp_warning=\'deprecated\'', '--help'],
                 ['--jscomp_warning', "'checkTypes'", '--help'], ['--warning_level VERBOSE', '--help'],
                 ['--remove_j2cl_asserts=false', '--help'], ['--version', '--logging_level', 'bogus'],
                 ['--browser_featureset_year', '2018', '--language_out', 'ECMASCRIPT_NEXT']]:
        add(argv)
    # Setup errors detected before any compiler work.
    for flag, values in {'language_in': ['bogus', 'UNSUPPORTED'], 'language_out': ['bogus', 'UNSUPPORTED'],
                         'j2cl_pass': ['bogus'], 'charset': ['bogus'], 'browser_featureset_year': ['2011', '2015', '2027'],
                         'chunk': ['foo', 'foo:-1', 'foo:bad', 'foo:1:dep:extra:fifth', 'foo:'],
                         'jscomp_error': ['not_a_group'], 'instrument_for_coverage_option': ['PRODUCTION']}.items():
        for value in values:
            add(['--' + flag, value])
    for value in ['bogus', 'warning', ' Warning ', '', '0x10', '2147483648', '-2147483649']:
        add(['--logging_level', value])
    for value in ['OFF', 'SEVERE', 'WARNING', 'INFO', 'CONFIG', 'FINE', 'FINER', 'FINEST', 'ALL', '123', '+123', '-123', '１２３']:
        add(['--version', '--logging_level', value])
    add(['--logging_level', 'bogus', '--language_in', 'bogus'])
    add(['--instrument_mapping_report', 'map.txt'])
    add(['--instrument_for_coverage_option', 'PRODUCTION', '--instrument_mapping_report', 'map.txt'])
    add(['--chunk_output_type', 'ES_MODULES', '--rename_prefix_namespace', 'root'])
    add(['--chunk_output_type', 'ES_MODULES', '--emit_use_strict'])
    add(['--chunk', 'a:0', '--chunk', 'b:auto'])
    add(['--js', 'fake.js', '--chunk', '1invalid:1'])
    add(['--js', 'fake.js', '--chunk', 'a:1:missing'])
    add(['--js', 'fake.js', '--chunk', 'a:0'])
    add(['--js', 'fake.js', '--chunk', 'a:2'])
    add(['--js', 'fake.js', '--chunk', 'a:1', '--chunk', 'a:0'])
    # Ensure these inputs terminate before compilation. Bare '-' would reach compilation; cover it under help.
    cases.remove(['-']); add(['-', '--help'])
    return cases


def run(argv: list[str]) -> tuple[int, bytes, bytes]:
    r = subprocess.run([str(rt.JAVA), '-Xmx2g', '-jar', rt.REF_JAR, *argv], input=b'', capture_output=True,
                       cwd=FIXTURE, env=rt.child_env(), timeout=120)
    return r.returncode, r.stdout, r.stderr


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--jobs', type=int, default=4, help='JVMs at a time (default 4)')
    ap.add_argument('--work', type=Path, help='work directory (default build/cli-fixtures/<tag>/ of this checkout)')
    ap.add_argument('--check', action='store_true', help='write nothing; exit 1 when the fixture differs')
    a = ap.parse_args()
    work = a.work or rt.default_work()
    work.mkdir(parents=True, exist_ok=True)
    FIXTURE.mkdir(parents=True, exist_ok=True)
    changed = [name for name, text in FILES.items()
               if rt.write_if_changed(FIXTURE / name, text.encode(), a.check)]
    cases = golden_cases(rt.flag_metadata(work))
    (work / 'golden-argv.json').write_text(json.dumps(cases, indent=2) + '\n')
    with concurrent.futures.ThreadPoolExecutor(a.jobs) as ex:
        results = list(ex.map(run, cases))
    old = json.loads((FIXTURE / 'cases.json').read_text()) if (FIXTURE / 'cases.json').exists() else []
    old_by_argv = {json.dumps(r['argv']): r for r in old}
    records, outputs, differ = [], {}, []
    for argv, (code, stdout, stderr) in zip(cases, results):
        streams = {}
        for name, value in (('stdout', stdout), ('stderr', stderr)):
            filename = hashlib.sha256(value).hexdigest()[:20] + '.' + name
            outputs[filename] = value
            streams[name] = filename
        record = {'argv': argv, 'exit_code': code, **streams}
        prev = old_by_argv.get(json.dumps(argv))
        if prev is not None and prev != record:
            differ.append(argv)
        records.append(record)
    for filename, value in outputs.items():
        if rt.write_if_changed(FIXTURE / filename, value, a.check):
            changed.append(filename)
    orphans = sorted(f.name for f in FIXTURE.iterdir() if OUTPUT_FILE.match(f.name) and f.name not in outputs)
    if not a.check:
        for name in orphans:
            (FIXTURE / name).unlink()
    if rt.write_if_changed(FIXTURE / 'cases.json', (json.dumps(records, indent=2) + '\n').encode(), a.check):
        changed.append('cases.json')
    added = [r['argv'] for r in records if json.dumps(r['argv']) not in old_by_argv]
    new_argv = {json.dumps(c) for c in cases}
    removed = [r['argv'] for r in old if json.dumps(r['argv']) not in new_argv]
    print(f'{FIXTURE.relative_to(rt.CHECKOUT)}: {len(records)} cases of {rt.REF_TAG}; {len(added)} added, '
          f'{len(removed)} removed, {len(differ)} with another output; {len(orphans)} unused output files')
    for what, rows in (('added', added), ('removed', removed), ('output changed', differ)):
        for argv in rows[:40]:
            print(f'  {what}: {json.dumps(argv)}')
        if len(rows) > 40:
            print(f'  ... and {len(rows) - 40} more {what}')
    if a.check:
        if changed or orphans:
            print(f'the fixture differs from {rt.REF_TAG} (python3 crates/cli/tools/generate_golden.py rewrites it)')
            return 1
        return 0
    return 0


if __name__ == '__main__':
    sys.exit(main())
