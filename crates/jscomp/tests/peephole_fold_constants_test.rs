/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/PeepholeFoldConstantsTest.java.

//! Port of `PeepholeFoldConstantsTest.java`: tests for PeepholeFoldConstants in isolation. Tests
//! for the interaction of multiple peephole passes are in PeepholeIntegrationTest.
use closure_jscomp::{
    abstract_peephole_optimization::AbstractPeepholeOptimization,
    accessor_summary::AccessorSummary,
    compiler_options::CompilerOptions,
    compiler_pass::CompilerPass,
    diagnostic_groups as DiagnosticGroups,
    node_util::NodeUtil,
    peephole_fold_constants::PeepholeFoldConstants,
    peephole_fold_constants::{FRACTIONAL_BITWISE_OPERAND, INVALID_GETELEM_INDEX_ERROR},
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    source_file::SourceFile,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    testing::test_externs_builder::TestExternsBuilder,
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct PeepholeFoldConstantsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    late: bool,
    use_types: bool,
    num_repetitions: i32,
    assume_getters_pure: bool,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeFoldConstantsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = vec![Box::new(
            PeepholeFoldConstants::new(self.late, self.use_types),
        )];
        let pass: Box<dyn CompilerPass> = Box::new(PeepholeOptimizationsPass::new(
            self.get_name(),
            optimizations,
        ));
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: PeepholeFoldConstantsTest#getNumRepetitions
    fn get_num_repetitions(&self) -> i32 {
        self.num_repetitions
    }

    // port: PeepholeFoldConstantsTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_assume_getters_are_pure(self.assume_getters_pure);
        Ok(options)
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeFoldConstantsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: PeepholeFoldConstantsTest#LITERAL_OPERANDS
const LITERAL_OPERANDS: [&str; 16] = [
    "null",
    "undefined",
    "void 0",
    "true",
    "false",
    "!0",
    "!1",
    "0",
    "1",
    "''",
    "'123'",
    "'abc'",
    "'def'",
    "NaN",
    "Infinity",
    // TODO(nicksantos): Add more literals
    "-Infinity",
    // "({})",
    // "[]"
    // "[0]",
    // "Object",
    // "(function() {})"
];

impl PeepholeFoldConstantsTest {
    // port: PeepholeFoldConstantsTest#PeepholeFoldConstantsTest
    // port: PeepholeFoldConstantsTest#setUp
    fn new() -> Self {
        let externs = TestExternsBuilder::new()
            .add_array()
            .add_iterable()
            .add_object()
            .add_undefined()
            .add_function()
            .add_string()
            .add_infinity()
            .add_nan()
            .build();
        let mut harness = CompilerTestCase::new(externs);
        harness.set_up();
        harness.disable_type_check().unwrap();
        harness.enable_normalize().unwrap();
        harness.disable_compare_js_doc().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeFoldConstantsTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                late: false,
                use_types: true,
                // Reduce this to 1 if we get better expression evaluators.
                num_repetitions: 2,
                assume_getters_pure: false,
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

    fn test_same_warning(
        &mut self,
        js: &str,
        warning: &'static closure_jscomp::diagnostic_type::DiagnosticType,
    ) {
        self.harness
            .test_same_warning(&mut self.hooks, js, warning)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_parts(&mut self, parts: Vec<TestPart>) {
        self.harness
            .test(&mut self.hooks, parts)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    // port: PeepholeFoldConstantsTest#join
    fn join(operand_a: &str, op: &str, operand_b: &str) -> String {
        format!("{operand_a} {op} {operand_b}")
    }

    // port: PeepholeFoldConstantsTest#assertSameResults
    fn assert_same_results(&mut self, expr_a: &str, expr_b: &str) {
        assert_eq!(
            self.process(expr_b),
            self.process(expr_a),
            "Expressions did not fold the same\nexprA: {expr_a}\nexprB: {expr_b}"
        );
    }

    // port: PeepholeFoldConstantsTest#assertNotSameResults
    fn assert_not_same_results(&mut self, expr_a: &str, expr_b: &str) {
        assert!(
            self.process(expr_a) != self.process(expr_b),
            "Expressions folded the same\nexprA: {expr_a}\nexprB: {expr_b}"
        );
    }

    // port: PeepholeFoldConstantsTest#process
    fn process(&mut self, js: &str) -> JsString {
        self.print_helper(js, true)
    }

    // port: PeepholeFoldConstantsTest#printHelper
    fn print_helper(&mut self, js: &str, run_processor: bool) -> JsString {
        let compiler = self.hooks.create_compiler(&self.harness).unwrap();
        let options = self.hooks.get_options(&mut self.harness).unwrap();
        compiler.borrow_mut().init(
            &[],
            &[Arc::new(SourceFile::from_code("testcode", js))],
            options,
        );
        let root = compiler.borrow_mut().parse_inputs();
        compiler
            .borrow_mut()
            .set_accessor_summary(Arc::new(AccessorSummary::create(
                IndexMap::<_, _>::default(),
            )));
        let Some(root) = root else {
            let errors: Vec<String> = compiler
                .borrow()
                .get_errors()
                .iter()
                .map(|e| e.to_string())
                .collect();
            panic!(
                "Unexpected parse error(s): {}\nEXPR: {js}",
                errors.join("\n")
            );
        };
        let externs_root = root.get_first_child(&compiler.borrow()).unwrap();
        let main_root = externs_root.get_next(&compiler.borrow()).unwrap();
        if run_processor {
            let DslValue::Pass(pass) = self.hooks.get_processor(compiler.clone()).unwrap() else {
                unreachable!("getProcessor returns a CompilerPass");
            };
            pass.borrow_mut()
                .process(&mut compiler.borrow_mut(), externs_root, main_root);
        }
        compiler.borrow_mut().to_source_for_node_utf16(main_root)
    }

    // port: PeepholeFoldConstantsTest#foldBigIntTypes
    fn fold_big_int_types(&mut self, js: &str, expected: &str) {
        self.test(
            &format!("function f(/** @type {{bigint}} */ x) {{ {js} }}"),
            &format!("function f(/** @type {{bigint}} */ x) {{ {expected} }}"),
        );
    }

    // port: PeepholeFoldConstantsTest#foldNumericTypes
    fn fold_numeric_types(&mut self, js: &str, expected: &str) {
        self.test(
            &format!("function f(/** @type {{number}} */ x) {{ {js} }}"),
            &format!("function f(/** @type {{number}} */ x) {{ {expected} }}"),
        );
    }

    // port: PeepholeFoldConstantsTest#foldStringTypes
    fn fold_string_types(&mut self, js: &str, expected: &str) {
        self.test(
            &format!("function f(/** @type {{string}} */ x) {{ {js} }}"),
            &format!("function f(/** @type {{string}} */ x) {{ {expected} }}"),
        );
    }
}

// port: PeepholeFoldConstantsTest#testUndefinedComparison1
#[test]
fn test_undefined_comparison1() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("undefined == undefined", "true");
    t.test("undefined == null", "true");
    t.test("undefined == void 0", "true");
    t.test("undefined == 0", "false");
    t.test("undefined == 1", "false");
    t.test("undefined == 'hi'", "false");
    t.test("undefined == true", "false");
    t.test("undefined == false", "false");
    t.test("undefined === undefined", "true");
    t.test("undefined === null", "false");
    t.test("undefined === void 0", "true");
    t.test_same("undefined == this");
    t.test_same("undefined == x");
    t.test("undefined != undefined", "false");
    t.test("undefined != null", "false");
    t.test("undefined != void 0", "false");
    t.test("undefined != 0", "true");
    t.test("undefined != 1", "true");
    t.test("undefined != 'hi'", "true");
    t.test("undefined != true", "true");
    t.test("undefined != false", "true");
    t.test("undefined !== undefined", "false");
    t.test("undefined !== void 0", "false");
    t.test("undefined !== null", "true");
    t.test_same("undefined != this");
    t.test_same("undefined != x");
    t.test("undefined < undefined", "false");
    t.test("undefined > undefined", "false");
    t.test("undefined >= undefined", "false");
    t.test("undefined <= undefined", "false");
    t.test("0 < undefined", "false");
    t.test("true > undefined", "false");
    t.test("'hi' >= undefined", "false");
    t.test("null <= undefined", "false");
    t.test("undefined < 0", "false");
    t.test("undefined > true", "false");
    t.test("undefined >= 'hi'", "false");
    t.test("undefined <= null", "false");
    t.test("null == undefined", "true");
    t.test("0 == undefined", "false");
    t.test("1 == undefined", "false");
    t.test("'hi' == undefined", "false");
    t.test("true == undefined", "false");
    t.test("false == undefined", "false");
    t.test("null === undefined", "false");
    t.test("void 0 === undefined", "true");
    t.test("undefined == NaN", "false");
    t.test("NaN == undefined", "false");
    t.test("undefined == Infinity", "false");
    t.test("Infinity == undefined", "false");
    t.test("undefined == -Infinity", "false");
    t.test("-Infinity == undefined", "false");
    t.test("({}) == undefined", "false");
    t.test("undefined == ({})", "false");
    t.test("([]) == undefined", "false");
    t.test("undefined == ([])", "false");
    t.test("(/a/g) == undefined", "false");
    t.test("undefined == (/a/g)", "false");
    t.test("(function(){}) == undefined", "false");
    t.test("undefined == (function(){})", "false");
    t.test("undefined != NaN", "true");
    t.test("NaN != undefined", "true");
    t.test("undefined != Infinity", "true");
    t.test("Infinity != undefined", "true");
    t.test("undefined != -Infinity", "true");
    t.test("-Infinity != undefined", "true");
    t.test("({}) != undefined", "true");
    t.test("undefined != ({})", "true");
    t.test("([]) != undefined", "true");
    t.test("undefined != ([])", "true");
    t.test("(/a/g) != undefined", "true");
    t.test("undefined != (/a/g)", "true");
    t.test("(function(){}) != undefined", "true");
    t.test("undefined != (function(){})", "true");
    t.test_same("this == undefined");
    t.test_same("x == undefined");
}

// port: PeepholeFoldConstantsTest#testUndefinedComparison2
#[test]
fn test_undefined_comparison2() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("\"123\" !== void 0", "true");
    t.test("\"123\" === void 0", "false");
    t.test("void 0 !== \"123\"", "true");
    t.test("void 0 === \"123\"", "false");
}

// port: PeepholeFoldConstantsTest#testUndefinedComparison3
#[test]
fn test_undefined_comparison3() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("\"123\" !== undefined", "true");
    t.test("\"123\" === undefined", "false");
    t.test("undefined !== \"123\"", "true");
    t.test("undefined === \"123\"", "false");
}

// port: PeepholeFoldConstantsTest#testUndefinedComparison4
#[test]
fn test_undefined_comparison4() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("1 !== void 0", "true");
    t.test("1 === void 0", "false");
    t.test("null !== void 0", "true");
    t.test("null === void 0", "false");
    t.test("undefined !== void 0", "false");
    t.test("undefined === void 0", "true");
}

// port: PeepholeFoldConstantsTest#testNullComparison1
#[test]
fn test_null_comparison1() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("null == undefined", "true");
    t.test("null == null", "true");
    t.test("null == void 0", "true");
    t.test("null == 0", "false");
    t.test("null == 1", "false");
    t.test("null == 0n", "false");
    t.test("null == 1n", "false");
    t.test("null == 'hi'", "false");
    t.test("null == true", "false");
    t.test("null == false", "false");
    t.test("null === undefined", "false");
    t.test("null === null", "true");
    t.test("null === void 0", "false");
    t.test_same("null === x");
    t.test_same("null == this");
    t.test_same("null == x");
    t.test("null != undefined", "false");
    t.test("null != null", "false");
    t.test("null != void 0", "false");
    t.test("null != 0", "true");
    t.test("null != 1", "true");
    t.test("null != 0n", "true");
    t.test("null != 1n", "true");
    t.test("null != 'hi'", "true");
    t.test("null != true", "true");
    t.test("null != false", "true");
    t.test("null !== undefined", "true");
    t.test("null !== void 0", "true");
    t.test("null !== null", "false");
    t.test_same("null != this");
    t.test_same("null != x");
    t.test("null < null", "false");
    t.test("null > null", "false");
    t.test("null >= null", "true");
    t.test("null <= null", "true");
    t.test("0 < null", "false");
    t.test("0 > null", "false");
    t.test("0 >= null", "true");
    t.test("0n < null", "false");
    t.test("0n > null", "false");
    t.test("0n >= null", "true");
    t.test("true > null", "true");
    t.test("'hi' < null", "false");
    t.test("'hi' >= null", "false");
    t.test("null <= null", "true");
    t.test("null < 0", "false");
    t.test("null < 0n", "false");
    t.test("null > true", "false");
    t.test("null < 'hi'", "false");
    t.test("null >= 'hi'", "false");
    t.test("null <= null", "true");
    t.test("null == null", "true");
    t.test("0 == null", "false");
    t.test("1 == null", "false");
    t.test("'hi' == null", "false");
    t.test("true == null", "false");
    t.test("false == null", "false");
    t.test("null === null", "true");
    t.test("void 0 === null", "false");
    t.test("null == NaN", "false");
    t.test("NaN == null", "false");
    t.test("null == Infinity", "false");
    t.test("Infinity == null", "false");
    t.test("null == -Infinity", "false");
    t.test("-Infinity == null", "false");
    t.test("({}) == null", "false");
    t.test("null == ({})", "false");
    t.test("([]) == null", "false");
    t.test("null == ([])", "false");
    t.test("(/a/g) == null", "false");
    t.test("null == (/a/g)", "false");
    t.test("(function(){}) == null", "false");
    t.test("null == (function(){})", "false");
    t.test("null != NaN", "true");
    t.test("NaN != null", "true");
    t.test("null != Infinity", "true");
    t.test("Infinity != null", "true");
    t.test("null != -Infinity", "true");
    t.test("-Infinity != null", "true");
    t.test("({}) != null", "true");
    t.test("null != ({})", "true");
    t.test("([]) != null", "true");
    t.test("null != ([])", "true");
    t.test("(/a/g) != null", "true");
    t.test("null != (/a/g)", "true");
    t.test("(function(){}) != null", "true");
    t.test("null != (function(){})", "true");
    t.test_same("({a:f()}) == null");
    t.test_same("null == ({a:f()})");
    t.test_same("([f()]) == null");
    t.test_same("null == ([f()])");
    t.test_same("this == null");
    t.test_same("x == null");
}

// port: PeepholeFoldConstantsTest#testBooleanBooleanComparison
#[test]
fn test_boolean_boolean_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("!x == !y");
    t.test_same("!x < !y");
    t.test_same("!x !== !y");
    t.test_same("!x == !x");
    // foldable
    t.test_same("!x < !x");
    // foldable
    t.test_same("!x !== !x");
    // foldable
}

// port: PeepholeFoldConstantsTest#testBooleanNumberComparison
#[test]
fn test_boolean_number_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("!x == +y");
    t.test_same("!x <= +y");
    t.test("!x !== +y", "true");
}

// port: PeepholeFoldConstantsTest#testNumberBooleanComparison
#[test]
fn test_number_boolean_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("+x == !y");
    t.test_same("+x <= !y");
    t.test("+x === !y", "false");
}

// port: PeepholeFoldConstantsTest#testBooleanStringComparison
#[test]
fn test_boolean_string_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("!x == '' + y");
    t.test_same("!x <= '' + y");
    t.test("!x !== '' + y", "true");
}

