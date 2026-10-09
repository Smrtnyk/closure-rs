/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/testing/JSCompCorrespondences.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/PureFunctionIdentifierTest.java.

//! Port of the unit-corpus helper `PureFunctionIdentifierTest_Helpers.java`
//! (oracle/replay/helpers): the holder of PureFunctionIdentifierTest's fields, the
//! `NoSideEffectCallEnumerator` pass of PureFunctionIdentifierTest#getProcessor, the
//! postcondition of assertPureCallsMarked and the `post` lambdas of nine test methods.
use crate::{
    compiler_test_case::{CompilerTestCase, contains_exactly_by},
    jscomp_api::{Compiler, SourceFile},
    replay::replay_dsl::{Ctx, DslValue, NativeObject, Object},
    throwable::{Throwable, assert_that},
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId, SideEffectFlags},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

const HOLDER: &str = "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers";
const POSTCONDITION: &str = "com.google.javascript.jscomp.CompilerTestCase$Postcondition";

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("{what} arguments"))
}

// port: PureFunctionIdentifierTest_Helpers#PureFunctionIdentifierTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    fields.insert("noSideEffectCalls".to_string(), DslValue::Null);
    field_types.insert(
        "noSideEffectCalls".to_string(),
        "java.util.List".to_string(),
    );
    fields.insert("regExpHaveSideEffects".to_string(), DslValue::Bool(true));
    field_types.insert("regExpHaveSideEffects".to_string(), "boolean".to_string());
    fields.insert(
        "enableArtificialPurityDebugError".to_string(),
        DslValue::Bool(false),
    );
    field_types.insert(
        "enableArtificialPurityDebugError".to_string(),
        "boolean".to_string(),
    );
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: HOLDER.into(),
        fields,
        field_types,
    }))))
}

// port: CompilerTestCase#postcondition
pub fn postcondition(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [postcondition] = args.as_slice() else {
        return Err(bad("CompilerTestCase.postcondition"));
    };
    Ok(CompilerTestCase::postcondition(postcondition.clone()))
}

// port: PureFunctionIdentifierTest_Helpers (boolean field read)
fn bool_field(outer: &Rc<RefCell<Object>>, name: &str) -> bool {
    matches!(
        outer.borrow().fields.get(name).map(DslValue::untyped),
        Some(DslValue::Bool(true))
    )
}

/// Run PureFunctionIdentifier, then gather a list of calls that are marked as having no side
/// effects.
struct NoSideEffectCallEnumerator {
    outer: Rc<RefCell<Object>>,
    driver: DslValue,
}

// port: PureFunctionIdentifierTest_Helpers.NoSideEffectCallEnumerator#NoSideEffectCallEnumerator
pub fn no_side_effect_call_enumerator(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(_), driver] = args.as_slice() else {
        return Err(bad(
            "PureFunctionIdentifierTest_Helpers$NoSideEffectCallEnumerator",
        ));
    };
    let pass: Box<dyn CompilerPass> = Box::new(NoSideEffectCallEnumerator {
        outer: outer.clone(),
        driver: driver.clone(),
    });
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

impl CompilerPass for NoSideEffectCallEnumerator {
    // port: PureFunctionIdentifierTest_Helpers.NoSideEffectCallEnumerator#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut no_side_effect_calls = Vec::new();
        // TODO(nickreid): Move these into 'getOptions' and 'getCompiler' overrides.
        compiler
            .set_has_reg_exp_global_references(bool_field(&self.outer, "regExpHaveSideEffects"));
        compiler
            .get_options_mut()
            .set_use_types_for_local_optimization(true);

        match &self.driver {
            DslValue::Pass(driver) => driver.borrow_mut().process(compiler, externs, root),
            other => panic!("{} is not a CompilerPass", other.class_name()),
        }
        let mut visitor = Visitor {
            no_side_effect_calls: &mut no_side_effect_calls,
        };
        NodeTraversal::traverse(compiler, externs, &mut visitor);
        NodeTraversal::traverse(compiler, root, &mut visitor);

