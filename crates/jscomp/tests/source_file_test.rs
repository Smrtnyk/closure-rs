/*
 * Copyright 2007 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/SourceFileTest.java.

use closure_jscomp::{
    region::Region,
    serialization::source_file_proto::{FileOnDisk, SourceFileProto, SourceKind as ProtoKind},
    source_file::SourceFile,
};
use closure_rhino::{
    java_lang::charset::Charset,
    static_source_file::{SourceKind, StaticSourceFile},
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};
struct TempDir(PathBuf);
impl TempDir {
    // port: TemporaryFolder#newFolder
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "closure-rs-diagnostics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    // port: TemporaryFolder#newFile
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for TempDir {
    // port: TemporaryFolder#delete
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
// port: SourceFileTest#createZipWithContent
fn create_zip_with_content(path: &Path, content: &str) {
    let stamp = fs::metadata(path).map_or(SystemTime::now(), |m| {
        m.modified().unwrap() + Duration::from_secs(1)
    });
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    zip.start_file(
        "foo.js",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    zip.write_all(content.as_bytes()).unwrap();
    zip.finish()
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(stamp))
        .unwrap();
}
// port: SourceFileTest#testSourceKind
#[test]
fn test_source_kind() {
    let sf1 = SourceFile::from_code("test1.js", "1");
    assert!(sf1.is_strong());
    assert!(!sf1.is_weak());
    assert!(!sf1.is_extern());
    sf1.set_kind(SourceKind::WEAK);
    assert!(!sf1.is_strong());
    assert!(sf1.is_weak());
    assert!(!sf1.is_extern());
    let sf2 = SourceFile::from_code_with_kind("test2.js", "2", SourceKind::WEAK);
    assert!(!sf2.is_strong());
    assert!(sf2.is_weak());
    assert!(!sf2.is_extern());
    sf2.set_kind(SourceKind::EXTERN);
    assert!(!sf2.is_strong());
    assert!(!sf2.is_weak());
    assert!(sf2.is_extern());
}
// port: SourceFileTest#testLineOffset
#[test]
fn test_line_offset() {
    test_line_offset_helper(|code| SourceFile::from_code("test.js", code));
}
// port: SourceFileTest#testLineOffsetHelper
fn test_line_offset_helper(factory: impl Fn(&str) -> SourceFile) {
    let f0 = factory("");
    assert_eq!(f0.get_line_of_offset(0), 1);
    assert_eq!(f0.get_column_of_offset(0), 0);
    assert_eq!(f0.get_line_of_offset(10), 1);
    assert_eq!(f0.get_column_of_offset(10), 10);
    assert_eq!(f0.get_num_bytes(), 0);
    assert_eq!(f0.get_num_lines(), 1);
    let f1 = factory("'1';\n'2';\n'3'\n");
    for (line, offset) in [(1, 0), (2, 5), (3, 10)] {
        assert_eq!(f1.get_line_offset(line), offset);
    }
    assert_eq!(f1.get_num_bytes(), 14);
    assert_eq!(f1.get_num_lines(), 4);
    let f2 = factory("'100';\n'200;'\n'300'\n");
    for (line, offset) in [(1, 0), (2, 7), (3, 14)] {
        assert_eq!(f2.get_line_offset(line), offset);
    }
    assert_eq!(f2.get_num_bytes(), 20);
    assert_eq!(f2.get_num_lines(), 4);
    let long_line = string_of_length(300);
    let f3 = factory(&format!("{long_line}\n{long_line}\n{long_line}\n"));
    for (line, offset) in [(1, 0), (2, 301), (3, 602)] {
        assert_eq!(f3.get_line_offset(line), offset);
    }
    for (offset, line) in [
        (0, 1),
        (300, 1),
        (301, 2),
        (601, 2),
        (602, 3),
        (902, 3),
        (903, 4),
    ] {
        assert_eq!(f3.get_line_of_offset(offset), line);
    }
    assert_eq!(f3.get_num_bytes(), 903);
    assert_eq!(f3.get_num_lines(), 4);
    assert_eq!(f3.get_line_of_offset(-1), 0);
    assert!(std::panic::catch_unwind(|| f3.get_column_of_offset(-1)).is_err());
    let starts_with_newline = factory("\n'a'\n'b'");
    for (line, offset) in [(1, 0), (2, 1), (3, 5)] {
        assert_eq!(starts_with_newline.get_line_offset(line), offset);
    }
    assert_eq!(starts_with_newline.get_num_bytes(), 8);
    assert_eq!(starts_with_newline.get_num_lines(), 3);
}
// port: SourceFileTest#stringOfLength
fn string_of_length(length: usize) -> String {
    "a".repeat(length)
}
// port: SourceFileTest#testCachingFile
#[test]
fn test_caching_file() {
    let tmp = TempDir::new();
    let p = tmp.file("test.js");
    fs::write(&p, "// content content content").unwrap();
    let source = SourceFile::from_path(&p, Charset::UTF_8);
    assert_eq!(source.get_code().unwrap(), "// content content content");
    fs::write(&p, "// new content new content new content").unwrap();
    source.clear_cached_source();
    assert_eq!(
        source.get_code().unwrap(),
        "// new content new content new content"
    );
}
// port: SourceFileTest#testCachingZipFile
#[test]
fn test_caching_zip_file() {
    let tmp = TempDir::new();
    let p = tmp.file("test.js.zip");
    create_zip_with_content(&p, "// content content content");
    let source = SourceFile::builder()
        .with_zip_entry_path(p.to_str().unwrap(), "foo.js")
        .build();
    assert_eq!(source.get_code().unwrap(), "// content content content");
    create_zip_with_content(&p, "// new content new content new content");
    assert_eq!(source.get_code().unwrap(), "// content content content");
    source.clear_cached_source();
    assert_eq!(
        source.get_code().unwrap(),
        "// new content new content new content"
    );
}
// port: SourceFileTest#testSourceFileResolvesZipEntries
#[test]
fn test_source_file_resolves_zip_entries() {
    let tmp = TempDir::new();
    let p = tmp.file("test.js.zip");
    create_zip_with_content(&p, "// <program goes here>");
    let weak = SourceFile::builder()
        .with_kind(SourceKind::WEAK)
        .with_zip_entry_path(p.to_str().unwrap(), "foo.js")
        .build();
    assert_eq!(weak.get_code().unwrap(), "// <program goes here>");
    assert_eq!(weak.get_kind(), SourceKind::WEAK);
    let strong = SourceFile::builder()
        .with_zip_entry_path(p.to_str().unwrap(), "foo.js")
        .build();
    assert_eq!(strong.get_code().unwrap(), "// <program goes here>");
    assert_eq!(strong.get_kind(), SourceKind::STRONG);
    let name = format!("{}!/foo.js", p.display());
    for source in [
        SourceFile::from_file_with_charset(&name, Charset::UTF_8),
        SourceFile::from_file_with_charset(&name, Charset::UTF_8),
        SourceFile::from_path(&name, Charset::UTF_8),
    ] {
        assert_eq!(source.get_code().unwrap(), "// <program goes here>");
    }
}
// port: SourceFileTest#testSourceFileFromZipFile
#[test]
fn test_source_file_from_zip_file() {
    let tmp = TempDir::new();
    let p = tmp.file("test.js.zip");
    create_zip_with_content(&p, "// <program goes here>");
    let sources = SourceFile::from_zip_file(p.to_str().unwrap(), Charset::UTF_8).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].get_name(), format!("{}!/foo.js", p.display()));
    assert_eq!(sources[0].get_code().unwrap(), "// <program goes here>");
}
// port: SourceFileTest#testSourceFileFromZipInput
#[test]
fn test_source_file_from_zip_input() {
    let tmp = TempDir::new();
    let p = tmp.file("test.js.zip");
    create_zip_with_content(&p, "// <program goes here>");
    let sources = SourceFile::from_zip_input(
        p.to_str().unwrap(),
        fs::File::open(&p).unwrap(),
        Charset::UTF_8,
    )
    .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].get_name(), format!("{}!/foo.js", p.display()));
    assert_eq!(sources[0].get_code().unwrap(), "// <program goes here>");
}
// port: SourceFileTest#testStubSourceFile_created_crashesOnGetCode
#[test]
fn test_stub_source_file_created_crashes_on_get_code() {
    let source = SourceFile::stub_source_file("file.js", SourceKind::WEAK);
    assert_eq!(source.get_name(), "file.js");
    assert_eq!(source.get_kind(), SourceKind::WEAK);
    assert!(std::panic::catch_unwind(|| source.get_code()).is_err());
}
// port: SourceFileTest#testDiskFile
#[test]
fn test_disk_file() {
    let tmp = TempDir::new();
    let p = tmp.file("test.js");
    fs::write(&p, "var c;").unwrap();
    let source = SourceFile::from_file(p.to_str().unwrap());
    assert_eq!(source.get_line(1).as_deref(), Some("var c;"));
    source.clear_cached_source();
    assert!(!source.has_source_in_memory());
    assert_eq!(source.get_line(1).as_deref(), Some("var c;"));
}
// port: SourceFileTest#testDiskFileWithOriginalPath
#[test]
fn test_disk_file_with_original_path() {
    let tmp = TempDir::new();
    let p = tmp.file("test.js");
    fs::write(&p, "var c;").unwrap();
    let source = SourceFile::builder()
        .with_original_path("original_test.js")
        .with_path(p.to_str().unwrap())
        .build();
    assert_eq!(source.get_line(1).as_deref(), Some("var c;"));
    source.clear_cached_source();
    assert!(!source.has_source_in_memory());
    assert_eq!(source.get_line(1).as_deref(), Some("var c;"));
    assert_eq!(source.get_name(), "original_test.js");
}
// port: SourceFileTest#testGetLines
#[test]
fn test_get_lines() {
    let s = SourceFile::from_code("file.js", "const a = 0;\nconst b = 1;\nconst c = 2;");
    for (line, len, expected) in [
        (1, 1, "const a = 0;"),
        (2, 1, "const b = 1;"),
        (3, 1, "const c = 2;"),
        (1, "const a = 0;\n".len(), "const a = 0;"),
        (
            1,
            "const a = 0;\nconst b".len(),
            "const a = 0;\nconst b = 1;",
        ),
        (
            2,
            "const b = 1;\nconst c".len(),
            "const b = 1;\nconst c = 2;",
        ),
        (3, "const c = 2;".len(), "const c = 2;"),
    ] {
        assert_eq!(
            s.get_lines(line, len as i32).unwrap().get_source_excerpt(),
            expected
        );
    }
}
// port: SourceFileTest#testGetLines_invalidLengths
#[test]
fn test_get_lines_invalid_lengths() {
    let s = SourceFile::from_code("file.js", "const a = 0;\nconst b = 1;\nconst c = 2;");
    for (line, len, expected) in [
        (0, -3, "const a = 0;"),
        (0, 0, "const a = 0;"),
        (1, 100000, "const a = 0;\nconst b = 1;\nconst c = 2;"),
        (2, 10000, "const b = 1;\nconst c = 2;"),
        (3, 10000, "const c = 2;"),
    ] {
        assert_eq!(
            s.get_lines(line, len).unwrap().get_source_excerpt(),
            expected
        );
    }
}
// port: SourceFileTest#testGetLines_whenFileEndsWithNewline
#[test]
fn test_get_lines_when_file_ends_with_newline() {
    let s = SourceFile::from_code("file.js", "const a = 0;\nconst b = 1;\nconst c = 2;\n");
    for (line, len, expected) in [
        (1, 100000, "const a = 0;\nconst b = 1;\nconst c = 2;"),
        (2, 10000, "const b = 1;\nconst c = 2;"),
        (3, 1, "const c = 2;"),
        (4, 1, ""),
    ] {
        assert_eq!(
            s.get_lines(line, len).unwrap().get_source_excerpt(),
            expected
        );
    }
}
// port: SourceFileTest#testGetLines_invalidLineNumbers
#[test]
fn test_get_lines_invalid_line_numbers() {
    let s = SourceFile::from_code("file.js", "const a = 0;\nconst b = 1;\nconst c = 2;");
    for line in [-20, 0] {
        assert_eq!(
            s.get_lines(line, 1).unwrap().get_source_excerpt(),
            "const a = 0;"
        );
    }
    assert!(s.get_lines(4, 1).is_none());
}
// port: SourceFileTest#testFromProtoPreloadedContents
#[test]
fn test_from_proto_preloaded_contents() {
    let s = SourceFile::from_proto(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_source_kind(ProtoKind::CODE)
            .set_preloaded_contents("42")
            .build(),
    );
    assert_eq!(s.get_name(), "file.js");
    assert_eq!(s.get_code().unwrap(), "42");
    assert_eq!(s.get_num_lines(), 1);
    assert_eq!(s.get_num_bytes(), 2);
}
// port: SourceFileTest#testFromProto_getNumLinesAvoidsFileRead
#[test]
fn test_from_proto_get_num_lines_avoids_file_read() {
    let s = SourceFile::from_proto(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_source_kind(ProtoKind::CODE)
            .set_file_on_disk(FileOnDisk::get_default_instance())
            .set_num_lines_plus_one(2)
            .set_num_bytes_plus_one(3)
            .build(),
    );
    assert_eq!(s.get_num_lines(), 1);
    assert_eq!(s.get_num_bytes(), 2);
    assert!(s.get_code().is_err());
}
// port: SourceFileTest#testFromProtoPreloadedContents_withNumLinesAndBytes
#[test]
fn test_from_proto_preloaded_contents_with_num_lines_and_bytes() {
    let s = SourceFile::from_proto(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_source_kind(ProtoKind::CODE)
            .set_preloaded_contents("42")
            .set_num_lines_plus_one(2)
            .set_num_bytes_plus_one(3)
            .build(),
    );
    assert_eq!(s.get_num_lines(), 1);
    assert_eq!(s.get_num_bytes(), 2);
}
// port: SourceFileTest#testGetProto_withoutGetCodeCalls
#[test]
fn test_get_proto_without_get_code_calls() {
    let s = SourceFile::from_code("file.js", "42");
    assert_eq!(
        s.get_proto(),
        SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_preloaded_contents("42")
            .set_source_kind(ProtoKind::CODE)
            .build()
    );
}
// port: SourceFileTest#testGetProto_withGetCodeCalls
#[test]
fn test_get_proto_with_get_code_calls() {
    let s = SourceFile::from_code("file.js", "42");
    s.get_code().unwrap();
    assert_eq!(
        s.get_proto(),
        SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_preloaded_contents("42")
            .set_num_lines_plus_one(2)
            .set_num_bytes_plus_one(3)
            .set_source_kind(ProtoKind::CODE)
            .build()
    );
}
// port: SourceFileTest#testGetProto_recordsClosureUnawareCodeMark
#[test]
fn test_get_proto_records_closure_unaware_code_mark() {
    let s = SourceFile::from_code("file.js", "42");
    s.mark_as_closure_unaware_code();
    assert_eq!(
        s.get_proto(),
        SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_preloaded_contents("42")
            .set_is_closure_unaware_code(true)
            .set_source_kind(ProtoKind::CODE)
            .build()
    );
}
// port: SourceFileTest#testGetProto_withGetCodeCalls_andClearCachedSource
#[test]
fn test_get_proto_with_get_code_calls_and_clear_cached_source() {
    let s = SourceFile::from_code("file.js", "42");
    s.get_code().unwrap();
    s.clear_cached_source();
    assert_eq!(
        s.get_proto(),
        SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_preloaded_contents("42")
            .set_num_lines_plus_one(2)
            .set_num_bytes_plus_one(3)
            .set_source_kind(ProtoKind::CODE)
            .build()
    );
}
// port: SourceFileTest#testRestoreCachedState_correctNumLines
#[test]
fn test_restore_cached_state_correct_num_lines() {
    let s = SourceFile::from_code("file.js", "42");
    s.restore_cached_state_from(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_num_lines_plus_one(2)
            .set_num_bytes_plus_one(3)
            .build(),
    );
    assert_eq!(s.get_num_lines(), 1);
    assert_eq!(s.get_num_bytes(), 2);
}
// port: SourceFileTest#testRestoreCachedState_setsClosureUnawareCodeTrue
#[test]
fn test_restore_cached_state_sets_closure_unaware_code_true() {
    let s = SourceFile::from_code("file.js", "42");
    assert!(!s.is_closure_unaware_code());
    s.restore_cached_state_from(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_is_closure_unaware_code(true)
            .build(),
    );
    assert!(s.is_closure_unaware_code());
}
// port: SourceFileTest#testRestoreCachedState_setsClosureUnawareCodeFalse_protoIsSourceOfTruth
#[test]
fn test_restore_cached_state_sets_closure_unaware_code_false_proto_is_source_of_truth() {
    let s = SourceFile::from_code("file.js", "42");
    s.mark_as_closure_unaware_code();
    assert!(s.is_closure_unaware_code());
    s.restore_cached_state_from(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_is_closure_unaware_code(false)
            .build(),
    );
    assert!(!s.is_closure_unaware_code());
}
// port: SourceFileTest#testRestoreCachedState_forcesRecalculationIfValuesUnsetInProto
#[test]
fn test_restore_cached_state_forces_recalculation_if_values_unset_in_proto() {
    let s = SourceFile::from_code("file.js", "42");
    s.restore_cached_state_from(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .build(),
    );
    assert_eq!(s.get_num_lines(), 1);
    assert_eq!(s.get_num_bytes(), 2);
}
// port: SourceFileTest#testRestoreCachedState_treatsProtoNumLinesAsCanonical
#[test]
fn test_restore_cached_state_treats_proto_num_lines_as_canonical() {
    let s = SourceFile::from_code("file.js", "42");
    assert_eq!(s.get_num_lines(), 1);
    s.restore_cached_state_from(
        &SourceFileProto::new_builder()
            .set_filename("file.js")
            .set_num_lines_plus_one(1000)
            .set_num_bytes_plus_one(-1)
            .build(),
    );
    assert_eq!(s.get_num_lines(), 999);
}
// port: SourceFileTest#testStripConfigSegment_barePrefixes
#[test]
fn test_strip_config_segment_bare_prefixes() {
    for p in [
        "bin/foo/bar.js",
        "genfiles/foo/bar.js",
        "testlogs/foo/bar.js",
    ] {
        assert_eq!(SourceFile::strip_config_segment(p), "foo/bar.js");
    }
}
// port: SourceFileTest#testStripConfigSegment_blazeOutPrefixes
#[test]
fn test_strip_config_segment_blaze_out_prefixes() {
    for p in [
        "blaze-out/k8-opt/bin/foo/bar.js",
        "blaze-out/k8-fastbuild/genfiles/foo/bar.js",
        "blaze-out/k8-dbg/testlogs/foo/bar.js",
        "/execroot/workspace/blaze-out/k8-opt/bin/foo/bar.js",
    ] {
        assert_eq!(SourceFile::strip_config_segment(p), "foo/bar.js");
    }
}
// port: SourceFileTest#testStripConfigSegment_bazelOutPrefixes
#[test]
fn test_strip_config_segment_bazel_out_prefixes() {
    for p in [
        "bazel-out/k8-opt/bin/foo/bar.js",
        "bazel-out/darwin_arm64-fastbuild/genfiles/foo/bar.js",
        "bazel-out/host/testlogs/foo/bar.js",
        "workspace/bazel-out/k8-opt/bin/foo/bar.js",
    ] {
        assert_eq!(SourceFile::strip_config_segment(p), "foo/bar.js");
    }
}
// port: SourceFileTest#testStripConfigSegment_unalteredPaths
#[test]
fn test_strip_config_segment_unaltered_paths() {
    for p in [
        "foo/bar.js",
        "my_bin/foo.js",
        "foo/bin/bar.js",
        "blaze-out/k8-opt",
        "blaze-out/",
        "blaze-out/k8-opt/other/foo.js",
        "bazel-out/",
        "bazel-out/k8-opt/other/foo.js",
        "",
    ] {
        assert_eq!(SourceFile::strip_config_segment(p), p);
    }
}
// port: SourceFileTest#testCanonicalizePathForMatching
#[test]
fn test_canonicalize_path_for_matching() {
    for p in [
        "blaze-out/k8-opt/bin/foo/bar.js",
        "bazel-out/k8-opt/bin/foo/bar.js",
        "bin/foo/bar.js",
        "foo/bar.js",
    ] {
        assert_eq!(SourceFile::canonicalize_path_for_matching(p), "foo/bar.js");
    }
}
// port: SourceFileTest#testRestoreCachedState_canonicalizedPathMatching
#[test]
fn test_restore_cached_state_canonicalized_path_matching() {
    let s = SourceFile::from_code("blaze-out/k8-opt/bin/foo/bar.js", "42");
    s.restore_cached_state_from(
        &SourceFileProto::new_builder()
            .set_filename("foo/bar.js")
            .set_num_lines_plus_one(10)
            .set_num_bytes_plus_one(20)
            .build(),
    );
    assert_eq!(s.get_num_lines(), 9);
    assert_eq!(s.get_num_bytes(), 19);
    s.restore_cached_state_from(
        &SourceFileProto::new_builder()
            .set_filename("bin/foo/bar.js")
            .set_num_lines_plus_one(15)
            .set_num_bytes_plus_one(25)
            .build(),
    );
    assert_eq!(s.get_num_lines(), 14);
    assert_eq!(s.get_num_bytes(), 24);
    let proto = SourceFileProto::new_builder()
        .set_filename("bazel-out/k8-opt/bin/other/bar.js")
        .build();
    assert!(std::panic::catch_unwind(|| s.restore_cached_state_from(&proto)).is_err());
}
