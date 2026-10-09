# Sourced by the shell scripts in scripts/ and gates/. Sets:
#   ROOT    the main checkout, also when the calling script runs from a git worktree (the shared
#           state under build/, reference/ and corpus-cache/ lives only there). $CLOSURE_RS_ROOT
#           overrides; without git it is the parent of scripts/.
#   WT      a directory for extra git worktrees: $CLOSURE_RS_WT, default closure-rs-wt next to ROOT.
#   SSD_WT  optional second disk for worktrees and build output: $CLOSURE_RS_SSD_WT, default
#           empty (worktrees are plain directories under WT).
# and the Java reference (docs/PORTING.md §9): the row of scripts/references.tsv (read from this
# checkout) named by $CLOSURE_RS_REF, default its `default` row. Exported, absolute under ROOT:
#   REF_TAG REF_COMMIT REF_JAR_SHA256 ("-" = not pinned yet) REF_GOLDEN_TAG (ref-<sha8>, or "-")
#   REF_SRC REF_RECORDING_WS REF_JAR ORACLE_JAR, CLOSURE_RS_REF (= REF_TAG, so child processes
#   resolve the same reference), CLOSURE_RS_REFERENCE_JAR (= REF_JAR) and CLOSURE_RS_ORACLE_JAR
#   (= ORACLE_JAR) for the Rust fuzz oracle client.
# Run directly (bash scripts/paths.sh) it prints the resolved values.
_paths_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -n "${CLOSURE_RS_ROOT:-}" ]; then
  ROOT="$CLOSURE_RS_ROOT"
else
  _paths_common="$(git -C "$_paths_dir" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
  if [ -n "$_paths_common" ] && [ "$(basename "$_paths_common")" = .git ]; then
    ROOT="$(dirname "$_paths_common")"
  else
    ROOT="$(dirname "$_paths_dir")"
  fi
fi
WT="${CLOSURE_RS_WT:-$(dirname "$ROOT")/closure-rs-wt}"
SSD_WT="${CLOSURE_RS_SSD_WT:-}"

# ref_row TAG: prints the registry row of TAG (tab-separated, without the tag), or fails.
ref_row() {
  awk -F'\t' -v t="$1" '!/^#/ && NF >= 7 && $1 == t { print $2 "\t" $3 "\t" $4 "\t" $5 "\t" $6 "\t" $7; f = 1 }
                        END { exit !f }' "$REF_REGISTRY"
}
REF_REGISTRY="$_paths_dir/references.tsv"
REF_TAG="${CLOSURE_RS_REF:-$(awk -F'\t' '!/^#/ && $1 == "default" { print $2 }' "$REF_REGISTRY")}"
if ! _paths_row="$(ref_row "$REF_TAG")"; then
  echo "scripts/paths.sh: unknown reference '$REF_TAG' (CLOSURE_RS_REF; tags in $REF_REGISTRY)" >&2
  exit 2
fi
IFS=$'\t' read -r REF_COMMIT REF_JAR_SHA256 REF_SRC REF_RECORDING_WS REF_JAR ORACLE_JAR <<< "$_paths_row"
REF_SRC="$ROOT/$REF_SRC"
REF_RECORDING_WS="$ROOT/$REF_RECORDING_WS"
REF_JAR="$ROOT/$REF_JAR"
ORACLE_JAR="$ROOT/$ORACLE_JAR"
if [ "$REF_JAR_SHA256" = - ]; then REF_GOLDEN_TAG=-; else REF_GOLDEN_TAG="ref-${REF_JAR_SHA256:0:8}"; fi
CLOSURE_RS_REF="$REF_TAG"
CLOSURE_RS_REFERENCE_JAR="$REF_JAR"
CLOSURE_RS_ORACLE_JAR="$ORACLE_JAR"
export REF_TAG REF_COMMIT REF_JAR_SHA256 REF_GOLDEN_TAG REF_SRC REF_RECORDING_WS REF_JAR ORACLE_JAR \
  CLOSURE_RS_REF CLOSURE_RS_REFERENCE_JAR CLOSURE_RS_ORACLE_JAR
unset _paths_dir _paths_common _paths_row
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  for _v in ROOT REF_TAG REF_COMMIT REF_JAR_SHA256 REF_GOLDEN_TAG REF_SRC REF_RECORDING_WS REF_JAR ORACLE_JAR; do
    echo "$_v=${!_v}"
  done
fi
