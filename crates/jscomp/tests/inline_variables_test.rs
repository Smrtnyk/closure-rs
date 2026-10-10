/*
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/InlineVariablesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of InlineVariablesTest. The tests run through the native CompilerTestCase port of
//! crates/testing, with the Java setUp (enableNormalize, enableNormalizeExpectedOutput,
//! disableCompareJsDoc) and getProcessor (InlineVariables in Mode ALL, or LOCALS_ONLY when
//! `inline_locals_only` is set).
use closure_jscomp::{
    compiler_pass::CompilerPass,
    inline_variables::{InlineVariables, Mode},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    testing::js_chunk_graph_builder::JSChunkGraphBuilder,
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct InlineVariablesTest {
    ctx: Ctx,
    inline_locals_only: bool,
    /// Rust-only: how often the pass runs on one compiler (Java's tests run it once).
    num_repetitions: i32,
}

impl CompilerTestCaseHooks for InlineVariablesTest {
    // port: InlineVariablesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        pass(Box::new(InlineVariables::new(if self.inline_locals_only {
            Mode::LOCALS_ONLY
        } else {
            Mode::ALL
        })))
    }

    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }

    // Rust-only: the tests of the skipped externs run the pass twice.
    fn get_num_repetitions(&self) -> i32 {
        self.num_repetitions
    }
}

struct Fixture {
    harness: CompilerTestCase,
    hooks: InlineVariablesTest,
}

impl InlineVariablesTest {
    // port: InlineVariablesTest#setUp
    fn set_up() -> Fixture {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.disable_compare_js_doc().unwrap();
        // NOTE: We are not enabling var checks here, so it is OK to use undeclared variables in
        // these tests. They will be treated as if they were externs.
        Fixture {
            harness,
            hooks: InlineVariablesTest {
                ctx: ctx("InlineVariablesTest"),
                inline_locals_only: false,
                num_repetitions: 1,
            },
        }
    }
}

// srcs(JSChunkGraphBuilder...build())
fn srcs_chunks(chunks: Vec<closure_jscomp::js_chunk::JSChunk>) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_chunks(chunks))
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
        IndexMap::<_, _>::default(),
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
fn test_arg_alias() {
    let mut t = InlineVariablesTest::set_up();
    // same scope
    t.test(
        r#"function foo(arg) {
  const argAlias = arg;
  const argAliasAlias = argAlias
  use(argAliasAlias);
}
"#,
        r#"function foo(arg) {
  use(arg);
}
"#,
    );
    // nested scope
    t.test(
        r#"function foo(arg) {
  const argAlias = arg;
  {
    const argAliasAlias = argAlias
    use(argAliasAlias);
  }
}
"#,
        r#"function foo(arg) {
  {
    use(arg);
  }
}
"#,
    );
}

#[test]
fn test_inline_expression_using_alias() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function foo(arg1, arg2) {
  const arg1Alias = arg1;
  const arg2Alias = arg2;
// Create an inner scope to force analysis of `product` before the alias
// variables.
  {
    const product = arg1Alias * arg2Alias;
// Use `product` twice so if it gets inlined, it will have to create new
// references to the operands.
// Either the expression should not be inlined, or some effort will have
// to be made to avoid creating new references to `arg1Alias` and `arg2Alias`
// before they are considered for inlining themselves.
    use(product);
    use(product);
  }
}
"#,
        r#"function foo(arg1, arg2) {
  {
    const product = arg1 * arg2;
    use(product);
    use(product);
  }
}
"#,
    );
}

#[test]
fn test_pass_doesnt_produce_invalid_code1() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f(x = void 0) {
  var z;
  {
    const y = {};
    x && (y['x'] = x);
    z = y;
  }
  return z;
}
"#,
    );
}

#[test]
fn test_pass_doesnt_produce_invalid_code2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f(x = void 0) {
  {
    var z;
    const y = {};
    x && (y['x'] = x);
    z = y;
  }
  return z;
}
"#,
    );
}

#[test]
fn test_pass_doesnt_produce_invalid_code3() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x = void 0) {
  var z;
  const y = {};
  x && (y['x'] = x);
  z = y;
  {
    return z;
  }
}
"#,
        r#"function f(x = void 0) {

  const y = {};
  x && (y['x'] = x);
  y;
  {
    return y;
  }
}
"#,
    );
}

#[test]
fn test_inline_global() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x = 1; var z = x;", "var z = 1;");
}

#[test]
fn test_no_inline_annotation() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("/** @noinline */ var x = 1; use(x);");
    t.test(
        "/** @noinline */ var x = 1; use(x); var z = x; use(z);",
        "/** @noinline */ var x = 1; use(x);            use(x);",
    );
}

#[test]
fn test_no_inline_exported_name() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var _x = 1; var z = _x;");
}

#[test]
fn test_no_inline_exported_name2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var f = function() {};
var _x = f;
var y = function() { _x(); };
var _y = f;
"#,
    );
}

#[test]
fn test_dont_treat_local_variables_as_exported_by_convention() {
    let mut t = InlineVariablesTest::set_up();
    // In the GoogleCodingConvention, globals starting with "_" are exported. (see the above two
    // tests). Verify that we don't accidentally apply the same convention to locals.
    t.test(
        "function f() { var _x = 1; var z = _x; }",
        "function f() { var z = 1; }",
    );
}

#[test]
fn test_do_not_inline_increment() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = 1; x++;");
}

#[test]
fn test_do_not_inline_decrement() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = 1; x--;");
}

#[test]
fn test_do_not_inline_into_lhs_of_assign() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = 1; x += 3;");
}

#[test]
fn test_inline_into_rhs_of_assign() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x = 1; var y = x;", "var y = 1;");
}

#[test]
fn test_inline_in_function1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "function baz() { var x = 1; var z = x; }",
        "function baz() { var z = 1;            }",
    );
}

#[test]
fn test_inline_in_function2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function baz() {
  var a = new obj();
  result = a;
}
"#,
        r#"function baz() {
  result = new obj();
}
"#,
    );
}

#[test]
fn test_inline_in_function3() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function baz() {
  var a = new obj();
  (function(){a;})();
  result = a;
}
"#,
    );
}

#[test]
fn test_inline_in_function4() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function baz() {
  var a = new obj();
  foo.result = a;
}
"#,
    );
}

#[test]
fn test_inline_in_function5() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function baz() {
var a = (foo = new obj());
foo.x();
result = a;
}
"#,
    );
}

#[test]
fn test_inline_in_function6() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "function baz() { { var x = 1; var z = x; } }",
        "function baz() { { var z = 1;            } }",
    );
}

#[test]
fn test_inline_in_function7() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "function baz() { var x = 1; { var z = x; } }",
        "function baz() {            { var z = 1; } }",
    );
}

