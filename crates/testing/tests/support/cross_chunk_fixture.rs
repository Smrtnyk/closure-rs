/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! The CompilerTestCase vocabulary the CrossChunk*Test ports use (`srcs`, `expected`, `externs`
//! with Java's overload choice), over the crates/testing CompilerTestCase port.
#![allow(dead_code)] // each test binary uses a different part

use closure_jscomp::{compiler_pass::CompilerPass, js_chunk::JSChunk};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

#[allow(unused_imports)] // not every test binary builds chunk graphs
pub use closure_testing::testing::js_chunk_graph_builder::JSChunkGraphBuilder;

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

/// A Java String argument of `srcs` / `expected` / `externs` (a literal or a concatenation).
pub trait JavaString {
    fn java_string(&self) -> JsString;
}
impl JavaString for &str {
    fn java_string(&self) -> JsString {
        JsString::from(*self)
    }
}
impl JavaString for String {
    fn java_string(&self) -> JsString {
        JsString::from(self.as_str())
    }
}

// port: CompilerTestCase#srcs(JSChunk...)
pub fn srcs(chunks: Vec<JSChunk>) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_chunks(chunks))
}

// port: CompilerTestCase#srcs(String) / srcs(String...)
pub fn srcs_strings(texts: &[&dyn JavaString]) -> TestPart {
    TestPart::Sources(if let [single] = texts {
        CompilerTestCase::srcs(single.java_string())
    } else {
        CompilerTestCase::srcs_strings(&texts.iter().map(|t| t.java_string()).collect::<Vec<_>>())
    })
}

// port: CompilerTestCase#expected(String) / expected(String...)
pub fn expected(texts: &[&dyn JavaString]) -> TestPart {
    TestPart::Expected(if let [single] = texts {
        CompilerTestCase::expected(single.java_string())
    } else {
        CompilerTestCase::expected_strings(
            &texts.iter().map(|t| t.java_string()).collect::<Vec<_>>(),
        )
    })
}

// port: CompilerTestCase#expected(JSChunk[])
pub fn expected_chunks(chunks: &[JSChunk]) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected_chunks(chunks).unwrap())
}

// port: CompilerTestCase#externs(String) / externs(String...)
pub fn externs(texts: &[&dyn JavaString]) -> TestPart {
    TestPart::Externs(if let [single] = texts {
        CompilerTestCase::externs(single.java_string())
    } else {
        CompilerTestCase::externs_strings(
            &texts.iter().map(|t| t.java_string()).collect::<Vec<_>>(),
        )
    })
}

/// JUnit: a test fails on any Throwable.
pub fn check(result: Result<(), Throwable>) {
    if let Err(t) = result {
        panic!("{t:?}");
    }
}
