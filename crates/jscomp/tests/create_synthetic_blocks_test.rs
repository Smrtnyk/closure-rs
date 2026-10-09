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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/CreateSyntheticBlocksTest.java.

//! Port of `CreateSyntheticBlocksTest.java`: tests for `CreateSyntheticBlocks`.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization,
    compiler_options::CompilerOptions,
    compiler_pass::CompilerPass,
    create_synthetic_blocks::{
        CreateSyntheticBlocks, INVALID_MARKER_USAGE, UNMATCHED_END_MARKER, UNMATCHED_START_MARKER,
    },
    denormalize::Denormalize,
    diagnostic_type::DiagnosticType,
    minimize_exit_points::MinimizeExitPoints,
    node_util::NodeUtil,
    normalize::Normalize,
    peephole_fold_constants::PeepholeFoldConstants,
    peephole_minimize_conditions::PeepholeMinimizeConditions,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_remove_dead_code::PeepholeRemoveDeadCode,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

// port: CreateSyntheticBlocksTest#START_MARKER
const START_MARKER: &str = "startMarker";
// port: CreateSyntheticBlocksTest#END_MARKER
const END_MARKER: &str = "endMarker";

struct CreateSyntheticBlocksTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: CreateSyntheticBlocksTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options = harness.get_options()?;
        // This is used by NodeTraversal and not by the pass under test here
        options.set_synthetic_block_start_marker(START_MARKER.into());
        Ok(options)
    }

    // port: CreateSyntheticBlocksTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let name = self.get_name();
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, js: NodeId| {
                CreateSyntheticBlocks::new(START_MARKER, Some(END_MARKER.into()))
                    .process(compiler, externs, js);
                Normalize::create_normalize_for_optimizations(compiler)
                    .process(compiler, externs, js);
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = vec![
                    Box::new(MinimizeExitPoints::new()),
                    Box::new(PeepholeRemoveDeadCode::new()),
                    Box::new(PeepholeMinimizeConditions::new(/* late= */ true)),
                    Box::new(PeepholeFoldConstants::new(true, false /* useTypes */)),
                ];
                PeepholeOptimizationsPass::new(name.clone(), optimizations)
                    .process(compiler, externs, js);
                Denormalize::new(compiler, FeatureSet::BARE_MINIMUM).process(compiler, externs, js);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "CreateSyntheticBlocksTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl CreateSyntheticBlocksTest {
    // port: CompilerTestCase#CompilerTestCase()
    // port: CreateSyntheticBlocksTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        // Can't use compare as a tree because of the added synthetic blocks.
        harness.disable_compare_as_tree().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "CreateSyntheticBlocksTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
            },
        }
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_same(&mut self, js: &str) {
        self.harness
            .test_same_string(&mut self.hooks, js)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_error(&mut self, js: &str, error: &'static DiagnosticType) {
        self.harness
            .test_error(&mut self.hooks, js, error)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

// port: CreateSyntheticBlocksTest#testFold1
#[test]
fn test_fold1() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test(
        "function f() { if (x) return; y(); }",
        "function f(){x||y()}",
    );
}

// port: CreateSyntheticBlocksTest#testFoldWithMarkers1
#[test]
fn test_fold_with_markers1() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("function f(){startMarker();if(x)return;endMarker();y()}");
}

// port: CreateSyntheticBlocksTest#testFoldWithMarkers1a
#[test]
fn test_fold_with_markers1a() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("function f(){startMarker();if(x)return;endMarker()}");
}

// port: CreateSyntheticBlocksTest#testFold2
#[test]
fn test_fold2() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test(
        "function f() { if (x) return; y(); if (a) return; b(); }",
        "function f(){if(!x){y();a||b()}}",
    );
}

// port: CreateSyntheticBlocksTest#testFoldWithMarkers2
#[test]
fn test_fold_with_markers2() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same(concat!(
        "function f(){startMarker(\"FOO\");startMarker(\"BAR\");",
        "if(x)return;endMarker(\"BAR\");y();if(a)return;",
        "endMarker(\"FOO\");b()}",
    ));
}

