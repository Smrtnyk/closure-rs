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
//   test/com/google/javascript/jscomp/InlineCostEstimatorTest.java.

//! Port of `InlineCostEstimatorTest.java`.

use closure_jscomp::{
    Compiler, code_printer, compiler_options::CompilerOptions,
    inline_cost_estimator::InlineCostEstimator,
};
use closure_rhino::node::NodeId;

// port: InlineCostEstimatorTest#parse
fn parse(compiler: &mut Compiler, js: &str) -> NodeId {
    let n = compiler.parse_test_code(js);
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
    n
}

// port: InlineCostEstimatorTest#minimize
fn minimize(js: &str) -> String {
    let mut compiler = Compiler::new();
    let mut options = CompilerOptions::new();
    options.set_line_length_threshold(i32::MAX);
    let root = parse(&mut compiler, js);
    code_printer::Builder::new(root)
        .set_compiler_options(&options)
        .build(&compiler)
        .to_string_lossy()
}

// port: InlineCostEstimatorTest#cost
fn cost(js: &str) -> i64 {
    let mut compiler = Compiler::new();
    let root = parse(&mut compiler, js);
    InlineCostEstimator::get_cost(&compiler, root) as i64
}

// port: InlineCostEstimatorTest#testCost
#[test]
fn test_cost() {
    check_cost("1", "1");
    check_cost("true", "1");
    check_cost("false", "1");
    check_cost("a", "xx");
    check_cost("a + b", "xx+xx");
    check_cost("foo()", "xx()");
    check_cost("foo(a,b)", "xx(xx,xx)");
    check_cost("10 + foo(a,b)", "0+xx(xx,xx)");
    check_cost("1 + foo(a,b)", "1+xx(xx,xx)");
    check_cost("a ? 1 : 0", "xx?1:0");
    check_cost("a.b", "xx.xx");
    check_cost("new Obj()", "new xx");
    check_cost(
        "function a() {return \"monkey\"}",
        "function xx(){return\"monkey\"}",
    );
}

// port: InlineCostEstimatorTest#checkCost
fn check_cost(source: &str, example: &str) {
    // The example string should have been minified already.
    assert_eq!(example, minimize(example));

    // cost estimate should be the same as the length of the example string.
    assert_eq!(cost(source), example.len() as i64);
}
