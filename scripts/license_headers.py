#!/usr/bin/env python3
"""Per-file license headers for the Rust port (see NOTICE and LICENSES/README.md).

Every tracked crates/**/*.rs file gets, at its top, the license notice(s) of the code it was
translated from, found through its `// port: Class#member` markers:

  * Closure Compiler (Apache-2.0) or its Rhino-derived files (MPL 1.1 / GPL 2.0+);
  * OpenJDK 21 classes (GPL-2.0 with the Classpath exception, or Apache-2.0 for the Apache
    Xerces code that the JDK bundles in java.xml);
  * Protocol Buffers for Java (BSD-3-Clause), Guava and Gson (Apache-2.0), args4j (MIT);
  * closure-rs' own oracle Java (Apache-2.0, The closure-rs Authors);
  * no upstream source -> an Apache-2.0 header for The closure-rs Authors.

Each origin file's own leading license comment is copied verbatim. Closure Compiler's (and
closure-rs') standard Apache-2.0 headers that differ only in their copyright year are merged into
one block that keeps every copyright line. Any other distinct notice is kept as its own block:
a file with several origins carries every applicable notice. After the notices, one comment line per
upstream project names the ported file(s) and the upstream version.

Markers naming a library class outside that library's own modules (HOME) count only where
TRANSLATED says the Rust code reproduces the library's internals; other such markers name an API
whose documented behaviour is reproduced and add no notice (the report lists them). Tables of
Unicode data taken from JDK 21 also get the Unicode data license notice.

Existing leading license comments of a Rust file (and lines this script wrote before) are replaced,
so the script is idempotent; the report says whether each replaced block was among the new ones.

  python3 scripts/license_headers.py [--apply | --check] [--report FILE] [--files-md FILE]
                                     [--libs DIR] [--explain FILE...]

Inputs (defaults resolve against the main checkout, scripts/paths.py ROOT, as reference/, tools/
and build/ are not tracked; $CLOSURE_RS_ROOT overrides it):
  reference/closure-compiler          Closure Compiler at the selected reference (paths.REF_SRC,
                                      v20261006 by default; scripts/fetch_reference.sh)
  tools/jdk-21/lib/src.zip            the pinned JDK 21 sources (DECISIONS.md D-003)
  protobuf v30.2 source tree          ~/.cache/bazel/_bazel_*/*/external/protobuf+ (Closure's
                                      Bazel build), or --protobuf DIR
  --libs DIR (build/license-sources)  guava-33.4.6-jre-sources.jar, gson-2.9.1-sources.jar,
                                      args4j-2.33-sources.jar (Maven Central; the versions
                                      Closure's MODULE.bazel pins). --apply downloads missing
                                      ones; --check never downloads.

Outputs: --apply rewrites the headers, LICENSES/headers.tsv (the classification: per file the
SHA-256 of its header, its licenses and its origin files) and LICENSES/FILES.md.

--check (gates/ci.sh step 1b) has two modes:
  * with every input present, the full check: it re-classifies every tracked Rust file and fails
    when a header, LICENSES/headers.tsv or LICENSES/FILES.md differs from what --apply writes;
  * without them (a CI runner, a fresh clone), the manifest check: every tracked crates/**/*.rs
    must start with a license block, a file listed in LICENSES/headers.tsv must carry exactly the
    header recorded there, and every listed file must exist. A file missing from the manifest
    passes if it has a license header (reported: only a full run can classify it) and fails
    otherwise. The output says which mode ran and which inputs were missing.
"""
import argparse
import collections
import glob
import os
import re
import subprocess
import sys
import urllib.request
import hashlib
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import paths  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))  # the checkout to check
MAIN = paths.ROOT   # the main checkout, which holds the untracked inputs
MANIFEST = os.path.join(ROOT, 'LICENSES', 'headers.tsv')
FILES_MD = os.path.join(ROOT, 'LICENSES', 'FILES.md')

ap = argparse.ArgumentParser()
ap.add_argument('--apply', action='store_true')
ap.add_argument('--check', action='store_true', help='exit 1 if any header is missing or stale')
ap.add_argument('--report')
ap.add_argument('--files-md', help='write the per-license file lists (LICENSES/FILES.md)')
ap.add_argument('--explain', nargs='*', default=[], help='print the origins of these files')
ap.add_argument('--ref', default=paths.REF_SRC)
ap.add_argument('--jdk-src', default=os.path.join(MAIN, 'tools', 'jdk-21', 'lib', 'src.zip'))
ap.add_argument('--protobuf')
ap.add_argument('--libs', default=os.path.join(MAIN, 'build', 'license-sources'))
ARGS = ap.parse_args()

CLOSURE_COMMIT = paths.REF_COMMIT[:7]
JDK_RELEASE = os.path.join(os.path.dirname(os.path.dirname(ARGS.jdk_src)), 'release')

LIB_JARS = {
    'guava': ('guava-33.4.6-jre-sources.jar',
              'https://repo1.maven.org/maven2/com/google/guava/guava/33.4.6-jre/'),
    'gson': ('gson-2.9.1-sources.jar',
             'https://repo1.maven.org/maven2/com/google/code/gson/gson/2.9.1/'),
    'args4j': ('args4j-2.33-sources.jar', 'https://repo1.maven.org/maven2/args4j/args4j/2.33/'),
}


def jdk_version():
    try:
        for line in open(JDK_RELEASE, encoding='utf-8'):
            if line.startswith('IMPLEMENTOR_VERSION='):
                return line.split('=', 1)[1].strip().strip('"')
    except OSError:
        pass
    return 'JDK 21'


PROJECTS = {
    'closure': 'Closure Compiler (https://github.com/google/closure-compiler), commit '
               + CLOSURE_COMMIT,
    'jdk': 'OpenJDK 21 (the src.zip of ' + jdk_version() + ')',
    'protobuf': 'Protocol Buffers for Java 4.30.2 (https://github.com/protocolbuffers/protobuf)',
    'guava': 'Guava 33.4.6-jre (https://github.com/google/guava)',
    'gson': 'Gson 2.9.1 (https://github.com/google/gson)',
    'args4j': 'args4j 2.33 (https://github.com/kohsuke/args4j)',
    'own': "closure-rs' own Java oracle tooling",
}

# --------------------------------------------------------------------------------------------
# Source access


class Source:
    """A tree of Java (and .proto) sources: a directory or a zip/jar."""

    def __init__(self, project, base, prefixes=('',), display_prefix=''):
        self.project, self.base, self.display_prefix = project, base, display_prefix
        self.zip = zipfile.ZipFile(base) if os.path.isfile(base) else None
        self.files = []
        if self.zip:
            for n in self.zip.namelist():
                if n.endswith(('.java', '.proto')) and n.startswith(prefixes):
                    self.files.append(n)
        else:
            for pre in prefixes:
                for dp, _, fs in os.walk(os.path.join(base, pre)):
                    for f in fs:
                        if f.endswith(('.java', '.proto')):
                            self.files.append(os.path.relpath(os.path.join(dp, f), base))
        self._cache = {}

    def read(self, rel):
        if rel not in self._cache:
            if self.zip:
                data = self.zip.read(rel)
            else:
                with open(os.path.join(self.base, rel), 'rb') as fh:
                    data = fh.read()
            # line endings are not content (some Gson sources use CRLF)
            self._cache[rel] = data.decode('utf-8', errors='replace').replace('\r\n', '\n')
        return self._cache[rel]

    def display(self, rel):
        return self.display_prefix + rel


