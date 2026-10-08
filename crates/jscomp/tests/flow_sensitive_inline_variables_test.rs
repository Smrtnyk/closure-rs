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
//   test/com/google/javascript/jscomp/FlowSensitiveInlineVariablesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of FlowSensitiveInlineVariablesTest. The tests run through the native CompilerTestCase
//! port of crates/testing, with the Java setUp (enableNormalize, enableNormalizeExpectedOutput),
//! getNumRepetitions (3) and getProcessor (PureFunctionIdentifier.Driver, then
//! FlowSensitiveInlineVariables).
use closure_jscomp::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass,
    flow_sensitive_inline_variables::FlowSensitiveInlineVariables,
    pure_function_identifier::Driver,
};
use closure_rhino::{js_string::JsString, node::NodeId};
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

const EXTERN_FUNCTIONS: &str = r#"var print;
var alert;
/** @nosideeffects */ function noSFX() {}
                      function hasSFX() {}
"#;

struct FlowSensitiveInlineVariablesTest {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for FlowSensitiveInlineVariablesTest {
    // port: FlowSensitiveInlineVariablesTest#getNumRepetitions
    fn get_num_repetitions(&self) -> i32 {
        // Test repeatedly inline.
        3
    }

    // port: FlowSensitiveInlineVariablesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        pass(Box::new(
            |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                Driver::new().process(compiler, externs, root);
                FlowSensitiveInlineVariables::new().process(compiler, externs, root);
            },
        ))
    }

    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

struct Fixture {
    harness: CompilerTestCase,
    hooks: FlowSensitiveInlineVariablesTest,
}

impl FlowSensitiveInlineVariablesTest {
    // port: FlowSensitiveInlineVariablesTest#setUp
    fn set_up() -> Fixture {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        Fixture {
            harness,
            hooks: FlowSensitiveInlineVariablesTest {
                ctx: ctx("FlowSensitiveInlineVariablesTest"),
            },
        }
    }
}

impl Fixture {
    // port: FlowSensitiveInlineVariablesTest#noInline
    fn no_inline(&mut self, input: &str) {
        self.inline(input, input);
    }

    // port: FlowSensitiveInlineVariablesTest#inline
    fn inline(&mut self, input: &str, expected_code: &str) {
        self.test_parts(vec![
            externs(EXTERN_FUNCTIONS),
            srcs(&"function _func() {\nINPUT\n}\n".replace("INPUT", input)),
            expected(&"function _func() {\nEXPECTED\n}\n".replace("EXPECTED", expected_code)),
        ]);
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
fn test_simple_assign() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x; x = 1; print(x)", "var x; print(1)");
    t.inline("var x; x = 1; x", "var x; 1");
    t.inline("var x; x = 1; var a = x", "var x; var a = 1");
    t.inline("var x; x = 1; x = x + 1", "var x; x = 1 + 1");
}

#[test]
fn test_simple_var() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 1; print(x)", "var x; print(1)");
    t.inline("var x = 1; x", "var x; 1");
    t.inline("var x = 1; var a = x", "var x; var a = 1");
    t.inline("var x = 1; x = x + 1", "var x; x = 1 + 1");
}

#[test]
fn test_simple_let() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("let x = 1; print(x)", "let x; print(1)");
    t.inline("let x = 1; x", "let x; 1");
    t.inline("let x = 1; let a = x", "let x; let a = 1");
    t.inline("let x = 1; x = x + 1", "let x; x = 1 + 1");
}

#[test]
fn test_simple_const() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("const x = 1; print(x)", "const x = undefined; print(1)");
    t.inline("const x = 1; x", "const x = undefined; 1");
    t.inline(
        "const x = 1; const a = x",
        "const x = undefined; const a = 1",
    );
}

#[test]
fn test_simple_for_in() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var a,b,x = a in b; x", "var a,b,x;  a in b;  ");
    t.no_inline("var a, b; var x = a in b; print(1); x");
    t.no_inline("var a,b,x = a in b; delete a[b]; x");
}

#[test]
fn test_exported() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var _x = 1; print(_x)");
}

#[test]
fn test_do_not_inline_increment() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1; x++;");
    t.no_inline("var x = 1; x--;");
}

#[test]
fn test_multi_use() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; x = 1; print(x); print (x);");
}

#[test]
fn test_multi_use_in_same_cfg_node() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; x = 1; print(x) || print (x);");
}

#[test]
fn test_multi_use_in_two_different_path() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1; if (print) { print(x) } else { alert(x) }");
}

#[test]
fn test_assignment_before_definition() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("x = 1; var x = 0; print(x)", "x = 1; var x    ; print(0)");
}

#[test]
fn test_var_in_condition_path() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("if (foo) { var x = 0 } print(x)");
}

#[test]
fn test_multi_definitions_before_use() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 0; x = 1; print(x)", "var x = 0; print(1)");
}

