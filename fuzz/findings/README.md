# Fuzz findings

`fuzz-driver run` writes one file per minimized mismatch here: `fz-<hash>.md`, where the hash is
the first 12 hex digits of the SHA-256 of the profile and the minimized repro (fuzz/DESIGN.md,
driver flow step 7). Each file is one porting defect (docs/PORTING.md §7). It holds the engines,
the profile, the argv, the difference, the minimized repro, both outcomes and the original
program with its origin (`jsgen` seed and index, or the mutated D2 file).

Once the defect is fixed, a `## Resolution` section is added by hand: the Java behaviour the port
missed, the fix, and the regression case under `crates/cli/tests/data/fuzz_regressions/` that pins
it (run by `crates/cli/tests/fuzz_regression_test.rs`). A difference found outside the fuzzer, on
an out-of-corpus benchmark (`bench/`), is written up in the same form
(`bench-fabric-sourcemap-resolve.md`).

| File | What it explains | Regression cases |
|---|---|---|
| [fz-6fc868fd3e39.md](fz-6fc868fd3e39.md) | A diagnostic argument holding an unpaired surrogate is printed as `?` (Java's UTF-8 error stream), not U+FFFD. | `typeof-lone-surrogate`, `typeof-lone-surrogate-json`, `namespace-lone-surrogate` |
| [fz-b67df42e8d08.md](fz-b67df42e8d08.md) | `OptimizeCalls.ReferenceMap#isCallTarget` on a reference whose parent was detached earlier in the same run answers false. | `optimize-calls-detached-call-target` |
| [bench-fabric-sourcemap-resolve.md](bench-fabric-sourcemap-resolve.md) | An input source map that fails to resolve while the report is printed is counted in the summary line but not printed. | `dangling-source-map-url`, `dangling-source-map-url-json` |

Runs with a synthetic engine (`--engine-b java-perturbed`, the driver's self-test) pass
`--findings` with a directory under `build/` (for example `build/fuzz/selftest-findings`), so
their files never land here.
