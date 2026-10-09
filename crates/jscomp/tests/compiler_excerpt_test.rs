/*
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/DiagnosticGroupTest.java,
//   test/com/google/javascript/jscomp/JSCompilerSourceExcerptProviderTest.java.

use closure_jscomp::{
    Compiler, compiler_options::CompilerOptions, region::Region, source_file::SourceFile,
};
use std::sync::Arc;

// port: JSCompilerSourceExcerptProviderTest#setUp
fn provider() -> Compiler {
    let mut compiler = Compiler::new();
    let sources = [
        ("foo", "foo:first line\nfoo:second line\nfoo:third line\n"),
        (
            "bar",
            "bar:first line\nbar:second line\nbar:third line\nbar:fourth line\n",
        ),
        ("foo2", "foo2:first line\nfoo2:second line\nfoo2:third line"),
    ]
    .map(|(name, code)| Arc::new(SourceFile::from_code(name, code)));
    compiler.init(&[], &sources, CompilerOptions::new());
    compiler
}

// port: JSCompilerSourceExcerptProviderTest#testExcerptOneLine
#[test]
fn test_excerpt_one_line() {
    let p = provider();
    for (name, count) in [("foo", 3), ("bar", 4)] {
        for (index, word) in ["first", "second", "third", "fourth"]
            .iter()
            .enumerate()
            .take(count)
        {
            assert_eq!(
                p.get_source_line(name, index as i32 + 1),
                Some(format!("{name}:{word} line"))
            );
        }
    }
}
// port: JSCompilerSourceExcerptProviderTest#testExcerptLineFromInexistentSource
#[test]
fn test_excerpt_line_from_inexistent_source() {
    let p = provider();
    for line in [1, 7, 90] {
        assert!(p.get_source_line("inexistent", line).is_none());
    }
}
// port: JSCompilerSourceExcerptProviderTest#testExcerptInexistentLine
#[test]
fn test_excerpt_inexistent_line() {
    let p = provider();
    for (name, line) in [("foo", 0), ("foo", 4), ("bar", 0), ("bar", 5)] {
        assert!(p.get_source_line(name, line).is_none());
    }
}
// port: JSCompilerSourceExcerptProviderTest#testExceptNoNewLine
#[test]
fn test_except_no_new_line() {
    let p = provider();
    for (index, word) in ["first", "second", "third"].iter().enumerate() {
        assert_eq!(
            p.get_source_line("foo2", index as i32 + 1),
            Some(format!("foo2:{word} line"))
        );
    }
    assert!(p.get_source_line("foo2", 4).is_none());
}
// port: JSCompilerSourceExcerptProviderTest#assertRegionWellFormed
fn assert_region_well_formed(p: &Compiler, name: &str, line: i32) {
    let region = p.get_source_region(name, line).unwrap();
    let source = region.get_source_excerpt();
    if line == 1 {
        assert_eq!(region.get_beginning_line_number(), 1);
    } else {
        assert!(region.get_beginning_line_number() <= line);
    }
    assert!(line <= region.get_ending_line_number());
    assert!(!source.is_empty());
    assert!(!source.starts_with('\n'));
    assert!(!source.ends_with('\n'));
    assert!(source.contains(&p.get_source_line(name, line).unwrap()));
}
// port: JSCompilerSourceExcerptProviderTest#testExcerptRegion
#[test]
fn test_excerpt_region() {
    let p = provider();
    for (name, count) in [("foo", 3), ("bar", 4)] {
        for line in 1..=count {
            assert_region_well_formed(&p, name, line);
        }
    }
}
// port: JSCompilerSourceExcerptProviderTest#testExcerptRegionFromInexistentSource
#[test]
fn test_excerpt_region_from_inexistent_source() {
    let p = provider();
    for line in [0, 6, 90] {
        assert!(p.get_source_region("inexistent", line).is_none());
    }
}
// port: JSCompilerSourceExcerptProviderTest#testExcerptInexistentRegion
#[test]
fn test_excerpt_inexistent_region() {
    let p = provider();
    for (name, line) in [("foo", 0), ("foo", 4), ("bar", 0), ("bar", 5)] {
        assert!(p.get_source_region(name, line).is_none());
    }
}
// port: DiagnosticGroupTest#testRegistration
#[test]
fn test_registration() {
    use closure_jscomp::diagnostic_groups::{DEPRECATED, DiagnosticGroups};
    assert!(Arc::ptr_eq(
        &DiagnosticGroups::for_name("deprecated").unwrap(),
        &DEPRECATED
    ));
}
