#!/usr/bin/env python3
"""Prepare all D2 candidates and a deterministic, outcome-independent fixture.

Run prepare, then CompilerWhitespace with the request/reference paths, then fixture.
Case flags/profiles are intentionally replaced by the same three flags for every
case: WHITESPACE_ONLY, ECMASCRIPT_NEXT input, ECMASCRIPT_NEXT output. Inputs,
explicit externs and shims retain manifest order and their relative filenames.
No environment-provided externs are added to either Compiler API invocation.
"""
import argparse
import json
import re
from pathlib import Path


def read_rows(path):
    with path.open() as stream:
        for line in stream:
            yield json.loads(line)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['prepare', 'fixture'])
    parser.add_argument('project', type=Path)
    parser.add_argument('worktree', type=Path)
    args = parser.parse_args()
    cache = args.project / 'corpus-cache/compiler'
    cache.mkdir(parents=True, exist_ok=True)
    requests = cache / 'whitespace-requests.jsonl'
    reference = cache / 'whitespace-reference.jsonl'
    selection = cache / 'whitespace-selection.json'
    if args.action == 'prepare':
        selected = []
        count = 0
        with requests.open('w') as stream:
            for manifest in sorted((args.project / 'corpus/d2/candidates').glob('*.jsonl')):
                candidates = list(read_rows(manifest))
                sizes = {}
                for case in candidates:
                    row = {key: case[key] for key in ('id', 'source', 'license')}
                    for field, paths in [('inputs', case['inputs'] + case.get('shims', [])),
                                         ('externs', case.get('externs', []))]:
                        row[field] = [{'name': name, 'code': (args.project / name).read_bytes().decode('utf-8')} for name in paths]
                    # The small regression exercises individual-source families. Whole
                    # packages and goog.module cases are measured in the complete run.
                    # Keep the original fixture selection (goog.module cases excluded).
                    if manifest.stem != 'whole-program' and not any(
                            re.search(r'goog\s*\.\s*module\s*\(', source['code'])
                            for source in row['inputs']):
                        sizes[case['id']] = sum(len(source['code'].encode('utf-8'))
                                               for field in ('inputs', 'externs') for source in row[field])
                    stream.write(json.dumps(row, ensure_ascii=True, separators=(',', ':')) + '\n')
                    count += 1
                # Eight smallest eligible cases from each individual-source family.
                selected.extend(sorted(sizes, key=lambda name: (sizes[name], name))[:8])
        selection.write_text(json.dumps(selected, indent=2) + '\n')
        print(f'{count} candidates; {len(selected)} fixture cases')
    else:
        selected = set(json.loads(selection.read_text()))
        fixture = []
        for request, result in zip(read_rows(requests), read_rows(reference), strict=True):
            assert request['id'] == result['id']
            if request['id'] in selected:
                request['expected'] = {key: value for key, value in result.items() if key != 'id'}
                fixture.append(request)
        assert len(fixture) == len(selected)
        path = args.worktree / 'crates/jscomp/tests/data/compiler_whitespace.json'
        text = json.dumps(fixture, ensure_ascii=True, indent=2) + '\n'
        assert len(text.encode()) < 1_000_000
        path.write_text(text)
        print(f'{len(fixture)} fixture cases, {len(text.encode())} bytes')


if __name__ == '__main__':
    main()
