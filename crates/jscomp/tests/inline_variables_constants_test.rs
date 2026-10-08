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
/*
 * Copyright 2026 The closure-rs Authors.
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
//   test/com/google/javascript/jscomp/InlineVariablesConstantsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of InlineVariablesConstantsTest. The tests run through the native CompilerTestCase port of
//! crates/testing, with the Java setUp (enableNormalize) and getProcessor (InlineVariables in Mode
//! CONSTANTS_ONLY).
use closure_jscomp::{
    compiler_pass::CompilerPass,
    inline_variables::{InlineVariables, Mode},
};
use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    throwable::Throwable,
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

struct InlineVariablesConstantsTest {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for InlineVariablesConstantsTest {
    // port: InlineVariablesConstantsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        pass(Box::new(InlineVariables::new(Mode::CONSTANTS_ONLY)))
    }

    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

struct Fixture {
    harness: CompilerTestCase,
    hooks: InlineVariablesConstantsTest,
}

impl InlineVariablesConstantsTest {
    // port: InlineVariablesConstantsTest#setUp
    fn set_up() -> Fixture {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_normalize().unwrap();
        Fixture {
            harness,
            hooks: InlineVariablesConstantsTest {
                ctx: ctx("InlineVariablesConstantsTest"),
            },
        }
    }
}

impl Fixture {
    // port: CompilerTestCase#test(String,String)
    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap();
    }

    // port: CompilerTestCase#testSame(String)
    fn test_same(&mut self, js: &str) {
        self.harness.test_same_string(&mut self.hooks, js).unwrap();
    }

    // port: CompilerTestCase#test(TestPart...)
    #[allow(dead_code)]
    fn test_parts(&mut self, parts: Vec<TestPart>) {
        self.harness.test(&mut self.hooks, parts).unwrap();
    }

    // port: CompilerTestCase#testSame(TestPart...)
    #[allow(dead_code)]
    fn test_same_parts(&mut self, parts: Vec<TestPart>) {
        self.harness.test_same(&mut self.hooks, parts).unwrap();
    }
}

/// The native test context the harness's DSL values live in.
fn ctx(name: &str) -> Ctx {
    Ctx::new(
        name.into(),
        object([]),
        IndexMap::new(),
        Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
    )
}

/// A CompilerPass as the value getProcessor returns.
fn pass(pass: Box<dyn CompilerPass>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: CompilerTestCase#externs(String)
#[allow(dead_code)]
fn externs(code: &str) -> TestPart {
    TestPart::Externs(CompilerTestCase::externs(code))
}

// port: CompilerTestCase#srcs(String)
#[allow(dead_code)]
fn srcs(code: &str) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs(code))
}

// port: CompilerTestCase#srcs(String...)
#[allow(dead_code)]
fn srcs_n(code: &[&str]) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_strings(
        &code.iter().map(|&c| JsString::from(c)).collect::<Vec<_>>(),
    ))
}

// port: CompilerTestCase#expected(String)
#[allow(dead_code)]
fn expected(code: &str) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected(code))
}

// port: CompilerTestCase#expected(String...)
#[allow(dead_code)]
fn expected_n(code: &[&str]) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected_strings(
        &code.iter().map(|&c| JsString::from(c)).collect::<Vec<_>>(),
    ))
}

#[test]
fn test_inline_variables_constants() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test("var ABC=2; var x = ABC;", "var x=2");
    t.test("var AA = 'aa'; AA;", "'aa'");
    t.test("var A_A=10; A_A + A_A;", "10+10");
    t.test("var AA=1", "");
    t.test("var AA; AA=1", "1");
    t.test("var AA; if (false) AA=1; AA;", "if (false) 1; 1;");
    t.test_same("var AA; if (false) AA=1; else AA=2; AA;");
    // Make sure that nothing explodes if there are undeclared variables.
    t.test_same("var x = AA;");
    // Inline even if it will make the output larger,
    // because we expect on average to get more gains from the logic we can pre-compute
    // than we lose from redundant strings.
    // However, see also AliasStrings, which attempts to reduce this redundancy after we
    // have already pre-computed all of the logic we can.
    t.test(
        "var AA = '1234567890'; foo(          AA); foo(          AA); foo(          AA);",
        "                       foo('1234567890'); foo('1234567890'); foo('1234567890');",
    );
    t.test("var AA = '123456789012345';AA;", "'123456789012345'");
}

#[test]
fn test_no_inline_arrays_or_regexps() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test_same("var AA = [10,20]; AA[0]");
    t.test_same("var AA = [10,20]; AA.push(1); AA[0]");
    t.test_same("var AA = /x/; AA.test('1')");
    t.test_same("/** @const */ var aa = /x/; aa.test('1')");
}

