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
//   test/com/google/javascript/jscomp/DevirtualizeMethodsTest.java.

//! Port of DevirtualizeMethodsTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::{
    Compiler, devirtualize_methods::DevirtualizeMethods, optimize_calls::OptimizeCalls,
};
use closure_rhino::{
    check_not_null, check_state,
    jscomp_colors::standard_colors,
    node::{Ast, NodeId},
};
use closure_testing::testing::js_chunk_graph_builder::JSChunkGraphBuilder;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
#[allow(unused_imports)]
use optimize_calls_support::*;

// port: DevirtualizeMethodsTest#EXTERNAL_SYMBOLS (DEFAULT_EXTERNS + this)
const EXTERNAL_SYMBOLS: &str = "var extern;extern.externalMethod";

/// Combine source strings using ';' as the separator.
// port: DevirtualizeMethodsTest#semicolonJoin
fn semicolon_join(parts: &[&str]) -> String {
    parts.join(";")
}

/// `AccessorSummary.PropertyAccessKind.<name>` as the harness's enum value.
fn property_access_kind(name: &str) -> DslValue {
    DslValue::Enum {
        class: "com.google.javascript.jscomp.AccessorSummary$PropertyAccessKind".into(),
        name: name.into(),
    }
}

/// Inputs for declaration used as an r-value tests.
// port: DevirtualizeMethodsTest.NoRewriteDeclarationUsedAsRValue
struct NoRewriteDeclarationUsedAsRValue;

impl NoRewriteDeclarationUsedAsRValue {
    const DECL: &str = "a.prototype.foo = function() {}";
    const CALL: &str = "o.foo()";
}

/// Inputs for object literal tests.
// port: DevirtualizeMethodsTest.NoRewritePrototypeObjectLiteralsTestInput
struct NoRewritePrototypeObjectLiteralsTestInput;

impl NoRewritePrototypeObjectLiteralsTestInput {
    const REGULAR: &str = "b.prototype.foo = function() { return 1; }";
    const OBJ_LIT: &str = "a.prototype = {foo : function() { return 2; }}";
    const CALL: &str = "o.foo()";
}

/// Inputs for invalidating reference tests.
// port: DevirtualizeMethodsTest.NoRewriteNonCallReferenceTestInput
struct NoRewriteNonCallReferenceTestInput;

impl NoRewriteNonCallReferenceTestInput {
    const BASE: &str = concat!(
        "function a() {}\n",
        "a.prototype.foo = function() {return this.x};\n",
        "var o = new a;\n"
    );
}

/// Inputs for nested definition tests.
// port: DevirtualizeMethodsTest.NoRewriteNestedFunctionTestInput
struct NoRewriteNestedFunctionTestInput;

impl NoRewriteNestedFunctionTestInput {
    const PREFIX: &str = "a.prototype.foo = function() {";
    const SUFFIX: &str = "o.foo()";
    const INNER: &str = "a.prototype.bar = function() {}; o.bar()";
    const EXPECTED_PREFIX: &str = concat!(
        "var JSCompiler_StaticMethods_foo=\n",
        "function(JSCompiler_StaticMethods_foo$self){\n"
    );
    const EXPECTED_SUFFIX: &str = "JSCompiler_StaticMethods_foo(o)";
}

// port: DevirtualizeMethodsTest.ModuleTestInput
struct ModuleTestInput;

impl ModuleTestInput {
    const DEFINITION: &str = "a.prototype.foo = function() {}";
    const USE: &str = "x.foo()";
    const REWRITTEN_DEFINITION: &str = concat!(
        "var JSCompiler_StaticMethods_foo=\n",
        "function(JSCompiler_StaticMethods_foo$self){}\n"
    );
    const REWRITTEN_USE: &str = "JSCompiler_StaticMethods_foo(x)";
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: DevirtualizeMethodsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(
            OptimizeCalls::builder()
                .set_compiler(&compiler.borrow())
                .set_consider_externs(false)
                .add_pass(Box::new(DevirtualizeMethods::new()))
                .build(),
        ))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: DevirtualizeMethodsTest#DevirtualizeMethodsTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(default_externs_plus(EXTERNAL_SYMBOLS));
        harness.set_up();
        harness.enable_normalize().unwrap(); // Required for `OptimizeCalls`.
        harness.disable_type_check().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("DevirtualizeMethodsTest"),
            },
        }
    }
}

impl Test {
    // port: DevirtualizeMethodsTest#testNoRewriteIfDefinitionSiteBetween
    fn test_no_rewrite_if_definition_site_between(&mut self, prefix: &str, suffix: &str) {
        self.test_same(
            &concat!(
                "function a(){}\n",
                "_PREFIX_a.prototype.foo = function() {return this.x}_SUFFIX_;\n",
                "var o = new a;\n",
                "o.foo()\n"
            )
            .replace("_PREFIX_", prefix)
            .replace("_SUFFIX_", suffix),
        );
    }

    // port: DevirtualizeMethodsTest#testRewritePreservesFunctionKind
    fn test_rewrite_preserves_function_kind(&mut self, fn_keyword: &str) {
        self.test_parts(vec![
            srcs(
                &concat!(
                    "function a(){}\n",
                    "a.prototype.foo = FN_KEYWORD() { return 0; };\n",
                    "\n",
                    "(new a()).foo();\n"
                )
                .replace("FN_KEYWORD", fn_keyword),
            ),
            expected(
                &concat!(
                    "function a(){}\n",
                    "var JSCompiler_StaticMethods_foo =\n",
                    "    FN_KEYWORD(JSCompiler_StaticMethods_foo$self) { return 0; };\n",
                    "\n",
                    "JSCompiler_StaticMethods_foo(new a());\n"
                )
                .replace("FN_KEYWORD", fn_keyword),
            ),
        ]);
    }

    // port: DevirtualizeMethodsTest#testRewrite_definedUsingAssignment_staticMethod_onFunction(String)
    fn test_rewrite_defined_using_assignment_static_method_on_function(
        &mut self,
        annotation: &str,
    ) {
        self.harness.disable_compare_js_doc().unwrap(); // multistage compilation simplifies jsdoc
        self.test_parts(vec![
            srcs(
                &[
                    annotation,
                    concat!(
                        "function Foo() { }\n",
                        "Foo.bar = function() { return 5; }\n",
                        "\n",
                        "// We need at least one normal call to trigger rewriting.\n",
                        "x.bar();\n"
                    ),
                ]
                .concat(),
            ),
            expected(concat!(
                "function Foo() { }\n",
                "\n",
                "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
                "  return 5;\n",
                "};\n",
                "\n",
                "JSCompiler_StaticMethods_bar(x);\n"
            )),
        ]);
    }
}

