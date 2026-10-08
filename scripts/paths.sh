# Sourced by the shell scripts in scripts/ and gates/ (not executable on its own). Sets:
#   ROOT    the main checkout, also when the calling script runs from a git worktree (the shared
#           state under build/, reference/ and corpus-cache/ lives only there). $CLOSURE_RS_ROOT
#           overrides; without git it is the parent of scripts/.
#   WT      a directory for extra git worktrees: $CLOSURE_RS_WT, default closure-rs-wt next to ROOT.
#   SSD_WT  optional second disk for worktrees and build output: $CLOSURE_RS_SSD_WT, default
#           empty (worktrees are plain directories under WT).
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
unset _paths_dir _paths_common