        let calls = no_side_effect_calls
            .into_iter()
            .map(|n| DslValue::Typed {
                class: n.get_class(compiler).into(),
                value: Box::new(DslValue::Node(n)),
            })
            .collect();
        self.outer
            .borrow_mut()
            .fields
            .insert("noSideEffectCalls".into(), DslValue::List(calls));
    }
}

/// The AbstractPostOrderCallback half of NoSideEffectCallEnumerator.
struct Visitor<'a> {
    no_side_effect_calls: &'a mut Vec<NodeId>,
}

impl Callback for Visitor<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: PureFunctionIdentifierTest_Helpers.NoSideEffectCallEnumerator#visit
    #[allow(clippy::collapsible_if)] // Retain the Java branches.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        if n.is_new(compiler) {
            if !compiler
                .get_ast_analyzer()
                .constructor_call_has_side_effects(compiler, n)
            {
                self.no_side_effect_calls
                    .push(n.get_first_child(compiler).unwrap());
            }
        } else if NodeUtil::is_invocation(compiler, n) {
            if !compiler
                .get_ast_analyzer()
                .function_call_has_side_effects(compiler, n)
            {
                self.no_side_effect_calls
                    .push(n.get_first_child(compiler).unwrap());
            }
        }
    }
}

/// The postcondition lambda of assertPureCallsMarked; the constructor parameters are its captured
/// `expected` and `post`.
struct AssertPureCallsMarkedPostcondition {
    outer: Rc<RefCell<Object>>,
    expected: Vec<JsString>,
    post: DslValue,
}

// port: PureFunctionIdentifierTest_Helpers.AssertPureCallsMarkedPostcondition#AssertPureCallsMarkedPostcondition
pub fn assert_pure_calls_marked_postcondition(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), expected, post] = args.as_slice() else {
        return Err(bad(
            "PureFunctionIdentifierTest_Helpers$AssertPureCallsMarkedPostcondition",
        ));
    };
    let DslValue::List(items) = expected.untyped() else {
        return Err(bad(
            "PureFunctionIdentifierTest_Helpers$AssertPureCallsMarkedPostcondition",
        ));
    };
    let expected = items
        .iter()
        .map(|v| match v.untyped() {
            DslValue::String(s) => Ok(s.clone()),
            _ => Err(bad("AssertPureCallsMarkedPostcondition expected")),
        })
        .collect::<Result<_, _>>()?;
    Ok(DslValue::Native(Rc::new(RefCell::new(
        AssertPureCallsMarkedPostcondition {
            outer: outer.clone(),
            expected,
            post: post.clone(),
        },
    ))))
}

/// JSCompCorrespondences#parseExpr: parses `expr` into an expression AST as leniently as possible.
/// Java parses with one static compiler; parsing only reads that compiler's options, so a fresh
/// compiler per assertion gives the same trees.
// port: JSCompCorrespondences#parseExpr
fn parse_expr(parser: &mut Compiler, expr: &JsString) -> NodeId {
    let code = JsString::from("(")
        .concat(expr)
        .concat(&JsString::from(")"));
    let script = parser.parse_file(Arc::new(SourceFile::from_code("expr", code))); // SCRIPT
    // EXPR_RESULT > expr
    script
        .get_first_first_child(parser)
        .expect("Failed to parse expression")
}

impl NativeObject for AssertPureCallsMarkedPostcondition {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$AssertPureCallsMarkedPostcondition"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == POSTCONDITION
    }
    // port: PureFunctionIdentifierTest_Helpers.AssertPureCallsMarkedPostcondition#verify
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        let ("verify", [DslValue::Compiler(compiler)]) = (method, args.as_slice()) else {
            return Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )));
        };
        let actual: Vec<NodeId> = match self.outer.borrow().fields.get("noSideEffectCalls") {
            Some(DslValue::List(items)) => items
                .iter()
                .map(|v| match v.untyped() {
                    DslValue::Node(n) => Ok(*n),
                    _ => Err(bad("noSideEffectCalls")),
                })
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(Throwable::Exception {
                    class: "java.lang.NullPointerException".into(),
                    message: None,
                });
            }
        };
        {
            let c = compiler.borrow();
            let ast: &Ast = &c;
            let mut parser = Compiler::new();
            let parsed: Vec<NodeId> = self
                .expected
                .iter()
                .map(|e| parse_expr(&mut parser, e))
                .collect();
            let parser_ast: &Ast = &parser;
            // assertThat(noSideEffectCalls)
            //     .comparingElementsUsing(JSCompCorrespondences.EQUALITY_WHEN_PARSED_AS_EXPRESSION)
            //     .containsExactlyElementsIn(expected);
            contains_exactly_by(
                &actual,
                &parsed,
                false,
                |&a, &e| e.is_equivalent_to_across(parser_ast, ast, a),
                "noSideEffectCalls",
            )?;
        }
        if !matches!(self.post, DslValue::Null) {
            let DslValue::Native(post) = &self.post else {
                return Err(Throwable::Unported(format!(
                    "{}#verify",
                    self.post.class_name()
                )));
            };
            post.borrow_mut()
                .call("verify", vec![DslValue::Compiler(compiler.clone())])?;
        }
        Ok(DslValue::Null)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The `post` lambdas of PureFunctionIdentifierTest, one helper class each.