#[test]
fn test_multi_definitions_in_same_cfg_node() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; (x = 1) || (x = 2); print(x)");
    t.no_inline("var x; x = (1 || (x = 2)); print(x)");
    t.no_inline("var x;(x = 1) && (x = 2); print(x)");
    t.no_inline("var x;x = (1 && (x = 2)); print(x)");
    t.no_inline("var x; x = 1 , x = 2; print(x)");
}

#[test]
fn test_not_reaching_definitions() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; if (foo) { x = 0 } print (x)");
}

#[test]
fn test_no_inline_loop_carried_definition() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // First print is undefined instead.
    t.no_inline("var x; while(true) { print(x); x = 1; }");
    // Prints 0 1 1 1 1....
    t.no_inline("var x = 0; while(true) { print(x); x = 1; }");
}

#[test]
fn test_do_not_exit_loop() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("while (z) { var x = 3; } var y = x;");
}

#[test]
fn test_do_not_inline_within_loop() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var y = noSFX(); do { var z = y.foo(); } while (true);");
}

#[test]
fn test_do_not_inline_catch_expression1() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline(
        r#"var a;
try {
  throw Error("");
}catch(err) {
   a = err;
}
return a.stack
"#,
    );
}

#[test]
fn test_do_not_inline_catch_expression1a() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline(
        r#"var a;
try {
  throw Error("");
} catch(err) {
   a = err + 1;
}
return a.stack
"#,
    );
}

#[test]
fn test_do_not_inline_catch_expression2() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline(
        r#"var a;
try {
  if (x) {throw Error("");}
} catch(err) {
   a = err;
}
return a.stack
"#,
    );
}

#[test]
fn test_do_not_inline_catch_expression3() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline(
        r#"var a;
try {
  throw Error("");
} catch(err) {
  err = x;
  a = err;
}
return a.stack
"#,
    );
}

#[test]
fn test_do_not_inline_catch_expression4() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Note: it is valid to inline "x" here but we currently don't.
    t.no_inline(
        r#"try {
  stuff();
} catch (e) {
  x = e;
  print(x);
}
"#,
    );
}

#[test]
fn test_definition_after_use() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 0; print(x); x = 1", "var x; print(0); x = 1");
}

#[test]
fn test_inline_same_variable_in_straight_line() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var x; x = 1; print(x); x = 2; print(x)",
        "var x;        print(1);        print(2)",
    );
}

#[test]
fn test_inline_in_different_paths() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var x; if (print) {x = 1; print(x)} else {x = 2; print(x)}",
        "var x; if (print) {       print(1)} else {       print(2)}",
    );
}

#[test]
fn test_no_inline_in_merged_path() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x,y;x = 1;while(y) { if(y){ print(x) } else { x = 1 } } print(x)");
}

#[test]
fn test_inline_into_expressions() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 1; print(x + 1);", "var x; print(1 + 1)");
}

#[test]
fn test_inline_expressions1() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var a, b; var x = a+b; print(x)",
        "var a, b; var x; print(a+b)",
    );
}

#[test]
fn test_inline_expressions2() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // We can't inline because of the redefinition of "a".
    t.no_inline("var a, b; var x = a + b; a = 1; print(x)");
}

#[test]
fn test_inline_expressions3() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var a,b,x; x=a+b; x=a-b; print(  x)",
        "var a,b,x; x=a+b;        print(a-b)",
    );
}

#[test]
fn test_inline_expressions4() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Precision is lost due to comma's.
    t.no_inline("var a,b,x; x=a+b, x=a-b; print(x)");
}

#[test]
fn test_inline_expressions5() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var a; var x = a = 1; print(x)");
}

#[test]
fn test_inline_expressions6() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var a, x; a = 1 + (x = 1); print(x)");
}

#[test]
fn test_inline_expression7() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Possible side effects in foo() that might conflict with bar();
    t.no_inline("var x = foo() + 1; bar(); print(x)");
    // This is a possible case but we don't have analysis to prove this yet.
    // TODO(user): It is possible to cover this case with the same algorithm
    //                as the missing return check.
    t.no_inline("var x = foo() + 1; print(x)");
}

#[test]
fn test_inline_expression8() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // The same variable inlined twice.
    t.inline(
        "var a,b; var x = a + b; print(    x); x = a - b; print(    x)",
        "var a,b; var x        ; print(a + b);            print(a - b)",
    );
}

#[test]
fn test_inline_expression9() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Check for actual control flow sensitivity.
    t.inline(
        "var a,b; var x; if (g) { x= a + b; print(    x)}  x = a - b; print(    x)",
        "var a,b; var x; if (g) {           print(a + b)}             print(a - b)",
    );
}

#[test]
fn test_inline_expression10() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // The DFA is not fine grain enough for this.
    t.no_inline("var x, y; x = ((y = 1), print(y))");
}

#[test]
fn test_inline_expressions11() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x; x = x + 1; print(x)", "var x; print(x + 1)");
    t.no_inline("var x; x = x + 1; print(x); print(x)");
}

