/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InstrumentAsyncContext.java.

//! Port of `InstrumentAsyncContext.java`.
use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    compiler_input::CompilerInput,
    compiler_pass::CompilerPass,
    js::runtime_js_lib_manager::JsLibField,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    this_and_arguments_reference_updater::{
        ThisAndArgumentsContext, ThisAndArgumentsReferenceUpdater,
    },
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    jsdoc_info::JSDocInfo,
    node::{NodeId, Prop},
};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

// NOTE: we prefix all internal symbols with a characteristic "ᵃᶜ" (U+1D43, U+1D9C) to
// significantly reduce the chance of conflicts with existing names in the code.  This isn't
// perfect, but it results in much more readable transpiled code compared to always generating a
// longer name, keeps tests simple and deterministic compared to using an ID generator, and is
// more performant than scanning for conflicting symbols to only disambiguate when necessary.
const FACTORY: &str = "ᵃᶜfactory";
const SUSPEND: &str = "ᵃᶜsuspend";
const RESUME: &str = "ᵃᶜresume";

/// Instruments `await` and `yield` for the `AsyncContext` polyfill.
///
/// Java keeps the compiler and its UniqueIdSupplier; here both are reached through the compiler
/// argument or the traversal (DESIGN §6).
pub struct InstrumentAsyncContext {
    start: Arc<dyn JsLibField>,

    ast_factory: AstFactory,

    // Whether `await`s should be instrumented.  When we're transpiling them away, we can skip
    // instrumenting them completely (the transpiled `yield`s don't need instrumentation because we
    // can rely on `Promise.then` to take care of it).
    should_instrument_await: bool,

    // TODO(sdh): Investigate whether async generator instrumentation can be skipped in ES2017
    // output.  They are transpiled to ordinary generators with Promise.then, so we may be able
    // to rely on runtime patching in that case as well.  If so, we'll add an additional boolean
    // constructor parameter for shouldInstrumentAsyncGenerators (and figure out whether that means
    // that both yield _and_ await can be skipped, or just await).
    try_function_stack: Vec<NodeId>,
    needs_instrumentation: IndexMap<NodeId, FunctionContext>,
    already_instrumented: IndexSet<NodeId>,
    has_super: IndexSet<NodeId>,
}

