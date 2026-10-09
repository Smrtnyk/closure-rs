"""Paths for the Python scripts, as scripts/paths.sh sets them for the shell scripts:
  ROOT    the main checkout, also when the calling script runs from a git worktree (the shared
          state under build/, reference/ and corpus-cache/ lives only there); $CLOSURE_RS_ROOT
          overrides; without git it is the parent of scripts/.
  WT      a directory for extra git worktrees: $CLOSURE_RS_WT, default closure-rs-wt next to ROOT.
  SSD_WT  optional build disk for worktrees: $CLOSURE_RS_SSD_WT, default '' (none).
and the Java reference (docs/PORTING.md §9): REF, the row of scripts/references.tsv (read from
this checkout) named by $CLOSURE_RS_REF, default its `default` row (paths relative to ROOT), and
the same values as paths.sh exports (paths absolute under ROOT): REF_TAG, REF_COMMIT,
REF_JAR_SHA256 ("-" = not pinned yet), REF_GOLDEN_TAG ("ref-<sha8>" or "-"), REF_SRC,
REF_RECORDING_WS, REF_JAR, ORACLE_JAR.
Scripts outside scripts/ import it with sys.path.insert(0, <repo>/scripts).
`python3 scripts/paths.py` prints the resolved values."""
from __future__ import annotations

import os
import subprocess
from typing import NamedTuple

_HERE = os.path.dirname(os.path.abspath(__file__))
REGISTRY = os.path.join(_HERE, "references.tsv")


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


class Reference(NamedTuple):
    """One registry row; the paths are relative to ROOT."""
    tag: str
    commit: str
    jar_sha256: str  # "-" until the jar is built and pinned
    src: str
    recording_ws: str
    jar: str
    oracle_jar: str

    @property
    def golden_tag(self) -> str:
        """The D2 golden store tag, ref-<first 8 hex digits of the jar sha256>, or "-"."""
        return "-" if self.jar_sha256 == "-" else "ref-" + self.jar_sha256[:8]


def load_registry(path: str = REGISTRY) -> tuple[dict, str]:
    """({tag: Reference}, default tag) of a references.tsv."""
    refs, default = {}, ""
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            cols = line.rstrip("\n").split("\t")
            if cols[0] == "default":
                default = cols[1]
            elif len(cols) >= 7:
                refs[cols[0]] = Reference(*cols[:7])
    return refs, default


def reference(tag: str | None = None) -> Reference:
    """The registry row of `tag`, default $CLOSURE_RS_REF, else the registry's default."""
    refs, default = load_registry()
    tag = tag or os.environ.get("CLOSURE_RS_REF") or default
    if tag not in refs:
        raise SystemExit(f"scripts/paths.py: unknown reference '{tag}' (CLOSURE_RS_REF; tags in {REGISTRY})")
    return refs[tag]


REF = reference()
REF_TAG = REF.tag
REF_COMMIT = REF.commit
REF_JAR_SHA256 = REF.jar_sha256
REF_GOLDEN_TAG = REF.golden_tag
REF_SRC = os.path.join(ROOT, REF.src)
REF_RECORDING_WS = os.path.join(ROOT, REF.recording_ws)
REF_JAR = os.path.join(ROOT, REF.jar)
ORACLE_JAR = os.path.join(ROOT, REF.oracle_jar)

if __name__ == "__main__":
    for _k in ("ROOT", "REF_TAG", "REF_COMMIT", "REF_JAR_SHA256", "REF_GOLDEN_TAG", "REF_SRC",
               "REF_RECORDING_WS", "REF_JAR", "ORACLE_JAR"):
        print(f"{_k}={globals()[_k]}")
