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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/TemplateAstMatcherTest.java.

//! Port of `TemplateAstMatcherTest.java`.

use closure_jscomp::{
    Compiler, compiler_options::CompilerOptions, source_file::SourceFile,
    template_ast_matcher::TemplateAstMatcher, type_matching_strategy::TypeMatchingStrategy,
};
use closure_rhino::node::NodeId;
use std::sync::Arc;

/// A Java `Node` reference read from the last compiler (`None` is Java's null).
#[derive(Clone, Copy)]
struct Nav<'a> {
    c: &'a Compiler,
    n: Option<NodeId>,
}

impl<'a> Nav<'a> {
    fn of(c: &'a Compiler, n: NodeId) -> Self {
        Self { c, n: Some(n) }
    }
    fn node(self) -> NodeId {
        self.n.expect("NullPointerException")
    }
    fn with(self, n: Option<NodeId>) -> Self {
        Self { c: self.c, n }
    }
}

// Nav's Node navigation, which names Closure's Rhino-derived Node (MPL-1.1 /
// GPL-2.0-or-later), is in its own file.
#[path = "rhino/template_ast_matcher_test.rs"]
mod rhino;

// port: TemplateAstMatcherTest.TestNodePair
struct TestNodePair {
    template_node: NodeId,
    test_node: NodeId,
}

impl TestNodePair {
    // port: TemplateAstMatcherTest.TestNodePair#TestNodePair
    fn new(c: &Compiler, template: NodeId, root: NodeId) -> Self {
        Self {
            template_node: template,
            test_node: root.get_last_child(c).unwrap().get_first_child(c).unwrap(),
        }
    }

    // port: TemplateAstMatcherTest.TestNodePair#getTestExprResultRoot
    fn get_test_expr_result_root<'a>(&self, c: &'a Compiler) -> Nav<'a> {
        let first = self.test_node.get_first_child(c).unwrap();
        Nav {
            c,
            n: if first.is_expr_result(c) {
                Some(first)
            } else {
                None
            },
        }
    }
}

#[derive(Default)]
struct TemplateAstMatcherTest {
    last_compiler: Option<Compiler>,
}

impl TemplateAstMatcherTest {
    // port: TemplateAstMatcherTest#assertMatch(Node,Node,boolean,TypeMatchingStrategy)
    fn assert_match_with(
        &mut self,
        template_root: NodeId,
        test_node: impl for<'a> Fn(&'a Compiler) -> Nav<'a>,
        should_match: bool,
        type_matching_strategy: TypeMatchingStrategy,
    ) {
        let compiler = self.last_compiler.as_mut().unwrap();
        let test_node = test_node(compiler).node();
        let template_function = template_root.get_first_child(compiler).unwrap();
        let mut matcher =
            TemplateAstMatcher::new(compiler, template_function, type_matching_strategy);
        let matched = matcher.matches(compiler, test_node);
        if matched != should_match {
            // Java builds the message eagerly; it is only observable when the assertion fails.
            let mut sb = String::new();
            sb.push_str("The nodes should");
            sb.push_str(if should_match { "" } else { " not" });
            sb.push_str(" have matched.\n");
            sb.push_str("Template node:\n");
            sb.push_str(&template_root.to_string_tree(compiler));
            sb.push('\n');
            sb.push_str("Test node:\n");
            sb.push_str(
                &test_node
                    .get_parent(compiler)
                    .unwrap()
                    .to_string_tree(compiler),
            );
            sb.push('\n');
            panic!("{sb}\nexpected: {should_match}\nbut was : {matched}");
        }
    }

    // port: TemplateAstMatcherTest#assertMatch(Node,Node)
    fn assert_match(
        &mut self,
        template_root: NodeId,
        test_node: impl for<'a> Fn(&'a Compiler) -> Nav<'a>,
    ) {
        self.assert_match_with(template_root, test_node, true, TypeMatchingStrategy::LOOSE);
    }

    // port: TemplateAstMatcherTest#assertNotMatch
    fn assert_not_match(
        &mut self,
        template_root: NodeId,
        test_node: impl for<'a> Fn(&'a Compiler) -> Nav<'a>,
    ) {
        self.assert_match_with(template_root, test_node, false, TypeMatchingStrategy::LOOSE);
    }

    /// Compiles the template and test code. The code must be compiled together using the same
    /// Compiler in order for the JsSourceMatcher to work properly.
    // port: TemplateAstMatcherTest#compile
    fn compile(&mut self, externs: &str, template: &str, code: &str) -> TestNodePair {
        let mut compiler = Compiler::new();
        compiler.disable_threads();
        let mut options = CompilerOptions::new();
        options.set_check_types(true);
        options.set_checks_only(true);
        options.set_preserve_detailed_source_info(true);

        let template_node =
            compiler.parse_file(Arc::new(SourceFile::from_code("template", template)));

        compiler.compile(
            &[Arc::new(SourceFile::from_code("externs", externs))],
            // The extra block allows easier separation of template and test
            // code.
            &[Arc::new(SourceFile::from_code(
                "test",
                format!("{{{code}}}"),
            ))],
            options,
        );
        let root = compiler.get_root().unwrap();
        let pair = TestNodePair::new(
            &compiler,
            template_node,
            root.get_last_child(&compiler).unwrap(),
        );
        self.last_compiler = Some(compiler);
        pair
    }
}