#[test]
fn test_inline_expressions12() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // ++ is an assignment and considered to modify state so it will not be
    // inlined.
    t.no_inline("var x = 10; x = c++; print(x)");
}

#[test]
fn test_inline_expressions13() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        r#"var a = 1, b = 2;
var x = a;
var y = b;
var z = x + y;
var i = z;
var j = z + y;
var k = i;
"#,
        r#"var a, b;
var x;
var y = 2;
var z = 1 + y;
var i;
var j = z + y;
var k = z;
"#,
    );
}

#[test]
fn test_inline_expressions14() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var a = function() {}; var b = a;",
        "var a; var b = function() {}",
    );
}

#[test]
fn test_no_inline_if_definition_may_not_reach() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; if (x=1) {} x;");
}

#[test]
fn test_no_inline_escaped_to_inner_function() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1; function foo() { x = 2 }; print(x)");
}

#[test]
fn test_no_inline_l_value() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; if (x = 1) { print(x) }");
}

#[test]
fn test_switch_case() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 1; switch(x) { }", "var x; switch(1) { }");
}

#[test]
fn test_shadowed_variable_inner_function() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var x = 1; print(x) || (function() {  var x; x = 1; print(x)})()",
        "var x    ; print(1) || (function() {  var x;        print(1)})()",
    );
}

#[test]
fn test_catch() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 0; try { } catch (x) { }");
    t.no_inline("try { } catch (x) { print(x) }");
}

#[test]
fn test_no_inline_get_prop1() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // We don't know if j aliases a.b
    t.no_inline("var x = a.b.c; j.c = 1; print(x);");
}

#[test]
fn test_no_inline_get_prop2() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1 * a.b.c; j.c = 1; print(x);");
}

#[test]
fn test_no_inline_get_prop3() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Anything inside a function is fine.
    t.inline(
        "var a = {b: {}}; var x = function(){1 * a.b.c}; print(x);",
        "var a = {b: {}}; var x; print(function(){1 * a.b.c});",
    );
}

#[test]
fn test_no_inline_get_elem() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Again we don't know if i = j
    t.no_inline("var x = a[i]; a[j] = 2; print(x); ");
}

#[test]
fn test_no_inline_constructors() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = new Iterator(); x.next();");
}

#[test]
fn test_no_inline_array_lits() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = []; print(x)");
}

#[test]
fn test_no_inline_object_lits() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = {}; print(x)");
}

#[test]
fn test_no_inline_reg_exp_lits() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = /y/; print(x)");
}

#[test]
fn test_inline_constructor_calls_into_loop() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Don't inline construction into loops.
    t.no_inline(
        r#"var x = new Iterator();
for(i = 0; i < 10; i++) {
  j = x.next();
}
"#,
    );
}

#[test]
fn test_remove_with_labels() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var x = 1; L: x = 2; print(x)",
        "var x = 1; L: {    } print(2)",
    );
    t.inline(
        "var x = 1; L: M: x = 2; print(x)",
        "var x = 1; L: M: {    } print(2)",
    );
    t.inline(
        "var x = 1; L: M: N: x = 2; print(x)",
        "var x = 1; L: M: N: {    } print(2)",
    );
}

#[test]
fn test_inline_across_side_effect1() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // This can't be inlined because print() has side-effects and might change
    // the definition of noSFX.
    //
    // noSFX must be both const and pure in order to inline it.
    t.no_inline("var y; var x = noSFX(y); print(x)");
    // inline("var y; var x = noSFX(y); print(x)", "var y;var x;print(noSFX(y))");
}

#[test]
fn test_inline_across_side_effect2() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Think noSFX() as a function that reads y.foo and return it
    // and SFX() write some new value of y.foo. If that's the case,
    // inlining across hasSFX() is not valid.
    // This is a case where hasSFX is right of the source of the inlining.
    t.no_inline("var y; var x = noSFX(y), z = hasSFX(y); print(x)");
    t.no_inline("var y; var x = noSFX(y), z = new hasSFX(y); print(x)");
    t.no_inline("var y; var x = new noSFX(y), z = new hasSFX(y); print(x)");
}

#[test]
fn test_inline_across_side_effect3() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // This is a case where hasSFX is left of the destination of the inlining.
    t.no_inline("var y; var x = noSFX(y); hasSFX(y), print(x)");
    t.no_inline("var y; var x = noSFX(y); new hasSFX(y), print(x)");
    t.no_inline("var y; var x = new noSFX(y); new hasSFX(y), print(x)");
}

#[test]
fn test_inline_across_side_effect4() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // This is a case where hasSFX is some control flow path between the
    // source and its destination.
    t.no_inline("var y; var x = noSFX(y); hasSFX(y); print(x)");
    t.no_inline("var y; var x = noSFX(y); new hasSFX(y); print(x)");
    t.no_inline("var y; var x = new noSFX(y); new hasSFX(y); print(x)");
}

