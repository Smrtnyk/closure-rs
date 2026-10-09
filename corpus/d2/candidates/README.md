# Candidates

These are the raw collector outputs (docs/PORTING.md §4.4), one JSONL file and one lock file per
source.

- `finalize.py` run on these files reproduces `../cases.jsonl` exactly.
- The collector generator scripts are not in the repository. The corpus is frozen: it is
  reproduced by `scripts/fetch_d2.sh`, which works from `../sources.lock.json`.