// port: PeepholeFoldConstantsTest#testStringBooleanComparison
#[test]
fn test_string_boolean_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("'' + x == !y");
    t.test_same("'' + x <= !y");
    t.test("'' + x === !y", "false");
}

// port: PeepholeFoldConstantsTest#testNumberNumberComparison
#[test]
fn test_number_number_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("1 > 1", "false");
    t.test("2 == 3", "false");
    t.test("3.6 === 3.6", "true");
    t.test_same("+x > +y");
    t.test_same("+x == +y");
    t.test_same("+x === +y");
    t.test_same("+x == +x");
    t.test_same("+x === +x");
    t.test_same("+x > +x");
    // foldable
}

// port: PeepholeFoldConstantsTest#testStringStringComparison
#[test]
fn test_string_string_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("'a' < 'b'", "true");
    t.test("'a' <= 'b'", "true");
    t.test("'a' > 'b'", "false");
    t.test("'a' >= 'b'", "false");
    t.test("+'a' < +'b'", "false");
    t.test_same("typeof a < 'a'");
    t.test_same("'a' >= typeof a");
    t.test("typeof a < typeof a", "false");
    t.test("typeof a >= typeof a", "true");
    t.test("typeof 3 > typeof 4", "false");
    t.test("typeof function() {} < typeof function() {}", "false");
    t.test("'a' == 'a'", "true");
    t.test("'b' != 'a'", "true");
    t.test_same("'undefined' == typeof a");
    t.test_same("typeof a != 'number'");
    t.test_same("'undefined' == typeof a");
    t.test_same("'undefined' == typeof a");
    t.test("typeof a == typeof a", "true");
    t.test("'a' === 'a'", "true");
    t.test("'b' !== 'a'", "true");
    t.test("typeof a === typeof a", "true");
    t.test("typeof a !== typeof a", "false");
    t.test_same("'' + x <= '' + y");
    t.test_same("'' + x != '' + y");
    t.test_same("'' + x === '' + y");
    t.test_same("'' + x <= '' + x");
    // potentially foldable
    t.test_same("'' + x != '' + x");
    // potentially foldable
    t.test_same("'' + x === '' + x");
    // potentially foldable
}

// port: PeepholeFoldConstantsTest#testNumberStringComparison
#[test]
fn test_number_string_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("1 < '2'", "true");
    t.test("2 > '1'", "true");
    t.test("123 > '34'", "true");
    t.test("NaN >= 'NaN'", "false");
    t.test("1 == '2'", "false");
    t.test("1 != '1'", "false");
    t.test("NaN == 'NaN'", "false");
    t.test("1 === '1'", "false");
    t.test("1 !== '1'", "true");
    t.test_same("+x > '' + y");
    t.test_same("+x == '' + y");
    t.test("+x !== '' + y", "true");
}

// port: PeepholeFoldConstantsTest#testStringNumberComparison
#[test]
fn test_string_number_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("'1' < 2", "true");
    t.test("'2' > 1", "true");
    t.test("'123' > 34", "true");
    t.test("'NaN' < NaN", "false");
    t.test("'1' == 2", "false");
    t.test("'1' != 1", "false");
    t.test("'NaN' == NaN", "false");
    t.test("'1' === 1", "false");
    t.test("'1' !== 1", "true");
    t.test_same("'' + x < +y");
    t.test_same("'' + x == +y");
    t.test("'' + x === +y", "false");
}

// port: PeepholeFoldConstantsTest#testBigIntNumberComparison
#[test]
fn test_big_int_number_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("1n < 2", "true");
    t.test("1n > 2", "false");
    t.test("1n == 1", "true");
    t.test("1n == 2", "false");
    // comparing with decimals is allowed
    t.test("1n < 1.1", "true");
    t.test("1n < 1.9", "true");
    t.test("1n < 0.9", "false");
    t.test("-1n < -1.1", "false");
    t.test("-1n < -1.9", "false");
    t.test("-1n < -0.9", "true");
    t.test("1n > 1.1", "false");
    t.test("1n > 0.9", "true");
    t.test("-1n > -1.1", "true");
    t.test("-1n > -0.9", "false");
    // Don't fold unsafely large numbers because there might be floating-point error
    let max_safe_int: i64 = 9007199254740991;
    t.test(&format!("0n > {max_safe_int}"), "false");
    t.test(&format!("0n < {max_safe_int}"), "true");
    t.test(&format!("0n > {}", -max_safe_int), "true");
    t.test(&format!("0n < {}", -max_safe_int), "false");
    t.test_same(&format!("0n > {}", max_safe_int + 1));
    t.test_same(&format!("0n < {}", max_safe_int + 1));
    t.test_same(&format!("0n > {}", -(max_safe_int + 1)));
    t.test_same(&format!("0n < {}", -(max_safe_int + 1)));
    // comparing with Infinity is allowed
    t.test("1n < Infinity", "true");
    t.test("1n > Infinity", "false");
    t.test("1n < -Infinity", "false");
    t.test("1n > -Infinity", "true");
    // null is interpreted as 0 when comparing with bigint
    t.test("1n < null", "false");
    t.test("1n > null", "true");
}

// port: PeepholeFoldConstantsTest#testBigIntStringComparison
#[test]
fn test_big_int_string_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("1n < '2'", "true");
    t.test("1n <= '2'", "true");
    t.test("1n > '2'", "false");
    t.test("1n >= '2'", "false");
    t.test("1n < '1'", "false");
    t.test("1n <= '1'", "true");
    t.test("1n > '1'", "false");
    t.test("1n >= '1'", "true");
    t.test("1n == '1'", "true");
    t.test("1n != '1'", "false");
    t.test("2n > '1'", "true");
    t.test("123n > '34'", "true");
    t.test("1n == '2'", "false");
    t.test("1n === '1'", "false");
    t.test("1n !== '1'", "true");
    t.test("10n == '  10  '", "true");
    t.test("10n < '  20  '", "true");
    t.test("16n == '0x10'", "true");
    t.test("2n == '0b10'", "true");
    t.test("8n == '0o10'", "true");
    t.test("-1n == '-1'", "true");
    t.test("1n == '+1'", "true");
    // Invalid BigInt strings (StringToBigInt returns undefined) evaluate to false for all
    // relational comparisons
    t.test("1n < 'foo'", "false");
    t.test("1n > 'foo'", "false");
    t.test("1n <= 'foo'", "false");
    t.test("1n >= 'foo'", "false");
    t.test("1n == 'foo'", "false");
    t.test("1n != 'foo'", "true");
    t.test("1n < '1e2'", "false");
    t.test("1n > '1e2'", "false");
    t.test("1n <= '1e2'", "false");
    t.test("1n >= '1e2'", "false");
    t.test("1n == '1e2'", "false");
    t.test("1n != '1e2'", "true");
    t.test("1n < '1.5'", "false");
    t.test("1n > '1.5'", "false");
    t.test("1n <= '1.5'", "false");
    t.test("1n >= '1.5'", "false");
    t.test("1n == '1.5'", "false");
    t.test("1n != '1.5'", "true");
    t.test("1n < 'Infinity'", "false");
    t.test("1n > 'Infinity'", "false");
    t.test("1n <= 'Infinity'", "false");
    t.test("1n >= 'Infinity'", "false");
    t.test("1n < '-Infinity'", "false");
    t.test("1n > '-Infinity'", "false");
    t.test("1n <= '-Infinity'", "false");
    t.test("1n >= '-Infinity'", "false");
}

// port: PeepholeFoldConstantsTest#testStringBigIntComparison
#[test]
fn test_string_big_int_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("'2' < 1n", "false");
    t.test("'2' <= 1n", "false");
    t.test("'2' > 1n", "true");
    t.test("'2' >= 1n", "true");
    t.test("'1' < 1n", "false");
    t.test("'1' <= 1n", "true");
    t.test("'1' > 1n", "false");
    t.test("'1' >= 1n", "true");
    t.test("'1' == 1n", "true");
    t.test("'1' != 1n", "false");
    t.test("'1' < 2n", "true");
    t.test("'123' > 34n", "true");
    t.test("'1' == 2n", "false");
    t.test("'1' === 1n", "false");
    t.test("'1' !== 1n", "true");
    t.test("'  10  ' == 10n", "true");
    t.test("'  20  ' > 10n", "true");
    t.test("'0x10' == 16n", "true");
    t.test("'0b10' == 2n", "true");
    t.test("'0o10' == 8n", "true");
    t.test("'-1' == -1n", "true");
    t.test("'+1' == 1n", "true");
    t.test("'foo' < 1n", "false");
    t.test("'foo' > 1n", "false");
    t.test("'foo' <= 1n", "false");
    t.test("'foo' >= 1n", "false");
    t.test("'foo' == 1n", "false");
    t.test("'foo' != 1n", "true");
    t.test("'1e2' < 1n", "false");
    t.test("'1e2' > 1n", "false");
    t.test("'1e2' <= 1n", "false");
    t.test("'1e2' >= 1n", "false");
    t.test("'1.5' < 1n", "false");
    t.test("'1.5' > 1n", "false");
    t.test("'1.5' <= 1n", "false");
    t.test("'1.5' >= 1n", "false");
}

// port: PeepholeFoldConstantsTest#testBigIntEqualityWithNullOrUndefined
#[test]
fn test_big_int_equality_with_null_or_undefined() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("null == 1n", "false");
    t.test("undefined == 1n", "false");
    t.test("1n == null", "false");
    t.test("1n == undefined", "false");
    t.test("null != 1n", "true");
    t.test("undefined != 1n", "true");
    t.test("1n != null", "true");
    t.test("1n != undefined", "true");
}

// port: PeepholeFoldConstantsTest#testBigIntEqualityWithVariables
#[test]
fn test_big_int_equality_with_variables() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("x == 1n");
    t.test_same("1n == y");
    t.test_same("x != 1n");
    t.test_same("1n != y");
}

// port: PeepholeFoldConstantsTest#testNaNComparison
#[test]
fn test_na_n_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("NaN < 1", "false");
    t.test("NaN <= 1", "false");
    t.test("NaN > 1", "false");
    t.test("NaN >= 1", "false");
    t.test("NaN < 1n", "false");
    t.test("NaN <= 1n", "false");
    t.test("NaN > 1n", "false");
    t.test("NaN >= 1n", "false");
    t.test("NaN < NaN", "false");
    t.test("NaN >= NaN", "false");
    t.test("NaN == NaN", "false");
    t.test("NaN === NaN", "false");
    t.test("NaN < null", "false");
    t.test("null >= NaN", "false");
    t.test("NaN == null", "false");
    t.test("null != NaN", "true");
    t.test("null === NaN", "false");
    t.test("NaN < undefined", "false");
    t.test("undefined >= NaN", "false");
    t.test("NaN == undefined", "false");
    t.test("undefined != NaN", "true");
    t.test("undefined === NaN", "false");
    t.test_same("NaN < x");
    t.test_same("x >= NaN");
    t.test_same("NaN == x");
    t.test_same("x != NaN");
    t.test("NaN === x", "false");
    t.test("x !== NaN", "true");
    t.test_same("NaN == foo()");
}

// port: PeepholeFoldConstantsTest#testObjectComparison1
#[test]
fn test_object_comparison1() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("!new Date()", "false");
    t.test("!!new Date()", "true");
    t.test("new Date() == null", "false");
    t.test("new Date() == undefined", "false");
    t.test("new Date() != null", "true");
    t.test("new Date() != undefined", "true");
    t.test("null == new Date()", "false");
    t.test("undefined == new Date()", "false");
    t.test("null != new Date()", "true");
    t.test("undefined != new Date()", "true");
}

// port: PeepholeFoldConstantsTest#testUnaryOps
#[test]
fn test_unary_ops() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Running on just changed code results in an exception on only the first invocation. Don't
    // repeat because it confuses the exception verification.
    t.hooks.num_repetitions = 1;
    // These cases are handled by PeepholeRemoveDeadCode.
    t.test_same("!foo()");
    t.test_same("~foo()");
    t.test_same("-foo()");
    // These cases are handled here.
    t.test("a=!true", "a=false");
    t.test("a=!10", "a=false");
    t.test("a=!false", "a=true");
    t.test_same("a=!foo()");
    t.test("a=-0", "a=-0.0");
    t.test("a=-(0)", "a=-0.0");
    t.test_same("a=-Infinity");
    t.test("a=-NaN", "a=NaN");
    t.test_same("a=-foo()");
    t.test("a=~~0", "a=0");
    t.test("a=~~10", "a=10");
    t.test("a=~-7", "a=6");
    t.test("a=+true", "a=1");
    t.test("a=+10", "a=10");
    t.test("a=+false", "a=0");
    t.test_same("a=+foo()");
    t.test_same("a=+f");
    t.test("a=+(f?true:false)", "a=+(f?1:0)");
    // TODO(johnlenz): foldable
    t.test("a=+0", "a=0");
    t.test("a=+Infinity", "a=Infinity");
    t.test("a=+NaN", "a=NaN");
    t.test("a=+-7", "a=-7");
    t.test("a=+.5", "a=.5");
    t.test("a=~0xffffffff", "a=0");
    t.test("a=~~0xffffffff", "a=-1");
    t.test_same_warning("a=~.5", &FRACTIONAL_BITWISE_OPERAND);
}

// port: PeepholeFoldConstantsTest#testUnaryOpsWithBigInt
#[test]
fn test_unary_ops_with_big_int() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("-(1n)", "-1n");
    t.test("- -1n", "1n");
    t.test("!1n", "false");
    t.test("~0n", "-1n");
}

// port: PeepholeFoldConstantsTest#testUnaryOpsStringCompare
#[test]
fn test_unary_ops_string_compare() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("a = -1");
    t.test("a = ~0", "a = -1");
    t.test("a = ~1", "a = -2");
    t.test("a = ~101", "a = -102");
}

// port: PeepholeFoldConstantsTest#testFoldLogicalOp
#[test]
fn test_fold_logical_op() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = true && x", "x = x");
    t.test("x = [foo()] && x", "x = ([foo()],x)");
    t.test("x = false && x", "x = false");
    t.test("x = true || x", "x = true");
    t.test("x = false || x", "x = x");
    t.test("x = 0 && x", "x = 0");
    t.test("x = 3 || x", "x = 3");
    t.test("x = 0n && x", "x = 0n");
    t.test("x = 3n || x", "x = 3n");
    t.test("x = false || 0", "x = 0");
    // unfoldable, because the right-side may be the result
    t.test("a = x && true", "a=x && true");
    t.test("a = x && false", "a=x && false");
    t.test("a = x || 3", "a=x || 3");
    t.test("a = x || false", "a=x || false");
    t.test("a = b ? c : x || false", "a=b ? c:x || false");
    t.test("a = b ? x || false : c", "a=b ? x || false:c");
    t.test("a = b ? c : x && true", "a=b ? c:x && true");
    t.test("a = b ? x && true : c", "a=b ? x && true:c");
    // folded, but not here.
    t.test_same("a = x || false ? b : c");
    t.test_same("a = x && true ? b : c");
    t.test("x = foo() || true || bar()", "x = foo() || true");
    t.test("x = foo() || true && bar()", "x = foo() || bar()");
    t.test("x = foo() || false && bar()", "x = foo() || false");
    t.test("x = foo() && false && bar()", "x = foo() && false");
    t.test("x = foo() && false || bar()", "x = (foo() && false,bar())");
    t.test("x = foo() || false || bar()", "x = foo() || bar()");
    t.test("x = foo() && true && bar()", "x = foo() && bar()");
    t.test("x = foo() || true || bar()", "x = foo() || true");
    t.test("x = foo() && false && bar()", "x = foo() && false");
    t.test("x = foo() && 0 && bar()", "x = foo() && 0");
    t.test("x = foo() && 1 && bar()", "x = foo() && bar()");
    t.test("x = foo() || 0 || bar()", "x = foo() || bar()");
    t.test("x = foo() || 1 || bar()", "x = foo() || 1");
    t.test("x = foo() && 0n && bar()", "x = foo() && 0n");
    t.test("x = foo() && 1n && bar()", "x = foo() && bar()");
    t.test("x = foo() || 0n || bar()", "x = foo() || bar()");
    t.test("x = foo() || 1n || bar()", "x = foo() || 1n");
    t.test_same("x = foo() || bar() || baz()");
    t.test_same("x = foo() && bar() && baz()");
    t.test("0 || b()", "b()");
    t.test("1 && b()", "b()");
    t.test("a() && (1 && b())", "a() && b()");
    t.test("(a() && 1) && b()", "a() && b()");
    t.test("(x || '') || y;", "x || y");
    t.test("false || (x || '');", "x || ''");
    t.test("(x && 1) && y;", "x && y");
    t.test("true && (x && 1);", "x && 1");
    // Really not foldable, because it would change the type of the
    // expression if foo() returns something truthy but not true.
    // Cf. FoldConstants.tryFoldAndOr().
    // An example would be if foo() is 1 (truthy) and bar() is 0 (falsey):
    // (1 && true) || 0 == true
    // 1 || 0 == 1, but true =/= 1
    t.test_same("x = foo() && true || bar()");
    t.test_same("foo() && true || bar()");
}

