/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/parsing/AttachJsdocsTest.java.

use closure_parsing::{
    config::{JsDocParsing, LanguageMode, RunMode, StrictMode},
    parser_runner::ParserRunner,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
    simple_source_file::SimpleSourceFile,
    static_source_file::SourceKind,
    testing::test_error_reporter::TestErrorReporter,
    token::Token,
};
use std::sync::Arc;
struct Harness {
    ast: Ast,
    mode: LanguageMode,
}
impl Harness {
    // port: AttachJsdocsTest#setUp
    fn new() -> Self {
        Self {
            ast: Ast::new(),
            mode: LanguageMode::ECMASCRIPT3,
        }
    }
    // port: AttachJsdocsTest#parse
    fn parse(&mut self, source: impl Into<JsString>, warnings: &[&str]) -> NodeId {
        let mut reporter = TestErrorReporter::new();
        reporter.expect_all_warnings(warnings);
        let config = ParserRunner::create_config_full(
            self.mode,
            JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
            RunMode::KEEP_GOING,
            None,
            true,
            StrictMode::SLOPPY,
        );
        let result = ParserRunner::parse(
            &mut self.ast,
            Arc::new(SimpleSourceFile::new("input", SourceKind::STRONG)),
            source.into(),
            &config,
            &mut reporter,
        );
        reporter.verify_has_encountered_all_warnings_and_errors();
        result.ast.unwrap()
    }
}

