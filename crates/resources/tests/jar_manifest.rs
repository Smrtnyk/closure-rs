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

//! The embedded resources equal the reference jar's, entry by entry (name, order, size, bytes).

mod common;

use closure_resources::jar_contents::{
    ArchiveEntry, EXTERNS_ZIP_ENTRIES, EntryContent, JAR_ENTRIES,
};
use common::{fixture_rows, sha256_hex};

fn check_archive(archive: &str, entries: &[ArchiveEntry]) {
    let rows: Vec<Vec<String>> = fixture_rows("jar_manifest.tsv")
        .into_iter()
        .filter(|row| row[0] == archive)
        .collect();
    assert_eq!(rows.len(), entries.len(), "{archive}: entry count");
    for (index, (row, entry)) in rows.iter().zip(entries).enumerate() {
        assert_eq!(row[1], index.to_string(), "{archive}: index");
        assert_eq!(row[2], entry.name, "{archive}: name at {index}");
        let (size, sha) = match entry.content {
            EntryContent::Directory => {
                assert!(entry.name.ends_with('/'), "{}", entry.name);
                ("0".to_string(), sha256_hex(b""))
            }
            EntryContent::File(bytes) => {
                assert!(!entry.name.ends_with('/'), "{}", entry.name);
                (bytes.len().to_string(), sha256_hex(bytes))
            }
            EntryContent::Zip(_) => ("-".to_string(), "-".to_string()),
        };
        assert_eq!(row[3], size, "{archive}: size of {}", entry.name);
        assert_eq!(row[4], sha, "{archive}: sha256 of {}", entry.name);
    }
}

// port: closure-compiler.jar resources (unzip -l / unzip -p, fixture jar_manifest.tsv)
#[test]
fn jar_entries_match_reference_jar() {
    check_archive("jar", JAR_ENTRIES);
}

// port: closure-compiler.jar!/externs.zip (Bazel rule externs_zip, fixture jar_manifest.tsv)
#[test]
fn externs_zip_entries_match_reference_jar() {
    check_archive("externs.zip", EXTERNS_ZIP_ENTRIES);
}

// port: closure-compiler.jar resource counts (126 externs, 162 runtime libraries)
#[test]
fn resource_counts() {
    let count = |entries: &[ArchiveEntry], pred: &dyn Fn(&ArchiveEntry) -> bool| {
        entries.iter().filter(|e| pred(e)).count()
    };
    let is_file = |e: &ArchiveEntry| matches!(e.content, EntryContent::File(_));
    assert_eq!(EXTERNS_ZIP_ENTRIES.len(), 127);
    assert_eq!(count(EXTERNS_ZIP_ENTRIES, &is_file), 126);
    assert_eq!(
        count(EXTERNS_ZIP_ENTRIES, &|e| e.name.ends_with(".js")
            && is_file(e)),
        126
    );
    assert_eq!(
        count(JAR_ENTRIES, &|e| {
            e.name.starts_with("com/google/javascript/jscomp/js/")
                && e.name.ends_with(".js")
                && is_file(e)
        }),
        162
    );
    for name in [
        "com/google/javascript/jscomp/js/polyfills.txt",
        "com/google/javascript/jscomp/js/transpilation_libs.txt",
        "runtime_libs.typedast",
    ] {
        assert_eq!(
            count(JAR_ENTRIES, &|e| e.name == name && is_file(e)),
            1,
            "{name}"
        );
    }
    let zip = JAR_ENTRIES
        .iter()
        .find(|e| e.name == "externs.zip")
        .unwrap();
    assert!(matches!(zip.content, EntryContent::Zip(entries) if entries.len() == 127));
}

// port: InputStreamReader(UTF_8) over every text resource (no replacement characters needed)
#[test]
fn text_resources_are_valid_utf8() {
    for entry in JAR_ENTRIES.iter().chain(EXTERNS_ZIP_ENTRIES) {
        if let EntryContent::File(bytes) = entry.content {
            if entry.name == "runtime_libs.typedast" {
                continue;
            }
            assert!(std::str::from_utf8(bytes).is_ok(), "{}", entry.name);
        }
    }
}