#[test]
fn test_can_inline_across_no_side_effect() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // This can't be inlined because print() has side-effects and might change
    // the definition of noSFX. We should be able to mark noSFX as const
    // in some way.
    t.no_inline(
        r#"var y;
var x = noSFX(y),
z = noSFX();
noSFX();
noSFX(),
print(x)
"#,
    );
    // inline(
    //    "var y; var x = noSFX(y), z = noSFX(); noSFX(); noSFX(), print(x)",
    //    "var y; var x, z = noSFX(); noSFX(); noSFX(), print(noSFX(y))");
}

#[test]
fn test_depend_on_outer_scope_variables() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; function foo() { var y = x; x = 0; print(y) }");
    t.no_inline("var x; function foo() { var y = x; x++; print(y) }");
    // Sadly, we don't understand the data flow of outer scoped variables as
    // it can be modified by code outside of this scope. We can't inline
    // at all if the definition has dependence on such variable.
    t.no_inline("var x; function foo() { var y = x; print(y) }");
}

#[test]
fn test_inline_if_name_is_left_side_of_assign() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 1; x = print(x) + 1", "var x    ; x = print(1) + 1");
    t.inline("var x = 1; L: x = x + 2", "var x    ; L: x = 1 + 2");
    t.inline("var x = 1; x = (x = x + 1)", "var x    ; x = (x = 1 + 1)");
    t.inline(
        r#"// Create a block scope within the function
{
  const C1 = 1;
  const C2 = 2;
// `var` gives `x` a larger scope than `C1`
  var x = C1;
  x = x == C1 ? C1 * 2 : C2 * 2;
}
// `x` still exists here
console.log(x);
"#,
        r#"{
  const C1 = 1;
  const C2 = undefined; // C2 was inlined
  var x = C1;
  x = x == C1 ? C1 * 2 : 2 * 2;
}
// Inlining `C1` to replace `x` here would not work, since `C1` is out of scope here.
console.log(x);
"#,
    );
    t.inline(
        r#"// Create a block scope within the function
{
  const C1 = 1;
  const C2 = 2;
// `let` gives `x` the same scope as `C1`
  let x = C1;
  x = x == C1 ? C1 * 2 : C2 * 2;
}
"#,
        r#"{
  const C1 = 1;
  const C2 = undefined; // C2 was inlined
  let x; // x was inlined
  x = C1 == C1 ? C1 * 2 : 2 * 2;
}
"#,
    );
    t.no_inline("var x = 1; x = (x = (x = 10) + x)");
    t.no_inline("var x = 1; x = (f(x) + (x = 10) + x);");
    t.no_inline("var x = 1; x=-1,foo(x)");
    t.no_inline("var x = 1; x-=1,foo(x)");
}

#[test]
fn test_inline_arguments() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test_same("function _func(x) { print(x) }");
    t.test_same("function _func(x,y) { if(y) { x = 1 }; print(x) }");
    t.test(
        "function f(x, y) { x = 1; print(x) }",
        "function f(x, y) {        print(1) }",
    );
    t.test(
        "function f(x, y) { if (y) { x = 1; print(x) }}",
        "function f(x, y) { if (y) {        print(1) }}",
    );
}

#[test]
fn test_invalid_inline_arguments1() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test_same("function f(x, y) { x = 1; arguments[0] = 2; print(x) }");
    t.test_same(
        r#"function f(x, y) {
  x = 1;
  var z = arguments;
  z[0] = 2;
  z[1] = 3;
  print(x);
}
"#,
    );
    t.test_same("function g(a){a[0]=2} function f(x){x=1;g(arguments);print(x)}");
}

#[test]
fn test_invalid_inline_arguments2() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test_same(
        r#"function f(c) {
  var f = c;
  arguments[0] = this;
  f.apply(this, arguments);
  return this;
}
"#,
    );
}

#[test]
fn test_for_in() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; var y = {}; for(x in y){}");
    t.no_inline("var x; var y = {}; var z; for(x in z = y){print(z)}");
    t.no_inline("var x; var y = {}; var z; for(x in y){print(z)}");
}

#[test]
fn test_for_in_destructuring() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1, y = [], z; for ({z = x} in y) {}");
    t.no_inline("var x = 1, y = [], z; for ([z = x] in y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for ({z = x} in y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for ([z = x] in y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for (let {z = x} in y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for (const {z = x} in y) {}");
    t.no_inline(
        "var x = 1; if (true) { x = 3; } var y = [[0]], z = x; for ([x] in y) {}; alert(z);",
    );
}

#[test]
fn test_not_ok_to_skip_check_path_between_nodes() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x; for(x = 1; foo(x);) {}");
    t.no_inline("var x; for(; x = 1;foo(x)) {}");
}

