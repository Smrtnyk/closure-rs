/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/FunctionInjector.java.

//! Port of `com.google.javascript.jscomp.FunctionInjector`.
//!
//! A set of utility functions that replaces CALL with a specified FUNCTION body, replacing and
//! aliasing function parameters as necessary.
use crate::{
    abstract_compiler::AbstractCompiler,
    expression_decomposer::DecompositionType,
    expression_decomposer::ExpressionDecomposer,
    function_argument_injector::FunctionArgumentInjector,
    function_to_block_mutator::FunctionToBlockMutator,
    inline_cost_estimator::{self, InlineCostEstimator},
    js_chunk::JSChunk,
    node_util::{MatchDeclaration, MatchShallowStatement, NodeUtil},
    scope::Scope,
};
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

/// Java's `NO_FUNCTIONS` / `MULTIPLE_FUNCTIONS` sentinel FUNCTION nodes (static, shared by every
/// compilation) are the two sentinel variants here; `One` holds the single inner function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InnerFunction {
    /// Sentinel value indicating that the key contains no functions.
    NoFunctions,
    /// Sentinel value indicating that the key contains multiple distinct functions.
    MultipleFunctions,
    One(NodeId),
}

pub struct FunctionInjector {
    allow_decomposition: bool,
    known_constant_functions: IndexSet<JsString>,
    assume_strict_this: bool,
    assume_minimum_capture: bool,
    safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    throwaway_name_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    function_argument_injector: Rc<FunctionArgumentInjector>,

    /// Cache of function node to whether it deeply contains an `eval` call.
    references_eval_cache: IndexMap<NodeId, bool>,

    /// Cache of function node to any inner function.
    inner_function_cache: IndexMap<NodeId, InnerFunction>,
}

impl FunctionInjector {
    // port: FunctionInjector#FunctionInjector
    fn new(builder: Builder) -> Self {
        let next_id = AtomicI32::new(0);
        Self {
            safe_name_id_supplier: builder.safe_name_id_supplier.unwrap(),
            assume_strict_this: builder.assume_strict_this,
            assume_minimum_capture: builder.assume_minimum_capture,
            allow_decomposition: builder.allow_decomposition,
            function_argument_injector: builder.function_argument_injector.unwrap(),
            known_constant_functions: IndexSet::new(),
            // port: FunctionInjector.<anonymous>#get (throwawayNameSupplier)
            throwaway_name_supplier: Arc::new(move || {
                next_id.fetch_add(1, Ordering::SeqCst).to_string()
            }),
            references_eval_cache: IndexMap::new(),
            inner_function_cache: IndexMap::new(),
        }
    }
}

pub struct Builder {
    safe_name_id_supplier: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    assume_strict_this: bool,
    assume_minimum_capture: bool,
    allow_decomposition: bool,
    function_argument_injector: Option<Rc<FunctionArgumentInjector>>,
}

impl Builder {
    // port: FunctionInjector.Builder#Builder
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            safe_name_id_supplier: None,
            assume_strict_this: true,
            assume_minimum_capture: true,
            allow_decomposition: true,
            function_argument_injector: None,
        }
    }

    /// Provide the name supplier to use for injection.
    ///
    /// If this method is not called, `compiler.getUniqueNameIdSupplier()` will be used.
    // port: FunctionInjector.Builder#safeNameIdSupplier
    pub fn safe_name_id_supplier(
        mut self,
        safe_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    ) -> Self {
        self.safe_name_id_supplier = Some(safe_name_id_supplier);
        self
    }

    /// Allow decomposition of expressions.
    ///
    /// Default is `true`.
    // port: FunctionInjector.Builder#allowDecomposition
    pub fn allow_decomposition(mut self, allow_decomposition: bool) -> Self {
        self.allow_decomposition = allow_decomposition;
        self
    }

    // port: FunctionInjector.Builder#assumeStrictThis
    pub fn assume_strict_this(mut self, assume_strict_this: bool) -> Self {
        self.assume_strict_this = assume_strict_this;
        self
    }

    // port: FunctionInjector.Builder#assumeMinimumCapture
    pub fn assume_minimum_capture(mut self, assume_minimum_capture: bool) -> Self {
        self.assume_minimum_capture = assume_minimum_capture;
        self
    }

    /// Specify the `FunctionArgumentInjector` to be used.
    ///
    /// Default is for the builder to create this. This method exists for testing purposes.
    // port: FunctionInjector.Builder#functionArgumentInjector
    pub fn function_argument_injector(
        mut self,
        function_argument_injector: Rc<FunctionArgumentInjector>,
    ) -> Self {
        self.function_argument_injector = Some(function_argument_injector);
        self
    }

    // port: FunctionInjector.Builder#build
    pub fn build(mut self, compiler: &AbstractCompiler) -> FunctionInjector {
        if self.safe_name_id_supplier.is_none() {
            self.safe_name_id_supplier = Some(compiler.get_unique_name_id_supplier());
        }
        if self.function_argument_injector.is_none() {
            self.function_argument_injector = Some(Rc::new(FunctionArgumentInjector::new(
                compiler.get_ast_analyzer(),
            )));
        }
        FunctionInjector::new(self)
    }
}

/// The type of inlining to perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InliningMode {
    /// Directly replace the call expression. Only functions of meeting strict preconditions can
    /// be inlined.
    DIRECT,

    /// Replaces the call expression with a block of statements. Conditions on the function are
    /// looser in mode, but stricter on the call site.
    BLOCK,
}

