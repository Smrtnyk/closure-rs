/*
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
//   test/com/google/javascript/jscomp/RemoveUnnecessarySyntheticExternsTest.java.

//! Port of RemoveUnnecessarySyntheticExternsTest (a CompilerTestCase) on the Rust CompilerTestCase
//! port.
//!
//! Java builds the `syntheticExternsToAdd` nodes before any compiler exists and the processor adds
//! `declaration.cloneTree()` to the synthesized externs. Rust nodes live in a compiler's arena, so
//! a test records each declaration (`IR.var(IR.name(name))` and its synthesized-unfulfilled flag)
//! and the processor builds that tree in the running compiler: the same tree cloneTree yields.
mod var_checks_support;

use closure_jscomp::{
    compiler_pass::CompilerPass,
    remove_unnecessary_synthetic_externs::RemoveUnnecessarySyntheticExterns,
};
use closure_rhino::ir::IR;
use closure_testing::compiler_test_case::CompilerTestCase;
use std::{cell::RefCell, rc::Rc};
use var_checks_support::{Hooks, expected, expected_n, externs, srcs, test_extern_changes_parts};

/// A declaration node a test adds to `syntheticExternsToAdd`: `IR.var(IR.name(name))`, marked
/// `isSynthesizedUnfulfilledNameDeclaration` or not.
#[derive(Clone)]
struct Declaration {
    name: &'static str,
    synthesized_unfulfilled: bool,
}

impl Declaration {
    /// `IR.var(IR.name(name))`
    fn var(name: &'static str) -> Self {
        Self {
            name,
            synthesized_unfulfilled: false,
        }
    }
}

// use this set to simulate an earlier compiler pass declaring a synthetic extern
// (a LinkedHashSet of distinct Node instances: every add is a new node, so a list in add order)
type SyntheticExternsToAdd = Rc<RefCell<Vec<Declaration>>>;

/// RemoveUnnecessarySyntheticExternsTest#getProcessor's lambda.
struct Processor {
    synthetic_externs_to_add: SyntheticExternsToAdd,
}

impl CompilerPass for Processor {
    // port: RemoveUnnecessarySyntheticExternsTest#getProcessor (the lambda's process)
    fn process(
        &mut self,
        compiler: &mut closure_jscomp::abstract_compiler::AbstractCompiler,
        externs: closure_rhino::node::NodeId,
        js: closure_rhino::node::NodeId,
    ) {
        let input = compiler.get_synthesized_externs_input().clone();
        let synthetic_root = input.get_ast_root(compiler);
        for declaration in self.synthetic_externs_to_add.borrow().iter() {
            // declaration.cloneTree()
            let ast = &mut compiler.ast;
            let name = IR::name(ast, declaration.name);
            let clone = IR::var(ast, name);
            if declaration.synthesized_unfulfilled {
                clone.set_is_synthesized_unfulfilled_name_declaration(ast, true);
            }
            synthetic_root.add_child_to_back(ast, clone);
        }
        RemoveUnnecessarySyntheticExterns::new(compiler).process(compiler, externs, js);
    }
}

// port: RemoveUnnecessarySyntheticExternsTest#setUp (with #getProcessor)
fn set_up() -> (CompilerTestCase, Hooks, SyntheticExternsToAdd) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.enable_multistage_compilation().unwrap();
    let synthetic_externs_to_add: SyntheticExternsToAdd = Rc::new(RefCell::new(Vec::new()));
    let state = synthetic_externs_to_add.clone();
    // port: RemoveUnnecessarySyntheticExternsTest#getProcessor
    let hooks = Hooks::new("RemoveUnnecessarySyntheticExternsTest", move |_| {
        Box::new(Processor {
            synthetic_externs_to_add: state.clone(),
        })
    });
    (harness, hooks, synthetic_externs_to_add)
}

// port: RemoveUnnecessarySyntheticExternsTest#createUnfulfilledDeclaration
fn create_unfulfilled_declaration(name: &'static str) -> Declaration {
    // only VAR nodes are allowed to be marked "synthesized unfulfilled" so no need to test other
    // kinds of declarations.
    let mut declaration = Declaration::var(name);
    declaration.synthesized_unfulfilled = true;
    declaration
}

#[test]
fn doesnt_change_synthetic_extern_that_is_not_declared_in_code() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("x;"), expected("var x; var y;")]);
}

#[test]
fn doesnt_remove_synthetic_extern_if_shadowed_in_code() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("function fn() { var x; }"), expected("var x;")],
    );
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("{ let x; }"), expected("var x;")]);
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("{ const x = 0; }"), expected("var x;")],
    );
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("{ class x {} }"), expected("var x;")],
    );
}

#[test]
fn removes_synthetic_extern_declared_in_code() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("var x;"), expected("var y;")]);
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("{ var x; }"), expected("var y;")]);
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("let x;"), expected("var y;")]);
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("const x = 0;"), expected("var y;")],
    );
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("function x() {}"), expected("var y;")],
    );
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("class x {}"), expected("var y;")]);
}

#[test]
fn removes_synthetic_extern_declared_in_goog_provide() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("goog.provide('x');"), expected("var y;")],
    );
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("goog.provide('x.y.z');"), expected("var y;")],
    );
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![
            srcs("goog.provide('other'); goog.provide('x.y');"),
            expected("var y;"),
        ],
    );
}

#[test]
fn does_not_removes_synthetic_extern_for_legacy_goog_module() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("goog.module('x');"), expected("var x; var y;")],
    );
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("goog.module('x.y');"), expected("var x; var y;")],
    );
}

#[test]
fn removes_synthetic_extern_declared_in_legacy_goog_module_namespace() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![
            srcs("goog.module('x'); goog.module.declareLegacyNamespace();"),
            expected("var y;"),
        ],
    );
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![
            srcs("goog.module('x.y'); goog.module.declareLegacyNamespace();"),
            expected("var y;"),
        ],
    );
}

#[test]
fn removes_synthetic_extern_declared_in_other_externs() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![
            externs("var x;"),
            srcs(""),
            expected_n(&["var y;", "var x;"]),
        ],
    );
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![externs("var x;"), srcs("var y;"), expected("var x;")],
    );
}

#[test]
fn removes_distinct_synthetic_externs_declared_in_code() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("var x; var y;"), expected("")]);
}

#[test]
fn removes_duplicate_unfulfilled_synthetic_declarations() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    test_extern_changes_parts(&mut t, &mut h, vec![srcs(""), expected("var x; var y;")]);
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("var x;"), expected("var y;")]);
}

#[test]
fn removes_synthetic_extern_declared_in_code_multiple_code_declarations() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("y"));
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![srcs("var x; function x() {}"), expected("var y;")],
    );
}

#[test]
fn doesnt_change_synthetic_extern_declared_in_code_not_marked_unfulfilled() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    // add an extern that is not marked as an 'unfulfilled declaration'
    // we assume this extern was added to prevent renaming, not just enforce that all referenced
    // names are declared.
    synthetic_externs_to_add
        .borrow_mut()
        .push(Declaration::var("x"));
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("x;"), expected("var x;")]);
}

#[test]
fn removes_only_unfulfilled_synthetic_externs_if_mix_of_fulfilled_and_unfulfilled() {
    let (mut t, mut h, synthetic_externs_to_add) = set_up();
    // add an extern that is not marked as an 'unfulfilled declaration'
    // we assume this extern was added to actually prevent renaming, and so must not be removed
    // even if a duplicate of a non-synthetic extern.
    synthetic_externs_to_add
        .borrow_mut()
        .push(Declaration::var("x"));
    // add a duplicate declaration of 'x', where the second and third only are unfulfilled.
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    synthetic_externs_to_add
        .borrow_mut()
        .push(create_unfulfilled_declaration("x"));
    test_extern_changes_parts(&mut t, &mut h, vec![srcs(""), expected("var x;")]);
    test_extern_changes_parts(&mut t, &mut h, vec![srcs("var x;"), expected("var x;")]);
}