// port: PeepholeFoldConstantsTest#testFoldLogicalOp2
#[test]
fn test_fold_logical_op2() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = function(){} && x", "x = x");
    t.test("x = true && function(){}", "x = function(){}");
    t.test(
        "x = [(function(){alert(x)})()] && x",
        "x = ([(function(){alert(x)})()],x)",
    );
}

// port: PeepholeFoldConstantsTest#testFoldNullishCoalesce
#[test]
fn test_fold_nullish_coalesce() {
    let mut t = PeepholeFoldConstantsTest::new();
    // fold if left is null/undefined
    t.test("null ?? 1", "1");
    t.test("undefined ?? false", "false");
    t.test("(a(), null) ?? 1", "(a(), null, 1)");
    t.test("x = [foo()] ?? x", "x = [foo()]");
    // short circuit on all non nullish LHS
    t.test("x = false ?? x", "x = false");
    t.test("x = true ?? x", "x = true");
    t.test("x = 0 ?? x", "x = 0");
    t.test("x = 3 ?? x", "x = 3");
    // unfoldable, because the right-side may be the result
    t.test_same("a = x ?? true");
    t.test_same("a = x ?? false");
    t.test_same("a = x ?? 3");
    t.test_same("a = b ? c : x ?? false");
    t.test_same("a = b ? x ?? false : c");
    // folded, but not here.
    t.test_same("a = x ?? false ? b : c");
    t.test_same("a = x ?? true ? b : c");
    t.test_same("x = foo() ?? true ?? bar()");
    t.test("x = foo() ?? (true && bar())", "x = foo() ?? bar()");
    t.test_same("x = (foo() || false) ?? bar()");
    t.test("a() ?? (1 ?? b())", "a() ?? 1");
    t.test("(a() ?? 1) ?? b()", "a() ?? 1 ?? b()");
}

// port: PeepholeFoldConstantsTest#testFoldOptChain
#[test]
fn test_fold_opt_chain() {
    let mut t = PeepholeFoldConstantsTest::new();
    // can't fold when optional part may execute
    t.test_same("a = x?.y");
    t.test_same("a = x?.()");
    // fold args of optional call
    t.test("x = foo() ?. (true && bar())", "x = foo() ?.(bar())");
    t.test("a() ?. (1 ?? b())", "a() ?. (1)");
    t.test("({a})?.a.b.c.d()?.x.y.z", "a.b.c.d()?.x.y.z");
    // potential optimization
    t.test_same("x = undefined?.y");
    // `x = void 0;`
}

// port: PeepholeFoldConstantsTest#testFoldOptChain_nonNullReceiver
#[test]
fn test_fold_opt_chain_non_null_receiver() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Constant folding on literals with optional chaining
    t.test("x = 'hello'?.length", "x = 5");
    t.test("x = ''?.length", "x = 0");
    t.test("x = [1, 2, 3]?.length", "x = 3");
    t.test("x = []?.length", "x = 0");
    t.test("x = ([10, 20])?.[0]", "x = 10");
    t.test("x = ([10, 20])?.[1]", "x = 20");
    t.test("x = 'abcdef'?.[1]", "x = 'b'");
    t.test("x = ({a: 1})?.a", "x = 1");
    t.test("x = ({a: 1})?.['a']", "x = 1");
    // Guard cases / unknown receivers must remain untouched
    t.test_same("x = str?.length");
    t.test_same("x = arr?.length");
    t.test_same("x = obj?.a");
    t.test_same("x = obj?.[key]");
    t.test_same("x = fn?.(arg)");
    t.test_same("x = [foo()]?.length");
    // side-effecting array
}

// port: PeepholeFoldConstantsTest#testFoldOptChain_nullishReceiver
#[test]
fn test_fold_opt_chain_nullish_receiver() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Current compiler behavior in PeepholeFoldConstants (unfolded / preserved)
    t.test_same("x = null?.y");
    t.test_same("x = undefined?.y");
    t.test_same("x = (void 0)?.y");
    t.test_same("x = null?.[0]");
    t.test_same("x = undefined?.[foo()]");
    t.test_same("x = null?.(foo())");
    t.test_same("x = (foo(), null)?.y");
}

// port: PeepholeFoldConstantsTest#testFoldNullishCoalesce_advanced
#[test]
fn test_fold_nullish_coalesce_advanced() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Basic nullish LHS folding
    t.test("null ?? 1", "1");
    t.test("undefined ?? false", "false");
    t.test("(void 0) ?? 'default'", "'default'");
    t.test("(a(), null) ?? 1", "(a(), null, 1)");
    // Non-nullish LHS folding
    t.test("x = false ?? x", "x = false");
    t.test("x = true ?? x", "x = true");
    t.test("x = 0 ?? x", "x = 0");
    t.test("x = '' ?? x", "x = ''");
    t.test("x = 'hello' ?? x", "x = 'hello'");
    t.test("x = 3 ?? x", "x = 3");
    t.test("x = [1, 2] ?? x", "x = [1, 2]");
    t.test("x = ({a: 1}) ?? x", "x = ({a: 1})");
    t.test("x = [foo()] ?? x", "x = [foo()]");
    // Guard cases: RHS cannot be folded if LHS is unknown
    t.test_same("a = x ?? true");
    t.test_same("a = x ?? false");
    t.test_same("a = x ?? 3");
    t.test_same("a = x ?? null");
    t.test_same("a = (foo() ? null : 1) ?? 2");
}

// port: PeepholeFoldConstantsTest#testFoldLogicalAssignments
#[test]
fn test_fold_logical_assignments() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Under normalization, logical assignments normalize to short-circuiting conditional assigns
    t.test_same("x || (x = y)");
    t.test_same("x && (x = y)");
    t.test_same("x ?? (x = y)");
    t.test_same("x || (x = false)");
    t.test_same("x && (x = true)");
    t.test_same("x ?? (x = null)");
    t.test_same("obj[foo()] || (obj[foo()] = y)");
}

// port: PeepholeFoldConstantsTest#testBatchC_templateLiteralsAndSpread
#[test]
fn test_batch_c_template_literals_and_spread() {
    let mut t = PeepholeFoldConstantsTest::new();
    // OPP-012: Template literal substitutions and guards
    t.test("`a${'b'}c`", "`abc`");
    t.test("`a${123}c`", "`a123c`");
    t.test("`a${true}c`", "`atruec`");
    t.test("`a${false}c`", "`afalsec`");
    t.test("`${''}a${''}`", "`a`");
    t.test_same("tag`a${'b'}c`");
    t.test_same("`a${foo()}c`");
    t.test_same("`a${x}c`");
    t.test_same("`$${''}{`");
    // OPP-013: Object spread constant flattening and guards
    t.test("x = {...{}}", "x = {}");
    t.test("x = {a, ...{}, b}", "x = {a, b}");
    t.test("x = {...{a, b}, c, ...{d, e}}", "x = {a, b, c, d, e}");
    t.test("x = {...{...{a}, b}, c}", "x = {a, b, c}");
    t.test_same("x = {...obj}");
    t.test_same("x = {...foo()}");
    t.test_same("x = {...{get a() { return 1; }}}");
    t.test_same("x = {...{set a(v) { }}}");
    // OPP-014: Array spread constant flattening and guards
    t.test("x = [...[]]", "x = []");
    t.test("x = [0, ...[], 1]", "x = [0, 1]");
    t.test("x = [...[0, 1], 2, ...[3, 4]]", "x = [0, 1, 2, 3, 4]");
    t.test("x = [...[...[0], 1], 2]", "x = [0, 1, 2]");
    t.test("foo(...[0], 1)", "foo(0, 1)");
    t.test("foo([...[...[0], 1], 2])", "foo([0, 1, 2])");
    t.test_same("x = [...iter]");
    t.test_same("foo(...iter)");
    t.test_same("x = [...{}]");
    t.test_same("x = {...[]}");
}

// port: PeepholeFoldConstantsTest#testBatchE_arithmeticNeutralElements
#[test]
fn test_batch_e_arithmetic_neutral_elements() {
    let mut t = PeepholeFoldConstantsTest::new();
    // OPP-020: Arithmetic Neutral Elements & Strength Reductions
    // Literal double negation
    t.test("x = -(-4)", "x = 4");
    t.test("x = -(-4n)", "x = 4n");
    // Guard cases: expressions that must not fold or are preserved
    t.test_same("x = foo() ** 0");
    // side-effectful base
    t.test_same("x = bigIntVal ** 1");
    // cannot mix BigInt with number 1
    t.test_same("x = (a + b) ** 1");
    // non-constant exponentiation
    t.test_same("x = (a + b) ** 0");
    t.test_same("x = -(-(a + b))");
    t.test_same("x = a % 1");
}

// port: PeepholeFoldConstantsTest#testBatchE_bitwiseNeutralAndAbsorptionIdentities
#[test]
fn test_batch_e_bitwise_neutral_and_absorption_identities() {
    let mut t = PeepholeFoldConstantsTest::new();
    // OPP-021: Bitwise Neutral, Idempotent & Absorption Identities
    // Fold constant bitwise operations
    t.test("x = 1.5 | 0", "x = 1");
    t.test("x = 4294967295 | 0", "x = -1");
    t.test("x = -1 & 0", "x = 0");
    t.test("x = 0 & -1", "x = 0");
    t.test("x = ~0", "x = -1");
    t.test("x = ~-7", "x = 6");
    // Guard cases: non-constant / side-effectful expressions
    t.test_same("x = foo() ^ foo()");
    // side effects cannot be dropped
    t.test_same("x = foo() & 0");
    // side effects cannot be dropped
    t.test_same("x = (a | b) | 0");
    t.test_same("x = (a & b) & -1");
    t.test_same("x = (a ^ b) ^ 0");
    t.test_same("x = (a >> b) >> 0");
    t.test_same("x = (a >>> b) >>> 0");
    t.test_same("x = ~~ (a | b)");
    t.test_same("x = (a + b) ^ (a + b)");
    t.test_same("x = (a + b) & 0");
    t.test_same("x = y | 0");
}

// port: PeepholeFoldConstantsTest#testFoldBitwiseOp
#[test]
fn test_fold_bitwise_op() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 1 & 1", "x = 1");
    t.test("x = 1 & 2", "x = 0");
    t.test("x = 3 & 1", "x = 1");
    t.test("x = 3 & 3", "x = 3");
    t.test("x = 1 | 1", "x = 1");
    t.test("x = 1 | 2", "x = 3");
    t.test("x = 3 | 1", "x = 3");
    t.test("x = 3 | 3", "x = 3");
    t.test("x = 1 ^ 1", "x = 0");
    t.test("x = 1 ^ 2", "x = 3");
    t.test("x = 3 ^ 1", "x = 2");
    t.test("x = 3 ^ 3", "x = 0");
    t.test("x = -1 & 0", "x = 0");
    t.test("x = 0 & -1", "x = 0");
    t.test("x = 1 & 4", "x = 0");
    t.test("x = 2 & 3", "x = 2");
    // make sure we fold only when we are supposed to -- not when doing so would
    // lose information or when it is performed on nonsensical arguments.
    t.test("x = 1 & 1.1", "x = 1");
    t.test("x = 1.1 & 1", "x = 1");
    t.test("x = 1 & 3000000000", "x = 0");
    t.test("x = 3000000000 & 1", "x = 0");
    // Try some cases with | as well
    t.test("x = 1 | 4", "x = 5");
    t.test("x = 1 | 3", "x = 3");
    t.test("x = 1 | 1.1", "x = 1");
    t.test_same("x = 1 | 3E9");
    // these cases look strange because bitwise OR converts unsigned numbers to be signed
    t.test("x = 1 | 3000000001", "x = -1294967295");
    t.test("x = 4294967295 | 0", "x = -1");
}

// port: PeepholeFoldConstantsTest#testFoldBitwiseOp2
#[test]
fn test_fold_bitwise_op2() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = y & 1 & 1", "x = y & 1");
    t.test("x = y & 1 & 2", "x = y & 0");
    t.test("x = y & 3 & 1", "x = y & 1");
    t.test("x = 3 & y & 1", "x = y & 1");
    t.test("x = y & 3 & 3", "x = y & 3");
    t.test("x = 3 & y & 3", "x = y & 3");
    t.test("x = y | 1 | 1", "x = y | 1");
    t.test("x = y | 1 | 2", "x = y | 3");
    t.test("x = y | 3 | 1", "x = y | 3");
    t.test("x = 3 | y | 1", "x = y | 3");
    t.test("x = y | 3 | 3", "x = y | 3");
    t.test("x = 3 | y | 3", "x = y | 3");
    t.test("x = y ^ 1 ^ 1", "x = y ^ 0");
    t.test("x = y ^ 1 ^ 2", "x = y ^ 3");
    t.test("x = y ^ 3 ^ 1", "x = y ^ 2");
    t.test("x = 3 ^ y ^ 1", "x = y ^ 2");
    t.test("x = y ^ 3 ^ 3", "x = y ^ 0");
    t.test("x = 3 ^ y ^ 3", "x = y ^ 0");
    t.test("x = Infinity | NaN", "x=0");
    t.test("x = 12 | NaN", "x=12");
}

