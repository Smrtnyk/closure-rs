#!/usr/bin/env bash
# Summarises the unit recording: per-class entries vs records, kinds, failures, unrepresentable.
# Writes build/unit/report/{classes.json,table.md}. Usage: scripts/unit_report.sh
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, WT, SSD_WT
mkdir -p "$ROOT/build/unit/report"
python3 - "$ROOT" <<'PY'
import json, glob, gzip, os, sys, collections
root = sys.argv[1]
stats = os.path.join(root, 'build/unit/recording/stats')
recs = os.path.join(root, 'corpus/unit/records')
rows = []
for f in sorted(glob.glob(stats + '/*.json')):
    if f.endswith('.crash.json'):
        continue
    d = json.load(open(f))
    name = d['record']
    kinds = collections.Counter()
    unrep = 0
    p = os.path.join(recs, name + '.jsonl.gz')
    if d['records'] and os.path.exists(p):
        with gzip.open(p, 'rt', encoding='utf-8') as g:
            for line in g:
                r = json.loads(line)
                kinds[r['kind']] += 1
                unrep += 1 if r['unrepresentable'] else 0
    entries = sum(d['entries'].values())
    outer = sum(d['outermost'].values())
    rows.append(dict(cls=d['class'], name=name, run=d['runCount'], failed=d['failureCount'],
                     ignored=d['ignoreCount'], entries=entries, outermost=outer, records=d['records'],
                     unrep=unrep, kinds=dict(kinds),
                     failedTests=[k for k, v in d['tests'].items() if v.get('status') == 'failed']))
json.dump(rows, open(os.path.join(root, 'build/unit/report/classes.json'), 'w'), indent=1)
tot = collections.Counter()
for r in rows:
    for k in ('run', 'failed', 'ignored', 'entries', 'outermost', 'records', 'unrep'):
        tot[k] += r[k]
with open(os.path.join(root, 'build/unit/report/table.md'), 'w') as o:
    o.write('| Class | kind | tests run | failed | entries | outermost | records | with unrepresentable |\n|---|---|---:|---:|---:|---:|---:|---:|\n')
    for r in sorted(rows, key=lambda r: -r['records']):
        k = ','.join(sorted(r['kinds'])) or '-'
        o.write(f"| {r['name']} | {k} | {r['run']} | {r['failed']} | {r['entries']} | {r['outermost']} | {r['records']} | {r['unrep']} |\n")
print(json.dumps(dict(tot)), 'classes', len(rows), 'with records', sum(1 for r in rows if r['records']))
PY