#[test]
fn test_inline_into_arrow_function1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = 0; var f = () => x + 1;",
        "           var f = () => 0 + 1;",
    );
}

#[test]
fn test_inline_into_arrow_function2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = 0; var f = () => { return x + 1; }",
        "           var f = () => { return 0 + 1; }",
    );
}

#[test]
fn test_inline_across_chunks() {
    let mut t = InlineVariablesTest::set_up();
    // TODO(kushal): Make decision about overlap with CrossChunkCodeMotion
    t.test_parts(vec![
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 2;")
                .add_chunk("var b = a;")
                .build(),
        ),
        expected_n(&["", "var b = 2;"]),
    ]);
}

#[test]
fn test_inlining_across_sibling_chunk_assignment_chunk_is_first() {
    let mut t = InlineVariablesTest::set_up();
    t.test_parts(vec![
        srcs_chunks(
            JSChunkGraphBuilder::for_tree()
                .add_chunk("var a;")
                .add_chunk("a = true;")
                .add_chunk("function foo() { if (a) console.log('test'); } foo();")
                .build(),
        ),
        expected_n(&[
            "var a;",
            "a = true;",
            "(function() { if (a) console.log('test'); })();",
        ]),
    ]);
    // child 2
}

#[test]
fn test_inlining_across_sibling_chunk_use_chunk_is_first() {
    let mut t = InlineVariablesTest::set_up();
    t.test_parts(vec![
        srcs_chunks(
            JSChunkGraphBuilder::for_tree()
                .add_chunk("var a;")
                .add_chunk("function foo() { if (a) console.log('test'); } foo();")
                .add_chunk("a = true;")
                .build(),
        ),
        expected_n(&[
            "var a;",
            "(function() { if (a) console.log('test'); })();",
            "a = true;",
        ]),
    ]);
    // child 2
}

#[test]
fn test_do_not_exit_conditional1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "if (true) { var x = 1; } var z = x; use(z);",
        "if (true) { var x = 1; }            use(x);",
    );
}

#[test]
fn test_do_not_exit_conditional2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "if (true) var x = 1; var z = x; use(z);",
        "if (true) var x = 1;            use(x);",
    );
}

#[test]
fn test_do_not_exit_conditional3() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x; if (true) x=1; var z = x; use(z);",
        "var x; if (true) x=1;            use(x);",
    );
}

#[test]
fn test_do_not_exit_loop() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "while (z) { var x = 3; } var y = x; use(y);",
        "while (z) { var x = 3; }            use(x);",
    );
}

#[test]
fn test_do_not_exit_for_loop() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "for (var i = 1; false; false) var z = i;",
        "for (         ; false; false) var z = 1;",
    );
    t.test_same("for (; false; false) var i = 1; var z = i;");
    t.test_same("for (var i in {}); var z = i;");
}

#[test]
fn test_const_in_loop_assigned_once_in_lifetime() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"for (; z; ) {
  const x = 1;
  use(x);
}
"#,
        r#"for (; z; ) {
  use(1);
}
"#,
    );
    t.test(
        r#"function foo(arg) {
  for (; z; ) {
    const alias = arg;
    use(alias);
    use2(alias);
  }
}
"#,
        r#"function foo(arg) {
  for (; z; ) {
    use(arg);
    use2(arg);
  }
}
"#,
    );
}

#[test]
fn test_inline_subscope_alias() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var x = function() {
  var self = this;
  return function() {
    var y = self;
    use(y);
  }
}
"#,
        r#"var x = function() {
  var self = this;
  return function() {

    use(self);
  }
}
"#,
    );
    t.test(
        r#"var x = function() {
  var y = [1];
  return function() {
    var z = y;
    use(z);
  };
}
"#,
        r#"var x = function() {
  var y = [1];
  return function() {

    use(y);
  };
}
"#,
    );
    t.test(
        r#"var x = function() {
  var y = 1;
  return function() {
    var z = y;
    use(z);
  };
}
"#,
        r#"var x = function() {

  return function() {

    use(1);
  };
}
"#,
    );
}

#[test]
fn test_do_not_exit_try() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("try { var x = y; } catch (e) {} var z = y; ");
    t.test_same("try { throw e; var x = 1; } catch (e) {} var z = x; ");
}

#[test]
fn test_do_not_enter_catch() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("try { } catch (e) { var z = e; } ");
}

#[test]
fn test_do_not_enter_finally() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"try { throw e; var x = 1; } catch (e) {}
finally  { var z = x; use(z); }
"#,
        r#"try { throw e; var x = 1; } catch (e) {}
finally  {            use(x); }
"#,
    );
}

#[test]
fn test_inside_if_conditional() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var a = foo(); if (a) { alert(3); }",
        "if (foo()) { alert(3); }",
    );
    t.test(
        "var a; a = foo(); if (a) { alert(3); }",
        "if (foo()) { alert(3); }",
    );
}

#[test]
fn test_only_read_at_initialization() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var a; a = foo();", "foo();");
    t.test(
        "var a; if (a = foo()) { alert(3); }",
        "if (foo()) { alert(3); }",
    );
    t.test("var a; switch (a = foo()) {}", "switch(foo()) {}");
    t.test(
        "var a; function f(){ return a = foo(); }",
        "       function f(){ return     foo(); }",
    );
    t.test(
        "function f(){ var a; return a = foo(); }",
        "function f(){        return     foo(); }",
    );
    t.test(
        "var a; with (a = foo()) { alert(3); }",
        "with (foo()) { alert(3); }",
    );
    t.test("var a; b = (a = foo());", "b = foo();");
    t.test(
        "var a; while(a = foo()) { alert(3); }",
        "       while(    foo()) { alert(3); }",
    );
    t.test(
        "var a; for(;a = foo();) { alert(3); }",
        "       for(;    foo();) { alert(3); }",
    );
    t.test(
        "var a; do {} while(a = foo()) { alert(3); }",
        "       do {} while(    foo()) { alert(3); }",
    );
}

#[test]
fn test_immutable_with_single_reference_after_initialzation() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var a; a = 1;", "1;");
    t.test("var a; if (a = 1) { alert(3); }", "if (1) { alert(3); }");
    t.test("var a; switch (a = 1) {}", "switch(1) {}");
    t.test(
        "var a; function f(){ return a = 1; }",
        "       function f(){ return 1; }",
    );
    t.test(
        "function f(){ var a; return a = 1; }",
        "function f(){        return     1; }",
    );
    t.test(
        "var a; with (a = 1) { alert(3); }",
        "with (1) { alert(3); }",
    );
    t.test("var a; b = (a = 1);", "b = 1;");
    t.test(
        "var a; while(a = 1) { alert(3); }",
        "       while(    1) { alert(3); }",
    );
    t.test(
        "var a; for(;a = 1;) { alert(3); }",
        "       for(;    1;) { alert(3); }",
    );
    t.test(
        "var a; do {} while(a = 1) { alert(3); }",
        "       do {} while(    1) { alert(3); }",
    );
}