#[test]
fn test_issue698() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Most of the flow algorithms operate on Vars. We want to make
    // sure the algorithm bails out appropriately if it sees
    // a var that it doesn't know about.
    t.inline(
        r#"var x = '';
unknown.length < 2 && (unknown='0' + unknown);
x = x + unknown;
unknown.length < 3 && (unknown='0' + unknown);
x = x + unknown;
return x;
"#,
        r#"var x;
unknown.length < 2 && (unknown='0' + unknown);
x = '' + unknown;
unknown.length < 3 && (unknown='0' + unknown);
x = x + unknown;
return x;
"#,
    );
}

#[test]
fn test_issue777() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        r#"function f(cmd, ta) {
  var temp = cmd;
  var temp2 = temp >> 2;
  cmd = STACKTOP;
  for (var src = temp2, dest = cmd >> 2, stop = src + 37;
       src < stop;
       src++, dest++) {
    HEAP32[dest] = HEAP32[src];
  }
  temp = ta;
  temp2 = temp >> 2;
  ta = STACKTOP;
  STACKTOP += 8;
  HEAP32[ta >> 2] = HEAP32[temp2];
  HEAP32[ta + 4 >> 2] = HEAP32[temp2 + 1];
}
"#,
        r#"function f(cmd, ta){
  var temp;
  var temp2 = cmd >> 2;
  cmd = STACKTOP;
  var src = temp2;
  var dest = cmd >> 2;
  var stop = src + 37;
  for(;src<stop;src++,dest++)HEAP32[dest]=HEAP32[src];
  temp2 = ta >> 2;
  ta = STACKTOP;
  STACKTOP += 8;
  HEAP32[ta>>2] = HEAP32[temp2];
  HEAP32[ta+4>>2] = HEAP32[temp2+1];
}
"#,
    );
}

#[test]
fn test_transitive_dependencies1() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        "function f(x) { var a = x; var b = a; x = 3; return b; }",
        "function f(x) { var a;     var b = x; x = 3; return b; }",
    );
}

#[test]
fn test_transitive_dependencies2() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        "function f(x) { var a = x; var b = a; var c = b; x = 3; return c; }",
        "function f(x) { var a    ; var b = x; var c    ; x = 3; return b; }",
    );
}

#[test]
fn test_issue794a() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline(
        r#"var x = 1;
try { x += someFunction(); } catch (e) {}
x += 1;
try { x += someFunction(); } catch (e) {}
return x;
"#,
    );
}

#[test]
fn test_issue794b() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline(
        r#"var x = 1;
try { x = x + someFunction(); } catch (e) {}
x = x + 1;
try { x = x + someFunction(); } catch (e) {}
return x;
"#,
    );
}

#[test]
fn test_var_assign_inside_hook_issue965() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var i = 0; return 1 ? (i = 5) : 0, i;");
    t.no_inline("var i = 0; return (1 ? (i = 5) : 0) ? i : 0;");
    t.no_inline("var i = 0; return (1 ? (i = 5) : 0) || i;");
    t.no_inline("var i = 0; return (1 ? (i = 5) : 0) * i;");
}

#[test]
fn test_inline_string_concat() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        r#"function f() {
  var x = '';
  x = x + '1';
  x = x + '2';
  x = x + '3';
  x = x + '4';
  x = x + '5';
  x = x + '6';
  x = x + '7';
  return x;
}
"#,
        "function f() { var x; return '' + '1' + '2' + '3' + '4' + '5' + '6' + '7'; }",
    );
}

#[test]
fn test_inline_in_arrow_functions() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        "() => {var v; v = 1; return v;} ",
        "() => {var v       ; return 1;}",
    );
    t.test("(v) => {v = 1; return v;}", "(v) => {       return 1;}");
}

#[test]
fn test_inline_in_class_member_functions() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        r#"class C {
  func() {
    var x;
    x = 1;
    return x;
  }
}
"#,
        r#"class C {
  func() {
    var x;
    return 1;
  }
}
"#,
    );
}

#[test]
fn test_inline_let() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("let a = 1; print(a + 1)", "let a;     print(1 + 1)");
    t.inline("let a; a = 1; print(a + 1)", "let a;        print(1 + 1)");
    t.no_inline("let a = noSFX(); print(a)");
}

#[test]
fn test_inline_const() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "const a = 1        ; print(a + 1)",
        "const a = undefined; print(1 + 1)",
    );
    t.inline(
        "const a =         1; const b =         a; print(b + 1)",
        "const a = undefined; const b = undefined; print(1 + 1)",
    );
    t.no_inline("const a = noSFX(); print(a)");
}

#[test]
fn test_specific() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("let a = 1; print(a + 1)", "let a    ; print(1 + 1)");
}

#[test]
fn test_block_scoping() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        r#"let a = 1
print(a + 1);
{
  let b = 2;
  print(b + 1);
}
"#,
        r#"let a;
