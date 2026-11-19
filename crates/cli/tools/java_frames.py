#!/usr/bin/env python3
"""Writes crates/cli/src/java_frames.rs, the Java stack frames of the reference's own classes that
the CLI prints in its emulated Java exceptions, from the line tables of the reference jar.

  python3 crates/cli/tools/java_frames.py           rewrite java_frames.rs
  python3 crates/cli/tools/java_frames.py --check   exit 1 when java_frames.rs is stale
  python3 crates/cli/tools/java_frames.py --show CLASS METHOD
                                                    list a method's calls with their lines

A frame's line is the source line of the bytecode instruction the frame stands at: the call of a
method or constructor (FRAMES names the method that makes the call, the callee and, when the
callee is called more than once there, which call). The script disassembles the classes with the
pinned JDK's javap and reads each method's LineNumberTable, so the lines follow the reference
(scripts/paths.py: $CLOSURE_RS_REF, else the registry's default) however its sources move, and a
call FRAMES names that the method no longer makes (or makes more often) is an error that names the
frame. The step cli-fixtures of scripts/upstream_sync.py runs it with the exception and CLI goldens,
whose stack traces check the frames end to end; gates/ci.sh runs --check, which skips when the
reference jar or the JDK is missing (as on CI).
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import reference_tools as rt  # noqa: E402

OUT = rt.CRATE / "src" / "java_frames.rs"
PKG = "com.google.javascript.jscomp."
# (constant, class, method (with its parameter types when overloaded), callee, which call: None =
# the only one, n = the n-th, from 0). The callee is a regular expression searched in javap's
# comment of the call, for example 'Method run:()V' or
# 'Method java/io/FileInputStream."<init>":(Ljava/lang/String;)V'. A constructor's name is <init>.
# Frames of the libraries in the jar (guava, gson) and of the JDK change only with their versions
# and stay where the CLI prints them.
RUNTIME_EXCEPTION = r'Method java/lang/RuntimeException\."<init>"'
FRAMES = [
    # CommandLineRunner#main, its constructor and initConfigFromFlags (FlagUsageException).
    ("COMMAND_LINE_RUNNER_MAIN_NEW", "CommandLineRunner", "main", r'Method "<init>":\(\[Ljava/lang/String;\)V', None),
    ("COMMAND_LINE_RUNNER_MAIN_RUN", "CommandLineRunner", "main", r"Method run:\(\)V", None),
    ("COMMAND_LINE_RUNNER_INIT_INIT_CONFIG", "CommandLineRunner", "<init>(java.lang.String[])",
     r"Method initConfigFromFlags:", None),
    ("COMMAND_LINE_RUNNER_INIT_CONFIG_FLAG_USAGE", "CommandLineRunner", "initConfigFromFlags",
     r'Method com/google/javascript/jscomp/FlagUsageException\."<init>"', None),
    # CommandLineRunner#createOptions: the XTB file and the conformance configs.
    ("COMMAND_LINE_RUNNER_CREATE_OPTIONS_XTB_STREAM", "CommandLineRunner", "createOptions",
     r'Method java/io/FileInputStream\."<init>"', None),
    ("COMMAND_LINE_RUNNER_CREATE_OPTIONS_XTB_BUNDLE", "CommandLineRunner", "createOptions",
     r'Method com/google/javascript/jscomp/XtbMessageBundle\."<init>"', None),
    ("COMMAND_LINE_RUNNER_CREATE_OPTIONS_XTB_ERROR", "CommandLineRunner", "createOptions", RUNTIME_EXCEPTION, None),
    ("COMMAND_LINE_RUNNER_CREATE_OPTIONS_CONFORMANCE", "CommandLineRunner", "createOptions",
     r"Method loadConformanceConfigs:", None),
    ("COMMAND_LINE_RUNNER_LOAD_CONFORMANCE_CONFIGS_LOAD", "CommandLineRunner", "loadConformanceConfigs",
     r"Method loadConformanceConfig:", None),
    ("COMMAND_LINE_RUNNER_LOAD_CONFORMANCE_CONFIGS_ERROR", "CommandLineRunner", "loadConformanceConfigs",
     RUNTIME_EXCEPTION, None),
    ("COMMAND_LINE_RUNNER_LOAD_CONFORMANCE_CONFIG_READ", "CommandLineRunner", "loadConformanceConfig",
     r"Method com/google/common/io/CharSource\.read:", None),
    ("COMMAND_LINE_RUNNER_LOAD_CONFORMANCE_CONFIG_MERGE", "CommandLineRunner", "loadConformanceConfig",
     r"Method com/google/protobuf/TextFormat\.merge:", None),
    ("COMMAND_LINE_RUNNER_LOAD_CONFORMANCE_CONFIG_ERROR", "CommandLineRunner", "loadConformanceConfig",
     RUNTIME_EXCEPTION, None),
    # AbstractCommandLineRunner#run and #doRun.
    ("ABSTRACT_COMMAND_LINE_RUNNER_RUN_DO_RUN", "AbstractCommandLineRunner", "run", r"Method doRun:\(\)I", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_DO_RUN_LOGGING_LEVEL", "AbstractCommandLineRunner", "doRun",
     r"Method java/util/logging/Level\.parse:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_DO_RUN_CREATE_OPTIONS", "AbstractCommandLineRunner", "doRun",
     r"Method createOptions:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_DO_RUN_SET_RUN_OPTIONS", "AbstractCommandLineRunner", "doRun",
     r"Method setRunOptions:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_DO_RUN_PARSE_JSON", "AbstractCommandLineRunner", "doRun",
     r"Method parseJsonFilesFromInputStream:", None),
    # The first call on a JSON file spec: a null spec throws the NullPointerException here.
    ("ABSTRACT_COMMAND_LINE_RUNNER_DO_RUN_JSON_SOURCE_MAP", "AbstractCommandLineRunner", "doRun",
     r"Method com/google/javascript/jscomp/AbstractCommandLineRunner\$JsonFileSpec\.getSourceMap:", 0),
    ("ABSTRACT_COMMAND_LINE_RUNNER_DO_RUN_PROCESS_RESULTS", "AbstractCommandLineRunner", "doRun",
     r"Method processResults:", None),
    # AbstractCommandLineRunner#setRunOptions: the variable map, then the property map.
    ("ABSTRACT_COMMAND_LINE_RUNNER_SET_RUN_OPTIONS_VARIABLE_MAP", "AbstractCommandLineRunner", "setRunOptions",
     r"Method com/google/javascript/jscomp/VariableMap\.load:", 0),
    ("ABSTRACT_COMMAND_LINE_RUNNER_SET_RUN_OPTIONS_PROPERTY_MAP", "AbstractCommandLineRunner", "setRunOptions",
     r"Method com/google/javascript/jscomp/VariableMap\.load:", 1),
    # AbstractCommandLineRunner#parseJsonFilesFromInputStream.
    ("ABSTRACT_COMMAND_LINE_RUNNER_PARSE_JSON_BEGIN_ARRAY", "AbstractCommandLineRunner",
     "parseJsonFilesFromInputStream", r"Method com/google/gson/stream/JsonReader\.beginArray:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_PARSE_JSON_HAS_NEXT", "AbstractCommandLineRunner",
     "parseJsonFilesFromInputStream", r"Method com/google/gson/stream/JsonReader\.hasNext:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_PARSE_JSON_FROM_JSON", "AbstractCommandLineRunner",
     "parseJsonFilesFromInputStream", r"Method com/google/gson/Gson\.fromJson:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_PARSE_JSON_END_ARRAY", "AbstractCommandLineRunner",
     "parseJsonFilesFromInputStream", r"Method com/google/gson/stream/JsonReader\.endArray:", None),
    # AbstractCommandLineRunner: the output files.
    ("ABSTRACT_COMMAND_LINE_RUNNER_PROCESS_RESULTS_OUTPUT", "AbstractCommandLineRunner", "processResults",
     r"Method outputSingleBinary:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_OUTPUT_SINGLE_BINARY", "AbstractCommandLineRunner", "outputSingleBinary",
     r"Method createDefaultOutput:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_CREATE_DEFAULT_OUTPUT", "AbstractCommandLineRunner", "createDefaultOutput",
     r"Method fileNameToLegacyOutputWriter:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_LEGACY_OUTPUT_WRITER", "AbstractCommandLineRunner",
     "fileNameToLegacyOutputWriter", r"Method filenameToOutputStream:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_OUTPUT_WRITER2", "AbstractCommandLineRunner", "fileNameToOutputWriter2",
     r"Method filenameToOutputStream:", None),
    ("ABSTRACT_COMMAND_LINE_RUNNER_OUTPUT_STREAM", "AbstractCommandLineRunner", "filenameToOutputStream",
     r'Method java/io/FileOutputStream\."<init>"', None),
    # VariableMap#load and the parse error of a malformed map.
    ("VARIABLE_MAP_LOAD_READ", "VariableMap", "load", r"Method com/google/common/io/Files\.toByteArray:", None),
    ("VARIABLE_MAP_LOAD_FROM_BYTES", "VariableMap", "load", r"Method fromBytes:", None),
    ("VARIABLE_MAP_LOAD_ERROR", "VariableMap", "load", r'Method java/io/IOException\."<init>"', None),
    ("VARIABLE_MAP_FROM_BYTES_FROM_LINES", "VariableMap", "fromBytes", r"Method fromLines:", None),
    ("VARIABLE_MAP_FROM_LINES_PARSE_ERROR", "VariableMap", "fromLines", r'Method java/text/ParseException\."<init>"',
     None),
    # XtbMessageBundle's constructor: a SAX or I/O error.
    ("XTB_MESSAGE_BUNDLE_INIT_ERROR", "XtbMessageBundle", "<init>(java.io.InputStream, java.lang.String)",
     RUNTIME_EXCEPTION, None),
]
HEADER = """\
/*
 * Copyright 2026 The closure-rs Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! The Java stack frames of the reference's own classes that the CLI's emulated exceptions print,
//! with the line numbers of the reference jar. Generated by `crates/cli/tools/java_frames.py` from
//! the jar's line tables; do not edit (the script rewrites it, `--check` fails when it is stale).
"""

METHOD = re.compile(r"^  (?:[\w$.<>\[\], ?]+ )?([\w$.<>]+)\(([^)]*)\)(?: throws [\w$., ]+)?;$")
INSN = re.compile(r"^\s+(\d+): (\w+)\s*(?:[#\d, ]*)(?://\s*(.*))?$")
LINE = re.compile(r"^\s+line (\d+): (\d+)$")


def disassemble(classes: list[str]) -> dict:
    """{class: {method key: (instructions [(bci, opcode, comment)], line table [(bci, line)])}};
    a method key is "name" and "name(param types)" (a constructor's name is <init>)."""
    r = subprocess.run([str(rt.JDK / "javap"), "-c", "-l", "-p", "-cp", rt.REF_JAR, *(PKG + c for c in classes)],
                       check=True, capture_output=True, text=True, env=rt.child_env())
    out, cls, method, section = {}, None, None, None
    for line in r.stdout.splitlines():
        m = re.match(r"^Compiled from \"(\w+)\.java\"", line)
        if m:
            continue
        m = re.match(r"^(?:[\w ]+ )?(?:class|interface|enum) " + re.escape(PKG) + r"([\w$]+)", line)
        if m:
            cls = m.group(1)
            out[cls] = {}
            continue
        if re.match(r"^  \S.*;$", line):  # a member: a method (its code follows), a field, static {}
            m = METHOD.match(line)
            method, section = None, None
            if m and cls:
                name = "<init>" if m.group(1) == PKG + cls else m.group(1)
                method = ([], [])
                for key in (name, f"{name}({m.group(2)})"):
                    out[cls].setdefault(key, []).append(method)
            continue
        if line.strip() in ("Code:", "LineNumberTable:", "LocalVariableTable:", "Exception table:",
                            "LocalVariableTypeTable:", "StackMapTable:"):
            section = line.strip()
            continue
        if method is None:
            continue
        if section == "Code:":
            m = INSN.match(line)
            if m:
                method[0].append((int(m.group(1)), m.group(2), m.group(3) or ""))
        elif section == "LineNumberTable:":
            m = LINE.match(line)
            if m:
                method[1].append((int(m.group(2)), int(m.group(1))))
    return out


def line_of(table: list[tuple[int, int]], bci: int) -> int:
    return max((start, line) for start, line in table if start <= bci)[1]


def frames(dis: dict) -> list[tuple[str, str, str]]:
    """[(constant, frame text, what it is)], or SystemExit naming the frames that do not resolve."""
    out, errors = [], []
    for const, cls, method, callee, nth in FRAMES:
        found = dis.get(cls, {}).get(method, [])
        if len(found) != 1:
            errors.append(f"{const}: {cls}.{method}: {len(found)} methods (add the parameter types)")
            continue
        insns, table = found[0]
        calls = [bci for bci, op, comment in insns if op.startswith("invoke") and re.search(callee, comment)]
        if (len(calls) != 1) if nth is None else nth >= len(calls):
            errors.append(f"{const}: {cls}.{method} makes {len(calls)} calls matching {callee!r}")
            continue
        name = method.split("(")[0]
        file = cls.split("$")[0]
        text = f"{PKG}{cls}.{name}({file}.java:{line_of(table, calls[nth or 0])})"
        which = "" if nth is None else f" (call {nth + 1} of {len(calls)})"
        out.append((const, text, f"`{cls}.{name}` calling `{callee_name(callee)}`{which}"))
    if errors:
        raise SystemExit("java_frames.py: frames that do not resolve in the reference jar "
                         f"({rt.REF_TAG}):\n  " + "\n  ".join(errors) + "\nupdate FRAMES in crates/cli/tools/java_frames.py")
    return out


def callee_name(callee: str) -> str:
    """A readable callee for the doc comment of a constant."""
    s = re.sub(r"\\(.)", r"\1", callee).replace("Method ", "").replace("InterfaceMethod ", "")
    s = s.split(":")[0].replace('"', "").replace("com/google/javascript/jscomp/", "").replace("/", ".")
    return s.rstrip(".") or callee


def render(rows) -> str:
    """The Rust module, formatted by rustfmt (as `cargo fmt` would leave it)."""
    lines = [HEADER]
    for const, text, what in rows:
        lines.append(f"/// {what}.\npub const {const}: &str = \"{text}\";\n")
    r = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], input="\n".join(lines),
                       capture_output=True, text=True, check=True, cwd=rt.CHECKOUT)
    return r.stdout


def show(cls: str, method: str) -> int:
    dis = disassemble([cls])
    for insns, table in dis.get(cls, {}).get(method, []):
        for bci, op, comment in insns:
            if op.startswith("invoke"):
                print(f"  line {line_of(table, bci)}  bci {bci}  {op} {comment}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="write nothing; exit 1 when java_frames.rs is stale")
    ap.add_argument("--show", nargs=2, metavar=("CLASS", "METHOD"), help="list a method's calls with their lines")
    a = ap.parse_args()
    if a.show:
        return show(*a.show)
    if a.check and not (Path(rt.REF_JAR).is_file() and (rt.JDK / "javap").is_file()):
        print(f"java_frames.py --check: the reference jar of {rt.REF_TAG} or the JDK is not available: skipped")
        return 0
    rows = frames(disassemble(sorted({c for _, c, _, _, _ in FRAMES})))
    text = render(rows)
    old = OUT.read_text() if OUT.exists() else ""
    rel = OUT.relative_to(rt.CHECKOUT)
    if old == text:
        print(f"{rel}: {len(rows)} frames, current for {rt.REF_TAG}")
        return 0
    stale = [f"  {c}: {t}" for c, t, _ in rows if f'"{t}"' not in old]
    if a.check:
        print(f"{rel} is stale for {rt.REF_TAG} (python3 crates/cli/tools/java_frames.py rewrites it):\n"
              + "\n".join(stale or ["  (layout)"]))
        return 1
    OUT.write_text(text)
    print(f"{rel}: rewrote {len(rows)} frames for {rt.REF_TAG}" + ("".join("\n" + s for s in stale)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