#[test]
fn test_single_reference_after_initialzation() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var a; a = foo();a;", "foo();");
    t.test_same("var a; if (a = foo()) { alert(3); } a;");
    t.test_same("var a; switch (a = foo()) {} a;");
    t.test_same("var a; function f(){ return a = foo(); } a;");
    t.test_same("function f(){ var a; return a = foo(); a;}");
    t.test_same("var a; with (a = foo()) { alert(3); } a;");
    t.test_same("var a; b = (a = foo()); a;");
    t.test_same("var a; while(a = foo()) { alert(3); } a;");
    t.test_same("var a; for(;a = foo();) { alert(3); } a;");
    t.test_same("var a; do {} while(a = foo()) { alert(3); } a;");
}

#[test]
fn test_inside_if_branch() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var a = foo(); if (1) { alert(a); }");
}

#[test]
fn test_inside_and_conditional() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var a = foo(); a && alert(3);", "foo() && alert(3);");
}

#[test]
fn test_inside_and_branch() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var a = foo(); 1 && alert(a);");
}

#[test]
fn test_inside_or_branch() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var a = foo(); 1 || alert(a);");
}

#[test]
fn test_inside_hook_branch() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var a = foo(); 1 ? alert(a) : alert(3)");
}

#[test]
fn test_inside_hook_conditional() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var a = foo(); a ? alert(1) : alert(3)",
        "        foo()    ? alert(1) : alert(3)",
    );
}

#[test]
fn test_inside_or_branch_inside_if_conditional() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var a = foo(); if (x || a) {}");
}

#[test]
fn test_inside_or_branch_inside_if_conditional_with_constant() {
    let mut t = InlineVariablesTest::set_up();
    // We don't inline non-immutable constants into branches.
    t.test_same("var a = [false]; if (x || a) {}");
}

#[test]
fn test_cross_functions_as_left_leaves() {
    let mut t = InlineVariablesTest::set_up();
    // Ensures getNext() understands how to walk past a function leaf
    t.test_parts(vec![
        srcs_n(&["var x = function() {};", "", "function cow() {} var z = x;"]),
        expected_n(&["", "", "function cow() {} var z = function() {};"]),
    ]);
    t.test_parts(vec![
        srcs_n(&[
            "var x = function() {};",
            "",
            "var cow = function() {}; var z = x;",
        ]),
        expected_n(&["", "", "var cow = function() {}; var z = function() {};"]),
    ]);
    t.test(
        "var x = a; (function() { a++; })(); var z = x; use(z);",
        "var x = a; (function() { a++; })();            use(x);",
    );
    t.test(
        r#"var x = a;
function cow(){ a++; }
cow();
var z = x;
"#,
        r#"var x = a;
 // declaration removed
(function(){ a++; })();
 // unused `z` removed
"#,
    );
    t.test(
        "var x = a; cow(); var z = x; z; function cow() { a++; };",
        "var x = a; cow();            x; function cow() { a++; };",
    );
}

#[test]
fn test_do_cross_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = 1; foo(); var z = x; use(z);",
        "           foo();            use(1);",
    );
}

#[test]
fn test_do_not_cross_referencing_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var f = function() { var z = x; };
var x = 1;
f();
var z = x;
use(z)
f();
"#,
        r#"var f = function() { var z$jscomp$1 = x; };
var x = 1;
f();

use(x)
f();
"#,
    );
}

#[test]
fn test_chained_assignment() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var a = 2, b = 2; var c = b;", "var a = 2; var c = 2;");
    t.test("var a = 2, b = 2; var c = a;", "var b = 2; var c = 2;");
    t.test(
        "var a = b = 2; var f = 3; var c = a;",
        "var f = 3; var c = b = 2;",
    );
    t.test_same("var a = b = 2; var c = b;");
}

#[test]
fn test_for_in() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("for (var i in j) { var c = i; }");
    t.test_same("var i = 0; for (i in j) ;");
    t.test_same("var i = 0; for (i in j) { var c = i; }");
    t.test_same("i = 0; for (var i in j) { var c = i; }");
    t.test_same("var j = {'key':'value'}; for (var i in j) {print(i)};");
}

#[test]
fn test_do_cross_new_variables() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x = foo(); var z = x;", "var z = foo();");
}

#[test]
fn test_alias_after_function_call() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = foo(); bar(); var z = x; use(z);",
        "var x = foo(); bar();            use(x);",
    );
}

#[test]
fn test_do_not_cross_assignment() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = {}; var y = x.a; x.a = 1; var z = y; use(z);",
        "var x = {}; var y = x.a; x.a = 1;            use(y);",
    );
    t.test_same("var a = this.id; foo(this.id = 3, a);");
}

#[test]
fn test_alias_after_delete() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = {}; var y = x.a; delete x.a; var z = y; use(z);",
        "var x = {}; var y = x.a; delete x.a;            use(y);",
    );
}

#[test]
fn test_inline_alias_but_not_snapshot() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var b = 1; var a = b; b += 2; var c = a; use(c);",
        "var b = 1; var a = b; b += 2;            use(a);",
    );
}

#[test]
fn test_alias_after_increment() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var a = b.c; b.c++; var d = a; use(d);",
        "var a = b.c; b.c++;            use(a);",
    );
}

#[test]
fn test_constructor_before_alias() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var a = b; new Foo(); var c = a; use(c);",
        "var a = b; new Foo();            use(a);",
    );
}

#[test]
fn test_do_not_inline_alias_assigned_before_aliased_var() {
    let mut t = InlineVariablesTest::set_up();
    // TODO(bradfordcsmith): This is consistent with past behavior, but it would be good to fix it.
    // Assumes we do not rely on undefined variables (not technically correct!)
    t.test(
        "var a = b; var b = 3; alert(a)",
        "           var b = 3; alert(b);",
    );
    t.test(
        r#"f();
function f() {
  var alias = original;
  alert(alias)
}
var original = 3;
"#,
        r#"f();
function f() {

  alert(original)
}
var original = 3;
"#,
    );
    t.test(
        r#"f();
var original = 3;
function f() {
  var alias = original;
  alert(alias)
}
"#,
        r#"f();
var original = 3;
function f() {

  alert(original)
}
"#,
    );
}

