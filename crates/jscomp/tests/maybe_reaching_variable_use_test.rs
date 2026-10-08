/*
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
//   test/com/google/javascript/jscomp/MaybeReachingVariableUseTest.java.

#![allow(dead_code)]
#[path = "support/reaching_use_def_tester.rs"]
mod reaching_use_def_tester;
use reaching_use_def_tester::ReachingUseDefTester;

// port: MaybeReachingVariableUseTest#testReachableUseUAtProgramPointD
#[test]
fn test_reachable_use_u_at_program_point_d() {
    assert_match(r###"var x = 1; D: var y = 2; y; U:x"###);
    assert_not_match(r###"D: var y = 1; var x = 2; y; U:x"###);
}

// port: MaybeReachingVariableUseTest#testStraightLine
#[test]
fn test_straight_line() {
    assert_match(r###"D:var x=1; U: x"###);
    assert_match(r###"var x; D:x=1; U: x"###);
    assert_not_match(r###"D:var x=1; x = 2; U: x"###);
    assert_match(r###"var x=1; D:x=2; U: x"###);
    assert_not_match(r###"U:x; D:var x = 1"###);
    assert_match(r###"D: var x = 1; var y = 2; y; U:x"###);
}

// port: MaybeReachingVariableUseTest#testIf
#[test]
fn test_if() {
    assert_match(r###"var x; if(a){ D:x=1 }else { x=2 }; U:x"###);
    assert_match(r###"var x; if(a){ x=1 }else { D:x=2 }; U:x"###);
    assert_match(r###"D:var x=1; if(a){ U1: x }else { U2: x };"###);

    assert_not_match(r###"D: var x; if(a){ x=1 }else { x=2 }; U:x"###);
}

// port: MaybeReachingVariableUseTest#testLoops
#[test]
fn test_loops() {
    assert_match(r###"var x=0; while(a){ D:x=1 }; U:x"###);
    assert_match(r###"var x=0; for(;cond;) { D:x=1 }; U:x"###);

    assert_not_match(r###"var x=0; for(;;) { D:x=1 }; U:x"###);
    assert_not_match(r###"var x=0; for(;true;) { D:x=1 }; U:x"###);
    assert_not_match(r###"var x=0; while (true) { D:x=1 }; U:x"###);

    assert_not_match(r###"D: var x=0; while (true) { y=1 }; U:x"###);

    assert_match(r###"D:var x=1; while(a) { U:x }"###);
    assert_match(r###"D:var x=1; for(;;)  { U:x }"###);
}

// port: MaybeReachingVariableUseTest#testShortCircuiting_usesOnlyCFGEdges
#[test]
fn test_short_circuiting_uses_only_cfg_edges() {
    assert_match(r###"var x=0; D: var y = false && (x=1); U:x"###);
    assert_match(r###"var x=0; D: var y = true || (x=1); U:x"###);
    assert_match(r###"var x=0; var y=0; D:(y=0)&&(x=1); U:x"###);

    assert_match(r###"D: var x=0; var y = true && (x=1); U:x"###);
    assert_match(r###"D: var x=0; var y = false || (x=1); U:x"###);
}

// port: MaybeReachingVariableUseTest#testConditional
#[test]
fn test_conditional() {
    assert_match(r###"var x=0; var y; D:(x=1)&&y; U:x"###);
    assert_not_match(r###"D: var x=0; var y; (x=1)&&(y); U:x"###);

    assert_match(r###"D: var x=0; var y=0; (y=1)&&((y=2)||(x=1)); U:x"###);
    assert_match(r###"D: var x=0; var y=1; (y)&&(x=1); U:x"###);
}

// port: MaybeReachingVariableUseTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    assert_match(r###"var x=0; var y; D:(x=1)??y; U:x"###);
    assert_not_match(r###"D: var x=0; var y; (x=1)??(y); U:x"###);

    assert_match(r###"var x=0; var y; D:y??(x=1); U:x"###);
    assert_match(r###"D: var x=0; var y; y??(x=1); U:x"###);
    assert_match(r###"D: var x=0; var y; (y)??((y)||(x=1)); U:x"###);

    assert_match(r###"var x=0; var y=1; D:(y=1)??(x=1); U:x"###);
    assert_match(r###"D: var x=0; var y; (y)&&((y)??(x=1)); U:x"###);
}

// port: MaybeReachingVariableUseTest#optChain
#[test]
fn opt_chain() {
    assert_match(r###"var x=0; var y; D:(x=1)?.y; U:x"###);
    assert_match(r###"var x=0; var y=0; D:(x=1)?.(y=0); U:x"###);
    assert_not_match(r###"D: var x=0; var y=0; (x=1)?.(y=0); U:x"###);

    assert_match(r###"var x=0; var y=0; D:(y=0)?.(x=1); U:x"###);
    assert_match(r###"var x=0; var y; D:y?.(x=1); U:x"###);

    assert_match(r###"var x=0; var y; D: y?.(true||(x=1)); U:x"###);

    assert_match(r###"D: var x=0; var y; y?.((y=2)||(x=1)); U:x"###);
    assert_match(r###"D: var x=0; var y; y?.(x=1); U:x"###);
}

// port: MaybeReachingVariableUseTest#testUseAndDefInSameInstruction
#[test]
fn test_use_and_def_in_same_instruction() {
    assert_not_match(r###"D:var x=0; U:x=1,x"###);
    assert_match(r###"D:var x=0; U:x,x=1"###);
}

// port: MaybeReachingVariableUseTest#testAssignmentInExpressions
#[test]
fn test_assignment_in_expressions() {
    assert_match(r###"var x=0; D:foo(bar(x=1)); U:x"###);
    assert_match(r###"var x=0; D:foo(bar + (x = 1)); U:x"###);
}

// port: MaybeReachingVariableUseTest#testHook
#[test]
fn test_hook() {
    assert_match(r###"var x=0; D:foo() ? x=1 : bar(); U:x"###);
    assert_match(r###"var x=0; D:foo() ? x=1 : x=2; U:x"###);

    assert_match(r###"D: var x=0; foo() ? x=1 : x=2; U:x"###);
}

// port: MaybeReachingVariableUseTest#testAssignmentOps
#[test]
fn test_assignment_ops() {
    assert_not_match(r###"D: var x = 0; U: x = 100"###);
    assert_match(r###"D: var x = 0; U: x += 100"###);
    assert_match(r###"D: var x = 0; U: x -= 100"###);
    assert_not_match(r###"D: var x = 0; x+=10; U:x"###);
}

// port: MaybeReachingVariableUseTest#testInc
#[test]
fn test_inc() {
    assert_match(r###"D: var x = 0; U:x++"###);
    assert_match(r###"var x = 0; D:x++; U:x"###);

    assert_match(r###"D: var x = 0; x++; U:x"###);
}

// port: MaybeReachingVariableUseTest#testForIn
#[test]
fn test_for_in() {
    assert_not_match(r###"D: var x = [], foo; U: for (x in foo) { }"###);
    assert_match(r###"D: var x = [], foo; for (x in foo) { U:x }"###);
    assert_match(r###"var x = [], foo; D: for (x in foo) { U:x }"###);
    assert_match(r###"var foo; D: for (let x in foo) { U:x }"###);
    assert_match(r###"var foo; D: for (const x in foo) { U:x }"###);
    assert_match(r###"D: var x = 1, foo; U: x; U: for (let [z = x] in foo) {}"###);
    assert_match(r###"D: var x = 1, foo; U: x; for (let [x] in foo) {}"###);
}

// port: MaybeReachingVariableUseTest#testForOf
#[test]
fn test_for_of() {
    assert_not_match(r###"D: var x = [], foo; U: for (x of foo) { }"###);
    assert_match(r###"D: var x = [], foo; for (x of foo) { U:x }"###);
    assert_match(r###"var x = [], foo; D: for (x of foo) { U:x }"###);
    assert_match(r###"var foo; D: for (let x of foo) { U:x }"###);
    assert_match(r###"var foo; D: for (const x of foo) { U:x }"###);
    assert_match(r###"D: var x = 1, foo; U: x; U: for (let [z = x] of foo) {}"###);
    assert_match(r###"D: var x = 1, foo; U: x; for (let [x] of foo) {}"###);
}

// port: MaybeReachingVariableUseTest#testForAwaitOf
#[test]
fn test_for_await_of() {
    assert_not_async_match(r###"D: var x = [], foo; U: for await (x of foo) { }"###);
    assert_async_match(r###"D: var x = [], foo; for await (x of foo) { U:x }"###);
    assert_async_match(r###"var x = [], foo; D: for await (x of foo) { U:x }"###);
    assert_async_match(r###"var foo; D: for await (let x of foo) { U:x }"###);
    assert_async_match(r###"var foo; D: for await (const x of foo) { U:x }"###);
    assert_async_match(r###"D: var x = 1, foo; U: x; U: for await (let [z = x] of foo) {}"###);
    assert_async_match(r###"D: var x = 1, foo; U: x; for await (let [x] of foo) {}"###);
}

// port: MaybeReachingVariableUseTest#testTryCatch
#[test]
fn test_try_catch() {
    assert_match(
        r###"D: var x = 1;
try { U: var y = foo() + x; } catch (e) {}
U: var z = x;
"###,
    );

    assert_match(
        r###"D: var x = 1;
try { x=2; U: var y = foo() + x; } catch (e) {} 
"###,
    );

    assert_match(
        r###"D: var x = 1;
try { x=2; U: var y = foo() + x; } catch (e) {}
U:x;
"###,
    );
}

// port: MaybeReachingVariableUseTest#testDestructuring
#[test]
fn test_destructuring() {
    assert_match(r###"D: var x = 1; U: var [y = x] = [];"###);
    assert_match(r###"D: var x = 1; var y; U: [y = x] = [];"###);
    assert_match(r###"D: var [x] = []; U: x;"###);
    assert_match(r###"var x; x = 3; D: [x] = 5; U: x;"###);
    assert_not_match(r###"D: var x; x = 3; [x] = 5; U: x;"###);
}

// port: MaybeReachingVariableUseTest#assertMatch
fn assert_match(src: &str) {
    assert_match_with_async(src, false);
}

// port: MaybeReachingVariableUseTest#assertAsyncMatch
fn assert_async_match(src: &str) {
    assert_match_with_async(src, true);
}

// port: MaybeReachingVariableUseTest#assertNotMatch
fn assert_not_match(src: &str) {
    assert_not_match_with_async(src, false);
}

// port: MaybeReachingVariableUseTest#assertNotAsyncMatch
fn assert_not_async_match(src: &str) {
    assert_not_match_with_async(src, true);
}

// port: MaybeReachingVariableUseTest#assertMatch(String, boolean)
fn assert_match_with_async(src: &str, is_async: bool) {
    let mut tester = ReachingUseDefTester::create();
    tester.compute_reaching_uses(src, is_async);
    tester.extract_def_and_uses_from_input_labels();
    let computed = tester.get_computed_uses();
    assert!(
        tester
            .get_extracted_uses()
            .iter()
            .all(|n| computed.contains(n))
    );
}
// port: MaybeReachingVariableUseTest#assertNotMatch(String, boolean)
fn assert_not_match_with_async(src: &str, is_async: bool) {
    let mut tester = ReachingUseDefTester::create();
    tester.compute_reaching_uses(src, is_async);
    tester.extract_def_and_uses_from_input_labels();
    let computed = tester.get_computed_uses();
    assert!(
        !tester
            .get_extracted_uses()
            .iter()
            .all(|n| computed.contains(n))
    );
}
