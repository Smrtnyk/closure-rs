#!/usr/bin/env bash
# The CI gate (docs/PORTING.md §6): formatting, clippy, license headers and the tests.
#   gates/ci.sh [--lint | --test] [checkout-dir]
# checkout-dir defaults to the checkout this script lives in (works in worktrees). Without a mode
# every step runs; --lint runs the checks and clippy (steps 1-4), --test only the tests (step 5),
# which is how .github/workflows/release.yml splits them into two parallel jobs.
# Step 5 uses cargo-nextest when it is installed (one process per test, all test binaries in
# parallel) and plain `cargo test` otherwise; both run the same tests.
set -uo pipefail
MODE=all
DIR=""
for a in "$@"; do
  case "$a" in
    --lint) MODE=lint ;;
    --test) MODE=test ;;
    -*) echo "ci.sh: unknown option $a" >&2; exit 2 ;;
    *) DIR="$a" ;;
  esac
done
DIR="${DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$DIR" || exit 2
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-8}"
fail=0
step() { echo "== $*"; }
bad() { echo "FAIL: $*"; fail=1; }

if [ "$MODE" != test ]; then
step "1. forbid(unsafe_code) in every crate under crates/"
for toml in crates/*/Cargo.toml; do
  [ -e "$toml" ] || continue
  d=$(dirname "$toml")
  if ! grep -Eq '^\[lints\]' "$toml" || ! awk '/^\[lints\]/{f=1;next} /^\[/{f=0} f' "$toml" | grep -Eq '^workspace *= *true'; then
    root=$(ls "$d"/src/lib.rs "$d"/src/main.rs 2>/dev/null | head -1)
    if [ -z "$root" ] || ! grep -q '#!\[forbid(unsafe_code)\]' "$root"; then
      bad "$d: neither [lints] workspace = true nor #![forbid(unsafe_code)]"
    fi
  fi
done
if grep -rnE 'allow\(unsafe_code\)|expect\(unsafe_code\)' crates/ --include=*.rs; then bad "unsafe_code override"; fi

step "1b. license headers (scripts/license_headers.py --check; --apply writes them)"
# Without the upstream sources (reference/, tools/jdk-21, protobuf, build/license-sources) the
# check compares the headers with LICENSES/headers.tsv; it never downloads anything.
if lic_out=$(python3 scripts/license_headers.py --check 2>&1); then
  printf '%s\n' "$lic_out" | grep -E '^(full check|manifest check|upstream sources not available)'
else
  printf '%s\n' "$lic_out" | tail -40; bad "license headers"
fi

step "2a. cargo fmt --check"
cargo fmt --all --check >/dev/null 2>&1 || { cargo fmt --all --check 2>&1 | head -40; bad "cargo fmt"; }

step "2b/3. cargo clippy -D warnings (crates/clippy.toml bans std HashMap/HashSet)"
cargo clippy --workspace --all-targets --quiet -- -D warnings || bad "clippy"

step "4. no ignored or skipped tests in crates/"
if grep -rnE '#\[ignore|#\[cfg\(any\(\)\)\]|xfail|skip_list|SKIP_LIST|TEST_ALLOW_?LIST|test_allow_?list\b|allow_?listed_tests|ALLOW_?LISTED_TESTS' crates/ --include=*.rs; then bad "ignored/skipped tests"; fi

fi

if [ "$MODE" != lint ]; then
if cargo nextest --version >/dev/null 2>&1; then
  step "5. cargo nextest run (and doctests with cargo test --doc)"
  cargo nextest run --workspace --locked --no-fail-fast --status-level fail --final-status-level slow \
    --hide-progress-bar 2>&1
  [ "${PIPESTATUS[0]}" -eq 0 ] || bad "cargo nextest"
  cargo test --workspace --locked --doc --quiet 2>&1 | grep -E 'FAILED|panicked|error(\[|:)' | head -40
  [ "${PIPESTATUS[0]}" -eq 0 ] || bad "doctests"
else
  step "5. cargo test"
  cargo test --workspace --quiet 2>&1 | grep -E '^test result|FAILED|panicked|error(\[|:)' | grep -v ' 0 failed' | head -40
  [ "${PIPESTATUS[0]}" -eq 0 ] || bad "cargo test"
fi
fi

if [ $fail -eq 0 ]; then echo "CI PASS"; else echo "CI FAIL"; fi
exit $fail