impl InstrumentAsyncContext {
    // port: InstrumentAsyncContext#InstrumentAsyncContext
    pub fn new(compiler: &mut AbstractCompiler, should_instrument_await: bool) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let runtime_js_lib_manager = compiler.get_runtime_js_lib_manager();
        let start = runtime_js_lib_manager
            .lock()
            .unwrap()
            .get_js_lib_field("$jscomp.asyncContextStart");
        Self {
            start,
            ast_factory,
            should_instrument_await,
            try_function_stack: Vec::new(),
            needs_instrumentation: IndexMap::new(),
            already_instrumented: IndexSet::new(),
            has_super: IndexSet::new(),
        }
    }

    // port: InstrumentAsyncContext#isReentrance
    fn is_reentrance(&self, t: &NodeTraversal<'_>, n: NodeId) -> bool {
        if n.is_yield(t) {
            return true;
        } else if !self.should_instrument_await {
            return false;
        }
        n.is_for_await_of(t) || n.is_await(t)
    }

    // When we see a reentrance node, we need to do several things:
    //  1. replace `await EXPR` with `ᵃᶜresume(await ᵃᶜsuspend(EXPR))`
    //  2. ensure the enclosing function has been instrumented with `$jscomp.asyncContextStart` and a
    //     `try`-`finally`
    //  3. look for an immediately-enclosing `try` block and instrument it with `ᵃᶜresume`
    // port: InstrumentAsyncContext#instrumentReentrance
    fn instrument_reentrance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: NodeId) {
        let mut enclosing_try = self.try_function_stack.last().copied();
        if enclosing_try.is_some_and(|enclosing_try| !enclosing_try.is_try(t)) {
            enclosing_try = None; // not a try
        }
        let Some(enclosing_function) = t.get_enclosing_function() else {
            // NOTE: This is a top-level await, which is currently not supported.  If it becomes
            // supported then we should consider instrumenting the module body as if it were a
            // function.
            // See https://github.com/google/closure-compiler/issues/3835.
            return;
        };
        if self.already_instrumented.contains(&enclosing_function) {
            // This function is already instrumented, so don't do any further instrumentation.
            return;
        } else if parent.is_function(t) {
            // `async (...) => await ...` - no reentrance, so no instrumentation needed.
            return;
        } else if parent.is_return(t) && enclosing_try.is_none() {
            // `return await ...` - no reentrance unless we're inside a `try`.
            return;
        }

        // At this point, instrumentation is required, both for the immediate node, as well as for
        // any enclosing function or try block.
        let function_context = match self.needs_instrumentation.get(&enclosing_function) {
            Some(context) => context.clone(),
            None => {
                let input = check_not_null!(t.get_input().cloned());
                let context = self.create_new_function_context(t.get_compiler(), &input);
                self.needs_instrumentation
                    .insert(enclosing_function, context.clone());
                context
            }
        };
        if let Some(enclosing_try) = enclosing_try {
            self.needs_instrumentation
                .insert(enclosing_try, function_context.clone());
        }

        if n.is_for_await_of(t) {
            self.instrument_for_await_of(t, n, parent, &function_context);
            return;
        }

        // Wrap yielded/awaited expression with a ᵃᶜsuspend(...) call
        let placeholder = IR::empty(t);
        let arg = n.get_first_child(t);
        match arg {
            None => {
                let suspend = function_context
                    .suspend(t.get_compiler(), &self.ast_factory)
                    .srcref_tree_if_missing(t, n);
                n.add_child_to_back(t, suspend);
            }
            Some(arg) => {
                arg.replace_with(t, placeholder);
                let suspend = function_context
                    .suspend_inner(t.get_compiler(), &self.ast_factory, arg)
                    .srcref_tree_if_missing(t, arg);
                placeholder.replace_with(t, suspend);
            }
        }

        // Wrap the entire yield/await with a ᵃᶜresume(...) call
        n.replace_with(t, placeholder);
        let resume = function_context
            .resume_inner(t.get_compiler(), &self.ast_factory, n)
            .srcref_tree_if_missing(t, n);
        placeholder.replace_with(t, resume);
    }

    // port: InstrumentAsyncContext#instrumentForAwaitOf
    fn instrument_for_await_of(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
        function_context: &FunctionContext,
    ) {
        // for await (const x of expr()) {
        //   use(x);
        // }
        //     becomes
        // for await (const x of suspend(expr())) {
        //   resume();
        //   try {
        //     use(x);
        //   } finally {
        //     suspend();
        //   }
        // }
        // resume();

        // First wrap the iterator argument
        let placeholder = IR::empty(t);
        let arg = check_not_null!(n.get_second_child(t));
        arg.replace_with(t, placeholder);
        let suspend = function_context
            .suspend_inner(t.get_compiler(), &self.ast_factory, arg)
            .srcref_tree_if_missing(t, arg);
        placeholder.replace_with(t, suspend);

        let body = check_not_null!(n.get_last_child(t));
        check_state!(body.is_block(t)); // NOTE: IRFactory normalizes non-block `for` bodies
        body.replace_with(t, placeholder);
        let resume = function_context
            .resume(t.get_compiler(), &self.ast_factory)
            .srcref_tree_if_missing(t, arg);
        let resume_statement = IR::expr_result(t, resume);
        let suspend = function_context
            .suspend(t.get_compiler(), &self.ast_factory)
            .srcref_tree_if_missing(t, arg);
        let suspend_statement = IR::expr_result(t, suspend);
        let finally_block = IR::block_with_child(t, suspend_statement);
        let try_finally = IR::try_finally(t, body, finally_block);
        let new_body = IR::block_with_children(t, &[resume_statement, try_finally]);
        placeholder.replace_with(t, new_body);

        // Add resume call after for-await-of
        let mut n = n;
        if !parent.is_block(t) {
            // NOTE: if the for-await-of is _not_ the child of a block, we could be in trouble.
            let block = IR::block(t);
            n.replace_with(t, block);
            block.add_child_to_front(t, n);
            n = block;
        }
        let resume = function_context
            .resume(t.get_compiler(), &self.ast_factory)
            .srcref_tree_if_missing(t, arg);
        IR::expr_result(t, resume).insert_after(t, n);
    }

    // port: InstrumentAsyncContext#instrumentTry
    fn instrument_try(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        function_context: &FunctionContext,
    ) {
        // Find either a catch or a finally block (if there is no catch).
        let mut block = check_not_null!(n.get_second_child(compiler));
        if block.has_children(compiler) {
            // There's a catch block: descend into the actual block
            block = check_not_null!(
                check_not_null!(block.get_first_child(compiler)).get_second_child(compiler)
            );
        } else {
            // Look for a finally block instead
            block = check_not_null!(block.get_next(compiler));
        }
        check_state!(block.is_block(compiler));
        // Prepend an empty resume call: ᵃᶜresume()
        let resume = function_context
            .resume(compiler, &self.ast_factory)
            .srcref_tree_if_missing(compiler, block);
        let resume_statement = IR::expr_result(compiler, resume);
        block.add_child_to_front(compiler, resume_statement);
    }

    // port: InstrumentAsyncContext#instrumentGeneratorFunction
    fn instrument_generator_function(
        &mut self,
        t: &mut NodeTraversal<'_>,
        f: NodeId,
        function_context: &FunctionContext,
    ) {
        // If a function is completely empty then there's no need to instrument it
        // because there's nothing within that can observe the state of any variables.
        if !check_not_null!(f.get_last_child(t)).has_children(t) {
            return;
        }
        // NOTE: if `f` is a generator _method_ then we need to be more careful.
        // Because generators are not allowed to be arrow functions, we need to
        // use the ThisAndArgumentsReferenceUpdater to extract any references to
        // those symbols within the vanilla IIFE generator.  But this doesn't help
        // with any references to `super`.  Instead, for methods we have a clear
        // method body and can move the generator body into a newly synthesized
        // method that we can directly.  If "arguments" is used, then we need to
        // call it with .apply and forward the arguments.  This is dangerous for
        // certain methods that might be collapsed (e.g. static methods), so we
        // only do when necessary (i.e. `super` is used), which corresponds to a
        // known-safe case (since `super` can't be collapsed).
        if self.has_super.contains(&f) {
            self.instrument_generator_method(t, f, function_context);
            return;
        }

        // Turn the generator into an ordinary function with an IIFE generator
        f.set_is_generator_function(t, false);
        let mut generator_body = check_not_null!(f.get_last_child(t)).detach(t);
        let inner = self
            .ast_factory
            .create_empty_generator_function(t.get_compiler(), AstFactory::type_node(f));

        if f.is_async_function(t) {
            f.set_is_async_function(t, false);
            inner.set_is_async_function(t, true);
        }
        check_not_null!(inner.get_last_child(t)).replace_with(t, generator_body);

        generator_body =
            self.add_finally_suspend(t.get_compiler(), generator_body, function_context);
        let resume = function_context.resume(t.get_compiler(), &self.ast_factory);
        let resume_statement = IR::expr_result(t, resume);
        generator_body.add_child_to_front(t, resume_statement);
        let factory_suspend_call =
            self.create_factory_suspend_call(t.get_compiler(), function_context.factory_name);
        let inner_call =
            self.ast_factory
                .create_call_with_unknown_type(t.get_compiler(), inner, &[]);
        let return_node = IR::return_node_with_expression(t, inner_call);
        let new_outer_body = IR::block_with_children(t, &[factory_suspend_call, return_node]);
        let first = check_not_null!(new_outer_body.get_first_child(t));
        self.add_suspend_resume_vars_after(t.get_compiler(), first, function_context);
        let new_outer_body = new_outer_body.srcref_tree_if_missing(t, generator_body);
        f.add_child_to_back(t, new_outer_body);

        // NOTE: if the IIFE generator refers to `this` or `arguments` then we need to extract these
        // references into local variables.
        let input = check_not_null!(t.get_input().cloned());
        let unique_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let mut this_context = ThisAndArgumentsContext::new(new_outer_body, false, unique_id);
        {
            let mut updater =
                ThisAndArgumentsReferenceUpdater::new(&mut this_context, &self.ast_factory);
            NodeTraversal::traverse(t.get_compiler(), generator_body, &mut updater);
        }
        // Check the context to see if we found this or arguments.
        let script = check_not_null!(t.get_current_script());
        this_context.add_var_declarations(t.get_compiler(), &self.ast_factory, script);

        t.get_compiler().report_change_to_change_scope(f);
        t.get_compiler().report_change_to_change_scope(inner);
        // Instrumentation may move functions into a new nested scope.
        NodeUtil::add_feature_to_script(
            t.get_compiler(),
            script,
            Feature::BLOCK_SCOPED_FUNCTION_DECLARATION,
        );
    }

    // port: InstrumentAsyncContext#instrumentGeneratorMethod
    fn instrument_generator_method(
        &mut self,
        t: &mut NodeTraversal<'_>,
        f: NodeId,
        function_context: &FunctionContext,
    ) {
        // This requires special handling compared to ordinary generator functions because
        // it may reference `super`, which would break in a vanilla IIFE.  Instead, copy the
        // body to a new method with the same parameters, plus one or two more for the
        // context and possibly `arguments`.

        // Specifically:
        //     *m(a, {b: c=1}, d=2, ...e) { ... }
        // would become
        //     *m$jscomp$123(ᵃᶜfactory, a, {b: c=1}, d=2, ...e) {
        //       var ᵃᶜsuspend = ᵃᶜfactory();
        //       var ᵃᶜresume = ᵃᶜfactory(1);
        //       try { ... } finally { ᵃᶜsuspend(); }
        //     }
        //     m(a, $jscomp$0, d, ...e) {
        //       const ᵃᶜfactory = $jscomp.asyncContextStart();
        //       return this.m$jscomp$123(ᵃᶜfactory, a, $jscomp$0, d, ...e);
        //     }
        // If the function refers to `arguments`, then an additional
        // `ᵃᶜarguments` parameter is added to the synthesized method after
        // the context argument.

        let f_parent = check_not_null!(f.get_parent(t));
        let method_prefix = if f_parent.is_member_function_def(t) {
            f_parent.get_string(t).to_string()
        } else {
            String::new()
        };
        let input = check_not_null!(t.get_input().cloned());
        let inner_method_name = format!(
            "{}$jscomp${}",
            method_prefix,
            t.get_compiler()
                .get_unique_id_supplier()
                .get_unique_id(&input)
        );
        let outer_params = check_not_null!(f.get_second_child(t));
        let inner_params = outer_params.clone_tree(t);

        // Add the ᵃᶜfactory parameter to the front of the inner parameter list
        let inner_factory_name = function_context.factory_name;
        let outer_factory_unique_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let outer_factory_name = self.ast_factory.create_name_with_unknown_type(
            t.get_compiler(),
            format!("{FACTORY}{outer_factory_unique_id}"),
        );
        inner_params.add_child_to_front(t, inner_factory_name);
        let unknown_type = AstFactory::type_node(inner_factory_name);

        // Add a finally-suspend wrapper and an initial resume call around the original generator
        // body (which has already had its yields/awaits instrumented).
        let f_body = check_not_null!(f.get_last_child(t));
        let generator_body = self.add_finally_suspend(t.get_compiler(), f_body, function_context);
        let resume = function_context.resume(t.get_compiler(), &self.ast_factory);
        let resume_statement = IR::expr_result(t, resume);
        generator_body.add_child_to_front(t, resume_statement);

        // Look for references to `arguments`.  If found, add an extra parameter to the inner
        // method. We need to save a reference to the parameter in order to replace it in the
        // function call arguments.
        let mut arguments_param = None;
        let mut arguments_reference = None;
        let mut renamer = ArgumentsRenamer {
            arguments_name: None,
        };
        NodeTraversal::traverse(t.get_compiler(), generator_body, &mut renamer);
        if let Some(arguments_name) = renamer.arguments_name {
            let reference = self
                .ast_factory
                .create_arguments_reference(t.get_compiler());
            let param = self.ast_factory.create_name(
                t.get_compiler(),
                arguments_name,
                AstFactory::type_node(reference),
            );
            param.insert_after(t, inner_factory_name);
            arguments_reference = Some(reference);
            arguments_param = Some(param);
        }

        // Detach the generator body and replace it with a new empty block.  We'll fill the empty
        // block later, and will insert the instrumented generator body into the new inner method.
        let outer_body = IR::block(t);
        generator_body.replace_with(t, outer_body);

        // Build the call to the inner method.  Iterate over innerParams to build up the arguments
        // list, replacing any non-simple arguments with simple names.
        let this_node = self.ast_factory.create_this(t.get_compiler(), unknown_type);
        let get_prop = self.ast_factory.create_get_prop(
            t.get_compiler(),
            this_node,
            inner_method_name.as_str(),
            AstFactory::type_node(f),
        );
        let inner_call =
            self.ast_factory
                .create_call_with_unknown_type(t.get_compiler(), get_prop, &[]);
        // Copy the context parameter.
        let mut inner_param = inner_params.get_first_child(t);
        let outer_factory_name_clone = outer_factory_name.clone_tree(t);
        inner_call.add_child_to_back(t, outer_factory_name_clone);
        inner_param = check_not_null!(inner_param).get_next(t);
        // Copy the arguments parameter if it's there.
        if inner_param.is_some() && inner_param == arguments_param {
            inner_call.add_child_to_back(t, check_not_null!(arguments_reference));
        }
        // Copy the remaining parameters.
        let mut outer_param = outer_params.get_first_child(t);
        while let Some(mut current) = outer_param {
            let current_clone = current.clone_tree(t);
            let arg_param_pair = self.simplify_parameter(t, current_clone);
            current.replace_with(t, arg_param_pair.parameter);
            current = arg_param_pair.parameter;
            inner_call.add_child_to_back(t, arg_param_pair.argument);
            outer_param = current.get_next(t);
        }

        // Make a new inner method with the instrumented generator body, and insert it after the
        // original method.
        let new_inner_method = self.ast_factory.create_function(
            t.get_compiler(),
            "",
            inner_params,
            generator_body,
            AstFactory::type_node(f),
        );
        IR::member_function_def(t, inner_method_name.as_str(), new_inner_method)
            .insert_after(t, f_parent);
        check_not_null!(new_inner_method.get_first_child(t)).make_non_indexable(t);

        // Move the async/generator flags from the original method to the new one.
        let is_async = f.is_async_function(t);
        new_inner_method.set_is_async_function(t, is_async);
        new_inner_method.set_is_generator_function(t, true);
        f.set_is_generator_function(t, false);
        f.set_is_async_function(t, false);

        // Copy static and/or @nocollapse from the parent (member_function_def) node.
        let fp = check_not_null!(f.get_parent(t));
        let np = check_not_null!(new_inner_method.get_parent(t));
        let is_static = fp.is_static_member(t);
        np.set_static_member(t, is_static);
        let jsdoc = fp.get_jsdoc_info(t);
        if let Some(jsdoc) = jsdoc {
            let mut builder = JSDocInfo::builder();
            if jsdoc.is_no_collapse() {
                builder.record_no_collapse();
            }
            if jsdoc.is_no_inline() {
                builder.record_no_inline();
            }
            np.set_jsdoc_info(t, builder.build());
        }

        // Populate the outer (non-generator) method body with an start() and call to the inner
        // method.
        let factory_suspend_call =
            self.create_factory_suspend_call(t.get_compiler(), outer_factory_name);
        outer_body.add_child_to_back(t, factory_suspend_call);
        self.add_suspend_resume_vars_at_start_of(
            t.get_compiler(),
            generator_body,
            inner_factory_name,
            function_context,
        );
        let return_node = IR::return_node_with_expression(t, inner_call);
        outer_body.add_child_to_back(t, return_node);

        // Various cleanups
        f.srcref_tree_if_missing(t, f);
        np.srcref_tree_if_missing(t, f);
        let script = check_not_null!(t.get_current_script());
        NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::MEMBER_DECLARATIONS);
        t.get_compiler().report_change_to_change_scope(f);
        t.get_compiler()
            .report_change_to_change_scope(new_inner_method);
        t.get_compiler().report_change_to_enclosing_scope(fp);
        // Instrumentation may move functions into a new nested scope.
        NodeUtil::add_feature_to_script(
            t.get_compiler(),
            script,
            Feature::BLOCK_SCOPED_FUNCTION_DECLARATION,
        );
    }

    // port: InstrumentAsyncContext#simplifyParameter
    fn simplify_parameter(&mut self, t: &mut NodeTraversal<'_>, param: NodeId) -> ArgParamPair {
        if param.is_rest(t) {
            // Replace REST with SPREAD.  For arguments, we know it has to be ITER_REST and
            // ITER_SPREAD.
            let script = check_not_null!(t.get_current_script());
            NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::SPREAD_EXPRESSIONS);
            let param_element = check_not_null!(param.remove_first_child(t));
            let simplified = self.simplify_parameter(t, param_element);
            let argument = IR::iter_spread(t, simplified.argument).srcref_tree_if_missing(t, param);
            let parameter = IR::iter_rest(t, simplified.parameter).srcref_tree_if_missing(t, param);
            return ArgParamPair {
                argument,
                parameter,
            };
        } else if param.is_default_value(t) {
            // Ignore any initializers; replace with just the name.
            let first = check_not_null!(param.remove_first_child(t));
            return self.simplify_parameter(t, first);
        } else if param.is_array_pattern(t) || param.is_object_pattern(t) {
            // Replace destructuring patterns with a synthesized name.
            let input = check_not_null!(t.get_input().cloned());
            let unique_id = t
                .get_compiler()
                .get_unique_id_supplier()
                .get_unique_id(&input);
            let new_param = self
                .ast_factory
                .create_name(
                    t.get_compiler(),
                    format!("$jscomp$param${unique_id}"),
                    AstFactory::type_node(param),
                )
                .srcref_tree_if_missing(t, param);
            let argument = new_param.clone_tree(t);
            return ArgParamPair {
                argument,
                parameter: new_param,
            };
        } else if param.is_name(t) {
            // Replace destructuring patterns with a synthesized name.
            let input = check_not_null!(t.get_input().cloned());
            let name = param.get_string(t).to_string();
            let unique_id = t
                .get_compiler()
                .get_unique_id_supplier()
                .get_unique_id(&input);
            let new_param = self
                .ast_factory
                .create_name(
                    t.get_compiler(),
                    format!("{name}$jscomp$param${unique_id}"),
                    AstFactory::type_node(param),
                )
                .srcref_tree_if_missing(t, param);
            let argument = new_param.clone_tree(t);
            return ArgParamPair {
                argument,
                parameter: new_param,
            };
        }
        panic!(
            "IllegalStateException: Unexpected parameter: {}",
            param.to_string(t)
        );
    }

    /// Given an async function node, instrument it by wrapping the body in a try-finally and adding
    /// an "start" call to the front.
    // port: InstrumentAsyncContext#instrumentAsyncFunction
    fn instrument_async_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        f: NodeId,
        function_context: &FunctionContext,
    ) {
        // Add a variable to the front of the body
        let block = check_not_null!(f.get_last_child(compiler));
        let new_block = self.add_finally_suspend(compiler, block, function_context);
        let factory_call = self
            .create_factory_call(compiler, function_context.factory_name, &[])
            .srcref_tree_if_missing(compiler, new_block);
        new_block.add_child_to_front(compiler, factory_call);
        let first = check_not_null!(new_block.get_first_child(compiler));
        self.add_suspend_resume_vars_after(compiler, first, function_context);
        compiler.report_change_to_change_scope(f);

        // TODO(sdh): Need to instrument async functions for unhandled rejections.
        // Ideally this would happen after devirtualization and then we can extract a
        // helper function to call rather than making a new closure.  Would also be nice
        // to not even bother with it if we're not worrying about that case.
    }

    /// Replace a function body with a new body containing only try (original body) finally
    /// (suspend).
    // port: InstrumentAsyncContext#addFinallySuspend
    fn add_finally_suspend(
        &self,
        compiler: &mut AbstractCompiler,
        block: NodeId,
        function_context: &FunctionContext,
    ) -> NodeId {
        let placeholder = IR::empty(compiler);
        block.replace_with(compiler, placeholder);
        let suspend = function_context.suspend(compiler, &self.ast_factory);
        let suspend_statement = IR::expr_result(compiler, suspend);
        let finally_block = IR::block_with_child(compiler, suspend_statement);
        let try_finally = IR::try_finally(compiler, block, finally_block);
        let new_block =
            IR::block_with_child(compiler, try_finally).srcref_tree_if_missing(compiler, block);
        placeholder.replace_with(compiler, new_block);
        new_block
    }

    // port: InstrumentAsyncContext#addSuspendResumeVarsAfter
    fn add_suspend_resume_vars_after(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        function_context: &FunctionContext,
    ) {
        // var ᵃᶜsuspend = ᵃᶜfactory();
        // var ᵃᶜresume = ᵃᶜfactory(1);
        let factory_name = function_context.factory_name.clone_node(compiler);
        let one = self.ast_factory.create_number(compiler, 1.0);
        let resume_call =
            self.ast_factory
                .create_call_with_unknown_type(compiler, factory_name, &[one]);
        IR::var_with_value(compiler, function_context.resume_name, resume_call)
            .srcref_tree_if_missing(compiler, node)
            .insert_after(compiler, node);
        let factory_name = function_context.factory_name.clone_node(compiler);
        let suspend_call =
            self.ast_factory
                .create_call_with_unknown_type(compiler, factory_name, &[]);
        IR::var_with_value(compiler, function_context.suspend_name, suspend_call)
            .srcref_tree_if_missing(compiler, node)
            .insert_after(compiler, node);
    }

    // port: InstrumentAsyncContext#addSuspendResumeVarsAtStartOf
    fn add_suspend_resume_vars_at_start_of(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        factory_name: NodeId,
        function_context: &FunctionContext,
    ) {
        // var ᵃᶜsuspend = ᵃᶜfactory();
        // var ᵃᶜresume = ᵃᶜfactory(1);
        let factory_name_clone = factory_name.clone_node(compiler);
        let one = self.ast_factory.create_number(compiler, 1.0);
        let resume_call =
            self.ast_factory
                .create_call_with_unknown_type(compiler, factory_name_clone, &[one]);
        let resume_var = IR::var_with_value(compiler, function_context.resume_name, resume_call)
            .srcref_tree_if_missing(compiler, node);
        node.add_child_to_front(compiler, resume_var);
        let factory_name_clone = factory_name.clone_node(compiler);
        let suspend_call =
            self.ast_factory
                .create_call_with_unknown_type(compiler, factory_name_clone, &[]);
        let suspend_var = IR::var_with_value(compiler, function_context.suspend_name, suspend_call)
            .srcref_tree_if_missing(compiler, node);
        node.add_child_to_front(compiler, suspend_var);
    }

    // port: InstrumentAsyncContext#createVar
    fn create_var(compiler: &mut AbstractCompiler, name: NodeId, value: NodeId) -> NodeId {
        IR::var_with_value(compiler, name, value)
    }

    /// Creates a statement `const ᵃᶜfactory = $jscomp$asyncContextStart();`.
    // port: InstrumentAsyncContext#createFactoryCall
    fn create_factory_call(
        &self,
        compiler: &mut AbstractCompiler,
        factory_name: NodeId,
        arg: &[NodeId],
    ) -> NodeId {
        let callee = self
            .ast_factory
            .create_qname_with_unknown_type_for_field(compiler, &*self.start);
        let call = self
            .ast_factory
            .create_call_with_unknown_type(compiler, callee, arg);
        Self::create_var(compiler, factory_name, call)
    }

    /// Creates a statement `const ᵃᶜfactory = $jscomp$asyncContextStart(1);`.
    // port: InstrumentAsyncContext#createFactorySuspendCall
    fn create_factory_suspend_call(
        &self,
        compiler: &mut AbstractCompiler,
        factory_name: NodeId,
    ) -> NodeId {
        let one = self.ast_factory.create_number(compiler, 1.0);
        self.create_factory_call(compiler, factory_name, &[one])
    }

    // port: InstrumentAsyncContext#createNewFunctionContext
    fn create_new_function_context(
        &self,
        compiler: &mut AbstractCompiler,
        input: &CompilerInput,
    ) -> FunctionContext {
        let unique_suffix = compiler.get_unique_id_supplier().get_unique_id(input);
        // Creates a unique name, prefixed with `ᵃᶜsuspend`
        let suspend_node = self
            .ast_factory
            .create_name_with_unknown_type(compiler, format!("{SUSPEND}{unique_suffix}"));

        // Creates a unique name, prefixed with `ᵃᶜresume`
        let resume_node = self
            .ast_factory
            .create_name_with_unknown_type(compiler, format!("{RESUME}{unique_suffix}"));

        // Creates a unique name, prefixed with `ᵃᶜfactory`
        let factory_name = self
            .ast_factory
            .create_name_with_unknown_type(compiler, format!("{FACTORY}{unique_suffix}"));

        FunctionContext {
            unique_suffix,
            suspend_name: suspend_node,
            resume_name: resume_node,
            factory_name,
        }
    }
}