#[test]
fn test_overlapping_inlines() {
    let mut t = InlineVariablesTest::set_up();
    let source = r#"a = function(el, x, opt_y) {
  var cur = bar(el);
  opt_y = x.y;
  x = x.x;
  var dx = x - cur.x;
  var dy = opt_y - cur.y;
  foo(
      el,
      el.offsetLeft + dx,
      el.offsetTop + dy);
};
"#;
    let expected = r#"a = function(el, x, opt_y) {
  var cur = bar(el);
  opt_y = x.y;
  x = x.x;
  foo(
      el,
      el.offsetLeft + (x - cur.x),
      el.offsetTop + (opt_y - cur.y));
};
"#;
    t.test(source, expected);
}

#[test]
fn test_overlapping_inline_functions() {
    let mut t = InlineVariablesTest::set_up();
    let source = r#"a = function() {
  var b = function(args) {var n;};
  var c = function(args) {};
  d(b,c);
};
"#;
    let expected = r#"a = function() {
  d(function(args){var n;}, function(args){});
};
"#;
    t.test(source, expected);
}

#[test]
fn test_inline_into_loops() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = true; while (true) alert(   x);",
        "              while (true) alert(true);",
    );
    t.test(
        "var x = true; while (true) for (var i in {}) alert(   x);",
        "              while (true) for (var i in {}) alert(true);",
    );
    t.test_same("var x = [true]; while (true) alert(x);");
}

#[test]
fn test_inline_into_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = false; var f = function() { alert(    x); };",
        "               var f = function() { alert(false); };",
    );
    t.test_same("var x = [false]; var f = function() { alert(x); };");
}

#[test]
fn test_no_inline_into_named_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("f(); var x = false; function f() { alert(x); };");
}

#[test]
fn test_inline_into_nested_non_hoisted_named_functions() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "f(); var x = false; if (false) function f() { alert(    x); };",
        "f();                if (false) function f() { alert(false); };",
    );
}

#[test]
fn test_no_inline_into_nested_named_functions() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("f(); var x = false; function f() { if (false) { alert(x); } };");
}

#[test]
fn test_no_inline_mutated_variable() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = false; if (true) { var y = x; x = true; }");
}

#[test]
fn test_inline_immutable_multiple_times() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = null; var y =    x, z =    x;",
        "              var y = null, z = null;",
    );
    t.test(
        "var x = 3; var y = x, z = x;",
        "           var y = 3, z = 3;",
    );
}

#[test]
fn test_inline_string_multiple_times_all_strings() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var x = 'abcdefghijklmnopqrstuvwxyz';
var y = x, z = x;
"#,
        r#"var y = 'abcdefghijklmnopqrstuvwxyz',
    z = 'abcdefghijklmnopqrstuvwxyz';
"#,
    );
}

#[test]
fn test_no_inline_backwards() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var y = x; var x = null;");
}

#[test]
fn test_no_inline_out_of_branch() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("if (true) var x = null; var y = x;");
}

#[test]
fn test_interfering_inlines() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var a = 3; var f = function() { var x = a; alert(x); };",
        "           var f = function() {            alert(3); };",
    );
}

#[test]
fn test_inline_into_try_catch() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var a = true;
try { var b = a; }
catch (e) { var c = a + b; var d = true; }
finally { var f = a + b + c + d; }
"#,
        r#"try { var b = true; }
catch (e) { var c = true + b; var d = true; }
finally { var f = true + b + c + d; }
"#,
    );
}

#[test]
fn test_inline_constants() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "function foo() { return  XXX; } var XXX = true;",
        "function foo() { return true; }                ",
    );
}

#[test]
fn test_inline_string_when_worthwhile() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x = 'a'; foo(x, x, x);", "foo('a', 'a', 'a');");
}

#[test]
fn test_inline_constant_alias() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var XXX = new Foo(); q(XXX); var YYY = XXX; bar(YYY)",
        "var XXX = new Foo(); q(XXX);                bar(XXX)",
    );
}

#[test]
fn test_inline_constant_alias_with_annotation() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "/** @const */ var xxx = new Foo(); q(xxx); var YYY = xxx; bar(YYY)",
        "/** @const */ var xxx = new Foo(); q(xxx);                bar(xxx)",
    );
}

#[test]
fn test_inline_constant_alias_with_non_constant() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var XXX = new Foo(); q(XXX); var y = XXX; bar(  y); baz(  y)",
        "var XXX = new Foo(); q(XXX);              bar(XXX); baz(XXX)",
    );
}

#[test]
fn test_cascading_inlines() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var XXX = 4;
function f() {
  var YYY = XXX;
  bar(YYY);
  baz(YYY);
}
"#,
        r#"function f() {
  bar(4);
  baz(4);
}
"#,
    );
}

#[test]
fn test_no_inline_getprop_into_call() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var a = b; a();", "b();");
    t.test("var a = b.c; f(a);", "f(b.c);");
    t.test_same("var a = b.c; a();");
}

#[test]
fn test_inline_function_declaration() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var f = function () {}; var a =              f;",
        "                        var a = function () {};",
    );
    t.test(
        "var f = function () {}; foo(); var a =              f;",
        "                        foo(); var a = function () {};",
    );
    t.test(
        "var f = function () {}; foo(             f);",
        "                        foo(function () {});",
    );
    t.test(
        "var f = function () {}; function g() {var a = f; return a;}",
        "var f = function () {}; function g() {           return f;}",
    );
    t.test_same("var f = function () {}; function g() {h(f);}");
}

#[test]
fn test2388531() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var f = function () {};
var g = function () {};
goog.inherits(f, g);
"#,
    );
    t.test_same(
        r#"var f = function () {};
var g = function () {};
goog$inherits(f, g);
"#,
    );
}

#[test]
fn test_recursive_function1() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = 0; (function x() { return x ? x() : 3; })();");
}

#[test]
fn test_recursive_function2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("function y() { return y(); }");
}

#[test]
fn test_unreferenced_bleeding_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = function y() {}");
}

#[test]
fn test_referenced_bleeding_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = function y() { return y(); }");
}

#[test]
fn test_inline_aliases1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = this.foo(); this.bar(); var y = x; this.baz(y);",
        "var x = this.foo(); this.bar();            this.baz(x);",
    );
}

#[test]
fn test_inline_aliases1b() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = this.foo(); this.bar(); var y; y = x; this.baz(y);",
        "var x = this.foo(); this.bar();            x; this.baz(x);",
    );
}

#[test]
fn test_inline_aliases1c() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x; x = this.foo(); this.bar(); var y = x; this.baz(y);",
        "var x; x = this.foo(); this.bar();            this.baz(x);",
    );
}

#[test]
fn test_inline_aliases1d() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x; x = this.foo(); this.bar(); var y; y = x; this.baz(y);",
        "var x; x = this.foo(); this.bar();            x; this.baz(x);",
    );
}