// port: AttachJsdocsTest#testOldJsdocAdd
#[test]
fn test_old_jsdoc_add() {
    let mut h = Harness::new();

    let root = h.parse("1 + /** attach */ value;", &[]);
    let plus = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        plus.get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocArrayLit
#[test]
fn test_old_jsdoc_array_lit() {
    let mut h = Harness::new();

    let root = h.parse("[1, /** attach */ 2]", &[]);
    let lit = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        lit.get_second_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocAssign1
#[test]
fn test_old_jsdoc_assign1() {
    let mut h = Harness::new();

    let root = h.parse("x = 1; /** attach */ y = 2;", &[]);
    let assign = root
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(assign.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocAssign2
#[test]
fn test_old_jsdoc_assign2() {
    let mut h = Harness::new();

    let root = h.parse("x = 1; /** attach */y.p = 2;", &[]);
    let assign = root
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(assign.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocAssign3
#[test]
fn test_old_jsdoc_assign3() {
    let mut h = Harness::new();

    let root = h.parse(
        "/** @const */ var g = {}; /** @type {number} */ (g.foo) = 3;",
        &[],
    );
    let assign = root
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(
        assign
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocBlock1
#[test]
fn test_old_jsdoc_block1() {
    let mut h = Harness::new();

    let root = h.parse("if (x) { /** attach */ x; }", &[]);
    let then_block = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap();
    assert!(
        then_block
            .get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocBlock2
#[test]
fn test_old_jsdoc_block2() {
    let mut h = Harness::new();

    let root = h.parse("if (x) { x; /** attach */ y; }", &[]);
    let then_block = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_second_child(&h.ast)
        .unwrap();
    assert!(
        then_block
            .get_last_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocBreak
#[test]
fn test_old_jsdoc_break() {
    let mut h = Harness::new();

    let root = h.parse("FOO: for (;;) { break /** don't attach */ FOO; }", &[]);
    let for_stm = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap();
    let break_stm = for_stm
        .get_child_at_index(&h.ast, 3)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert_eq!(break_stm.get_token(&h.ast), Token::BREAK);
    assert!(break_stm.get_jsdoc_info(&h.ast).is_none());
    assert!(
        break_stm
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
}

// port: AttachJsdocsTest#testOldJsdocCall2
#[test]
fn test_old_jsdoc_call2() {
    let mut h = Harness::new();

    let root = h.parse("foo(/** attach */ 1, 2);", &[]);
    let call = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        call.get_second_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocCall3
#[test]
fn test_old_jsdoc_call3() {
    let mut h = Harness::new();

    let root = h.parse("foo(1 /** attach to 2nd parameter */, 2);", &[]);
    let call = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        call.get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocCall4
#[test]
fn test_old_jsdoc_call4() {
    let mut h = Harness::new();

    let root = h.parse("foo(1, 2 /** don't attach */);", &[]);
    let call = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        call.get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
}

// port: AttachJsdocsTest#testOldJsdocCall5
#[test]
fn test_old_jsdoc_call5() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ x(); function f() {}", &[]);
    assert!(
        root.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocCall6
#[test]
fn test_old_jsdoc_call6() {
    let mut h = Harness::new();

    let root = h.parse("(function f() { /** attach */ var x = 1; })();", &[]);
    let func = root
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(func.is_function(&h.ast));
    assert!(
        func.get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocCall7
#[test]
fn test_old_jsdoc_call7() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ obj.prop();", &[]);
    assert!(
        root.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocCall8
#[test]
fn test_old_jsdoc_call8() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ (obj).prop();", &[]);
    let get_prop = root
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(get_prop.is_get_prop(&h.ast));
    assert!(
        get_prop
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocComma3
#[test]
fn test_old_jsdoc_comma3() {
    let mut h = Harness::new();

    let root = h.parse("(x, y, /** attach */ z);", &[]);
    let right_comma = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        right_comma
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocContinue
#[test]
fn test_old_jsdoc_continue() {
    let mut h = Harness::new();

    let root = h.parse("FOO: for (;;) { continue /** don't attach */ FOO; }", &[]);
    let for_stm = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap();
    let cont = for_stm
        .get_child_at_index(&h.ast, 3)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert_eq!(cont.get_token(&h.ast), Token::CONTINUE);
    assert!(cont.get_jsdoc_info(&h.ast).is_none());
    assert!(
        cont.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
}

// port: AttachJsdocsTest#testOldJsdocDot
#[test]
fn test_old_jsdoc_dot() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */a.b;", &[]);
    assert!(
        root.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocDot2
#[test]
fn test_old_jsdoc_dot2() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */\n// test\na.b = {};\n", &[]);
    assert!(
        root.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocForInLoop2
#[test]
fn test_old_jsdoc_for_in_loop2() {
    let mut h = Harness::new();

    let root = h.parse("for (/** attach */ var p in {}) {}", &[]);
    let fil = root.get_first_child(&h.ast).unwrap();
    assert!(fil.get_jsdoc_info(&h.ast).is_none());
    assert!(
        fil.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocForInLoop3
#[test]
fn test_old_jsdoc_for_in_loop3() {
    let mut h = Harness::new();

    let root = h.parse("for (var p in /** attach */ {}) {}", &[]);
    let fil = root.get_first_child(&h.ast).unwrap();
    assert!(fil.get_jsdoc_info(&h.ast).is_none());
    assert!(
        fil.get_second_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocForLoop2
#[test]
fn test_old_jsdoc_for_loop2() {
    let mut h = Harness::new();

    let root = h.parse("for (/** attach */ i = 0; i < 5; i++) {}", &[]);
    let fl = root.get_first_child(&h.ast).unwrap();
    assert!(fl.get_jsdoc_info(&h.ast).is_none());
    assert!(
        fl.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocForLoop4
#[test]
fn test_old_jsdoc_for_loop4() {
    let mut h = Harness::new();

    let root = h.parse("for (i = /** attach */ 0; i < 5; i++) {}", &[]);
    let fl = root.get_first_child(&h.ast).unwrap();
    let init = fl.get_first_child(&h.ast).unwrap();
    assert!(
        init.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
    assert!(
        init.get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocForLoop7
#[test]
fn test_old_jsdoc_for_loop7() {
    let mut h = Harness::new();

    let root = h.parse("for (i = 0; i < /** attach */ 5; i++) {}", &[]);
    let fl = root.get_first_child(&h.ast).unwrap();
    assert!(fl.get_jsdoc_info(&h.ast).is_none());
    let cond = fl.get_second_child(&h.ast).unwrap();
    assert!(
        cond.get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocForLoop8
#[test]
fn test_old_jsdoc_for_loop8() {
    let mut h = Harness::new();

    let root = h.parse("for (i = 0; i < 5; /** attach */ i++) {}", &[]);
    let fl = root.get_first_child(&h.ast).unwrap();
    assert!(fl.get_jsdoc_info(&h.ast).is_none());
    assert!(
        fl.get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocForLoop11
#[test]
fn test_old_jsdoc_for_loop11() {
    let mut h = Harness::new();

    let root = h.parse("for (/** attach */ var i = 0; i < 5; i++) {}", &[]);
    let fl = root.get_first_child(&h.ast).unwrap();
    assert!(fl.get_jsdoc_info(&h.ast).is_none());
    assert!(
        fl.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocFun1
#[test]
fn test_old_jsdoc_fun1() {
    let mut h = Harness::new();

    let root = h.parse("function f(/** string */ e) {}", &[]);
    let fun = root.get_first_child(&h.ast).unwrap();
    let params = fun.get_second_child(&h.ast).unwrap();
    assert!(
        params
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocFun2
#[test]
fn test_old_jsdoc_fun2() {
    let mut h = Harness::new();

    let root = h.parse("(function() {/** don't attach */})()", &[]);
    let call = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        call.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
}

// port: AttachJsdocsTest#testOldJsdocFun3
#[test]
fn test_old_jsdoc_fun3() {
    let mut h = Harness::new();

    let root = h.parse("function /** string */ f (e) {}", &[]);
    let fun = root.get_first_child(&h.ast).unwrap();
    assert!(
        fun.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocFun4
#[test]
fn test_old_jsdoc_fun4() {
    let mut h = Harness::new();

    let root = h.parse("f = /** attach */ function(e) {};", &[]);
    let assign = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        assign
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocFun5
#[test]
fn test_old_jsdoc_fun5() {
    let mut h = Harness::new();

    let root = h.parse("x = 1; /** attach */ function f(e) {}", &[]);
    assert!(
        root.get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocFun6
#[test]
fn test_old_jsdoc_fun6() {
    let mut h = Harness::new();

    let root = h.parse("function f() { /** attach */ function Foo(){} }", &[]);
    let inner_fun = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(inner_fun.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocFun7
#[test]
fn test_old_jsdoc_fun7() {
    let mut h = Harness::new();

    let root = h.parse("(function f() { /** attach */function Foo(){} })();", &[]);
    let outer_fun = root
        .get_first_first_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(
        outer_fun
            .get_last_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocGetElem1
#[test]
fn test_old_jsdoc_get_elem1() {
    let mut h = Harness::new();

    let root = h.parse("(/** attach */ {})['prop'];", &[]);
    let get_elem = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        get_elem
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocGetElem3
#[test]
fn test_old_jsdoc_get_elem3() {
    let mut h = Harness::new();

    let root = h.parse("({})[/** attach */ 'prop'];", &[]);
    let get_elem = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        get_elem
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocGetProp1
#[test]
fn test_old_jsdoc_get_prop1() {
    let mut h = Harness::new();

    let root = h.parse("(/** attach */ {}).prop;", &[]);
    let get_prop = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        get_prop
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocGetProp2
#[test]
fn test_old_jsdoc_get_prop2() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ ({}).prop;", &[]);
    let get_prop = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        get_prop
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocGetProp3
#[test]
fn test_old_jsdoc_get_prop3() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ obj.prop;", &[]);
    let get_prop = root.get_first_first_child(&h.ast).unwrap();
    assert!(get_prop.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocGetter1
#[test]
fn test_old_jsdoc_getter1() {
    let mut h = Harness::new();

    h.mode = LanguageMode::ECMASCRIPT5;
    let root = h.parse("({/** attach */ get foo() {}});", &[]);
    let objlit = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        objlit
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocGetter2
#[test]
fn test_old_jsdoc_getter2() {
    let mut h = Harness::new();

    h.mode = LanguageMode::ECMASCRIPT5;
    let root = h.parse("({/** attach */ get 1() {}});", &[]);
    let objlit = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        objlit
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocGetter3
#[test]
fn test_old_jsdoc_getter3() {
    let mut h = Harness::new();

    h.mode = LanguageMode::ECMASCRIPT5;
    let root = h.parse("({/** attach */ get 'foo'() {}});", &[]);
    let objlit = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        objlit
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testJsdocHook1
#[test]
fn test_jsdoc_hook1() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ (true) ? 1 : 2;", &[]);
    let hook = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        hook.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocHook3
#[test]
fn test_old_jsdoc_hook3() {
    let mut h = Harness::new();

    let root = h.parse("true ? /** attach */ 1 : 2;", &[]);
    let hook = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        hook.get_second_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocHook5
#[test]
fn test_old_jsdoc_hook5() {
    let mut h = Harness::new();

    let root = h.parse("true ? 1 : /** attach */ 2;", &[]);
    let hook = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        hook.get_child_at_index(&h.ast, 2)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocIf1
#[test]
fn test_old_jsdoc_if1() {
    let mut h = Harness::new();

    let root = h.parse("if (/** attach */ x) {}", &[]);
    assert!(
        root.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocNew1
#[test]
fn test_old_jsdoc_new1() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ new Foo();", &[]);
    let newexp = root.get_first_first_child(&h.ast).unwrap();
    assert!(newexp.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocNew2
#[test]
fn test_old_jsdoc_new2() {
    let mut h = Harness::new();

    let root = h.parse("new /** don't attach */ Foo();", &[]);
    let newexp = root.get_first_first_child(&h.ast).unwrap();
    assert!(newexp.get_jsdoc_info(&h.ast).is_none());
}

// port: AttachJsdocsTest#testOldJsdocObjLit2
#[test]
fn test_old_jsdoc_obj_lit2() {
    let mut h = Harness::new();

    let root = h.parse("({1: /** attach */ 2, 3: 4});", &[]);
    let objlit = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        objlit
            .get_first_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocObjLit3
#[test]
fn test_old_jsdoc_obj_lit3() {
    let mut h = Harness::new();

    let root = h.parse("({'1': /** attach */ (foo())});", &[]);
    let objlit = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        objlit
            .get_first_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocPostfix1
#[test]
fn test_old_jsdoc_postfix1() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ (x)++;", &[]);
    let unary = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        unary
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocPostfix2
#[test]
fn test_old_jsdoc_postfix2() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ x++;", &[]);
    let unary = root.get_first_first_child(&h.ast).unwrap();
    assert!(unary.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocReturn1
#[test]
fn test_old_jsdoc_return1() {
    let mut h = Harness::new();

    let root = h.parse("function f(x) { return /** string */ x; }", &[]);
    let ret = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(
        ret.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocReturn3
#[test]
fn test_old_jsdoc_return3() {
    let mut h = Harness::new();

    let root = h.parse("function f(e) { return /** 1 */(g(1 /** 2 */)); }\n", &[]);
    let ret = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(
        ret.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
    assert!(
        ret.get_first_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
}

// port: AttachJsdocsTest#testOldJsdocSetter
#[test]
fn test_old_jsdoc_setter() {
    let mut h = Harness::new();

    h.mode = LanguageMode::ECMASCRIPT5;
    let root = h.parse("({/** attach */ set foo(x) {}});", &[]);
    let objlit = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        objlit
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocScript1
#[test]
fn test_old_jsdoc_script1() {
    let mut h = Harness::new();

    let root = h.parse("{ 1; /** attach */ 2; }", &[]);
    let block = root.get_first_child(&h.ast).unwrap();
    assert!(
        block
            .get_last_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocScript2
#[test]
fn test_old_jsdoc_script2() {
    let mut h = Harness::new();

    let root = h.parse("1; /** attach */ 2;", &[]);
    assert!(
        root.get_last_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocScript3
#[test]
fn test_old_jsdoc_script3() {
    let mut h = Harness::new();

    let root = h.parse("1;/** attach */ function f(){}", &[]);
    assert!(
        root.get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocSwitch1
#[test]
fn test_old_jsdoc_switch1() {
    let mut h = Harness::new();

    let root = h.parse("switch /** attach */ (x) {}", &[]);
    let sw = root.get_first_child(&h.ast).unwrap();
    assert!(
        sw.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocSwitch3
#[test]
fn test_old_jsdoc_switch3() {
    let mut h = Harness::new();

    let root = h.parse("switch (x) { case /** attach */ 1: ; }", &[]);
    let sw = root.get_first_child(&h.ast).unwrap();
    let number_node = sw
        .get_second_child(&h.ast)
        .unwrap()
        .get_first_first_child(&h.ast)
        .unwrap();
    assert!(number_node.is_number(&h.ast));
    assert!(number_node.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocSwitch4
#[test]
fn test_old_jsdoc_switch4() {
    let mut h = Harness::new();

    let root = h.parse("switch (x) { case 1: /** don't attach */ {}; }", &[]);
    let sw = root.get_first_child(&h.ast).unwrap();
    let case_block = sw
        .get_second_child(&h.ast)
        .unwrap()
        .get_only_child(&h.ast)
        .get_last_child(&h.ast)
        .unwrap();
    assert!(case_block.is_block(&h.ast));
    assert!(case_block.get_jsdoc_info(&h.ast).is_none());
}

// port: AttachJsdocsTest#testOldJsdocSwitch5
#[test]
fn test_old_jsdoc_switch5() {
    let mut h = Harness::new();

    let root = h.parse("switch (x) { default: /** don't attach */ {}; }", &[]);
    let sw = root.get_first_child(&h.ast).unwrap();
    let default_block = sw
        .get_second_child(&h.ast)
        .unwrap()
        .get_only_child(&h.ast)
        .get_last_child(&h.ast)
        .unwrap();
    assert!(default_block.is_block(&h.ast));
    assert!(default_block.get_jsdoc_info(&h.ast).is_none());
}

// port: AttachJsdocsTest#testOldJsdocSwitch6
#[test]
fn test_old_jsdoc_switch6() {
    let mut h = Harness::new();

    let root = h.parse("switch (x) { case 1: /** don't attach */ }", &[]);
    let sw = root.get_first_child(&h.ast).unwrap();
    let case_block = sw
        .get_second_child(&h.ast)
        .unwrap()
        .get_only_child(&h.ast)
        .get_last_child(&h.ast)
        .unwrap();
    assert!(case_block.is_block(&h.ast));
    assert!(
        sw.get_second_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_none()
    );
}

// port: AttachJsdocsTest#testOldJsdocSwitch7
#[test]
fn test_old_jsdoc_switch7() {
    let mut h = Harness::new();

    let root = h.parse(
        "switch (x) {\n  case 1:\n    /** attach */ y;\n    /** attach */ z;\n}\n",
        &[],
    );
    let sw = root.get_first_child(&h.ast).unwrap();
    let case_body = sw
        .get_second_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap();
    assert!(case_body.is_block(&h.ast));
    assert!(
        case_body
            .get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
    assert!(
        case_body
            .get_second_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocThrow
#[test]
fn test_old_jsdoc_throw() {
    let mut h = Harness::new();

    let root = h.parse("throw /** attach */ new Foo();", &[]);
    assert!(
        root.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocTryCatch1
#[test]
fn test_old_jsdoc_try_catch1() {
    let mut h = Harness::new();

    let root = h.parse("try {} catch (/** attach */ e) {}", &[]);
    let catch_node = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap()
        .get_first_child(&h.ast)
        .unwrap();
    assert!(
        catch_node
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocTryFinally
#[test]
fn test_old_jsdoc_try_finally() {
    let mut h = Harness::new();

    let root = h.parse("try {} finally { /** attach */ e; }", &[]);
    let finally_block = root
        .get_first_child(&h.ast)
        .unwrap()
        .get_last_child(&h.ast)
        .unwrap();
    assert!(
        finally_block
            .get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocUnary
#[test]
fn test_old_jsdoc_unary() {
    let mut h = Harness::new();

    let root = h.parse("!(/** attach */ x);", &[]);
    let exp = root.get_first_first_child(&h.ast).unwrap();
    assert!(
        exp.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocVar1
#[test]
fn test_old_jsdoc_var1() {
    let mut h = Harness::new();

    let root = h.parse("/** attach */ var a;", &[]);
    assert!(
        root.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocVar2
#[test]
fn test_old_jsdoc_var2() {
    let mut h = Harness::new();

    let root = h.parse("var a = /** attach */ (x);", &[]);
    let var = root.get_first_child(&h.ast).unwrap();
    assert!(
        var.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocVar3
#[test]
fn test_old_jsdoc_var3() {
    let mut h = Harness::new();

    let root = h.parse("var a = (/** attach */ {});", &[]);
    let var = root.get_first_child(&h.ast).unwrap();
    assert!(
        var.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocVar4
#[test]
fn test_old_jsdoc_var4() {
    let mut h = Harness::new();

    let root = h.parse("var /** number */ a = x;", &[]);
    let var = root.get_first_child(&h.ast).unwrap();
    assert!(
        var.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocVar5
#[test]
fn test_old_jsdoc_var5() {
    let mut h = Harness::new();

    let root = h.parse("x = 1; /** attach */ var y = 5;", &[]);
    let var = root.get_last_child(&h.ast).unwrap();
    assert!(var.get_jsdoc_info(&h.ast).is_some());
}

// port: AttachJsdocsTest#testOldJsdocWhile2
#[test]
fn test_old_jsdoc_while2() {
    let mut h = Harness::new();

    let root = h.parse("while /** attach */ (x) {}", &[]);
    let wh = root.get_first_child(&h.ast).unwrap();
    assert!(
        wh.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocWith1
#[test]
fn test_old_jsdoc_with1() {
    let mut h = Harness::new();

    let root = h.parse("with (/** attach */ obj) {};", &[]);
    let with = root.get_first_child(&h.ast).unwrap();
    assert!(
        with.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocWith4
#[test]
fn test_old_jsdoc_with4() {
    let mut h = Harness::new();

    let root = h.parse(
        "/** @suppress {with} */ with (context) {\n  eval('[' + expr + ']');\n}\n",
        &[],
    );
    assert!(
        root.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocManyComments1
#[test]
fn test_old_jsdoc_many_comments1() {
    let mut h = Harness::new();

    let root = h.parse(
        "function /** number */ f(/** number */ x, /** number */ y) {\n  return x + y;\n}\n",
        &[],
    );
    let fun = root.get_first_child(&h.ast).unwrap();
    assert!(
        fun.get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
    assert!(
        fun.get_second_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
    assert!(
        fun.get_second_child(&h.ast)
            .unwrap()
            .get_last_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocManyComments2
#[test]
fn test_old_jsdoc_many_comments2() {
    let mut h = Harness::new();

    let root = h.parse("var /** number */ x = 1; var /** string */ y = 2;", &[]);
    assert!(
        root.get_first_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
    assert!(
        root.get_last_child(&h.ast)
            .unwrap()
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}

// port: AttachJsdocsTest#testOldJsdocManyCommentsOnOneNode
#[test]
fn test_old_jsdoc_many_comments_on_one_node() {
    let mut h = Harness::new();

    let root = h.parse("var x; /** foo */ /** bar */ function f() {}", &[]);
    let info = root.get_last_child(&h.ast).unwrap().get_jsdoc_info(&h.ast);
    assert!(info.is_some());
    assert_eq!(
        info.as_ref().unwrap().get_original_comment_string(),
        Some(JsString::from("/** bar */"))
    );
}

// port: AttachJsdocsTest#testInlineInExport
#[test]
fn test_inline_in_export() {
    let mut h = Harness::new();

    h.mode = LanguageMode::ECMASCRIPT_2015;
    let root = h.parse("export var /** number */ x;", &[]);
    let module_body = root.get_first_child(&h.ast).unwrap();
    let export_node = module_body.get_first_child(&h.ast).unwrap();
    let var_node = export_node.get_first_child(&h.ast).unwrap();
    assert!(
        var_node
            .get_first_child(&h.ast)
            .unwrap()
            .get_jsdoc_info(&h.ast)
            .is_some()
    );
}
