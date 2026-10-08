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
//   src/com/google/javascript/jscomp/RewriteAsyncIteration.java.

//! Port of `RewriteAsyncIteration.java`.
//!
//! Converts async generator functions into a function returning a new $jscomp.AsyncGenWrapper
//! around the original block and awaits/yields converted to yields of ActionRecords.
//!
//! ```text
//! async function* foo() {
//!   let res = await myPromise;
//!   yield res + 1;
//! }
//! ```
//!
//! becomes (prefixes trimmed for clarity)
//!
//! ```text
//! function foo() {
//!   return new $jscomp.AsyncGeneratorWrapper((function*(){
//!     let res = yield new $ActionRecord($ActionEnum.AWAIT_VALUE, myPromise);
//!     yield new $ActionRecord($ActionEnum.YIELD_VALUE, res + 1);
//!   })());
//! }
//! ```
//!
//! Rust-only layout: `LexicalContext` and `ThisSuperArgsContext` objects live in arenas owned by
//! the pass; `ContextId` and `ThisSuperArgsContextId` are Java's object identities.

#![allow(clippy::needless_late_init, clippy::collapsible_match)] // Java statement shape

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js::runtime_js_lib_manager::JsLibField,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    jscomp_colors::standard_colors,
    node::{NodeId, Prop},
    token::Token,
};
use indexmap::IndexSet;
use std::collections::VecDeque;
use std::sync::Arc;

// port: RewriteAsyncIteration#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM.with_features(&[Feature::ASYNC_GENERATORS, Feature::FOR_AWAIT_OF])
}

// port: RewriteAsyncIteration#CANNOT_CONVERT_ASYNCGEN
pub static CANNOT_CONVERT_ASYNCGEN: DiagnosticType = DiagnosticType::error(
    "JSC_CANNOT_CONVERT_ASYNCGEN",
    "Cannot convert async generator. {0}",
);

// Variables with these names get created when rewriting for-await-of loops
// port: RewriteAsyncIteration#FOR_AWAIT_ITERATOR_TEMP_NAME
const FOR_AWAIT_ITERATOR_TEMP_NAME: &str = "$jscomp$forAwait$tempIterator";
// port: RewriteAsyncIteration#FOR_AWAIT_RESULT_TEMP_NAME
const FOR_AWAIT_RESULT_TEMP_NAME: &str = "$jscomp$forAwait$tempResult";
// port: RewriteAsyncIteration#FOR_AWAIT_ERROR_RESULT_TEMP_NAME
const FOR_AWAIT_ERROR_RESULT_TEMP_NAME: &str = "$jscomp$forAwait$errResult";
// port: RewriteAsyncIteration#FOR_AWAIT_CATCH_PARAM_TEMP_NAME
const FOR_AWAIT_CATCH_PARAM_TEMP_NAME: &str = "$jscomp$forAwait$catchErrParam";
// port: RewriteAsyncIteration#FOR_AWAIT_RETURN_FN_TEMP_NAME
const FOR_AWAIT_RETURN_FN_TEMP_NAME: &str = "$jscomp$forAwait$retFn";

// port: RewriteAsyncIteration#THIS_VAR_NAME
const THIS_VAR_NAME: &str = "$jscomp$asyncIter$this$";
// port: RewriteAsyncIteration#ARGUMENTS_VAR_NAME
const ARGUMENTS_VAR_NAME: &str = "$jscomp$asyncIter$arguments";
// port: RewriteAsyncIteration#SUPER_PROP_GETTER_PREFIX
const SUPER_PROP_GETTER_PREFIX: &str = "$jscomp$asyncIter$super$get$";

/// A `LexicalContext` handle: Java's object identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ContextId(usize);

/// A `ThisSuperArgsContext` handle: Java's object identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ThisSuperArgsContextId(usize);

/// Tracks a function and its context of this/arguments/super, if such a context exists.
struct LexicalContext {
    // Node that creates the context
    context_root: NodeId,
    // The current function, or null if root scope where we are not in a function.
    function: Option<NodeId>,
    // The context of the most recent definition of this/super/arguments
    this_super_args_context: Option<ThisSuperArgsContextId>,
}

/// Tracks how this/arguments/super were used in the function so declarations of replacement
/// variables can be prepended
struct ThisSuperArgsContext {
    /// The LexicalContext representing the function that declared this/super/args
    ctx: ContextId,

    used_super_properties: IndexSet<NodeId>,
    this_node_to_add: Option<NodeId>,
    used_arguments: bool,
    // unique id to append to names in this context. This is used to ensure that names
    // in different contexts don't collide (e.g. 2 functions don't get the same `let
    // $jscomp$async$this` name declared in their bodies)
    unique_id: String,
}

impl ThisSuperArgsContext {
    // port: RewriteAsyncIteration.ThisSuperArgsContext#ThisSuperArgsContext
    fn new(ctx: ContextId, unique_id: String) -> Self {
        Self {
            ctx,
            used_super_properties: IndexSet::new(),
            this_node_to_add: None,
            used_arguments: false,
            unique_id,
        }
    }
}

pub struct RewriteAsyncIteration {
    async_generator_wrapper: Arc<dyn JsLibField>,
    action_record_name: Arc<dyn JsLibField>,
    action_enum_await: Arc<dyn JsLibField>,
    action_enum_yield: Arc<dyn JsLibField>,
    action_enum_yield_star: Arc<dyn JsLibField>,

    next_for_await_id: i32,

    context_stack: VecDeque<ContextId>,
    ast_factory: AstFactory,
    namespace: TranspilationNamespace,

    /// Rust-only: the arenas of every `LexicalContext` / `ThisSuperArgsContext` created.
    contexts: Vec<LexicalContext>,
    this_super_args_contexts: Vec<ThisSuperArgsContext>,
}

