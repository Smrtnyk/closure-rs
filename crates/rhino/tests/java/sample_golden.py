"""Select CI records from the Java-generated full corpora, never from Rust output."""
import gzip
import re
import sys
from pathlib import Path
import sys as _sys
_sys.path.insert(0, str(Path(__file__).resolve().parents[4] / 'scripts'))
import paths as _paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout

CACHE = Path(_paths.ROOT) / 'corpus-cache/dtoa'
DEST = Path(__file__).resolve().parent.parent / 'data'
COUNTS = {'standard': 8000, 'modes': 3000, 'raw': 1000}


def output_units(field):
    if not field.startswith('='):
        return 0
    # Every escape is a single UTF-16 unit; all unescaped characters are ASCII.
    return len(re.sub(r'\\u[0-9a-f]{4}|\\\\', 'x', field[1:]))


def eligible(line):
    return output_units(line.rstrip('\n').split('\t')[-1 if kind != 'standard' else 1]) <= 2000


for kind in sys.argv[1:] or COUNTS:
    wanted = COUNTS[kind]
    pool = []
    with gzip.open(CACHE / f'{kind}-sample.tsv.gz', 'rt', encoding='ascii') as source:
        pool.extend(line for line in source if eligible(line))
    # Preserve the first examples of each exceptional/non-ASCII/surrogate outcome.
    forced = []
    seen = set()
    with gzip.open(CACHE / f'{kind}.tsv.gz', 'rt', encoding='ascii') as source:
        for line in source:
            field = line.rstrip('\n').split('\t')[-1 if kind != 'standard' else 1]
            if not eligible(line):
                continue
            if len(pool) < wanted:
                pool.append(line)
            tags = []
            if field.startswith('!'):
                tags.append(field)
            if field.startswith('?'):
                tags.append('infeasible')
            if '\\u' in field:
                tags.append('non_ascii')
                if re.search(r'\\u[dD][89aAbBcCdDeEfF][0-9a-f]{2}', field):
                    tags.append('surrogate')
            # Modes and raw retain each formatting mode / bias variant of rare outcomes.
            key = tuple(line.split('\t')[1:3]) if kind != 'standard' else ()
            for tag in tags:
                if (tag, key) not in seen:
                    forced.append(line)
                    seen.add((tag, key))
    pool.extend(forced)
    selected = [pool[i * len(pool) // (wanted - len(forced))]
                for i in range(wanted - len(forced))] + forced
    (DEST / f'{kind}.tsv').write_text(''.join(selected), encoding='ascii')
    print(f'{kind}: rows={len(selected)} bytes={sum(map(len, selected))} forced={len(forced)}')
