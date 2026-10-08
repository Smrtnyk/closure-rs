# Fuzz findings

`fuzz-driver run` writes one file per minimized mismatch here: `fz-<sha256(profile, repro)[:12]>.md`.
Each file is one porting defect to fix (docs/PORTING.md §7). It holds the
engines, the profile, the argv, the difference, the minimized repro, both outcomes and the
original program with its origin (`jsgen` seed and index, or the mutated D2 file).

Self-tests that use a synthetic engine (`--engine-b java-perturbed`) write to
`build/fuzz/selftest-findings/` instead, never here.