impl RewriteAsyncIteration {
    // port: RewriteAsyncIteration#RewriteAsyncIteration
    fn new(
        compiler: &mut AbstractCompiler,
        ast_factory: AstFactory,
        namespace: TranspilationNamespace,
    ) -> Self {
        let runtime_lib_manager = compiler.get_runtime_js_lib_manager();
        let mut runtime_lib_manager = runtime_lib_manager.lock().unwrap();
        let async_generator_wrapper =
            runtime_lib_manager.get_js_lib_field("$jscomp.AsyncGeneratorWrapper");
        let action_record_name =
            runtime_lib_manager.get_js_lib_field("$jscomp.AsyncGeneratorWrapper$ActionRecord");
        let action_enum_await = runtime_lib_manager
            .get_js_lib_field("$jscomp.AsyncGeneratorWrapper$ActionEnum.AWAIT_VALUE");
        let action_enum_yield = runtime_lib_manager
            .get_js_lib_field("$jscomp.AsyncGeneratorWrapper$ActionEnum.YIELD_VALUE");
        let action_enum_yield_star = runtime_lib_manager
            .get_js_lib_field("$jscomp.AsyncGeneratorWrapper$ActionEnum.YIELD_STAR");
        Self {
            ast_factory,
            namespace,
            context_stack: VecDeque::new(),
            async_generator_wrapper,
            action_record_name,
            action_enum_await,
            action_enum_yield,
            action_enum_yield_star,
            next_for_await_id: 0,
            contexts: Vec::new(),
            this_super_args_contexts: Vec::new(),
        }
    }

    // port: RewriteAsyncIteration#create
    pub fn create(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let namespace = TranspilationNamespace::get(compiler);
        Self::new(compiler, ast_factory, namespace)
    }

    fn ctx(&self, ctx: ContextId) -> &LexicalContext {
        &self.contexts[ctx.0]
    }

    fn tsa(&self, id: ThisSuperArgsContextId) -> &ThisSuperArgsContext {
        &self.this_super_args_contexts[id.0]
    }

    fn tsa_mut(&mut self, id: ThisSuperArgsContextId) -> &mut ThisSuperArgsContext {
        &mut self.this_super_args_contexts[id.0]
    }

    /// Represents the global/root scope. Should only exist on the bottom of the contextStack.
    // port: RewriteAsyncIteration.LexicalContext#LexicalContext(Node)
    // port: RewriteAsyncIteration.LexicalContext#newGlobalContext
    fn new_global_context(&mut self, context_root: NodeId) -> ContextId {
        let id = ContextId(self.contexts.len());
        self.contexts.push(LexicalContext {
            context_root,
            function: None,
            // no need for global context to have a this/super/args context
            this_super_args_context: None,
        });
        id
    }

    /// Represents the context of a function or its parameter list.
    ///
    /// `parent`: enclosing context; `context_root`: FUNCTION or PARAM_LIST node; `function`:
    /// same as contextRoot or the FUNCTION containing the PARAM_LIST
    // port: RewriteAsyncIteration.LexicalContext#LexicalContext(LexicalContext,Node,Node,AbstractCompiler)
    fn new_lexical_context(
        &mut self,
        parent: ContextId,
        context_root: NodeId,
        function: Option<NodeId>,
        compiler: &mut AbstractCompiler,
    ) -> ContextId {
        check_argument!(
            Some(context_root) == function || context_root.is_param_list(compiler),
            &context_root.to_string(compiler)
        );
        let function = check_not_null!(function);
        check_argument!(
            function.is_function(compiler),
            &function.to_string(compiler)
        );
        let id = ContextId(self.contexts.len());

        let this_super_args_context = if function.is_arrow_function(compiler) {
            // Use the parent context to inherit this, arguments, and super for an arrow function
            // or its parameter list.
            self.ctx(parent).this_super_args_context
        } else if context_root.is_function(compiler) {
            // Non-arrow function gets its own context defining `this`, `arguments`, and `super`.
            let input_id = NodeUtil::get_input_id(compiler, context_root)
                .expect("java.lang.NullPointerException");
            let input = compiler
                .get_input(&input_id)
                .cloned()
                .expect("java.lang.NullPointerException");
            let new_unique_id = compiler.get_unique_id_supplier().get_unique_id(&input);
            let tsa = ThisSuperArgsContextId(self.this_super_args_contexts.len());
            self.this_super_args_contexts
                .push(ThisSuperArgsContext::new(id, new_unique_id));
            Some(tsa)
        } else {
            // contextRoot is a parameter list.
            // Never alias `this`, `arguments`, or `super` for normal function parameter lists.
            // They are implicitly defined there.
            None
        };
        self.contexts.push(LexicalContext {
            context_root,
            function: Some(function),
            this_super_args_context,
        });
        id
    }

    // port: RewriteAsyncIteration.LexicalContext#newContextForFunction
    fn new_context_for_function(
        &mut self,
        parent: ContextId,
        function: NodeId,
        compiler: &mut AbstractCompiler,
    ) -> ContextId {
        // Functions need their own context because:
        //     - async generator functions must be transpiled
        //     - non-async generator functions must NOT be transpiled
        //     - arrow functions inside of async generator functions need to have
        //       `this`, `arguments`, and `super` references aliased, including in their
        //       parameter lists
        self.new_lexical_context(parent, function, Some(function), compiler)
    }

    // port: RewriteAsyncIteration.LexicalContext#newContextForParamList
    fn new_context_for_param_list(
        &mut self,
        parent: ContextId,
        param_list: NodeId,
        compiler: &mut AbstractCompiler,
    ) -> ContextId {
        // Parameter lists need their own context because `this`, `arguments`, and `super` must
        // NOT be aliased for non-arrow function parameter lists, even for async generator
        // functions.
        let function = self.ctx(parent).function;
        self.new_lexical_context(parent, param_list, function, compiler)
    }

    // port: RewriteAsyncIteration.LexicalContext#getFunctionDeclaringThisArgsSuper
    fn get_function_declaring_this_args_super(&self, ctx: ContextId) -> NodeId {
        let tsa = self
            .ctx(ctx)
            .this_super_args_context
            .expect("java.lang.NullPointerException");
        self.ctx(self.tsa(tsa).ctx)
            .function
            .expect("java.lang.NullPointerException")
    }

    /// Is it necessary to replace `this`, `super`, and `arguments` with aliases in this context?
    // port: RewriteAsyncIteration.LexicalContext#mustReplaceThisSuperArgs
    fn must_replace_this_super_args(&self, compiler: &AbstractCompiler, ctx: ContextId) -> bool {
        self.ctx(ctx).this_super_args_context.is_some()
            && self
                .get_function_declaring_this_args_super(ctx)
                .is_async_generator_function(compiler)
    }