#[test]
fn test_inline_aliases2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var x = this.foo();
this.bar();
function f() {
  var y = x;
  this.baz(y);
}
"#,
        r#"var x = this.foo();
this.bar();
function f() {
  this.baz(x);
}
"#,
    );
}

#[test]
fn test_inline_aliases2b() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var x = this.foo();
this.bar();
function f() {
  var y;
  y = x;
  this.baz(y);
}
"#,
        r#"var x = this.foo();
this.bar();
function f() {

  x;
  this.baz(x);
}
"#,
    );
}

#[test]
fn test_inline_aliases2c() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var x;
x = this.foo();
this.bar();
function f() {
  var y = x;
  this.baz(y);
}
"#,
        r#"var x;
x = this.foo();
this.bar();
function f() {
  this.baz(x);
}
"#,
    );
}

#[test]
fn test_inline_aliases2d() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var x;
x = this.foo();
this.bar();
function f() {
  var y;
  y = x;
  this.baz(y);
}
"#,
        r#"var x;
x = this.foo();
this.bar();
function f() {

  x;
  this.baz(x);
}
"#,
    );
}

#[test]
fn test_inline_aliases_in_loop() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f() {
  var x = extern();
  for (var i = 0; i < 5; i++) {
    (function() {
       var y = x; window.setTimeout(function() { extern(y); }, 0);
     })();
  }
}
"#,
        r#"function f() {
  var x = extern();
  for (var i = 0; i < 5; i++) {
    (function() {
       window.setTimeout(function() { extern(x); }, 0);
     })();
  }
}
"#,
    );
}

#[test]
fn test_no_inline_aliases_in_loop() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
  for (var i = 0; i < 5; i++) {
    var x = extern();
    (function() {
       var y = x; window.setTimeout(function() { extern(y); }, 0);
     })();
  }
}
"#,
    );
}

#[test]
fn test_no_inline_aliases1() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = this.foo(); this.bar(); var y = x; x = 3; this.baz(y);");
}

#[test]
fn test_no_inline_aliases1b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = this.foo(); this.bar(); var y; y = x; x = 3; this.baz(y);");
}

#[test]
fn test_no_inline_aliases2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = this.foo(); this.bar(); var y = x; y = 3; this.baz(y); ");
}

#[test]
fn test_no_inline_aliases2b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x = this.foo(); this.bar(); var y; y = x; y = 3; this.baz(y); ");
}

#[test]
fn test_no_inline_aliases3() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
function f() {
  var y = x;
  g();
  this.baz(y);
}
function g() {
  x = 3;
}
"#,
    );
}

#[test]
fn test_no_inline_aliases3b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();

function f() {
var y;
y = x;
g();
this.baz(y);
}
function g() {
x = 3;
}
"#,
    );
}

#[test]
fn test_no_inline_aliases4() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
function f() {
  var y = x;
  y = 3;
  this.baz(y);
}
"#,
    );
}

#[test]
fn test_no_inline_aliases4b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
function f() {
  var y;
  y = x;
  y = 3;
  this.baz(y);
}
"#,
    );
}

#[test]
fn test_no_inline_aliases5() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
var y = x;
this.bing();
this.baz(y);
x = 3;
"#,
    );
}

#[test]
fn test_no_inline_aliases5b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
var y;
y = x;
this.bing();
this.baz(y);
x = 3;
"#,
    );
}

#[test]
fn test_no_inline_aliases6() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
var y = x;
this.bing();
this.baz(y);
y = 3;
"#,
    );
}

#[test]
fn test_no_inline_aliases6b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
var y;
y = x;
this.bing();
this.baz(y);
y = 3;
"#,
    );
}

#[test]
fn test_no_inline_aliases7() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
function f() {
  var y = x;
  this.bing();
  this.baz(y);
  x = 3;
}
"#,
    );
}

#[test]
fn test_no_inline_aliases7b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
function f() {
  var y;
  y = x;
  this.bing();
  this.baz(y);
  x = 3;
}
"#,
    );
}

#[test]
fn test_no_inline_aliases8() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
function f() {
  var y = x;
  this.baz(y);
  y = 3;
}
"#,
    );
}

#[test]
fn test_no_inline_aliases8b() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x = this.foo();
this.bar();
function f() {
  var y;
  y = x;
  this.baz(y);
  y = 3;
}
"#,
    );
}

#[test]
fn test_side_effect_order() {
    let mut t = InlineVariablesTest::set_up();
    // z can not be changed by the call to y, so x can be inlined.
    let externs_code = "var z; function f(){}";
    t.test_parts(vec![
        externs(externs_code),
        srcs("var x = f(y.a, y); z =          x"),
        expected("               z = f(y.a, y);"),
    ]);
    // z.b can be changed by the call to y, so x can not be inlined.
    t.test_same_parts(vec![
        externs(externs_code),
        srcs("var x = f(y.a, y); z.b = x;"),
    ]);
}

#[test]
fn test_inline_parameter_alias1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x) {
  var y = x;
  g();
  y;y;
}
"#,
        r#"function f(x) {
  g();
  x;x;
}
"#,
    );
}

#[test]
fn test_inline_parameter_alias2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x) {
  var y; y = x;
  g();
  y;y;
}
"#,
        r#"function f(x) {
  x;
  g();
  x;x;
}
"#,
    );
}

#[test]
fn test_inline_function_alias1a() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x) {}
var y = f;
g();
y();y();
"#,
        r#"var y = function(x) {};
g();
y();y();
"#,
    );
}

#[test]
fn test_inline_function_alias1b() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x) {};
f;var y = f;
g();
y();y();
"#,
        r#"function f(x) {};
f;g();
f();f();
"#,
    );
}

#[test]
fn test_inline_function_alias2a() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x) {}
var y; y = f;
g();
y();y();
"#,
        r#"var y; y = function(x) {};
g();
y();y();
"#,
    );
}

#[test]
fn test_inline_function_alias2b() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x) {};
f; var y; y = f;
g();
y();y();
"#,
        r#"function f(x) {};
f; f;
g();
f();f();
"#,
    );
}

#[test]
fn test_inline_switch_var() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x = y; switch (x) {}", "           switch (y) {}");
}

#[test]
fn test_inline_switch_let() {
    let mut t = InlineVariablesTest::set_up();
    t.test("let x = y; switch (x) {}", "           switch (y) {}");
}

#[test]
fn test_inline_into_for_loop1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function calculate_hashCode() {
  var values = [1, 2, 3, 4, 5];
  var hashCode = 1;
  for (var $array = values, i = 0; i < $array.length; i++) {
    var e = $array[i];
    hashCode = 31 * hashCode + calculate_hashCode(e);
  }
  return hashCode;
}
"#,
        r#"function calculate_hashCode() {
  var hashCode = 1;
  var $array = [1, 2, 3, 4, 5];
  var i = 0;
  for (; i < $array.length; i++) {
    hashCode = 31 * hashCode + calculate_hashCode($array[i]);
  }
  return hashCode;
}
"#,
    );
}