// port: PeepholeFoldConstantsTest#testFoldBitwiseOpWithBigInt
#[test]
fn test_fold_bitwise_op_with_big_int() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 1n & 1n", "x = 1n");
    t.test("x = 1n & 2n", "x = 0n");
    t.test("x = 3n & 1n", "x = 1n");
    t.test("x = 3n & 3n", "x = 3n");
    t.test("x = 1n | 1n", "x = 1n");
    t.test("x = 1n | 2n", "x = 3n");
    t.test("x = 1n | 3n", "x = 3n");
    t.test("x = 3n | 1n", "x = 3n");
    t.test("x = 3n | 3n", "x = 3n");
    t.test("x = 1n | 4n", "x = 5n");
    t.test("x = 1n ^ 1n", "x = 0n");
    t.test("x = 1n ^ 2n", "x = 3n");
    t.test("x = 3n ^ 1n", "x = 2n");
    t.test("x = 3n ^ 3n", "x = 0n");
    t.test("x = -1n & 0n", "x = 0n");
    t.test("x = 0n & -1n", "x = 0n");
    t.test("x = 1n & 4n", "x = 0n");
    t.test("x = 2n & 3n", "x = 2n");
    t.test("x = 1n & 3000000000n", "x = 0n");
    t.test("x = 3000000000n & 1n", "x = 0n");
    // bitwise OR does not affect the sign of a bigint
    t.test("x = 1n | 3000000001n", "x = 3000000001n");
    t.test("x = 4294967295n | 0n", "x = 4294967295n");
    t.test("x = y & 1n & 1n", "x = y & 1n");
    t.test("x = y & 1n & 2n", "x = y & 0n");
    t.test("x = y & 3n & 1n", "x = y & 1n");
    t.test("x = 3n & y & 1n", "x = y & 1n");
    t.test("x = y & 3n & 3n", "x = y & 3n");
    t.test("x = 3n & y & 3n", "x = y & 3n");
    t.test("x = y | 1n | 1n", "x = y | 1n");
    t.test("x = y | 1n | 2n", "x = y | 3n");
    t.test("x = y | 3n | 1n", "x = y | 3n");
    t.test("x = 3n | y | 1n", "x = y | 3n");
    t.test("x = y | 3n | 3n", "x = y | 3n");
    t.test("x = 3n | y | 3n", "x = y | 3n");
    t.test("x = y ^ 1n ^ 1n", "x = y ^ 0n");
    t.test("x = y ^ 1n ^ 2n", "x = y ^ 3n");
    t.test("x = y ^ 3n ^ 1n", "x = y ^ 2n");
    t.test("x = 3n ^ y ^ 1n", "x = y ^ 2n");
    t.test("x = y ^ 3n ^ 3n", "x = y ^ 0n");
    t.test("x = 3n ^ y ^ 3n", "x = y ^ 0n");
}

// port: PeepholeFoldConstantsTest#testFoldingMixTypesLate
#[test]
fn test_folding_mix_types_late() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    t.test("x = x + '2'", "x+='2'");
    t.test("x = +x + +'2'", "x = +x + 2");
    t.test("x = x - '2'", "x-=2");
    t.test("x = x ^ '2'", "x^=2");
    t.test("x = '2' ^ x", "x^=2");
    t.test("x = '2' & x", "x&=2");
    t.test("x = '2' | x", "x|=2");
    t.test("x = '2' | y", "x=2|y");
    t.test("x = y | '2'", "x=y|2");
    t.test("x = y | (a && '2')", "x=y|(a&&2)");
    t.test("x = y | (a,'2')", "x=y|(a,2)");
    t.test("x = y | (a?'1':'2')", "x=y|(a?1:2)");
    t.test("x = y | ('x'?'1':'2')", "x=y|('x'?1:2)");
}

// port: PeepholeFoldConstantsTest#testFoldingMixTypesEarly
#[test]
fn test_folding_mix_types_early() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = false;
    t.test_same("x = x + '2'");
    t.test("x = +x + +'2'", "x = +x + 2");
    t.test("x = x - '2'", "x = x - 2");
    t.test("x = x ^ '2'", "x = x ^ 2");
    t.test("x = '2' ^ x", "x = 2 ^ x");
    t.test("x = '2' & x", "x = 2 & x");
    t.test("x = '2' | x", "x = 2 | x");
    t.test("x = '2' | y", "x=2|y");
    t.test("x = y | '2'", "x=y|2");
    t.test("x = y | (a && '2')", "x=y|(a&&2)");
    t.test("x = y | (a,'2')", "x=y|(a,2)");
    t.test("x = y | (a?'1':'2')", "x=y|(a?1:2)");
    t.test("x = y | ('x'?'1':'2')", "x=y|('x'?1:2)");
}

// port: PeepholeFoldConstantsTest#testFoldingAdd1
#[test]
fn test_folding_add1() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = null + true", "x=1");
    t.test_same("x = a + true");
    t.test("x = '' + {}", "x = '[object Object]'");
    t.test("x = [] + {}", "x = '[object Object]'");
    t.test("x = {} + []", "x = '[object Object]'");
    t.test("x = {} + ''", "x = '[object Object]'");
}

// port: PeepholeFoldConstantsTest#testFoldingAdd2
#[test]
fn test_folding_add2() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = false + []", "x='false'");
    t.test("x = [] + true", "x='true'");
    t.test("NaN + []", "'NaN'");
}

// port: PeepholeFoldConstantsTest#testFoldBitwiseOpStringCompare
#[test]
fn test_fold_bitwise_op_string_compare() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = -1 | 0", "x = -1");
}

// port: PeepholeFoldConstantsTest#testFoldBitShifts
#[test]
fn test_fold_bit_shifts() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Running on just changed code results in an exception on only the first invocation. Don't
    // repeat because it confuses the exception verification.
    t.hooks.num_repetitions = 1;
    t.test("x = 1 << 0", "x = 1");
    t.test("x = -1 << 0", "x = -1");
    t.test("x = 1 << 1", "x = 2");
    t.test("x = 3 << 1", "x = 6");
    t.test("x = 1 << 8", "x = 256");
    t.test("x = 1 >> 0", "x = 1");
    t.test("x = -1 >> 0", "x = -1");
    t.test("x = 1 >> 1", "x = 0");
    t.test("x = 2 >> 1", "x = 1");
    t.test("x = 5 >> 1", "x = 2");
    t.test("x = 127 >> 3", "x = 15");
    t.test("x = 3 >> 1", "x = 1");
    t.test("x = 3 >> 2", "x = 0");
    t.test("x = 10 >> 1", "x = 5");
    t.test("x = 10 >> 2", "x = 2");
    t.test("x = 10 >> 5", "x = 0");
    t.test("x = 10 >>> 1", "x = 5");
    t.test("x = 10 >>> 2", "x = 2");
    t.test("x = 10 >>> 5", "x = 0");
    t.test("x = -1 >>> 1", "x = 2147483647");
    // 0x7fffffff
    t.test("x = -1 >>> 0", "x = 4294967295");
    // 0xffffffff
    t.test("x = -2 >>> 0", "x = 4294967294");
    // 0xfffffffe
    t.test("x = 0x90000000 >>> 28", "x = 9");
    t.test("x = 0xffffffff << 0", "x = -1");
    t.test("x = 0xffffffff << 4", "x = -16");
    t.test_same("1 << 32");
    t.test_same("1 << -1");
    t.test_same("1 >> 32");
    t.test_same_warning("1.5 << 0", &FRACTIONAL_BITWISE_OPERAND);
    t.test_same_warning("1 << .5", &FRACTIONAL_BITWISE_OPERAND);
    t.test_same_warning("1.5 >>> 0", &FRACTIONAL_BITWISE_OPERAND);
    t.test_same_warning("1 >>> .5", &FRACTIONAL_BITWISE_OPERAND);
    t.test_same_warning("1.5 >> 0", &FRACTIONAL_BITWISE_OPERAND);
    t.test_same_warning("1 >> .5", &FRACTIONAL_BITWISE_OPERAND);
}

// port: PeepholeFoldConstantsTest#testFoldBitShiftsStringCompare
#[test]
fn test_fold_bit_shifts_string_compare() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = -1 << 1", "x = -2");
    t.test("x = -1 << 8", "x = -256");
    t.test("x = -1 >> 1", "x = -1");
    t.test("x = -2 >> 1", "x = -1");
    t.test("x = -1 >> 0", "x = -1");
}

// port: PeepholeFoldConstantsTest#testStringAdd
#[test]
fn test_string_add() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 'a' + 'bc'", "x = 'abc'");
    t.test("x = 'a' + 5", "x = 'a5'");
    t.test("x = 5 + 'a'", "x = '5a'");
    t.test("x = 'a' + 5n", "x = 'a5'");
    t.test("x = 5n + 'a'", "x = '5a'");
    t.test("x = 'a' + ''", "x = 'a'");
    t.test("x = 'a' + foo()", "x = 'a'+foo()");
    t.test("x = foo() + 'a' + 'b'", "x = foo()+'ab'");
    t.test("x = (foo() + 'a') + 'b'", "x = foo()+'ab'");
    // believe it!
    t.test(
        "x = foo() + 'a' + 'b' + 'cd' + bar()",
        "x = foo()+'abcd'+bar()",
    );
    t.test("x = foo() + 2 + 'b'", "x = foo()+2+\"b\"");
    // don't fold!
    t.test("x = foo() + 'a' + 2", "x = foo()+\"a2\"");
    t.test("x = '' + null", "x = 'null'");
    t.test("x = true + '' + false", "x = 'truefalse'");
    t.test("x = '' + []", "x = ''");
    t.test("x = foo() + 'a' + 1 + 1", "x = foo() + 'a11'");
    t.test("x = 1 + 1 + 'a'", "x = '2a'");
    t.test("x = 1 + 1 + 'a'", "x = '2a'");
    t.test("x = 'a' + (1 + 1)", "x = 'a2'");
    t.test("x = '_' + p1 + '_' + ('' + p2)", "x = '_' + p1 + '_' + p2");
    t.test("x = 'a' + ('_' + 1 + 1)", "x = 'a_11'");
    t.test("x = 'a' + ('_' + 1) + 1", "x = 'a_11'");
    t.test("x = 1 + (p1 + '_') + ('' + p2)", "x = 1 + (p1 + '_') + p2");
    t.test("x = 1 + p1 + '_' + ('' + p2)", "x = 1 + p1 + '_' + p2");
    t.test("x = 1 + 'a' + p1", "x = '1a' + p1");
    t.test("x = (p1 + (p2 + 'a')) + 'b'", "x = (p1 + (p2 + 'ab'))");
    t.test("'a' + ('b' + p1) + 1", "'ab' + p1 + 1");
    t.test("x = 'a' + ('b' + p1 + 'c')", "x = 'ab' + (p1 + 'c')");
    t.test_same("x = 'a' + (4 + p1 + 'a')");
    t.test_same("x = p1 / 3 + 4");
    t.test_same("foo() + 3 + 'a' + foo()");
    t.test_same("x = 'a' + ('b' + p1 + p2)");
    t.test_same("x = 1 + ('a' + p1)");
    t.test_same("x = p1 + '' + p2");
    t.test_same("x = 'a' + (1 + p1)");
    t.test_same("x = (p2 + 'a') + (1 + p1)");
    t.test_same("x = (p2 + 'a') + (1 + p1 + p2)");
    t.test_same("x = (p2 + 'a') + (1 + (p1 + p2))");
}

// port: PeepholeFoldConstantsTest#testStringAdd_identity
#[test]
fn test_string_add_identity() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.harness.enable_type_check().unwrap();
    t.harness.replace_types_with_colors().unwrap();
    t.harness.disable_compare_js_doc().unwrap();
    t.fold_string_types("x + ''", "x");
    t.fold_string_types("'' + x", "x");
}

// port: PeepholeFoldConstantsTest#testIssue821
#[test]
fn test_issue821() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("var a =(Math.random()>0.5? '1' : 2 ) + 3 + 4;");
    t.test_same("var a = ((Math.random() ? 0 : 1) || (Math.random()>0.5? '1' : 2 )) + 3 + 4;");
}

// port: PeepholeFoldConstantsTest#testFoldConstructor
#[test]
fn test_fold_constructor() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = this[new String('a')]", "x = this['a']");
    t.test("x = ob[new String(12)]", "x = ob['12']");
    t.test("x = ob[new String(false)]", "x = ob['false']");
    t.test("x = ob[new String(null)]", "x = ob['null']");
    t.test("x = 'a' + new String('b')", "x = 'ab'");
    t.test("x = 'a' + new String(23)", "x = 'a23'");
    t.test("x = 2 + new String(1)", "x = '21'");
    t.test_same("x = ob[new String(a)]");
    t.test_same("x = new String('a')");
    t.test_same("x = (new String('a'))[3]");
}

// port: PeepholeFoldConstantsTest#testFoldArithmetic
#[test]
fn test_fold_arithmetic() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 10 + 20", "x = 30");
    t.test("x = 2 / 4", "x = 0.5");
    t.test("x = 2.25 & 3", "x = 2");
    t.test_same("z = x & y");
    t.test_same("x = y & 5");
    t.test_same("x = 1 / 0");
    t.test("x = 3 % 2", "x = 1");
    t.test("x = 3 % -2", "x = 1");
    t.test("x = -1 % 3", "x = -1");
    t.test_same("x = 1 % 0");
    t.test("x = 2 ** 3", "x = 8");
    t.test("x = 2 ** -3", "x = 0.125");
    t.test_same("x = 2 ** 55");
    // backs off folding because 2 ** 55 is too large
    t.test_same("x = 3 ** -1");
    // backs off because 3**-1 is shorter than 0.3333333333333333
}

// port: PeepholeFoldConstantsTest#testFoldArithmetic2
#[test]
fn test_fold_arithmetic2() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("x = y + 10 + 20");
    t.test_same("x = y / 2 / 4");
    t.test("x = y & 2.25 & 3", "x = y & 2");
    t.test_same("x = y & 2.25 & z & 3");
    t.test_same("z = x &y");
    t.test_same("x = y & 5");
    t.test("x = y + (z & 24 & 60 & 60 & 1000)", "x = y + (z & 8)");
}

// port: PeepholeFoldConstantsTest#testFoldArithmetic3
#[test]
fn test_fold_arithmetic3() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = null | undefined", "x = 0");
    t.test("x = null | 1", "x = 1");
    t.test("x = (null - 1)| 2", "x = -1");
    t.test("x = (null + 1) | 2", "x = 3");
    t.test("x = null ** 0", "x = 1");
    t.test("x = (-0) ** 3", "x = -0");
}

// port: PeepholeFoldConstantsTest#testFoldArithmeticInfinity
#[test]
fn test_fold_arithmetic_infinity() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x=-Infinity-2", "x=-Infinity");
    t.test("x=Infinity-2", "x=Infinity");
    t.test("x=Infinity*5", "x=Infinity");
    t.test("x = Infinity ** 2", "x = Infinity");
    t.test("x = Infinity ** -2", "x = 0");
}

// port: PeepholeFoldConstantsTest#testFoldArithmeticStringComp
#[test]
fn test_fold_arithmetic_string_comp() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 10 - 20", "x = -10");
}

// port: PeepholeFoldConstantsTest#testNoFoldArithmeticWithSideEffects
#[test]
fn test_no_fold_arithmetic_with_side_effects() {
    let mut t = PeepholeFoldConstantsTest::new();
    // can't fold this to "x = y & 6.75" because you can't remove the "sideEffects()" call
    t.test_same("x = y & 2.25 & (sideEffects(), 3)");
}

// port: PeepholeFoldConstantsTest#testFoldBigIntArithmetic
#[test]
fn test_fold_big_int_arithmetic() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 1n + 2n", "x = 3n");
    t.test("x = 1n - 2n", "x = -1n");
    t.test("x = 2n * 3n", "x = 6n");
    t.test("x = 6n / 2n", "x = 3n");
    t.test("x = 3n % 2n", "x = 1n");
    t.test("x = 2n ** 3n", "x = 8n");
    // The compiler is not designed to fold expressions with an exponent > 2147483647
    t.test("x = 1n ** 2147483647n", "x = 1n");
    t.test_same("x = 1n ** 2147483648n");
    t.test("x = y & 2n & 3n", "x = y & 2n");
    // TODO b/361826515: Optimize associative bigint operations
    t.test_same("x = y * 2n * z * 3n");
    t.test_same("x = y + 2n + z + 3n");
}