def strip_comments_and_strings(s):
    s = re.sub(r'/\*.*?\*/', ' ', s, flags=re.S)
    s = re.sub(r'//[^\n]*', ' ', s)
    s = re.sub(r'"(?:\\.|[^"\\\n])*"', '""', s)
    return s


TYPE_DECL = re.compile(r'\b(?:class|interface|enum|record|@interface)\s+([A-Z][A-Za-z0-9_$]*)')
PROTO_DECL = re.compile(r'\b(?:message|enum)\s+([A-Z][A-Za-z0-9_]*)')


class Index:
    def __init__(self):
        self.top = collections.defaultdict(list)     # simple name -> [(src, rel)]
        self.nested = collections.defaultdict(list)  # nested simple name -> [(src, rel)]
        self.fqn = {}                                # pkg.Top -> (src, rel)

    def add_source(self, src):
        for rel in src.files:
            text = src.read(rel)
            body = strip_comments_and_strings(text)
            m = re.search(r'^\s*(?:package|option\s+java_package\s*=)\s*"?([\w.]+)"?\s*;', body,
                          re.M)
            pkg = m.group(1) if m else ''
            if rel.endswith('.java'):
                top = os.path.basename(rel)[:-5]
                self.top[top].append((src, rel))
                self.fqn[pkg + '.' + top] = (src, rel)
                for n in set(TYPE_DECL.findall(body)) - {top}:
                    self.nested[n].append((src, rel))
            else:  # .proto: messages/enums are generated Java classes of java_package
                names = PROTO_DECL.findall(body)
                depth_top = top_level_proto_names(body)
                outer = re.search(r'java_outer_classname\s*=\s*"(\w+)"', body)
                if outer:
                    depth_top.add(outer.group(1))
                for n in set(names):
                    (self.top if n in depth_top else self.nested)[n].append((src, rel))
                for n in depth_top:
                    self.fqn[pkg + '.' + n] = (src, rel)


def top_level_proto_names(body):
    out, depth = set(), 0
    for tok in re.finditer(r'[{}]|\b(?:message|enum)\s+([A-Z]\w*)', body):
        t = tok.group(0)
        if t == '{':
            depth += 1
        elif t == '}':
            depth -= 1
        elif depth == 0:
            out.add(tok.group(1))
    return out


def find_protobuf():
    if ARGS.protobuf:
        return ARGS.protobuf if os.path.isdir(ARGS.protobuf) else None
    cands = glob.glob(os.path.expanduser('~/.cache/bazel/_bazel_*/*/external/protobuf+'))
    cands = [c for c in cands if os.path.isfile(
        os.path.join(c, 'java/core/src/main/java/com/google/protobuf/CodedInputStream.java'))]
    return cands[0] if cands else None


def lib_jar(name):
    fname, url = LIB_JARS[name]
    path = os.path.join(ARGS.libs, fname)
    if not os.path.isfile(path):
        os.makedirs(ARGS.libs, exist_ok=True)
        print(f'downloading {url + fname}', file=sys.stderr)
        urllib.request.urlretrieve(url + fname, path)
    return path


def missing_inputs():
    """The upstream sources that are not available (an empty list: all of them are)."""
    missing = []
    if not os.path.isdir(os.path.join(ARGS.ref, 'src')):
        missing.append(ARGS.ref)
    if not os.path.isfile(ARGS.jdk_src):
        missing.append(ARGS.jdk_src)
    if not find_protobuf():
        missing.append(ARGS.protobuf or 'protobuf v30.2 sources (--protobuf DIR)')
    for fname, _ in LIB_JARS.values():
        if not os.path.isfile(os.path.join(ARGS.libs, fname)):
            missing.append(os.path.join(ARGS.libs, fname))
    return missing


def own_java_files():
    """closure-rs' own Java: the oracle, and the probes/dumpers under crates/ and scripts/."""
    out = subprocess.check_output(['git', '-C', ROOT, 'ls-files', '*.java'], text=True)
    return [l for l in out.splitlines() if l.endswith('.java')]


def patch_added_files():
    """Java files that oracle/patches add to the recording build (closure-rs' own code)."""
    added = {}
    for p in sorted(glob.glob(os.path.join(ROOT, 'oracle', 'patches', '*.patch'))):
        text = open(p, encoding='utf-8').read()
        for m in re.finditer(r'^diff --git a/(\S+) b/\S+\nnew file mode[^\n]*\n(?:[^\n]*\n)*?\+\+\+ '
                             r'b/\S+\n@@[^\n]*\n((?:\+[^\n]*\n)+)', text, re.M):
            if m.group(1).endswith('.java'):
                body = ''.join(l[1:] + '\n' for l in m.group(2).splitlines())
                added[m.group(1)] = (os.path.relpath(p, ROOT), body)
    return added


class OwnSource(Source):
    def __init__(self):
        self.project, self.base, self.display_prefix, self.zip = 'own', ROOT, '', None
        self.files, self._cache = [], {}
        for rel in own_java_files():
            self.files.append(rel)
        for rel, (patch, body) in patch_added_files().items():
            key = patch + ':' + rel
            self.files.append(key)
            self._cache[key] = body

    def display(self, rel):
        if ':' in rel:
            patch, f = rel.split(':', 1)
            return f'{os.path.basename(f)} ({patch})'
        return rel


SOURCES, INDEX = {}, {}
# The oracle's *_Helpers classes hold methods copied VERBATIM from Closure's tests; a port of one
# of them is a port of that Closure test file (named in the helper's header comment).
HELPER_ORIGINS = {}


def index_sources():
    protobuf = find_protobuf()
    if not os.path.isdir(os.path.join(ARGS.ref, 'src')) or not os.path.isfile(ARGS.jdk_src) \
            or not protobuf:
        sys.exit('upstream sources missing: ' + ', '.join(m for m in missing_inputs()
                                                            if not m.endswith('.jar')))
    print('indexing sources...', file=sys.stderr)
    SOURCES.update({
        'closure': [Source('closure', ARGS.ref, ('src', 'test'))],
        'jdk': [Source('jdk', ARGS.jdk_src)],
        'protobuf': [Source('protobuf', protobuf, ('java/core/src/main/java',))],
        'guava': [Source('guava', lib_jar('guava'))],
        'gson': [Source('gson', lib_jar('gson'))],
        'args4j': [Source('args4j', lib_jar('args4j'))],
        'own': [OwnSource()],
    })
    for proj, srcs in SOURCES.items():
        idx = Index()
        for s in srcs:
            idx.add_source(s)
        INDEX[proj] = idx
    for rel in SOURCES['own'][0].files:
        if rel.endswith(('_Helpers.java', 'TestHelpers.java', '_Compile.java')):
            head = SOURCES['own'][0].read(rel)[:4000]
            m = re.match(r'\s*/\*.*?\*/', head, re.S)
            refs = re.findall(r'\b((?:src|test)/com/google/[\w/]+\.java)', m.group(0) if m else '')
            HELPER_ORIGINS[rel] = [r for r in dict.fromkeys(refs)
                                   if os.path.isfile(os.path.join(ARGS.ref, r))]