#[test]
fn test_inline_into_for_loop2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function calculate_hashCode() {
  let values = [1, 2, 3, 4, 5];
  let hashCode = 1;
  for (let $array = values, i = 0; i < $array.length; i++) {
    let e = $array[i];
    hashCode = 31 * hashCode + calculate_hashCode(e);
  }
  return hashCode;
}
"#,
        r#"function calculate_hashCode() {
  let values = [1, 2, 3, 4, 5];
  let hashCode = 1;
  for (let $array = values, i = 0; i < $array.length; i++) {
    hashCode = 31 * hashCode + calculate_hashCode($array[i]);
  }
  return hashCode;
}
"#,
    );
}

#[test]
fn test_no_inline_catch_alias_var1() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"try {
} catch (e) {
  var y = e;
  g();
  y;y;
}
"#,
    );
}

#[test]
fn test_no_inline_catch_alias_var2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"try {
} catch (e) {
  var y; y = e;
  g();
  y;y;
}
"#,
    );
}

#[test]
fn test_inline_catch_alias_let1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"try {
} catch (e) {
  let y = e;
  g();
  y;y;
}
"#,
        r#"try {
} catch (e) {
  g();
  e;e;
}
"#,
    );
}

#[test]
fn test_inline_catch_alias_let2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"try {
} catch (e) {
  let y; y = e;
  g();
  y;y;
}
"#,
        r#"try {
} catch (e) {
  e;
  g();
  e;e;
}
"#,
    );
}

#[test]
fn test_inline_this() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"/** @constructor */
function C() {}

C.prototype.m = function() {
  var self = this;
  if (true) {
    alert(self);
  }
};
"#,
        r#"(/** @constructor */
function() {}).prototype.m = function() {
  if (true) {
    alert(this);
  }
};
"#,
    );
}

#[test]
fn test_var_in_block1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "function f(x) { if (true) {var y = x; y; y;} }",
        "function f(x) { if (true) {           x; x;} }",
    );
}

#[test]
fn test_var_in_block2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "function f(x) { switch (0) { case 0: { var y = x; y; y; } } }",
        "function f(x) { switch (0) { case 0: { x; x; } } }",
    );
}

#[test]
fn test_locals_only1() {
    let mut t = InlineVariablesTest::set_up();
    t.hooks.inline_locals_only = true;
    t.test(
        "var x=1; x; function f() {var x = 1; x;}",
        "var x=1; x; function f() {           1;}",
    );
}

#[test]
fn test_locals_only2() {
    let mut t = InlineVariablesTest::set_up();
    t.hooks.inline_locals_only = true;
    t.test(
        r#"/** @const */
var X=1;
X;
function f() {
  /** @const */
  var X = 1; X;
}
"#,
        r#"/** @const */
var X=1;
X;
function f() {
  1;
}
"#,
    );
}

#[test]
fn test_inline_undefined1() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x; x;", "void 0;");
}

#[test]
fn test_inline_undefined2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x; x++;");
}

#[test]
fn test_inline_undefined3() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x; var x;");
}

#[test]
fn test_inline_undefined4() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x; x; x;", "void 0; void 0;");
}

#[test]
fn test_inline_undefined5() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var x; for(x in a) {}");
}

#[test]
fn test_issue90() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var x;      x && alert(1)", "       void 0 && alert(1)");
}

#[test]
fn test_rename_property_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var JSCompiler_renameProperty;
JSCompiler_renameProperty('foo')
"#,
    );
}

#[test]
fn test_this_alias() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "function f() { var a = this;    a.y();    a.z(); }",
        "function f() {               this.y(); this.z(); }",
    );
}

#[test]
fn test_this_escaped_alias() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("function f() { var a = this; var g = function() { a.y(); }; a.z(); }");
}

#[test]
fn test_this_alias_class_member_field_def() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
  var self = this;
  use(self);
  class H {
    owner = self;
  }
}
"#,
    );
}

#[test]
fn test_this_alias_class_static_member_field_def() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
  var self = this;
  use(self);
  class H {
    static owner = self;
  }
}
"#,
    );
}

#[test]
fn test_this_alias_class_computed_field_def_value() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
  var self = this;
  use(self);
  class H {
    ['owner'] = self;
  }
}
"#,
    );
}

#[test]
fn test_this_alias_class_computed_field_def_key() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f() {
  var self = this;
  use(self);
  class H {
    [self] = 1;
  }
}
"#,
        r#"function f() {
  use(this);
  class H {
    [this] = 1;
  }
}
"#,
    );
}

#[test]
fn test_this_alias_class_static_block() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
  var self = this;
  use(self);
  class H {
    static {
      use(self);
    }
  }
}
"#,
    );
}

#[test]
fn test_this_alias_global_class_member_field_def() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var self = this;
use(self);
class H {
  owner = self;
}
"#,
    );
}

#[test]
fn test_inline_named_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test("function f() {} f();", "(function(){})()");
}

#[test]
fn test_issue378_modified_arguments1() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function g(callback) {
  var f = callback;
  arguments[0] = this;
  f.apply(this, arguments);
}
"#,
    );
}

#[test]
fn test_issue378_modified_arguments2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function g(callback) {
  /** @const */
  var f = callback;
  arguments[0] = this;
  f.apply(this, arguments);
}
"#,
    );
}

#[test]
fn test_issue378_escaped_arguments1() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function g(callback) {
  var f = callback;
  h(arguments,this);
  f.apply(this, arguments);
}
function h(a,b) {
  a[0] = b;
}
"#,
    );
}

#[test]
fn test_issue378_escaped_arguments2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function g(callback) {
  /** @const */
  var f = callback;
  h(arguments,this);
  f.apply(this);
}
function h(a,b) {
  a[0] = b;
}
"#,
    );
}

#[test]
fn test_issue378_escaped_arguments3() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function g(callback) {
  var f = callback;
  f.apply(this, arguments);
}
"#,
        r#"function g(callback) {

  callback.apply(this, arguments);
}
"#,
    );
}

#[test]
fn test_issue378_escaped_arguments4() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function g(callback) {
  var f = callback;
  h(arguments[0],this);
  f.apply(this, arguments);
}
function h(a,b) {
  a[0] = b;
}
"#,
        r#"function g(callback) {

  h(arguments[0],this);
  callback.apply(this, arguments);
}
function h(a,b) {
  a[0] = b;
}
"#,
    );
}