// port: PeepholeFoldConstantsTest#testNoFoldBigIntArithmeticWithSideEffects
#[test]
fn test_no_fold_big_int_arithmetic_with_side_effects() {
    let mut t = PeepholeFoldConstantsTest::new();
    // can't fold this to "x = y * 6.75" because you can't remove the "sideEffects()" call
    t.test_same("x = y * 2n * (sideEffects(), 3n)");
}

// port: PeepholeFoldConstantsTest#testFoldComparison
#[test]
fn test_fold_comparison() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 0 == 0", "x = true");
    t.test("x = 1 == 2", "x = false");
    t.test("x = 0n == 0n", "x = true");
    t.test("x = 1n == 2n", "x = false");
    t.test("x = 'abc' == 'def'", "x = false");
    t.test("x = 'abc' == 'abc'", "x = true");
    t.test("x = \"\" == ''", "x = true");
    t.test("x = foo() == bar()", "x = foo()==bar()");
    t.test("x = 1 != 0", "x = true");
    t.test("x = 1 != 1", "x = false");
    t.test("x = 1n != 0n", "x = true");
    t.test("x = 1n != 1n", "x = false");
    t.test("x = 'abc' != 'def'", "x = true");
    t.test("x = 'a' != 'a'", "x = false");
    t.test("x = 1 < 20", "x = true");
    t.test("x = 3 < 3", "x = false");
    t.test("x = 10 > 1.0", "x = true");
    t.test("x = 10 > 10.25", "x = false");
    t.test("x = 1 <= 1", "x = true");
    t.test("x = 1 <= 0", "x = false");
    t.test("x = 0 >= 0", "x = true");
    t.test("x = -1 >= 9", "x = false");
    t.test("x = 1n < 20n", "x = true");
    t.test("x = 3n < 3n", "x = false");
    t.test("x = 10n > 1n", "x = true");
    t.test("x = 10n > 10n", "x = false");
    t.test("x = 1n <= 1n", "x = true");
    t.test("x = 1n <= 0n", "x = false");
    t.test("x = 0n >= 0n", "x = true");
    t.test("x = -1n >= 9n", "x = false");
    t.test_same("x = y == y");
    t.test("x = y < y", "x = false");
    t.test("x = y > y", "x = false");
    t.test("x = true == true", "x = true");
    t.test("x = false == false", "x = true");
    t.test("x = false == null", "x = false");
    t.test("x = false == true", "x = false");
    t.test("x = true == null", "x = false");
    t.test("0 == 0", "true");
    t.test("1 == 2", "false");
    t.test("0n == 0n", "true");
    t.test("1n == 2n", "false");
    t.test("'abc' == 'def'", "false");
    t.test("'abc' == 'abc'", "true");
    t.test("\"\" == ''", "true");
    t.test_same("foo() == bar()");
    t.test("1 != 0", "true");
    t.test("1 != 1", "false");
    t.test("1n != 0n", "true");
    t.test("1n != 1n", "false");
    t.test("'abc' != 'def'", "true");
    t.test("'a' != 'a'", "false");
    t.test("1 < 20", "true");
    t.test("3 < 3", "false");
    t.test("10 > 1.0", "true");
    t.test("10 > 10.25", "false");
    t.test_same("x == x");
    t.test("x < x", "false");
    t.test("x > x", "false");
    t.test("1 <= 1", "true");
    t.test("1 <= 0", "false");
    t.test("0 >= 0", "true");
    t.test("-1 >= 9", "false");
    t.test("1n < 20n", "true");
    t.test("3n < 3n", "false");
    t.test("10n > 1n", "true");
    t.test("10n > 10n", "false");
    t.test("1n <= 1n", "true");
    t.test("1n <= 0n", "false");
    t.test("0n >= 0n", "true");
    t.test("-1n >= 9n", "false");
    t.test("true == true", "true");
    t.test("false == null", "false");
    t.test("false == true", "false");
    t.test("true == null", "false");
}

// port: PeepholeFoldConstantsTest#testFoldComparison2
#[test]
fn test_fold_comparison2() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = 0 === 0", "x = true");
    t.test("x = 1 === 2", "x = false");
    t.test("x = 0n === 0n", "x = true");
    t.test("x = 1n === 2n", "x = false");
    t.test("x = 'abc' === 'def'", "x = false");
    t.test("x = 'abc' === 'abc'", "x = true");
    t.test("x = \"\" === ''", "x = true");
    t.test("x = foo() === bar()", "x = foo()===bar()");
    t.test("x = 1 !== 0", "x = true");
    t.test("x = 1 !== 1", "x = false");
    t.test("x = 1n !== 0n", "x = true");
    t.test("x = 1n !== 1n", "x = false");
    t.test("x = 'abc' !== 'def'", "x = true");
    t.test("x = 'a' !== 'a'", "x = false");
    t.test("x = y === y", "x = y===y");
    t.test("x = true === true", "x = true");
    t.test("x = false === false", "x = true");
    t.test("x = false === null", "x = false");
    t.test("x = false === true", "x = false");
    t.test("x = true === null", "x = false");
    t.test("0 === 0", "true");
    t.test("1 === 2", "false");
    t.test("0n === 0n", "true");
    t.test("1n === 2n", "false");
    t.test("'abc' === 'def'", "false");
    t.test("'abc' === 'abc'", "true");
    t.test("\"\" === ''", "true");
    t.test_same("foo() === bar()");
    t.test("1 === '1'", "false");
    t.test("1 === true", "false");
    t.test("1 !== '1'", "true");
    t.test("1 !== true", "true");
    t.test("1 !== 0", "true");
    t.test("'abc' !== 'def'", "true");
    t.test("'a' !== 'a'", "false");
    t.test_same("x === x");
    t.test("true === true", "true");
    t.test("false === null", "false");
    t.test("false === true", "false");
    t.test("true === null", "false");
}

// port: PeepholeFoldConstantsTest#testFoldComparison3
#[test]
fn test_fold_comparison3() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = !1 == !0", "x = false");
    t.test("x = !0 == !0", "x = true");
    t.test("x = !1 == !1", "x = true");
    t.test("x = !1 == null", "x = false");
    t.test("x = !1 == !0", "x = false");
    t.test("x = !0 == null", "x = false");
    t.test("!0 == !0", "true");
    t.test("!1 == null", "false");
    t.test("!1 == !0", "false");
    t.test("!0 == null", "false");
    t.test("x = !0 === !0", "x = true");
    t.test("x = !1 === !1", "x = true");
    t.test("x = !1 === null", "x = false");
    t.test("x = !1 === !0", "x = false");
    t.test("x = !0 === null", "x = false");
    t.test("!0 === !0", "true");
    t.test("!1 === null", "false");
    t.test("!1 === !0", "false");
    t.test("!0 === null", "false");
}

// port: PeepholeFoldConstantsTest#testFoldComparison4
#[test]
fn test_fold_comparison4() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("[] == false");
    // true
    t.test_same("[] == true");
    // false
    t.test_same("[0] == false");
    // true
    t.test_same("[0] == true");
    // false
    t.test_same("[1] == false");
    // false
    t.test_same("[1] == true");
    // true
    t.test_same("({}) == false");
    // false
    t.test_same("({}) == true");
    // true
}

// port: PeepholeFoldConstantsTest#testFoldGetElem1
#[test]
fn test_fold_get_elem1() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Running on just changed code results in an exception on only the first invocation. Don't
    // repeat because it confuses the exception verification.
    t.hooks.num_repetitions = 1;
    t.test("x = [,10][0]", "x = void 0");
    t.test("x = [10, 20][0]", "x = 10");
    t.test("x = [10, 20][1]", "x = 20");
    t.test_same_warning("x = [10, 20][0.5]", &INVALID_GETELEM_INDEX_ERROR);
    t.test("x = [10, 20][-1]", "x = void 0;");
    t.test("x = [10, 20][2]", "x = void 0;");
    // TODO(b/538156673): Fix PeepholeFoldConstants to not fold out-of-bounds GETELEM on arrays with
    // side-effect elements
    t.test_same("x = [foo(), 0][1]");
    t.test("x = [0, foo()][1]", "x = foo()");
    t.test_same("x = [0, foo()][0]");
    t.test_same("x = [foo(), 0][-1]");
    t.test_same("x = [0, foo()][-1]");
    t.test_same("x = [foo(), 0][2]");
    t.test_same("x = [0, foo()][2]");
    t.test_same("for([1][0] in {});");
}

// port: PeepholeFoldConstantsTest#testFoldOptChainGetElem1
#[test]
fn test_fold_opt_chain_get_elem1() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test("x = [,10]?.[0]", "x = void 0");
    t.test("x = [10, 20]?.[0]", "x = 10");
    t.test("x = [10, 20]?.[1]", "x = 20");
    t.test_same_warning("x = [10, 20]?.[0.5]", &INVALID_GETELEM_INDEX_ERROR);
    t.test("x = [10, 20]?.[-1]", "x = void 0;");
    t.test("x = [10, 20]?.[2]", "x = void 0;");
    t.test_same("x = [foo(), 0]?.[1]");
    t.test("x = [0, foo()]?.[1]", "x = foo()");
    t.test_same("x = [0, foo()]?.[0]");
    t.test_same("x = [foo(), 0]?.[-1]");
    t.test_same("x = [0, foo()]?.[-1]");
    t.test_same("x = [foo(), 0]?.[2]");
    t.test_same("x = [0, foo()]?.[2]");
}

// port: PeepholeFoldConstantsTest#testFoldGetElem2
#[test]
fn test_fold_get_elem2() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Running on just changed code results in an exception on only the first invocation. Don't
    // repeat because it confuses the exception verification.
    t.hooks.num_repetitions = 1;
    t.test("x = 'string'[5]", "x = 'g'");
    t.test("x = 'string'[0]", "x = 's'");
    t.test("x = 's'[0]", "x = 's'");
    t.test_same("x = '\u{1f4a9}'[0]");
    t.test_same_warning("x = 'string'[0.5]", &INVALID_GETELEM_INDEX_ERROR);
    t.test("x = 'string'[-1]", "x = void 0;");
    t.test("x = 'string'[6]", "x = void 0;");
}

/// Optional versions of the above `testFoldGetElem2` tests
// port: PeepholeFoldConstantsTest#testFoldOptChainGetElem2
#[test]
fn test_fold_opt_chain_get_elem2() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Running on just changed code results in an exception on only the first invocation. Don't
    // repeat because it confuses the exception verification.
    t.hooks.num_repetitions = 1;
    t.test("x = 'string'?.[5]", "x = 'g'");
    t.test("x = 'string'?.[0]", "x = 's'");
    t.test("x = 's'?.[0]", "x = 's'");
    t.test_same("x = '\u{1f4a9}'?.[0]");
    t.test_same_warning("x = 'string'?.[0.5]", &INVALID_GETELEM_INDEX_ERROR);
    t.test("x = 'string'?.[-1]", "x = void 0;");
    t.test("x = 'string'?.[6]", "x = void 0;");
}

// port: PeepholeFoldConstantsTest#testFoldArrayLitSpreadGetElem
#[test]
fn test_fold_array_lit_spread_get_elem() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test("x = [...[0]][0]", "x = 0;");
    t.test("x = [0, 1, ...[2, 3, 4]][3]", "x = 3;");
    t.test("x = [...[0, 1], 2, ...[3, 4]][3]", "x = 3;");
    t.test("x = [...[...[0, 1], 2, 3], 4][0]", "x = 0");
    t.test("x = [...[...[0, 1], 2, 3], 4][3]", "x = 3");
    t.test_parts(vec![
        TestPart::Sources(CompilerTestCase::srcs("x = [...[]][100]")),
        TestPart::Expected(CompilerTestCase::expected("x = void 0;")),
    ]);
    t.test_parts(vec![
        TestPart::Sources(CompilerTestCase::srcs("x = [...[0]][100]")),
        TestPart::Expected(CompilerTestCase::expected("x = void 0;")),
    ]);
}

/// Optional versions of the above `testFoldArrayLitSpreadGetElem` tests
// port: PeepholeFoldConstantsTest#testFoldArrayLitSpreadOptChainGetElem
#[test]
fn test_fold_array_lit_spread_opt_chain_get_elem() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test("x = [...[0]]?.[0]", "x = 0;");
    t.test("x = [0, 1, ...[2, 3, 4]]?.[3]", "x = 3;");
    t.test("x = [...[0, 1], 2, ...[3, 4]]?.[3]", "x = 3;");
    t.test("x = [...[...[0, 1], 2, 3], 4]?.[0]", "x = 0");
    t.test("x = [...[...[0, 1], 2, 3], 4]?.[3]", "x = 3");
    t.test_parts(vec![
        TestPart::Sources(CompilerTestCase::srcs("x = [...[]]?.[100]")),
        TestPart::Expected(CompilerTestCase::expected("x = void 0;")),
    ]);
    t.test_parts(vec![
        TestPart::Sources(CompilerTestCase::srcs("x = [...[0]]?.[100]")),
        TestPart::Expected(CompilerTestCase::expected("x = void 0;")),
    ]);
}

// port: PeepholeFoldConstantsTest#testDontFoldNonLiteralSpreadGetElem
#[test]
fn test_dont_fold_non_literal_spread_get_elem() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("x = [...iter][0];");
    t.test_same("x = [0, 1, ...iter][2];");
    //  `...iter` could have side effects, so don't replace `x` with `0`
    t.test_same("x = [0, 1, ...iter][0];");
}

// port: PeepholeFoldConstantsTest#testFoldArraySpread
#[test]
fn test_fold_array_spread() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test("x = [...[]]", "x = []");
    t.test("x = [0, ...[], 1]", "x = [0, 1]");
    t.test("x = [...[0, 1], 2, ...[3, 4]]", "x = [0, 1, 2, 3, 4]");
    t.test("x = [...[...[0], 1], 2]", "x = [0, 1, 2]");
    t.test_same("[...[x]] = arr");
    t.test("foo([...[...[0], 1], 2])", "foo([0, 1, 2])");
    t.test_same("x = [1, ...[,], 2];");
    t.test_same("x = [0, ...[,,], 3];");
    t.test_same("x = [...[,]];");
}

// port: PeepholeFoldConstantsTest#testFoldArrayLitSpreadInArg
#[test]
fn test_fold_array_lit_spread_in_arg() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("foo(...[0], 1)", "foo(0, 1)");
    t.test_same("foo(...[,]);");
    t.test_same("foo(...[,,,,\"foo\"], 1)");
    t.test_same("foo(...(false ? [0] : [1]))");
    // other opts need to fold the ternery first
}

// port: PeepholeFoldConstantsTest#testFoldObjectLitSpreadGetProp
#[test]
fn test_fold_object_lit_spread_get_prop() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test("x = {...{a}}.a", "x = a;");
    t.test("x = {a, b, ...{c, d, e}}.d", "x = d;");
    t.test("x = {...{a, b}, c, ...{d, e}}.d", "x = d;");
    t.test("x = {...{...{a, b}, c, d}, e}.a", "x = a");
    t.test("x = {...{...{a, b}, c, d}, e}.d", "x = d");
}

// port: PeepholeFoldConstantsTest#testDontFoldNonLiteralObjectSpreadGetProp_gettersImpure
#[test]
fn test_dont_fold_non_literal_object_spread_get_prop_getters_impure() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.assume_getters_pure = false;
    t.test_same("x = {...obj}.a;");
    t.test_same("x = {a, ...obj, c}.a;");
    t.test_same("x = {a, ...obj, c}.c;");
}

