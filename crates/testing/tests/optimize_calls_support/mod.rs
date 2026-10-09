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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
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