#[test]
fn test_issue378_arguments_read1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function g(callback) {
  var f = callback;
  var g = arguments[0];
  f.apply(this, arguments);
}
"#,
        r#"function g(callback) {

  var g = arguments[0];
  callback.apply(this, arguments);
}
"#,
    );
}

#[test]
fn test_issue378_arguments_read2() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function g(callback) {
  var f = callback;
  h(arguments[0],this);
  f.apply(this, arguments[0]);
}
function h(a,b) {
  a[0] = b;
}
"#,
        r#"function g(callback) {
  h(arguments[0],this);
  callback.apply(this, arguments[0]);
}
function h(a,b) {
  a[0] = b;
}
"#,
    );
}

#[test]
fn test_arguments_modified_in_outer_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function g(callback) {
  var f = callback;
  arguments[0] = this;
  f.apply(this, arguments);
  function inner(callback) {
    var x = callback;
    x.apply(this);
  }
}
"#,
        r#"function g(callback) {
  var f = callback;
  arguments[0] = this;
  f.apply(this, arguments);
  function inner(callback) {
    callback.apply(this);
  }
}
"#,
    );
}

#[test]
fn test_arguments_modified_in_inner_function() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function g(callback) {
  var f = callback;
  f.apply(this, arguments);
  function inner(callback) {
    var x = callback;
    arguments[0] = this;
    x.apply(this);
  }
}
"#,
        r#"function g(callback) {
  callback.apply(this, arguments);
  function inner(callback) {
    var x = callback;
    arguments[0] = this;
    x.apply(this);
  }
}
"#,
    );
}

#[test]
fn test_no_inline_redeclared_externs() {
    let mut t = InlineVariablesTest::set_up();
    let externs_code = "var test = 1;";
    let code = "/** @suppress {duplicate} */ var test = 2;alert(test);";
    t.test_same_parts(vec![externs(externs_code), srcs(code)]);
}

#[test]
fn test_bug6598844() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function F() { this.a = 0; }
F.prototype.inc = function() { this.a++; return 10; };
F.prototype.bar = function() { var x = this.inc(); this.a += x; };
"#,
    );
}

#[test]
fn test_external_issue1053() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("var u; function f() { u = Random(); var x = u; f(); alert(x===u)}");
}

#[test]
fn test_hoisted_function1() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var x = 1; function f() { return x; }",
        "function f() { return 1; }",
    );
}

#[test]
fn test_hoisted_function2() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var impl_0;
b(a());
function a() { impl_0 = {}; }
function b() { window['f'] = impl_0; }
"#,
    );
}

#[test]
fn test_hoisted_function3() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var impl_0;
b();
impl_0 = 1;
function b() { window['f'] = impl_0; }
"#,
    );
}

#[test]
fn test_hoisted_function4() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var impl_0;
impl_0 = 1;
b();
function b() { window['f'] = impl_0; }
"#,
        r#"1;
b();
function b() { window['f'] = 1; }
"#,
    );
}

#[test]
fn test_hoisted_function5() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"a();
var debug = 1;
function b() { return debug; }
function a() { return b(); }
"#,
    );
}

#[test]
fn test_hoisted_function6() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var debug = 1;
a();
function b() { return debug; }
function a() { return b(); }
"#,
        r#"a();
function b() { return 1; }
function a() { return b(); }
"#,
    );
}

#[test]
fn test_issue354() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var enabled = true;
function Widget() {}
Widget.prototype = {
  frob: function() {
    search();
  }
};
function search() {
  if (enabled)
    alert(1);
  else
    alert(2);
}
window.foo = new Widget();
window.bar = search;
"#,
        r#"function Widget() {}
Widget.prototype = {
  frob: function() {
    search();
  }
};
function search() {
  if (true)
    alert(1);
  else
    alert(2);
}
window.foo = new Widget();
window.bar = search;
"#,
    );
}

#[test]
fn test_issue1177() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("function x_64(){var x_7;for(;;);var x_68=x_7=x_7;}");
    t.test_same("function x_64(){var x_7;for(;;);var x_68=x_7=x_7++;}");
    t.test_same("function x_64(){var x_7;for(;;);var x_68=x_7=x_7*2;}");
}

#[test]
fn test_switch_github_issue1234() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var x;
switch ('a') {
  case 'a':
    break;
  default:
    x = 1;
    break;
}
use(x);
"#,
    );
}

#[test]
fn test_late_guarded_assign() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"let x;
function f() {
  x === void 0 && (x = 1);
  const y = x;
  use(y);
}
"#,
        r#"let x;
function f() {
  x === void 0 && (x = 1);

  use(x);
}
"#,
    );
}

#[test]
fn test_let_const() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f(x) {
  if (true) {
    let y = x; y; y;
  }
}
"#,
        r#"function f(x) {
  if (true) {
    x; x;
  }
}
"#,
    );
    t.test(
        r#"function f(x) {
  if (true) {
    const y = x; y; y;
    }
  }
"#,
        r#"function f(x) {
  if (true) {
    x; x;
  }
}
"#,
    );
    t.test(
        r#"function f(x) {
  let y;
  {
    let y = x; y;
  }
}
"#,
        r#"function f(x) {
  let y;
  {
    x;
  }
}
"#,
    );
    t.test(
        r#"function f(x) {
  let y = x; y; const g = 2;
  {
    const g = 3; let y = g; y;
  }
}
"#,
        r#"function f(x) {
  x;
  {3;}
}
"#,
    );
}

#[test]
fn test_generators() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function* f() {
  let x = 1;
  yield x;
}
"#,
        r#"function* f() {
  yield 1;
}
"#,
    );
    t.test(
        r#"function* f(x) {
  let y = x++
  yield y;
}
"#,
        r#"function* f(x) {
  yield x++;
}
"#,
    );
}

#[test]
fn test_for_of() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(" var i = 0; for(i of n) {}");
    t.test_same("for( var i of n) { var x = i; }");
}

#[test]
fn test_template_strings() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        " var name = 'Foo'; `Hello ${name}`",
        "                   `Hello ${'Foo'}`",
    );
    t.test(
        "var name = 'Foo'; var foo = name; `Hello ${foo}`",
        "                                  `Hello ${'Foo'}`",
    );
    t.test("var age = 3; `Age: ${age}`", "             `Age: ${3}`");
}