# --------------------------------------------------------------------------------------------
# License comments of origin files


def leading_comments(text, proto=False):
    """The comment blocks before `package` (Java) or the first statement (.proto), verbatim."""
    blocks, i, n = [], 0, len(text)
    while True:
        while i < n and text[i] in ' \t\r\n﻿':
            i += 1
        if text.startswith('/*', i):
            j = text.find('*/', i + 2)
            if j < 0:
                break
            blocks.append(text[i:j + 2])
            i = j + 2
        elif text.startswith('//', i):
            j = i
            lines = []
            while text.startswith('//', j):
                e = text.find('\n', j)
                e = n if e < 0 else e
                lines.append(text[j:e].rstrip())
                j = e + 1
                k = j
                while k < n and text[k] in ' \t':
                    k += 1
                if not text.startswith('//', k):
                    break
                j = k
            blocks.append('\n'.join(lines))
            i = j
        else:
            break
    return blocks


LICENSE_WORDS = re.compile(r'Copyright|License|LICENSE|licensed|DO NOT REMOVE OR ALTER|'
                           r'Permission (?:is hereby|to use)', re.I)

ARGS4J_MIT = """/*
 * Copyright (c) 2013 Kohsuke Kawaguchi and other contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and associated documentation files (the "Software"), to deal in
 * the Software without restriction, including without limitation the rights to
 * use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is furnished to do
 * so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */"""

APACHE_BODY = """ * Licensed under the Apache License, Version 2.0 (the "License");
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
 */"""

OWN_HEADER = '/*\n * Copyright 2026 The closure-rs Authors.\n *\n' + APACHE_BODY
CLOSURE_NO_HEADER = '/*\n * Copyright The Closure Compiler Authors.\n *\n' + APACHE_BODY


def origin_notices(proj, src, rel):
    """[(kind, block)] for one origin file, plus a note when the notice is not the file's own."""
    if proj == 'args4j':  # args4j's sources carry no header; its LICENSE is MIT
        return [('mit', ARGS4J_MIT)], 'args4j LICENSE'
    if proj == 'own':
        return [('apache-own', OWN_HEADER)], None
    text = src.read(rel)
    blocks = [b for b in leading_comments(text, rel.endswith('.proto'))]
    lic = [b for b in blocks if LICENSE_WORDS.search(b)]
    if not lic:
        if proj == 'closure':
            return [('apache', CLOSURE_NO_HEADER)], 'no license comment; Closure LICENSE'
        raise SystemExit(f'no license comment in {proj}:{rel}')
    # keep every leading license block (JDK's Xerces files start with a "reserved comment block /
    # DO NOT REMOVE OR ALTER!" block, DToA.java has two notices) but not class descriptions
    keep = [b for b in blocks if LICENSE_WORDS.search(b) or 'reserved comment block' in b]
    if rel.endswith('.proto'):
        keep = lic[:1]
        if proj == 'closure' and 'Apache' not in keep[0]:
            # mapping.proto: "Copyright 2009 Google Inc." without a license grant
            return ([(kind_of(keep[0]), keep[0]), ('apache', CLOSURE_NO_HEADER)],
                    'copyright line only; Closure LICENSE')
    return [(kind_of(b), b) for b in keep], None


def kind_of(block):
    if 'Mozilla Public License' in block:
        return 'mpl'
    if 'GNU General Public License version 2 only' in block:
        return 'gpl-cpe' if '"Classpath" exception' in block else 'gpl-only'
    if 'Licensed to the Apache Software Foundation' in block:
        return 'apache-asf'
    if 'Apache License' in block:
        return 'apache'
    if 'BSD-style' in block:
        return 'bsd'
    if 'David M. Gay' in block:
        return 'dtoa'
    if 'reserved comment block' in block:
        return 'reserved'
    if 'This file is available under and governed by the GNU General Public' in block:
        return 'jsr166-note'      # JDK's JSR-166 files: GPL-2.0 (+CPE) governs, see the block
    if 'public domain' in block.lower():
        return 'public-domain'
    return 'copyright'


# --------------------------------------------------------------------------------------------
# Markers -> origins

REF_HEAD = re.compile(r'\s*((?:[a-z_][a-z0-9_]*\.)*)([A-Z][A-Za-z0-9_$]*(?:\.[A-Z][A-Za-z0-9_$]*)*)')
REF_HASH = re.compile(r'(?<![\w.#$])((?:[a-z_][a-z0-9_]*\.)*)([A-Z][A-Za-z0-9_$]*'
                      r'(?:\.[A-Z][A-Za-z0-9_$]*)*)#')
MARKER = re.compile(r'(?://|/\*|^\s*\*)[^\n]*?\bport:([^\n]*)', re.M)

# Markers naming closure-rs' own differential tests and fixture dumpers (Java programs kept
# outside the repository, under build/), or plain words: no upstream source.
NOT_A_CLASS = {'UnixPathTest', 'URITest', 'MatcherTest', 'SourcemapDiffDriver', 'OptionsStringsDump',
               'OptionsConfigurationsDump', 'PatternExpectationsDump', 'Unported', 'JDK', 'Java',
               'Rust', 'JS', 'JSON', 'I', 'N', 'T'}

# Test-library assertions whose markers name the API semantics the Rust test helper reimplements
# (Truth subjects, JUnit's TemporaryFolder, Guava testlib's EqualsTester); no library source is
# translated, so they add no notice. Each marker was read.
TEST_API_ONLY = {'Truth', 'Correspondence', 'IterableSubject', 'BooleanSubject', 'DoubleSubject',
                 'Subject', 'TemporaryFolder', 'EqualsTester'}

# Simple names that the generic lookup would resolve to the wrong library class.
CLASS_OVERRIDES = {
    'Descriptor': ('protobuf', 'java/core/src/main/java/com/google/protobuf/Descriptors.java'),
    'FieldDescriptor': ('protobuf', 'java/core/src/main/java/com/google/protobuf/Descriptors.java'),
    'Logger': ('jdk', 'java.logging/java/util/logging/Logger.java'),
    'Files': ('jdk', 'java.base/java/nio/file/Files.java'),
    'UnixFileSystem': ('jdk', 'java.base/sun/nio/fs/UnixFileSystem.java'),
    'Supplier': ('jdk', 'java.base/java/util/function/Supplier.java'),
}
FILE_CLASS_OVERRIDES = {   # the SAX API is org.xml.sax (java.xml), not java.net/jdk.internal
    'crates/rhino/src/java_lang/sax_parser.rs': {
        'ContentHandler': ('jdk', 'java.xml/org/xml/sax/ContentHandler.java'),
        'Attributes': ('jdk', 'java.xml/org/xml/sax/Attributes.java'),
        'SAXParseException': ('jdk', 'java.xml/org/xml/sax/SAXParseException.java'),
        'XMLReader': ('jdk', 'java.xml/org/xml/sax/XMLReader.java'),
    },
    # the file header names com.sun.org.apache.xerces.internal.util.XMLChar (not Xalan's copy)
    'crates/rhino/src/java_lang/xml_char.rs': {
        'XMLChar': ('jdk', 'java.xml/com/sun/org/apache/xerces/internal/util/XMLChar.java'),
    },
}