print(1 + 1);
{
  let b;
  print(2 + 1);
}
"#,
    );
    t.inline(
        r#"let a = 1
{
  let a = 2;
  print(a + 1);
}
print(a + 1);
"#,
        r#"let a = 1
{
  let a;
  print(2 + 1);
}
print(a + 1);
"#,
    );
    t.inline(
        r#"let a = 1;
  {let b;}
print(a)
"#,
        r#"let a;
  {let b;}
print(1)
"#,
    );
    // This test fails to inline due to CheckPathsBetweenNodes analysis in the canInline function
    // in FlowSensitiveInlineVariables.
    t.no_inline(
        r#"let a = 1;
{
  let b;
  f(b);
}
return(a)
"#,
    );
}

#[test]
fn test_block_scoping_shouldnt_inline() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline(
        r#"var JSCompiler_inline_result;
{
  let a = 1;
  if (3 < 4) {
    a = 2;
  }
  JSCompiler_inline_result = a;
}
alert(JSCompiler_inline_result);
"#,
    );
    // test let/const shadowing of a var
    t.no_inline(
        r#"var JSCompiler_inline_result;
var a = 0;
{
  let a = 1;
  if (3 < 4) {
    a = 2;
  }
  JSCompiler_inline_result = a;
}
alert(JSCompiler_inline_result);
"#,
    );
    t.no_inline("{ let value = 1; var g = () => value; } return g;");
}

#[test]
fn test_inline_in_generators() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        r#"function* f() {
  var x = 1;
  return x + 1;
}
"#,
        r#"function* f() {
  var x;
  return 1 + 1;
}
"#,
    );
}

#[test]
fn test_no_inline_for_of() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("for (var x of n){} ");
    t.no_inline("var x = 1; var n = {}; for(x of n) {}");
}

#[test]
fn test_for_of_destructuring() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1, y = [], z; for ({z = x} of y) {}");
    t.no_inline("var x = 1, y = [], z; for ([z = x] of y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for ({z = x} of y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for ([z = x] of y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for (let [z = x] of y) {}");
    t.no_inline("var x = 1, y = [], z; print(x); for (const [z = x] of y) {}");
    t.no_inline(
        "var x = 1; if (true) { x = 3; } var y = [[0]], z = x; for ([x] of y) {}; alert(z);",
    );
}

#[test]
fn test_template_strings() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var name = 'Foo'; `Hello ${ name}`",
        "var name        ; `Hello ${'Foo'}`",
    );
    t.inline(
        "var name = 'Foo'; var foo = name; `Hello ${ foo }`",
        "var name        ; var foo       ; `Hello ${'Foo'}`",
    );
    t.inline("var age = 3; `Age: ${age}`", "var age    ; `Age: ${  3}`");
}

#[test]
fn test_array_destructuring() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var [a, b, c] = [1, 2, 3]; print(a + b + c);");
    t.no_inline("var arr = [1, 2, 3, 4]; var [a, b, ,d] = arr;");
    t.no_inline("var x = 3; [x] = 4; print(x);");
    t.inline("var [x] = []; x = 3; print(x);", "var [x] = []; print(3);");
}

#[test]
fn test_object_destructuring() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var {a, b} = {a: 3, b: 4}; print(a + b);");
    t.no_inline("var obj = {a: 3, b: 4}; var {a, b} = obj;");
}

#[test]
fn test_dont_inline_over_changing_rvalue_destructuring() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1; if (true) { x = 2; } var y = x; var [z = (x = 3, 4)] = []; print(y);");
    t.no_inline("var x = 1; if (true) { x = 2; } var y = x; [x] = []; print(y);");
    t.no_inline("var x = 1; if (true) { x = 2; } var y = x; ({x} = {}); print(y);");
    t.no_inline("var x = 1; if (true) { x = 2; } var y = x; var [z] = [x = 3]; print(y);");
}

#[test]
fn test_destructuring_default_value() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 1; var [y = x] = [];", "var x; var [y = 1] = [];");
    t.inline("var x = 1; var {y = x} = {};", "var x; var {y = 1} = {};");
    t.inline(
        "var x = 1; var {[3]: y = x} = {};",
        "var x; var {[3]: y = 1} = {};",
    );
    t.no_inline("var x = 1; var {[x]: y = x} = {};");
    t.no_inline("var x = 1; var [y = x] = []; print(x);");
    // don't inline because x is only conditionally reassigned to 2.
    t.no_inline("var x = 1; var [y = (x = 2, 4)] = []; print(x);");
    t.no_inline("var x = 1; print(x); var [y = (x = 2, 4)] = [0]; print(x);");
    // x = 2 is executed before reading x in the default value.
    t.inline(
        "var x = 1; print(x); var obj = {}; [obj[x = 2] = x] = [];",
        "var x    ; print(1); var obj = {}; [obj[x = 2] = x] = [];",
    );
    // [x] is evaluated before obj[x = 2] is executed
    t.no_inline("var x = 1; print(x); var obj = {}; [[obj[x = 2]] = [x]] = [];");
    t.no_inline("var x = 1; alert(x); ({x = x * 2} = {});");
    t.no_inline("var x = 1; alert(x); [x = x * 2] = [];");
}