impl CompilerPass for InstrumentAsyncContext {
    // port: InstrumentAsyncContext#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(
            compiler.get_life_cycle_stage().is_normalized(),
            "%s",
            format!("{:?}", compiler.get_life_cycle_stage())
        );
        // Scan through code and find uses of AsyncContext. Look specifically for Variable.run.
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for InstrumentAsyncContext {
    // port: InstrumentAsyncContext#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // The top of tryFunctionStack should always point to the innermost try or function block.
        // This is a minor optimization to avoid recursing up the ancestor chain to determine if a
        // reentrance is within a try block, in which case a "resume" call needs to be added to the
        // subsequent catch or finally.  We add functions to the stack as well because functions
        // have their own separate scope, so any try block surrounding the function body should
        // _not_ be instrumented.
        if n.is_try(t) || n.is_function(t) {
            self.try_function_stack.push(n);
        }
        true
    }

    // port: InstrumentAsyncContext#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) && n.get_string_ref(t).to_string().starts_with(FACTORY) {
            // Java adds a null enclosing function to the set; no function node ever matches it.
            if let Some(enclosing_function) = t.get_enclosing_function() {
                self.already_instrumented.insert(enclosing_function);
            }
        } else if n.is_super(t) {
            if let Some(enclosing_function) = t.get_enclosing_function() {
                self.has_super.insert(enclosing_function);
            }
        } else if self.is_reentrance(t, n) {
            self.instrument_reentrance(t, n, check_not_null!(parent));
        } else if self.try_function_stack.last() == Some(&n) {
            self.try_function_stack.pop();
            if self.already_instrumented.contains(&n) {
                return;
            }
            let context = if n.is_generator_function(t) {
                match self.needs_instrumentation.get(&n) {
                    Some(context) => Some(context.clone()),
                    None => {
                        let input = check_not_null!(t.get_input().cloned());
                        let context = self.create_new_function_context(t.get_compiler(), &input);
                        self.needs_instrumentation.insert(n, context.clone());
                        Some(context)
                    }
                }
            } else {
                self.needs_instrumentation.get(&n).cloned()
            };
            let Some(context) = context else {
                return;
            };
            if n.is_try(t) {
                self.instrument_try(t.get_compiler(), n, &context);
            } else if n.is_generator_function(t) {
                self.instrument_generator_function(t, n, &context);
            } else {
                check_state!(n.is_async_function(t));
                self.instrument_async_function(t.get_compiler(), n, &context);
            }
        }
    }
}