#[derive(Clone, Copy)]
enum Post {
    TestFunctionProperties1,
    TestTaggedTemplatelitPropagatesCalleeSideEffects,
    TestCallCache,
    TestCallCacheWithKeyFn,
    TestCallCacheAnonymousFn,
    TestCallCacheAnonymousFnHasSideEffects,
    TestCallCacheHasSideEffects,
    TestCallCacheWithKeyFnHasSideEffects,
    TestCallCachePropagatesSideEffects,
}

struct PostLambda {
    class: &'static str,
    post: Post,
}

macro_rules! post_ctor {
    ($fn_name:ident, $class:literal, $post:ident) => {
        // port: PureFunctionIdentifierTest_Helpers (Post_* constructor)
        pub fn $fn_name(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
            Ok(DslValue::Native(Rc::new(RefCell::new(PostLambda {
                class: concat!(
                    "com.google.javascript.jscomp.PureFunctionIdentifierTest_Helpers$",
                    $class
                ),
                post: Post::$post,
            }))))
        }
    };
}

post_ctor!(
    post_test_function_properties1,
    "Post_testFunctionProperties1",
    TestFunctionProperties1
);
post_ctor!(
    post_test_tagged_templatelit_propagates_callee_side_effects,
    "Post_testTaggedTemplatelit_propagatesCalleeSideEffects",
    TestTaggedTemplatelitPropagatesCalleeSideEffects
);
post_ctor!(post_test_call_cache, "Post_testCallCache", TestCallCache);
post_ctor!(
    post_test_call_cache_with_key_fn,
    "Post_testCallCache_withKeyFn",
    TestCallCacheWithKeyFn
);
post_ctor!(
    post_test_call_cache_anonymous_fn,
    "Post_testCallCache_anonymousFn",
    TestCallCacheAnonymousFn
);
post_ctor!(
    post_test_call_cache_anonymous_fn_has_side_effects,
    "Post_testCallCache_anonymousFn_hasSideEffects",
    TestCallCacheAnonymousFnHasSideEffects
);
post_ctor!(
    post_test_call_cache_has_side_effects,
    "Post_testCallCache_hasSideEffects",
    TestCallCacheHasSideEffects
);
post_ctor!(
    post_test_call_cache_with_key_fn_has_side_effects,
    "Post_testCallCache_withKeyFn_hasSideEffects",
    TestCallCacheWithKeyFnHasSideEffects
);
post_ctor!(
    post_test_call_cache_propagates_side_effects,
    "Post_testCallCache_propagatesSideEffects",
    TestCallCachePropagatesSideEffects
);

// port: PureFunctionIdentifierTest_Helpers#findQualifiedNameNode
fn find_qualified_name_node(ast: &Ast, name: &str, root: NodeId) -> Result<NodeId, Throwable> {
    CompilerTestCase::find_qualified_name_node(ast, &JsString::from(name), root)
}

// port: PureFunctionIdentifierTest_Helpers (the cached call checks of the testCallCache lambdas)
fn assert_cache_call(ast: &Ast, call: NodeId, pure: bool) -> Result<(), Throwable> {
    assert_that(
        call.is_no_side_effects_call(ast) == pure,
        format!("isNoSideEffectsCall() should be {pure}"),
    )?;
    assert_that(
        call.may_mutate_global_state_or_throw(ast) != pure,
        format!("mayMutateGlobalStateOrThrow() should be {}", !pure),
    )
}