    /// Moves the body of an async generator function into a nested generator function and
    /// removes the async and generator props from the original function.
    ///
    /// ```text
    /// async function* foo() {
    ///   bar();
    /// }
    /// ```
    ///
    /// becomes
    ///
    /// ```text
    /// function foo() {
    ///   return new $jscomp.AsyncGeneratorWrapper((function*(){
    ///     bar();
    ///   })())
    /// }
    /// ```
    // port: RewriteAsyncIteration#convertAsyncGenerator
    fn convert_async_generator(&self, compiler: &mut AbstractCompiler, original_function: NodeId) {
        let af = &self.ast_factory;
        check_state!(original_function.is_async_generator_function(compiler));

        let async_generator_wrapper_ref = af.create_qname_for_field(
            compiler,
            &self.namespace,
            self.async_generator_wrapper.as_ref(),
        );
        let inner_function = af.create_empty_async_generator_wrapper_argument(compiler, None);

        let inner_block = original_function.get_last_child(compiler).unwrap();
        inner_block.detach(compiler);
        inner_function
            .get_last_child(compiler)
            .unwrap()
            .replace_with(compiler, inner_block);

        // Body should be:
        // return new $jscomp.AsyncGeneratorWrapper((new function with original block here)());
        let call = af.create_call(
            compiler,
            inner_function,
            AstFactory::type_color_id(standard_colors::GENERATOR_ID),
            &[],
        );
        let new_node = af.create_new_node(compiler, async_generator_wrapper_ref, &[call]);
        let ret = af.create_return(compiler, new_node);
        let outer_block = af.create_block(compiler, &[ret]);
        original_function.add_child_to_back(compiler, outer_block);

        original_function.set_is_async_function(compiler, false);
        original_function.set_is_generator_function(compiler, false);
        original_function.srcref_tree_if_missing(compiler, original_function);
        // Both the inner and original functions should be marked as changed.
        compiler.report_change_to_change_scope(original_function);
        compiler.report_change_to_change_scope(inner_function);
    }

    /// Converts an await into a yield of an ActionRecord to perform "AWAIT".
    ///
    /// `await myPromise` becomes `yield new ActionRecord(ActionEnum.AWAIT_VALUE, myPromise)`
    // port: RewriteAsyncIteration#convertAwaitOfAsyncGenerator
    fn convert_await_of_async_generator(
        &self,
        compiler: &mut AbstractCompiler,
        ctx: ContextId,
        await_node: NodeId,
    ) {
        let af = &self.ast_factory;
        check_state!(await_node.is_await(compiler));
        let function = self.ctx(ctx).function;
        check_state!(function.is_some());
        check_state!(function.unwrap().is_async_generator_function(compiler));

        let expression = await_node.remove_first_child(compiler);
        let expression = check_not_null!(expression, "await needs an expression");
        let record =
            af.create_qname_for_field(compiler, &self.namespace, self.action_record_name.as_ref());
        let enum_await =
            af.create_qname_for_field(compiler, &self.namespace, self.action_enum_await.as_ref());
        let new_action_record = af.create_new_node(compiler, record, &[enum_await, expression]);
        new_action_record.srcref_tree_if_missing(compiler, await_node);
        await_node.add_child_to_front(compiler, new_action_record);
        await_node.set_token(compiler, Token::YIELD);
    }

    /// Converts a yield into a yield of an ActionRecord to perform "YIELD" or "YIELD_STAR".
    ///
    /// ```text
    /// yield;
    /// yield first;
    /// yield* second;
    /// ```
    ///
    /// becomes
    ///
    /// ```text
    /// yield new ActionRecord(ActionEnum.YIELD_VALUE, undefined);
    /// yield new ActionRecord(ActionEnum.YIELD_VALUE, first);
    /// yield new ActionRecord(ActionEnum.YIELD_STAR, second);
    /// ```
    // port: RewriteAsyncIteration#convertYieldOfAsyncGenerator
    fn convert_yield_of_async_generator(
        &self,
        compiler: &mut AbstractCompiler,
        ctx: ContextId,
        yield_node: NodeId,
    ) {
        let af = &self.ast_factory;
        check_state!(yield_node.is_yield(compiler));
        let function = self.ctx(ctx).function;
        check_state!(function.is_some());
        check_state!(function.unwrap().is_async_generator_function(compiler));

        let expression = yield_node.remove_first_child(compiler);
        let record =
            af.create_qname_for_field(compiler, &self.namespace, self.action_record_name.as_ref());
        let new_action_record = af.create_new_node(compiler, record, &[]);

        if yield_node.is_yield_all(compiler) {
            let expression = check_not_null!(expression);
            // yield* expression becomes new ActionRecord(YIELD_STAR, expression)
            let enum_yield_star = af.create_qname_for_field(
                compiler,
                &self.namespace,
                self.action_enum_yield_star.as_ref(),
            );
            new_action_record.add_child_to_back(compiler, enum_yield_star);
            new_action_record.add_child_to_back(compiler, expression);
        } else {
            let expression = match expression {
                None => NodeUtil::new_undefined_node(compiler, None),
                Some(expression) => expression,
            };
            // yield expression becomes new ActionRecord(YIELD, expression)
            let enum_yield = af.create_qname_for_field(
                compiler,
                &self.namespace,
                self.action_enum_yield.as_ref(),
            );
            new_action_record.add_child_to_back(compiler, enum_yield);
            new_action_record.add_child_to_back(compiler, expression);
        }

        new_action_record.srcref_tree_if_missing(compiler, yield_node);
        yield_node.add_child_to_front(compiler, new_action_record);
        yield_node.put_boolean_prop(compiler, Prop::YIELD_ALL, false);
    }

    /// Converts a return into a return of an ActionRecord.
    ///
    /// ```text
    /// return;
    /// return value;
    /// ```
    ///
    /// becomes
    ///
    /// ```text
    /// return new ActionRecord(ActionEnum.YIELD_VALUE, undefined);
    /// return new ActionRecord(ActionEnum.YIELD_VALUE, value);
    /// ```
    // port: RewriteAsyncIteration#convertReturnOfAsyncGenerator
    fn convert_return_of_async_generator(
        &self,
        compiler: &mut AbstractCompiler,
        ctx: ContextId,
        return_node: NodeId,
    ) {
        let af = &self.ast_factory;
        check_state!(return_node.is_return(compiler));
        let function = self.ctx(ctx).function;
        check_state!(function.is_some());
        check_state!(function.unwrap().is_async_generator_function(compiler));

        let expression = return_node.remove_first_child(compiler);
        let record =
            af.create_qname_for_field(compiler, &self.namespace, self.action_record_name.as_ref());
        let new_action_record = af.create_new_node(compiler, record, &[]);

        let expression = match expression {
            None => NodeUtil::new_undefined_node(compiler, None),
            Some(expression) => expression,
        };
        // return expression becomes new ActionRecord(YIELD, expression)
        let enum_yield =
            af.create_qname_for_field(compiler, &self.namespace, self.action_enum_yield.as_ref());
        new_action_record.add_child_to_back(compiler, enum_yield);
        new_action_record.add_child_to_back(compiler, expression);
        new_action_record.srcref_tree_if_missing(compiler, return_node);
        return_node.add_child_to_front(compiler, new_action_record);
    }

