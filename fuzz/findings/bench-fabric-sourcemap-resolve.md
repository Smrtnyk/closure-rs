# Finding bench-fabric-sourcemap-resolve (D7 benchmark, not a fuzz program)

- engines: A = the pinned reference jar (`$REF_JAR`, `scripts/paths.sh`), B = closure-rs
- profile: the D7 fabric job (`bench/projects.json`), SIMPLE and ADVANCED
- difference: stderr summary line, `0 error(s), 51 warning(s)` (Java) vs `50` (Rust); ADVANCED 3506 vs 3505
- origin: `bench-cache/fabric/dist/index.mjs` ends with `//# sourceMappingURL=index.mjs.map`; that file is not shipped

## Minimized repro

```js
// --- in.js ---
function f() {
  return 1;
  alert(2);
}
alert(f());
//# sourceMappingURL=missing.map
```

`--js in.js`: Java prints one JSC_UNREACHABLE_CODE warning and `0 error(s), 2 warning(s)`; the port
printed `1 warning(s)`.

## Resolution

- status: fixed in [#3](https://github.com/Smrtnyk/closure-rs/pull/3) (`SortingErrorManager` counts the reports a formatter makes during
  `generateReport`).
- root cause: Java resolves an input source map lazily. While `PrintStreamErrorReportGenerator`
  (or `JsonErrorReportGenerator`) prints, the formatter calls `Compiler#getSourceMapping`, whose
  `SourceMapInput#getSourceMap` reports SOURCEMAP_RESOLVE_FAILED straight into the error manager.
  The generator iterates over the copy `SortingErrorManager#getSortedDiagnostics` made before, so
  the warning is not printed, but `report` counts it, and the summary that follows includes it.
  The port queued that report in the excerpt provider and replayed it only after the whole report.
- fix: the queue (`sorting_error_manager::DeferredReports`, the excerpt provider's
  `pending_errors`) is attached to the SortingErrorManager the Compiler or the CLI creates; both
  generators call `SortingErrorManager::report_deferred` after their loop and before the summary
  (the generator trait takes `&mut SortingErrorManager`, as Java's generator gets the mutable
  manager). `Compiler#generateReport` also replays reports queued before the report, which Java
  made when they happened.
- verified: fabric SIMPLE and ADVANCED stderr and output byte-identical to Java (51 and 3506 warnings).
- regression tests: `crates/cli/tests/data/fuzz_regressions/dangling-source-map-url/` (text format,
  two inputs, only one with a warning) and `dangling-source-map-url-json/` (`--error_format=JSON`,
  ADVANCED, with `--create_source_map`), run by `crates/cli/tests/fuzz_regression_test.rs`.
