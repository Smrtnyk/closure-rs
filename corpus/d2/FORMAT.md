# D2 differential corpus — case format

A case is one compilation unit, run under one or more option profiles. Cases are listed
one per line in `cases.jsonl`. The input files themselves are **not** committed. Each
input is downloaded into `corpus-cache/d2/` by `scripts/fetch_d2.sh`, which checks it
against the sha256 recorded in `sources.lock.json`.

```json
{
  "id": "npm-lodash-es-4.17.21-advanced-entry",
  "source": "npm | closure-library | test262 | closure-self | whole-program",
  "group": "npm:lodash-es@4.17.21",
  "license": "MIT",
  "inputs":  ["corpus-cache/d2/npm/lodash-es@4.17.21/package/lodash.js"],
  "externs": [],
  "shims":   ["corpus/d2/shims/<id>/entry.js"],
  "extra_flags": ["--module_resolution=NODE"],
  "profiles": ["ws", "simple", "advanced", "advanced_strict", "lang_es5", "lang_es2015",
               "lang_next", "pretty", "sourcemap"],
  "test_command": null
}
```

- **`group`:** groups the cases of one origin (a package version or a library
  directory); `scripts/fetch_d2.sh --group` fetches one group.
- **`shims`:** small generated entry files, committed to the repository. A shim keeps
  ADVANCED-mode output non-trivial, for example by writing the exports of the code under
  test to `globalThis['name']`.
- **`profiles`:** each name refers to a flag set in `profiles.json`.
- **`test_command`:** only for whole-program cases. It is a command that runs the program's
  own tests against the compiled output.

## Sources

- **Group keys.** `group` collects the cases of one origin. The key `npm:<name>@<version>` is
  shared by the npm and whole-program collectors, so all cases of one package version form one
  group. test262 groups are directories: 3 path levels under `language/`, 2 under `built-ins/`.
- **Pins.** The Closure Library is pinned to its final release tag, v20230802 (`ccf3f1dc`);
  test262 to `5992dc3b`.
- **npm package cases** compile from a committed shim entry, with `--module_resolution=NODE`,
  `--dependency_mode=PRUNE` and `--entry_point`. The shim writes the package exports to
  `globalThis`, so that ADVANCED output is non-trivial.
- **Whole-program cases** run their own zero-dependency test suites with
  `node corpus/d2/shims/<id>/run.mjs {output}`. Every library's test suite passes on its original
  sources.
- **closure-self cases** read their inputs straight from a reference checkout, verified by
  sha256 rather than downloaded. The inputs stay at reference `bb8c8e7` (release `v20261005`):
  their group keys (`@bb8c8e7`) and input paths under `reference/closure-compiler/` do not change
  when the default reference moves, so that checkout is kept as a permanent input checkout
  (docs/PORTING.md §9). Only the compiler that processes those inputs changes with the reference.

## Final corpus (`cases.jsonl`)

`cases.jsonl` is the validated corpus, sorted by `id`, written by `corpus/d2/validate/finalize.py`
from the candidates and the golden results (pipeline: `gates/lib/golden_all.py`, then
`corpus/d2/validate/scan_golden.py`, `wp_test.py` and `finalize.py`; work files stay in
`build/validate/`). Compared with a candidate line:

- **`profiles`** is the final, explicit list: the candidate's profiles plus auto-applied ones
  (every other profile that applies, i.e. the full matrix of docs/PORTING.md §4.4 including both chunk
  configurations `chunks2` and `chunks3`; see `PROFILES.md`), minus every pair where the Java
  reference crashed (internal compiler error or
  uncaught exception, exit 254) or timed out, and minus ADVANCED/ADVANCED-strict runs with empty
  output and zero diagnostics. Also minus every pair of a case that references a
  file missing from the repository or cache (`case_files_missing`; such a golden result, e.g.
  `JSC_READ_ERROR` on a deleted shim, is not reference behaviour). Pairs that end in compile
  errors are kept (diagnostic parity). Dropped pairs are listed in `JAVA_FAILURES.md`. A case
  with no profile left is dropped. `case_profiles()` in `gates/lib/case_args.py` returns a final
  case's `profiles` unchanged (it recognises final cases by their `tags`/`test_profiles` keys);
  it auto-appends the missing profiles only to candidate cases.
- **`tags`** (list of strings): `test262:java-parser-rejects-valid`,
  `test262:java-errors-on-valid`, `test262:java-accepts-invalid`, `test262:negative-<phase>`,
  `java-crash-profiles-dropped`. Tags never change how a case is run:
  - `test262:java-parser-rejects-valid`: test262 expects the file to parse (not a negative parse
    test), but Java reports `ERROR - [JSC_PARSE_ERROR]`. Kept for diagnostic parity.
  - `test262:java-errors-on-valid`: expected to parse; Java parses it but reports another error
    (for example `JSC_VAR_ARGUMENTS_SHADOWED_ERROR`).
  - `test262:java-accepts-invalid`: a negative parse test that Java compiles with no error.
  - `test262:negative-<phase>`: the test262 `negative` phase.
  - `java-crash-profiles-dropped`: at least one profile was dropped because Java crashed.
- **`test_profiles`**: whole-program cases only (otherwise `null`). The profiles whose Java
  output passes `test_command`; only these carry the test requirement. For a chunk profile,
  `{output}` is the chunk files concatenated in chunk order with a newline between them
  (`c0.js`, `\n`, `c1.js`, and for `chunks3` `\n`, `c2.js`). See `WHOLE_PROGRAM.md`.

`sources.lock.json` merges the five candidate locks: `files` maps every repo-relative path to
its sha256, byte count, source and how to fetch it (`url`, `tarball`, `tarball_file`, or
`in_repo` for shims and the reference checkout); `tarballs` holds the npm tarballs (url, sha256,
integrity, shasum); `groups` lists, per group, its cases, files and tarballs.
`scripts/fetch_d2.sh` re-creates and verifies the cache from it (`--group`, `--source`,
`--check`, `--cache-dir`).