    /// Rewrites for await of loop.
    ///
    /// ```text
    /// for await (lhs of rhs) { block(); }
    /// ```
    ///
    /// ...becomes...
    ///
    /// ```text
    /// var errorRes, retFn, tmpRes;
    /// try {
    ///   for (var tmpIterator = makeAsyncIterator(rhs);;) {
    ///      tmpRes = void 0;
    ///      tmpRes = await tmpIterator.next();
    ///      if (tmpRes.done) {
    ///        break;
    ///      }
    ///      lhs = $tmpRes.value;
    ///      {
    ///        block(); // Wrapped in a block in case block re-declares lhs variable.
    ///      }
    ///   }
    /// } catch(e) {
    ///   errorRes = { error: e };
    /// } finally {
    ///   try {
    ///     if (tmpRes && !tmpRes.done && (retFn = _tmpIterator.return)) await retFn.call(tmpIterator);
    ///   }
    ///   finally { if (errorRes) throw errorRes.error; }
    /// }
    /// ```
    // port: RewriteAsyncIteration#replaceForAwaitOf
    fn replace_for_await_of(
        &mut self,
        compiler: &mut AbstractCompiler,
        ctx: ContextId,
        for_await_of: NodeId,
    ) {
        let for_await_id = self.next_for_await_id;
        self.next_for_await_id += 1;
        let iterator_temp_name = format!("{FOR_AWAIT_ITERATOR_TEMP_NAME}{for_await_id}");
        let result_temp_name = format!("{FOR_AWAIT_RESULT_TEMP_NAME}{for_await_id}");
        let error_result_temp_name = format!("{FOR_AWAIT_ERROR_RESULT_TEMP_NAME}{for_await_id}");
        let catch_error_param_temp_name =
            format!("{FOR_AWAIT_CATCH_PARAM_TEMP_NAME}{for_await_id}");
        let return_func_temp_name = format!("{FOR_AWAIT_RETURN_FN_TEMP_NAME}{for_await_id}");

        let af = &self.ast_factory;
        check_state!(
            for_await_of.has_parent(compiler),
            "Cannot replace parentless for-await-of"
        );

        let for_await_of_parent = for_await_of.get_parent(compiler).unwrap();
        let replacement_point = if for_await_of_parent.is_label(compiler) {
            // If the forAwaitOf is a label's statement child, then the label must move with the
            // for upon rewriting.
            check_state!(
                for_await_of.is_second_child_of(compiler, Some(for_await_of_parent)),
                &for_await_of_parent.to_string(compiler)
            );
            for_await_of_parent
        } else {
            for_await_of
        };

        let lhs = for_await_of.remove_first_child(compiler).unwrap();
        let rhs = for_await_of.remove_first_child(compiler).unwrap();
        let original_body = for_await_of.remove_first_child(compiler).unwrap();

        // Generate `var tmpIterator = makeAsyncIterator(rhs);`
        let make_async_iterator =
            af.create_jscomp_make_async_iterator_call(compiler, rhs, &self.namespace);
        let initializer = af
            .create_single_var_name_declaration_with_value(
                compiler,
                iterator_temp_name.as_str(),
                make_async_iterator,
            )
            .srcref_tree_if_missing(compiler, rhs);

        // IIterableResult<VALUE> - it's a structural type so optimizations treat it as Object
        let iterable_result_type = AstFactory::type_(standard_colors::TOP_OBJECT.clone());

        // Create code `if (tmpRes.done) {break;}`
        let result_name = af.create_name(
            compiler,
            result_temp_name.as_str(),
            iterable_result_type.clone(),
        );
        let done = af.create_get_prop(
            compiler,
            result_name,
            "done",
            AstFactory::type_(standard_colors::BOOLEAN.clone()),
        );
        let break_node = af.create_break(compiler);
        let break_block = af.create_block(compiler, &[break_node]);
        let break_if_done = af.create_if(compiler, done, break_block);

        // Assignment statement to be moved from lhs into body of new for-loop
        let lhs_assignment: NodeId;
        let result_type: Type;
        if lhs.is_valid_assignment_target(compiler) {
            // In case of "for await (x of _)" just assign into the lhs.
            // Generate `lhs = $tmpRes.value;`
            result_type = AstFactory::type_node(lhs);
            let result_name = af.create_name(
                compiler,
                result_temp_name.as_str(),
                iterable_result_type.clone(),
            );
            let value = af.create_get_prop(compiler, result_name, "value", result_type.clone());
            let assign = af.create_assign(compiler, lhs, value);
            lhs_assignment = af.expr_result(compiler, assign);
        } else if NodeUtil::is_name_declaration(compiler, Some(lhs)) {
            let declaration_target = lhs.get_first_child(compiler).unwrap();
            if declaration_target.is_name(compiler) {
                // `for await (let x of _)`
                // Add a child to the `NAME` node to create `let x = res.value`
                result_type = AstFactory::type_node(declaration_target);
                let result_name = af.create_name(
                    compiler,
                    result_temp_name.as_str(),
                    iterable_result_type.clone(),
                );
                let value = af.create_get_prop(compiler, result_name, "value", result_type.clone());
                declaration_target.add_child_to_back(compiler, value);
            } else {
                // Generate `for await (let [x, y] of _)`
                // Add a child to the DESTRUCTURING_LHS node to create `[x, y] = res.value`
                check_state!(
                    declaration_target.is_destructuring_lhs(compiler),
                    &declaration_target.to_string(compiler)
                );
                let destructuring_pattern = declaration_target.get_only_child(compiler);
                result_type = AstFactory::type_node(destructuring_pattern);
                let result_name = af.create_name(
                    compiler,
                    result_temp_name.as_str(),
                    iterable_result_type.clone(),
                );
                let value = af.create_get_prop(compiler, result_name, "value", result_type.clone());
                declaration_target.add_child_to_back(compiler, value);
            }
            lhs_assignment = lhs;
        } else {
            panic!("java.lang.AssertionError: unexpected for-await-of lhs");
        }
        lhs_assignment.srcref_tree_if_missing(compiler, lhs);

        // Generate `var errorRes;`
        let error_res_decl = af
            .create_single_var_name_declaration(compiler, error_result_temp_name.as_str())
            .srcref_tree_if_missing(compiler, for_await_of);

        // Generate `var tmpRes;`
        let temp_result_decl = af
            .create_single_var_name_declaration(compiler, result_temp_name.as_str())
            .srcref_tree_if_missing(compiler, for_await_of);

        // Generate `var returnFunc;`
        let return_func_decl = af
            .create_single_var_name_declaration(compiler, return_func_temp_name.as_str())
            .srcref_tree_if_missing(compiler, for_await_of);

        // Generate `tmpRes = void 0;`
        let undefined = af.create_undefined_value(compiler);
        let assign = af.create_assign_to_name(compiler, result_temp_name.as_str(), undefined);
        let reset_temp_result = af.expr_result(compiler, assign);

        // Generate `tmpRes = await tmpIterator.next()`
        let await_next = self.construct_await_next_result(
            compiler,
            ctx,
            &iterator_temp_name,
            result_type.clone(),
            iterable_result_type.clone(),
        );
        let af = &self.ast_factory;
        let assign = af.create_assign_to_name(compiler, result_temp_name.as_str(), await_next);
        let result_declaration = af.expr_result(compiler, assign);

        let init = af.create_empty(compiler);
        let cond = af.create_empty(compiler);
        let incr = af.create_empty(compiler);
        let body = self.ensure_block(compiler, original_body);
        let af = &self.ast_factory;
        let for_body = af.create_block(
            compiler,
            &[
                reset_temp_result,
                result_declaration,
                break_if_done,
                lhs_assignment,
                body,
            ],
        );
        let mut new_for_loop = af.create_for(compiler, init, cond, incr, for_body);

        if replacement_point.is_label(compiler) {
            let label = replacement_point
                .get_first_child(compiler)
                .unwrap()
                .clone_node(compiler);
            new_for_loop = af.create_label(compiler, label, new_for_loop);
        }

        // Generates code `try { .. newForLoop .. }`
        let try_node = self.create_outer_try(compiler, new_for_loop);
        initializer.insert_before(compiler, new_for_loop);

        // Generate code `catch(e) { errorRes = { error: e }; }`
        let catch_node = self.create_outer_catch(
            compiler,
            &catch_error_param_temp_name,
            &error_result_temp_name,
        );

        // Generate the finally code block.
        let finally_node = self.create_outer_finally(
            compiler,
            ctx,
            iterable_result_type,
            result_type,
            &result_temp_name,
            &return_func_temp_name,
            &iterator_temp_name,
            &error_result_temp_name,
        );

        let try_catch_finally =
            self.ast_factory
                .create_try_catch_finally(compiler, try_node, catch_node, finally_node);
        replacement_point.replace_with(compiler, try_catch_finally);
        try_catch_finally.srcref_tree_if_missing(compiler, replacement_point);
        error_res_decl.insert_before(compiler, try_catch_finally);
        temp_result_decl.insert_before(compiler, try_catch_finally);
        return_func_decl.insert_before(compiler, try_catch_finally);

        compiler.report_change_to_enclosing_scope(try_catch_finally);
    }