# Markers naming an upstream interface that closure-rs' own code implements (no upstream code).
FILE_API_ONLY = {
    # ParseDump (closure-rs' oracle tool) implements rhino's ErrorReporter interface
    'crates/parsing/src/bin/parse_dump.rs': {'ErrorReporter'},
    # java.util.regex.Matcher#replaceFirst, called (not Closure's refactoring.Matcher)
    'crates/jscomp/src/js_message_visitor.rs': {'Matcher'},
    # FlowScope declares the abstract methods of rhino's StaticTypedScope interface, and FlowSlot
    # dispatches StaticTypedSlot's abstract getters to its two variants; neither interface's
    # default method (lookupQualifiedName) is ported here (it is in typed_scope_creator_rhino.rs)
    'crates/jscomp/src/flow_scope.rs': {'StaticTypedScope', 'StaticTypedSlot'},
    # the Ast field that stores JSDocSerializer's placeholderType static; JSDocSerializer's code
    # is in crates/jscomp/src/serialization/jsdoc_serializer.rs
    'crates/rhino/src/node.rs': {'JSDocSerializer'},
}

# Library code. In a library's own modules (HOME) every
# marker naming that library is a translation. Elsewhere a marker naming a library class counts
# as a translation only where TRANSLATED says so: the Rust code there reproduces the library's
# internals (private helpers, hash/bucket orders, tables, constants, internal message formats).
# Every other such marker names a public API whose documented behaviour the Rust code reproduces
# (Object#toString, Enum#name, String#trim, Throwable#getMessage, ImmutableMap.Builder#put, ...);
# those add no notice and are listed in the report's "API-only" column. Decided by reading each
# marked function (2026-10-08).
HOME = {
    'jdk': ('crates/rhino/src/java_lang/', 'crates/rhino/src/java_lang.rs',
            'crates/rhino/src/java_util/', 'crates/sourcemap/src/java_string.rs',
            'crates/sourcemap/src/java_math.rs', 'crates/sourcemap/src/java_character_case.rs',
            'crates/cli/src/jdk_globs.rs'),
    'args4j': ('crates/cli/src/args4j/',),
    'gson': ('crates/cli/src/gson/', 'crates/sourcemap/src/gson/'),
    'protobuf': ('crates/jscomp/src/serialization/protobuf.rs', 'crates/jscomp/src/protobuf/'),
    'guava': ('crates/rhino/src/common_hash/', 'crates/jscomp/src/guava_ascii.rs',
              'crates/resources/src/guava.rs'),
    'closure': ('crates/',),
    'own': ('crates/',),
}
TRANSLATED = {
    # The OpenJDK internals of a Closure/Rhino/protobuf port live in a sibling `*_jdk.rs` module,
    # so that every file carries the notices of one license family.
    'crates/cli/src/abstract_command_line_runner_jdk.rs': {'Throwable'},  # printEnclosedStackTrace
    'crates/jscomp/src/lint/check_no_mutated_es6_exports.rs': {'HashMultimap', 'Maps'},
    'crates/jscomp/src/lint/check_no_mutated_es6_exports_jdk.rs': {'HashMap'},
    'crates/jscomp/src/remove_unused_code.rs': {'HashMultimap', 'AbstractMapBasedMultimap'},
    'crates/jscomp/src/remove_unused_code_jdk.rs': {'HashMap'},
    'crates/jscomp/src/serialization/fast_gzip_output_stream_jdk.rs': {
        'GZIPOutputStream', 'DeflaterOutputStream', 'Deflater'},
    'crates/jscomp/src/protobuf/text_format_jdk.rs': {'Long'},  # parseLong's NumberFormatException text
    'crates/rhino/src/dtoa/d_to_a_jdk.rs': {'BigInteger'},     # multiplyToLen, shiftLeft, checkRange
    'crates/testing/src/proto_neutral_jdk.rs': {'String', 'Base64'},  # String#isMalformed3 etc.
    'crates/regex/tests/jdk/java_util_random.rs': {'Random'},  # Random#initialScramble / #next
    'crates/resources/src/jar.rs': {'Class', 'ZipFile'},   # Class#resolveName, Source#getEntryPos
    'crates/jscomp/src/variable_map.rs': {'Hashing', 'ImmutableMap', 'JdkBackedImmutableBiMap',
                                          'JdkBackedImmutableMap'},
    'crates/jscomp/src/serialization/js_type_color_id_hasher.rs': {
        'AbstractHasher', 'AbstractNonStreamingHashFunction', 'HashCode', 'Hashing'},
    'crates/jscomp/src/source_map_resolver.rs': {'BaseEncoding'},
    'crates/jscomp/src/transpile/transpile_result.rs': {'PercentEscaper', 'UnicodeEscaper'},
    'crates/jscomp/src/js_message.rs': {'CaseFormat', 'Ascii'},
    'crates/jscomp/src/deps/js_file_line_parser.rs': {'CharMatcher'},  # Whitespace TABLE/SHIFT
    'crates/rhino/src/jscomp_base/mod.rs': {'Strings', 'Preconditions'},  # Strings#lenientFormat
    'crates/jscomp/src/diagnostic/logs_gson.rs': {'Gson', 'TypeAdapter', 'TypeAdapters',
                                                  'CollectionTypeAdapterFactory',
                                                  'MapTypeAdapterFactory'},
    'crates/jscomp/src/disambiguate/disambiguate_properties.rs': {'Gson'},
    'crates/jscomp/src/serialization/serialize_types_to_pointers.rs': {'Gson'},
}

# Files without `port:` markers whose origin is known otherwise (generated data, constant-only
# files). (project, path in that project's source tree)
EXTRA_ORIGINS = {
    'crates/rhino/src/java_lang/character_data.rs': [('jdk', 'java.base/java/lang/Character.java')],
    'crates/sourcemap/src/java_character_case.rs': [('jdk', 'java.base/java/lang/Character.java')],
    'crates/sourcemap/src/gson/stream/json_scope.rs': [('gson', 'com/google/gson/stream/JsonScope.java')],
}

# Tables generated from JDK 21's java.lang.Character, i.e. Unicode Character Database 15.0
# properties: they also carry the Unicode data license notice (legal/java.base/unicode.md).
UNICODE_DATA = {
    'crates/rhino/src/java_lang/character_data.rs', 'crates/rhino/src/java_lang/character_bmp_data.rs',
    'crates/rhino/src/java_lang/character_case_data.rs', 'crates/rhino/src/java_lang/regex_categories.rs',
    'crates/sourcemap/src/java_character_case.rs',
}
UNICODE_NOTICE = """/*
 * Copyright © 1991-2022 Unicode, Inc. All rights reserved.
 * Distributed under the Terms of Use in https://www.unicode.org/copyright.html.
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of the Unicode data files and any associated documentation
 * (the "Data Files") or Unicode software and any associated documentation
 * (the "Software") to deal in the Data Files or Software
 * without restriction, including without limitation the rights to use,
 * copy, modify, merge, publish, distribute, and/or sell copies of
 * the Data Files or Software, and to permit persons to whom the Data Files
 * or Software are furnished to do so, provided that either
 * (a) this copyright and permission notice appear with all copies
 * of the Data Files or Software, or
 * (b) this copyright and permission notice appear in associated
 * Documentation.
 *
 * THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF
 * ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
 * WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT OF THIRD PARTY RIGHTS.
 * IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS
 * NOTICE BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL
 * DAMAGES, OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE,
 * DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
 * TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
 * PERFORMANCE OF THE DATA FILES OR SOFTWARE.
 *
 * Except as contained in this notice, the name of a copyright holder
 * shall not be used in advertising or otherwise to promote the sale,
 * use or other dealings in these Data Files or Software without prior
 * written authorization of the copyright holder.
 */"""