#[test]
fn test_inline_variables_constants_js_doc_style() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test("/** @const */var abc=2; var x = abc;", "var x=2");
    t.test("/** @const */var aa = 'aa'; aa;", "'aa'");
    t.test("/** @const */var a_a=10; a_a + a_a;", "10+10");
    t.test("/** @const */var aa=1;", "");
    t.test("/** @const */var aa; aa=1;", "1");
    t.test_same("/** @const */var aa;(function () {var y; aa=y})(); var z=aa");
    // Inline even if it will make the output larger,
    // because we expect on average to get more gains from the logic we can pre-compute
    // than we lose from redundant strings.
    // However, see also AliasStrings, which attempts to reduce this redundancy after we
    // have already pre-computed all of the logic we can.
    t.test(
        r#"/** @const */
var aa = '1234567890';
foo(aa);
foo(aa);
foo(aa);
"#,
        r#"foo('1234567890');
foo('1234567890');
foo('1234567890');
"#,
    );
    t.test(
        "/** @const */var aa = '123456789012345';aa;",
        "'123456789012345'",
    );
}

#[test]
fn test_inline_conditionally_defined_constant1() {
    let mut t = InlineVariablesConstantsTest::set_up();
    // Note that inlining conditionally defined constants can change the
    // run-time behavior of code (e.g. when y is true and x is false in the
    // example below). We inline them anyway because if the code author didn't
    // want one inlined, they could define it as a non-const variable instead.
    t.test("if (x) var ABC = 2; if (y) f(ABC);", "if (x); if (y) f(2);");
}

#[test]
fn test_inline_conditionally_defined_constant2() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test(
        "if (x); else var ABC = 2; if (y) f(ABC);",
        "if (x); else; if (y) f(2);",
    );
}

#[test]
fn test_inline_conditionally_defined_constant3() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test(
        "if (x) { var ABC = 2; } if (y) { f(ABC); }",
        "if (x) {} if (y) { f(2); }",
    );
}

#[test]
fn test_inline_defined_constant() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test(
        r#"/**
 * @define {string}
 */
var aa = '1234567890';
foo(aa); foo(aa); foo(aa);
"#,
        "foo('1234567890');foo('1234567890');foo('1234567890')",
    );
    t.test(
        r#"/**
 * @define {string}
 */
var ABC = '1234567890';
foo(ABC); foo(ABC); foo(ABC);
"#,
        "foo('1234567890');foo('1234567890');foo('1234567890')",
    );
}

#[test]
fn test_inline_variables_constants_with_inline_all_strings_on() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test(
        "var AA = '1234567890'; foo(AA); foo(AA); foo(AA);",
        "foo('1234567890'); foo('1234567890'); foo('1234567890')",
    );
}

#[test]
fn test_no_inline_without_const_declaration() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test_same("var abc = 2; var x = abc;");
}

// @Ignore in Java: testInlineConstantAlias
#[test]
fn test_no_inline_aliases() {
    let mut t = InlineVariablesConstantsTest::set_up();
    t.test_same("var XXX = new Foo(); var yyy = XXX; bar(yyy)");
    t.test_same("var xxx = new Foo(); var YYY = xxx; bar(YYY)");
}
