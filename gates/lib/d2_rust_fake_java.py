#!/usr/bin/env python3
"""Self-test fake for gates/d2_rust.py: a "Rust CLI" that is really the pinned Java reference.

It execs `java <run_reference.jvm_flags> -jar closure-compiler.jar <argv...>` with absolute
paths into the data root (the runner's cwd is its private run root).  It adds nothing to
stdout or stderr, so on every D2 pair it must reproduce the golden result exactly.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d2_rust_core as core  # noqa: E402
import run_reference  # noqa: E402


def main() -> None:
    root = core.find_data_root()
    java = os.path.join(root, run_reference.JAVA)
    jar = os.path.join(root, run_reference.JAR)
    cds = os.path.join(root, run_reference.CDS_ARCHIVE)
    use_cds = os.path.exists(cds)
    os.makedirs(os.path.join(root, run_reference.JVM_CRASH_DIR), exist_ok=True)
    flags = []
    for f in run_reference.jvm_flags(run_reference.DEFAULT_XMX, use_cds):
        if f.startswith("-XX:SharedArchiveFile="):
            f = f"-XX:SharedArchiveFile={cds}"
        elif f.startswith(("-XX:ErrorFile=", "-XX:ReplayDataFile=")):
            # The runner's cwd is its run root: the crash files go to the data root's crash dir.
            name, value = f.split("=", 1)
            f = f"{name}={os.path.join(root, value)}"
        flags.append(f)
    os.execv(java, [java, *flags, "-jar", jar, *sys.argv[1:]])


if __name__ == "__main__":
    main()