// port: PeepholeFoldConstantsTest#testDontFoldNonLiteralObjectSpreadGetProp_assumeGettersPure
#[test]
fn test_dont_fold_non_literal_object_spread_get_prop_assume_getters_pure() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.assume_getters_pure = true;
    t.test_same("x = {...obj}.a;");
    t.test_same("x = {a, ...obj, c}.a;");
    t.test("x = {a, ...obj, c}.c;", "x = c;");
    // We assume object spread has no side-effects.
}

// port: PeepholeFoldConstantsTest#testFoldObjectSpread
#[test]
fn test_fold_object_spread() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test("x = {...{}}", "x = {}");
    t.test("x = {a, ...{}, b}", "x = {a, b}");
    t.test("x = {...{a, b}, c, ...{d, e}}", "x = {a, b, c, d, e}");
    t.test("x = {...{...{a}, b}, c}", "x = {a, b, c}");
    t.test_same("({...{x}} = obj)");
}

// port: PeepholeFoldConstantsTest#testDontFoldObjectSpread_withGetterSetter
#[test]
fn test_dont_fold_object_spread_with_getter_setter() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    // At runtime, object spread converts getters to regular data properties, and omits setters.
    // We back off on folding spread with getters & setters.
    // In theory, we could make the compiler smart enough to turn
    //   `{...{get a() { return 1; }}}` into `{a: 1}`, or `{...{set a(v) { }}}` into `{}`, but
    // it doesn't seem worth the complexity right now.
    t.test_same("x = {...{get a() { return 1; }}}");
    t.test_same("x = {...{set a(v) { }}}");
    // Test case with side-effects. If in the future we implement spread folding for getters/setters
    // we would need to preserve evaluation of the console.log.
    t.test_same("x = {...{get a() { console.log('hi'); return 1; }}}");
}

// port: PeepholeFoldConstantsTest#testDontFoldObjectSpread_withComputedGetterSetter
#[test]
fn test_dont_fold_object_spread_with_computed_getter_setter() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test_same("x = {...{get [a]() { return 1; }}}");
    t.test_same("x = {...{set [a](v) { }}}");
}

// port: PeepholeFoldConstantsTest#testDontFoldMixedObjectAndArraySpread
#[test]
fn test_dont_fold_mixed_object_and_array_spread() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.num_repetitions = 1;
    t.test_same("x = [...{}]");
    t.test_same("x = {...[]}");
    t.test("x = [a, ...[...{}]]", "x = [a, ...{}]");
    t.test("x = {a, ...{...[]}}", "x = {a, ...[]}");
}

// port: PeepholeFoldConstantsTest#testFoldComplex
#[test]
fn test_fold_complex() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = (3 / 1.0) + (1 * 2)", "x = 5");
    t.test("x = (1 == 1.0) && foo() && true", "x = foo()&&true");
    t.test("x = 'abc' + 5 + 10", "x = \"abc510\"");
}

// port: PeepholeFoldConstantsTest#testFoldLeft
#[test]
fn test_fold_left() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("(+x - 1) + 2");
    // not yet
    t.test("(+x & 1) & 2", "+x & 0");
}

// port: PeepholeFoldConstantsTest#testFoldArrayLength
#[test]
fn test_fold_array_length() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Can fold
    t.test("x = [].length", "x = 0");
    t.test("x = [1,2,3].length", "x = 3");
    t.test("x = [a,b].length", "x = 2");
    // Not handled yet
    t.test("x = [,,1].length", "x = 3");
    // Cannot fold
    t.test("x = [foo(), 0].length", "x = [foo(),0].length");
    t.test_same("x = y.length");
    t.test_same("[1, 2].length = 0;");
    t.test("[1, 2].length >>= 1;", "[1, 2].length = 1;");
    t.test_same("[1, 2].length++;");
    t.test_same("++[1, 2].length;");
    t.test_same("[1, 2].length--;");
    t.test_same("--[1, 2].length;");
    t.test_same("x = [...'abc'].length");
    t.test_same("x = [1, ...'ab', 3].length");
    t.test_same("x = [...`abc`].length");
}

// port: PeepholeFoldConstantsTest#testFoldStringLength
#[test]
fn test_fold_string_length() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Can fold basic strings.
    t.test("x = ''.length", "x = 0");
    t.test("x = '123'.length", "x = 3");
    // Test Unicode escapes are accounted for.
    t.test("x = '123\u{1dc}'.length", "x = 4");
    // Cannot fold when length is an lvalue
    t.test_same("\"a\".length = 1;");
    t.test("\"a\".length >>= 1;", "\"a\".length = 0;");
    t.test_same("\"a\".length++;");
    t.test_same("++\"a\".length;");
    t.test_same("\"a\".length--;");
    t.test_same("--\"a\".length;");
}

// port: PeepholeFoldConstantsTest#testFoldTypeof
#[test]
fn test_fold_typeof() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x = typeof 1", "x = \"number\"");
    t.test("x = typeof 'foo'", "x = \"string\"");
    t.test("x = typeof true", "x = \"boolean\"");
    t.test("x = typeof false", "x = \"boolean\"");
    t.test("x = typeof null", "x = \"object\"");
    t.test("x = typeof undefined", "x = \"undefined\"");
    t.test("x = typeof void 0", "x = \"undefined\"");
    t.test("x = typeof []", "x = \"object\"");
    t.test("x = typeof [1]", "x = \"object\"");
    t.test("x = typeof [1,[]]", "x = \"object\"");
    t.test("x = typeof {}", "x = \"object\"");
    t.test("x = typeof function() {}", "x = 'function'");
    t.test_same("x = typeof[1,[foo()]]");
    t.test_same("x = typeof{bathwater:baby()}");
}

// port: PeepholeFoldConstantsTest#testFoldInstanceOf
#[test]
fn test_fold_instance_of() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Non object types are never instances of anything.
    t.test("64 instanceof Object", "false");
    t.test("64 instanceof Number", "false");
    t.test("'' instanceof Object", "false");
    t.test("'' instanceof String", "false");
    t.test("true instanceof Object", "false");
    t.test("true instanceof Boolean", "false");
    t.test("!0 instanceof Object", "false");
    t.test("!0 instanceof Boolean", "false");
    t.test("false instanceof Object", "false");
    t.test("null instanceof Object", "false");
    t.test("undefined instanceof Object", "false");
    t.test("NaN instanceof Object", "false");
    t.test("Infinity instanceof Object", "false");
    // Untagged template literals always evaluate to strings.
    t.test("`` instanceof Object", "false");
    t.test("`${[]}` instanceof Object", "false");
    t.test("`${function(){}}` instanceof Object", "false");
    // Back off when substitutions have side-effects.
    t.test_same("`${(console.log(0), [])}` instanceof Object");
    // Tagged template literals may evaluate to a non-string, though. Would require type information
    // to fold.
    t.test_same("tag`${function(){}}` instanceof Object");
    // Array and object literals are known to be objects.
    t.test("[] instanceof Object", "true");
    t.test("({}) instanceof Object", "true");
    // These cases is foldable, but no handled currently.
    t.test_same("new Foo() instanceof Object");
    // These would require type information to fold.
    t.test_same("[] instanceof Foo");
    t.test_same("({}) instanceof Foo");
    t.test("(function() {}) instanceof Object", "true");
    // An unknown value should never be folded.
    t.test_same("x instanceof Foo");
}

// port: PeepholeFoldConstantsTest#testDivision
#[test]
fn test_division() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Make sure the 1/3 does not expand to 0.333333
    t.test_same("print(1/3)");
    // Decimal form is preferable to fraction form when strings are the
    // same length.
    t.test("print(1/2)", "print(0.5)");
}

// port: PeepholeFoldConstantsTest#testAssignOpsLate
#[test]
fn test_assign_ops_late() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    t.test("x=x+y", "x+=y");
    t.test_same("x=y+x");
    t.test("x=x*y", "x*=y");
    t.test("x=y*x", "x*=y");
    t.test("x.y=x.y+z", "x.y+=z");
    t.test_same("next().x = next().x + 1");
    t.test("x=x-y", "x-=y");
    t.test_same("x=y-x");
    t.test("x=x|y", "x|=y");
    t.test("x=y|x", "x|=y");
    t.test("x=x|y|z", "x|=y|z");
    t.test_same("x=x&&y&&z");
    t.test("x=x*y", "x*=y");
    t.test("x=y*x", "x*=y");
    t.test_same("x=foo()*x");
    t.test_same("x=(x=5)*x");
    t.test_same("x=(y++)*x");
    t.test_same("x=foo()|x");
    t.test_same("x=foo()&x");
    t.test_same("x=foo()^x");
    t.test("x=x*foo()", "x*=foo()");
    t.test("x=x|foo()", "x|=foo()");
    t.test("x=x&foo()", "x&=foo()");
    t.test("x=x^foo()", "x^=foo()");
    t.test("x=x+foo()", "x+=foo()");
    t.test("x=x**y", "x**=y");
    t.test_same("x=y**x");
    t.test("x.y=x.y+z", "x.y+=z");
    t.test_same("next().x = next().x + 1");
    // This is OK, really.
    t.test("({a:1}).a = ({a:1}).a + 1", "({a:1}).a = 2");
}

// port: PeepholeFoldConstantsTest#testAssignOpsEarly
#[test]
fn test_assign_ops_early() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = false;
    t.test_same("x=x+y");
    t.test_same("x=y+x");
    t.test_same("x=x*y");
    t.test_same("x=y*x");
    t.test_same("x.y=x.y+z");
    t.test_same("next().x = next().x + 1");
    t.test_same("x=x-y");
    t.test_same("x=y-x");
    t.test_same("x=x|y");
    t.test_same("x=y|x");
    t.test_same("x=x*y");
    t.test_same("x=y*x");
    t.test_same("x=x**y");
    t.test_same("x=y**2");
    t.test_same("x.y=x.y+z");
    t.test_same("next().x = next().x + 1");
    // This is OK, really.
    t.test("({a:1}).a = ({a:1}).a + 1", "({a:1}).a = 2");
}

// port: PeepholeFoldConstantsTest#testUnfoldAssignOpsLate
#[test]
fn test_unfold_assign_ops_late() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    t.test_same("x+=y");
    t.test_same("x*=y");
    t.test_same("x.y+=z");
    t.test_same("x-=y");
    t.test_same("x|=y");
    t.test_same("x*=y");
    t.test_same("x**=y");
    t.test_same("x.y+=z");
    t.test_same("\"a\".length >>= 1;");
    t.test_same("[1, 2].length >>= 1;");
}

// port: PeepholeFoldConstantsTest#testUnfoldAssignOpsEarly
#[test]
fn test_unfold_assign_ops_early() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = false;
    t.test("x+=y", "x=x+y");
    t.test("x*=y", "x=x*y");
    t.test("x.y+=z", "x.y=x.y+z");
    t.test("x-=y", "x=x-y");
    t.test("x|=y", "x=x|y");
    t.test("x*=y", "x=x*y");
    t.test("x**=y", "x=x**y");
    t.test("x.y+=z", "x.y=x.y+z");
}

// port: PeepholeFoldConstantsTest#testFoldAdd1
#[test]
fn test_fold_add1() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x=false+1", "x=1");
    t.test("x=true+1", "x=2");
    t.test("x=1+false", "x=1");
    t.test("x=1+true", "x=2");
}

// port: PeepholeFoldConstantsTest#testFoldLiteralNames
#[test]
fn test_fold_literal_names() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("NaN == NaN", "false");
    t.test("Infinity == Infinity", "true");
    t.test("Infinity == NaN", "false");
    t.test("undefined == NaN", "false");
    t.test("undefined == Infinity", "false");
    t.test("Infinity >= Infinity", "true");
    t.test("NaN >= NaN", "false");
}

// port: PeepholeFoldConstantsTest#testFoldLiteralsTypeMismatches
#[test]
fn test_fold_literals_type_mismatches() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("true == true", "true");
    t.test("true == false", "false");
    t.test("true == null", "false");
    t.test("false == null", "false");
    // relational operators convert its operands
    t.test("null <= null", "true");
    // 0 = 0
    t.test("null >= null", "true");
    t.test("null > null", "false");
    t.test("null < null", "false");
    t.test("false >= null", "true");
    // 0 = 0
    t.test("false <= null", "true");
    t.test("false > null", "false");
    t.test("false < null", "false");
    t.test("true >= null", "true");
    // 1 > 0
    t.test("true <= null", "false");
    t.test("true > null", "true");
    t.test("true < null", "false");
    t.test("true >= false", "true");
    // 1 > 0
    t.test("true <= false", "false");
    t.test("true > false", "true");
    t.test("true < false", "false");
}

// port: PeepholeFoldConstantsTest#testFoldLeftChildConcat
#[test]
fn test_fold_left_child_concat() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("x +5 + \"1\"");
    t.test("x+\"5\" + \"1\"", "x + \"51\"");
    // test("\"a\"+(c+\"b\")", "\"a\"+c+\"b\"");
    t.test("\"a\"+(\"b\"+c)", "\"ab\"+c");
}

// port: PeepholeFoldConstantsTest#testFoldLeftChildOp
#[test]
fn test_fold_left_child_op() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x & Infinity & 2", "x & 0");
    t.test_same("x - Infinity - 2");
    // want "x-Infinity"
    t.test_same("x - 1 + Infinity");
    t.test_same("x - 2 + 1");
    t.test_same("x - 2 + 3");
    t.test_same("1 + x - 2 + 1");
    t.test_same("1 + x - 2 + 3");
    t.test_same("1 + x - 2 + 3 - 1");
    t.test_same("f(x)-0");
    t.test("x-0-0", "x-0");
    t.test_same("x+2-2+2");
    t.test_same("x+2-2+2-2");
    t.test_same("x-2+2");
    t.test_same("x-2+2-2");
    t.test_same("x-2+2-2+2");
    t.test_same("1+x-0-NaN");
    t.test_same("1+f(x)-0-NaN");
    t.test_same("1+x-0+NaN");
    t.test_same("1+f(x)-0+NaN");
    t.test_same("1+x+NaN");
    // unfoldable
    t.test_same("x+2-2");
    // unfoldable
    t.test_same("x+2");
    // nothing to do
    t.test_same("x-2");
    // nothing to do
}

// port: PeepholeFoldConstantsTest#testFoldSimpleArithmeticOp
#[test]
fn test_fold_simple_arithmetic_op() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("x|NaN");
    t.test_same("NaN/y");
    t.test_same("f(x)-0");
    t.test_same("f(x)|1");
    t.test_same("1|f(x)");
    t.test_same("0+a+b");
    t.test_same("0-a-b");
    t.test_same("a+b-0");
    t.test_same("(1+x)|NaN");
    t.test_same("(1+f(x))|NaN");
    // don't fold side-effects
}

// port: PeepholeFoldConstantsTest#testFoldLiteralsAsNumbers
#[test]
fn test_fold_literals_as_numbers() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("x/'12'", "x/12");
    t.test("x/('12'+'6')", "x/126");
    t.test("true*x", "1*x");
    t.test("x/false", "x/0");
    // should we add an error check? :)
}

// port: PeepholeFoldConstantsTest#testNotFoldBackToTrueFalse
#[test]
fn test_not_fold_back_to_true_false() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = false;
    t.test("!0", "true");
    t.test("!1", "false");
    t.test("!3", "false");
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    t.test_same("!0");
    t.test_same("!1");
    t.test("!3", "false");
    t.test_same("false");
    t.test_same("true");
}