    // Generates code `try { .. newForLoop .. }`
    // port: RewriteAsyncIteration#createOuterTry
    fn create_outer_try(&self, compiler: &mut AbstractCompiler, new_for_loop: NodeId) -> NodeId {
        let try_node = self.ast_factory.create_block(compiler, &[]);
        try_node.add_child_to_back(compiler, new_for_loop);
        try_node
    }

    // Generates code `catch(e) { errorRes = { error: e }; }`
    // port: RewriteAsyncIteration#createOuterCatch
    fn create_outer_catch(
        &self,
        compiler: &mut AbstractCompiler,
        catch_error_param_temp_name: &str,
        error_result_temp_name: &str,
    ) -> NodeId {
        let af = &self.ast_factory;
        // Generate `errorRes = { error: e };`
        let name = af.create_name_with_unknown_type(compiler, catch_error_param_temp_name);
        let string_key = af.create_string_key(compiler, "error", name);
        let object_lit = af.create_object_lit(compiler, &[string_key]);
        let assign = af.create_assign_to_name(compiler, error_result_temp_name, object_lit);
        let catch_body_stmt = af.expr_result(compiler, assign);
        // Generate `{ errorRes = { error: e }; }`
        let wrapper_catch_block_node = af.create_block(compiler, &[]);
        wrapper_catch_block_node.add_child_to_back(compiler, catch_body_stmt);

        // Generate `catch(e) { errorRes = { error: e }; }`
        let param = af.create_name_with_unknown_type(compiler, catch_error_param_temp_name);
        af.create_catch(compiler, param, wrapper_catch_block_node)
    }