UNICODE_LINE = ('// The tables are data of the Unicode Character Database 15.0 as JDK 21\'s '
                'java.lang.Character\n// reports it; the Unicode license notice above applies '
                'to them (LICENSES/Unicode-DFS-2016.txt).')

# Marker-less stand-ins: they name a Closure class but translate none of its code.
STAND_IN = re.compile(r'^// STAND-IN', re.M)


def marker_refs(text):
    for m in MARKER.finditer(text):
        body = m.group(1)
        if '(static import)' in body:     # a wrapper calling the imported method, not a port
            continue
        refs = []
        h = REF_HEAD.match(body)
        if h:
            refs.append((h.group(1), h.group(2)))
        for r in REF_HASH.finditer(body):
            if (r.group(1), r.group(2)) not in refs:
                refs.append((r.group(1), r.group(2)))
        for r in refs:
            yield r


def priority(path):
    parts = path.split('/')
    if 'java_lang' in parts or 'java_util' in parts or path.startswith(HOME['jdk']):
        return ['jdk', 'closure', 'own', 'protobuf', 'gson', 'args4j', 'guava']
    if 'args4j' in parts:
        return ['args4j', 'closure', 'own', 'jdk', 'protobuf', 'gson', 'guava']
    if 'gson' in parts:
        return ['gson', 'closure', 'own', 'jdk', 'protobuf', 'args4j', 'guava']
    return ['closure', 'own', 'protobuf', 'gson', 'args4j', 'guava', 'jdk']


CRATE_HINT = {
    'rhino': 'javascript/rhino/', 'jstype': 'rhino/jstype', 'parsing': 'jscomp/parsing',
    'sourcemap': 'debugging/sourcemap', 'regex': 'jscomp/regex', 'cli': 'jscomp/',
}


def dir_hint(path):
    """`crates/rhino/src/jscomp_parsing_parser/util/x.rs` -> 'jscomp/parsing/parser/util'."""
    parts = path.split('/')[3:-1]
    return '/'.join(parts).replace('_', '/')


def pick(cands, path):
    if len(cands) == 1:
        return cands[0]
    crate = path.split('/')[1]
    hint = CRATE_HINT.get(crate, 'jscomp/')
    dh = dir_hint(path)
    is_test = '/tests/' in path or path.endswith('_test.rs')

    def suffix_match(rel):
        d = os.path.dirname(rel.split(':')[-1]).replace('_', '/')
        n = 0
        segs = dh.split('/') if dh else []
        for k in range(len(segs), 0, -1):
            if d.endswith('/'.join(segs[-k:])) or ('/'.join(segs[:k]) and '/'.join(segs[:k]) in d):
                n = k
                break
        return n

    def score(c):
        rel = c[1]
        return (-suffix_match(rel), hint not in rel,
                rel.startswith('test/') != is_test if rel.startswith(('src/', 'test/')) else False,
                not rel.startswith(('src/', 'java.base/', 'java/core', 'com/', 'org/')),
                '/jdk/internal/' in rel or '/sun/' in rel, len(rel), rel)

    ranked = sorted(cands, key=score)
    if c_kind(ranked[0]) != c_kind(ranked[1]) and score(ranked[0])[:3] == score(ranked[1])[:3]:
        AMBIGUOUS[path].add(f'{ranked[0][1]} over {ranked[1][1]}')
    return ranked[0]


AMBIGUOUS = collections.defaultdict(set)


def c_kind(c):
    src, rel = c
    if src.project != 'closure':
        return src.project
    return kind_of(leading_lic('closure', rel))


def resolve(pkg, chain, path):
    chain = chain.split('$')[0]                       # Outer$Inner, Outer$1 (anonymous)
    if chain.startswith('AutoValue_'):                # AutoValue_Outer_Inner -> Outer
        chain = chain[len('AutoValue_'):].split('_')[0]
    names = chain.split('.')
    if names[0] in NOT_A_CLASS:
        return ('ignored', None, names[0])
    if names[0] in TEST_API_ONLY:
        return ('api-only', None, names[0])
    if pkg:
        fq = pkg.rstrip('.')
        for proj, idx in INDEX.items():
            if fq + '.' + names[0] in idx.fqn:
                return (proj,) + idx.fqn[fq + '.' + names[0]]
        for proj in priority(path):                   # a partial package (`spi.Messages`)
            hits = [v for k, v in INDEX[proj].fqn.items() if k.endswith('.' + fq + '.' + names[0])]
            if hits:
                return (proj,) + pick(hits, path)
        # qualified name of something we do not index
        return ('unresolved-qualified', None, pkg + chain)
    order = priority(path)
    if names[0] in FILE_API_ONLY.get(path, ()):
        return ('api-only', None, names[0])
    fo = FILE_CLASS_OVERRIDES.get(path, {}).get(names[0])
    if fo:
        return (fo[0], src_of(fo[0]), fo[1])
    c = INDEX['closure'].top.get(names[0]) or INDEX['own'].top.get(names[0])
    if names[0] in CLASS_OVERRIDES and not (c and order[0] in ('closure', 'own')):
        o = CLASS_OVERRIDES[names[0]]
        return (o[0], src_of(o[0]), o[1])
    for proj in order:
        c = INDEX[proj].top.get(names[0])
        if c:
            return (proj,) + pick(c, path)
    for proj in order:
        c = INDEX[proj].nested.get(names[0])
        if c:
            return (proj,) + pick(c, path)
    # flattened nested proto messages (`TypePoolDebugInfo` for TypePool.DebugInfo)
    words = re.findall(r'[A-Z][a-z0-9]*', names[0])
    for k in range(len(words) - 1, 0, -1):
        c = [x for x in INDEX['closure'].top.get(''.join(words[:k]), []) if x[1].endswith('.proto')]
        if c:
            return ('closure',) + pick(c, path)
    return None


def git_files():
    out = subprocess.check_output(['git', '-C', ROOT, 'ls-files', 'crates/*.rs'], text=True)
    return [l for l in out.splitlines() if l]


# --------------------------------------------------------------------------------------------
# Existing headers

