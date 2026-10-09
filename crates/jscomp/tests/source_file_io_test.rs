/*
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/JSCompZipFileCache.java,
//   src/com/google/javascript/jscomp/SourceFile.java.

use closure_jscomp::source_file::SourceFile;
use closure_rhino::java_lang::charset::Charset;
use serde_json::Value;
use std::{fs, io::Write};
// port: CharsetDecoder#decode (JVM differential table)
#[test]
fn charset_parity() {
    let table: Value =
        serde_json::from_str(include_str!("data/source_file_io_cases.json")).unwrap();
    let cases = table["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 5688);
    for row in cases {
        let bytes: Vec<u8> = row["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        let actual = Charset::for_name(row["charset"].as_str().unwrap())
            .decode(&bytes, row["replace"].as_bool().unwrap());
        if let Some(units) = row["units"].as_array() {
            let expected: Vec<u16> = units.iter().map(|v| v.as_u64().unwrap() as u16).collect();
            assert_eq!(actual.unwrap().as_units(), expected, "{row}");
        } else {
            assert_eq!(
                actual.unwrap_err().get_message(),
                row["exception"].as_str().unwrap(),
                "{row}"
            );
        }
    }
}
// port: SourceFile.CodeLoader.OnDisk#loadUncachedCode
// port: JSCompZipFileCache.CachedZipFile#getEntryStream (JVM exception texts)
#[test]
fn source_file_io_messages() {
    let table: Value =
        serde_json::from_str(include_str!("data/source_file_io_cases.json")).unwrap();
    let directory =
        std::env::temp_dir().join(format!("closure-rs-diagnostics-io-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    // The directory the Java recording ran in (SourceFileIoCases.java, repo-relative); the
    // messages name it where this run names its own directory.
    let reference = "build/codex/diagnostics/java/io";
    let missing = directory.join("missing.js");
    assert_eq!(
        SourceFile::from_file(missing.to_str().unwrap())
            .get_code()
            .unwrap_err()
            .get_message(),
        table["messages"]["missing"]
            .as_str()
            .unwrap()
            .replace(reference, directory.to_str().unwrap())
    );
    let invalid = directory.join("invalid.js");
    fs::write(&invalid, [0xff]).unwrap();
    assert_eq!(
        SourceFile::from_file(invalid.to_str().unwrap())
            .get_code()
            .unwrap_err()
            .get_message(),
        table["messages"]["malformed"]
            .as_str()
            .unwrap()
            .replace(reference, directory.to_str().unwrap())
    );
    let path = directory.join("entry.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    zip.start_file(
        "yes.js",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(b"A").unwrap();
    zip.finish().unwrap();
    let error = std::panic::catch_unwind(|| {
        SourceFile::builder()
            .with_zip_entry_path(path.to_str().unwrap(), "missing.js")
            .build()
            .get_code()
    })
    .unwrap_err();
    let message = error.downcast_ref::<String>().unwrap();
    assert_eq!(
        message,
        &table["messages"]["zip_missing"]
            .as_str()
            .unwrap()
            .replace(reference, directory.to_str().unwrap())
    );
    fs::remove_dir_all(directory).unwrap();
}
// port: SourceFile#fromZipInput (JVM stream and malformed-header cases)
#[test]
fn zip_input_stream_parity() {
    let cases: Value = serde_json::from_str(include_str!("data/zip_input_cases.json")).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 19);
    for row in cases.as_array().unwrap() {
        let bytes: Vec<u8> = row[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        let actual = SourceFile::from_zip_input(
            "virtual.zip",
            std::io::Cursor::new(bytes),
            Charset::for_name(row[1].as_str().unwrap()),
        );
        if let Some(names) = row[2]["names"].as_array() {
            use closure_rhino::static_source_file::StaticSourceFile;
            let expected: Vec<&str> = names.iter().map(|v| v.as_str().unwrap()).collect();
            let files = actual.unwrap();
            assert_eq!(
                files.iter().map(|f| f.get_name()).collect::<Vec<_>>(),
                expected,
                "{row}"
            );
        } else {
            assert_eq!(
                actual.unwrap_err().get_message(),
                row[2]["exception"].as_str().unwrap_or("null"),
                "{row}"
            );
        }
    }
}