#[test]
fn test_destructuring_computed_property() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("var x = 1; var {[x]: y} = {};", "var x; var {[1]: y} = {};");
    t.no_inline("var x = 1; var {[x]: y} = {}; print(x);");
    t.no_inline("var x = 1; alert(x); ({[x]: x} = {}); alert(x);");
    t.inline(
        "var x = 1; var y = x; ({[y]: x} = {});",
        "var x; var y; ({[1]: x} = {});",
    );
}

#[test]
fn test_dead_assignments() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "let a = 3; if (3 < 4) { a = 8; } else { print(a); }",
        "let a; if (3 < 4) { a = 8 } else { print(3); }",
    );
    t.inline(
        "let a = 3; if (3 < 4) { [a] = 8; } else { print(a); }",
        "let a; if (3 < 4) { [a] = 8 } else { print(3); }",
    );
}

#[test]
fn test_destructuring_evaluation_order() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Should not inline "x = 2" in these cases because x is changed beforehand
    t.no_inline("var x = 2; var {x, y = x} = {x: 3};");
    t.no_inline("var x = 2; var {y = (x = 3), z = x} = {};");
    // These examples are safe to inline, but FlowSensitiveInlineVariables never inlines variables
    // used twice in the same CFG node even when safe to do so.
    t.no_inline("var x = 2; var {a: y = (x = 3)} = {a: x};");
    t.no_inline("var x = 1; var {a = x} = {a: (x = 2, 3)};");
    t.no_inline("var x = 2; var {a: x = 3} = {a: x};");
    t.no_inline("var x = 1; print(x); var {a: x} = {a: x};");
    t.no_inline("var x = 1; print(x); ({a: x} = {a: x});");
    t.no_inline("var x = 1; print(x); var y; [y = x, x] = [];");
    t.inline(
        "var x = 1; print(x); var y; [x, y = x] = [2];",
        "var x    ; print(1); var y; [x, y = x] = [2];",
    );
}

#[test]
fn test_destructuring_with_side_effects() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("function f() { x++; }  var x = 2; var {y = x} = {key: f()}");
    t.no_inline("function f() { x++; } var x = 2; var y = x; var {z = y} = {a: f()}");
    t.no_inline("function f() { x++; } var x = 2; var y = x; var {a = f(), b = y} = {}");
    t.no_inline("function f() { x++; } var x = 2; var y = x; var {[f()]: z = y} = {}");
    t.no_inline("function f() { x++; } var x = 2; var y = x; var {a: {b: z = y} = f()} = {};");
    t.no_inline(
        "function f() { x++; } var x = 2; var y; var {z = (y = x, 3)} = {a: f()}; print(y);",
    );
    t.inline(
        "function f() { x++; }  var x = 2; var y = x; var {z = f()} = {a: y}",
        "function f() { x++; }  var x = 2; var y    ; var {z = f()} = {a: x}",
    );
    t.inline(
        "function f() { x++; }  var x = 2; var y = x; var {a = y, b = f()} = {}",
        "function f() { x++; }  var x = 2; var y    ; var {a = x, b = f()} = {}",
    );
    t.inline(
        "function f() { x++; }  var x = 2; var y = x; var {[y]: z = f()} = {}",
        "function f() { x++; }  var x = 2; var y    ; var {[x]: z = f()} = {}",
    );
    t.inline(
        "function f() { x++; } var x = 2; var y = x; var {a: {b: z = f()} = y} = {};",
        "function f() { x++; } var x = 2; var y    ; var {a: {b: z = f()} = x} = {};",
    );
}

#[test]
fn test_github_issue2818() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.no_inline("var x = 1; var y = x; print(x++, y);");
    t.no_inline("var x = 1; var y = x; print(x = x + 3, y);");
    t.no_inline("var x = 1; var y = x; print(({x} = {x: x * 2}), y); print(x);");
}

#[test]
fn test_no_inline_on_optional_get_prop() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // b/360959953 - github issue 4187
    // `const t2 = y?.left` should not get inlined into `node.right = t2` (to be
    // `node.right = y?.left`), because the `y?.left` value needs to be stored here
    // in t2, before it is rewritten by `y.left = node`.
    t.test_same(
        r#"function swap(node) {
const y = node.right;
const t2 = y?.left;
y.left = node;
node.right = t2;
return node;
}
"#,
    );
}

#[test]
fn test_no_inline_on_optional_get_elem() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // b/360959953 - github issue 4187
    t.test_same(
        r#"function swap(node) {
const y = node.right;
const t2 = y?.['foo'];
y['foo'] = node;
node.right = t2;
return node;
}
"#,
    );
}

#[test]
fn test_no_inline_on_await() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        "async function f() {var x = 1; print(x) }",
        "async function f() { var x; print(1) }",
    );
    t.test_same("async function f() {var x = await 1; print(x) }");
}