def split_existing_header(text):
    """(removed blocks, rest) for the leading license comments and lines this script wrote."""
    blocks, i, n = [], 0, len(text)
    while True:
        j = i
        while j < n and text[j] in ' \t\r\n':
            j += 1
        if text.startswith('/*', j) and not text.startswith('/*!', j) and \
                not (text.startswith('/**', j) and not text.startswith(('/***', '/**/'), j)):
            depth, k = 0, j
            while k < n:
                if text.startswith('/*', k):
                    depth += 1
                    k += 2
                elif text.startswith('*/', k):
                    depth -= 1
                    k += 2
                    if depth == 0:
                        break
                else:
                    k += 1
            blk = text[j:k]
            if not HEADER_WORDS.search(blk):
                break
            blocks.append(blk)
            i = k
        elif text.startswith('//', j) and not text.startswith(('///', '//!'), j):
            k, lines = j, []
            while text.startswith('//', k) and not text.startswith(('///', '//!'), k):
                e = text.find('\n', k)
                e = n if e < 0 else e
                lines.append(text[k:e])
                k = e + 1
            blk = '\n'.join(lines)
            if not (HEADER_WORDS.search(blk) or blk.startswith(GENERATED_PREFIXES)):
                break
            blocks.append(blk)
            i = k
        else:
            break
    rest = text[i:].lstrip('\r\n')
    return blocks, rest


# What marks a leading comment of a Rust file as a license notice (stricter than LICENSE_WORDS,
# so that a module comment merely mentioning "license" stays).
HEADER_WORDS = re.compile(r'Copyright|Licensed under|Licensed to the Apache|BEGIN LICENSE BLOCK|'
                          r'Use of this source code is governed|DO NOT (?:ALTER|REMOVE)|'
                          r'governed by the GNU General Public|Permission is hereby granted|'
                          r'Permission to use, copy|reserved comment block')
GENERATED_PREFIXES = ('// Ported from ', '// Rust port of ', '// No upstream source:',
                      '// (Notice of ', '//   ')


def norm(s):
    """Comment text without decoration and line wrapping."""
    s = re.sub(r'^\s*(?:/\*+|\*+/|\*+|//)', ' ', s, flags=re.M)
    s = s.replace('*/', ' ')
    return re.sub(r'\s+', ' ', s).strip()


# --------------------------------------------------------------------------------------------
# Header assembly

APACHE_STD = re.compile(r'^/\*\n((?: \* Copyright [^\n]*\n)+) \*\n( \* Licensed under the Apache '
                        r'License, Version 2\.0.*\*/)$', re.S)


def merge_blocks(notices):
    """notices: [(kind, block)] in order -> [(kind, block)] merged/deduplicated."""
    out, apache_groups = [], {}
    for kind, b in notices:
        m = APACHE_STD.match(b)
        if m and kind in ('apache', 'apache-own'):
            key = (kind, m.group(2))
            if key in apache_groups:
                pos = apache_groups[key]
                lines = out[pos][2]
                for l in m.group(1).splitlines():
                    if l not in lines:
                        lines.append(l)
                continue
            apache_groups[key] = len(out)
            out.append([kind, key[1], m.group(1).splitlines()])
            continue
        if any(o[0] == kind and len(o) == 2 and o[1] == b for o in out):
            continue
        out.append([kind, b])
    res = []
    for o in out:
        if len(o) == 3:
            kind, body, lines = o

            def year(l):
                y = re.search(r'\d{4}', l)
                return y.group(0) if y else '0'
            lines = sorted(lines, key=year)
            res.append((kind, '/*\n' + '\n'.join(lines) + '\n *\n' + body))
        else:
            res.append((o[0], o[1]))
    return res


def wrap_list(prefix, items, width=100):
    lines, cur = [], prefix
    for k, it in enumerate(items):
        piece = it + (',' if k < len(items) - 1 else '.')
        if len(cur) + 1 + len(piece) > width:
            lines.append(cur)
            cur = '//   ' + piece
        else:
            cur = cur + ' ' + piece
    lines.append(cur)
    return lines


LICENSE_NAME = {
    'apache': 'Apache-2.0', 'apache-own': 'Apache-2.0', 'apache-asf': 'Apache-2.0',
    'mpl': 'MPL-1.1 OR GPL-2.0-or-later', 'gpl-cpe': 'GPL-2.0-only WITH Classpath-exception-2.0',
    'gpl-only': 'GPL-2.0-only', 'bsd': 'BSD-3-Clause', 'mit': 'MIT', 'dtoa': 'dtoa',
    'reserved': None, 'copyright': None, 'public-domain': 'public domain', 'jsr166-note': None,
    'unicode': 'Unicode-DFS-2016',
}


def classify(path, text):
    origins = collections.OrderedDict()   # (proj, rel) -> count
    unresolved = collections.Counter()
    jdk_api = collections.Counter()
    for pkg, chain in marker_refs(text):
        r = resolve(pkg, chain, path)
        if r is None:
            unresolved[pkg + chain] += 1
            continue
        if r[0] == 'unresolved-qualified':
            unresolved[r[2]] += 1
            continue
        if r[0] == 'api-only':
            jdk_api[r[2]] += 1
            continue
        if r[0] == 'ignored':
            continue
        proj, src, rel = r
        top = chain.split('$')[0].split('.')[0]
        if proj not in ('closure', 'own') and not path.startswith(HOME[proj]) \
                and top not in TRANSLATED.get(path, ()):
            jdk_api[f'{proj}:{top}'] += 1
            continue
        if proj == 'own' and rel in HELPER_ORIGINS and HELPER_ORIGINS[rel]:
            for t in HELPER_ORIGINS[rel]:
                key = ('closure', t)
                origins[key] = origins.get(key, 0) + 1
            continue
        origins[(proj, rel)] = origins.get((proj, rel), 0) + 1
        NAMES[(path, proj, rel)].add(chain.split('$')[0])
    for proj, rel in EXTRA_ORIGINS.get(path, ()):
        origins[(proj, rel)] = origins.get((proj, rel), 0) + 1
    if not origins and not STAND_IN.search(text):
        origins.update(markerless_origins(path, text))
    return origins, unresolved, jdk_api


def snake(name):
    return re.sub(r'(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])', '_', name).lower()


def markerless_origins(path, text):
    """A file without markers ports the class it is named after (`js_doc_token.rs` defines
    `JsDocToken`), or the Java file its module doc names (`//! Port of x/YTest.java.`)."""
    out = collections.OrderedDict()
    stem = os.path.basename(path)[:-3]
    for t in re.findall(r'^\s*pub(?:\([a-z]+\))? (?:struct|enum|trait|type) ([A-Z]\w*)', text, re.M):
        if snake(t) == stem:
            r = resolve('', t, path)
            if r and r[0] in INDEX and path.startswith(HOME[r[0]]):
                out[(r[0], r[2])] = 1
    for m in re.finditer(r'^//! (?:Faithful )?[Pp]ort of `?([\w/]+\.java)`?', text, re.M):
        cands = [rel for rel in SOURCES['closure'][0].files if rel.endswith('/' + m.group(1))]
        if len(cands) == 1:
            out[('closure', cands[0])] = 1
    return out


NAMES = collections.defaultdict(set)
STALE_FILES = []


def src_of(proj):
    return SOURCES[proj][0]


PROJECT_ORDER = ['closure', 'jdk', 'protobuf', 'guava', 'gson', 'args4j', 'own']