// port: DevirtualizeMethodsTest#testRewritePrototypeMethodsWithCorrectColors
#[test]
fn test_rewrite_prototype_methods_with_correct_colors() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    let input = concat!(
        "/** @constructor */\n",
        "function A() { this.x = 3; }\n",
        "A: A;\n",
        "/** @return {number} */\n",
        "A.prototype.foo = function() { return this.x; };\n",
        "/** @param {number} p\n",
        "    @return {number} */\n",
        "A.prototype.bar = function(p) { return this.x; };\n",
        "A.prototype.baz = function() {};\n",
        "var o = new A();\n",
        "FOO_RESULT: o.foo();\n",
        "BAR_RESULT: o.bar(2);\n",
        "BAZ_RESULT: o.baz()\n"
    );
    let expected = concat!(
        "/** @constructor */\n",
        "function A(){ this.x = 3; }\n",
        "A: A;\n",
        "var JSCompiler_StaticMethods_foo =\n",
        "  function(JSCompiler_StaticMethods_foo$self) {\n",
        "    return JSCompiler_StaticMethods_foo$self.x\n",
        "  };\n",
        "var JSCompiler_StaticMethods_bar =\n",
        "  function(JSCompiler_StaticMethods_bar$self, p) {\n",
        "    return JSCompiler_StaticMethods_bar$self.x\n",
        "  };\n",
        "var JSCompiler_StaticMethods_baz =\n",
        "  function(JSCompiler_StaticMethods_baz$self) {};\n",
        "var o = new A();\n",
        "FOO_RESULT: JSCompiler_StaticMethods_foo(o);\n",
        "BAR_RESULT: JSCompiler_StaticMethods_bar(o, 2);\n",
        "BAZ_RESULT: JSCompiler_StaticMethods_baz(o)\n"
    );

    t.harness.enable_type_check().unwrap();
    t.harness.replace_types_with_colors().unwrap();
    t.harness.disable_compare_js_doc().unwrap();

    t.test(input, expected);
    t.check_color_of_rewritten_methods();
}

impl Test {
    // port: DevirtualizeMethodsTest#checkColorOfRewrittenMethods
    fn check_color_of_rewritten_methods(&self) {
        let compiler = self.harness.get_last_compiler().unwrap();
        let compiler = compiler.borrow();
        let ast = &compiler.ast;
        let first_child_color = |label: &str| {
            get_labelled_expression(&compiler, label)
                .get_first_child(ast)
                .unwrap()
                .get_color(ast)
        };
        let foo_type = first_child_color("FOO_RESULT");
        let bar_type = first_child_color("BAR_RESULT");
        let baz_type = first_child_color("BAZ_RESULT");
        let foo_result_type = get_labelled_expression(&compiler, "FOO_RESULT").get_color(ast);
        let bar_result_type = get_labelled_expression(&compiler, "BAR_RESULT").get_color(ast);
        let baz_result_type = get_labelled_expression(&compiler, "BAZ_RESULT").get_color(ast);

        assert_eq!(foo_result_type, Some(standard_colors::NUMBER.clone()));
        assert_eq!(bar_result_type, Some(standard_colors::NUMBER.clone()));
        assert_eq!(baz_result_type, Some(standard_colors::NULL_OR_VOID.clone()));

        assert_eq!(foo_type, Some(standard_colors::TOP_FUNCTION.clone()));
        assert_eq!(bar_type, Some(standard_colors::TOP_FUNCTION.clone()));
        assert_eq!(baz_type, Some(standard_colors::TOP_FUNCTION.clone()));
    }
}

// port: DevirtualizeMethodsTest#getLabelledExpression
fn get_labelled_expression(compiler: &Compiler, label: &str) -> NodeId {
    let root = compiler.get_js_root().unwrap();

    check_not_null!(
        get_labelled_expression_if_present(&compiler.ast, label, root),
        "Could not find statement matching label %s",
        label
    )
}

// port: DevirtualizeMethodsTest#getLabelledExpressionIfPresent
fn get_labelled_expression_if_present(ast: &Ast, label: &str, root: NodeId) -> Option<NodeId> {
    if root.is_label(ast) && root.get_first_child(ast).unwrap().get_string(ast) == label {
        let labelled_block = root.get_second_child(ast).unwrap();
        check_state!(
            labelled_block.is_block(ast),
            "%s",
            labelled_block.to_string(ast)
        );
        check_state!(
            labelled_block.has_one_child(ast)
                && labelled_block.get_only_child(ast).is_expr_result(ast),
            "Unexpected children of BLOCK %s",
            labelled_block.to_string(ast)
        );
        return Some(labelled_block.get_only_child(ast).get_only_child(ast));
    }
    let mut child = root.get_first_child(ast);
    while let Some(c) = child {
        let possible_match = get_labelled_expression_if_present(ast, label, c);
        if possible_match.is_some() {
            return possible_match;
        }
        child = c.get_next(ast);
    }
    None
}

// port: DevirtualizeMethodsTest#testRewriteChained
#[test]
fn test_rewrite_chained() {
    let mut t = Test::new();
    let source = concat!(
        "A.prototype.foo = function(){return this.b};\n",
        "B.prototype.bar = function(){};\n",
        "o.foo().bar()\n"
    );
    let expected_js = concat!(
        "var JSCompiler_StaticMethods_foo =\n",
        "function(JSCompiler_StaticMethods_foo$self) {\n",
        "  return JSCompiler_StaticMethods_foo$self.b\n",
        "};\n",
        "var JSCompiler_StaticMethods_bar =\n",
        "function(JSCompiler_StaticMethods_bar$self) {\n",
        "};\n",
        "JSCompiler_StaticMethods_bar(JSCompiler_StaticMethods_foo(o))\n"
    );
    t.test(source, expected_js);
}