// port: CreateSyntheticBlocksTest#testUnmatchedStartMarker
#[test]
fn test_unmatched_start_marker() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_error("startMarker()", &UNMATCHED_START_MARKER);
}

// port: CreateSyntheticBlocksTest#testUnmatchedEndMarker1
#[test]
fn test_unmatched_end_marker1() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_error("endMarker()", &UNMATCHED_END_MARKER);
}

// port: CreateSyntheticBlocksTest#testUnmatchedEndMarker2
#[test]
fn test_unmatched_end_marker2() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_error("if(y){startMarker();x()}endMarker()", &UNMATCHED_END_MARKER);
}

// port: CreateSyntheticBlocksTest#testInvalid1
#[test]
fn test_invalid1() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_error("startMarker() && true", &INVALID_MARKER_USAGE);
}

// port: CreateSyntheticBlocksTest#testInvalid2
#[test]
fn test_invalid2() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_error("false && endMarker()", &INVALID_MARKER_USAGE);
}

// port: CreateSyntheticBlocksTest#testDenormalize
#[test]
fn test_denormalize() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("startMarker();for(;;);endMarker()");
}

// port: CreateSyntheticBlocksTest#testNonMarkingUse
#[test]
fn test_non_marking_use() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("function foo(endMarker){}");
    t.test_same("function foo(){startMarker:foo()}");
}

// port: CreateSyntheticBlocksTest#testContainingBlockPreservation
#[test]
fn test_containing_block_preservation() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("if(y){startMarker();x();endMarker()}");
}

// port: CreateSyntheticBlocksTest#testArrowFunction
#[test]
fn test_arrow_function() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("var y=()=>{startMarker();x();endMarker()}");
    t.test_error(
        "var y=()=>{startMarker();x();};endMarker()",
        &UNMATCHED_END_MARKER,
    );
    t.test_error("var y=()=>startMarker();", &INVALID_MARKER_USAGE);
}

// port: CreateSyntheticBlocksTest#testFunctionDeclaration1
#[test]
fn test_function_declaration1() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test(
        "startMarker(); a(); function fn() {}; b(); endMarker()",
        "startMarker();var fn=function(){};a();b();endMarker()",
    );
}

// port: CreateSyntheticBlocksTest#testFunctionDeclaration2
#[test]
fn test_function_declaration2() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("startMarker();a();var fn=function(){};b();endMarker()");
}

// port: CreateSyntheticBlocksTest#testClassDeclaration1
#[test]
fn test_class_declaration1() {
    let mut t = CreateSyntheticBlocksTest::new();
    // Document that classes are mishandled with regard to block scoping
    t.test_same("startMarker();class C{}endMarker()");
}

// port: CreateSyntheticBlocksTest#testVariableDeclaration1
#[test]
fn test_variable_declaration1() {
    let mut t = CreateSyntheticBlocksTest::new();
    // Document that let and const are mishandled with regard to block scoping
    t.test_same("startMarker();var x=1;endMarker()");
    t.test_same("startMarker();let x=1;endMarker()");
    t.test_same("startMarker();const x=1;endMarker()");
}

// port: CreateSyntheticBlocksTest#testSyntheticBlock_doesNotCreateNewScope
#[test]
fn test_synthetic_block_does_not_create_new_scope() {
    let mut t = CreateSyntheticBlocksTest::new();
    t.test_same("startMarker();var x=1;endMarker()");
    let compiler = t.harness.get_last_compiler().unwrap();
    let c = compiler.borrow();
    let script = c.get_js_root().unwrap().get_only_child(&c);
    assert!(script.is_script(&c));
    let syncthetic_block = script.get_first_child(&c).unwrap();
    assert!(syncthetic_block.is_block(&c));
    assert!(syncthetic_block.is_synthetic_block(&c));
    // confirm that synthetic block does not create a new block scope
    assert!(!NodeUtil::creates_block_scope(&c, syncthetic_block));
}
