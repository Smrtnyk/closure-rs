/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/LiveVariablesAnalysisTest.java.

#![allow(dead_code)]

// port: LiveVariablesAnalysisTest#testStraightLine
#[test]
fn test_straight_line() {
    assert_not_live_before_x(r###"X:var a;"###, r###"a"###);
    assert_not_live_after_x(r###"X:var a;"###, r###"a"###);
    assert_not_live_after_x(r###"X:var a=1;"###, r###"a"###);
    assert_live_after_x(r###"X:var a=1; a()"###, r###"a"###);
    assert_not_live_before_x(r###"X:var a=1; a()"###, r###"a"###);
    assert_live_before_x(r###"var a;X:a;"###, r###"a"###);
    assert_live_before_x(r###"var a;X:a=a+1;"###, r###"a"###);
    assert_live_before_x(r###"var a;X:a+=1;"###, r###"a"###);
    assert_live_before_x(r###"var a;X:a++;"###, r###"a"###);
    assert_not_live_after_x(r###"var a,b;X:b();"###, r###"a"###);
    assert_not_live_before_x(r###"var a,b;X:b();"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:b(a);"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:b(1,2,3,b(a + 1));"###, r###"a"###);
    assert_not_live_before_x(r###"var a,b;X:a=1;b(a)"###, r###"a"###);
    assert_not_live_after_x(r###"var a,b;X:b(a);b()"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:b();b=1;a()"###, r###"b"###);
    assert_live_after_x(r###"X:a();var a;a()"###, r###"a"###);
    assert_not_live_after_x(r###"X:a();var a=1;a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:a,b=1"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testProperties
#[test]
fn test_properties() {
    assert_live_before_x(r###"var a,b;X:a.P;"###, r###"a"###);

    assert_live_before_x(r###"var a,b;X:a.P=1;b()"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:a.P.Q=1;b()"###, r###"a"###);

    assert_not_live_after_x(r###"var a,b;X:b.P.Q.a=1;"###, r###"a"###);

    assert_live_before_x(r###"var a,b;X:b.P.Q=a;"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testConditions
#[test]
fn test_conditions() {
    assert_live_before_x(r###"var a,b;X:if(a){}"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:if(a||b) {}"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:if(b||a) {}"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:if(b||b(a)) {}"###, r###"a"###);
    assert_not_live_after_x(r###"var a,b;X:b();if(a) {}"###, r###"b"###);

    assert_not_live_after_x(r###"var a,b;X:a();if(a=b){}a()"###, r###"a"###);
    assert_not_live_after_x(r###"var a,b;X:a();while(a=b){}a()"###, r###"a"###);

    assert_not_live_after_x(r###"var a,b;X:a();if((a=b)&&b){}a()"###, r###"a"###);
    assert_not_live_after_x(r###"var a,b;X:a();while((a=b)&&b){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b;a();X:if(b&&(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b;a();X:if(a&&(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b;a();X:while(b&&(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b;a();X:while(a&&(a=b)){}a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    assert_live_before_x(r###"var a,b;X:if(a??b) {}"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:if(b??a) {}"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:if(b??b(a)) {}"###, r###"a"###);

    assert_not_live_after_x(r###"var a,b;X:a();if((a=b)??b){}a()"###, r###"a"###);
    assert_not_live_after_x(r###"var a,b;X:a();while((a=b)??b){}a()"###, r###"a"###);

    assert_live_before_x(r###"var a,b; X:if(b??(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:if(a??(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:while(b??(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:while(a??(a=b)){}a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#logicalAssignment
#[test]
fn logical_assignment() {
    assert_live_before_x(r###"var a,b;X:a??=b"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:a??=b"###, r###"b"###);
}

// port: LiveVariablesAnalysisTest#optChainGetProp
#[test]
fn opt_chain_get_prop() {
    assert_not_live_before_x(r###"var a,b; X:if(b) {}"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:if(a?.b) {}"###, r###"a"###);

    assert_not_live_before_x(r###"var a,b;X:if(b?.a) {}"###, r###"a"###);

    assert_not_live_after_x(r###"var a,b;X:a();if((a=c)?.b){} a()"###, r###"a"###);
    assert_not_live_after_x(r###"var a,b;X:a();while((a=b)?.b){} a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#optChainCall
#[test]
fn opt_chain_call() {
    assert_live_before_x(r###"var a,b; X:if(b?.(a)){}"###, r###"a"###);

    assert_not_live_after_x(r###"var a,b; X:a(); if((a=b)?.b()){} a()"###, r###"a"###);

    assert_live_before_x(r###"var a,b; X:if(b?.(a=c)){} a();"###, r###"a"###);

    assert_live_before_x(r###"var a,b; X:if(b?.(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:if(a?.(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:while(b?.(a=b)){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:while(a?.(a=b)){}a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#optChainGetElem
#[test]
fn opt_chain_get_elem() {
    assert_live_before_x(r###"var a,b; X:if(b?.[a]) {}"###, r###"a"###);

    assert_not_live_after_x(r###"var a,b; X:a(); if((a=b)?.[b]){} a()"###, r###"a"###);

    assert_live_before_x(r###"var a,b; X:if(b?.[a=c]) {} a();"###, r###"a"###);

    assert_live_before_x(r###"var a,b; X:if(b?.[a=b]){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:if(a?.[a=b]){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:while(b?.[a=b]){}a()"###, r###"a"###);
    assert_live_before_x(r###"var a,b; X:while(a?.[a=b]){}a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testArrays
#[test]
fn test_arrays() {
    assert_live_before_x(r###"var a;X:a[1]"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:b[a]"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:b[1,2,3,4,b(a)]"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:b=[a,'a']"###, r###"a"###);
    assert_not_live_before_x(r###"var a,b;X:a=[];b(a)"###, r###"a"###);

    assert_live_before_x(r###"var a;X:a[1]=1"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testTwoPaths
#[test]
fn test_two_paths() {
    assert_live_before_x(r###"var a,b;X:if(b){b(a)}else{b(a)};"###, r###"a"###);

    assert_live_before_x(r###"var a,b;X:if(b){b(b)}else{b(a)};"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:if(b){b(a)}else{b(b)};"###, r###"a"###);

    assert_not_live_after_x(r###"var a,b;X:if(b){b(b)}else{b(b)};"###, r###"a"###);

    assert_live_before_x(r###"var a,b;X:if(b){b(b)}else{b(b)}a();"###, r###"a"###);

    assert_live_before_x(r###"var a;X:while(param1){a()};"###, r###"a"###);
    assert_live_before_x(r###"var a;X:while(param1){a=1};a()"###, r###"a"###);

    assert_live_before_x(r###"var a;X:if(param1){a()};"###, r###"a"###);
    assert_live_before_x(r###"var a;X:if(param1){a=1};a()"###, r###"a"###);

    assert_not_live_after_x(r###"X:var a;do{a=1}while(param1);a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testThreePaths
#[test]
fn test_three_paths() {
    assert_live_before_x(r###"var a;X:if(1){}else if(2){}else{a()};"###, r###"a"###);
    assert_live_before_x(r###"var a;X:if(1){}else if(2){a()}else{};"###, r###"a"###);
    assert_live_before_x(r###"var a;X:if(1){a()}else if(2){}else{};"###, r###"a"###);
    assert_live_before_x(r###"var a;X:if(1){}else if(2){}else{};a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testHooks
#[test]
fn test_hooks() {
    assert_live_before_x(r###"var a;X:1?a=1:1;a()"###, r###"a"###);

    assert_live_before_x(r###"var a,b;X:b=1?a:2"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testForLoops
#[test]
fn test_for_loops() {
    assert_not_live_before_x(r###"var a,b;for(a=0;a<9;a++){b(a)};X:b"###, r###"a"###);
    assert_not_live_before_x(r###"var a,b;for(a in b){a()};X:b"###, r###"a"###);
    assert_not_live_before_x(r###"var a,b;for(a in b){a()};X:a"###, r###"b"###);
    assert_live_before_x(r###"var b;for(var a in b){X:a()};"###, r###"a"###);

    assert_live_before_x(r###"var a,b;for(a=0;a<9;a++){X:1}"###, r###"a"###);
    assert_live_after_x(r###"var a,b;for(a in b){X:b};"###, r###"a"###);

    assert_live_before_x(r###"var a,b; X:for(a in b){ }"###, r###"a"###);

    assert_live_before_x(r###"var a,b;X:a();b();for(a in b){a()};"###, r###"a"###);

    assert_live_before_x(r###"var a,b;X:b;for(b=a;;){};"###, r###"a"###);
    assert_not_live_before_x(r###"var a,b;X:a;for(b=a;;){b()};b();"###, r###"b"###);
}

// port: LiveVariablesAnalysisTest#testForOfLoopsVar
#[test]
fn test_for_of_loops_var() {
    assert_live_before_x(r###"var a; for (a of [1, 2, 3]) {X:{}}"###, r###"a"###);
    assert_live_after_x(r###"for (var a of [1, 2, 3]) {X:{}}"###, r###"a"###);
    assert_live_before_x(
        r###"var a,b; for (var y of a = [0, 1, 2]) { X:a[y] }"###,
        r###"a"###,
    );
}

// port: LiveVariablesAnalysisTest#testForOfLoopsDestructuring
#[test]
fn test_for_of_loops_destructuring() {
    assert_live_before_x(
        r###"var key, value; X:for ([key, value] of arr) {value;} value;"###,
        r###"value"###,
    );
    assert_live_before_x(
        r###"let x = 3; X:for (var [y = x] of arr) { y; }"###,
        r###"x"###,
    );
    assert_live_before_x(
        r###"for (let [key, value] of arr) { X: key; value; }"###,
        r###"key"###,
    );
}

// port: LiveVariablesAnalysisTest#testForAwaitOfLoopsVar
#[test]
fn test_for_await_of_loops_var() {
    assert_live_before_x_with_async(
        r###"var a; for await (a of [1, 2, 3]) {X:{}}"###,
        r###"a"###,
        true,
    );
    assert_live_after_x_with_async(
        r###"for await (var a of [1, 2, 3]) {X:{}}"###,
        r###"a"###,
        true,
    );
    assert_live_before_x_with_async(
        r###"var a,b; for await (var y of a = [0, 1, 2]) { X:a[y] }"###,
        r###"a"###,
        true,
    );
}

// port: LiveVariablesAnalysisTest#testForAwaitOfLoopsDestructuring
#[test]
fn test_for_await_of_loops_destructuring() {
    assert_live_before_x_with_async(
        r###"var key, value; X:for await ([key, value] of arr) {value;} value;"###,
        r###"value"###,
        true,
    );
    assert_live_before_x_with_async(
        r###"let x = 3; X:for await (var [y = x] of arr) { y; }"###,
        r###"x"###,
        true,
    );
    assert_live_before_x_with_async(
        r###"for await (let [key, value] of arr) { X: key; value; }"###,
        r###"key"###,
        true,
    );
}

// port: LiveVariablesAnalysisTest#testNestedLoops
#[test]
fn test_nested_loops() {
    assert_live_before_x(r###"var a;X:while(1){while(1){a()}}"###, r###"a"###);
    assert_live_before_x(
        r###"var a;X:while(1){while(1){while(1){a()}}}"###,
        r###"a"###,
    );
    assert_live_before_x(r###"var a;X:while(1){while(1){a()};a=1}"###, r###"a"###);
    assert_live_after_x(r###"var a;while(1){while(1){a()};X:a=1;}"###, r###"a"###);
    assert_live_after_x(r###"var a;while(1){X:a=1;while(1){a()}}"###, r###"a"###);
    assert_not_live_before_x(
        r###"var a;X:1;do{do{do{a=1;}while(1)}while(1)}while(1);a()"###,
        r###"a"###,
    );
}

// port: LiveVariablesAnalysisTest#testSwitches
#[test]
fn test_switches() {
    assert_live_before_x(r###"var a,b;X:switch(a){}"###, r###"a"###);
    assert_live_before_x(r###"var a,b;X:switch(b){case(a):break;}"###, r###"a"###);
    assert_live_before_x(
        r###"var a,b;X:switch(b){case(b):case(a):break;}"###,
        r###"a"###,
    );
    assert_not_live_before_x(
        r###"var a,b;X:switch(b){case 1:a=1;break;default:a=2;break};a()"###,
        r###"a"###,
    );

    assert_live_before_x(r###"var a,b;X:switch(b){default:a();break;}"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testAssignAndReadInCondition
#[test]
fn test_assign_and_read_in_condition() {
    assert_live_before_x(
        r###"var a, b; X: if ((a = this) && (b = a)) {}"###,
        r###"a"###,
    );
    assert_not_live_before_x(r###"var a, b; X: a = 1, b = 1;"###, r###"a"###);
    assert_not_live_before_x(r###"var a; X: a = 1, a = 1;"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testParam
#[test]
fn test_param() {
    assert_not_live_after_x(r###"var a;X:a()"###, r###"param1"###);
    assert_live_before_x(r###"var a;X:a(param1)"###, r###"param1"###);
    assert_not_live_after_x(r###"var a;X:a();a(param2)"###, r###"param1"###);
}

// port: LiveVariablesAnalysisTest#testExpressionInForIn
#[test]
fn test_expression_in_for_in() {
    assert_live_before_x(r###"var a = [0]; X:for (a[1] in foo) { }"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testArgumentsArray
#[test]
fn test_arguments_array() {
    assert_escaped(r###"arguments[0]"###, r###"param1"###);
    assert_not_escaped(r###"arguments[0]"###, r###"param2"###);
    assert_not_escaped(r###"arguments[0]"###, r###"param3"###);

    assert_escaped(r###"var args = arguments"###, r###"param1"###);
    assert_not_escaped(r###"var args = arguments"###, r###"param2"###);
    assert_not_escaped(r###"var args = arguments"###, r###"param3"###);

    assert_not_escaped(r###"arguments = []"###, r###"param1"###);
    assert_not_escaped(r###"arguments = []"###, r###"param2"###);
    assert_not_escaped(r###"arguments = []"###, r###"param3"###);

    assert_escaped(r###"arguments[0] = 1"###, r###"param1"###);
    assert_not_escaped(r###"arguments[0] = 1"###, r###"param2"###);
    assert_not_escaped(r###"arguments[0] = 1"###, r###"param3"###);

    assert_escaped(r###"arguments[arguments[0]] = 1"###, r###"param1"###);
    assert_not_escaped(r###"arguments[arguments[0]] = 1"###, r###"param2"###);
    assert_not_escaped(r###"arguments[arguments[0]] = 1"###, r###"param3"###);
}

// port: LiveVariablesAnalysisTest#testArgumentsArray_doesNotEscape_destructuredParams
#[test]
fn test_arguments_array_does_not_escape_destructured_params() {
    assert_not_escaped_with_wrapper(
        r###"function f([a]) { arguments; }"###,
        r###"a"###,
        Wrapper::NONE,
    );
    assert_not_escaped_with_wrapper(
        r###"function f([a] = []) { arguments; }"###,
        r###"a"###,
        Wrapper::NONE,
    );
    assert_not_escaped_with_wrapper(
        r###"function f(...[a]) { arguments; }"###,
        r###"a"###,
        Wrapper::NONE,
    );
    assert_not_escaped_with_wrapper(
        r###"function f({a}) { arguments; }"###,
        r###"a"###,
        Wrapper::NONE,
    );
    assert_not_escaped_with_wrapper(
        r###"function f({a} = {}) { arguments; }"###,
        r###"a"###,
        Wrapper::NONE,
    );
}

// port: LiveVariablesAnalysisTest#testTryCatchFinally
#[test]
fn test_try_catch_finally() {
    assert_live_after_x(r###"var a; try {X:a=1} finally {a}"###, r###"a"###);
    assert_live_after_x(
        r###"var a; try {a()} catch(e) {X:a=1} finally {a}"###,
        r###"a"###,
    );

    assert_not_live_after_x(
        r###"var a = 1; try {
try {a()} catch(e) {X:1} } catch(E) {a}
"###,
        r###"a"###,
    );
    assert_live_after_x(
        r###"var a; while(1) { try {X:a=1;break} finally {a}}"###,
        r###"a"###,
    );
}

// port: LiveVariablesAnalysisTest#testForInAssignment
#[test]
fn test_for_in_assignment() {
    assert_live_before_x(
        r###"var a,b; for (var y in a = b) { X:a[y] }"###,
        r###"a"###,
    );

    assert_not_live_before_x(
        r###"var a,b; for (var y in a = b) { X:a[y] }"###,
        r###"b"###,
    );
    assert_live_before_x(
        r###"var a,b; for (var y in a = b) { X:a[y] }"###,
        r###"y"###,
    );
    assert_live_after_x(
        r###"var a,b; for (var y in a = b) { a[y]; X: y();}"###,
        r###"a"###,
    );
}

// port: LiveVariablesAnalysisTest#testExceptionThrowingAssignments
#[test]
fn test_exception_throwing_assignments() {
    assert_live_before_x(r###"try{var a; X:a=foo();a} catch(e) {e()}"###, r###"a"###);
    assert_live_before_x(r###"try{X:var a=foo();a} catch(e) {e()}"###, r###"a"###);
    assert_live_before_x(r###"try{X:var a=foo()} catch(e) {e(a)}"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testInnerFunctions
#[test]
fn test_inner_functions() {
    assert_live_before_x(r###"function a() {}; X: a()"###, r###"a"###);
    assert_not_live_before_x(r###"X:; function a() {}"###, r###"a"###);
    assert_live_before_x(
        r###"a = function(){}; function a() {}; X: a()"###,
        r###"a"###,
    );

    assert_live_after_x(
        r###"X: a = function(){}; function a() {}; a()"###,
        r###"a"###,
    );
    assert_not_live_before_x(
        r###"X: a = function(){}; function a() {}; a()"###,
        r###"a"###,
    );
}

// port: LiveVariablesAnalysisTest#testEscaped
#[test]
fn test_escaped() {
    assert_escaped(r###"var a;function b(){a()}"###, r###"a"###);
    assert_escaped(r###"var a;function b(){param1()}"###, r###"param1"###);
    assert_escaped(r###"var a;function b(){function c(){a()}}"###, r###"a"###);
    assert_escaped(
        r###"var a;function b(){param1.x = function() {a()}}"###,
        r###"a"###,
    );
    assert_not_escaped(r###"var a;function b(){var c; c()}"###, r###"c"###);
    assert_not_escaped(
        r###"var a;function f(){function b(){var c;c()}}"###,
        r###"c"###,
    );
    assert_not_escaped(r###"var a;function b(){};a()"###, r###"a"###);
    assert_not_escaped(r###"var a;function f(){function b(){}}a()"###, r###"a"###);
    assert_not_escaped(r###"var a;function b(){var a;a()};a()"###, r###"a"###);

    assert_escaped(r###"var _x"###, r###"_x"###);
}

// port: LiveVariablesAnalysisTest#testEscapedInClassStaticBlock
#[test]
fn test_escaped_in_class_static_block() {
    assert_escaped(r###"var a; class C{ static{a()}}"###, r###"a"###);
    assert_escaped(
        r###"var a; class C{static{ param1() } }"###,
        r###"param1"###,
    );
}

// port: LiveVariablesAnalysisTest#testNotEscapedInClassStaticBlock
#[test]
fn test_not_escaped_in_class_static_block() {
    assert_not_escaped(r###"var a; class C{static{}} a()"###, r###"a"###);
    assert_not_escaped(r###"let a; class C{static{let a;a()}}a()"###, r###"a"###);
    assert_not_escaped(r###"var a; class C{static{var c; c()}}"###, r###"c"###);
}

// port: LiveVariablesAnalysisTest#testNotEscapedWithCatch
#[test]
fn test_not_escaped_with_catch() {
    assert_escaped(r###"try{} catch(e){}"###, r###"e"###);
}

// port: LiveVariablesAnalysisTest#testEscapedLiveness
#[test]
fn test_escaped_liveness() {
    assert_not_live_before_x(r###"var a;X:a();function b(){a()}"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testBug1449316
#[test]
fn test_bug1449316() {
    assert_live_before_x(
        r###"try {var x=[]; X:var y=x[0]} finally {foo()}"###,
        r###"x"###,
    );
}

// port: LiveVariablesAnalysisTest#testSimpleLet
#[test]
fn test_simple_let() {
    assert_not_live_before_decl(r###"let a;"###, r###"a"###);
    assert_not_live_after_decl(r###"let a;"###, r###"a"###);
    assert_not_live_after_decl(r###"let a=1;"###, r###"a"###);

    assert_live_after_decl(r###"let a=1; a()"###, r###"a"###);
    assert_not_live_before_decl(r###"let a=1; a()"###, r###"a"###);

    assert_live_before_x(r###"let a;X:a;"###, r###"a"###);
    assert_not_live_after_x(r###"let a,b;X:b();"###, r###"a"###);
    assert_live_before_x(r###"let a,b;X:b(a);"###, r###"a"###);
    assert_not_live_before_x(r###"let a,b;X:a=1;b(a)"###, r###"a"###);
    assert_not_live_after_x(r###"let a,b;X:b(a);b()"###, r###"a"###);
    assert_live_before_x(r###"let a,b;X:b();b=1;a()"###, r###"b"###);

    assert_live_after_x(r###"X:a();let a;a()"###, r###"a"###);
    assert_not_live_after_x(r###"X:a();let a=1;a()"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testLetInnerBlock
#[test]
fn test_let_inner_block() {
    assert_not_live_after_x(r###"let x; { X:x = 2; let y; }"###, r###"x"###);
}

// port: LiveVariablesAnalysisTest#testSimpleConst
#[test]
fn test_simple_const() {
    assert_live_before_x(r###"const a = 4; X:a;"###, r###"a"###);
    assert_not_live_before_decl(r###"let a = 1;"###, r###"a"###);
    assert_not_live_before_decl(r###"const a = 1;"###, r###"a"###);
    assert_not_live_after_decl(r###"const a = 1;"###, r###"a"###);
}

// port: LiveVariablesAnalysisTest#testArrayDestructuring
#[test]
fn test_array_destructuring() {
    assert_live_before_x(r###"var [a, b] = [1, 2]; X:a;"###, r###"a"###);
    assert_not_live_before_x(r###"X: var [...a] = f();"###, r###"a"###);
    assert_not_escaped(r###"var [a, ...b] = [1, 2];"###, r###"b"###);
    assert_not_escaped(r###"var [a, ...b] = [1, 2];"###, r###"a"###);
    assert_not_escaped(r###"var [a, ,b] = [1, 2, 3];"###, r###"a"###);
    assert_not_escaped(r###"var [a, ,b] = [1, 2, 3];"###, r###"b"###);
    assert_not_live_before_x(r###"var x = 3; X: [x] = [4]; x;"###, r###"x"###);
    assert_live_before_x(r###"var x = {}; X: [x.a] = [3]; x.a;"###, r###"x"###);
    assert_live_before_x(r###"var x = []; X: var [c] = x;"###, r###"x"###);
}

// port: LiveVariablesAnalysisTest#testObjectDestructuring
#[test]
fn test_object_destructuring() {
    assert_live_before_x(r###"var {a: x, b: y} = g(); X:x"###, r###"x"###);
    assert_not_live_before_x(r###"X: var {a: x, b: y} = g();"###, r###"y"###);
    assert_not_escaped(r###"var {a: x, b: y} = g()"###, r###"x"###);
    assert_not_escaped(r###"var {a: x, b: y} = g()"###, r###"y"###);
    assert_not_escaped(r###"var {a: x = 3, b: y} = g();"###, r###"x"###);
    assert_not_live_before_x(r###"var x = {}; X: ({x} = {}); x;"###, r###"x"###);
    assert_live_before_x(r###"var x = {}; X: ({a: x.a} = {}); x.a;"###, r###"x"###);
    assert_live_before_x(r###"var x = {}; X: var {c} = x;"###, r###"x"###);
}

// port: LiveVariablesAnalysisTest#testComplexDestructuringPattern
#[test]
fn test_complex_destructuring_pattern() {
    assert_live_before_x(r###"var x = 3; X: var [y = x] = [];"###, r###"x"###);
    assert_live_before_x(r###"var x = 3, y; X: [y = x] = [];"###, r###"x"###);
    assert_live_before_x(r###"var x = 3; X: var {y = x} = {};"###, r###"x"###);
    assert_live_before_x(r###"var x = 3; X: var {key: y = x} = {};"###, r###"x"###);
    assert_live_before_x(
        r###"var x = 3; X: var {[x + x]: foo} = obj; x;"###,
        r###"x"###,
    );
    assert_live_before_x(
        r###"var x = 3; X: var {[x + x]: x} = obj; x;"###,
        r###"x"###,
    );

    assert_not_live_before_x(
        r###"var a, x, obj; a = 1; X: [x = (a = 2)] = obj; a();"###,
        r###"a"###,
    );
    assert_not_live_before_x(
        r###"var a, x, obj; a = 1; X: ({x = (a = 2)} = obj); a();"###,
        r###"a"###,
    );
}

// port: LiveVariablesAnalysisTest#testComplicatedDeclaration
#[test]
fn test_complicated_declaration() {
    assert_not_escaped(r###"var a = 1, {b: b} = f(), c = g()"###, r###"a"###);
    assert_not_escaped(r###"var a = 1, {b: b} = f(), c = g()"###, r###"b"###);
    assert_not_escaped(r###"var a = 1, {b: b} = f(), c = g()"###, r###"c"###);
}
use closure_jscomp::{
    Compiler,
    abstract_compiler::LifeCycleStage,
    compiler_options::{CompilerOptions, LanguageMode},
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::ControlFlowGraph,
    data_flow_analysis::{DataFlowAnalysis, LinearFlowState},
    google_coding_convention::GoogleCodingConvention,
    graph::{adjacency_graph::AdjacencyGraph, annotatable::Annotatable, graph_node::GraphNode},
    live_variables_analysis::{LiveVariableLattice, LiveVariablesAnalysis},
    node_util::NodeUtil,
    scope::ScopeId,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_rhino::{check_state, input_id::InputId, node::NodeId, token::Token};
use std::sync::Arc;
#[allow(non_camel_case_types, clippy::upper_case_acronyms)] // Java enum constants.
enum Wrapper {
    NONE,
    FUNCTION,
    ASYNC_FUNCTION,
}
// port: LiveVariablesAnalysisTest#computeLiveness(String)
fn compute_liveness(src: &str) -> (Compiler, LiveVariablesAnalysis) {
    compute_liveness_with_wrapper(src, Wrapper::FUNCTION)
}
// port: LiveVariablesAnalysisTest#computeLiveness(String, Wrapper)
fn compute_liveness_with_wrapper(src: &str, wrapper: Wrapper) -> (Compiler, LiveVariablesAnalysis) {
    let mut compiler = Compiler::new();
    let mut options = CompilerOptions::new();
    options.set_language(LanguageMode::UNSUPPORTED);
    options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
    compiler.init_options(options);
    compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
    let src = match wrapper {
        Wrapper::FUNCTION => format!("function _FUNCTION(param1, param2 = 1, ...param3){{{src}}}"),
        Wrapper::ASYNC_FUNCTION => {
            format!("async function _FUNCTION(param1, param2 = 1, ...param3){{{src}}}")
        }
        Wrapper::NONE => src.into(),
    };
    let parsed = compiler.parse_test_code(src.as_str());
    let n = parsed.remove_first_child(&mut compiler).unwrap();
    check_state!(n.is_function(&compiler), "%s", n.to_string(&compiler));
    let script = compiler.new_node_with_child(Token::SCRIPT, n);
    script.set_input_id(&mut compiler, Some(Arc::new(InputId::new("test"))));
    assert!(compiler.get_errors().is_empty());
    let mut scope_creator = SyntacticScopeCreator::new();
    let global = ScopeId::create_global_scope(&mut compiler, script);
    let scope = scope_creator.create_scope(&mut compiler, n, Some(global));
    let body = NodeUtil::get_function_body(&compiler, n);
    let child = scope_creator.create_scope(&mut compiler, body, Some(scope));
    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(n)
        .set_include_edge_annotations(true)
        .compute_cfg(&mut compiler);
    let all_vars =
        NodeUtil::get_all_vars_declared_in_function(&mut compiler, &mut scope_creator, scope);
    let mut analysis = LiveVariablesAnalysis::new(
        &mut compiler,
        cfg,
        scope,
        Some(child),
        &mut scope_creator,
        all_vars,
    );
    analysis.analyze(&mut compiler);
    (compiler, analysis)
}
// port: LiveVariablesAnalysisTest#getFlowStateAtX(String, boolean)
fn get_flow_state_at_x(
    src: &str,
    is_async: bool,
) -> (
    Compiler,
    LiveVariablesAnalysis,
    Option<LinearFlowState<LiveVariableLattice>>,
) {
    let (compiler, liveness) = compute_liveness_with_wrapper(
        src,
        if is_async {
            Wrapper::ASYNC_FUNCTION
        } else {
            Wrapper::FUNCTION
        },
    );
    let state = get_flow_state_at_x_node(
        &compiler,
        liveness
            .get_cfg()
            .get_entry()
            .get_value(liveness.get_cfg())
            .unwrap(),
        liveness.get_cfg(),
    );
    (compiler, liveness, state)
}
// port: LiveVariablesAnalysisTest#getFlowStateAtX(Node, ControlFlowGraph)
fn get_flow_state_at_x_node(
    compiler: &Compiler,
    node: NodeId,
    cfg: &ControlFlowGraph<NodeId>,
) -> Option<LinearFlowState<LiveVariableLattice>> {
    if node.is_label(compiler)
        && node.get_first_child(compiler).unwrap().get_string(compiler) == "X"
    {
        return cfg
            .get_node(&node.get_last_child(compiler))
            .unwrap()
            .get_annotation_as::<LinearFlowState<LiveVariableLattice>>(cfg)
            .cloned();
    }
    for c in node.children(compiler) {
        let state = get_flow_state_at_x_node(compiler, c, cfg);
        if state.is_some() {
            return state;
        }
    }
    None
}
// port: LiveVariablesAnalysisTest#getFlowStateAtDeclaration(String, String)
fn get_flow_state_at_declaration(
    src: &str,
    name: &str,
) -> (
    Compiler,
    LiveVariablesAnalysis,
    Option<LinearFlowState<LiveVariableLattice>>,
) {
    let (compiler, liveness) = compute_liveness(src);
    let state = get_flow_state_at_declaration_node(
        &compiler,
        liveness
            .get_cfg()
            .get_entry()
            .get_value(liveness.get_cfg())
            .unwrap(),
        liveness.get_cfg(),
        name,
    );
    (compiler, liveness, state)
}
// port: LiveVariablesAnalysisTest#getFlowStateAtDeclaration(Node, ControlFlowGraph, String)
fn get_flow_state_at_declaration_node(
    compiler: &Compiler,
    node: NodeId,
    cfg: &ControlFlowGraph<NodeId>,
    name: &str,
) -> Option<LinearFlowState<LiveVariableLattice>> {
    if NodeUtil::is_name_declaration(compiler, Some(node))
        && node.get_first_child(compiler).unwrap().get_string(compiler) == name
    {
        return cfg
            .get_node(&Some(node))
            .unwrap()
            .get_annotation_as::<LinearFlowState<LiveVariableLattice>>(cfg)
            .cloned();
    }
    for c in node.children(compiler) {
        let state = get_flow_state_at_declaration_node(compiler, c, cfg, name);
        if state.is_some() {
            return state;
        }
    }
    None
}
// port: LiveVariablesAnalysisTest#assertEscaped
fn assert_escaped(src: &str, name: &str) {
    let (compiler, liveness) = compute_liveness(src);
    for &var in liveness.get_escaped_locals() {
        if var.get_name(&compiler) == name {
            return;
        }
    }
    panic!("Variable {name} should be in the escaped local list.");
}
// port: LiveVariablesAnalysisTest#assertNotEscaped(String, String)
fn assert_not_escaped(src: &str, name: &str) {
    assert_not_escaped_with_wrapper(src, name, Wrapper::FUNCTION);
}
// port: LiveVariablesAnalysisTest#assertNotEscaped(String, String, Wrapper)
fn assert_not_escaped_with_wrapper(src: &str, name: &str, wrapper: Wrapper) {
    let (compiler, liveness) = compute_liveness_with_wrapper(src, wrapper);
    for &var in liveness.get_escaped_locals() {
        assert_ne!(
            var.get_name(&compiler),
            closure_rhino::js_string::JsString::from(name)
        );
    }
}

// port: LiveVariablesAnalysisTest#assertLiveBeforeX(String, String)
fn assert_live_before_x(src: &str, var: &str) {
    assert_live_before_x_with_async(src, var, false);
}

// port: LiveVariablesAnalysisTest#assertLiveBeforeX(String, String, boolean)
fn assert_live_before_x_with_async(src: &str, var: &str, is_async: bool) {
    let (_compiler, liveness, state) = get_flow_state_at_x(src, is_async);
    assert!(state.is_some(), "{src} should contain a label 'X:'");
    let state = state.unwrap();
    let live = state.get_in().is_live(liveness.get_var_index(&var.into()));
    assert!(live, "Variable {var} should be live before X");
}

// port: LiveVariablesAnalysisTest#assertLiveAfterX(String, String)
fn assert_live_after_x(src: &str, var: &str) {
    assert_live_after_x_with_async(src, var, false);
}

// port: LiveVariablesAnalysisTest#assertLiveAfterX(String, String, boolean)
fn assert_live_after_x_with_async(src: &str, var: &str, is_async: bool) {
    let (_compiler, liveness, state) = get_flow_state_at_x(src, is_async);
    assert!(state.is_some(), "Label X should be in the input program.");
    let state = state.unwrap();
    let live = state.get_out().is_live(liveness.get_var_index(&var.into()));
    assert!(live, "Variable {var} should be live after X");
}

// port: LiveVariablesAnalysisTest#assertNotLiveBeforeX
fn assert_not_live_before_x(src: &str, var: &str) {
    let (_compiler, liveness, state) = get_flow_state_at_x(src, false);
    assert!(state.is_some(), "Label X should be in the input program.");
    let state = state.unwrap();
    let live = state.get_in().is_live(liveness.get_var_index(&var.into()));
    assert!(!live, "Variable {var} should not be live before X");
}

// port: LiveVariablesAnalysisTest#assertNotLiveAfterX
fn assert_not_live_after_x(src: &str, var: &str) {
    let (_compiler, liveness, state) = get_flow_state_at_x(src, false);
    assert!(state.is_some(), "Label X should be in the input program.");
    let state = state.unwrap();
    let live = state.get_out().is_live(liveness.get_var_index(&var.into()));
    assert!(!live, "Variable {var} should not be live after X");
}

// port: LiveVariablesAnalysisTest#assertLiveAfterDecl
fn assert_live_after_decl(src: &str, var: &str) {
    let (_compiler, liveness, state) = get_flow_state_at_declaration(src, var);
    assert!(state.is_some(), "Variable {var} should be declared");
    let state = state.unwrap();
    let live = state.get_out().is_live(liveness.get_var_index(&var.into()));
    assert!(live, "Variable{var} should be live after its declaration");
}

// port: LiveVariablesAnalysisTest#assertNotLiveAfterDecl
fn assert_not_live_after_decl(src: &str, var: &str) {
    let (_compiler, liveness, state) = get_flow_state_at_declaration(src, var);
    assert!(state.is_some(), "Variable {var} should be declared");
    let state = state.unwrap();
    let live = state.get_out().is_live(liveness.get_var_index(&var.into()));
    assert!(
        !live,
        "Variable {var} should not be live after its declaration"
    );
}

// port: LiveVariablesAnalysisTest#assertNotLiveBeforeDecl
fn assert_not_live_before_decl(src: &str, var: &str) {
    let (_compiler, liveness, state) = get_flow_state_at_declaration(src, var);
    assert!(state.is_some(), "Variable {var} should be declared");
    let state = state.unwrap();
    let live = state.get_in().is_live(liveness.get_var_index(&var.into()));
    assert!(
        !live,
        "Variable {var} should not be live before its declaration"
    );
}