// port: DevirtualizeMethodsTest#testRewriteDeclIsExpressionStatement
#[test]
fn test_rewrite_decl_is_expression_statement() {
    let mut t = Test::new();
    t.test(
        &semicolon_join(&[
            NoRewriteDeclarationUsedAsRValue::DECL,
            NoRewriteDeclarationUsedAsRValue::CALL,
        ]),
        concat!(
            "var JSCompiler_StaticMethods_foo =\n",
            "function(JSCompiler_StaticMethods_foo$self) {};\n",
            "JSCompiler_StaticMethods_foo(o)\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testNoRewriteDeclUsedAsAssignmentRhs
#[test]
fn test_no_rewrite_decl_used_as_assignment_rhs() {
    let mut t = Test::new();
    t.test_same(&semicolon_join(&[
        &["var c = ", NoRewriteDeclarationUsedAsRValue::DECL].concat(),
        NoRewriteDeclarationUsedAsRValue::CALL,
    ]));
}

// port: DevirtualizeMethodsTest#testNoRewriteDeclUsedAsCallArgument
#[test]
fn test_no_rewrite_decl_used_as_call_argument() {
    let mut t = Test::new();
    t.test_same(&semicolon_join(&[
        &["f(", NoRewriteDeclarationUsedAsRValue::DECL, ")"].concat(),
        NoRewriteDeclarationUsedAsRValue::CALL,
    ]));
}

// port: DevirtualizeMethodsTest#testRewrite_ifDefined_unconditionally
#[test]
fn test_rewrite_if_defined_unconditionally() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function a(){}\n",
            "a.prototype.foo = function() {return this.x};\n",
            "var o = new a;\n",
            "o.foo()\n"
        ),
        concat!(
            "function a(){}\n",
            "var JSCompiler_StaticMethods_foo =\n",
            "function(JSCompiler_StaticMethods_foo$self) {\n",
            "  return JSCompiler_StaticMethods_foo$self.x\n",
            "};\n",
            "var o = new a;\n",
            "JSCompiler_StaticMethods_foo(o);\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#noRewrite_forPropertyNamesAccessedReflectively
#[test]
fn no_rewrite_for_property_names_accessed_reflectively() {
    let mut t = Test::new();
    t.test_parts(vec![
        externs("function use() {}"),
        srcs(concat!(
            "class C { m() {} n() {} }\n",
            "const c = new C();\n",
            "c.m();\n",
            "c.n();\n",
            "// this call should prevent devirtualizing m() but not n()\n",
            "use(C.prototype, goog.reflect.objectProperty('m', C.prototype));\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_n = function(JSCompiler_StaticMethods_n$self) {};\n",
            "class C { m() {} }\n",
            "const c = new C();\n",
            "c.m();\n",
            "JSCompiler_StaticMethods_n(c);\n",
            "use(C.prototype, goog.reflect.objectProperty('m', C.prototype));\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedIn_ifScope
#[test]
fn test_no_rewrite_if_defined_in_if_scope() {
    let mut t = Test::new();
    t.test_no_rewrite_if_definition_site_between("if (true) ", "");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedIn_loopScope
#[test]
fn test_no_rewrite_if_defined_in_loop_scope() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_no_rewrite_if_definition_site_between("while (true) ", "");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedIn_switchScope
#[test]
fn test_no_rewrite_if_defined_in_switch_scope() {
    let mut t = Test::new();
    t.test_no_rewrite_if_definition_site_between("switch (true) { case true: ", "; }");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedIn_functionScope
#[test]
fn test_no_rewrite_if_defined_in_function_scope() {
    let mut t = Test::new();
    t.test_no_rewrite_if_definition_site_between("function f() { ", "; }");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedIn_arrowFunctionScope
#[test]
fn test_no_rewrite_if_defined_in_arrow_function_scope() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_no_rewrite_if_definition_site_between("() => ", "");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedIn_blockScope
#[test]
fn test_no_rewrite_if_defined_in_block_scope() {
    let mut t = Test::new();
    // Some declarations are block scoped in ES6 and so might have different values. This could make
    // multiple definitions with identical node structure behave differently.
    t.test_no_rewrite_if_definition_site_between("{ ", "; }");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedBy_andExpression_withLiteral
#[test]
fn test_no_rewrite_if_defined_by_and_expression_with_literal() {
    let mut t = Test::new();
    // TODO(nickreid): This may be unnecessarily restrictive. We could probably rely on the
    // definitions being identical or not to filter this case.
    t.test_no_rewrite_if_definition_site_between("", " && function() {}");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedBy_andExpression_withReference
#[test]
fn test_no_rewrite_if_defined_by_and_expression_with_reference() {
    let mut t = Test::new();
    t.test_no_rewrite_if_definition_site_between("", " && bar");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedBy_orExpression_withLiteral
#[test]
fn test_no_rewrite_if_defined_by_or_expression_with_literal() {
    let mut t = Test::new();
    // TODO(nickreid): This may be unnecessarily restrictive. We could probably rely on the
    // definitions being identical or not to filter this case.
    t.test_no_rewrite_if_definition_site_between("", " || function() {}");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedBy_orExpression_withReference
#[test]
fn test_no_rewrite_if_defined_by_or_expression_with_reference() {
    let mut t = Test::new();
    t.test_no_rewrite_if_definition_site_between("", " || bar");
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedBy_ternaryExpression_withLiteral
#[test]
fn test_no_rewrite_if_defined_by_ternary_expression_with_literal() {
    let mut t = Test::new();
    // TODO(nickreid): This may be unnecessarily restrictive. We could probably rely on the
    // definitions being identical or not to filter this case.
    // Make the functions look different, just to be safe.
    t.test_no_rewrite_if_definition_site_between(
        "",
        " ? function() { this.a; } : function() { this.b; }",
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedBy_ternaryExpression_withReference
#[test]
fn test_no_rewrite_if_defined_by_ternary_expression_with_reference() {
    let mut t = Test::new();
    t.test_no_rewrite_if_definition_site_between("", " ? function() { } : bar");
}

// port: DevirtualizeMethodsTest#testRewrite_preservesAsync
#[test]
fn test_rewrite_preserves_async() {
    let mut t = Test::new();
    t.test_rewrite_preserves_function_kind("async function");
}

// port: DevirtualizeMethodsTest#testRewrite_preservesAsyncGenerator
#[test]
fn test_rewrite_preserves_async_generator() {
    let mut t = Test::new();
    t.test_rewrite_preserves_function_kind("async function*");
}

// port: DevirtualizeMethodsTest#testRewrite_preservesGenerator
#[test]
fn test_rewrite_preserves_generator() {
    let mut t = Test::new();
    t.test_rewrite_preserves_function_kind("function*");
}

// port: DevirtualizeMethodsTest#testRewrite_replacesMultipleThisRefreences
#[test]
fn test_rewrite_replaces_multiple_this_refreences() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  a() {\n",
            "     alert(this);\n",
            "     alert(this, this);\n",
            "  }\n",
            "}\n",
            "new Foo().a();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_a = function(JSCompiler_StaticMethods_a$self) {\n",
            "  alert(JSCompiler_StaticMethods_a$self);\n",
            "  alert(JSCompiler_StaticMethods_a$self, JSCompiler_StaticMethods_a$self);\n",
            "};\n",
            "\n",
            "class Foo { }\n",
            "JSCompiler_StaticMethods_a(new Foo());\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_defaultParams_usesThisReference
#[test]
fn test_rewrite_default_params_uses_this_reference() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar(y = this) { return y; }\n",
            "}\n",
            "\n",
            "// We need at least one normal call to trigger rewriting.\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(\n",
            "    JSCompiler_StaticMethods_bar$self,\n",
            "    y = JSCompiler_StaticMethods_bar$self) {\n",
            "  return y;\n",
            "};\n",
            "class Foo { }\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_defaultParams_usesSuperReference
#[test]
fn test_no_rewrite_default_params_uses_super_reference() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  bar(y = super.toString()) { return y; }\n",
        "}\n",
        "\n",
        "// We need at least one normal call to trigger rewriting.\n",
        "x.bar();\n"
    ));
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifDefinedByArrow
#[test]
fn test_no_rewrite_if_defined_by_arrow() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_same(concat!(
        "function a(){};\n",
        "a.prototype.foo = () => 5;\n",
        "\n",
        "(new a()).foo();\n"
    ));
}

// port: DevirtualizeMethodsTest#testNoRewrite_namespaceFunctions
#[test]
fn test_no_rewrite_namespace_functions() {
    let mut t = Test::new();
    let source = "function a(){}; a.foo = function() {return this.x}; a.foo()";
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testRewrite_ifMultipleIdenticalDefinitions
#[test]
fn test_rewrite_if_multiple_identical_definitions() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function A(){};\n",
            "A.prototype.getFoo = function() { return 1; };\n",
            "\n",
            "function B(){};\n",
            "B.prototype.getFoo = function() { return 1; };\n",
            "\n",
            "x.getFoo();\n"
        ),
        concat!(
            "function A() {};\n",
            "var JSCompiler_StaticMethods_getFoo =\n",
            "    function(JSCompiler_StaticMethods_getFoo$self) { return 1; };\n",
            "\n",
            "function B(){};\n",
            "B.prototype.getFoo = function() { return 1 }; // Dead definition.\n",
            "\n",
            "JSCompiler_StaticMethods_getFoo(x);\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testRewrite_ifMultipleIdenticalDefinitions_ensuresDefinitionBeforeInvocations
#[test]
fn test_rewrite_if_multiple_identical_definitions_ensures_definition_before_invocations() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function A() {};\n",
            "A.prototype.getFoo = function() { return 1; };\n",
            "\n",
            "x.getFoo();\n",
            "\n",
            "function B() {};\n",
            "B.prototype.getFoo = function() { return 1; };\n",
            "\n",
            "y.getFoo();\n"
        ),
        concat!(
            "function A() {};\n",
            "var JSCompiler_StaticMethods_getFoo =\n",
            "    function(JSCompiler_StaticMethods_getFoo$self) { return 1; };\n",
            "\n",
            "JSCompiler_StaticMethods_getFoo(x);\n",
            "\n",
            "function B() {};\n",
            "B.prototype.getFoo=function() { return 1; }; // Dead definition.\n",
            "\n",
            "JSCompiler_StaticMethods_getFoo(y);\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testRewrite_ifMultipleIdenticalDefinitions_withThis
#[test]
fn test_rewrite_if_multiple_identical_definitions_with_this() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function A() {};\n",
            "A.prototype.getFoo = function() { return this._foo + 1; };\n",
            "\n",
            "function B() {};\n",
            "B.prototype.getFoo = function() { return this._foo + 1; };\n",
            "\n",
            "x.getFoo();\n"
        ),
        concat!(
            "function A() {};\n",
            "var JSCompiler_StaticMethods_getFoo =\n",
            "    function(JSCompiler_StaticMethods_getFoo$self) {\n",
            "      return JSCompiler_StaticMethods_getFoo$self._foo + 1\n",
            "    };\n",
            "\n",
            "function B() {};\n",
            "B.prototype.getFoo = function() { return this._foo + 1 }; // Dead definition.\n",
            "\n",
            "JSCompiler_StaticMethods_getFoo(x);\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testRewrite_ifMultipleIdenticalDefinitions_withLocalNames
#[test]
fn test_rewrite_if_multiple_identical_definitions_with_local_names() {
    let mut t = Test::new();
    // This case is included for completeness. `Normalization` is a prerequisite for this pass so
    // the naming conflict is resolved before devirtualization even begins. The change in names
    // invalidates devirtualization by making the definition subtrees unequal.
    t.test(
        concat!(
            "function A() {};\n",
            "A.prototype.getFoo = function f() { return f.prop; };\n",
            "\n",
            "function B() {};\n",
            "B.prototype.getFoo = function f() { return f.prop; };\n",
            "\n",
            "x.getFoo();\n"
        ),
        concat!(
            "function A() {};\n",
            "A.prototype.getFoo = function f() { return f.prop; };\n",
            "\n",
            "function B() {};\n",
            "B.prototype.getFoo = function f$jscomp$1() { return f$jscomp$1.prop; };\n",
            "\n",
            "x.getFoo();\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_ifMultipleDistinctDefinitions
#[test]
fn test_no_rewrite_if_multiple_distinct_definitions() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function A(){}; A.prototype.getFoo = function() { return 1; };\n",
        "function B(){}; B.prototype.getFoo = function() { return 2; };\n",
        "var x = Math.random() ? new A() : new B();\n",
        "alert(x.getFoo());\n"
    ));
}

// port: DevirtualizeMethodsTest#testRewritePrototypeNoObjectLiterals
#[test]
fn test_rewrite_prototype_no_object_literals() {
    let mut t = Test::new();
    t.test(
        &semicolon_join(&[
            NoRewritePrototypeObjectLiteralsTestInput::REGULAR,
            NoRewritePrototypeObjectLiteralsTestInput::CALL,
        ]),
        concat!(
            "var JSCompiler_StaticMethods_foo =\n",
            "function(JSCompiler_StaticMethods_foo$self) { return 1; };\n",
            "JSCompiler_StaticMethods_foo(o)\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingProtoObjectLit
#[test]
fn test_rewrite_defined_using_proto_object_lit() {
    let mut t = Test::new();
    t.test(
        &semicolon_join(&[
            NoRewritePrototypeObjectLiteralsTestInput::OBJ_LIT,
            NoRewritePrototypeObjectLiteralsTestInput::CALL,
        ]),
        concat!(
            "var JSCompiler_StaticMethods_foo = function(JSCompiler_StaticMethods_foo$self) {\n",
            "  return 2;\n",
            "};\n",
            "a.prototype={};\n",
            "\n",
            "JSCompiler_StaticMethods_foo(o)\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_multipleDefinitions_definedUsingProtoObjectLit_definedUsingGetProp
#[test]
fn test_no_rewrite_multiple_definitions_defined_using_proto_object_lit_defined_using_get_prop() {
    let mut t = Test::new();
    t.test_same(&semicolon_join(&[
        NoRewritePrototypeObjectLiteralsTestInput::OBJ_LIT,
        NoRewritePrototypeObjectLiteralsTestInput::REGULAR,
        NoRewritePrototypeObjectLiteralsTestInput::CALL,
    ]));
}

// port: DevirtualizeMethodsTest#testNoRewrite_noDefinition
#[test]
fn test_no_rewrite_no_definition() {
    let mut t = Test::new();
    t.test_same("a.externalMethod()");
}

// port: DevirtualizeMethodsTest#testNoRewrite_externMethod
#[test]
fn test_no_rewrite_extern_method() {
    let mut t = Test::new();
    t.test_same_parts(vec![
        externs("A.prototype.externalMethod = function(){};"),
        srcs("o.externalMethod()"),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_exportedMethod_viaCodingConvention
#[test]
fn test_no_rewrite_exported_method_via_coding_convention() {
    let mut t = Test::new();
    // no rewriting without call; regardless of leading underscore
    t.test_same("a.prototype.foo = function() {};");
    t.test_same("a.prototype._foo = function() {};");
    // renames as expected
    t.test(
        "function a() {} a.prototype.foo = function() {}; let o = new a; o.foo();",
        concat!(
            "function a() {}\n",
            "var JSCompiler_StaticMethods_foo =\n",
            "function(JSCompiler_StaticMethods_foo$self) {};\n",
            "let o = new a;\n",
            "JSCompiler_StaticMethods_foo(o);\n"
        ),
    );
    // no renaming, as leading _ indicates exported symbol
    t.test_same("function a() {} a.prototype._foo = function() {}; let o = new a; o._foo();");
}

// port: DevirtualizeMethodsTest#testRewriteNoVarArgs
#[test]
fn test_rewrite_no_var_args() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "o.foo()\n"
    );
    let expected_js = concat!(
        "function a(){}\n",
        "var JSCompiler_StaticMethods_foo =\n",
        "  function(JSCompiler_StaticMethods_foo$self, args) {return args};\n",
        "var o = new a;\n",
        "JSCompiler_StaticMethods_foo(o)\n"
    );
    t.test(source, expected_js);
}

// port: DevirtualizeMethodsTest#testRewrite_argInsideOptionalChainingCall
#[test]
fn test_rewrite_arg_inside_optional_chaining_call() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "a?.(o.foo())\n"
    );
    let expected_js = concat!(
        "function a(){}\n",
        "var JSCompiler_StaticMethods_foo =\n",
        "  function(JSCompiler_StaticMethods_foo$self, args) {return args};\n",
        "var o = new a;\n",
        "a?.(JSCompiler_StaticMethods_foo(o))\n"
    );
    t.test(source, expected_js);
}

// port: DevirtualizeMethodsTest#testRewrite_lhsOfOptionalChainingCall
#[test]
fn test_rewrite_lhs_of_optional_chaining_call() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "o.foo()?.a\n"
    );
    let expected_js = concat!(
        "function a(){}\n",
        "var JSCompiler_StaticMethods_foo =\n",
        "  function(JSCompiler_StaticMethods_foo$self, args) {return args};\n",
        "var o = new a;\n",
        "JSCompiler_StaticMethods_foo(o)?.a\n"
    );
    t.test(source, expected_js);
}

// port: DevirtualizeMethodsTest#testNoRewriteVarArgs
#[test]
fn test_no_rewrite_var_args() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(var_args) {return arguments};\n",
        "var o = new a;\n",
        "o.foo()\n"
    );
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testRewrite_callReference
#[test]
fn test_rewrite_call_reference() {
    let mut t = Test::new();
    let expected_js = concat!(
        "function a(){}\n",
        "var JSCompiler_StaticMethods_foo =\n",
        "function(JSCompiler_StaticMethods_foo$self) {\n",
        "  return JSCompiler_StaticMethods_foo$self.x\n",
        "};\n",
        "var o = new a;\n",
        "JSCompiler_StaticMethods_foo(o);\n"
    );
    t.test(
        &[NoRewriteNonCallReferenceTestInput::BASE, "o.foo()"].concat(),
        expected_js,
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_noReferences
#[test]
fn test_no_rewrite_no_references() {
    let mut t = Test::new();
    t.test_same(NoRewriteNonCallReferenceTestInput::BASE);
}

// port: DevirtualizeMethodsTest#testNoRewrite_nonCallReference_viaGetprop
#[test]
fn test_no_rewrite_non_call_reference_via_getprop() {
    let mut t = Test::new();
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!(
                "o.foo(); // We need at least one normal call to trigger rewriting.\n",
                "o.foo;\n"
            ),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_nonCallReference_viaDestructuring
#[test]
fn test_no_rewrite_non_call_reference_via_destructuring() {
    let mut t = Test::new();
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!(
                "o.foo(); // We need at least one normal call to trigger rewriting.\n",
                "const {foo: x} = o;\n"
            ),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_nonCallReference_viaGetprop_usingFnCall
#[test]
fn test_no_rewrite_non_call_reference_via_getprop_using_fn_call() {
    let mut t = Test::new();
    // TODO(nickreid): Add rewriting support for this.
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!(
                "o.foo(); // We need at least one normal call to trigger rewriting.\n",
                "o.foo.call(null);\n"
            ),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_nonCallReference_viaGetprop_usingFnApply
#[test]
fn test_no_rewrite_non_call_reference_via_getprop_using_fn_apply() {
    let mut t = Test::new();
    // TODO(nickreid): Add rewriting support for this.
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!(
                "o.foo(); // We need at least one normal call to trigger rewriting.\n",
                "o.foo.apply(null);\n"
            ),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_nonCallReference_viaGetprop_asArgument
#[test]
fn test_no_rewrite_non_call_reference_via_getprop_as_argument() {
    let mut t = Test::new();
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!(
                "o.foo(); // We need at least one normal call to trigger rewriting.\n",
                "bar(o.foo, null);\n"
            ),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_nonCallReference_viaTaggedTemplateString
#[test]
fn test_no_rewrite_non_call_reference_via_tagged_template_string() {
    let mut t = Test::new();
    // TODO(nickreid): Add rewriting support for this.
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!(
                "o.foo(); // We need at least one normal call to trigger rewriting.\n",
                "o.foo`Hello World!`;\n"
            ),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_optChain
#[test]
fn test_no_rewrite_opt_chain() {
    let mut t = Test::new();
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!(
                "o.foo(); // We need at least one normal call to trigger rewriting.\n",
                "o.foo?.();\n"
            ),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testNoRewrite_nonCallReference_viaNew
#[test]
fn test_no_rewrite_non_call_reference_via_new() {
    let mut t = Test::new();
    // TODO(nickreid): Add rewriting support for this.
    t.test_same(
        &[
            NoRewriteNonCallReferenceTestInput::BASE,
            concat!("o.foo();\n", "new o.foo();\n"),
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testRewriteNoNestedFunction
#[test]
fn test_rewrite_no_nested_function() {
    let mut t = Test::new();
    t.test(
        &semicolon_join(&[
            &[NoRewriteNestedFunctionTestInput::PREFIX, "}"].concat(),
            NoRewriteNestedFunctionTestInput::SUFFIX,
            NoRewriteNestedFunctionTestInput::INNER,
        ]),
        &semicolon_join(&[
            &[NoRewriteNestedFunctionTestInput::EXPECTED_PREFIX, "}"].concat(),
            NoRewriteNestedFunctionTestInput::EXPECTED_SUFFIX,
            concat!(
                "var JSCompiler_StaticMethods_bar=\n",
                "function(JSCompiler_StaticMethods_bar$self){}\n"
            ),
            "JSCompiler_StaticMethods_bar(o)",
        ]),
    );
}

// port: DevirtualizeMethodsTest#testNoRewriteNestedFunction
#[test]
fn test_no_rewrite_nested_function() {
    let mut t = Test::new();
    t.test(
        &[
            NoRewriteNestedFunctionTestInput::PREFIX,
            NoRewriteNestedFunctionTestInput::INNER,
            "};",
            NoRewriteNestedFunctionTestInput::SUFFIX,
        ]
        .concat(),
        &[
            NoRewriteNestedFunctionTestInput::EXPECTED_PREFIX,
            NoRewriteNestedFunctionTestInput::INNER,
            "};",
            NoRewriteNestedFunctionTestInput::EXPECTED_SUFFIX,
        ]
        .concat(),
    );
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingGetProp_withArgs_callUsingGetProp
#[test]
fn test_rewrite_defined_using_get_prop_with_args_call_using_get_prop() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "o.foo()\n"
    );
    let expected_js = concat!(
        "function a(){}\n",
        "var JSCompiler_StaticMethods_foo =\n",
        "  function(JSCompiler_StaticMethods_foo$self, args) {return args};\n",
        "var o = new a;\n",
        "JSCompiler_StaticMethods_foo(o)\n"
    );
    t.test(source, expected_js);
}

// port: DevirtualizeMethodsTest#testNoRewrite_optChainGetProp
#[test]
fn test_no_rewrite_opt_chain_get_prop() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "o?.foo()\n"
    );
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingGetElem_withArgs_callUsingGetProp
#[test]
fn test_rewrite_defined_using_get_elem_with_args_call_using_get_prop() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype['foo'] = function(args) {return args};\n",
        "var o = new a;\n",
        "o.foo()\n"
    );
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testNoRewrite_definedUsingGetProp_withArgs_noCall_bracketAccess
#[test]
fn test_no_rewrite_defined_using_get_prop_with_args_no_call_bracket_access() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "o['foo']\n"
    );
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testNoRewrite_optChainGetElemAccess
#[test]
fn test_no_rewrite_opt_chain_get_elem_access() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "o?.['foo']\n"
    );
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testNoRewrite_definedUsingGetElem_withArgs_noCall_bracketAccess
#[test]
fn test_no_rewrite_defined_using_get_elem_with_args_no_call_bracket_access() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype['foo'] = function(args) {return args};\n",
        "var o = new a;\n",
        "o['foo']\n"
    );
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testRewrite_definedInExpression_thatCreatesScope_reportsScopeAsDeleted
#[test]
fn test_rewrite_defined_in_expression_that_creates_scope_reports_scope_as_deleted() {
    let mut t = Test::new();
    t.test(
        concat!(
            "(function() {}).prototype.foo = function() {extern();};\n",
            "// A call is needed to trigger rewriting.\n",
            "a.foo();\n"
        ),
        concat!(
            "var JSCompiler_StaticMethods_foo = function(JSCompiler_StaticMethods_foo$self) {\n",
            "  extern();\n",
            "};\n",
            "JSCompiler_StaticMethods_foo(a);\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testRewrite_definedInExpression_withSideEffects
#[test]
fn test_rewrite_defined_in_expression_with_side_effects() {
    let mut t = Test::new();
    // TODO(nickreid): Expect this not to be a rewrite (or confirm it's safe).
    t.test(
        concat!(
            "extern().prototype.foo = function() { };\n",
            "// A call is needed to trigger rewriting.\n",
            "a.foo()\n"
        ),
        concat!(
            "var JSCompiler_StaticMethods_foo=function(JSCompiler_StaticMethods_foo$self){};\n",
            "JSCompiler_StaticMethods_foo(a)\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testRewrite_definedInExpression_withInvocation
#[test]
fn test_rewrite_defined_in_expression_with_invocation() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "(\n",
            "  class { bar() { } },\n",
            "  a.bar()\n",
            ");\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "};\n",
            "(\n",
            "  class { },\n",
            "  JSCompiler_StaticMethods_bar(a)\n",
            ");\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_definedUsingStringKey_inPrototypeLiteral_usingSuper
#[test]
fn test_no_rewrite_defined_using_string_key_in_prototype_literal_using_super() {
    let mut t = Test::new();
    // TODO(b/120452418): Add rewriting support for this.
    t.test_same_parts(vec![srcs(concat!(
        "function a(){}\n",
        "a.prototype = {\n",
        "// Don't use `super.bar()` because that might effect the test.\n",
        "  foo() { return super.x; }\n",
        "};\n",
        "\n",
        "// We need at least one normal call to trigger rewriting.\n",
        "x.foo()\n"
    ))]);
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingClassMember_prototypeMethod
#[test]
fn test_rewrite_defined_using_class_member_prototype_method() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() { return 5; }\n",
            "}\n",
            "\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  return 5;\n",
            "};\n",
            "class Foo { }\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingClassMember_prototypeMethod_usingThis
#[test]
fn test_rewrite_defined_using_class_member_prototype_method_using_this() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() { return this; }\n",
            "}\n",
            "\n",
            "// We need at least one normal call to trigger rewriting.\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "    return JSCompiler_StaticMethods_bar$self;\n",
            "};\n",
            "class Foo { }\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_definedUsingClassMember_prototypeMethod_usingSuper
#[test]
fn test_no_rewrite_defined_using_class_member_prototype_method_using_super() {
    let mut t = Test::new();
    // TODO(b/120452418): Add rewriting support for this.
    t.test_same_parts(vec![srcs(concat!(
        "class Foo {\n",
        "// Don't use `super.bar()` because that might effect the test.\n",
        "  bar() { return super.x; }\n",
        "}\n",
        "\n",
        "// We need at least one normal call to trigger rewriting.\n",
        "x.bar();\n"
    ))]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_definedUsingClassMember_constructor
#[test]
fn test_no_rewrite_defined_using_class_member_constructor() {
    let mut t = Test::new();
    t.test_same_parts(vec![srcs(concat!(
        "class Foo {\n",
        "  constructor() { }\n",
        "}\n",
        "\n",
        "// We need at least one normal call to trigger rewriting.\n",
        "x.constructor();\n"
    ))]);
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingClassMember_staticMethod
#[test]
fn test_rewrite_defined_using_class_member_static_method() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  static bar() { return 5; }\n",
            "}\n",
            "\n",
            "// We need at least one normal call to trigger rewriting.\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  return 5;\n",
            "};\n",
            "class Foo { }\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingAssignment_staticMethod_onClass
#[test]
fn test_rewrite_defined_using_assignment_static_method_on_class() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo { }\n",
            "Foo.bar = function() { return 5; }\n",
            "\n",
            "// We need at least one normal call to trigger rewriting.\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "class Foo { }\n",
            "\n",
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  return 5;\n",
            "};\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingAssignment_staticMethod_onFunction
#[test]
fn test_rewrite_defined_using_assignment_static_method_on_function() {
    let mut t = Test::new();
    for annotation in ["/** @constructor */", "/** @interface */", "/** @record*/"] {
        t.test_rewrite_defined_using_assignment_static_method_on_function(annotation);
    }
}

// port: DevirtualizeMethodsTest#testRewrite_definedUsingClassMember_staticMethod_usingThis
#[test]
fn test_rewrite_defined_using_class_member_static_method_using_this() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  static bar() { return this; }\n",
            "}\n",
            "\n",
            "// We need at least one normal call to trigger rewriting.\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  return JSCompiler_StaticMethods_bar$self;\n",
            "};\n",
            "class Foo { }\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_definedUsingClassMember_staticMethod_usingSuper
#[test]
fn test_no_rewrite_defined_using_class_member_static_method_using_super() {
    let mut t = Test::new();
    // TODO(b/120452418): Add rewriting support for this.
    t.test_same_parts(vec![srcs(concat!(
        "class Foo {\n",
        "// Don't use `super.bar()` because that might effect the test.\n",
        "  static bar() { return super.x; }\n",
        "}\n",
        "\n",
        "// We need at least one normal call to trigger rewriting.\n",
        "x.bar();\n"
    ))]);
}

// port: DevirtualizeMethodsTest#testRewrite_callWithSuperReceiver_passesThis
#[test]
fn test_rewrite_call_with_super_receiver_passes_this() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "// Don't use `super.bar()` because that might effect the test.\n",
            "  bar() { return super.qux(); }\n",
            "}\n",
            "\n",
            "class Tig {\n",
            "  qux() { return 5; }\n",
            "}\n"
        )),
        expected(concat!(
            "class Foo {\n",
            "  bar() { return JSCompiler_StaticMethods_qux(this); }\n",
            "}\n",
            "\n",
            "var JSCompiler_StaticMethods_qux = function(JSCompiler_StaticMethods_qux$self) {\n",
            "  return 5;\n",
            "};\n",
            "class Tig { }\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_inClassWithLocalName_asClassName
#[test]
fn test_no_rewrite_in_class_with_local_name_as_class_name() {
    let mut t = Test::new();
    t.test_same_parts(vec![srcs(concat!(
        "const Qux = class Foo {\n",
        "// TODO(nickreid): Add rewriting support for this so long as the local name is not\n",
        "// referenced.\n",
        "  bar() { return 5; }\n",
        "}\n",
        "\n",
        "// We need at least one normal call to trigger rewriting.\n",
        "x.bar();\n"
    ))]);
}

// port: DevirtualizeMethodsTest#testPropDefinition_notOnQualifiedName_doesNotCrash
#[test]
fn test_prop_definition_not_on_qualified_name_does_not_crash() {
    let mut t = Test::new();
    t.test_same_parts(vec![srcs(concat!(
        "class Qux { }\n",
        "\n",
        "new Qux().prop = function() { }\n"
    ))]);
}

// port: DevirtualizeMethodsTest#testRewriteDeclWithConstJSDoc
#[test]
fn test_rewrite_decl_with_const_js_doc() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {\n",
            "  /** @const */ foo() {}\n",
            "}\n",
            "o.foo();\n"
        ),
        concat!(
            "/** @const */\n",
            "var JSCompiler_StaticMethods_foo =\n",
            "  function(JSCompiler_StaticMethods_foo$self) {};\n",
            "class C {}\n",
            "JSCompiler_StaticMethods_foo(o)\n"
        ),
    );
}

// port: DevirtualizeMethodsTest#testNoRewriteGet1
#[test]
fn test_no_rewrite_get1() {
    let mut t = Test::new();
    // Getters and setter require special handling.
    t.test_same("function a(){}; a.prototype = {get foo(){return f}}; var o = new a; o.foo()");
}

// port: DevirtualizeMethodsTest#testNoRewriteGet2
#[test]
fn test_no_rewrite_get2() {
    let mut t = Test::new();
    // Getters and setter require special handling.
    t.test_same("function a(){}; a.prototype = {get foo(){return 1}}; var o = new a; o.foo");
}

// port: DevirtualizeMethodsTest#testNoRewriteSet1
#[test]
fn test_no_rewrite_set1() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    // Getters and setter require special handling.
    let source = "function a(){}; a.prototype = {set foo(a){}}; var o = new a; o.foo()";
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testNoRewriteSet2
#[test]
fn test_no_rewrite_set2() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    // Getters and setter require special handling.
    let source = "function a(){}; a.prototype = {set foo(a){}}; var o = new a; o.foo = 1";
    t.test_same(source);
}

// port: DevirtualizeMethodsTest#testNoRewrite_notImplementedMethod
#[test]
fn test_no_rewrite_not_implemented_method() {
    let mut t = Test::new();
    t.test_same("function a(){}; var o = new a; o.foo()");
}

// port: DevirtualizeMethodsTest#testWrapper
#[test]
fn test_wrapper() {
    let mut t = Test::new();
    t.test_same("(function() {})()");
}

// port: DevirtualizeMethodsTest#testRewrite_nestedFunction_hasThisBoundCorrectly
#[test]
fn test_rewrite_nested_function_has_this_bound_correctly() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    return function() { return this; };\n",
            "  }\n",
            "}\n",
            "\n",
            "// We need at least one normal call to trigger rewriting.\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  return function() { return this; };\n",
            "};\n",
            "class Foo { }\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_nestedArrow_hasThisBoundCorrectly
#[test]
fn test_rewrite_nested_arrow_has_this_bound_correctly() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    return () => this;\n",
            "  }\n",
            "}\n",
            "\n",
            "// We need at least one normal call to trigger rewriting.\n",
            "x.bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  return () => JSCompiler_StaticMethods_bar$self;\n",
            "};\n",
            "class Foo { }\n",
            "\n",
            "JSCompiler_StaticMethods_bar(x);\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_nestedClass_instanceFieldInitializer
#[test]
fn test_rewrite_nested_class_instance_field_initializer() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    class Nested {\n",
            "      val = this;\n",
            "    }\n",
            "    return new Nested();\n",
            "  }\n",
            "}\n",
            "new Foo().bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  class Nested {\n",
            "    val = this;\n",
            "  }\n",
            "  return new Nested();\n",
            "};\n",
            "class Foo { }\n",
            "JSCompiler_StaticMethods_bar(new Foo());\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_nestedClass_computedFieldInitializer
#[test]
fn test_rewrite_nested_class_computed_field_initializer() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    class Nested {\n",
            "      ['c'] = this;\n",
            "    }\n",
            "    return new Nested();\n",
            "  }\n",
            "}\n",
            "new Foo().bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  class Nested {\n",
            "    ['c'] = this;\n",
            "  }\n",
            "  return new Nested();\n",
            "};\n",
            "class Foo { }\n",
            "JSCompiler_StaticMethods_bar(new Foo());\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_nestedClass_computedFieldKey
#[test]
fn test_rewrite_nested_class_computed_field_key() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    class Nested {\n",
            "      [this.key] = 1;\n",
            "    }\n",
            "    return new Nested();\n",
            "  }\n",
            "}\n",
            "new Foo().bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  class Nested {\n",
            "    [JSCompiler_StaticMethods_bar$self.key] = 1;\n",
            "  }\n",
            "  return new Nested();\n",
            "};\n",
            "class Foo { }\n",
            "JSCompiler_StaticMethods_bar(new Foo());\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_nestedClass_computedFieldKeyAndValue
#[test]
fn test_rewrite_nested_class_computed_field_key_and_value() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    class Nested {\n",
            "      [this.key] = this;\n",
            "    }\n",
            "    return new Nested();\n",
            "  }\n",
            "}\n",
            "new Foo().bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  class Nested {\n",
            "    [JSCompiler_StaticMethods_bar$self.key] = this;\n",
            "  }\n",
            "  return new Nested();\n",
            "};\n",
            "class Foo { }\n",
            "JSCompiler_StaticMethods_bar(new Foo());\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_nestedClass_staticFieldInitializer
#[test]
fn test_rewrite_nested_class_static_field_initializer() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    class Nested {\n",
            "      static val = this;\n",
            "    }\n",
            "    return Nested;\n",
            "  }\n",
            "}\n",
            "new Foo().bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  class Nested {\n",
            "    static val = this;\n",
            "  }\n",
            "  return Nested;\n",
            "};\n",
            "class Foo { }\n",
            "JSCompiler_StaticMethods_bar(new Foo());\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_nestedClass_staticBlock
#[test]
fn test_rewrite_nested_class_static_block() {
    let mut t = Test::new();
    t.test_parts(vec![
        srcs(concat!(
            "class Foo {\n",
            "  bar() {\n",
            "    class Nested {\n",
            "      static {\n",
            "        this.val = 1;\n",
            "      }\n",
            "    }\n",
            "    return Nested;\n",
            "  }\n",
            "}\n",
            "new Foo().bar();\n"
        )),
        expected(concat!(
            "var JSCompiler_StaticMethods_bar = function(JSCompiler_StaticMethods_bar$self) {\n",
            "  class Nested {\n",
            "    static {\n",
            "      this.val = 1;\n",
            "    }\n",
            "  }\n",
            "  return Nested;\n",
            "};\n",
            "class Foo { }\n",
            "JSCompiler_StaticMethods_bar(new Foo());\n"
        )),
    ]);
}

// port: DevirtualizeMethodsTest#testExistenceOfAGetter_preventsDevirtualization
#[test]
fn test_existence_of_a_getter_prevents_devirtualization() {
    let mut t = Test::new();
    t.harness
        .declare_accessor("foo".into(), property_access_kind("GETTER_ONLY"))
        .unwrap();

    // Imagine the getter returned a function.
    t.test_same(concat!(
        "class Foo {\n",
        "  foo() {}\n",
        "}\n",
        "x.foo();\n"
    ));
}

// port: DevirtualizeMethodsTest#testExistenceOfASetter_preventsDevirtualization
#[test]
fn test_existence_of_a_setter_prevents_devirtualization() {
    let mut t = Test::new();
    t.harness
        .declare_accessor("foo".into(), property_access_kind("SETTER_ONLY"))
        .unwrap();

    // This doesn't actually seem like a risk but it's hard to say, and other optimizations that use
    // optimize calls would be dangerous on setters.
    t.test_same(concat!(
        "class Foo {\n",
        "  foo() {}\n",
        "}\n",
        "x.foo();\n"
    ));
}

// port: DevirtualizeMethodsTest#testThisProperty
#[test]
fn test_this_property() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_same(concat!(
        "class Foo {\n",
        "  constructor() {\n",
        "    this.a = function b() { return 5; };\n",
        "    this.plus1 = (arg) => arg + 1;\n",
        "    this.tmp = this.plus1(1);\n",
        "  }\n",
        "}\n",
        "console.log(new Foo().a());\n"
    ));
}

// port: DevirtualizeMethodsTest#testNonStaticClassFieldNoRHS
#[test]
fn test_non_static_class_field_no_rhs() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  a;\n",
        "}\n",
        "console.log(new Foo().a);\n"
    ));
}

// port: DevirtualizeMethodsTest#testNonStaticClassFieldNonFunction
#[test]
fn test_non_static_class_field_non_function() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  a = 2;\n",
        "}\n",
        "console.log(new Foo().a);\n"
    ));
}

// port: DevirtualizeMethodsTest#testNonStaticClassFieldFunction
#[test]
fn test_non_static_class_field_function() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  a = function x() { return 5; };\n",
        "}\n",
        "console.log(new Foo().a);\n"
    ));
}

// port: DevirtualizeMethodsTest#testStaticClassFieldNoRHS
#[test]
fn test_static_class_field_no_rhs() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  static a;\n",
        "}\n",
        "console.log(Foo.a);\n"
    ));
}

// port: DevirtualizeMethodsTest#testStaticClassFieldNonFunction
#[test]
fn test_static_class_field_non_function() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  static a = 2;\n",
        "}\n",
        "console.log(Foo.a);\n"
    ));
}

// port: DevirtualizeMethodsTest#testStaticClassFieldFunction
#[test]
fn test_static_class_field_function() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  static a = function x() { return 5; };\n",
        "}\n",
        "console.log(Foo.a);\n"
    ));
}

// port: DevirtualizeMethodsTest#testRewriteSameModule1
#[test]
fn test_rewrite_same_module1() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk(semicolon_join(&[
            ModuleTestInput::DEFINITION,
            ModuleTestInput::USE,
        ]))
        // m2
        .add_chunk("")
        .build();

    t.test_parts(vec![
        srcs_chunks(chunks),
        expected_strings(&[
            // m1
            &semicolon_join(&[
                ModuleTestInput::REWRITTEN_DEFINITION,
                ModuleTestInput::REWRITTEN_USE,
            ]),
            // m2
            "",
        ]),
    ]);
}

// port: DevirtualizeMethodsTest#testRewriteSameModule2
#[test]
fn test_rewrite_same_module2() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("")
        // m2
        .add_chunk(semicolon_join(&[
            ModuleTestInput::DEFINITION,
            ModuleTestInput::USE,
        ]))
        .build();

    t.test_parts(vec![
        srcs_chunks(chunks),
        expected_strings(&[
            // m1
            "",
            // m2
            &semicolon_join(&[
                ModuleTestInput::REWRITTEN_DEFINITION,
                ModuleTestInput::REWRITTEN_USE,
            ]),
        ]),
    ]);
}

// port: DevirtualizeMethodsTest#testRewriteSameModule3
#[test]
fn test_rewrite_same_module3() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk(semicolon_join(&[
            ModuleTestInput::USE,
            ModuleTestInput::DEFINITION,
        ]))
        // m2
        .add_chunk("")
        .build();

    t.test_parts(vec![
        srcs_chunks(chunks),
        expected_strings(&[
            // m1
            &semicolon_join(&[
                ModuleTestInput::REWRITTEN_USE,
                ModuleTestInput::REWRITTEN_DEFINITION,
            ]),
            // m2
            "",
        ]),
    ]);
}

// port: DevirtualizeMethodsTest#testRewrite_definitionModule_beforeUseModule
#[test]
fn test_rewrite_definition_module_before_use_module() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk(ModuleTestInput::DEFINITION)
        // m2
        .add_chunk(ModuleTestInput::USE)
        .build();

    t.test_parts(vec![
        srcs_chunks(chunks),
        expected_strings(&[
            // m1
            ModuleTestInput::REWRITTEN_DEFINITION,
            // m2
            ModuleTestInput::REWRITTEN_USE,
        ]),
    ]);
}

// port: DevirtualizeMethodsTest#testNoRewrite_definitionModule_afterUseModule
#[test]
fn test_no_rewrite_definition_module_after_use_module() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_star()
        .add_chunk(ModuleTestInput::USE)
        .add_chunk(ModuleTestInput::DEFINITION)
        .build();

    t.test_same_parts(vec![srcs_chunks(chunks)]);
}