    /// Generates the outer finally code of the rewriting.
    ///
    /// ```text
    /// finally {
    ///   try {
    ///     if (tmpRes && !tmpRes.done && (retFn = _tmpIterator.return)) await retFn.call(tmpIterator);
    ///   }
    ///   finally { if (errorRes) throw errorRes.error; }
    /// }
    /// ```
    // port: RewriteAsyncIteration#createOuterFinally
    #[allow(clippy::too_many_arguments)] // Java's signature
    fn create_outer_finally(
        &self,
        compiler: &mut AbstractCompiler,
        ctx: ContextId,
        iterable_result_type: Type,
        result_type: Type,
        result_temp_name: &str,
        return_func_temp_name: &str,
        iterator_temp_name: &str,
        error_result_temp_name: &str,
    ) -> NodeId {
        let af = &self.ast_factory;
        let finally_node = af.create_block(compiler, &[]);

        // Generate `tmpRes`
        let tmp_res_name_node = af.create_name_with_unknown_type(compiler, result_temp_name);
        let result_name = af.create_name(compiler, result_temp_name, iterable_result_type.clone());
        let tmp_res_done_get_prop = af.create_get_prop(
            compiler,
            result_name,
            "done",
            AstFactory::type_(standard_colors::BOOLEAN.clone()),
        );

        // Generate `tmpRes && !tmpRes.done`
        let not = af.create_not(compiler, tmp_res_done_get_prop);
        let and = af.create_and(compiler, tmp_res_name_node, not);

        // Generate `(retFn = _tmpIterator.return)`
        let ret_fn = af.create_name_with_unknown_type(compiler, return_func_temp_name);
        let iterator_name = af.create_name(compiler, iterator_temp_name, result_type.clone());
        let ret_get_prop = af.create_get_prop(
            compiler,
            iterator_name,
            "return",
            AstFactory::type_(standard_colors::UNKNOWN.clone()),
        );
        let assign = af.create_assign(compiler, ret_fn, ret_get_prop);

        // Generate `(tmpRes && !tmpRes.done && (retFn = _tmpIterator.return))`
        let if_cond = af.create_and(compiler, and, assign);
        let await_or_yield_stmt: NodeId;
        let function = self
            .ctx(ctx)
            .function
            .expect("java.lang.NullPointerException");
        if function.is_async_generator_function(compiler) {
            // We are in an AsyncGenerator and must instead yield an "await" ActionRecord
            let record = af.create_qname_for_field(
                compiler,
                &self.namespace,
                self.action_record_name.as_ref(),
            );
            let enum_await = af.create_qname_for_field(
                compiler,
                &self.namespace,
                self.action_enum_await.as_ref(),
            );
            let ret_fn_name = af.create_name(
                compiler,
                return_func_temp_name,
                AstFactory::type_(standard_colors::UNKNOWN.clone()),
            );
            let call_prop = af.create_get_prop_with_unknown_type(compiler, ret_fn_name, "call");
            let iterator_name = af.create_name(compiler, iterator_temp_name, result_type.clone());
            let call = af.create_call(
                compiler,
                call_prop,
                AstFactory::type_(standard_colors::UNKNOWN.clone()),
                &[iterator_name],
            );
            let new_node = af.create_new_node(compiler, record, &[enum_await, call]);
            let yield_node = af.create_yield(compiler, iterable_result_type.clone(), new_node);
            await_or_yield_stmt = af.expr_result(compiler, yield_node);
        } else {
            //  Generate `await retFn.call(tmpIterator);`
            let ret_fn_name = af.create_name(
                compiler,
                return_func_temp_name,
                AstFactory::type_(standard_colors::UNKNOWN.clone()),
            );
            let call_prop = af.create_get_prop_with_unknown_type(compiler, ret_fn_name, "call");
            let iterator_name = af.create_name(compiler, iterator_temp_name, result_type.clone());
            let call = af.create_call(
                compiler,
                call_prop,
                AstFactory::type_color_id(standard_colors::PROMISE_ID),
                &[iterator_name],
            );
            let await_node = af.create_await(compiler, iterable_result_type.clone(), call);
            await_or_yield_stmt = af.expr_result(compiler, await_node);
        }

        let if_body = af.create_block(compiler, &[]);
        if_body.add_child_to_back(compiler, await_or_yield_stmt);

        let if_block = af.create_if(compiler, if_cond, if_body);

        let inner_try_block = af.create_block(compiler, &[]);
        inner_try_block.add_child_to_back(compiler, if_block);

        //  `finally { if (errorRes) throw errorRes.error; }`
        let inner_finally_block = af.create_block(compiler, &[]);

        // if (errorRes) throw errorRes.error;
        let second_if_body = af.create_block(compiler, &[]);
        let error_res = af.create_name_with_unknown_type(compiler, error_result_temp_name);
        let error_prop = af.create_get_prop_with_unknown_type(compiler, error_res, "error");
        let throw_stmt = af.create_throw(compiler, error_prop);
        second_if_body.add_child_to_back(compiler, throw_stmt);
        let second_if_cond = af.create_name_with_unknown_type(compiler, error_result_temp_name);
        let second_if_block = af.create_if(compiler, second_if_cond, second_if_body);
        inner_finally_block.add_child_to_back(compiler, second_if_block);

        let finally_body = af.create_try_finally(compiler, inner_try_block, inner_finally_block);
        finally_node.add_child_to_back(compiler, finally_body);
        finally_node
    }

    // port: RewriteAsyncIteration#ensureBlock
    fn ensure_block(&self, compiler: &mut AbstractCompiler, possibly_block: NodeId) -> NodeId {
        if possibly_block.is_block(compiler) {
            possibly_block
        } else {
            self.ast_factory
                .create_block(compiler, &[possibly_block])
                .srcref(compiler, possibly_block)
        }
    }

    // port: RewriteAsyncIteration#constructAwaitNextResult
    fn construct_await_next_result(
        &self,
        compiler: &mut AbstractCompiler,
        ctx: ContextId,
        iterator_temp_name: &str,
        iterator_type: Type,
        iterable_result_type: Type,
    ) -> NodeId {
        let af = &self.ast_factory;
        let function = check_not_null!(self.ctx(ctx).function);
        let result: NodeId;

        let iterator_temp = af.create_name(compiler, iterator_temp_name, iterator_type);

        if function.is_async_generator_function(compiler) {
            // We are in an AsyncGenerator and must instead yield an "await" ActionRecord
            let record = af.create_qname_for_field(
                compiler,
                &self.namespace,
                self.action_record_name.as_ref(),
            );
            let enum_await = af.create_qname_for_field(
                compiler,
                &self.namespace,
                self.action_enum_await.as_ref(),
            );
            let next = af.create_get_prop_with_unknown_type(compiler, iterator_temp, "next");
            let call = af.create_call_with_unknown_type(compiler, next, &[]);
            let new_node = af.create_new_node(compiler, record, &[enum_await, call]);
            result = af.create_yield(compiler, iterable_result_type, new_node);
        } else {
            let next = af.create_get_prop_with_unknown_type(compiler, iterator_temp, "next");
            let call = af.create_call(
                compiler,
                next,
                AstFactory::type_color_id(standard_colors::PROMISE_ID),
                &[],
            );
            result = af.create_await(compiler, iterable_result_type, call);
        }

        result
    }

    // port: RewriteAsyncIteration#replaceThis
    fn replace_this(&mut self, compiler: &mut AbstractCompiler, ctx: ContextId, n: NodeId) {
        check_argument!(n.is_this(compiler));
        check_argument!(self.must_replace_this_super_args(compiler, ctx));
        let function = self.ctx(ctx).function;
        check_argument!(
            function.is_some(),
            "Cannot prepend declarations to root scope"
        );
        let tsa = check_not_null!(self.ctx(ctx).this_super_args_context);

        let name = format!("{THIS_VAR_NAME}{}", self.tsa(tsa).unique_id);
        let replacement = self
            .ast_factory
            .create_name(compiler, name, AstFactory::type_node(n))
            .srcref(compiler, n);
        n.replace_with(compiler, replacement);
        let this_node = self
            .ast_factory
            .create_this(compiler, AstFactory::type_node(n));
        self.tsa_mut(tsa).this_node_to_add = Some(this_node);
        compiler.report_change_to_change_scope(function.unwrap());
    }

