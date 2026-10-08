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
standard library is linked as well; its license is stated at the end.

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
    # zlib's C sources, compiled in when libz-sys builds zlib itself (no system zlib found, or a
    # static build) instead of linking the system's libz.
    'libz-sys': [('src/zlib/LICENSE', 'zlib (bundled C sources, built in when no system zlib is used)')],
}


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
              'OR Apache-2.0 (<https://www.rust-lang.org/policies/licenses>); see the end of this '
              'file.', '']
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
    return '\n'.join(lines).rstrip('\n') + '\n'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', default=os.path.join(ROOT, 'LICENSES', 'THIRD_PARTY_RUST.md'))
    ap.add_argument('--check', action='store_true')
    args = ap.parse_args()
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