// port: TemplateAstMatcherTest#testMatches_primitives
#[test]
fn test_matches_primitives() {
    let mut t = TemplateAstMatcherTest::default();
    let mut template: String = r#"function template() {
  3;
}
"#
    .to_string();

    let mut pair = t.compile("", &template, "3");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    pair = t.compile("", &template, "5");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "var foo = 3;");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_first_child()
    });
    pair = t.compile("", &template, "obj.foo();");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });

    template = r#"function template() {
  'str';
}
"#
    .to_string();
    pair = t.compile("", &template, "'str'");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "'not_str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "var foo = 'str';");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_first_child()
    });
    pair = t.compile("", &template, "obj.foo();");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });

    template = r#"function template() {
  true;
}
"#
    .to_string();
    pair = t.compile("", &template, "true");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "!true");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "false");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "var foo = true;");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_first_child()
    });
    pair = t.compile("", &template, "!undefined");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
}

// port: TemplateAstMatcherTest#testMatches_varDeclarations
#[test]
fn test_matches_var_declarations() {
    let mut t = TemplateAstMatcherTest::default();
    let mut template: String = r#"function template() {
  var a = 3;
}
"#
    .to_string();
    let mut pair;

    pair = t.compile("", &template, "var a = 3;");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    // Make sure to let variable names differ in var declarations.
    pair = t.compile("", &template, "var b = 3;");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    pair = t.compile("", &template, "var a = 5;");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    pair = t.compile("", &template, "5;");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });

    template = r#"function template() {
  var a = {};
}
"#
    .to_string();
    pair = t.compile("", &template, "var a = {};");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    pair = t.compile("", &template, "var a = {'a': 'b'};");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });

    template = r#"function template() {
  var a = {
    'a': 'b'
  };
}
"#
    .to_string();
    pair = t.compile("", &template, "var a = {'a': 'b'};");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    pair = t.compile("", &template, "var a = {};");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_templateParameterType
#[test]
fn test_matches_template_parameter_type() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = String::new();
    let mut template: String = r#"/**
 * @param {string} foo
 */
function template(foo) {
  foo;
}
"#
    .to_string();

    let mut pair = t.compile(&externs, &template, "'str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "'different_str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "var foo = 'str';");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_first_child()
    });

    template = r#"/**
 * @param {*} foo
 */
function template(foo) {
  foo;
}
"#
    .to_string();
    pair = t.compile(&externs, &template, "'str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "3");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "new Object()");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });

    template = r#"/**
 * @param {string} foo
 * @param {number} bar
 */
function template(foo, bar) {
  bar + foo;
}
"#
    .to_string();
    pair = t.compile(&externs, &template, "'str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "3");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "new Object()");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "3 + ''");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "7 + 'str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });

    template = r#"/**
 * @param {string} string_literal_foo
 */
function template(string_literal_foo) {
  string_literal_foo;
}
"#
    .to_string();
    pair = t.compile("", &template, "\"foo\"");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "\"foo\" + \"bar\"");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "3");
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile("", &template, "var s = \"3\"; s;");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_second_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_againstUnresolvedType_fromTemplate_withImplicitNullability_noMatch
#[test]
fn test_matches_against_unresolved_type_from_template_with_implicit_nullability_no_match() {
    assert_matches_against_unresolved_type_from_template_with_type_modifier("");
}

// port: TemplateAstMatcherTest#testMatches_againstUnresolvedType_fromTemplate_withBangNullability_noMatch
#[test]
fn test_matches_against_unresolved_type_from_template_with_bang_nullability_no_match() {
    assert_matches_against_unresolved_type_from_template_with_type_modifier("!");
}