    // port: RewriteAsyncIteration#replaceArguments
    fn replace_arguments(&mut self, compiler: &mut AbstractCompiler, ctx: ContextId, n: NodeId) {
        check_argument!(n.is_name(compiler) && n.get_string(compiler) == "arguments");
        check_argument!(self.must_replace_this_super_args(compiler, ctx));
        let function = self.ctx(ctx).function;
        check_argument!(
            function.is_some(),
            "Cannot prepend declarations to root scope"
        );
        let tsa = check_not_null!(self.ctx(ctx).this_super_args_context);

        let replacement = self
            .ast_factory
            .create_name(compiler, ARGUMENTS_VAR_NAME, AstFactory::type_node(n))
            .srcref(compiler, n);
        n.replace_with(compiler, replacement);
        self.tsa_mut(tsa).used_arguments = true;
        compiler.report_change_to_change_scope(function.unwrap());
    }

    // port: RewriteAsyncIteration#replaceSuper
    fn replace_super(
        &mut self,
        compiler: &mut AbstractCompiler,
        ctx: ContextId,
        n: NodeId,
        parent: NodeId,
    ) {
        if !parent.is_get_prop(compiler) {
            compiler.report(JSError::make(
                compiler,
                parent,
                &CANNOT_CONVERT_ASYNCGEN,
                &["super only allowed with getprop (like super.foo(), not super['foo']())"],
            ));
            return;
        }
        check_argument!(n.is_super(compiler));
        check_argument!(self.must_replace_this_super_args(compiler, ctx));
        let function = self.ctx(ctx).function;
        check_argument!(
            function.is_some(),
            "Cannot prepend declarations to root scope"
        );
        let tsa = check_not_null!(self.ctx(ctx).this_super_args_context);
        let unique_id = self.tsa(tsa).unique_id.clone();
        let context_root = self.ctx(ctx).context_root;

        let property_name = parent.get_string(compiler);
        let property_replacement_name_text = format!(
            "{SUPER_PROP_GETTER_PREFIX}{}",
            property_name.to_string_lossy()
        );

        // super.x   =>   $super$get$x()
        let af = &self.ast_factory;
        let getter_name = af.create_name(
            compiler,
            property_replacement_name_text,
            AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
        );
        let mut get_prop_replacement =
            af.create_call(compiler, getter_name, AstFactory::type_node(parent), &[]);
        let grandparent = parent.get_parent(compiler).unwrap();
        if grandparent.is_call(compiler) && grandparent.get_first_child(compiler) == Some(parent) {
            // super.x(args) => super.x.call($this, args)
            get_prop_replacement =
                af.create_get_prop_with_unknown_type(compiler, get_prop_replacement, "call");
            let member = context_root.get_parent(compiler).unwrap();
            let this_node_to_add = af.create_this_for_es6_class_member(compiler, member);
            self.tsa_mut(tsa).this_node_to_add = Some(this_node_to_add);
            let af = &self.ast_factory;
            af.create_name(
                compiler,
                format!("{THIS_VAR_NAME}{unique_id}"),
                AstFactory::type_node(this_node_to_add),
            )
            .srcref(compiler, parent)
            .insert_after(compiler, parent);
        } else if grandparent.is_tagged_template_lit(compiler)
            && grandparent.get_first_child(compiler) == Some(parent)
        {
            // super.x`template` => super.x().bind($this)`template`
            let member = context_root.get_parent(compiler).unwrap();
            let this_node_to_add = af.create_this_for_es6_class_member(compiler, member);
            self.tsa_mut(tsa).this_node_to_add = Some(this_node_to_add);
            let af = &self.ast_factory;
            let this_alias = af
                .create_name(
                    compiler,
                    format!("{THIS_VAR_NAME}{unique_id}"),
                    AstFactory::type_node(this_node_to_add),
                )
                .srcref(compiler, parent);
            let bind = af.create_get_prop_with_unknown_type(compiler, get_prop_replacement, "bind");
            get_prop_replacement = af.create_call_with_unknown_type(compiler, bind, &[this_alias]);
            grandparent.put_boolean_prop(compiler, Prop::FREE_CALL, true);
        }
        get_prop_replacement.srcref_tree(compiler, parent);
        parent.replace_with(compiler, get_prop_replacement);
        self.tsa_mut(tsa).used_super_properties.insert(parent);
        compiler.report_change_to_change_scope(function.unwrap());
    }

