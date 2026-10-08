# Candidates (visible part only)

These are the raw collector outputs (docs/PORTING.md §4.4) with every holdout group removed
(docs/PORTING.md §4.5, DECISIONS D-008).

- `finalize.py` run on these files reproduces `../cases.jsonl` exactly.
- The collector generator scripts were moved out of the repository at the holdout split,
  because they enumerate every candidate, holdout included.
- The corpus is frozen: it is reproduced by `scripts/fetch_d2.sh`, which works from
  `../sources.lock.json`. The generators are not used for this.
