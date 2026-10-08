/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/NodeUtilTest.java.

use super::*;
use closure_jscomp::{node_util::GoogRequire, syntactic_scope_creator::SyntacticScopeCreator};
use closure_rhino::jscomp_base::tri::Tri;
mod boolean_value_tests {
    use super::*;
    // port: NodeUtilTest.BooleanValueTests#getBooleanValue
    #[test]
    fn get_boolean_value() {
        let cases = [
            ("true", Tri::TRUE),
            ("10", Tri::TRUE),
            ("1n", Tri::TRUE),
            ("'0'", Tri::TRUE),
            ("/a/", Tri::TRUE),
            ("{}", Tri::TRUE),
            ("[]", Tri::TRUE),
            ("false", Tri::FALSE),
            ("null", Tri::FALSE),
            ("0", Tri::FALSE),
            ("0n", Tri::FALSE),
            ("''", Tri::FALSE),
            ("undefined", Tri::FALSE),
            ("{a:foo()}", Tri::TRUE),
            ("[foo()]", Tri::TRUE),
            ("void 0", Tri::FALSE),
            ("void foo()", Tri::FALSE),
            ("!true", Tri::FALSE),
            ("!false", Tri::TRUE),
            ("!''", Tri::TRUE),
            ("class Klass {}", Tri::TRUE),
            ("new Date()", Tri::TRUE),
            ("b", Tri::UNKNOWN),
            ("+'0.0'", Tri::FALSE),
            ("+'1.0'", Tri::TRUE),
            ("+'3.1'", Tri::TRUE),
            ("+3.1", Tri::TRUE),
            ("-'0.0'", Tri::FALSE),
            ("-'1.0'", Tri::TRUE),
            ("-'1.0'", Tri::TRUE),
            ("-'3.1'", Tri::TRUE),
            ("-0.0", Tri::FALSE),
            ("-1.0", Tri::TRUE),
            ("-3.1", Tri::TRUE),
            ("-Infinity", Tri::TRUE),
            ("-NaN", Tri::FALSE),
            ("0.0", Tri::FALSE),
            ("1", Tri::TRUE),
            ("Infinity", Tri::TRUE),
            ("NaN", Tri::FALSE),
            ("~'-1.0'", Tri::FALSE),
            ("~-1.0", Tri::FALSE),
            ("~0.0", Tri::TRUE),
            ("~3.1", Tri::TRUE),
            ("~NaN", Tri::TRUE),
            ("0n", Tri::FALSE),
            ("-0n", Tri::FALSE),
            ("~0n", Tri::TRUE),
            ("~-0n", Tri::TRUE),
            ("-~0n", Tri::TRUE),
            ("1n", Tri::TRUE),
            ("-1n", Tri::TRUE),
            ("~1n", Tri::TRUE),
            ("~-1n", Tri::FALSE),
            ("-~1n", Tri::TRUE),
            ("``", Tri::FALSE),
            ("`definiteLength`", Tri::TRUE),
            ("`${some}str`", Tri::UNKNOWN),
            ("a=true", Tri::TRUE),
            ("a=false", Tri::FALSE),
            ("a=(false,true)", Tri::TRUE),
            ("a=(true,false)", Tri::FALSE),
            ("a=(false || true)", Tri::TRUE),
            ("a=(true && false)", Tri::FALSE),
            ("a=!(true && false)", Tri::TRUE),
            ("a,true", Tri::TRUE),
            ("a,false", Tri::FALSE),
            ("true||false", Tri::TRUE),
            ("false||false", Tri::FALSE),
            ("true&&true", Tri::TRUE),
            ("true&&false", Tri::FALSE),
            ("a||(a=true)", Tri::TRUE),
            ("a||(a=false)", Tri::UNKNOWN),
            ("a||=true", Tri::TRUE),
            ("a||=false", Tri::UNKNOWN),
            ("a&&(a=true)", Tri::UNKNOWN),
            ("a&&(a=false)", Tri::FALSE),
            ("a&&=true", Tri::UNKNOWN),
            ("a&&=false", Tri::FALSE),
            ("a??(a=true)", Tri::UNKNOWN),
            ("a??=true", Tri::UNKNOWN),
            ("a??(a=false)", Tri::UNKNOWN),
            ("a??=false", Tri::UNKNOWN),
            ("a *= 2", Tri::UNKNOWN),
            ("2 + 2", Tri::UNKNOWN),
            ("a=1", Tri::TRUE),
            ("a=/a/", Tri::TRUE),
            ("a={}", Tri::TRUE),
            ("a?true:true", Tri::TRUE),
            ("a?false:false", Tri::FALSE),
            ("a?true:false", Tri::UNKNOWN),
            ("a?true:foo()", Tri::UNKNOWN),
            ("null??false", Tri::FALSE),
            ("2??[]", Tri::TRUE),
            ("{}??false", Tri::TRUE),
            ("undefined??[]", Tri::UNKNOWN),
            ("foo()??true", Tri::UNKNOWN),
        ];
        assert_eq!(cases.len(), 97);
        for (js_expression, expected_result) in cases {
            let mut compiler = Compiler::new();
            let n = parse_expr(&mut compiler, js_expression);
            assert_eq!(
                NodeUtil::get_boolean_value(&compiler, n),
                expected_result,
                "{js_expression}"
            );
        }
        println!("BooleanValueTests: 97 parameter rows executed");
    }
}
mod goog_require_info_test {
    use super::*;
    // port: NodeUtilTest.GoogRequireInfoTest#test
    #[test]
    fn test() {
        let cases = [
            (
                "goog.module('a.b.c'); const {Bar} = goog.require('d.Foo');",
                "Bar",
                Some(GoogRequire::from_namespace_and_property(
                    JsString::from("d.Foo"),
                    JsString::from("Bar"),
                    true,
                )),
            ),
            (
                "goog.module('a.b.c'); const {Bar} = goog.require('d.Foo');",
                "Foo",
                None,
            ),
            (
                "goog.module('a.b.c'); const {Bar: BarLocal} = goog.require('d.Foo');",
                "BarLocal",
                Some(GoogRequire::from_namespace_and_property(
                    JsString::from("d.Foo"),
                    JsString::from("Bar"),
                    true,
                )),
            ),
            (
                "goog.module('a.b.c'); const {Bar: BarLocal} =\n goog.require('d.Foo');\n",
                "Bar",
                None,
            ),
            (
                "goog.module('a.b.c'); const Foo = goog.require('d.Foo');",
                "Foo",
                Some(GoogRequire::from_namespace(JsString::from("d.Foo"), true)),
            ),
            (
                "goog.module('a.b.c'); const dFoo = goog.require('d.Foo');",
                "dFoo",
                Some(GoogRequire::from_namespace(JsString::from("d.Foo"), true)),
            ),
            (
                "goog.module('a.b.c'); const Foo = goog.requireType('d.Foo');",
                "Foo",
                Some(GoogRequire::from_namespace(JsString::from("d.Foo"), false)),
            ),
            (
                "goog.module('a.b.c'); const {Bar} = goog.requireType('d.Foo');",
                "Bar",
                Some(GoogRequire::from_namespace_and_property(
                    JsString::from("d.Foo"),
                    JsString::from("Bar"),
                    false,
                )),
            ),
            ("goog.module('a.b.c'); let Foo;", "Foo", None),
            ("goog.module('a.b.c'); let [Foo] = arr;", "Foo", None),
            ("goog.module('a.b.c'); let {Bar: {Foo}} = obj;", "Foo", None),
            ("goog.module('a.b.c'); const Foo = 0;", "Foo", None),
            ("const Foo = goog.require('d.Foo');", "Foo", None),
        ];
        assert_eq!(cases.len(), 13);
        for (src, name, require) in cases {
            let mut compiler = Compiler::new();
            let first = parse(&mut compiler, src);
            let mut scope_creator = SyntacticScopeCreator::new();
            let global_scope = scope_creator.create_scope(&mut compiler, first, None);
            let module = get_node_or_null(&compiler, first, Token::MODULE_BODY);
            let local_scope = if let Some(module) = module {
                scope_creator.create_scope(&mut compiler, module, Some(global_scope))
            } else {
                global_scope
            };
            assert_eq!(
                NodeUtil::get_goog_require_info_name(
                    &mut compiler,
                    JsString::from(name),
                    local_scope
                ),
                require,
                "{src}: {name}"
            );
        }
        println!("GoogRequireInfoTest: 13 parameter rows executed");
    }
}
mod get_r_value_of_l_value_test {
    use super::*;
    // port: NodeUtilTest.GetRValueOfLValueTest#test
    #[test]
    fn test() {
        let cases = [
            (Token::MEMBER_FUNCTION_DEF, "constructor() { }"),
            (Token::MEMBER_FUNCTION_DEF, "foo() { }"),
            (Token::MEMBER_FUNCTION_DEF, "static foo() { }"),
            (Token::GETTER_DEF, "get foo() { }"),
            (Token::GETTER_DEF, "static get foo() { }"),
            (Token::SETTER_DEF, "set foo(x) { }"),
            (Token::SETTER_DEF, "static set foo(x) { }"),
        ];
        assert_eq!(cases.len(), 7);
        for (token, member) in cases {
            let mut compiler = Compiler::new();
            let n = parse_first(&mut compiler, token, format!("class F {{ {member} }}"));
            assert_eq!(
                NodeUtil::get_r_value_of_l_value(&compiler, n)
                    .unwrap()
                    .get_token(&compiler),
                Token::FUNCTION,
                "{member}"
            );
        }
        println!("GetRValueOfLValueTest: 7 parameter rows executed");
    }
}
mod iterates_impure_iterable_test {
    use super::*;
    // port: NodeUtilTest.IteratesImpureIterableTest#test
    #[test]
    fn test() {
        let cases = [
            (Token::ITER_SPREAD, "[...[]]", false),
            (Token::ITER_SPREAD, "[...[danger()]]", false),
            (Token::ITER_SPREAD, "[...'lit']", false),
            (Token::ITER_SPREAD, "[...`template`]", false),
            (Token::ITER_SPREAD, "[...`template ${sub}`]", false),
            (Token::ITER_SPREAD, "[...`template ${danger()}`]", false),
            (Token::ITER_SPREAD, "[...danger]", true),
            (Token::ITER_SPREAD, "[...danger()]", true),
            (Token::ITER_SPREAD, "[...5]", true),
            (Token::ITER_SPREAD, "foo(...[])", false),
            (Token::ITER_SPREAD, "foo(...[danger()])", false),
            (Token::ITER_SPREAD, "foo(...'lit')", false),
            (Token::ITER_SPREAD, "foo(...`template`)", false),
            (Token::ITER_SPREAD, "foo(...`template ${safe}`)", false),
            (Token::ITER_SPREAD, "foo(...`template ${danger()}`)", false),
            (Token::ITER_SPREAD, "foo(...danger)", true),
            (Token::ITER_SPREAD, "foo(...danger())", true),
            (Token::ITER_SPREAD, "foo(...5)", true),
            (Token::ITER_SPREAD, "new foo(...[])", false),
            (Token::ITER_SPREAD, "new foo(...[danger()])", false),
            (Token::ITER_SPREAD, "new foo(...'lit')", false),
            (Token::ITER_SPREAD, "new foo(...`template`)", false),
            (Token::ITER_SPREAD, "new foo(...`template ${safe}`)", false),
            (
                Token::ITER_SPREAD,
                "new foo(...`template ${danger()}`)",
                false,
            ),
            (Token::ITER_SPREAD, "new foo(...danger)", true),
            (Token::ITER_SPREAD, "new foo(...danger())", true),
            (Token::ITER_SPREAD, "new foo(...5)", true),
            (Token::ITER_REST, "const [...rest] = []", true),
            (Token::ITER_REST, "const [...rest] = 'lit'", true),
            (Token::ITER_REST, "const [...rest] = `template`", true),
            (Token::ITER_REST, "const [...rest] = safe", true),
            (Token::ITER_REST, "function f([...rest]) { }", true),
            (Token::ITER_REST, "const [[...rest]] = safe", true),
            (Token::ITER_REST, "const {key: [...rest]} = safe", true),
            (Token::ITER_REST, "function f(...x) { }", false),
            (Token::ITER_REST, "function f(a, ...x) { }", false),
            (Token::ITER_REST, "async function f(...x) { }", false),
            (Token::ITER_REST, "function* f(...x) { }", false),
            (Token::ITER_REST, "async function* f(...x) { }", false),
            (Token::ITER_REST, "((...x) => { })", false),
            (Token::FOR_OF, "for (let x of []) {}", false),
            (Token::FOR_OF, "for (let x of [danger()]) {}", false),
            (Token::FOR_OF, "for (let x of 'lit') {}", false),
            (Token::FOR_OF, "for (let x of `template`) {}", false),
            (Token::FOR_OF, "for (let x of `template ${safe}`) {}", false),
            (
                Token::FOR_OF,
                "for (let x of `template ${danger()}`) {}",
                false,
            ),
            (Token::FOR_OF, "for (let x of danger) {}", true),
            (Token::FOR_OF, "for (let x of danger()) {}", true),
            (Token::FOR_OF, "for (let x of 5) {}", true),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of []) {} })",
                false,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of [danger()]) {} })",
                false,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of 'literal') {} })",
                false,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of `template`) {} })",
                false,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of `t ${safe}`) {} })",
                false,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of `t ${dn()}`) {} })",
                false,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of danger) {} })",
                true,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of danger()) {} })",
                true,
            ),
            (
                Token::FOR_AWAIT_OF,
                "(async()=>{ for await (let x of 5) {} })",
                true,
            ),
            (Token::YIELD, "function* f() { yield* []; }", false),
            (Token::YIELD, "function* f() { yield* [danger()]; }", false),
            (Token::YIELD, "function* f() { yield* 'lit'; }", false),
            (Token::YIELD, "function* f() { yield* `template`; }", false),
            (
                Token::YIELD,
                "function* f() { yield* `template ${sub}`; }",
                false,
            ),
            (
                Token::YIELD,
                "function* f() { yield* `template ${danger()}`; }",
                false,
            ),
            (Token::YIELD, "function* f() { yield* danger; }", true),
            (Token::YIELD, "function* f() { yield* danger(); }", true),
            (Token::YIELD, "function* f() { yield* 5; }", true),
            (Token::YIELD, "function* f() { yield danger(); }", false),
        ];
        assert_eq!(cases.len(), 68);
        for (token, source, expectation) in cases {
            let mut compiler = Compiler::new();
            let node = parse_first(&mut compiler, token, source);
            assert_eq!(
                NodeUtil::iterates_impure_iterable(&compiler, node),
                expectation,
                "{source}"
            );
        }
        println!("IteratesImpureIterableTest: 68 parameter rows executed");
    }
}
mod can_be_side_effected_test {
    use super::*;
    // port: NodeUtilTest.CanBeSideEffectedTest#test
    #[test]
    fn test() {
        let cases = [
            (Token::SUPER, "super()", false),
            (Token::SUPER, "super.foo()", false),
            (Token::CALL, "super()", true),
            (Token::CALL, "super.foo()", true),
            (Token::OPTCHAIN_CALL, "x?.()", true),
            (Token::OPTCHAIN_GETPROP, "x?.y", true),
            (Token::OPTCHAIN_GETELEM, "x?.[y]", true),
            (Token::GETPROP, "Symbol.iterator", false),
            (Token::GETPROP, "Fake.Symbol.iterator", true),
            (Token::TAGGED_TEMPLATELIT, "tag`literal`", true),
            (Token::AWAIT, "(async () => { await p; })", true),
            (Token::DYNAMIC_IMPORT, "import('m')", true),
        ];
        assert_eq!(cases.len(), 12);
        for (token, source, expectation) in cases {
            let mut compiler = Compiler::new();
            let node = parse_first(&mut compiler, token, source);
            assert_eq!(
                NodeUtil::can_be_side_effected(&compiler, node),
                expectation,
                "{source}"
            );
        }
        println!("CanBeSideEffectedTest: 12 parameter rows executed");
    }
}
mod references_receiver_test {
    use super::*;
    // port: NodeUtilTest.ReferencesReceiverTest#cases
    fn cases() -> Vec<(String, bool, bool)> {
        let template_to_defines_own_receiver = [
            ("      (x = (%s)) => {}", false),
            ("      (      ) => (%s)", false),
            ("async (x = (%s)) => {}", false),
            ("async (      ) => (%s)", false),
            ("      function   f(        ) { (%s); }", true),
            ("async function   f(        ) { (%s); }", true),
            ("async function  *f(        ) { (%s); }", true),
            ("      function  *f(        ) { (%s); }", true),
            ("      function   f(x = (%s)) {       }", true),
            ("      function  *f(x = (%s)) {       }", true),
            ("async function   f(x = (%s)) {       }", true),
            ("async function  *f(x = (%s)) {       }", true),
            ("class F {        f(        ) { (%s); } }", true),
            ("class F {       *f(        ) { (%s); } }", true),
            ("class F { async  f(        ) { (%s); } }", true),
            ("class F { async *f(        ) { (%s); } }", true),
            ("class F {        f(x = (%s)) {       } }", true),
            ("class F {       *f(x = (%s)) {       } }", true),
            ("class F { async  f(x = (%s)) {       } }", true),
            ("class F { async *f(x = (%s)) {       } }", true),
            ("({               f(        ) { (%s); } })", true),
            ("({              *f(        ) { (%s); } })", true),
            ("({        async  f(        ) { (%s); } })", true),
            ("({        async *f(        ) { (%s); } })", true),
            ("({               f(x = (%s)) {       } })", true),
            ("({              *f(x = (%s)) {       } })", true),
            ("({        async  f(x = (%s)) {       } })", true),
            ("({        async *f(x = (%s)) {       } })", true),
        ];
        let expr_to_uses_receiver = [
            ("this", true),
            ("1 || this", true),
            ("{ x: this, }", true),
            ("1", false),
        ];
        let mut cases = Vec::new();
        for (outer_template, outer_receiver) in template_to_defines_own_receiver {
            for (inner_template, inner_receiver) in template_to_defines_own_receiver {
                for (expr, uses_receiver) in expr_to_uses_receiver {
                    let case_src =
                        outer_template.replace("%s", &inner_template.replace("%s", expr));
                    cases.push((
                        case_src,
                        !outer_receiver && !inner_receiver && uses_receiver,
                        outer_receiver && !inner_receiver && uses_receiver,
                    ));
                }
            }
        }
        cases.extend([
            ("class F { f() { super.a() } }".to_owned(), false, true),
            (
                "class F { f() { () => super.a(); } }".to_owned(),
                false,
                true,
            ),
            (
                "() => class F { f() { super.a(); } }".to_owned(),
                false,
                false,
            ),
            ("({ f() { super.a() } })".to_owned(), false, true),
            ("({ f() { () => super.a(); } })".to_owned(), false, true),
            ("() => ({ f() { super.a(); } })".to_owned(), false, false),
            ("function f() { new.target; }".to_owned(), false, false),
            (
                "function f() { () => new.target; }".to_owned(),
                false,
                false,
            ),
            (
                "() => function f() { new.target; }".to_owned(),
                false,
                false,
            ),
        ]);
        assert_eq!(cases.len(), 28 * 28 * 4 + 9);
        cases
    }
    // port: NodeUtilTest.ReferencesReceiverTest#testReferencesEnclosingReceiver_ofScript
    #[test]
    fn test_references_enclosing_receiver_of_script() {
        let cases = cases();
        let count = cases.len();
        for (js, ref_to_enclosing, _ref_to_outer_fn_own) in cases {
            let mut compiler = Compiler::new();
            let node = parse_first(&mut compiler, Token::SCRIPT, js.clone());
            assert_eq!(
                NodeUtil::references_enclosing_receiver(&compiler, node),
                ref_to_enclosing,
                "{js}"
            );
        }
        println!(
            "ReferencesReceiverTest#testReferencesEnclosingReceiver_ofScript: {count} parameter rows executed"
        );
    }
    // port: NodeUtilTest.ReferencesReceiverTest#testReferencesEnclosingReceiver_ofFn
    #[test]
    fn test_references_enclosing_receiver_of_fn() {
        let cases = cases();
        let count = cases.len();
        for (js, ref_to_enclosing, _ref_to_outer_fn_own) in cases {
            let mut compiler = Compiler::new();
            let node = parse_first(&mut compiler, Token::FUNCTION, js.clone());
            assert_eq!(
                NodeUtil::references_enclosing_receiver(&compiler, node),
                ref_to_enclosing,
                "{js}"
            );
        }
        println!(
            "ReferencesReceiverTest#testReferencesEnclosingReceiver_ofFn: {count} parameter rows executed"
        );
    }
    // port: NodeUtilTest.ReferencesReceiverTest#testReferencesOwnReceiver_ofFn
    #[test]
    fn test_references_own_receiver_of_fn() {
        let cases = cases();
        let count = cases.len();
        for (js, _ref_to_enclosing, ref_to_outer_fn_own) in cases {
            let mut compiler = Compiler::new();
            let node = parse_first(&mut compiler, Token::FUNCTION, js.clone());
            assert_eq!(
                NodeUtil::references_own_receiver(&compiler, node),
                ref_to_outer_fn_own,
                "{js}"
            );
        }
        println!(
            "ReferencesReceiverTest#testReferencesOwnReceiver_ofFn: {count} parameter rows executed"
        );
    }
}