// port: InstrumentAsyncContext.ArgParamPair
struct ArgParamPair {
    argument: NodeId,
    parameter: NodeId,
}

// Renames all instances of `arguments` with a new unique name.
// port: InstrumentAsyncContext.ArgumentsRenamer
struct ArgumentsRenamer {
    arguments_name: Option<String>,
}

impl Callback for ArgumentsRenamer {
    // port: InstrumentAsyncContext.ArgumentsRenamer#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !n.is_function(t) || n.is_arrow_function(t)
    }

    // port: InstrumentAsyncContext.ArgumentsRenamer#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(t) && n.get_string_ref(t) == "arguments" {
            if self.arguments_name.is_none() {
                let input = check_not_null!(t.get_input().cloned());
                let unique_id = t
                    .get_compiler()
                    .get_unique_id_supplier()
                    .get_unique_id(&input);
                self.arguments_name = Some(format!("$jscomp$arguments${unique_id}"));
            }
            let arguments_name = check_not_null!(self.arguments_name.clone());
            n.set_string(t, arguments_name);
            if t.get_compiler()
                .get_options()
                .preserves_detailed_source_info()
            {
                n.set_original_name(t, Some("arguments".into()));
            }
            n.make_non_indexable(t);
            n.put_boolean_prop(t, Prop::IS_CONSTANT_NAME, true);
        }
    }
}

