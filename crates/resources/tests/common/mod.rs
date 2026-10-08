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

//! Fixture helpers shared by the integration tests. The fixtures under tests/fixtures/ were
//! generated from the reference closure-compiler.jar (tools/GenResourceFixtures.java,
//! tools/GenFieldsTableError.java, tools/sync_from_jar.py).
#![allow(dead_code)]

use sha2::{Digest, Sha256};

/// The data lines of a fixture split at tabs. The header lines at the top of the file start
/// with '#'; data lines may start with '#' too (escaped inputs), so only the header is skipped.
pub fn fixture_rows(name: &str) -> Vec<Vec<String>> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.split('\n')
        .skip_while(|line| line.starts_with('#'))
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(str::to_string).collect())
        .collect()
}

/// Undoes the fixture escaping of GenResourceFixtures#esc: `\\`, `\n`, `\r`, `\t`.
pub fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                other => panic!("bad escape \\{other:?} in {s:?}"),
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The panic message of `f`, which must panic.
pub fn panic_message<R>(f: impl FnOnce() -> R + std::panic::UnwindSafe) -> String {
    let payload = match std::panic::catch_unwind(f) {
        Ok(_) => panic!("expected a panic"),
        Err(payload) => payload,
    };
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else {
        panic!("non-string panic payload")
    }
}