// port: TemplateAstMatcherTest#testMatches_againstUnresolvedType_fromTemplate_withQmarkNullability_noMatch
#[test]
fn test_matches_against_unresolved_type_from_template_with_qmark_nullability_no_match() {
    assert_matches_against_unresolved_type_from_template_with_type_modifier("?");
}

// port: TemplateAstMatcherTest#testMatches_againstUnresolvedType_fromTemplate_inUnion_noMatch
#[test]
fn test_matches_against_unresolved_type_from_template_in_union_no_match() {
    // TODO(b/146173738): consider making this a match. It's certain that `str` is a subtype of
    // the union `!Some.Missing.Type|string`, even though the compiler does not know what type
    // `Some.Missing.Type` is.
    assert_matches_against_unresolved_type_from_template_with_type_modifier("string|?");
}

// port: TemplateAstMatcherTest#assertMatches_againstUnresolvedType_fromTemplate_withTypeModifier
fn assert_matches_against_unresolved_type_from_template_with_type_modifier(modifier: &str) {
    let mut t = TemplateAstMatcherTest::default();
    let pair = t.compile(
        "",
        &r#"/**
 * @param {MODIFIERSome.Missing.Type} foo
 */
function template(foo) {
  foo;
}
"#
        .to_string()
        .replace("MODIFIER", modifier),
        "'str'",
    );
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_functionCall
#[test]
fn test_matches_function_call() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"function foo() {};
function bar(arg) {};
"#
    .to_string();
    let template: String = r#"function template() {
  foo();
}
"#
    .to_string();
    let mut pair = t.compile(&externs, &template, "foo();");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    pair = t.compile(&externs, &template, "bar();");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    pair = t.compile(&externs, &template, "bar(foo());");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_last_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_functionCallWithArguments
#[test]
fn test_matches_function_call_with_arguments() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @return {string} */
function foo() {};
/** @param {string} arg */
function bar(arg) {};
/**
 * @param {string} arg
 * @param {number arg2
 */
function baz(arg, arg2) {};
"#
    .to_string();
    let mut template: String = r#"function template() {
  bar('str');
}
"#
    .to_string();
    let mut pair = t.compile(&externs, &template, "foo();");
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "bar();");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "bar('str');");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "bar(foo());");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c)
            .get_first_child()
            .get_last_child()
    });

    template = r#"/** @param {string} str */
function template(str) {
  bar(str);
}
"#
    .to_string();
    pair = t.compile(&externs, &template, "foo();");
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "bar('str');");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "bar('str' + 'other_str');");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "bar(String(3));");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });

    template = r#"/**
 * @param {string} str
 * @param {number} num
 */
function template(str, num) {
  baz(str, num);
}
"#
    .to_string();
    pair = t.compile(&externs, &template, "foo();");
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "baz('str', 3);");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "baz('str' + 'other_str', 3 + 4);");
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_methodCall
#[test]
fn test_matches_method_call() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @return {string} */
function foo() {};
"#
    .to_string();
    let template: String = r#"/**
 * @param {string} str
 */
function template(str) {
  str.toString();
}
"#
    .to_string();

    let mut pair = t.compile(&externs, &template, "'str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "'str'.toString()");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(&externs, &template, "foo().toString()");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_methodCallWithArguments
#[test]
fn test_matches_method_call_with_arguments() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @constructor */
function AppContext() {}
AppContext.prototype.init = function() {};
/**
 * @param {string} arg
 */
AppContext.prototype.get = function(arg) {};
/**
 * @param {string} arg
 */
AppContext.prototype.getOrNull = function(arg) {};
"#
    .to_string();
    let mut template: String = r#"/**
 * @param {AppContext} context
 */
function template(context) {
  context.init();
}
"#
    .to_string();

    let mut pair = t.compile(&externs, &template, "'str'");
    t.assert_not_match(pair.template_node, |c| pair.get_test_expr_result_root(c));
    t.assert_not_match(pair.template_node, |c| {
        pair.get_test_expr_result_root(c).get_first_child()
    });
    pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.init();",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
    pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.get('str');",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });

    template = r#"/**
 * @param {AppContext} context
 * @param {string} service
 */
function template(context, service) {
  context.get(service);
}
"#
    .to_string();

    pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.init();",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
    pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.get('s');",
    );
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
    pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.get(3);",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
    pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.getOrNull('s');",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_instantiation