#[test]
fn test_no_inline_on_yeild() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test(
        "function *f() {var x = 1; print(x) }",
        "function *f() { var x; print(1) }",
    );
    t.test_same("function *f() {var x = yield 1; print(x) }");
}

#[test]
fn test_no_inline_on_class() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
const x = class {};
const y = x;
}
"#,
    );
}

#[test]
fn test_no_inline_on_tagged_template() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
var f = (a)=>{};
const x = f`tagged`;
const y = x;
}
"#,
    );
}

#[test]
fn test_inline_on_optional_call() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline("let x = 1; const y = print(x)", "let x; const y = print(1)");
    t.inline(
        "let x = 1; const y = print?.(x)",
        "let x; const y = print?.(1)",
    );
}

#[test]
fn test_okay_to_inline_with_side_effects() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    t.inline(
        "var x = 1; var y = x; var z = 1; print(z++, y);",
        "var x    ; var y    ; var z = 1; print(z++, 1);",
    );
    t.inline(
        "var x = 1; var y = x; var z = 1; print([z] = [], y);",
        "var x    ; var y    ; var z = 1; print([z] = [], 1);",
    );
    t.inline(
        "var x = 1; var y = x; print(x = 3, y);",
        "var x; var y; print(x = 3, 1);",
    );
    t.inline(
        "var x = 1; if (true) { x = 2; } var y = x; var z; z = x = y + 1;",
        "var x = 1; if (true) { x = 2; } var y    ; var z; z = x = x + 1;",
    );
}

#[test]
fn test_path_check_with_side_effects() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Control: call blocks inlining
    t.no_inline("var x = 1; hasSFX(); var z = 1; print(x);");
    // Tagged template blocks inlining
    t.no_inline("var x = 1; hasSFX`${1}`; var z = 1; print(x);");
    // Optional chain call blocks inlining
    t.no_inline("var x = 1; hasSFX?.(1); var z = 1; print(x);");
}

#[test]
fn test_path_check_without_side_effects() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Optional chain call without side effects allows inlining
    t.inline(
        "var x = 1; noSFX?.(1); var a = x;",
        "var x; noSFX?.(1); var a = 1;",
    );
    // Tagged template without side effects allows inlining
    t.inline(
        "var x = 1; noSFX`${1}`; var a = x;",
        "var x; noSFX`${1}`; var a = 1;",
    );
}

#[test]
fn test_no_inline_across_property_writes() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // Property assignment via dot notation (obj.p = v)
    t.no_inline(
        r#"var user = {};
var r = (function(){ return user.role; })();
user.role = 'admin';
if (r !== 'admin') doPrivileged();
"#,
    );
    // Property assignment via bracket notation (obj[k] = v)
    t.no_inline(
        r#"var user = {};
var r = (function(){ return user['role']; })();
user['role'] = 'admin';
if (r !== 'admin') doPrivileged();
"#,
    );
    // Compound property assignment (obj.p += v)
    t.no_inline(
        r#"var user = {};
var r = (function(){ return user.role; })();
user.role += 'admin';
if (r !== 'admin') doPrivileged();
"#,
    );
    // Property increment/decrement (obj.p++)
    t.no_inline(
        r#"var user = {};
var r = (function(){ return user.role; })();
user.role++;
if (r !== 'admin') doPrivileged();
"#,
    );
    // Property assignment in the same statement, evaluated before the use
    t.no_inline(
        r#"var user = {};
var r = (function(){ return user.role; })();
print(user.role = 'admin', r);
"#,
    );
    // Property assignment in the same statement, evaluated after the definition
    t.no_inline(
        r#"var user = {};
var r = (function(){ return user.role; })(), _ = user.role = 'admin';
if (r !== 'admin') doPrivileged();
"#,
    );
    // Safe to inline: property assignment is the top-level assign target of the use,
    // so the mutation occurs after the RHS is evaluated
    t.inline(
        r#"var user = {};
var r = (function(){ return user.role; })();
user.role = r;
"#,
        r#"var user = {};
var r;
user.role = (function(){ return user.role; })();
"#,
    );
}

#[test]
fn test_no_inline_enhanced_for_loops_lhs() {
    let mut t = FlowSensitiveInlineVariablesTest::set_up();
    // For-in variant
    t.test_same(
        r#"function f(input, obj, sink) {
  var x; x = input;
  x = '';
  print(x);
  for (x in obj) {}
  sink(x);
}
"#,
    );
    // For-of variant
    t.test_same(
        r#"function f(input, arr, sink) {
  var x; x = input;
  x = '';
  print(x);
  for (x of arr) {}
  sink(x);
}
"#,
    );
    // For-await-of variant
    t.test_same(
        r#"async function f(input, arr, sink) {
  var x; x = input;
  x = '';
  print(x);
  for await (x of arr) {}
  sink(x);
}
"#,
    );
}
