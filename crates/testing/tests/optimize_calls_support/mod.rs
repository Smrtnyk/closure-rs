/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit b2f69b3:
//   src/com/google/javascript/jscomp/NodeUtil.java,
//   src/com/google/javascript/jscomp/SourceFile.java,
//   src/com/google/javascript/jscomp/testing/CodeSubTree.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! The CompilerTestCase vocabulary the Rust ports of the optimize-calls test classes use
//! (`test(String, String)`, `testSame(String)`, `test(TestPart...)`, `srcs` / `expected` /
//! `externs`), over the crates/testing CompilerTestCase port.
#![allow(dead_code)] // each test binary uses a different part

use closure_jscomp::{compiler_pass::CompilerPass, source_file::SourceFile};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

/// A replay context for hooks that run natively (no record behind them).
pub fn native_ctx(class: &str) -> Ctx {
    Ctx::new(
        class.into(),
        closure_testing::replay::replay_values::object([]),
        IndexMap::<_, _>::default(),
        Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
    )
}

/// The value `getProcessor` returns.
pub fn pass(p: impl CompilerPass + 'static) -> DslValue {
    DslValue::Pass(Rc::new(RefCell::new(Box::new(p))))
}

/// JUnit: a test fails on any Throwable.
pub fn check(result: Result<(), Throwable>) {
    if let Err(t) = result {
        panic!("{t:?}");
    }
}

/// Java's `assertThrows(AssertionError.class, ..)` around a harness call: the AssertionError's
/// message.
pub fn assertion_message(result: Result<(), Throwable>) -> String {
    match result {
        Err(Throwable::Assertion { message }) => message,
        Err(t) => panic!("expected an AssertionError, got {t:?}"),
        Ok(()) => panic!("expected an AssertionError"),
    }
}

/// The text diff Java's `NodeSubject` puts in its message (Truth's `TextDiffFactsBuilder`: each
/// line of the expected and actual JS prefixed with ` `, `-` or `+`), rebuilt from the Rust
/// `NodeSubject` message, which shows the expected and the actual JS of a "JS diff" whole. Lines
/// are matched by a longest common subsequence; the hunk headers and the context limit of the
/// Java diff are not rebuilt, so this is for `contains` checks on changed lines.
pub fn js_text_diff(message: &str) -> String {
    let (_, rest) = message
        .split_once("JS diff\nexpected:\n")
        .expect("no JS diff in the message");
    let (expected, actual) = rest
        .split_once("\nactual:\n")
        .expect("no actual JS in the message");
    assert!(
        !actual.contains("JS diff\n"),
        "more than one JS diff in the message"
    );
    let e: Vec<&str> = expected.split('\n').collect();
    let a: Vec<&str> = actual.split('\n').collect();
    // lcs[i][j]: length of a longest common subsequence of e[i..] and a[j..].
    let mut lcs = vec![vec![0usize; a.len() + 1]; e.len() + 1];
    for i in (0..e.len()).rev() {
        for j in (0..a.len()).rev() {
            lcs[i][j] = if e[i] == a[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < e.len() || j < a.len() {
        if i < e.len() && j < a.len() && e[i] == a[j] {
            out.push(format!(" {}", e[i]));
            i += 1;
            j += 1;
        } else if i < e.len() && (j == a.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            out.push(format!("-{}", e[i]));
            i += 1;
        } else {
            out.push(format!("+{}", a[j]));
            j += 1;
        }
    }
    out.join("\n")
}

/// `CompilerTestCase.DEFAULT_EXTERNS + extra`.
pub fn default_externs_plus(extra: &str) -> JsString {
    CompilerTestCase::default_externs()
        .unwrap()
        .concat(&JsString::from(extra))
}

// port: CompilerTestCase#srcs(String)
pub fn srcs(text: &str) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs(text))
}

// port: CompilerTestCase#srcs(String...)
pub fn srcs_strings(texts: &[&str]) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_strings(
        &texts.iter().map(|t| JsString::from(*t)).collect::<Vec<_>>(),
    ))
}

// port: CompilerTestCase#expected(String)
pub fn expected(text: &str) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected(text))
}

