#!/usr/bin/env python3
"""Writes the exception goldens (crates/cli/tests/data/exception_golden.json) from the reference jar.

  python3 crates/cli/tools/generate_exception_golden.py [--check]

Records the CLI-owned runtime exceptions (an unwritable output, malformed JSON streams, missing
conformance, XTB and variable map files...) with their stack traces through the reference jar
(scripts/paths.py: $CLOSURE_RS_REF, else the registry's default) and the pinned JDK, whose frames
the goldens hold. The run's files live in corpus-cache/cli/r2-errors/<tag>/ of the main checkout;
its path becomes {tmp} in the fixture, which the test replaces with a directory of its own. Lists
the cases whose output changed; --check writes nothing and exits 1 when the fixture differs. The
step cli-fixtures of scripts/upstream_sync.py runs it.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import reference_tools as rt  # noqa: E402

FIXTURE = rt.FIXTURES / 'exception_golden.json'
# One directory per reference, so that recording a new reference leaves the current one's alone.
CACHE = rt.ROOT / 'corpus-cache/cli/r2-errors' / rt.REF_TAG
CASES = [
    ('output-directory', ['--compilation_level=WHITESPACE_ONLY', '--js_output_file=/tmp'], ''),
    ('output-missing', ['--compilation_level=WHITESPACE_ONLY', '--js_output_file=/dev/null/out.js'], ''),
    ('bad-json-start', ['--json_streams=IN'], '{}'),
    ('bad-json-array', ['--json_streams=IN'], '['),
    ('bad-json-object', ['--json_streams=IN'], '[1]'),
    ('bad-json-string', ['--json_streams=IN'], '[{"src":"oops}]'),
    ('bad-json-null', ['--json_streams=IN'], '[null]'),
    ('conformance-missing', ['--conformance_configs=/nonexistent-cli-config'], ''),
    ('xtb-missing', ['--translations_file=/nonexistent-cli-xtb'], ''),
    ('variable-map-missing', ['--variable_map_input_file=/nonexistent-cli-map'], ''),
]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--check', action='store_true', help='write nothing; exit 1 when the fixture differs')
    a = ap.parse_args()
    CACHE.mkdir(parents=True, exist_ok=True)
    malformed_map = CACHE / 'malformed-map.txt'
    malformed_map.write_text('bad-map-line\n')
    cases = CASES + [('variable-map-malformed', ['--variable_map_input_file=' + str(malformed_map)], '')]
    records = []
    for name, args, stdin in cases:
        result = subprocess.run([str(rt.JAVA), '-Xmx2g', '-XX:-OmitStackTraceInFastThrow', '-jar', rt.REF_JAR, *args],
                                input=stdin.encode(), capture_output=True, check=False, env=rt.child_env())
        records.append(dict(name=name, args=args, stdin=stdin, exit=result.returncode,
                            stdout=result.stdout.decode(), stderr=result.stderr.decode()))
        if name == 'variable-map-malformed':
            records[-1]['files'] = {str(malformed_map): malformed_map.read_text()}
    # The test creates `files` in a directory of its own and substitutes it for {tmp} everywhere.
    records = json.loads(json.dumps(records).replace(json.dumps(str(CACHE))[1:-1], '{tmp}'))
    text = (json.dumps(records, indent=2) + '\n').encode()
    (CACHE / 'exception_golden.json').write_bytes(text)
    old = {r['name']: r for r in json.loads(FIXTURE.read_text())} if FIXTURE.exists() else {}
    differ = [r['name'] for r in records if old.get(r['name']) != r]
    changed = rt.write_if_changed(FIXTURE, text, a.check)
    print(f'{FIXTURE.relative_to(rt.CHECKOUT)}: {len(records)} cases of {rt.REF_TAG}, '
          f'{len(differ)} with another output' + ''.join(f'\n  output changed: {n}' for n in differ))
    if a.check and changed:
        print(f'the fixture differs from {rt.REF_TAG} (python3 crates/cli/tools/generate_exception_golden.py '
              'rewrites it)')
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