impl NativeObject for PostLambda {
    fn class_name(&self) -> &str {
        self.class
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class || class == POSTCONDITION
    }
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        let ("verify", [DslValue::Compiler(compiler)]) = (method, args.as_slice()) else {
            return Err(Throwable::Unported(format!("{}#{method}", self.class)));
        };
        let c = compiler.borrow();
        let ast: &Ast = &c;
        let root = c.get_root().unwrap();
        match self.post {
            // port: PureFunctionIdentifierTest_Helpers.Post_testFunctionProperties1#verify
            Post::TestFunctionProperties1 => {
                let last_root = root;
                let call = find_qualified_name_node(ast, "g.call", last_root)?
                    .get_parent(ast)
                    .unwrap();
                let mut expected = SideEffectFlags::new();
                expected.clear_all_flags().set_mutates_arguments();
                assert_that(
                    call.get_side_effect_flags(ast) == expected.value_of(),
                    "g.call side effect flags",
                )?;
            }
            // port: PureFunctionIdentifierTest_Helpers.Post_testTaggedTemplatelit_propagatesCalleeSideEffects#verify
            Post::TestTaggedTemplatelitPropagatesCalleeSideEffects => {
                let last_root = root;
                let tag_def = find_qualified_name_node(ast, "tag", last_root)?
                    .get_parent(ast)
                    .unwrap();
                let foo_def = find_qualified_name_node(ast, "foo", last_root)?
                    .get_parent(ast)
                    .unwrap();
                assert_that(
                    foo_def.get_side_effect_flags(ast) == tag_def.get_side_effect_flags(ast),
                    "foo and tag side effect flags",
                )?;
            }
            // port: PureFunctionIdentifierTest_Helpers.Post_testCallCache#verify
            // port: PureFunctionIdentifierTest_Helpers.Post_testCallCache_withKeyFn#verify
            // port: PureFunctionIdentifierTest_Helpers.Post_testCallCache_anonymousFn#verify
            // port: PureFunctionIdentifierTest_Helpers.Post_testCallCache_anonymousFn_hasSideEffects#verify
            // port: PureFunctionIdentifierTest_Helpers.Post_testCallCache_hasSideEffects#verify
            // port: PureFunctionIdentifierTest_Helpers.Post_testCallCache_withKeyFn_hasSideEffects#verify
            Post::TestCallCache
            | Post::TestCallCacheWithKeyFn
            | Post::TestCallCacheAnonymousFn
            | Post::TestCallCacheAnonymousFnHasSideEffects
            | Post::TestCallCacheHasSideEffects
            | Post::TestCallCacheWithKeyFnHasSideEffects => {
                let pure = matches!(
                    self.post,
                    Post::TestCallCache
                        | Post::TestCallCacheWithKeyFn
                        | Post::TestCallCacheAnonymousFn
                );
                let last_root = root.get_last_child(ast).unwrap();
                let call = find_qualified_name_node(ast, "goog.reflect.cache", last_root)?
                    .get_parent(ast)
                    .unwrap();
                assert_cache_call(ast, call, pure)?;
            }
            // port: PureFunctionIdentifierTest_Helpers.Post_testCallCache_propagatesSideEffects#verify
            Post::TestCallCachePropagatesSideEffects => {
                let last_root = root.get_last_child(ast).unwrap();
                let cache_call = find_qualified_name_node(ast, "goog.reflect.cache", last_root)?
                    .get_parent(ast)
                    .unwrap();
                assert_cache_call(ast, cache_call, true)?;

                let helper_call = *CompilerTestCase::find_qualified_name_nodes(
                    ast,
                    &JsString::from("helper"),
                    last_root,
                )
                .last()
                .ok_or_else(|| Throwable::Exception {
                    class: "java.util.NoSuchElementException".into(),
                    message: None,
                })?;
                let helper_call = helper_call.get_parent(ast).unwrap();
                assert_cache_call(ast, helper_call, true)?;
            }
        }
        Ok(DslValue::Null)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