def build_header(origins, path=''):
    """(header text, set of license kinds, notes)"""
    if not origins:
        return OWN_HEADER + '\n', {'apache-own'}, []
    by_proj = collections.OrderedDict()
    for (proj, rel), cnt in sorted(origins.items(), key=lambda kv: -kv[1]):
        by_proj.setdefault(proj, []).append(rel)
    notices, notes = [], []
    # notices of the main upstream project first, MPL before Apache inside Closure
    def weight(p):   # the main origin first
        return (-sum(origins[(p, r)] for r in by_proj[p]), PROJECT_ORDER.index(p))
    for proj in sorted(by_proj, key=weight):
        rels = by_proj[proj]
        if proj == 'closure':
            rels = sorted(rels, key=lambda r: (kind_of(leading_lic(proj, r)) != 'mpl', ))
        for rel in rels:
            ns, note = origin_notices(proj, src_of(proj), rel)
            if note:
                notes.append(f'{rel}: {note}')
            notices.extend(ns)
    if path in UNICODE_DATA:
        notices.append(('unicode', UNICODE_NOTICE))
    merged = merge_blocks(notices)
    kinds = {k for k, _ in merged}
    lines = []
    for proj in sorted(by_proj, key=weight):
        rels = sorted(src_of(proj).display(r) for r in by_proj[proj])
        lines += wrap_list(f'// Ported from {PROJECTS[proj]}:', rels)
    if path in UNICODE_DATA:
        lines.append(UNICODE_LINE)
    gap = '\n' if merged[-1][1].startswith('//') else ''   # after a `//` notice (protobuf's)
    text = '\n'.join(b for _, b in merged) + '\n' + gap + '\n'.join(lines) + '\n'
    return text, kinds, notes


_lic_cache = {}


def leading_lic(proj, rel):
    if (proj, rel) not in _lic_cache:
        bl = [b for b in leading_comments(src_of(proj).read(rel)) if LICENSE_WORDS.search(b)]
        _lic_cache[(proj, rel)] = bl[0] if bl else ''
    return _lic_cache[(proj, rel)]


def check_rust_comment(block):
    """A Java block comment must stay one Rust block comment (Rust block comments nest)."""
    inner = block[2:-2]
    if '/*' in inner or '*/' in inner:
        raise SystemExit('nested comment marker in notice:\n' + block)


# --------------------------------------------------------------------------------------------

def main():
    rows, counts, manifest_rows = [], collections.Counter(), []
    all_unresolved = collections.Counter()
    for f in git_files():
        p = os.path.join(ROOT, f)
        with open(p, encoding='utf-8', newline='') as fh:
            raw = fh.read()
        crlf = raw.split('\n', 1)[0].endswith('\r')   # keep the file's own line endings
        text = raw
        old_blocks, rest = split_existing_header(text)
        origins, unresolved, jdk_api = classify(f, rest)
        all_unresolved.update({k: v for k, v in unresolved.items()})
        if f in ARGS.explain:
            print(f'== {f}', file=sys.stderr)
            for (pr, r), c in origins.items():
                print(f'   {c:4d} {pr}:{r} [{kind_of(leading_lic(pr, r)) if pr != "args4j" else "mit"}]'
                      f' {sorted(NAMES[(f, pr, r)])}', file=sys.stderr)
        header, kinds, notes = build_header(origins, f)
        expected = {norm(b) for (pr, r) in origins for _, b in origin_notices(pr, src_of(pr), r)[0]}
        expected |= {norm(OWN_HEADER)}
        for b in re.findall(r'/\*.*?\*/', header, re.S):
            check_rust_comment(b)
        new_blocks = {norm(b) for b in re.findall(r'(?s)/\*.*?\*/', header)}
        new_norm = norm(header)
        stale = [b for b in old_blocks if b.startswith('/*') and norm(b) not in new_blocks
                 and norm(b) not in new_norm and norm(b) not in expected]
        lic = sorted({LICENSE_NAME[k] for k in kinds if LICENSE_NAME.get(k)})
        for l in lic:
            counts[l] += 1
        status = 'new' if not old_blocks else ('replaced-ok' if not stale else 'replaced-DIFFERENT')
        projs = sorted({pr for pr, _ in origins}, key=PROJECT_ORDER.index) or ['none']
        manifest_rows.append('\t'.join((
            f, header_hash(header_prefix(header + '\n' + rest)[1]), spdx_and(lic),
            '; '.join(f'{pr}:{src_of(pr).display(r)}' for (pr, r) in origins) or '-')))
        rows.append((f, ' + '.join(lic), ','.join(projs), status,
                     '; '.join(f'{pr}:{src_of(pr).display(r)}'
                               + (' {' + ','.join(sorted(NAMES[(f, pr, r)])) + '}'
                                  if pr != 'closure' else '')
                               for (pr, r) in origins),
                     '; '.join(f'{k}x{v}' for k, v in unresolved.most_common()),
                     '; '.join(f'{k}x{v}' for k, v in jdk_api.most_common()),
                     '; '.join(notes + sorted('ambiguous: ' + a for a in AMBIGUOUS[f])), stale))
        stale_files = STALE_FILES
        sep_ = '\r\n' if crlf else '\n'
        if (header + '\n').replace('\n', sep_) + rest != raw:
            stale_files.append(f)
        if ARGS.apply:
            sep = '\r\n' if crlf else '\n'
            new = (header + '\n').replace('\n', sep) + rest
            if new != raw:
                with open(p, 'w', encoding='utf-8', newline='') as fh:
                    fh.write(new)
    print('files per license:', dict(counts.most_common()), file=sys.stderr)
    print('by status:', dict(collections.Counter(r[3] for r in rows)), file=sys.stderr)
    print('by origin set:', dict(collections.Counter(r[2] for r in rows).most_common()),
          file=sys.stderr)
    if all_unresolved:
        print('unresolved marker names:', dict(all_unresolved.most_common(60)), file=sys.stderr)
    generated = {MANIFEST: manifest_text(manifest_rows), FILES_MD: files_md_text(rows)}
    for out, text in generated.items():
        if ARGS.apply:
            with open(out, 'w', encoding='utf-8') as fh:
                fh.write(text)
        elif ARGS.check:
            try:
                current = open(out, encoding='utf-8').read()
            except OSError:
                current = None
            if current != text:
                STALE_FILES.append(os.path.relpath(out, ROOT))
    if ARGS.check and STALE_FILES:
        print(f'{len(STALE_FILES)} files need their license header (or the license manifests) '
              'updated (run with --apply):', *STALE_FILES[:20], sep='\n  ', file=sys.stderr)
    if ARGS.check:
        print(f'full check: {len(rows)} files re-classified against the upstream sources',
              file=sys.stderr)
    if ARGS.files_md:
        with open(ARGS.files_md, 'w', encoding='utf-8') as fh:
            fh.write(generated[FILES_MD])
    if ARGS.report:
        os.makedirs(os.path.dirname(os.path.abspath(ARGS.report)), exist_ok=True)
        with open(ARGS.report, 'w', encoding='utf-8') as fh:
            fh.write('file\tclassification\tupstream projects\theader status\torigin files\t'
                     'unresolved marker names\tJDK API-only markers (no notice)\tnotes\n')
            for r in rows:
                fh.write('\t'.join(r[:8]) + '\n')
            fh.write('\n# summary: files per license (a file with several origins counts for each)\n')
            for l, c in counts.most_common():
                fh.write(f'# {l}\t{c}\n')
            for s, c in collections.Counter(r[1] for r in rows).most_common():
                fh.write(f'# classification {s}\t{c}\n')
            for s, c in collections.Counter(r[3] for r in rows).most_common():
                fh.write(f'# header status {s}\t{c}\n')
            for s, c in collections.Counter(r[2] for r in rows).most_common():
                fh.write(f'# upstream projects {s}\t{c}\n')
        stale_path = os.path.splitext(ARGS.report)[0] + '.replaced-different.txt'
        with open(stale_path, 'w', encoding='utf-8') as fh:
            for r in rows:
                for b in r[8]:
                    fh.write(f'=== {r[0]}\n{b}\n')


