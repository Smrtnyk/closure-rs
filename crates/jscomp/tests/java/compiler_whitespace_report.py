#!/usr/bin/env python3
"""Report every candidate, retaining raw Java WTF-16 and UTF-8 diagnostic comparisons."""
import argparse
from collections import Counter
import json
from pathlib import Path


def rows(path):
    with path.open() as stream:
        yield from map(json.loads, stream)


def utf8_boundary(value):
    if isinstance(value, dict):
        if set(value) == {'utf16'}:
            units = value['utf16']
            raw = b''.join(unit.to_bytes(2, 'little') for unit in units)
            return raw.decode('utf-16-le', errors='surrogatepass').encode('utf-8', errors='replace').decode()
        return {key: utf8_boundary(item) for key, item in value.items()}
    if isinstance(value, list):
        return [utf8_boundary(item) for item in value]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('cache', type=Path)
    args = parser.parse_args()
    reference = list(rows(args.cache / 'whitespace-reference.jsonl'))
    actual = list(rows(args.cache / 'whitespace-rust.jsonl'))
    summary = Counter()
    mismatches = []
    for expected, found in zip(reference, actual, strict=True):
        assert expected['id'] == found['id']
        summary['candidates'] += 1
        summary['raw_exact'] += expected == found
        summary['utf8_exact'] += utf8_boundary(expected) == utf8_boundary(found)
        summary['source_exact'] += expected.get('output') == found.get('output')
        if expected == found:
            continue
        classes = []
        if expected.get('output') != found.get('output'):
            if isinstance(expected.get('output'), str) and 'goog.loadModule(function(exports)' in expected['output']:
                classes.append('unported WhitespaceWrapGoogModules')
            else:
                classes.append('unclassified source difference')
        if expected['warnings'] != found['warnings']:
            extra = [warning for warning in found['warnings'] if warning not in expected['warnings']]
            missing = [warning for warning in expected['warnings'] if warning not in found['warnings']]
            if (extra and not missing
                    and any(warning['type'] == 'JSC_TYPE_PARSE_ERROR' for warning in extra)
                    and all(warning['type'] in ('JSC_TYPE_PARSE_ERROR', 'SOURCEMAP_RESOLVE_FAILED')
                            for warning in extra)):
                # Formatting a spurious type warning also resolves its input map.
                if any(warning['type'] == 'SOURCEMAP_RESOLVE_FAILED' for warning in extra):
                    summary['type_warning_source_map_followup_cases'] += 1
                classes.append('unported CHECK_TYPES diagnostic members')
            else:
                classes.append('unclassified warning difference')
        if expected['errors'] != found['errors']:
            if utf8_boundary(expected['errors']) == utf8_boundary(found['errors']):
                classes.append('diagnostics String cannot retain lone UTF-16 surrogates')
            else:
                classes.append('unclassified error difference')
        if expected.get('exception') != found.get('exception'):
            classes.append('unclassified exception difference')
        for category in classes:
            summary[category] += 1
        mismatches.append({'id': expected['id'], 'classes': classes,
                           'fields': sorted(key for key in expected.keys() | found.keys()
                                            if expected.get(key) != found.get(key))})
    summary = dict(summary)
    summary['raw_percent'] = 100 * summary['raw_exact'] / summary['candidates']
    summary['utf8_percent'] = 100 * summary['utf8_exact'] / summary['candidates']
    (args.cache / 'whitespace-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    (args.cache / 'whitespace-mismatches.json').write_text(json.dumps(mismatches, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
