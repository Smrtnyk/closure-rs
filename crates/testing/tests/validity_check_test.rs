/*
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   test/com/google/javascript/jscomp/ValidityCheckTest.java.

//! Port of ValidityCheckTest (a CompilerTestCase) on the Rust CompilerTestCase port.
//!
//! Java's `otherPass` field holds an anonymous CompilerPass per test; here each test passes its
//! `otherPass` to set_up. The anonymous passes call `getLastCompiler()`, which is the compiler the
//! processor runs on, so they use the compiler handed to `process`. Java's `try { test(..);
//! fail(); } catch (IllegalStateException | RuntimeException e)` becomes a check that the run
//! fails (an exception Throwable or a panic) with a message containing the expected text.
mod var_checks_support;

use closure_jscomp::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    compiler_pass::CompilerPass,
    validity_check::ValidityCheck,
};
use closure_rhino::{
    node::{NodeId, Prop},
    token::Token,
};
use closure_testing::{compiler_test_case::CompilerTestCase, throwable::Throwable};
use std::panic::{AssertUnwindSafe, catch_unwind};
use var_checks_support::Hooks;

type OtherPass = fn(&mut AbstractCompiler, NodeId, NodeId);

/// ValidityCheckTest#getProcessor's anonymous CompilerPass.
struct Processor {
    other_pass: OtherPass,
}

impl CompilerPass for Processor {
    // port: ValidityCheckTest#getProcessor (the anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        (self.other_pass)(compiler, externs, root);
        ValidityCheck::new(compiler).process(compiler, externs, root);
    }
}

// port: ValidityCheckTest#setUp (with #getProcessor; otherPass is the test's assignment)
fn set_up(other_pass: OtherPass) -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    // port: ValidityCheckTest#getProcessor
    let hooks = Hooks::new("ValidityCheckTest", move |_| {
        Box::new(Processor { other_pass })
    });
    (harness, hooks)
}

/// The message of the exception a `test(..)` call ended with (a Throwable it returned, or the
/// panic it raised); fails like `assertWithMessage(..).fail()` when it completed normally.
fn exception_message(
    expected_exception: &str,
    run: impl FnOnce() -> Result<(), Throwable>,
) -> String {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(())) => panic!("Expected {expected_exception}"),
        Ok(Err(Throwable::Exception { message, .. })) => message.unwrap_or_default(),
        Ok(Err(other)) => panic!("Expected {expected_exception}, got {other}"),
        Err(payload) => payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default(),
    }
}

#[test]
fn test_unnormalize_node_types() {
    // port: ValidityCheckTest#testUnnormalizeNodeTypes (the anonymous otherPass)
    fn other_pass(compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let script = root.get_first_child(compiler).unwrap();
        let true_node = compiler.new_node(Token::TRUE);
        let empty = compiler.new_node(Token::EMPTY);
        let if_node = compiler.new_node_with_children2(Token::IF, true_node, empty);
        if_node.srcref_tree(compiler, script);
        root.get_first_child(compiler)
            .unwrap()
            .add_child_to_back(compiler, if_node);
        compiler.report_change_to_enclosing_scope(script);
    }
    let (mut t, mut h) = set_up(other_pass);
    let message = exception_message("IllegalStateException", || {
        t.test_strings(&mut h, "var x = 3;", "var x=3;0;0")
    });
    assert!(
        message.contains("Expected BLOCK but was EMPTY"),
        "{message}"
    );
}

#[test]
fn test_unnormalized() {
    // port: ValidityCheckTest#testUnnormalized (the anonymous otherPass)
    fn other_pass(compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
    }
    let (mut t, mut h) = set_up(other_pass);
    let message = exception_message("RuntimeException", || {
        t.test_same_string(&mut h, "while(1){}")
    });
    assert!(
        message.contains("Normalize constraints violated:\nWHILE node"),
        "{message}"
    );
}

#[test]
fn test_constant_annotation_mismatch() {
    // port: ValidityCheckTest#testConstantAnnotationMismatch (the anonymous otherPass)
    fn other_pass(compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let script = root.get_first_child(compiler).unwrap();
        let name = compiler.new_string_with_token(Token::NAME, "x");
        name.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        let expr_result = compiler.new_node_with_child(Token::EXPR_RESULT, name);
        let expr_result = expr_result.srcref_tree(compiler, script);
        script.add_child_to_back(compiler, expr_result);
        compiler.report_change_to_enclosing_scope(script);
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
    }
    let (mut t, mut h) = set_up(other_pass);
    let message = exception_message("RuntimeException", || {
        t.test_strings(&mut h, "var x;", "var x; x;")
    });
    assert!(
        message.contains("The name x is not consistently annotated as constant."),
        "{message}"
    );
}