#[test]
fn test_tagged_template_literals() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var name = 'Foo';
function myTag(strings, nameExp, numExp) {
  var modStr;
  if (numExp > 2) {
    modStr = nameExp + 'Bar'
  } else {
    modStr = nameExp + 'BarBar'
  }
}
var output = myTag`My name is ${name} ${3}`;
"#,
        r#"var output = function(strings, nameExp, numExp) {
  var modStr;
  if (numExp > 2) {
    modStr = nameExp + 'Bar'
  } else {
    modStr = nameExp + 'BarBar'
  }
}`My name is ${'Foo'} ${3}`;
"#,
    );
    t.test(
        r#"var name = 'Foo';
function myTag(strings, nameExp, numExp) {
  var modStr;
  if (numExp > 2) {
    modStr = nameExp + 'Bar'
  } else {
    modStr = nameExp + 'BarBar'
  }
}
var output = myTag`My name is ${name} ${3}`;
output = myTag`My name is ${name} ${2}`;
"#,
        r#"function myTag(strings, nameExp, numExp) {
  var modStr;
  if (numExp > 2) {
    modStr = nameExp + 'Bar'
  } else {
    modStr = nameExp + 'BarBar'
  }
}
var output = myTag`My name is ${'Foo'} ${3}`;
output = myTag`My name is ${'Foo'} ${2}`;
"#,
    );
}

#[test]
fn test_destructuring() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"var [a, b, c] = [1, 2, 3]
var x = a;
x; x;
"#,
        r#"var a;
var b;
var c;
[a, b, c] = [1, 2, 3]
var x = a; // we fail to inline a into x because normalization adds stub declaration
// for a. But that's an acceptable compromise.
x; x;
"#,
    );
    t.test_same("var x = 1; ({[0]: x} = {});");
}

#[test]
fn dont_inline_conditional_default_value_assignment() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same("let x; const {y = (x = 0)} = obj(); use(x);");
}

#[test]
fn test_function_inlined_across_script() {
    let mut t = InlineVariablesTest::set_up();
    let srcs = ["function f() {}", "use(f);"];
    let expected = ["", "use(function() {});"];
    t.test_parts(vec![srcs_n(&srcs), expected_n(&expected)]);
}

#[test]
fn test_function_var_inlining_elides_function_name() {
    let mut t = InlineVariablesTest::set_up();
    // When there's only one reference to a function name (e.g. in the rhs of an assignment), we
    // should elide that function name.
    t.test(
        "function notRecursive(x) { return 4 }; exports.y = notRecursive;",
        ";exports.y = function(x) { return 4; }",
    );
    // In this case, the function is recursive, so there's more than one reference, we should keep
    // the name.
    t.test_same(
        r#"function factorial(x) { if (x == 1) return 1; return x + factorial(x - 1); };
exports.x = factorial;
"#,
    );
}

#[test]
fn test_no_inline_past_optional_chain_get_prop() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f() {
  var t = this.consume();
  var ready = this?.ready;
  return t + ready;
}
"#,
        r#"function f() {
  var t = this.consume();
  return t + this?.ready;
}
"#,
    );
}

#[test]
fn test_no_inline_past_optional_chain_get_elem() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f() {
  var t = this.consume();
  var ready = this?.['ready'];
  return t + ready;
}
"#,
        r#"function f() {
  var t = this.consume();
  return t + this?.['ready'];
}
"#,
    );
}

#[test]
fn test_no_inline_past_get_prop() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f() {
  var t = this.consume();
  var ready = this.ready;
  return t + ready;
}
"#,
        r#"function f() {
  var t = this.consume();
  return t + this.ready;
}
"#,
    );
}

#[test]
fn test_no_inline_past_optional_chain_call() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"function f() {
  var t = this.consume();
  var ready = this?.ready();
  return t + ready;
}
"#,
    );
}

#[test]
fn test_cross_scope_alias_chain_with_intervening_reassignment() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        r#"function f() {
  var C = compute();
  var D = C;
  {
    let A = D;
    C = attacker();
    sink(A); sink(A);
  }
}
"#,
        r#"function f() {
  var C = compute();
  {
    let A = C;
    C = attacker();
    sink(A); sink(A);
  }
}
"#,
    );
}

#[test]
fn test_class_declaration_not_inlined() {
    let mut t = InlineVariablesTest::set_up();
    // When there's only one reference to a class name (e.g. in the rhs of an assignment), we
    // should elide that class name.
    t.test_same("class MyClass { foo() {} } ns.y = MyClass;");
}

#[test]
fn test_class_expression_inlined() {
    let mut t = InlineVariablesTest::set_up();
    t.test("var C = class {}; use(C);", "use(class {});");
}

#[test]
fn test_class_expression_with_extends_inlined() {
    let mut t = InlineVariablesTest::set_up();
    t.test(
        "var C = class extends Base { foo() {} }; use(C);",
        "use(class extends Base { foo() {} });",
    );
}

#[test]
fn test_class_with_side_effects_not_moved_across_calls() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var C = class { static { doSideEffects(); } };
doSomething();
use(C);
"#,
    );
}

#[test]
fn test_class_with_side_effecting_static_field_not_moved_across_calls() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var C = class { static x = doSideEffects(); };
doSomething();
use(C);
"#,
    );
}

#[test]
fn test_class_with_pure_static_field_inlined_across_calls() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var C = class { static x = 1; };
doSomething();
use(C);
"#,
    );
}

#[test]
fn test_class_with_impure_static_field_not_inlined_across_calls() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var C = class { static x = compute(); };
doSomething();
use(C);
"#,
    );
}

#[test]
fn test_class_with_side_effecting_computed_property_not_moved_across_calls() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var C = class { [computeKey()]() {} };
doSomething();
use(C);
"#,
    );
}

#[test]
fn test_class_with_instance_field_side_effects_can_move_before_instantiation() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var C = class { x = compute(); };
doSomething();
use(C);
"#,
    );
}

#[test]
fn test_class_with_impure_extends_not_moved_across_calls() {
    let mut t = InlineVariablesTest::set_up();
    t.test_same(
        r#"var C = class extends getBase() {};
doSomething();
use(C);
"#,
    );
}

// Rust-only (DECISIONS.md D-025): the second run of the pass on a compiler skips the externs when
// their NAMEs resolve only to vars declared in the externs, and traverses them otherwise.
#[test]
fn test_rerun_skips_externs_that_refer_only_to_externs() {
    let mut t = InlineVariablesTest::set_up();
    t.hooks.num_repetitions = 2;
    t.test_parts(vec![
        externs("var z; function f(a) { return a; } f.prototype.g = function(b) {};"),
        srcs("var x = f(); z = x;"),
        expected("z = f();"),
    ]);
}

#[test]
fn test_rerun_traverses_externs_that_refer_to_code() {
    let mut t = InlineVariablesTest::set_up();
    t.hooks.num_repetitions = 2;
    // The reference to x in the externs keeps x from being inlined on every run.
    t.test_same_parts(vec![
        externs("var z; function f() {} function g() { return x; }"),
        srcs("var x = f(); z = x;"),
    ]);
}