// port: PeepholeFoldConstantsTest#testFoldBangConstants
#[test]
fn test_fold_bang_constants() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("1 + !0", "2");
    t.test("1 + !1", "1");
    t.test("'a ' + !1", "'a false'");
    t.test("'a ' + !0", "'a true'");
}

// port: PeepholeFoldConstantsTest#testFoldMixed
#[test]
fn test_fold_mixed() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("''+[1]", "'1'");
    t.test("false+[]", "\"false\"");
}

// port: PeepholeFoldConstantsTest#testFoldVoid
#[test]
fn test_fold_void() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("void 0");
    t.test("void 1", "void 0");
    t.test("void x", "void 0");
    t.test_same("void x()");
}

// port: PeepholeFoldConstantsTest#testObjectLiteral
#[test]
fn test_object_literal() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("(!{})", "false");
    t.test("(!{a:1})", "false");
    t.test_same("(!{a:foo()})");
    t.test_same("(!{'a':foo()})");
}

// port: PeepholeFoldConstantsTest#testArrayLiteral
#[test]
fn test_array_literal() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("(![])", "false");
    t.test("(![1])", "false");
    t.test("(![a])", "false");
    t.test_same("(![foo()])");
}

// port: PeepholeFoldConstantsTest#testIssue601
#[test]
fn test_issue601() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("'\\v' == 'v'");
    t.test_same("'v' == '\\v'");
    t.test_same("'\\u000B' == '\\v'");
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteralRef1
#[test]
fn test_fold_object_literal_ref1() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Leave extra side-effects in place
    t.test_same("var x = ({a:foo(),b:bar()}).a");
    t.test_same("var x = ({a:1,b:bar()}).a");
    t.test_same("function f() { return {b:foo(), a:2}.a; }");
    // on the LHS the object act as a temporary leave it in place.
    t.test_same("({a:x}).a = 1");
    t.test("({a:x}).a += 1", "({a:x}).a = x + 1");
    t.test_same("({a:x}).a ++");
    t.test_same("({a:x}).a --");
    // Getters should not be inlined.
    t.test_same("({get a() {return this}}).a");
    t.test_same("({get a() {return this}})?.a");
    // Except, if we can see that the getter function never references 'this'.
    t.test("({get a() {return 0}}).a", "(function() {return 0})()");
    t.test("({get a() {return 0}})?.a", "(function() {return 0})()");
    t.test("({get a() {return 0}})?.a.b", "(function() {return 0})().b");
    // It's okay to inline functions, as long as they're not immediately called.
    // (For tests where they are immediately called, see testFoldObjectLiteral_X)
    t.test(
        "({a:function(){return this}}).a",
        "(function(){return this})",
    );
    t.test(
        "({a:function(){return this}})?.a",
        "(function(){return this})",
    );
    // It's also okay to inline functions that are immediately called, so long as we know for
    // sure the function doesn't reference 'this'.
    t.test("({a:function(){return 0}}).a()", "(function(){return 0})()");
    t.test(
        "({a:function(){return 0}})?.a()",
        "(function(){return 0})()",
    );
    t.test(
        "({a:function(){return 0}})?.a().b",
        "(function(){return 0})().b",
    );
    // Don't inline setters.
    t.test_same("({set a(b) {return this}}).a");
    t.test_same("({set a(b) {this._a = b}}).a");
    // Don't inline if there are side-effects.
    t.test_same("({[foo()]: 1,   a: 0}).a");
    t.test_same("({['x']: foo(), a: 0}).a");
    t.test_same("({x: foo(),     a: 0}).a");
    // Leave unknown props alone, the might be on the prototype
    t.test_same("({}).a");
    // setters by themselves don't provide a definition
    t.test_same("({}).a");
    t.test_same("({set a(b) {}}).a");
    // sets don't hide other definitions.
    t.test("({a:1,set a(b) {}}).a", "1");
    // get is transformed to a call (gets don't have self referential names)
    t.test("({get a() {}}).a", "(function (){})()");
    // sets don't hide other definitions.
    t.test("({get a() {},set a(b) {}}).a", "(function (){})()");
    // a function remains a function not a call.
    t.test(
        "var x = ({a:function(){return 1}}).a",
        "var x = function(){return 1}",
    );
    t.test("var x = ({a:1}).a", "var x = 1");
    t.test("var x = ({a:1, a:2}).a", "var x = 2");
    t.test("var x = ({a:1, a:foo()}).a", "var x = foo()");
    t.test("var x = ({a:foo()}).a", "var x = foo()");
    t.test(
        "function f() { return {a:1, b:2}.a; }",
        "function f() { return 1; }",
    );
    // GETELEM is handled the same way.
    t.test("var x = ({'a':1})['a']", "var x = 1");
    // try folding string computed properties
    t.test("var a = {['a']:x}['a']", "var a = x");
    t.test("var a = {['a']:x}?.['a']", "var a = x");
    t.test("var a = {['a']:x}?.['a'].b", "var a = x.b");
    t.test("var a = {a: {b: 1}}?.a?.b", "var a = 1");
    t.test("var a = {a: null}?.a?.b", "var a = null?.b");
    t.test("var a = {['a']: {b: 1}}?.['a']?.b", "var a = 1");
    t.test("var a = {['a']: null}?.['a']?.b", "var a = null?.b");
    t.test(
        "var a = { get ['a']() { return 1; }}['a']",
        "var a = function() { return 1; }();",
    );
    t.test("var a = {'a': x, ['a']: y}['a']", "var a = y;");
    t.test_same("var a = {['foo']: x}.a;");
    // Note: it may be useful to fold symbols in the future.
    t.test_same("var y = Symbol(); var a = {[y]: 3}[y];");
    // We can fold member functions sometimes.
    // <p>Even though they're different from fn expressions and arrow fns, extracting them only
    // causes programs that would have thrown errors to change behaviour.
    // /
    t.test("var x = {a() { 1; }}.a;", "var x = function() { 1; };");
    // Notice `a` isn't invoked, so beahviour didn't change.
    t.test(
        "var x = {a() { return this; }}.a;",
        "var x = function() { return this; };",
    );
    // `super` is invisibly captures the object that declared the method so we can't fold.
    t.test_same("var x = {a() { return super.a; }}.a;");
    t.test(
        "var x = {a: 1, a() { 2; }}.a;",
        "var x = function() { 2; };",
    );
    t.test("var x = {a() {}, a: 1}.a;", "var x = 1;");
    t.test_same("var x = {a() {}}.b");
    // Don't fold non-computed setters.
    t.test_same("var x = ({ set a(v) { sideEffect() } }).a;");
    // Don't fold computed setters.
    t.test_same("var x = ({ set ['a'](v) { sideEffect() } }).a;");
    // Fold computed getters.
    t.test(
        "var x = ({ get ['a']() { return 1; } }).a;",
        "var x = function() { return 1; }();",
    );
    t.test(
        "var x = ({ get ['a']() { return 1; }, set ['a'](v) {} }).a;",
        "var x = function() { return 1; }();",
    );
    t.test(
        "var x = ({ set ['a'](v) {}, get ['a']() { return 1; } }).a;",
        "var x = function() { return 1; }();",
    );
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteralRef2
#[test]
fn test_fold_object_literal_ref2() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = false;
    t.test("({a:x}).a += 1", "({a:x}).a = x + 1");
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    t.test_same("({a:x}).a += 1");
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteral_methodCall_nonLiteralFn
#[test]
fn test_fold_object_literal_method_call_non_literal_fn() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("({a:x}).a()");
    t.test_same("({a:x})?.a()");
    t.test_same("({a:x})?.a()?.b");
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteral_freeMethodCall
#[test]
fn test_fold_object_literal_free_method_call() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("({a() { return 1; }}).a()", "(function() { return 1; })()");
    t.test("({a() { return 1; }})?.a()", "(function() { return 1; })()");
    // grandparent of optional chaining AST continues the chain
    t.test(
        "({a() { return 1; }})?.a().b",
        "(function() { return 1; })().b",
    );
    t.test(
        "({a() { return 1; }})?.a().b.c?.d",
        "(function() { return 1; })().b.c?.d",
    );
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteral_freeArrowCall_usingEnclosingThis_late
#[test]
fn test_fold_object_literal_free_arrow_call_using_enclosing_this_late() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    t.test("({a: () => this }).a()", "(() => this)()");
    t.test("({a: () => this })?.a()", "(() => this)()");
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteral_unfreeMethodCall_dueToThis
#[test]
fn test_fold_object_literal_unfree_method_call_due_to_this() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("({a() { return this; }}).a()");
    t.test_same("({a() { return this; }})?.a()");
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteral_unfreeMethodCall_dueToSuper
#[test]
fn test_fold_object_literal_unfree_method_call_due_to_super() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("({a() { return super.toString(); }}).a()");
    t.test_same("({a() { return super.toString(); }})?.a()");
}

// port: PeepholeFoldConstantsTest#testFoldObjectLiteral_paramToInvocation
#[test]
fn test_fold_object_literal_param_to_invocation() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("console.log({a: 1}.a)", "console.log(1)");
    t.test("console.log({a: 1}?.a)", "console.log(1)");
}

// port: PeepholeFoldConstantsTest#testIEString
#[test]
fn test_ie_string() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("!+'\\v1'");
}

// port: PeepholeFoldConstantsTest#testIssue522
#[test]
fn test_issue522() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("[][1] = 1;");
}

// port: PeepholeFoldConstantsTest#foldDefineProperties
#[test]
fn fold_define_properties() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test("Object.defineProperties({}, {})", "({})");
    t.test("Object.defineProperties(a, {})", "a");
    t.test_same("Object.defineProperties(a, {anything:1})");
}

// port: PeepholeFoldConstantsTest#testTypeBasedFoldConstant
#[test]
fn test_type_based_fold_constant() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("function f(/** number */ x) { x + 1 + 1 + x; }");
    t.test_same("function f(/** boolean */ x) { x + 1 + 1 + x; }");
    t.test_same("function f(/** null */ x) { var y = true > x; }");
    t.test_same("function f(/** null */ x) { var y = null > x; }");
    t.test_same("function f(/** string */ x) { x + 1 + 1 + x; }");
    t.hooks.use_types = false;
    t.test_same("function f(/** number */ x) { x + 1 + 1 + x; }");
}

// port: PeepholeFoldConstantsTest#testColorBasedFoldConstant
#[test]
fn test_color_based_fold_constant() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.harness.enable_type_check().unwrap();
    t.harness.replace_types_with_colors().unwrap();
    t.harness.disable_compare_js_doc().unwrap();
    t.test_same("function f(/** number */ x) { x + 1 + 1 + x; }");
    t.test_same("function f(/** boolean */ x) { x + 1 + 1 + x; }");
    t.test_same("function f(/** null */ x) { var y = true > x; }");
    t.test_same("function f(/** null */ x) { var y = null > x; }");
    t.test_same("function f(/** string */ x) { x + 1 + 1 + x; }");
    t.hooks.use_types = false;
    t.test_same("function f(/** number */ x) { x + 1 + 1 + x; }");
}

// port: PeepholeFoldConstantsTest#testES6Features
#[test]
fn test_es6_features() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test(
        "var x = {[undefined != true] : 1};",
        "var x = {[true] : 1};",
    );
    t.test("let x = false && y;", "let x = false;");
    t.test("const x = null == undefined", "const x = true");
    t.test(
        "var [a, , b] = [false+1, true+1, ![]]",
        "var a; var b; [a, , b] = [1, 2, false]",
    );
    t.test(
        "var x = () =>  true || x;",
        "var x = () => { return true; }",
    );
    t.test(
        "function foo(x = (1 !== void 0), y) {return x+y;}",
        "function foo(x = true, y) {return x+y;}",
    );
    t.test(
        "class Foo {\n  constructor() {this.x = null <= null;}\n}\n",
        "class Foo {\n  constructor() {this.x = true;}\n}\n",
    );
    t.test(
        "function foo() {return `${false && y}`}",
        "function foo() {return `false`}",
    );
}

// port: PeepholeFoldConstantsTest#testES6Features_late
#[test]
fn test_es6_features_late() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    t.test(
        "var [a, , b] = [false+1, true+1, ![]]",
        "var [a, , b] = [1, 2, false]",
    );
    t.test("var x = () =>  true || x;", "var x = () => true;");
}

// port: PeepholeFoldConstantsTest#testClassField
#[test]
fn test_class_field() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test(
        "class Foo {\n  x = null <= null;\n}\n",
        "class Foo {\n  x = true;\n}\n",
    );
}

// port: PeepholeFoldConstantsTest#testInvertibleOperators
#[test]
#[allow(clippy::needless_range_loop)]
fn test_invertible_operators() {
    let mut t = PeepholeFoldConstantsTest::new();
    let inverses: IndexMap<&str, &str> = IndexMap::<_, _>::from_iter([
        ("==", "!="),
        ("===", "!=="),
        ("<=", ">"),
        ("<", ">="),
        (">=", "<"),
        (">", "<="),
        ("!=", "=="),
        ("!==", "==="),
    ]);
    let comparators = ["<=", "<", ">=", ">"];
    let equalitors = ["==", "==="];
    let uncomparables = ["undefined", "void 0"];
    let operators: Vec<&str> = inverses.values().copied().collect();
    for i_operand_a in 0..LITERAL_OPERANDS.len() {
        for i_operand_b in 0..LITERAL_OPERANDS.len() {
            for i_op in 0..operators.len() {
                let a = LITERAL_OPERANDS[i_operand_a];
                let b = LITERAL_OPERANDS[i_operand_b];
                let op = operators[i_op];
                let inverse = inverses[op];

                // Test invertability.
                if comparators.contains(&op) {
                    if uncomparables.contains(&a)
                        || uncomparables.contains(&b)
                        || (a == "null"
                            && NodeUtil::get_string_number_value(&JsString::from(b)).is_none())
                    {
                        t.assert_same_results(&PeepholeFoldConstantsTest::join(a, op, b), "false");
                        t.assert_same_results(
                            &PeepholeFoldConstantsTest::join(a, inverse, b),
                            "false",
                        );
                    }
                } else if a == b && equalitors.contains(&op) {
                    if a == "NaN" || a == "Infinity" || a == "-Infinity" {
                        t.test(
                            &PeepholeFoldConstantsTest::join(a, op, b),
                            if a == "NaN" { "false" } else { "true" },
                        );
                    } else {
                        t.assert_same_results(&PeepholeFoldConstantsTest::join(a, op, b), "true");
                        t.assert_same_results(
                            &PeepholeFoldConstantsTest::join(a, inverse, b),
                            "false",
                        );
                    }
                } else {
                    t.assert_not_same_results(
                        &PeepholeFoldConstantsTest::join(a, op, b),
                        &PeepholeFoldConstantsTest::join(a, inverse, b),
                    );
                }
            }
        }
    }
}

// port: PeepholeFoldConstantsTest#testCommutativeOperators
#[test]
fn test_commutative_operators() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    let operators = ["==", "!=", "===", "!==", "*", "|", "&", "^"];
    for a in LITERAL_OPERANDS {
        for b in LITERAL_OPERANDS {
            for op in operators {
                // Test commutativity.
                t.assert_same_results(
                    &PeepholeFoldConstantsTest::join(a, op, b),
                    &PeepholeFoldConstantsTest::join(b, op, a),
                );
            }
        }
    }
}