// port: InstrumentAsyncContext.FunctionContext
#[derive(Clone)]
struct FunctionContext {
    #[allow(dead_code)] // Java record component, never read.
    unique_suffix: String,
    suspend_name: NodeId,
    resume_name: NodeId,
    factory_name: NodeId,
}

impl FunctionContext {
    /// Creates an empty suspend call: `ᵃᶜsuspend()`.
    // port: InstrumentAsyncContext.FunctionContext#suspend()
    fn suspend(&self, compiler: &mut AbstractCompiler, ast_factory: &AstFactory) -> NodeId {
        let callee = self.suspend_name.clone_node(compiler);
        ast_factory.create_call(
            compiler,
            callee,
            AstFactory::type_node(self.suspend_name),
            &[],
        )
    }

    /// Wraps the given node in a suspend call: `ᵃᶜsuspend(NODE)`.
    // port: InstrumentAsyncContext.FunctionContext#suspend(Node)
    fn suspend_inner(
        &self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
        inner: NodeId,
    ) -> NodeId {
        let callee = self.suspend_name.clone_node(compiler);
        ast_factory.create_call(compiler, callee, AstFactory::type_node(inner), &[inner])
    }

    /// Creates an empty resume call: `ᵃᶜresume()`.
    // port: InstrumentAsyncContext.FunctionContext#resume()
    fn resume(&self, compiler: &mut AbstractCompiler, ast_factory: &AstFactory) -> NodeId {
        let callee = self.resume_name.clone_node(compiler);
        ast_factory.create_call(
            compiler,
            callee,
            AstFactory::type_node(self.resume_name),
            &[],
        )
    }

    /// Wraps the given node in a resume call: `ᵃᶜresume(NODE)`.
    // port: InstrumentAsyncContext.FunctionContext#resume(Node)
    fn resume_inner(
        &self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
        inner: NodeId,
    ) -> NodeId {
        let callee = self.resume_name.clone_node(compiler);
        ast_factory.create_call(compiler, callee, AstFactory::type_node(inner), &[inner])
    }
}
