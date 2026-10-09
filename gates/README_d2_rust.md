# D2 differential runner and ratchet (`gates/d2_rust.py`, `gates/full.sh`)

A change is checked for two things (docs/PORTING.md §7): CI passes, and no previously passing D2
pair regresses. `gates/d2_rust.py` runs a Rust CLI on the visible D2 corpus, compares it with the
golden Java results byte for byte, and writes a report and `ratchet.json`. `gates/full.sh`
runs CI, then the D2 runner, then the unit-record runner hook.

## Running

```sh
python3 gates/d2_rust.py run --bin target/release/closure-rs                  # complete corpus
python3 gates/d2_rust.py run --bin BIN --profile ws --profile pretty          # some profiles
python3 gates/d2_rust.py run --bin BIN --source test262 --case-regex 'arrow' --keep-failing
python3 gates/d2_rust.py run --bin BIN --sample 200 --seed 3                  # stratified sample
python3 gates/d2_rust.py run --bin BIN --baseline gates/d2_ratchet.json       # + ratchet check
python3 gates/d2_rust.py check --baseline OLD/ratchet.json --current NEW/ratchet.json
python3 gates/d2_rust.py selftest
```

Options of `run`: `--jobs N` (default `$D2_JOBS`, else `min(8, cpu count)`), `--timeout S`
(default 180, as the golden runs), `--out DIR` (default `build/d2-rust/<UTC time>` in the gates
checkout; `build/` is gitignored), `--data-root DIR` (default below), `--keep-failing` (keep the
output files, stdout, stderr and argv of failing pairs in `<out>/failing/<case>/<profile>/`).
Filters: `--profile` and `--source` (repeatable), `--case-regex`, `--sample N --seed K`
(deterministic, stratified by profile x source x "has --module_resolution /
--process_common_js_modules", at least one pair per stratum), `--limit N`. A run without a
filter is *complete*.

Exit codes: 0 = the run completed (any pass rate) and, with `--baseline`, no regression;
1 = ratchet regression; 2 = harness error (golden file missing, argv differs from the golden
`compiler_args`, data root not found) or the two ratchets are not comparable.

## What is compared

Pairs: every case of `corpus/d2/cases.jsonl` (the final visible corpus, 2,085 cases) x
`case_args.case_profiles(case)` = 21,155 pairs. For each pair:

- argv = `case_args.compiler_args(case, profile)` with the default out_dir
  `build/golden-tmp/<case>/<profile>`, exactly the golden argv (checked against the golden
  file's `compiler_args`); the binary is run as `BIN <argv...>`;
- env = `run_reference.child_env()` (fixed), stdin `/dev/null`, own session; on timeout the
  whole process group is killed;
- the out_dir is created empty before the run and removed afterwards;
- compared with `corpus-cache/d2/_golden/$REF_GOLDEN_TAG/<case>/<profile>.json` (`ref-<first 8
  hex digits of the reference jar's sha256>`: `ref-cfa8886f` for `v20261006`; the store of
  every older reference, such as `ref-4ef5a893`, stays next to it), **byte for byte,
  no normalization**: exit code, stdout, stderr, the set of files in the out_dir and each
  file's bytes (`out.js`, `out.js.map`, `c0.js`...). A pair passes only if everything is
  identical and it did not time out.

### Data root and run root

`corpus-cache/`, `reference/`, `tools/` and `build/` are gitignored and exist only in the main
checkout. The *data root* is `--data-root`, else `$CLOSURE_RS_DATA_ROOT`, else the gates
checkout if it holds the golden store, else the main checkout of the git repository (for
worktrees). Each run makes a private *run root* `<out>/root/` with symlinks
`corpus-cache -> <data root>/corpus-cache`, `reference -> <data root>/reference`,
`corpus -> <gates checkout>/corpus` and a real `build/golden-tmp/`; the binary runs with
cwd = run root, so the repo-relative argv resolves and concurrent runs never share out_dirs.
The Java self-test shows that this layout reproduces the golden results.

## Outputs (`<out>/`)

- `report.md` / `report.json`: per-profile and per-source pass counts, source maps (D5: pairs
  whose golden has a `.map` file, and how many have every `.map` identical), failures grouped
  by cause and sub-key with up to 5 examples each. Every difference shows the first differing
  byte offset, its line:column in the expected bytes, both lengths and an escaped context
  window from both sides.
- `results.jsonl`: one line per pair (case, profile, source, pass, cause, sub, exit codes,
  wall_ms, diffs).
- `ratchet.json`: `{schema, golden, corpus_sha256, complete, filter, binary_sha256,
  counts: {profile: {pass, total}}, total, passing: {profile: [case ids]},
  failing: {profile: [case ids]}}`.

### Cause heuristics

One primary cause per failing pair, the first that applies, in this order:

| Cause | Meaning | Sub-key |
|---|---|---|
| `harness_error` | the runner could not run the pair (missing golden, argv mismatch) | message |
| `spawn_error` | the binary could not be started | error |
| `timeout` | killed after `--timeout` seconds | |
| `crash` | killed by a signal, or a Rust panic (`panicked at`) in stderr that Java does not have | `signal N` / `panic` |
| `no_output` | empty stdout, empty stderr and no files, while Java produced something | |
| `exit_code` | exit code differs | `expected -> actual` |
| `missing_file` | Java wrote a file the binary did not | file |
| `extra_file` | the binary wrote a file Java did not | file |
| `stderr` | diagnostics differ | `JSC_ key of the first differing line, expected \| actual` |
| `stdout` | stdout differs | |
| `js_output` | a `.js` output file differs | file |
| `sourcemap` | only `.map` files differ | file |

## Ratchet

`check --baseline OLD --current NEW` (and `run --baseline OLD`) exits 1 if any profile's pass
count went down **or** any pair that passed in OLD does not pass in NEW (listed), so a gain in
one place cannot hide a loss in another. A pair that passed in OLD but is no longer in the
corpus is reported, not counted as a regression. Ratchets with a different golden tag or a
different filter (e.g. a sample vs a complete run) are not comparable (exit 2). The deltas per
profile are printed in every case.

## `gates/full.sh [checkout] [--bin PATH] [--baseline PATH] [-- extra run args]`

1. `gates/ci.sh <checkout>`.
2. D2 runner on the complete corpus. Binary: `--bin`, else `$CLOSURE_RS_BIN`, else the bin
   target of a package under `crates/` (`closure-rs`, the CLI's binary, if there are several,
   else `closure-compiler`; built with `cargo build --release` into `$CARGO_TARGET_DIR` or
   `target/`). With no binary it prints
   `D2: SKIP (no Rust CLI binary yet)`, which fails only if the baseline has passing pairs.
   Baseline: `--baseline`, else `$D2_BASELINE`, else `<checkout>/gates/d2_ratchet.json` if
   that file exists (the committed baseline, see "Ratchet baselines" below). A ratchet
   regression fails the gate.
   With `D2_GATE_MODE=passing` it runs only the pairs the baseline lists as passing
   (`run --only-passing BASELINE`): a quicker regression check whose `ratchet.json` must not be
   used as a new baseline.
3. Unit-record runner hook: `<checkout>/gates/unit_rust.sh <checkout>` if it exists and is
   executable, else `UNIT: SKIP (unit-record runner not added yet)`. See "Unit-record ratchet".

Prints `FULL PASS` or `FULL FAIL`; exit 0 only if every step passed.

## Unit-record ratchet (`gates/unit_rust.sh`)

`gates/unit_rust.sh [checkout] [--update]` (checkout default: the one the script lives in) runs
the Rust unit-record runner on the complete unit corpus exactly as `crates/testing/README.md`
documents (`cargo run --release -p closure-testing --bin unit_replay -- --all --report ...
--records-out ...`, in the checkout, honouring `$CARGO_TARGET_DIR` and `$CARGO_BUILD_JOBS`,
default 2). It keeps the runner's output in `<checkout>/build/unit-rust/<UTC time>/`
(`report.json`, `records.jsonl`, `stdout.txt`, `stderr.txt`, and `ratchet.json` of this run),
prints pass / fail / unported counts per non-empty record class and `UNIT_TOTAL ...`, and
compares the passing records with `<checkout>/gates/unit_ratchet.json` when that file exists.

- **Record id:** `Class[index]`, the runner's stable record id: `Class` is the record file stem
  under `corpus/unit/records/`, `index` the 0-based line in `Class.jsonl.gz` (corpus/unit/FORMAT.md,
  crates/testing/README.md).
- **Exit codes:** 0 = the run completed and every record that passes in the baseline passes now
  (failing records are fine: the runner itself exits 1 while any record fails); 1 = ratchet
  regression, every such record listed as `REGRESSION: Class[index] ...`; 2 = harness error (the
  build failed, the runner crashed or did not print `REPLAY_TOTAL`, its counts are inconsistent,
  or the baseline is unreadable). A baseline record that is no longer in the corpus is reported,
  not counted; a changed `corpus/unit/records/` (fingerprint `corpus_records_sha256`) is noted.
- **`--update`** rewrites `gates/unit_ratchet.json` from the current run (still listing what
  regressed against the old one) and exits 0 once the run completed.
- **`unit_ratchet.json`:** `{schema: "closure-rs/unit-ratchet/1", record_id, corpus_records_sha256,
  totals: {records, pass, fail, unported}, passing: ["Class[index]", ...]}`, one passing record
  per line, sorted by class and index.

### Ratchet baselines

`gates/d2_ratchet.json` (the `ratchet.json` of a complete D2 run) and `gates/unit_ratchet.json`
are committed; `gates/full.sh` picks both up, so it fails on a D2 pair or unit record that passed
before and does not now. After a change that makes more pass, raise them: copy the complete D2
run's `ratchet.json` to `gates/d2_ratchet.json` and run `gates/unit_rust.sh <checkout> --update`.

## Self-test

`python3 gates/d2_rust.py selftest [--java-sample N] [--seed K]`:

- `gates/lib/d2_rust_fake_java.py`, a "CLI" that execs the pinned Java reference jar with the
  golden JVM flags, on a stratified sample (at least 2 pairs per (profile, source), including
  module-resolution cases, chunk and source-map profiles), run with `--jobs 1` (one -Xmx3g JVM
  at a time): must pass 100%. The complete corpus would take ~15 JVM-hours.
- `gates/lib/d2_rust_fake_empty.py` (prints nothing, exits 0) on the complete corpus: must
  pass 0 of 21,155.
- Ratchet checks on those results: itself = OK, empty -> java = OK, java -> empty =
  regression, sample vs complete = not comparable.

Unit tests for the comparison, classification, sampling and ratchet logic:
`python3 gates/lib/d2_rust_test.py`.
