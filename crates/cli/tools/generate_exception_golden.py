#!/usr/bin/env python3
"""Record CLI-owned runtime exception boundaries against the pinned JDK/reference jar."""
import json
import os
from pathlib import Path
import sys as _sys
_sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
import paths as _paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout
import subprocess

DATA = Path(_paths.ROOT)
# The pinned JDK: the goldens hold JDK stack frames, whose line numbers differ between JDK builds.
JAVA = DATA / 'tools/jdk-21/bin/java'
# One directory per reference, so that recording a new reference leaves the current one's alone.
CACHE = DATA / 'corpus-cache/cli/r2-errors' / _paths.REF_TAG
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
CACHE.mkdir(parents=True, exist_ok=True)
malformed_map = CACHE / 'malformed-map.txt'
malformed_map.write_text('bad-map-line\n')
CASES.append(('variable-map-malformed', ['--variable_map_input_file=' + str(malformed_map)], ''))
records = []
for name, args, stdin in CASES:
    result = subprocess.run([str(JAVA), '-Xmx2g', '-XX:-OmitStackTraceInFastThrow', '-jar',
                             _paths.REF_JAR, *args],
                            input=stdin.encode(), capture_output=True, check=False,
                            env={'PATH': '/usr/bin:/bin', 'HOME': os.environ.get('HOME', '/tmp'), 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8', 'TZ': 'UTC'})
    records.append(dict(name=name, args=args, stdin=stdin, exit=result.returncode,
                        stdout=result.stdout.decode(), stderr=result.stderr.decode()))
    if name == 'variable-map-malformed':
        records[-1]['files'] = {str(malformed_map): malformed_map.read_text()}
# The test creates `files` in a directory of its own and substitutes it for {tmp} everywhere.
records = json.loads(json.dumps(records).replace(json.dumps(str(CACHE))[1:-1], '{tmp}'))
output = CACHE / 'exception_golden.json'
output.write_text(json.dumps(records, indent=2) + '\n')
print(output)
