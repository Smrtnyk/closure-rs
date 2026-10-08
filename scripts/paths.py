"""Paths for the Python scripts, as scripts/paths.sh sets them for the shell scripts:
  ROOT    the main checkout, also when the calling script runs from a git worktree (the shared
          state under build/, reference/ and corpus-cache/ lives only there); $CLOSURE_RS_ROOT
          overrides; without git it is the parent of scripts/.
  WT      a directory for extra git worktrees: $CLOSURE_RS_WT, default closure-rs-wt next to ROOT.
  SSD_WT  optional build disk for worktrees: $CLOSURE_RS_SSD_WT, default '' (none).
Scripts outside scripts/ import it with sys.path.insert(0, <repo>/scripts)."""
import os
import subprocess

_HERE = os.path.dirname(os.path.abspath(__file__))


def _main_checkout():
    if os.environ.get("CLOSURE_RS_ROOT"):
        return os.environ["CLOSURE_RS_ROOT"]
    try:
        common = subprocess.run(["git", "-C", _HERE, "rev-parse", "--path-format=absolute", "--git-common-dir"],
                                capture_output=True, text=True, check=True).stdout.strip()
        if os.path.basename(common) == ".git":
            return os.path.dirname(common)
    except (OSError, subprocess.CalledProcessError):
        pass  # not a git checkout
    return os.path.dirname(_HERE)


ROOT = _main_checkout()
WT = os.environ.get("CLOSURE_RS_WT") or os.path.join(os.path.dirname(ROOT), "closure-rs-wt")
SSD_WT = os.environ.get("CLOSURE_RS_SSD_WT", "")