    /// Prepends this/super/argument replacement variables to the top of the context's block
    ///
    /// ```text
    /// function() {
    ///   return new AsyncGenWrapper(function*() {
    ///     // code using replacements for this and super.foo
    ///   }())
    /// }
    /// ```
    ///
    /// will be converted to
    ///
    /// ```text
    /// function() {
    ///   const $jscomp$asyncIter$this = this;
    ///   const $jscomp$asyncIter$super$get$foo = () => super.foo;
    ///   return new AsyncGenWrapper(function*() {
    ///     // code using replacements for this and super.foo
    ///   }())
    /// }
    /// ```
    // port: RewriteAsyncIteration#prependTempVarDeclarations
    fn prepend_temp_var_declarations(&self, ctx: ContextId, t: &mut NodeTraversal<'_>) {
        let function = self.ctx(ctx).function;
        check_argument!(
            function.is_some(),
            "Cannot prepend declarations to root scope"
        );
        let this_super_args_ctx = self.tsa(check_not_null!(self.ctx(ctx).this_super_args_context));

        let function = function.unwrap();
        let block = function.get_last_child(t);
        let block = check_not_null!(block, &function.to_string(t));
        let af = &self.ast_factory;
        // Temporary block to hold all declarations
        let prefix_block = af.create_block(t.get_compiler(), &[]);

        if let Some(this_node_to_add) = this_super_args_ctx.this_node_to_add {
            // { // prefixBlock
            //   const $jscomp$asyncIter$this = this;
            // }
            let declaration = af
                .create_single_const_name_declaration(
                    t.get_compiler(),
                    format!("{THIS_VAR_NAME}{}", this_super_args_ctx.unique_id),
                    this_node_to_add,
                )
                .srcref_tree(t, block);
            prefix_block.add_child_to_back(t, declaration);
        }
        if this_super_args_ctx.used_arguments {
            // { // prefixBlock
            //   const $jscomp$asyncIter$this = this;
            //   const $jscomp$asyncIter$arguments = arguments;
            // }
            let arguments_reference = af.create_arguments_reference(t.get_compiler());
            let declaration = af
                .create_single_const_name_declaration(
                    t.get_compiler(),
                    ARGUMENTS_VAR_NAME,
                    arguments_reference,
                )
                .srcref_tree(t, block);
            prefix_block.add_child_to_back(t, declaration);
        }
        for replaced_method_reference in this_super_args_ctx.used_super_properties.iter().copied() {
            let getter = self.create_super_method_reference_getter(replaced_method_reference, t);
            prefix_block.add_child_to_back(t, getter);
        }
        prefix_block.srcref_tree_if_missing(t, block);
        // Pulls all declarations out of prefixBlock and prepends in block
        // block: {
        //   // declarations
        //   // code using this/super/args
        // }
        let children = prefix_block.remove_children(t);
        block.add_children_to_front(t, children);

        if this_super_args_ctx.this_node_to_add.is_some()
            || this_super_args_ctx.used_arguments
            || !this_super_args_ctx.used_super_properties.is_empty()
        {
            t.get_compiler().report_change_to_change_scope(function);
            let script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::CONST_DECLARATIONS);
        }
    }

    // port: RewriteAsyncIteration#createSuperMethodReferenceGetter
    fn create_super_method_reference_getter(
        &self,
        replaced_method_reference: NodeId,
        t: &mut NodeTraversal<'_>,
    ) -> NodeId {
        let af = &self.ast_factory;
        // const super$get$x = () => { return super.x; };
        let type_of_super =
            AstFactory::type_node(replaced_method_reference.get_first_child(t).unwrap());
        let super_reference = af.create_super(t.get_compiler(), type_of_super);
        let replaced_method_name = replaced_method_reference.get_string(t);
        let get_prop = af.create_get_prop(
            t.get_compiler(),
            super_reference,
            replaced_method_name.clone(),
            AstFactory::type_node(replaced_method_reference),
        );
        let ret = af.create_return(t.get_compiler(), get_prop);
        let block = af.create_block(t.get_compiler(), &[ret]);
        let arrow_function =
            af.create_zero_arg_arrow_function_for_expression(t.get_compiler(), block);
        t.get_compiler()
            .report_change_to_change_scope(arrow_function);
        let script = t.get_current_script().unwrap();
        NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::ARROW_FUNCTIONS);
        let super_replacement_name = format!(
            "{SUPER_PROP_GETTER_PREFIX}{}",
            replaced_method_name.to_string_lossy()
        );
        af.create_single_const_name_declaration(
            t.get_compiler(),
            super_replacement_name,
            arrow_function,
        )
    }
}

impl CompilerPass for RewriteAsyncIteration {
    // port: RewriteAsyncIteration#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(self.context_stack.is_empty());
        let global_context = self.new_global_context(root);
        self.context_stack.push_front(global_context);
        TranspilationPasses::process_transpile(compiler, root, transpiled_features(), &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
        let first = *self
            .context_stack
            .front()
            .expect("java.util.NoSuchElementException");
        check_state!(self.ctx(first).function.is_none());
        self.context_stack
            .pop_front()
            .expect("java.util.NoSuchElementException");
        check_state!(self.context_stack.is_empty());
    }
}

impl Callback for RewriteAsyncIteration {
    // port: RewriteAsyncIteration#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_function(node_traversal) {
            let first = *self
                .context_stack
                .front()
                .expect("java.util.NoSuchElementException");
            let context = self.new_context_for_function(first, n, node_traversal.get_compiler());
            self.context_stack.push_front(context);
        } else if n.is_param_list(node_traversal) {
            let first = *self
                .context_stack
                .front()
                .expect("java.util.NoSuchElementException");
            let context = self.new_context_for_param_list(first, n, node_traversal.get_compiler());
            self.context_stack.push_front(context);
        }
        true
    }

    // port: RewriteAsyncIteration#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let ctx = *self
            .context_stack
            .front()
            .expect("java.util.NoSuchElementException");
        match n.get_token(t) {
            Token::PARAM_LIST => {
                // Async Generators (and popping contexts)
                // Done handling parameter list, so pop its context
                check_state!(n == self.ctx(ctx).context_root, &n.to_string(t));
                self.context_stack.pop_front();
            }
            Token::FUNCTION => {
                check_state!(n == self.ctx(ctx).context_root);
                if n.is_async_generator_function(t) {
                    self.convert_async_generator(t.get_compiler(), n);
                    self.prepend_temp_var_declarations(ctx, t);
                }
                // Done handling function, so pop its context
                self.context_stack.pop_front();
            }
            Token::AWAIT => {
                let function = check_not_null!(self.ctx(ctx).function);
                if function.is_async_generator_function(t) {
                    self.convert_await_of_async_generator(t.get_compiler(), ctx, n);
                }
            }
            Token::YIELD => {
                // Includes yield*
                let function = check_not_null!(self.ctx(ctx).function);
                if function.is_async_generator_function(t) {
                    self.convert_yield_of_async_generator(t.get_compiler(), ctx, n);
                }
            }
            Token::RETURN => {
                let function = check_not_null!(self.ctx(ctx).function);
                if function.is_async_generator_function(t) {
                    self.convert_return_of_async_generator(t.get_compiler(), ctx, n);
                }
                // For-Await-Of loops
            }
            Token::FOR_AWAIT_OF => {
                let function = check_not_null!(self.ctx(ctx).function);
                check_state!(function.is_async_function(t));
                self.replace_for_await_of(t.get_compiler(), ctx, n);
                let script = t.get_current_script().unwrap();
                NodeUtil::add_feature_to_script(
                    t.get_compiler(),
                    script,
                    Feature::CONST_DECLARATIONS,
                );
                // Maintaining references to this/arguments/super
            }
            Token::THIS => {
                if self.must_replace_this_super_args(t.get_compiler(), ctx) {
                    self.replace_this(t.get_compiler(), ctx, n);
                }
            }
            Token::NAME => {
                if self.must_replace_this_super_args(t.get_compiler(), ctx)
                    && n.matches_name(t, "arguments")
                {
                    self.replace_arguments(t.get_compiler(), ctx, n);
                }
            }
            Token::SUPER => {
                if self.must_replace_this_super_args(t.get_compiler(), ctx) {
                    let parent = parent.expect("java.lang.NullPointerException");
                    self.replace_super(t.get_compiler(), ctx, n, parent);
                }
            }
            _ => {}
        }
    }
}
