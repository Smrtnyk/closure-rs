"""What the CLI fixture generators of this directory share: the selected reference
(scripts/paths.py: $CLOSURE_RS_REF, else the registry's default), the pinned JDK, the fixed child
environment and the Java helpers (CliMetadata.java, CliOptionSetup.java) compiled against the
reference jar.

Not a script; generate_golden.py, generate_exception_golden.py and generate_setup.py import
it. Their work directory (--work) holds the compiled helpers and the flag
metadata; it defaults to build/cli-fixtures/<tag>/ of this checkout.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
CRATE = TOOLS.parent
CHECKOUT = CRATE.parents[1]
sys.path.insert(0, str(CHECKOUT / "scripts"))
import paths  # noqa: E402  ROOT is the main checkout (tools/, reference/ and the jars live there)

ROOT = Path(paths.ROOT)
REF_TAG = paths.REF_TAG
REF_JAR = paths.REF_JAR
REF_SRC = paths.REF_SRC
JDK = ROOT / "tools" / "jdk-21" / "bin"
# The pinned JDK: the goldens hold JDK stack frames, whose line numbers differ between JDK builds.
JAVA = Path(os.environ.get("JAVA") or JDK / "java")
FIXTURES = CRATE / "tests" / "data"


def default_work() -> Path:
    return CHECKOUT / "build" / "cli-fixtures" / REF_TAG


def child_env() -> dict:
    """The environment of every reference run: nothing inherited (JAVA_TOOL_OPTIONS and the like
    would print "Picked up ..." on stderr), a UTF-8 locale, UTC."""
    return {"PATH": "/usr/bin:/bin", "HOME": os.environ.get("HOME", "/tmp"), "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8", "TZ": "UTC"}


def classes(work: Path) -> Path:
    """CliMetadata and CliOptionSetup compiled against the reference jar, in work/classes (compiled
    again when the jar or a source is newer than the classes)."""
    out = work / "classes"
    sources = [TOOLS / "CliMetadata.java", TOOLS / "CliOptionSetup.java"]
    stamp = out / ".compiled"
    newest = max(os.path.getmtime(p) for p in [*sources, Path(REF_JAR)])
    if not stamp.exists() or os.path.getmtime(stamp) < newest or stamp.read_text() != REF_JAR:
        out.mkdir(parents=True, exist_ok=True)
        subprocess.run([str(JDK / "javac"), "-nowarn", "-encoding", "UTF-8", "-proc:none", "-cp", REF_JAR,
                        "-d", str(out), *map(str, sources)], check=True, env=child_env())
        stamp.write_text(REF_JAR)
    return out


def flag_metadata(work: Path) -> dict:
    """CliMetadata's dump of the reference's CommandLineRunner flags (declaration order), also
    written to work/metadata.json."""
    cls = classes(work)
    r = subprocess.run([str(JAVA), "-Xmx1g", "-cp", f"{cls}:{REF_JAR}", "com.google.javascript.jscomp.CliMetadata"],
                       check=True, capture_output=True, env=child_env())
    (work / "metadata.json").write_bytes(r.stdout)
    return json.loads(r.stdout)


def write_if_changed(path: Path, data: bytes, check: bool) -> bool:
    """Writes data to path unless it is already there or check is set; True when it differs."""
    if path.exists() and path.read_bytes() == data:
        return False
    if not check:
        path.write_bytes(data)
    return True