// port: PeepholeFoldConstantsTest#testConvertToNumberNegativeInf
#[test]
fn test_convert_to_number_negative_inf() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.test_same("var x = 3 & (r ? Infinity : -Infinity);");
}

// port: PeepholeFoldConstantsTest#testAlgebraicIdentities
#[test]
fn test_algebraic_identities() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.harness.enable_type_check().unwrap();
    t.harness.replace_types_with_colors().unwrap();
    t.harness.disable_compare_js_doc().unwrap();
    t.fold_numeric_types("x+0", "x");
    t.fold_numeric_types("0+x", "x");
    t.fold_numeric_types("x+0+0+x+x+0", "x+x+x");
    t.fold_numeric_types("x-0", "x");
    t.fold_numeric_types("x-0-0-0", "x");
    // 'x-0' is numeric even if x isn't
    t.test("var x='a'; x-0-0", "var x='a';x-0");
    t.fold_numeric_types("0-x", "-x");
    t.test(
        "for (var i = 0; i < 5; i++) var x = 0 + i * 1",
        "var i = 0; for(; i < 5; i++) var x=i",
    );
    t.fold_numeric_types("x*1", "x");
    t.fold_numeric_types("1*x", "x");
    // can't optimize these without a non-NaN prover
    t.test_same("x*0");
    t.test_same("0*x");
    t.test_same("0/x");
    t.fold_numeric_types("x/1", "x");
}

// port: PeepholeFoldConstantsTest#testBigIntAlgebraicIdentities
#[test]
fn test_big_int_algebraic_identities() {
    let mut t = PeepholeFoldConstantsTest::new();
    t.harness.enable_type_check().unwrap();
    t.harness.replace_types_with_colors().unwrap();
    t.harness.disable_compare_js_doc().unwrap();
    t.fold_big_int_types("x+0n", "x");
    t.fold_big_int_types("0n+x", "x");
    t.fold_big_int_types("x+0n+0n+x+x+0n", "x+x+x");
    t.fold_big_int_types("x-0n", "x");
    t.fold_big_int_types("0n-x", "-x");
    t.fold_big_int_types("x-0n-0n-0n", "x");
    t.fold_big_int_types("x*1n", "x");
    t.fold_big_int_types("1n*x", "x");
    t.fold_big_int_types("x*1n*1n*x*x*1n", "x*x*x");
    t.fold_big_int_types("x/1n", "x");
    t.fold_big_int_types("x/0n", "x/0n");
    t.test(
        "for (var i = 0n; i < 5n; i++) var x = 0n + i * 1n",
        "var i = 0n; for(; i < 5n; i++) var x=i",
    );
    t.test(
        "for (var i = 0n; i % 2n === 0n; i++) var x = i % 2n",
        "var i = 0n; for (; i % 2n === 0n; i++) var x = i % 2n",
    );
    t.test("(doSomething(),0n)*1n", "(doSomething(),0n)");
    t.test("1n*(doSomething(),0n)", "(doSomething(),0n)");
    t.harness
        .ignore_warning_groups(std::slice::from_ref(&DiagnosticGroups::CHECK_TYPES))
        .unwrap();
    t.test_same("(0n,doSomething())*1n");
    t.test_same("1n*(0n,doSomething())");
}

// port: PeepholeFoldConstantsTest#testAssociativeFoldConstantsWithVariables
#[test]
fn test_associative_fold_constants_with_variables() {
    let mut t = PeepholeFoldConstantsTest::new();
    // MUL and ADD should not fold
    t.test_same("alert(x * 12 * 20);");
    t.test_same("alert(12 * x * 20);");
    t.test_same("alert(x + 12 + 20);");
    t.test_same("alert(12 + x +  20);");
    t.test("alert(x & 12 & 20);", "alert(x & 4);");
    t.test("alert(12 & x & 20);", "alert(x & 4);");
}

// port: PeepholeFoldConstantsTest#testTemplateLiteralConcat
#[test]
fn test_template_literal_concat() {
    let mut t = PeepholeFoldConstantsTest::new();
    // join at variables
    t.test("const a = `${x}` + `${x}`;\n", "const a = `${x}${x}`;\n");
    t.test(
        "const a = `${x}${y}` + `${x}${y}`;\n",
        "const a = `${x}${y}${x}${y}`;\n",
    );
    // join at variable and string
    t.test("const a = `${x}` + `a${x}`;\n", "const a =`${x}a${x}`;\n");
    t.test("const a = `${x}` + ` ${x}`;\n", "const a =`${x} ${x}`;\n");
    // join at string and variable
    t.test(
        "const a = `a${x}b` + `${x}c`;\n",
        "const a = `a${x}b${x}c`;\n",
    );
    t.test(
        "const a = `a${x} ` + `${x}b`;\n",
        "const a = `a${x} ${x}b`;\n",
    );
    // join at strings
    t.test(
        "const x = 'X';\nconsole.log(`a${x}b` + `c${x}d`);\n",
        "const x = 'X';\nconsole.log(`a${x}bc${x}d`);\n",
    );
    // complex joins
    t.test(
        "const a = `${foo() + bar()}` + `${baz()}`;\n",
        "const a =`${foo() + bar()}${baz()}`;\n",
    );
    t.test(
        "console.log(`<h1>${url}</h1>` + `<p>The URL is ${url}.</p>`);\n",
        "console.log(`<h1>${url}</h1><p>The URL is ${url}.</p>`);\n",
    );
    t.test(
        "console.log(`${(() => {return url;})()}</h1>` + `<p>The URL is ${url}.</p>`);\n",
        "console.log(`${(() => {return url;})()}</h1><p>The URL is ${url}.</p>`);\n",
    );
    // don't fold tagged template literals
    t.test_same("const a = foo`${b}` + `${b}`;\n");
    t.test_same("const a = foo`${b}` + bar`${b}`;\n");
    // don't fold when raw strings meet at unescaped $ and {
    t.test_same("`${x}$` + `{y}${z}`");
    t.test_same("`${x}$` + `{y}`");
    t.test("`${x}\\$` + `{y}`", "`${x}\\${y}`");
    t.test_same("`${x}\\\\$` + `{y}`");
}

// port: PeepholeFoldConstantsTest#testFoldTemplateLiteralSubstitutions
#[test]
fn test_fold_template_literal_substitutions() {
    let mut t = PeepholeFoldConstantsTest::new();
    // Basic substitution
    t.test("`a${'b'}c`", "`abc`");
    // Number substitution
    t.test("`a${123}c`", "`a123c`");
    // Boolean substitution
    t.test("`a${true}c`", "`atruec`");
    t.test("`a${false}c`", "`afalsec`");
    // Multiple substitutions
    t.test("`a${'b'}c${'d'}e`", "`abcde`");
    // Adjacent substitutions
    t.test("`${'a'}${'b'}`", "`ab`");
    // Empty strings
    t.test("`${''}a${''}`", "`a`");
    // Corner cases where it should NOT fold
    t.test_same("`a${'`'}c`");
    t.test_same("`a${'$'}c`");
    t.test_same("`a${'\\\\'}c`");
    t.test_same("`a${'\\\\n'}c`");
    t.test_same("`a${'\\\\r'}c`");
    t.test_same("`${x} ${'$'}${'{'}`");
    // specifically tests that $ and { are not folded together
    t.test("`$${''}{${''}`", "`$${''}{`");
    t.test_same("`$${''}{foo}`");
    t.test("`\\$${''}{foo}`", "`\\${foo}`");
    // These should not fold because \0 followed by a digit is a syntax error in untagged templates.
    t.test_same("`\\0${'77'}`");
    t.test_same("`\\0${77}`");
    t.test("`\\0${'a'}`", "`\\0a` ");
    t.test("`\\\\0${'7'}`", "`\\\\07` ");
    t.test("`\\0${''}${'7'}`", "`\\0${'7'}`");
    // Tagged template literals should NOT fold
    t.test_same("tag`a${'b'}c`");
    // Non-constant substitutions should NOT fold
    t.test_same("`a${x}c`");
    t.test_same("`a${foo()}c`");
}

// port: PeepholeFoldConstantsTest#testFoldAddTemplateLiteralAndConstant
#[test]
fn test_fold_add_template_literal_and_constant() {
    let mut t = PeepholeFoldConstantsTest::new();
    // String merge
    t.test("`a${x}` + 'b'", "`a${x}b` ");
    t.test("'a' + `b${x}`", "`ab${x}` ");
    // Number merge
    t.test("`a${x}` + 1", "`a${x}1` ");
    t.test("1 + `a${x}`", "`1a${x}` ");
    // Boolean merge
    t.test("`a${x}` + true", "`a${x}true` ");
    t.test("true + `a${x}`", "`truea${x}` ");
    // BigInt merge
    t.test("`a${x}` + 10n", "`a${x}10` ");
    // Null/Undefined merge
    t.test("`a${x}` + null", "`a${x}null` ");
    t.test("`a${x}` + undefined", "`a${x}undefined` ");
    // Nested templates
    t.test("`a${x}` + `b` ", "`a${x}b` ");
    t.test("`a` + `b${x}` ", "`ab${x}` ");
    // Restricted octal escape
    t.test_same("`\\0` + 7");
    t.test_same("`\\0` + '7'");
    t.test("`\\0a` + '7'", "`\\0a7` ");
    // Safety - special chars should NOT fold
    t.test_same("`a${x}` + '$'");
    t.test_same("`a${x}` + '{'");
    t.test_same("`a${x}` + '`'");
    t.test_same("`a${x}` + '\\\\'");
    t.test_same("`a${x}` + '\\\\n'");
    // Compound addition
    t.test("`a` + `b${x}` + `c` + 'd'", "`ab${x}cd` ");
}

// port: PeepholeFoldConstantsTest#testFoldAddTemplateLiterals_validateRawAndCookedStringForSpecialChars_andValidateChildren
#[test]
fn test_fold_add_template_literals_validate_raw_and_cooked_string_for_special_chars_and_validate_children()
 {
    let mut t = PeepholeFoldConstantsTest::new();
    // This is a bit tricky because there are two layers of escaping.
    // Java sees "\\n" as "(\\)n" so it becomes "\n" for the javascript compiler.
    // The compiler then interprets this as the special character "\n"
    t.test("var x = `${a}\\n` + `\\t${b}`", "var x = `${a}\\n\\t${b}`");

    // getJsRoot() returns the ROOT node for inputs only.
    // The first child is the SCRIPT node containing the input code.
    let compiler = t.harness.get_last_compiler().unwrap();
    let c = compiler.borrow();
    let script = c.get_js_root().unwrap().get_first_child(&c).unwrap();
    let var = script.get_first_child(&c).unwrap();
    let name = var.get_first_child(&c).unwrap();
    let template_lit = name.get_first_child(&c).unwrap();
    assert!(template_lit.is_template_lit(&c));

    // TEMPLATELIT should have 5 total children
    assert_eq!(template_lit.get_child_count(&c), 5);

    // Index 0: TEMPLATELIT_STRING ""
    let child0 = template_lit.get_child_at_index(&c, 0).unwrap();
    assert!(child0.is_template_lit_string(&c));
    assert!(child0.get_cooked_string(&c).unwrap().is_empty());
    assert!(child0.get_raw_string(&c).is_empty());

    // Index 1: SUB a
    let child1 = template_lit.get_child_at_index(&c, 1).unwrap();
    assert!(child1.is_template_lit_sub(&c));
    assert!(child1.get_only_child(&c).is_name(&c));
    assert_eq!(child1.get_only_child(&c).get_string(&c), "a");

    // Index 2: TEMPLATELIT_STRING "\n\t"
    let child2 = template_lit.get_child_at_index(&c, 2).unwrap();
    assert!(child2.is_template_lit_string(&c));
    assert_eq!(child2.get_cooked_string(&c).unwrap(), "\n\t");
    assert_eq!(child2.get_raw_string(&c), "\\n\\t");

    // Index 3: SUB b
    let child3 = template_lit.get_child_at_index(&c, 3).unwrap();
    assert!(child3.is_template_lit_sub(&c));
    assert!(child3.get_only_child(&c).is_name(&c));
    assert_eq!(child3.get_only_child(&c).get_string(&c), "b");

    // Index 4: TEMPLATELIT_STRING ""
    let child4 = template_lit.get_child_at_index(&c, 4).unwrap();
    assert!(child4.is_template_lit_string(&c));
    assert!(child4.get_cooked_string(&c).unwrap().is_empty());
    assert!(child4.get_raw_string(&c).is_empty());
}

// port: PeepholeFoldConstantsTest#testFoldAddTemplateLiterals_validateRawAndCookedStringForLiteralBackslash_andValidateChildren
#[test]
fn test_fold_add_template_literals_validate_raw_and_cooked_string_for_literal_backslash_and_validate_children()
 {
    let mut t = PeepholeFoldConstantsTest::new();
    // This is a bit tricky because there are two layers of escaping.
    // Java sees "\\\\n" as "(\\)(\\)n" so it becomes "\\n" for the javascript compiler.
    // The compiler then interprets this as "(\\)n" so it becomes "\" followed by
    // "n" NOT the special character "\n".
    t.test(
        "var x = `${foo()}\\\\` + `n\\t${()=> {return b;}}`",
        "var x = `${foo()}\\\\n\\t${()=> {return b;}}`",
    );

    // getJsRoot() returns the ROOT node for inputs only.
    // The first child is the SCRIPT node containing the input code.
    let compiler = t.harness.get_last_compiler().unwrap();
    let c = compiler.borrow();
    let script = c.get_js_root().unwrap().get_first_child(&c).unwrap();
    let var = script.get_first_child(&c).unwrap();
    let name = var.get_first_child(&c).unwrap();
    let template_lit = name.get_first_child(&c).unwrap();
    assert!(template_lit.is_template_lit(&c));

    // TEMPLATELIT should have 5 children:
    assert_eq!(template_lit.get_child_count(&c), 5);

    // Index 0: TEMPLATELIT_STRING ""
    let child0 = template_lit.get_child_at_index(&c, 0).unwrap();
    assert!(child0.is_template_lit_string(&c));
    assert!(child0.get_cooked_string(&c).unwrap().is_empty());
    assert!(child0.get_raw_string(&c).is_empty());

    // Index 1: SUB foo()
    let child1 = template_lit.get_child_at_index(&c, 1).unwrap();
    assert!(child1.is_template_lit_sub(&c));
    assert!(child1.get_only_child(&c).is_call(&c));
    assert_eq!(
        child1.get_only_child(&c).get_only_child(&c).get_string(&c),
        "foo"
    );

    // Index 2: TEMPLATELIT_STRING "\n\t"
    let child2 = template_lit.get_child_at_index(&c, 2).unwrap();
    assert!(child2.is_template_lit_string(&c));
    assert_eq!(child2.get_cooked_string(&c).unwrap(), "\\n\t");
    assert_eq!(child2.get_raw_string(&c), "\\\\n\\t");

    // Index 3: SUB ()=> {return b;}
    let child3 = template_lit.get_child_at_index(&c, 3).unwrap();
    assert!(child3.is_template_lit_sub(&c));
    assert!(child3.get_only_child(&c).is_function(&c));
    assert!(
        child3
            .get_only_child(&c)
            .get_first_child(&c)
            .unwrap()
            .get_string(&c)
            .is_empty()
    );

    // Index 4: TEMPLATELIT_STRING ""
    let child4 = template_lit.get_child_at_index(&c, 4).unwrap();
    assert!(child4.is_template_lit_string(&c));
    assert!(child4.get_cooked_string(&c).unwrap().is_empty());
    assert!(child4.get_raw_string(&c).is_empty());
}