/// Holds a reference to the call node of a function call
#[derive(Clone)]
pub struct Reference {
    pub call_node: NodeId,
    pub scope: Scope,
    pub chunk: Option<JSChunk>,
    pub mode: InliningMode,
}

impl Reference {
    // port: FunctionInjector.Reference#Reference
    pub fn new(
        call_node: NodeId,
        scope: Scope,
        chunk: Option<JSChunk>,
        mode: InliningMode,
    ) -> Self {
        Self {
            call_node,
            scope,
            chunk,
            mode,
        }
    }

    // port: FunctionInjector.Reference#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        format!("Reference @ {}", self.call_node.to_string(ast))
    }
}

/// In order to estimate the cost of lining, we make the assumption that Identifiers are reduced
/// 2 characters. For the call arguments, the important thing is that the cost is assumed to be
/// the same in the call and the function, so the actual length doesn't matter in most cases.
const NAME_COST_ESTIMATE: i32 = inline_cost_estimator::ESTIMATED_IDENTIFIER_COST;

/// The cost of a argument separator (a comma).
const COMMA_COST: i32 = 1;

/// The cost of the parentheses needed to make a call.
const PAREN_COST: i32 = 2;

/// Supported call site types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallSiteType {
    /// Used for a call site for which there does not exist a method to inline it.
    UNSUPPORTED,

    /// A call as a statement. For example: "foo();". EXPR_RESULT CALL
    SIMPLE_CALL,

    /// An assignment, where the result of the call is assigned to a simple name. For example:
    /// "a = foo();". EXPR_RESULT NAME A CALL FOO
    SIMPLE_ASSIGNMENT,

    /// An var declaration and initialization, where the result of the call is assigned to the
    /// declared name name. For example: "var a = foo();". VAR NAME A CALL FOO
    VAR_DECL_SIMPLE_ASSIGNMENT,

    /// An arbitrary expression, the root of which is a EXPR_RESULT, IF, RETURN, SWITCH or VAR.
    /// The call must be the first side-effect in the expression.
    ///
    /// Examples include: "if (foo()) {..." "return foo();" "var a = 1 + foo();" "a = 1 + foo()"
    /// "foo() ? 1:0" "foo() && x"
    EXPRESSION,

    /// An arbitrary expression, the root of which is a EXPR_RESULT, IF, RETURN, SWITCH or VAR.
    /// Where the call is not the first side-effect in the expression.
    DECOMPOSABLE_EXPRESSION,
}

