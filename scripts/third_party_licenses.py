#!/usr/bin/env python3
"""Collect the licenses of the Rust crates linked into the closure-rs binary.

Writes LICENSES/THIRD_PARTY_RUST.md: for every crates.io crate that the `closure-rs` binary
(package closure-cli) links, its name, version, the license its Cargo.toml declares, and the full
text of every license file the published crate ships, as found in the local Cargo registry
(~/.cargo/registry/src). Identical texts are printed once and referenced afterwards.

  python3 scripts/third_party_licenses.py [--out FILE] [--check]

The crate set is `cargo tree -e normal,no-proc-macro --target all -p closure-cli`: normal
dependencies only (no build or dev dependencies, no proc macros, which run in the compiler and are
not linked), for every target platform, so that one file covers every npm platform package. Package
metadata comes from `cargo metadata`. Both run with --locked --offline: the script reads Cargo.lock
and the registry and changes neither (fetch missing crates with `cargo fetch` first). The Rust
standard library is linked as well; its license is stated after the crates. Last comes the C runtime
that the Rust targets of the Linux binaries link statically (STATIC_RUNTIME below), with the
license texts kept verbatim in LICENSES/.

--check exits 1 when FILE differs from what the script would write.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACKAGE = 'closure-cli'
BINARY = 'closure-rs'

# License files at a crate's root.
LICENSE_FILE = re.compile(r'^(licen[cs]e|copying|unlicense|notice|copyright)([-._].*)?$', re.I)
# License files of code vendored inside a crate and compiled into the binary.
VENDORED = {
    # zlib's C sources, compiled in when libz-sys builds zlib itself instead of linking the
    # system's libz: always in the release binaries, which are built with LIBZ_SYS_STATIC=1.
    'libz-sys': [('src/zlib/LICENSE', 'zlib (bundled C sources, compiled into the released binaries)')],
    # Microsoft's mimalloc C sources, always compiled in (the v2 sources; v3 only with the `v3`
    # feature, which closure-rs does not enable).
    'libmimalloc-sys': [('c_src/mimalloc/v2/LICENSE', 'mimalloc (bundled C sources, Microsoft Corporation)')],
}


# The linux-x64 and linux-arm64 binaries are built for the Rust targets x86_64-unknown-linux-musl
# and aarch64-unknown-linux-musl, which link them statically and self-contained: rustc links the
# objects of the target's rust-std component (lib/rustlib/<target>/lib/self-contained), not the
# system's. For a static-pie executable these are rcrt1.o, crti.o, crtbeginS.o, crtendS.o and
# crtn.o, libc.a (`-lc`, from the libc crate) and libunwind.a (`-lunwind`, from std's unwind
# crate). Rust's CI builds them in src/ci/docker/host-x86_64/dist-x86_64-musl and
# dist-arm-linux-musl, both with the same scripts/musl-toolchain.sh and musl patches, and
# crtbegin/crtend and libunwind from the src/llvm-project submodule (bootstrap's llvm::CrtBeginEnd
# and llvm::Libunwind); so both targets carry the same components and versions. The versions below
# are those of Rust STATIC_RUNTIME_TOOLCHAIN; --check fails when rust-toolchain.toml names another
# toolchain, so that they are confirmed again on an update. The macOS and Windows binaries link the
# operating system's C libraries dynamically and contain none of these.
STATIC_RUNTIME_TOOLCHAIN = '1.95.0'
STATIC_RUNTIME_TARGETS = ['x86_64-unknown-linux-musl', 'aarch64-unknown-linux-musl']
LLVM_VERSION = ('LLVM 22.1.2 (rust-lang/llvm-project commit 1cb4e3833c1919c2e6fb579a23ac0e2b22587b7e, '
                'the src/llvm-project submodule of Rust 1.95.0)')
STATIC_RUNTIME = [
    # (component, version, SPDX, what the binary contains, license file in LICENSES/, its source)
    ('musl', 'musl 1.2.5, with the patches for CVE-2025-26519, CVE-2026-6042 and CVE-2026-40200',
     'MIT', 'the C library (`libc.a`) and the startup objects `rcrt1.o`, `crti.o`, `crtn.o`',
     'MIT-musl.txt',
     '`COPYRIGHT` of musl v1.2.5, <https://git.musl-libc.org/cgit/musl/tree/COPYRIGHT?h=v1.2.5>'),
    ('LLVM libunwind', LLVM_VERSION, 'Apache-2.0 WITH LLVM-exception',
     'the stack unwinder (`libunwind.a`) that Rust panics use',
     'Apache-2.0-WITH-LLVM-exception-libunwind.txt',
     '`libunwind/LICENSE.TXT` of that commit'),
    ('LLVM compiler-rt', LLVM_VERSION, 'Apache-2.0 WITH LLVM-exception',
     '`crtbeginS.o` and `crtendS.o` (`compiler-rt/lib/builtins/crtbegin.c`, `crtend.c`); also the '
     'compiler-rt builtins in the standard library\'s `compiler_builtins` crate',
     'Apache-2.0-WITH-LLVM-exception-compiler-rt.txt',
     '`compiler-rt/LICENSE.TXT` of that commit'),
]


def run(args):
    return subprocess.check_output(args, cwd=ROOT, text=True)


def linked_packages():
    out = run(['cargo', 'tree', '--locked', '--offline', '-e', 'normal,no-proc-macro',
               '--target', 'all', '-p', PACKAGE, '--prefix', 'none', '--format', '{p}'])
    found = set()
    for line in out.splitlines():
        line = line.strip().removesuffix('(*)').strip()
        if not line or ' (' in line:  # workspace members print their path: "name v0.1.0 (/...)"
            continue
        name, version = line.split(' ')
        found.add((name, version.removeprefix('v')))
    return found


def metadata():
    meta = json.loads(run(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1']))
    return {(p['name'], p['version']): p for p in meta['packages']}


def read(path):
    with open(path, encoding='utf-8', errors='replace') as fh:
        return fh.read().replace('\r\n', '\n').rstrip('\n') + '\n'


def license_files(pkg):
    crate_dir = os.path.dirname(pkg['manifest_path'])
    files = []
    for name in sorted(os.listdir(crate_dir), key=str.lower):
        if LICENSE_FILE.match(name) and os.path.isfile(os.path.join(crate_dir, name)):
            files.append((name, None))
    if pkg.get('license_file') and pkg['license_file'] not in [f for f, _ in files]:
        files.append((pkg['license_file'], None))
    files += VENDORED.get(pkg['name'], [])
    return crate_dir, files


def fence(text):
    longest = max((len(m) for m in re.findall(r'`+', text)), default=0)
    return '`' * max(3, longest + 1)


def toolchain_channel():
    with open(os.path.join(ROOT, 'rust-toolchain.toml'), encoding='utf-8') as fh:
        m = re.search(r'^channel\s*=\s*"([^"]+)"', fh.read(), re.M)
    return m.group(1) if m else 'see rust-toolchain.toml'


# The MIT terms of the Rust project (rust-lang/rust LICENSE-MIT). Its Apache-2.0 text is the one
# printed above for the crates and in ../LICENSE.
RUST_MIT = """Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
"""


def render():
    pkgs = metadata()
    linked = sorted(linked_packages(), key=lambda nv: (nv[0].lower(), nv[1]))
    missing = [nv for nv in linked if nv not in pkgs]
    if missing:
        sys.exit(f'not in cargo metadata: {missing}')
    lines = [
        '# Third-party Rust crates in the closure-rs binary',
        '',
        'Generated by `python3 scripts/third_party_licenses.py` from `Cargo.lock`; do not edit. '
        f'The `{BINARY}` executable links these crates from crates.io (the normal, non-proc-macro '
        f'dependencies of `{PACKAGE}` on any target platform), each under its own license. Below '
        'are the license each crate declares and the full text of every license file it ships. '
        'Where a crate offers a choice of licenses (`OR`), closure-rs uses it under any one of them; '
        'all texts are reproduced.',
        '',
        '| Crate | Version | License | License files |',
        '| --- | --- | --- | --- |',
    ]
    sections = []
    seen = {}  # sha256 of a text -> (crate, file) where it is printed
    for name, version in linked:
        pkg = pkgs[(name, version)]
        crate_dir, files = license_files(pkg)
        if not files:
            sys.exit(f'{name} {version}: no license file in {crate_dir}')
        lines.append(f'| {name} | {version} | {pkg.get("license") or "(see files)"} | '
                     + ', '.join(f'`{f}`' for f, _ in files) + ' |')
        sections += [f'## {name} {version}', '', f'License: {pkg.get("license") or "(see files)"}.',
                     f'Source: <https://crates.io/crates/{name}/{version}>.', '']
        for fname, note in files:
            text = read(os.path.join(crate_dir, fname))
            digest = hashlib.sha256(text.encode()).hexdigest()
            title = f'### `{fname}`' + (f': {note}' if note else '')
            if digest in seen:
                other, other_file = seen[digest]
                sections += [title, '', f'Identical to `{other_file}` of {other} above.', '']
                continue
            seen[digest] = (f'{name} {version}', fname)
            f = fence(text)
            sections += [title, '', f + 'text', text.rstrip('\n'), f, '']
    lines += ['', 'The binary also links the Rust standard library (`std`, `core`, `alloc` and their '
              f'dependencies) of the pinned toolchain (`rust-toolchain.toml`), which is licensed MIT '
              'OR Apache-2.0 (<https://www.rust-lang.org/policies/licenses>), and the Linux '
              'binaries, which are statically linked, also contain the C library musl (MIT) and '
              'runtime code of LLVM (Apache-2.0 WITH LLVM-exception); see the last two sections of '
              'this file.', '']
    lines += sections
    apache = next((f'`{f}` of {c}' for c, f in seen.values() if f == 'LICENSE-APACHE'),
                  '`../LICENSE`')
    lines += ['## Rust standard library', '',
              f'Toolchain: Rust {toolchain_channel()} (`rust-toolchain.toml`).',
              'License: MIT OR Apache-2.0. Copyrights in the Rust project are retained by their '
              'contributors (the toolchain\'s share/doc/rust/COPYRIGHT-library.html).',
              '', f'The Apache License 2.0 text is the one printed above ({apache}) and in '
              'closure-rs\' `LICENSE`. The MIT terms (rust-lang/rust `LICENSE-MIT`):', '']
    f = fence(RUST_MIT)
    lines += [f + 'text', RUST_MIT.rstrip('\n'), f, '']
    targets = ' and '.join(f'`{t}`' for t in STATIC_RUNTIME_TARGETS)
    lines += ['## C runtime statically linked into the Linux binaries', '',
              f'The linux-x64 and linux-arm64 binaries are built for the Rust targets {targets} and '
              'are statically linked: besides the crates above each contains the C library and '
              'runtime objects that its target\'s `rust-std` component of Rust '
              f'{STATIC_RUNTIME_TOOLCHAIN} ships (self-contained linking), each under its own '
              'license; both targets ship the same components and versions. The macOS and Windows '
              'binaries contain none of them. The license texts below are verbatim copies of the '
              'files named, kept in `LICENSES/`.', '',
              '| Component | License | In the binary | License file |', '| --- | --- | --- | --- |']
    for name, _, spdx, contents, fname, _ in STATIC_RUNTIME:
        lines.append(f'| {name} | {spdx} | {contents} | `LICENSES/{fname}` |')
    lines.append('')
    for name, version, spdx, _, fname, origin in STATIC_RUNTIME:
        path = os.path.join(ROOT, 'LICENSES', fname)
        if not os.path.isfile(path):
            sys.exit(f'{name}: license file {path} is missing')
        text = read(path)
        f = fence(text)
        lines += [f'### {name}', '', f'Version: {version}.', f'License: {spdx}.',
                  f'License text (`LICENSES/{fname}`): {origin}.', '',
                  f + 'text', text.rstrip('\n'), f, '']
    return '\n'.join(lines).rstrip('\n') + '\n'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', default=os.path.join(ROOT, 'LICENSES', 'THIRD_PARTY_RUST.md'))
    ap.add_argument('--check', action='store_true')
    args = ap.parse_args()
    channel = toolchain_channel()
    if channel != STATIC_RUNTIME_TOOLCHAIN:
        sys.exit(f'rust-toolchain.toml names Rust {channel}, STATIC_RUNTIME was confirmed for '
                 f'{STATIC_RUNTIME_TOOLCHAIN}: check the musl and LLVM versions (and license texts) '
                 f'that the new toolchain\'s {" and ".join(STATIC_RUNTIME_TARGETS)} targets link, '
                 'then update STATIC_RUNTIME and STATIC_RUNTIME_TOOLCHAIN')
    text = render()
    if args.check:
        try:
            current = read(args.out)
        except OSError:
            current = None
        if current != text:
            print(f'{args.out} is stale: run python3 scripts/third_party_licenses.py')
            sys.exit(1)
        return
    with open(args.out, 'w', encoding='utf-8') as fh:
        fh.write(text)
    print(f'wrote {args.out}')


main()