#[test]
fn test_matches_instantiation() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @constructor */
function AppContext() {}
"#
    .to_string();
    let template: String = r#"function template() {
  new AppContext();
}
"#
    .to_string();

    let mut pair = t.compile(&externs, &template, "var foo = new AppContext()");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_first_child()
    });
    pair = t.compile(&externs, &template, "var foo = new Object()");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_first_first_child()
            .get_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_propertyAccess
#[test]
fn test_matches_property_access() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @constructor */
function AppContext() {}
/** @type {string} */
AppContext.prototype.location;
"#
    .to_string();
    let template: String = r#"/**
 * @param {AppContext} context
 */
function template(context) {
  context.location;
}
"#
    .to_string();

    let pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.location = '3';",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_multiLineTemplates
#[test]
fn test_matches_multi_line_templates() {
    let mut t = TemplateAstMatcherTest::default();
    let mut externs: String = r#"/** @constructor */
function AppContext() {}
/** @type {string} */
AppContext.prototype.location;
"#
    .to_string();
    let mut template: String = r#"/**
 * @param {AppContext} context
 * @param {string} str
 */
function template(context, str) {
  context.location = str;
  delete context.location;
}
"#
    .to_string();

    let mut pair = t.compile(
        &externs,
        &template,
        "var context = new AppContext(); context.location = '3';",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_first_child()
    });

    pair = t.compile(
        &externs,
        &template,
        "var ac = new AppContext(); ac.location = '3'; delete ac.location;",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_second_child()
    });

    // Ensure that if a variable is declared within the template and reused
    // across multiple statements, ensure that the test code matches the same
    // pattern. For example:
    // var a = b();
    // fn(a);
    externs = r#"/** @param {string} arg */
function bar(arg) {};
"#
    .to_string();
    template = r#"function template() {
  var a = 'string';
  bar(a);
}
"#
    .to_string();

    pair = t.compile(&externs, &template, "var loc = 'string'; bar(loc);");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    pair = t.compile(&externs, &template, "var loc = 'string'; bar('foo');");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    pair = t.compile(
        &externs,
        &template,
        "var baz = 'qux'; var loc = 'string'; bar(baz);",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_second_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_subclasses
#[test]
fn test_matches_subclasses() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @constructor */
function AppContext() {}
/** @type {string} */
AppContext.prototype.location;
/**
 * @constructor
 * @extends {AppContext}
 */
function SubAppContext() {}
"#
    .to_string();
    let template: String = r#"/**
 * @param {AppContext} context
 * @param {string} str
 */
function template(context, str) {
  context.location = str;
}
"#
    .to_string();

    let pair = t.compile(
        &externs,
        &template,
        "var context = new SubAppContext(); context.location = '3';",
    );
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
}

// port: TemplateAstMatcherTest#testMatches_nonDefaultStrategy
#[test]
fn test_matches_non_default_strategy() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @constructor */
function AppContext() {}
/** @type {string} */
AppContext.prototype.location;
/**
 * @constructor
 * @extends {AppContext}
 */
function SubAppContext() {}
var context = new AppContext();
var subContext = new SubAppContext();
"#
    .to_string();
    let template: String = r#"/**
 * @param {!AppContext} context
 * @param {string} str
 */
function template(context, str) {
  context.location = str;
}
"#
    .to_string();

    let pair = t.compile(&externs, &template, "subContext.location = '3';");
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
    t.assert_match_with(
        pair.template_node,
        |c| {
            Nav::of(c, pair.test_node)
                .get_last_child()
                .get_first_child()
        },
        false,
        TypeMatchingStrategy::EXACT,
    );
}

// port: TemplateAstMatcherTest#testMatches_namespace_method
#[test]
fn test_matches_namespace_method() {
    let mut t = TemplateAstMatcherTest::default();
    let externs: String = r#"/** @const */ const ns = {};
/** @const */ ns.sub = {};
/** @return {boolean} */ ns.sub.method = function() {};
"#
    .to_string();

    let template: String = r#"/**
 * @param {typeof ns.sub} target
 */
function template(target) {
  target.method();
}
"#
    .to_string();

    let pair = t.compile(&externs, &template, "var alias = ns.sub; alias.method();");
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_child()
    });
    t.assert_not_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node).get_first_first_child()
    });
    t.assert_match(pair.template_node, |c| {
        Nav::of(c, pair.test_node)
            .get_last_child()
            .get_first_child()
    });
}