impl CallSiteType {
    // port: FunctionInjector.CallSiteType#prepare
    fn prepare(
        self,
        injector: &FunctionInjector,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
    ) {
        match self {
            // port: FunctionInjector.CallSiteType.UNSUPPORTED#prepare
            CallSiteType::UNSUPPORTED => {
                panic!("unexpected: {}", r#ref.to_string(compiler));
            }
            // port: FunctionInjector.CallSiteType.SIMPLE_CALL#prepare
            CallSiteType::SIMPLE_CALL => {
                // Nothing to do.
            }
            // port: FunctionInjector.CallSiteType.SIMPLE_ASSIGNMENT#prepare
            CallSiteType::SIMPLE_ASSIGNMENT => {
                // Nothing to do.
            }
            // port: FunctionInjector.CallSiteType.VAR_DECL_SIMPLE_ASSIGNMENT#prepare
            CallSiteType::VAR_DECL_SIMPLE_ASSIGNMENT => {
                // Nothing to do.
            }
            // port: FunctionInjector.CallSiteType.EXPRESSION#prepare
            CallSiteType::EXPRESSION => {
                let call_node = r#ref.call_node;
                injector
                    .get_decomposer(compiler, r#ref.scope)
                    .move_expression(compiler, call_node);

                // Reclassify after move
                let call_site_type = injector.classify_call_site(compiler, r#ref);
                check_state!(self != call_site_type);
                call_site_type.prepare(injector, compiler, r#ref);
            }
            // port: FunctionInjector.CallSiteType.DECOMPOSABLE_EXPRESSION#prepare
            CallSiteType::DECOMPOSABLE_EXPRESSION => {
                let call_node = r#ref.call_node;
                injector
                    .get_decomposer(compiler, r#ref.scope)
                    .maybe_expose_expression(compiler, call_node);

                // Reclassify after decomposition
                let call_site_type = injector.classify_call_site(compiler, r#ref);
                check_state!(self != call_site_type);
                call_site_type.prepare(injector, compiler, r#ref);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanInlineResult {
    YES,
    AFTER_PREPARATION,
    NO,
}

impl FunctionInjector {
    /// Returns whether the function node meets the minimum requirements for inlining.
    ///
    /// `fn_name` is the name of this function. This either the name of the variable to which the
    /// function is assigned or the name from the FUNCTION node. `fn_node` is the FUNCTION node of
    /// the function to inspect.
    // port: FunctionInjector#doesFunctionMeetMinimumRequirements
    pub fn does_function_meet_minimum_requirements(
        &self,
        ast: &Ast,
        fn_name: &JsString,
        fn_node: NodeId,
    ) -> bool {
        let block = NodeUtil::get_function_body(ast, fn_node);

        // Basic restrictions on functions that can be inlined:
        // 1) It contains a reference to itself.
        // 2) It uses its parameters indirectly using "arguments" (it isn't
        //    handled yet.
        // 3) It references "eval". Inline a function containing eval can have
        //    large performance implications.

        let fn_recursion_name = fn_node.get_first_child(ast).unwrap().get_string(ast);

        // If the function references "arguments" directly in the function or in an arrow
        // function
        let references_arguments = NodeUtil::is_name_referenced_with_predicate(
            ast,
            block,
            "arguments",
            &NodeUtil::MATCH_ANYTHING_BUT_NON_ARROW_FUNCTION,
        );

        let blocks_injection = |ast: &Ast, n: NodeId| -> bool {
            if n.is_name(ast) {
                // References "eval" or one of its names anywhere.
                let name = n.get_string(ast);
                name == "eval"
                    || (!fn_name.is_empty() && name == *fn_name)
                    || (!fn_recursion_name.is_empty() && name == fn_recursion_name)
            } else if n.is_super(ast) || n.get_token(ast) == Token::NEW_TARGET {
                // Don't inline if this function or its inner functions contains super or
                // new.target
                true
            } else {
                false
            }
        };

        !references_arguments && !NodeUtil::has(ast, block, &blocks_injection, &|_, _| true)
    }

    /// Returns whether the inlining can occur.
    ///
    /// `need_aliases` is a set of function parameter names that can not be used without
    /// aliasing (returned by getUnsafeParameterNames()), `references_this` whether fnNode
    /// contains references to its this object and `contains_functions` whether fnNode contains
    /// inner functions.
    // port: FunctionInjector#canInlineReferenceToFunction
    pub fn can_inline_reference_to_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_node: NodeId,
        need_aliases: &IndexSet<JsString>,
        references_this: bool,
        contains_functions: bool,
    ) -> CanInlineResult {
        // TODO(johnlenz): This function takes too many parameter, without
        // context.  Modify the API to take a structure describing the function.

        // Allow direct function calls or "fn.call" style calls.
        let call_node = r#ref.call_node;
        if !self.is_supported_call_type(compiler, call_node, fn_node) {
            return CanInlineResult::NO;
        }

        if Self::has_spread_call_argument(compiler, call_node) {
            return CanInlineResult::NO;
        }

        // Limit where functions that contain functions can be inline.  Introducing
        // an inner function into another function can capture a variable and cause
        // a memory leak.  This isn't a problem in the global scope as those values
        // last until explicitly cleared.
        if contains_functions {
            if !self.assume_minimum_capture && !r#ref.scope.is_global(compiler) {
                // TODO(johnlenz): Allow inlining into any scope without local names or inner
                // functions.
                return CanInlineResult::NO;
            } else if NodeUtil::is_within_loop(compiler, call_node) {
                // An inner closure maybe relying on a local value holding a value for a
                // single iteration through a loop.
                return CanInlineResult::NO;
            }
        }

        // TODO(johnlenz): Add support for 'apply'
        if references_this && !NodeUtil::is_function_object_call(compiler, call_node) {
            // TODO(johnlenz): Allow 'this' references to be replaced with a
            // global 'this' object.
            return CanInlineResult::NO;
        }

        if r#ref.mode == InliningMode::DIRECT {
            self.can_inline_reference_directly(compiler, r#ref, fn_node, need_aliases)
        } else {
            self.can_inline_reference_as_statement_block(compiler, r#ref, fn_node, need_aliases)
        }
    }

    /// Only ".call" calls and direct calls to functions are supported.
    ///
    /// Returns whether the call is of a type that is supported.
    // port: FunctionInjector#isSupportedCallType
    fn is_supported_call_type(&self, ast: &Ast, call_node: NodeId, fn_node: NodeId) -> bool {
        if !call_node.get_first_child(ast).unwrap().is_name(ast) {
            if NodeUtil::is_function_object_call(ast, call_node) {
                if !self.assume_strict_this && !fn_node.is_arrow_function(ast) {
                    let this_value = call_node.get_second_child(ast);
                    if this_value.is_none_or(|this_value| !this_value.is_this(ast)) {
                        return false;
                    }
                }
            } else if NodeUtil::is_function_object_apply(ast, call_node) {
                return false;
            }
        }

        true
    }

    // port: FunctionInjector#hasSpreadCallArgument
    fn has_spread_call_argument(ast: &Ast, call_node: NodeId) -> bool {
        check_argument!(
            NodeUtil::is_normal_or_opt_chain_call(ast, call_node),
            "%s",
            call_node.to_string(ast)
        );
        let mut arg = call_node.get_second_child(ast);
        while let Some(current) = arg {
            if current.is_spread(ast) {
                return true;
            }
            arg = current.get_next(ast);
        }
        false
    }

    /// Inline a function into the call site.
    // port: FunctionInjector#inline
    pub fn inline(
        &self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_name: &JsString,
        fn_node: NodeId,
    ) -> NodeId {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        self.internal_inline(compiler, r#ref, fn_name, fn_node)
    }

    /// Inline a function into the call site. Note that this unsafe version doesn't verify if
    /// the AST is normalized. You should use `inline` instead, unless you are 100% certain that
    /// the bit of code you're inlining is safe without being normalized first.
    // port: FunctionInjector#unsafeInline
    pub fn unsafe_inline(
        &self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_name: &JsString,
        fn_node: NodeId,
    ) -> NodeId {
        self.internal_inline(compiler, r#ref, fn_name, fn_node)
    }

    // port: FunctionInjector#internalInline
    fn internal_inline(
        &self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_name: &JsString,
        fn_node: NodeId,
    ) -> NodeId {
        let result = if r#ref.mode == InliningMode::DIRECT {
            self.inline_return_value(compiler, r#ref, fn_node)
        } else {
            self.inline_function(compiler, r#ref, fn_node, fn_name)
        };
        compiler.report_change_to_enclosing_scope(result);
        result
    }

    /// Inline a function that fulfills the requirements of canInlineReferenceDirectly into the
    /// call site, replacing only the CALL node.
    // port: FunctionInjector#inlineReturnValue
    fn inline_return_value(
        &self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_node: NodeId,
    ) -> NodeId {
        let call_node = r#ref.call_node;
        let block = fn_node.get_last_child(compiler).unwrap();

        // NOTE: As the normalize pass guarantees globals aren't being
        // shadowed and an expression can't introduce new names, there is
        // no need to check for conflicts.

        // Create an paramName -> argument value map, checking for side effects.
        let param_to_arg_map = self
            .function_argument_injector
            .get_function_call_parameter_map(
                compiler,
                fn_node,
                call_node,
                &*self.safe_name_id_supplier,
            );
        let mut param_replacements: IndexMap<JsString, NodeId> = IndexMap::new();
        for (key, value) in &param_to_arg_map {
            param_replacements.insert(key.clone(), value.arg());
        }

        let new_expression;
        if !block.has_children(compiler) {
            let src_location = block;
            new_expression = NodeUtil::new_undefined_node(compiler, Some(src_location));
        } else {
            let return_node = block.get_first_child(compiler).unwrap();
            check_argument!(
                return_node.is_return(compiler),
                "%s",
                return_node.to_string(compiler)
            );

            // Clone the return node first.
            let safe_return_node = return_node.clone_tree(compiler);
            // Java passes a null compiler here; inject only reads it when it replaces `this` with
            // a non-`this` value, which canInlineReferenceDirectly never allows for this mode.
            let inline_result = self.function_argument_injector.inject(
                compiler,
                safe_return_node,
                None,
                &mut param_replacements,
            );
            check_argument!(safe_return_node == inline_result);
            new_expression = safe_return_node.remove_first_child(compiler).unwrap();
            NodeUtil::mark_new_scopes_changed(compiler, new_expression);
        }

        // If the call site had a cast ensure it's persisted to the new expression that replaces
        // it.
        let type_before_cast = call_node.get_jstype_before_cast(compiler);
        if type_before_cast.is_some() {
            new_expression.set_jstype_before_cast(compiler, type_before_cast);
            let call_type = call_node.get_jstype(compiler);
            new_expression.set_jstype(compiler, call_type);
        }
        // If the new expression has no color or the UNKNOWN color, attach the color the call
        // node. It may be more accurate if the call node was in a cast (we don't track
        // information about casts, though)
        if call_node.get_color(compiler).is_some() && call_node.is_color_from_type_cast(compiler) {
            let color = call_node.get_color(compiler);
            new_expression.set_color(compiler, color);
            new_expression.set_color_from_type_cast(compiler);
        }
        call_node.replace_with(compiler, new_expression);
        NodeUtil::mark_functions_deleted(compiler, call_node);
        new_expression
    }

    /// Determine which, if any, of the supported types the call site is.
    ///
    /// Constant vars are treated differently so that we don't break their const-ness when we
    /// decompose the expression. Once the CONSTANT_VAR annotation is used everywhere instead of
    /// coding conventions, we should just teach this pass how to remove the annotation.
    // port: FunctionInjector#classifyCallSite
    fn classify_call_site(
        &self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
    ) -> CallSiteType {
        let call_node = r#ref.call_node;
        let parent = call_node.get_parent(compiler).unwrap();
        let grand_parent = parent.get_parent(compiler);

        // Verify the call site:
        if NodeUtil::is_expr_call(compiler, parent) {
            // This is a simple call. Example: "foo();".
            return CallSiteType::SIMPLE_CALL;
        } else if NodeUtil::is_expr_assign(compiler, grand_parent.unwrap())
            && !NodeUtil::is_name_decl_or_simple_assign_lhs(compiler, call_node, parent)
            && parent.get_first_child(compiler).unwrap().is_name(compiler)
            // TODO(nicksantos): Remove this once everyone is using
            // the CONSTANT_VAR annotation. We know how to remove that.
            && !NodeUtil::is_constant_name(compiler, parent.get_first_child(compiler).unwrap())
        {
            // This is a simple assignment.  Example: "x = foo();"
            return CallSiteType::SIMPLE_ASSIGNMENT;
        } else if parent.is_name(compiler)
            // TODO(nicksantos): Remove this once everyone is using the CONSTANT_VAR annotation.
            && !NodeUtil::is_constant_name(compiler, parent)
            // Note: not let or const. See InlineFunctionsTest.testInlineFunctions35
            && grand_parent.unwrap().is_var(compiler)
            && grand_parent.unwrap().has_one_child(compiler)
        {
            // This is a var declaration.  Example: "var x = foo();"
            // TODO(johnlenz): Should we be checking for constants on the
            // left-hand-side of the assignments and handling them as EXPRESSION?
            return CallSiteType::VAR_DECL_SIMPLE_ASSIGNMENT;
        } else {
            let decomposer = self.get_decomposer(compiler, r#ref.scope);
            match decomposer.can_expose_expression(compiler, call_node) {
                DecompositionType::MOVABLE => {
                    return CallSiteType::EXPRESSION;
                }
                DecompositionType::DECOMPOSABLE => {
                    return CallSiteType::DECOMPOSABLE_EXPRESSION;
                }
                DecompositionType::UNDECOMPOSABLE => {}
            }
        }

        CallSiteType::UNSUPPORTED
    }

    // port: FunctionInjector#getDecomposer
    fn get_decomposer(
        &self,
        compiler: &mut AbstractCompiler,
        scope: Scope,
    ) -> ExpressionDecomposer {
        compiler.create_expression_decomposer(
            self.safe_name_id_supplier.clone(),
            self.known_constant_functions.clone(),
            scope,
        )
    }

    /// If required, rewrite the statement containing the call expression.
    // port: FunctionInjector#maybePrepareCall
    pub fn maybe_prepare_call(&self, compiler: &mut AbstractCompiler, r#ref: &Reference) {
        let call_site_type = self.classify_call_site(compiler, r#ref);
        call_site_type.prepare(self, compiler, r#ref);
    }

    /// Inline a function which fulfills the requirements of canInlineReferenceAsStatementBlock
    /// into the call site, replacing the parent expression.
    // port: FunctionInjector#inlineFunction
    fn inline_function(
        &self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_node: NodeId,
        fn_name: &JsString,
    ) -> NodeId {
        let call_node = r#ref.call_node;
        let parent = call_node.get_parent(compiler).unwrap();
        let grand_parent = parent.get_parent(compiler).unwrap();

        // TODO(johnlenz): Consider storing the callSite classification in the
        // reference object and passing it in here.
        let call_site_type = self.classify_call_site(compiler, r#ref);
        check_argument!(call_site_type != CallSiteType::UNSUPPORTED);

        // Store the name for the result. This will be used to
        // replace "return expr" statements with "resultName = expr"
        // to replace
        let result_name: Option<JsString>;
        let mut needs_default_return_result = true;
        match call_site_type {
            CallSiteType::SIMPLE_ASSIGNMENT => {
                let name = parent
                    .get_first_child(compiler)
                    .unwrap()
                    .get_string(compiler);
                Self::remove_constant_var_annotation(compiler, r#ref.scope, &name);
                result_name = Some(name);
            }
            CallSiteType::VAR_DECL_SIMPLE_ASSIGNMENT => {
                let name = parent.get_string(compiler);
                Self::remove_constant_var_annotation(compiler, r#ref.scope, &name);
                result_name = Some(name);
            }
            CallSiteType::SIMPLE_CALL => {
                result_name = None; // "foo()" doesn't need a result.
                needs_default_return_result = false;
            }
            CallSiteType::EXPRESSION => {
                panic!("Movable expressions must be moved before inlining.")
            }
            CallSiteType::DECOMPOSABLE_EXPRESSION => {
                panic!("Decomposable expressions must be decomposed before inlining.")
            }
            _ => panic!("Unexpected call site type."),
        }

        let mutator = FunctionToBlockMutator::new(compiler, self.safe_name_id_supplier.clone());

        let is_call_in_loop = NodeUtil::is_within_loop(compiler, call_node);
        let new_block = mutator.mutate(
            compiler,
            fn_name,
            fn_node,
            call_node,
            result_name.as_ref(),
            needs_default_return_result,
            is_call_in_loop,
        );
        NodeUtil::mark_new_scopes_changed(compiler, new_block);

        // TODO(nicksantos): Create a common mutation function that
        // can replace either a VAR name assignment, assignment expression or
        // a EXPR_RESULT.
        match call_site_type {
            CallSiteType::VAR_DECL_SIMPLE_ASSIGNMENT => {
                // Remove the call from the name node.
                let first_child = parent.remove_first_child(compiler).unwrap();
                NodeUtil::mark_functions_deleted(compiler, first_child);
                check_state!(!parent.has_children(compiler));
                // Add the call, after the VAR.
                new_block.insert_after(compiler, grand_parent);
            }
            CallSiteType::SIMPLE_ASSIGNMENT => {
                // The assignment is now part of the inline function so
                // replace it completely.
                check_state!(grand_parent.is_expr_result(compiler));
                grand_parent.replace_with(compiler, new_block);
                NodeUtil::mark_functions_deleted(compiler, grand_parent);
            }
            CallSiteType::SIMPLE_CALL => {
                // If nothing is looking at the result just replace the call.
                check_state!(parent.is_expr_result(compiler));
                parent.replace_with(compiler, new_block);
                NodeUtil::mark_functions_deleted(compiler, parent);
            }
            _ => panic!("Unexpected call site type."),
        }

        new_block
    }

    // port: FunctionInjector#removeConstantVarAnnotation
    fn remove_constant_var_annotation(
        compiler: &mut AbstractCompiler,
        scope: Scope,
        name: &JsString,
    ) {
        let var = scope.get_var(compiler, name);
        let name_node = match var {
            None => None,
            Some(var) => var.get_name_node(compiler),
        };
        let Some(name_node) = name_node else {
            return;
        };

        if name_node.is_declared_constant_var(compiler) {
            name_node.set_declared_constant_var(compiler, false);
        }
    }

    /// Checks if the given function matches the criteria for an inlinable function, and if so,
    /// adds it to our set of inlinable functions.
    // port: FunctionInjector#isDirectCallNodeReplacementPossible
    pub fn is_direct_call_node_replacement_possible(ast: &Ast, fn_node: NodeId) -> bool {
        // Only inline single-statement functions
        let block = NodeUtil::get_function_body(ast, fn_node);

        // Check if this function is suitable for direct replacement of a CALL node:
        // a function that consists of single return that returns an expression.
        if !block.has_children(ast) {
            // special case empty functions.
            return true;
        } else if block.has_one_child(ast) {
            // Only inline functions that return something.
            if block.get_first_child(ast).unwrap().is_return(ast)
                && block.get_first_first_child(ast).is_some()
            {
                return true;
            }
        }

        false
    }

    /// Determines whether a function can be inlined at a particular call site. There are several
    /// criteria that the function and reference must hold in order for the functions to be
    /// inlined: - It must be a simple call, or assignment, or var initialization.
    ///
    /// ```text
    ///    f();
    ///    a = foo();
    ///    var a = foo();
    /// ```
    // port: FunctionInjector#canInlineReferenceAsStatementBlock
    fn can_inline_reference_as_statement_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_node: NodeId,
        names_to_alias: &IndexSet<JsString>,
    ) -> CanInlineResult {
        let call_site_type = self.classify_call_site(compiler, r#ref);
        if call_site_type == CallSiteType::UNSUPPORTED {
            return CanInlineResult::NO;
        }

        if !self.allow_decomposition
            && (call_site_type == CallSiteType::DECOMPOSABLE_EXPRESSION
                || call_site_type == CallSiteType::EXPRESSION)
        {
            return CanInlineResult::NO;
        }

        if !self.call_meets_block_inlining_requirements(compiler, r#ref, fn_node, names_to_alias) {
            return CanInlineResult::NO;
        }

        if call_site_type == CallSiteType::DECOMPOSABLE_EXPRESSION
            || call_site_type == CallSiteType::EXPRESSION
        {
            CanInlineResult::AFTER_PREPARATION
        } else {
            CanInlineResult::YES
        }
    }

    /// Returns whether or not `fn` includes a call to `eval` in its scope.
    ///
    /// Results are cached to make subsequent calls faster.
    // port: FunctionInjector#referencesEval
    fn references_eval(&mut self, ast: &Ast, r#fn: NodeId) -> bool {
        check_state!(r#fn.is_function(ast));

        let cached = self.references_eval_cache.get(&r#fn).copied();
        if let Some(cached) = cached {
            return cached;
        }

        let result = NodeUtil::has(
            ast,
            r#fn,
            &|ast, n| n.is_name(ast) && n.get_string_ref(ast) == "eval", // Match predicate
            &|ast, n| !n.is_function(ast) || n == r#fn,                  // Explore node predicate
        );
        self.references_eval_cache.insert(r#fn, result);
        result
    }

    /// Returns any inner function of `container_fn`.
    ///
    /// If there are no inner functions, or multilple inner functions, the sentinel values
    /// NO_FUNCTIONS, MULTIPLE_FUNCTIONS are returned respectively.
    // port: FunctionInjector#innerFunctionOf
    fn inner_function_of(&mut self, ast: &mut Ast, container_fn: NodeId) -> InnerFunction {
        check_state!(container_fn.is_function(ast));

        let cached = self.inner_function_cache.get(&container_fn).copied();
        if let Some(cached) = cached {
            return cached;
        }

        let mut inner_fns: Vec<NodeId> = Vec::new();
        NodeUtil::visit_pre_order(ast, container_fn, &mut |ast: &mut Ast, n: NodeId| {
            if n == container_fn {
                return;
            }

            if n.is_function(ast) {
                inner_fns.push(n);
            }
        });

        let cached = match inner_fns.len() {
            0 => InnerFunction::NoFunctions,
            1 => InnerFunction::One(inner_fns[0]),
            _ => InnerFunction::MultipleFunctions,
        };

        self.inner_function_cache.insert(container_fn, cached);
        cached
    }

    /// Determines whether a function can be inlined at a particular call site. - Don't inline if
    /// the calling function contains an inner function and inlining would introduce new globals.
    // port: FunctionInjector#callMeetsBlockInliningRequirements
    fn call_meets_block_inlining_requirements(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_ref: &Reference,
        callee_fn: NodeId,
        names_to_alias: &IndexSet<JsString>,
    ) -> bool {
        // Note: functions that contain function definitions are filtered out
        // in isCandidateFunction.

        // TODO(johnlenz): Determining if the called function contains VARs
        // or if the caller contains inner functions accounts for 20% of the
        // run-time cost of this pass.
        //
        // Note that as of 2019-10-11, "eval" and function checking are cached so this may be
        // less of an issue.

        // Don't inline functions with var declarations into a scope with inner
        // functions as the new vars would leak into the inner function and
        // cause memory leaks.
        let callee_contains_vars = NodeUtil::has(
            compiler,
            NodeUtil::get_function_body(compiler, callee_fn),
            &|ast, n| MatchDeclaration.apply(ast, n),
            &|ast, n| MatchShallowStatement.apply(ast, n),
        );

        let mut forbid_temps = false;
        let hoist_scope = call_ref.scope.get_closest_hoist_scope(compiler).unwrap();
        if !hoist_scope.is_global(compiler) {
            let caller_fn = hoist_scope
                .get_root_node(compiler)
                .get_parent(compiler)
                .unwrap();

            // Don't allow any new vars into a scope that contains eval or one
            // that contains functions (excluding the function being inlined).
            if self.references_eval(compiler, caller_fn) {
                forbid_temps = true;
            } else if !self.assume_minimum_capture {
                let inner_fn = self.inner_function_of(compiler, caller_fn);
                let callee_is_only_inner_fn = inner_fn == InnerFunction::NoFunctions
                    || inner_fn == InnerFunction::One(callee_fn);
                forbid_temps = !callee_is_only_inner_fn;
            }
        }

        if callee_contains_vars && forbid_temps {
            return false;
        }

        // If the caller contains functions or evals, verify we aren't adding any
        // additional VAR declarations because aliasing is needed.
        if forbid_temps {
            let args = self
                .function_argument_injector
                .get_function_call_parameter_map(
                    compiler,
                    callee_fn,
                    call_ref.call_node,
                    &*self.safe_name_id_supplier,
                );
            let has_args = !args.is_empty();
            if has_args {
                // Limit the inlining
                if !names_to_alias.is_empty() {
                    return false;
                }

                let all_temps = self
                    .function_argument_injector
                    .gather_call_arguments_needing_temps(
                        compiler,
                        callee_fn,
                        &args,
                        names_to_alias,
                        None,
                    );
                if !all_temps.is_empty() {
                    return false;
                }
            }
        }

        true
    }

    /// Determines whether a function can be inlined at a particular call site. There are several
    /// criteria that the function and reference must hold in order for the functions to be
    /// inlined: 1) If a call's arguments have side effects, the corresponding argument in the
    /// function must only be referenced once. For instance, this will not be inlined:
    ///
    /// ```text
    ///     function foo(a) { return a + a }
    ///     x = foo(i++);
    /// ```
    // port: FunctionInjector#canInlineReferenceDirectly
    fn can_inline_reference_directly(
        &self,
        compiler: &mut AbstractCompiler,
        r#ref: &Reference,
        fn_node: NodeId,
        names_to_alias: &IndexSet<JsString>,
    ) -> CanInlineResult {
        if !Self::is_direct_call_node_replacement_possible(compiler, fn_node) {
            return CanInlineResult::NO;
        }

        // CALL NODE: [ NAME, ARG1, ARG2, ... ]
        let call_node = r#ref.call_node;
        let mut c_arg = call_node.get_second_child(compiler);

        // Functions called via 'call' and 'apply' have a this-object as
        // the first parameter, but this is not part of the called function's
        // parameter list.
        if !call_node
            .get_first_child(compiler)
            .unwrap()
            .is_name(compiler)
        {
            if NodeUtil::is_function_object_call(compiler, call_node) {
                // TODO(johnlenz): Support replace this with a value.
                if !fn_node.is_arrow_function(compiler)
                    && c_arg.is_none_or(|c_arg| !c_arg.is_this(compiler))
                {
                    return CanInlineResult::NO;
                }
                if let Some(arg) = c_arg {
                    c_arg = arg.get_next(compiler);
                }
            } else {
                // ".apply" call should be filtered before this.
                check_state!(!NodeUtil::is_function_object_apply(compiler, call_node));
            }
        }
        let _ = c_arg;

        let args = self
            .function_argument_injector
            .get_function_call_parameter_map(
                compiler,
                fn_node,
                call_node,
                &*self.throwaway_name_supplier,
            );
        let has_args = !args.is_empty();
        if has_args {
            // Limit the inlining
            if !names_to_alias.is_empty() {
                return CanInlineResult::NO;
            }

            let all_temps = self
                .function_argument_injector
                .gather_call_arguments_needing_temps(
                    compiler,
                    fn_node,
                    &args,
                    names_to_alias,
                    None,
                );
            if !all_temps.is_empty() {
                return CanInlineResult::NO;
            }
        }

        CanInlineResult::YES
    }

    /// Determine if inlining the function is likely to reduce the code size.
    // port: FunctionInjector#inliningLowersCost
    #[allow(clippy::too_many_arguments)]
    pub fn inlining_lowers_cost(
        &self,
        compiler: &AbstractCompiler,
        fn_chunk: Option<&JSChunk>,
        fn_node: NodeId,
        refs: &[&Reference],
        names_to_alias: &IndexSet<JsString>,
        mut is_removable: bool,
        references_this: bool,
    ) -> bool {
        let reference_count = refs.len() as i32;
        if reference_count == 0 {
            return true;
        }

        let mut references_using_block_inlining = 0;

        let mut check_chunks = is_removable && fn_chunk.is_some();
        let chunk_graph = compiler.get_chunk_graph();

        for r#ref in refs {
            if r#ref.mode == InliningMode::BLOCK {
                references_using_block_inlining += 1;
            }

            // Check if any of the references cross the chunk boundaries.
            if check_chunks
                && let Some(ref_chunk) = &r#ref.chunk
                && Some(ref_chunk) != fn_chunk
                && !chunk_graph
                    .unwrap()
                    .depends_on(ref_chunk, fn_chunk.unwrap())
            {
                // Calculate the cost as if the function were non-removable,
                // if it still lowers the cost inline it.
                is_removable = false;
                check_chunks = false; // no need to check additional chunks.
            }
        }

        let references_using_direct_inlining = reference_count - references_using_block_inlining;

        // Don't bother calculating the cost of function for simple functions where
        // possible.
        // However, when inlining a complex function, even a single reference may be
        // larger than the original function if there are many returns (resulting
        // in additional assignments) or many parameters that need to be aliased
        // so use the cost estimating.
        if reference_count == 1 && is_removable && references_using_direct_inlining == 1 {
            return true;
        }

        let call_cost = Self::estimate_call_cost(compiler, fn_node, references_this);
        let overall_call_cost = call_cost * reference_count;

        let cost_delta_direct =
            Self::inline_cost_delta(compiler, fn_node, names_to_alias, InliningMode::DIRECT);
        let cost_delta_block =
            Self::inline_cost_delta(compiler, fn_node, names_to_alias, InliningMode::BLOCK);

        Self::does_lower_cost(
            compiler,
            fn_node,
            overall_call_cost,
            references_using_direct_inlining,
            cost_delta_direct,
            references_using_block_inlining,
            cost_delta_block,
            is_removable,
        )
    }

    /// Returns whether inlining will lower cost.
    // port: FunctionInjector#doesLowerCost
    #[allow(clippy::too_many_arguments)]
    fn does_lower_cost(
        ast: &Ast,
        fn_node: NodeId,
        call_cost: i32,
        direct_inlines: i32,
        cost_delta_direct: i32,
        block_inlines: i32,
        cost_delta_block: i32,
        removable: bool,
    ) -> bool {
        // Determine the threshold value for this inequality:
        //     inline_cost < call_cost
        // But solve it for the function declaration size so the size of it
        // is only calculated once and terminated early if possible.

        let fn_instance_count = direct_inlines + block_inlines - if removable { 1 } else { 0 };
        // Prevent division by zero.
        if fn_instance_count == 0 {
            // Special case single reference function that are being block inlined:
            // If the cost of the inline is greater than the function definition size,
            // don't inline.
            return block_inlines <= 0 || cost_delta_block <= 0;
        }

        let cost_delta =
            (direct_inlines * -cost_delta_direct) + (block_inlines * -cost_delta_block);
        let threshold = (call_cost + cost_delta) / fn_instance_count;

        InlineCostEstimator::get_cost_with_threshold(ast, fn_node, threshold + 1) <= threshold
    }

    /// Gets an estimate of the cost in characters of making the function call: the sum of the
    /// identifiers and the separators.
    // port: FunctionInjector#estimateCallCost
    fn estimate_call_cost(ast: &Ast, fn_node: NodeId, references_this: bool) -> i32 {
        let args_node = NodeUtil::get_function_parameters(ast, fn_node);
        let num_args = args_node.get_child_count(ast);

        let mut call_cost = NAME_COST_ESTIMATE + PAREN_COST;
        if num_args > 0 {
            call_cost += (num_args * NAME_COST_ESTIMATE) + ((num_args - 1) * COMMA_COST);
        }

        if references_this {
            // TODO(johnlenz): Update this if we start supporting inlining
            // other functions that reference this.
            // The only functions that reference this that are currently inlined
            // are those that are called via ".call" with an explicit "this".
            call_cost += 5 + 5; // ".call" + "this,"
        }

        call_cost
    }

    /// Returns the difference between the function definition cost and inline cost.
    // port: FunctionInjector#inlineCostDelta
    fn inline_cost_delta(
        ast: &Ast,
        fn_node: NodeId,
        names_to_alias: &IndexSet<JsString>,
        mode: InliningMode,
    ) -> i32 {
        // The part of the function that is never inlined:
        //    "function xx(xx,xx){}" (15 + (param count * 3) -1;
        let param_count = NodeUtil::get_function_parameters(ast, fn_node).get_child_count(ast);
        let comma_count = if param_count > 1 { param_count - 1 } else { 0 };
        let cost_delta_function_overhead =
            15 + comma_count + (param_count * inline_cost_estimator::ESTIMATED_IDENTIFIER_COST);

        let block = fn_node.get_last_child(ast).unwrap();
        if !block.has_children(ast) {
            // Assume the inline cost is zero for empty functions.
            return -cost_delta_function_overhead;
        }

        if mode == InliningMode::DIRECT {
            // The part of the function that is inlined using direct inlining:
            //    "return " (7)
            -(cost_delta_function_overhead + 7)
        } else {
            let alias_count = names_to_alias.len() as i32;

            // Originally, we estimated purely base on the function code size, relying
            // on later optimizations. But that did not produce good results, so here
            // we try to estimate the something closer to the actual inlined coded.

            // NOTE 1: Result overhead is only if there is an assignment, but
            // getting that information would require some refactoring.
            // NOTE 2: The aliasing overhead is currently an under-estimate,
            // as some parameters are aliased because of the parameters used.
            // Perhaps we should just assume all parameters will be aliased?
            let inline_block_overhead = 4; // "X:{}"
            let per_return_overhead = 2; // "return" --> "break X"
            let per_return_result_overhead = 3; // "XX="
            let per_alias_overhead = 3; // "XX="

            // TODO(johnlenz): Counting the number of returns is relatively expensive.
            //   This information should be determined during the traversal and cached.
            let return_count =
                NodeUtil::get_node_type_reference_count(ast, block, Token::RETURN, &|ast, n| {
                    MatchShallowStatement.apply(ast, n)
                });
            let result_count = if return_count > 0 {
                return_count - 1
            } else {
                0
            };
            let base_overhead = if return_count > 0 {
                inline_block_overhead
            } else {
                0
            };

            let overhead = base_overhead
                + return_count * per_return_overhead
                + result_count * per_return_result_overhead
                + alias_count * per_alias_overhead;

            overhead - cost_delta_function_overhead
        }
    }

    /// Store the names of known constants to be used when classifying call-sites in
    /// expressions.
    // port: FunctionInjector#setKnownConstantFunctions
    pub fn set_known_constant_functions(&mut self, known_constant_functions: IndexSet<JsString>) {
        // This is only expected to be set once. The same set should be used
        // when evaluating call-sites and inlining calls.
        check_state!(self.known_constant_functions.is_empty());
        self.known_constant_functions = known_constant_functions;
    }
}