def is_mpl(proj, rel):
    return proj == 'closure' and kind_of(leading_lic('closure', rel)) == 'mpl'


# (title, SPDX, file predicate on a report row, origin predicate (project, path) for the listing)
GROUPS = [
    ('Rhino-derived files of Closure Compiler', 'MPL-1.1 OR GPL-2.0-or-later',
     lambda r: 'MPL-1.1' in r[1], is_mpl),
    ('OpenJDK 21', 'GPL-2.0-only WITH Classpath-exception-2.0; the files ported from the Apache '
     'Xerces code in java.xml are Apache-2.0 with the Xerces NOTICE',
     lambda r: 'jdk' in r[2].split(','), lambda p, _: p == 'jdk'),
    ('Unicode Character Database data, via JDK 21', 'Unicode-DFS-2016',
     lambda r: 'Unicode' in r[1], lambda p, _: False),
    ("David M. Gay's dtoa notice, via Closure's DToA.java", 'dtoa',
     lambda r: 'dtoa' in r[1], lambda p, _: False),
    ('Protocol Buffers for Java', 'BSD-3-Clause',
     lambda r: 'protobuf' in r[2].split(','), lambda p, _: p == 'protobuf'),
    ('Guava', 'Apache-2.0', lambda r: 'guava' in r[2].split(','), lambda p, _: p == 'guava'),
    ('Gson', 'Apache-2.0', lambda r: 'gson' in r[2].split(','), lambda p, _: p == 'gson'),
    ('args4j', 'MIT', lambda r: 'args4j' in r[2].split(','), lambda p, _: p == 'args4j'),
]


def files_md_text(rows):
    lines = ['# Files by license and origin', '',
             'Generated by `python3 scripts/license_headers.py --files-md LICENSES/FILES.md`. The header '
             'of each file is authoritative; this list repeats it per upstream origin. A file that is '
             'not listed below is Apache-2.0 only: ported from Closure Compiler (Copyright The Closure '
             'Compiler Authors) or written for closure-rs (Copyright The closure-rs Authors).', '']
    for title, spdx, pred, opred in GROUPS:
        sel = [r for r in rows if pred(r)]
        n = f'{len(sel)} file' + ('' if len(sel) == 1 else 's')
        lines += [f'## {title}', '', f'License: {spdx}. {n}.', '']
        for r in sel:
            origins = []
            for o in r[4].split('; '):
                proj, rest = o.split(':', 1)
                rel = rest.split(' {')[0]
                if opred(proj, rel):
                    origins.append(rel)
            lines.append(f'- `{r[0]}`' + (': ' + ', '.join(origins) if origins else ''))
        lines.append('')
    return '\n'.join(lines)


# --------------------------------------------------------------------------------------------
# The license manifest (LICENSES/headers.tsv) and the check that needs only it

MANIFEST_HEAD = ('# Generated by `python3 scripts/license_headers.py --apply`; do not edit. One row per '
                 'tracked crates/**/*.rs file:\n# path, the first 16 hex digits of the SHA-256 of its '
                 'license header (the leading comment lines, LF line endings, up to the first line '
                 'of code), its licenses (SPDX) and its origin files (project:path). '
                 '`license_headers.py --check` compares the headers with it where the upstream '
                 'sources are not available.\n')


def header_prefix(text):
    """(license blocks, header text) of a Rust file: the leading comment blocks that
    split_existing_header removes, as they stand in the file (LF line endings)."""
    blocks, rest = split_existing_header(text)
    return blocks, text[:len(text) - len(rest)].replace('\r\n', '\n')


def header_hash(prefix):
    return hashlib.sha256(prefix.encode('utf-8')).hexdigest()[:16]


def spdx_and(licenses):
    return ' AND '.join(f'({l})' if ' OR ' in l else l for l in licenses) or '-'


def manifest_text(manifest_rows):
    return MANIFEST_HEAD + ''.join(r + '\n' for r in sorted(manifest_rows))


def read_manifest():
    out = {}
    with open(MANIFEST, encoding='utf-8') as fh:
        for line in fh:
            if line.startswith('#') or not line.strip():
                continue
            path, digest, licenses, _origins = line.rstrip('\n').split('\t')
            out[path] = (digest, licenses)
    return out


def manifest_check(missing):
    """--check without the upstream sources: the headers against LICENSES/headers.tsv."""
    print('upstream sources not available, so --check compares the headers with '
          'LICENSES/headers.tsv only (the full re-classification runs where they are present). '
          'Missing:', *missing, sep='\n  ', file=sys.stderr)
    try:
        manifest = read_manifest()
    except (OSError, ValueError) as e:
        print(f'cannot read {os.path.relpath(MANIFEST, ROOT)}: {e}', file=sys.stderr)
        return 1
    errors, unlisted = [], []
    files = git_files()
    for f in files:
        with open(os.path.join(ROOT, f), encoding='utf-8', newline='') as fh:
            text = fh.read()
        blocks, prefix = header_prefix(text)
        if not blocks or not HEADER_WORDS.search(blocks[0]):
            errors.append(f'{f}: no license header (the file must start with its license notice)')
            continue
        if f not in manifest:
            unlisted.append(f)
            continue
        digest, licenses = manifest[f]
        if header_hash(prefix) != digest:
            found = sorted({LICENSE_NAME.get(kind_of(b)) or kind_of(b) for b in blocks
                            if b.startswith('/*')})
            errors.append(f'{f}: header differs from the one LICENSES/headers.tsv records '
                          f'({licenses}); the header found has {", ".join(found) or "no notice"}')
    tracked = set(files)
    errors += [f'{f}: listed in LICENSES/headers.tsv but not tracked' for f in sorted(manifest)
               if f not in tracked]
    if unlisted:
        print(f'{len(unlisted)} files carry a license header but are not in LICENSES/headers.tsv '
              '(a full run classifies them: license_headers.py --apply):', *unlisted[:20],
              sep='\n  ', file=sys.stderr)
    if errors:
        print(f'{len(errors)} license header problems:', *errors[:40], sep='\n  ', file=sys.stderr)
        return 1
    print(f'manifest check: {len(files) - len(unlisted)} headers match LICENSES/headers.tsv',
          file=sys.stderr)
    return 0


MISSING = missing_inputs()
if ARGS.check and not ARGS.apply and MISSING:
    sys.exit(manifest_check(MISSING))
index_sources()
main()
if ARGS.check and STALE_FILES and not ARGS.apply:
    sys.exit(1)