// port: CompilerTestCase#expected(String...)
pub fn expected_strings(texts: &[&str]) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected_strings(
        &texts.iter().map(|t| JsString::from(*t)).collect::<Vec<_>>(),
    ))
}

// port: CompilerTestCase#externs(String)
pub fn externs(text: &str) -> TestPart {
    TestPart::Externs(CompilerTestCase::externs(text))
}

// port: CompilerTestCase#externs(String...)
pub fn externs_strings(texts: &[&str]) -> TestPart {
    TestPart::Externs(CompilerTestCase::externs_strings(
        &texts.iter().map(|t| JsString::from(*t)).collect::<Vec<_>>(),
    ))
}

/// A CompilerTestCase subclass instance: the harness plus the subclass's overrides.
pub struct Fixture<H: CompilerTestCaseHooks> {
    pub harness: CompilerTestCase,
    pub hooks: H,
}

impl<H: CompilerTestCaseHooks> Fixture<H> {
    // port: CompilerTestCase#test(String,String)
    pub fn test(&mut self, js: &str, expected: &str) {
        check(self.harness.test_strings(&mut self.hooks, js, expected));
    }
    // port: CompilerTestCase#testSame(String)
    pub fn test_same(&mut self, js: &str) {
        check(self.harness.test_same_string(&mut self.hooks, js));
    }
    // port: CompilerTestCase#test(TestPart...)
    pub fn test_parts(&mut self, parts: Vec<TestPart>) {
        check(self.harness.test(&mut self.hooks, parts));
    }
    // port: CompilerTestCase#testSame(TestPart...)
    pub fn test_same_parts(&mut self, parts: Vec<TestPart>) {
        check(self.harness.test_same(&mut self.hooks, parts));
    }
    /// `assertThrows(AssertionError.class, () -> testSame(js))`: the AssertionError's message.
    pub fn test_same_assertion_message(&mut self, js: &str) -> String {
        assertion_message(self.harness.test_same_string(&mut self.hooks, js))
    }
}

// port: SourceFile#fromCode(String,String)
pub fn source_file_from_code(name: &str, code: &str) -> std::sync::Arc<SourceFile> {
    std::sync::Arc::new(SourceFile::from_code(name, code))
}

// port: CompilerTestCase#externs(SourceFile...)
pub fn externs_files(files: &[std::sync::Arc<SourceFile>]) -> TestPart {
    TestPart::Externs(CompilerTestCase::externs_file_array(files).unwrap())
}

// port: NodeUtil#visitPreOrder
fn visit_pre_order(
    ast: &closure_rhino::node::Ast,
    node: closure_rhino::node::NodeId,
    visitor: &mut dyn FnMut(closure_rhino::node::NodeId),
) {
    visitor(node);
    for child in node.children(ast) {
        visit_pre_order(ast, child, visitor);
    }
}

// port: CodeSubTree#findNodesNonEmpty
pub fn find_nodes_non_empty(
    ast: &closure_rhino::node::Ast,
    root_node: closure_rhino::node::NodeId,
    predicate: impl Fn(closure_rhino::node::NodeId, &closure_rhino::node::Ast) -> bool,
) -> Vec<closure_rhino::node::NodeId> {
    let mut list_builder = Vec::new();
    visit_pre_order(ast, root_node, &mut |node| {
        if predicate(node, ast) {
            list_builder.push(node);
        }
    });
    assert!(!list_builder.is_empty(), "no nodes found");
    list_builder
}

// port: CompilerTestCase#srcs(JSChunk[])
pub fn srcs_chunks(chunks: Vec<closure_jscomp::js_chunk::JSChunk>) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_chunks(chunks))
}

/// `CompilerOptions.LanguageMode.<name>` as the harness's option value.
pub fn language_mode(name: &str) -> DslValue {
    DslValue::Enum {
        class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
        name: name.into(),
    }
}

/// Java's `assertThrows(RuntimeException.class, ..)` around a harness call, for the message
/// checks on `Throwables.getRootCause(e)`: a compiler crash is a panic in Rust (the root cause's
/// class is not kept), so this returns the panic message.
pub fn assert_throws_message(f: impl FnOnce()) -> String {
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .expect_err("expected a RuntimeException");
    if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    }
}
