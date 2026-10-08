/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/MustBeReachingVariableDefTest.java.

#![allow(dead_code)]
#[path = "support/reaching_use_def_tester.rs"]
mod reaching_use_def_tester;
use reaching_use_def_tester::ReachingUseDefTester;

// port: MustBeReachingVariableDefTest#testStraightLine
#[test]
fn test_straight_line() {
    assert_match(r###"D:var x=1; U: x"###);
    assert_match(r###"var x; D:x=1; U: x"###);
    assert_not_match(r###"D:var x=1; x = 2; U: x"###);
    assert_match(r###"var x=1; D:x=2; U: x"###);
    assert_not_match(r###"U:x; D:var x = 1"###);
    assert_not_match(r###"D:var x; U:x; x=1"###);
    assert_not_match(r###"D:var x; U:x; x=1; x"###);
    assert_match(r###"D: var x = 1; var y = 2; y; U:x"###);

    assert_match(r###"let x; D:x=1; U: x"###);
}

// port: MustBeReachingVariableDefTest#testIf
#[test]
fn test_if() {
    assert_not_match(r###"var x; if(a){ D:x=1 } else { x=2 }; U:x"###);
    assert_not_match(r###"var x; if(a){ x=1 } else { D:x=2 }; U:x"###);
    assert_match(r###"D:var x=1; if(a){ U:x } else { x };"###);
    assert_match(r###"D:var x=1; if(a){ x } else { U:x };"###);
    assert_not_match(r###"var x; if(a) { D: x = 1 }; U:x;"###);
}

// port: MustBeReachingVariableDefTest#testLoops
#[test]
fn test_loops() {
    assert_not_match(r###"var x=0; while(a){ D:x=1 }; U:x"###);
    assert_not_match(r###"var x=0; for(;;) { D:x=1 }; U:x"###);
    assert_match(r###"D:var x=1; while(a) { U:x }"###);
    assert_match(r###"D:var x=1; for(;;)  { U:x }"###);
}

// port: MustBeReachingVariableDefTest#testConditional
#[test]
fn test_conditional() {
    assert_match(r###"var x=0,y; D:(x=1)&&y; U:x"###);
    assert_not_match(r###"var x=0,y; D:y&&(x=1); U:x"###);
    assert_not_match(r###"D:var x=0; var y; y&&(x=1); U:x"###);
}

// port: MustBeReachingVariableDefTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    assert_match(r###"var x=0,y; D:(x=1)??y; U:x"###);

    assert_not_match(r###"var x=0; var y; D:y??(x=1); U:x"###);
    assert_not_match(r###"D:var x=0; var y; y??(x=1); U:x"###);
}

// port: MustBeReachingVariableDefTest#optChain
#[test]
fn opt_chain() {
    assert_match(r###"var x=0,y; D:(x=1)?.y; U:x"###);
    assert_match(r###"var x=0,y; D:(x=1)?.[y]; U:x"###);
    assert_match(r###"var x=0,y; D:(x=1)?.(y); U:x"###);
    assert_match(r###"var x=0,y,z; D:z(x=1)?.y; U:x"###);
    assert_match(r###"var x=0,y,z; D:z[x=1]?.y; U:x"###);

    assert_not_match(r###"var x = 0,y; D:y?.(x=1); U:x"###);
    assert_not_match(r###"D:var x = 0; var y; y?.(x=1); U:x"###);
    assert_not_match(r###"var x=0,y; D:y?.[x=1]; U:x"###);
}

// port: MustBeReachingVariableDefTest#testUseAndDefInSameInstruction
#[test]
fn test_use_and_def_in_same_instruction() {
    assert_match(r###"D:var x=0; U:x=1,x"###);
    assert_match(r###"D:var x=0; U:x,x=1"###);
}

// port: MustBeReachingVariableDefTest#testAssignmentInExpressions
#[test]
fn test_assignment_in_expressions() {
    assert_match(r###"var x=0; D:foo(bar(x=1)); U:x"###);
    assert_match(r###"var x=0; D:foo(bar + (x = 1)); U:x"###);
}

// port: MustBeReachingVariableDefTest#testHook
#[test]
fn test_hook() {
    assert_not_match(r###"var x=0; D:foo() ? x=1 : bar(); U:x"###);
    assert_not_match(r###"var x=0; D:foo() ? x=1 : x=2; U:x"###);
}

// port: MustBeReachingVariableDefTest#testExpressionVariableReassignment
#[test]
fn test_expression_variable_reassignment() {
    assert_match(r###"var a,b; D: var x = a + b; U:x"###);
    assert_not_match(r###"var a,b,c; D: var x = a + b; a = 1; U:x"###);
    assert_not_match(r###"var a,b,c; D: var x = a + b; f(b = 1); U:x"###);
    assert_match(r###"var a,b,c; D: var x = a + b; c = 1; U:x"###);

    assert_not_match(r###"var a,b,c; D: var x = a + b; c ? a = 1 : 0; U:x"###);
}

// port: MustBeReachingVariableDefTest#testMergeDefinitions
#[test]
fn test_merge_definitions() {
    assert_not_match(r###"var x,y; D: y = x + x; if(x) { x = 1 }; U:y"###);
}

// port: MustBeReachingVariableDefTest#testMergesWithOneDefinition
#[test]
fn test_merges_with_one_definition() {
    assert_not_match(r###"var x,y; while(y) { if (y) { print(x) } else { D: x = 1 } } U:x"###);
}

// port: MustBeReachingVariableDefTest#testRedefinitionUsingItself
#[test]
fn test_redefinition_using_itself() {
    assert_match(r###"var x = 1; D: x = x + 1; U:x;"###);
    assert_not_match(r###"var x = 1; D: x = x + 1; x = 1; U:x;"###);
}

// port: MustBeReachingVariableDefTest#testMultipleDefinitionsWithDependence
#[test]
fn test_multiple_definitions_with_dependence() {
    assert_match(r###"var x, a, b; D: x = a, x = b; U: x"###);
    assert_match(r###"var x, a, b; D: x = a, x = b; a = 1; U: x"###);
    assert_not_match(r###"var x, a, b; D: x = a, x = b; b = 1; U: x"###);
}

// port: MustBeReachingVariableDefTest#testExterns
#[test]
fn test_externs() {
    assert_not_match(r###"var goog = {}; D:goog = {}; U: goog"###);
}

// port: MustBeReachingVariableDefTest#testAssignmentOp
#[test]
fn test_assignment_op() {
    assert_match(r###"var x = 0; D: x += 1; U: x"###);
    assert_match(r###"var x = 0; D: x *= 1; U: x"###);
    assert_not_match(r###"D: var x = 0; x += 1; U: x"###);
}

// port: MustBeReachingVariableDefTest#testIncAndDec
#[test]
fn test_inc_and_dec() {
    assert_match(r###"var x; D: x++; U: x"###);
    assert_match(r###"var x; D: x--; U: x"###);
}

// port: MustBeReachingVariableDefTest#testFunctionParams1
#[test]
fn test_function_params1() {
    assert_not_match(r###"if (x) { D: x = 1; U: x }"###);
}

// port: MustBeReachingVariableDefTest#testFunctionParams2
#[test]
fn test_function_params2() {
    assert_not_match(r###"if (y) { D: x = 1} U: x"###);
}

// port: MustBeReachingVariableDefTest#testArgumentsObjectModifications
#[test]
fn test_arguments_object_modifications() {
    assert_not_match(r###"D: x = 1; arguments[0] = 2; U: x"###);
}

// port: MustBeReachingVariableDefTest#testArgumentsObjectEscaped
#[test]
fn test_arguments_object_escaped() {
    assert_not_match(r###"D: x = 1; var y = arguments; y[0] = 2; U: x"###);
}

// port: MustBeReachingVariableDefTest#testArgumentsObjectEscapedDependents
#[test]
fn test_arguments_object_escaped_dependents() {
    assert_not_match(r###"param1=1; var x; D:x=param1; var y=arguments; U:x"###);
}

// port: MustBeReachingVariableDefTest#testSideEffects
#[test]
fn test_side_effects() {
    assert_not_match(r###"var a = 1; D: var x = a; a++; U: print(x);"###);

    assert_match(r###"var a = 1; D: var x = a; U: print(a++, x);"###);
}

// port: MustBeReachingVariableDefTest#testDestructuringDefinitions
#[test]
fn test_destructuring_definitions() {
    assert_match(r###"D: var [x] = [1]; U: x;"###);
    assert_match(r###"D: var x = [1]; U: var [y] = [x];"###);
    assert_match(r###"var x = 1; D: [x] = [2]; U: x;"###);

    assert_not_match(r###"D: var x = 1; [x] = [2]; U: x;"###);
    assert_not_match(r###"var y = 1; D: var x = y; [y] = [2]; U: x;"###);

    assert_match(r###"var x; var y; D: [x] = [x = y]; y = 1; U: x;"###);
    assert_match(r###"var x; D: var [y] = [x = 1]; U: x;"###);
}

// port: MustBeReachingVariableDefTest#testDestructuringDefaultValue
#[test]
fn test_destructuring_default_value() {
    assert_not_match(r###"var x; D: var [y = x = 3] = []; U: x;"###);
    assert_not_match(r###"var x, obj = {}; D: [obj[x = 1] = x = 2] = []; U: x;"###);

    assert_match(r###"var x, obj = {}; D: [[obj[x = 1]] = [x = 2]] = []; U: x;"###);
    assert_match(r###"var x; D: [x = x = 3] = []; U: x"###);
}

// port: MustBeReachingVariableDefTest#assertMatch
fn assert_match(src: &str) {
    let mut tester = ReachingUseDefTester::create();
    tester.compute_reaching_def(src);
    tester.extract_def_and_uses_from_input_labels();
    assert_eq!(tester.get_computed_def(), Some(tester.get_extracted_def()));
}
// port: MustBeReachingVariableDefTest#assertNotMatch
fn assert_not_match(src: &str) {
    let mut tester = ReachingUseDefTester::create();
    tester.compute_reaching_def(src);
    tester.extract_def_and_uses_from_input_labels();
    assert_ne!(tester.get_computed_def(), Some(tester.get_extracted_def()));
}
