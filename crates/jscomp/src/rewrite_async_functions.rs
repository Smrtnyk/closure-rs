/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteAsyncFunctions.java.

//! Port of `RewriteAsyncFunctions.java`.
//!
//! Converts async functions to valid ES6 generator functions code.
//!
//! This pass must run before the passes that transpile let declarations, arrow functions, and
//! generators.
//!
//! An async function, foo(a, b), will be rewritten as:
//!
//! ```text
//! function foo(a, b) {
//!   let $jscomp$async$this = this;
//!   let $jscomp$async$arguments = arguments;
//!   let $jscomp$async$new$target = new.target;
//!   let $jscomp$async$super$get$x = () => super.x;
//!   return $jscomp.asyncExecutePromiseGeneratorFunction(
//!       function* () {
//!         // original body of foo() with:
//!         // - await (x) replaced with yield (x)
//!         // - arguments replaced with $jscomp$async$arguments
//!         // - this replaced with $jscomp$async$this
//!         // - new.target replaced with $jscomp$async$new$target
//!         // - super.x replaced with $jscomp$async$super$get$x()
//!         // - super.x(5) replaced with $jscomp$async$super$get$x().call($jscomp$async$this, 5)
//!       });
//! }
//! ```
//!
//! Rust-only layout: the `LexicalContext` objects (Java's abstract class with the inner classes
//! `RootContext`, `ParameterListContext` and `FunctionContext`) live in an arena owned by the pass;
//! `ContextId` is Java's object identity, and their methods are pass methods taking the receiver's
//! `ContextId` (they read the pass's `astFactory` and `compiler`, as Java's inner classes do).

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    compiler_pass::CompilerPass,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
    transpilation_util,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    jscomp_colors::{Color, standard_colors},
    node::{NodeId, Prop},
    token::Token,
};

// port: RewriteAsyncFunctions#ASYNC_ARGUMENTS
const ASYNC_ARGUMENTS: &str = "$jscomp$async$arguments$";
// port: RewriteAsyncFunctions#ASYNC_THIS
const ASYNC_THIS: &str = "$jscomp$async$this$";
// port: RewriteAsyncFunctions#ASYNC_NEW_TARGET
const ASYNC_NEW_TARGET: &str = "$jscomp$async$new$target$";
// port: RewriteAsyncFunctions#ASYNC_SUPER_PROP_GETTER_PREFIX
const ASYNC_SUPER_PROP_GETTER_PREFIX: &str = "$jscomp$async$super$get$";

// port: RewriteAsyncFunctions#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM.with(Feature::ASYNC_FUNCTIONS)
}

/// Information needed to replace a reference to `super.propertyName` with a call to a wrapper
/// function.
#[derive(Clone)]
struct SuperPropertyWrapperInfo {
    // The first `super.property` Node we come across during traversal.
    // Information from this node will be used when creating the wrapper function.
    first_instance_of_super_dot_property: NodeId,
    wrapper_function_name: String,
    // The type to use for the wrapper function.
    // Will be null if type checking has not run.
    wrapper_function_return_type: Type,
}

impl SuperPropertyWrapperInfo {
    // port: RewriteAsyncFunctions.SuperPropertyWrapperInfo#SuperPropertyWrapperInfo
    fn new(
        first_super_dot_property_node: NodeId,
        wrapper_function_name: String,
        wrapper_function_return_type: Type,
    ) -> Self {
        Self {
            first_instance_of_super_dot_property: first_super_dot_property_node,
            wrapper_function_name,
            wrapper_function_return_type,
        }
    }

    // port: RewriteAsyncFunctions.SuperPropertyWrapperInfo#getPropertyType
    fn get_property_type(&self, compiler: &AbstractCompiler) -> Option<Color> {
        self.first_instance_of_super_dot_property
            .get_color(compiler)
    }

    // port: RewriteAsyncFunctions.SuperPropertyWrapperInfo#createWrapperFunctionNameNode
    fn create_wrapper_function_name_node(
        &self,
        ast_factory: &AstFactory,
        compiler: &mut AbstractCompiler,
    ) -> NodeId {
        ast_factory.create_name(
            compiler,
            self.wrapper_function_name.as_str(),
            AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
        )
    }

    // port: RewriteAsyncFunctions.SuperPropertyWrapperInfo#createWrapperFunctionCallNode
    fn create_wrapper_function_call_node(
        &self,
        ast_factory: &AstFactory,
        compiler: &mut AbstractCompiler,
    ) -> NodeId {
        let name = self.create_wrapper_function_name_node(ast_factory, compiler);
        ast_factory.create_call(
            compiler,
            name,
            self.wrapper_function_return_type.clone(),
            &[],
        )
    }
}

/// Used to collect information about properties referenced via `super.propertyName` within an
/// async function.
///
/// We'll have to replace these references with calls to a wrapper function.
#[derive(Default)]
struct SuperPropertyWrappers {
    // Use LinkedHashMap in order to ensure the ordering is the same on every compile of the same
    // source code.
    // Note that the Colors will be null if type checking hasn't run.
    property_name_to_type_map: IndexMap<JsString, SuperPropertyWrapperInfo>,
}

impl SuperPropertyWrappers {
    // port: RewriteAsyncFunctions.SuperPropertyWrappers#getOrCreateSuperPropertyWrapperInfo
    fn get_or_create_super_property_wrapper_info(
        &mut self,
        compiler: &mut AbstractCompiler,
        super_dot_property_node: NodeId,
    ) -> &SuperPropertyWrapperInfo {
        check_argument!(
            super_dot_property_node.is_get_prop(compiler),
            &super_dot_property_node.to_string(compiler)
        );
        let super_node = super_dot_property_node.get_first_child(compiler).unwrap();
        check_argument!(
            super_node.is_super(compiler),
            &super_node.to_string(compiler)
        );

        let property_name = super_dot_property_node.get_string(compiler);
        let property_type = super_dot_property_node.get_color(compiler);
        if self.property_name_to_type_map.contains_key(&property_name) {
            let super_property_wrapper_info = &self.property_name_to_type_map[&property_name];
            // Every reference to `super.propertyName` within a single lexical context should
            // have the same type.  Make sure this is true.
            let existing_color = super_property_wrapper_info.get_property_type(compiler);
            check_state!(
                existing_color == property_type,
                "Previous reference type: %s differs from current reference type: %s",
                java_color_string(&existing_color),
                java_color_string(&property_type)
            );
        } else {
            let super_property_wrapper_info =
                Self::create_new_info(compiler, super_dot_property_node);
            self.property_name_to_type_map
                .insert(property_name.clone(), super_property_wrapper_info);
        }
        &self.property_name_to_type_map[&property_name]
    }

    // port: RewriteAsyncFunctions.SuperPropertyWrappers#createNewInfo
    fn create_new_info(
        compiler: &mut AbstractCompiler,
        first_super_dot_property_node: NodeId,
    ) -> SuperPropertyWrapperInfo {
        check_argument!(
            first_super_dot_property_node.is_get_prop(compiler),
            &first_super_dot_property_node.to_string(compiler)
        );
        let property_name = first_super_dot_property_node.get_string(compiler);
        let input_id = NodeUtil::get_input_id(compiler, first_super_dot_property_node)
            .expect("java.lang.NullPointerException");
        let input = compiler
            .get_input(&input_id)
            .cloned()
            .expect("java.lang.NullPointerException");
        let wrapper_function_name = format!(
            "{ASYNC_SUPER_PROP_GETTER_PREFIX}{}${}",
            compiler.get_unique_id_supplier().get_unique_id(&input),
            property_name.to_string_lossy()
        );
        SuperPropertyWrapperInfo::new(
            first_super_dot_property_node,
            wrapper_function_name,
            AstFactory::type_node(first_super_dot_property_node),
        )
    }

    // port: RewriteAsyncFunctions.SuperPropertyWrappers#asCollection
    fn as_collection(&self) -> impl Iterator<Item = &SuperPropertyWrapperInfo> {
        self.property_name_to_type_map.values()
    }
}

/// Java's `String.valueOf(Color)` for the precondition message.
fn java_color_string(color: &Option<Color>) -> String {
    match color {
        None => "null".to_string(),
        Some(color) => color.to_string(),
    }
}

/// A `LexicalContext` handle: Java's object identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ContextId(usize);

/// Determines both what to do when visiting a node and how to determine the context for its
/// descendents.
struct LexicalContext {
    context_root_node: NodeId,
    // precomputed unique id to append to names in this context. This is used to ensure that names
    // in different contexts don't collide (e.g. 2 functions don't get the same `let
    // $jscomp$async$this` name declared in their bodies)
    unique_id: String,
    kind: LexicalContextKind,
}

#[allow(clippy::enum_variant_names)] // Java class names
enum LexicalContextKind {
    /// Defines behavior for nodes in the root scope, outside of any functions.
    RootContext,
    /// Defines the behavior for function definition parameter lists and their contents.
    ParameterListContext { function_context: ContextId },
    /// Defines behavior for replacing references to `this`, `arguments`, and `super` within
    /// async functions and rewriting the async functions themselves.
    FunctionContext(FunctionContext),
}

struct FunctionContext {
    // If references to `this`, `arguments`, and `super` should be considered in the context of
    // an async function, this will point to that function's FunctionContext.
    // Otherwise, it will be `null`.
    // TODO(bradfordcsmith): It would cost less memory if we defined a separate object to hold
    // the data for async context accounting instead of having the booleans and super property
    // wrapper fields on every FunctionContext.
    async_this_and_arguments_context: Option<ContextId>,
    super_property_wrappers: SuperPropertyWrappers,

    must_add_async_this_variable: bool,
    // null if mustAddAsyncThisVariable is false
    type_of_this: Option<Type>,
    must_add_async_arguments_variable: bool,
    must_add_async_new_target_variable: bool,
    // null if mustAddAsyncNewTargetVariable is false
    type_of_new_target: Option<Type>,
}

impl FunctionContext {
    fn new(async_this_and_arguments_context: Option<ContextId>) -> Self {
        Self {
            async_this_and_arguments_context,
            super_property_wrappers: SuperPropertyWrappers::default(),
            must_add_async_this_variable: false,
            type_of_this: None,
            must_add_async_arguments_variable: false,
            must_add_async_new_target_variable: false,
            type_of_new_target: None,
        }
    }
}

pub struct RewriteAsyncFunctions {
    namespace: TranspilationNamespace,
    context_stack: Vec<ContextId>,
    ast_factory: AstFactory,
    /// Rust-only: the arena of every `LexicalContext` this pass created.
    contexts: Vec<LexicalContext>,
}

impl RewriteAsyncFunctions {
    // port: RewriteAsyncFunctions#RewriteAsyncFunctions
    fn new(ast_factory: AstFactory, namespace: TranspilationNamespace) -> Self {
        Self {
            ast_factory,
            namespace,
            context_stack: Vec::new(),
            contexts: Vec::new(),
        }
    }

    // port: RewriteAsyncFunctions#create
    pub fn create(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let namespace = TranspilationNamespace::get(compiler);
        Self::new(ast_factory, namespace)
    }

    fn add_context(&mut self, context: LexicalContext) -> ContextId {
        let id = ContextId(self.contexts.len());
        self.contexts.push(context);
        id
    }

    fn function_context(&self, context: ContextId) -> &FunctionContext {
        match &self.contexts[context.0].kind {
            LexicalContextKind::FunctionContext(f) => f,
            _ => panic!("java.lang.ClassCastException: not a FunctionContext"),
        }
    }

    fn function_context_mut(&mut self, context: ContextId) -> &mut FunctionContext {
        match &mut self.contexts[context.0].kind {
            LexicalContextKind::FunctionContext(f) => f,
            _ => panic!("java.lang.ClassCastException: not a FunctionContext"),
        }
    }

    // port: RewriteAsyncFunctions.LexicalContext#LexicalContext
    // port: RewriteAsyncFunctions.RootContext#RootContext
    fn new_root_context(&mut self, context_root_node: NodeId, unique_id: String) -> ContextId {
        self.add_context(LexicalContext {
            context_root_node,
            unique_id,
            kind: LexicalContextKind::RootContext,
        })
    }

    // port: RewriteAsyncFunctions.ParameterListContext#ParameterListContext
    fn new_parameter_list_context(
        &mut self,
        function_context: ContextId,
        context_root_node: NodeId,
        unique_id: String,
    ) -> ContextId {
        self.add_context(LexicalContext {
            context_root_node,
            unique_id,
            kind: LexicalContextKind::ParameterListContext { function_context },
        })
    }

    // port: RewriteAsyncFunctions.FunctionContext#FunctionContext(Node,String)
    fn new_function_context(
        &mut self,
        compiler: &AbstractCompiler,
        context_root_node: NodeId,
        unique_id: String,
    ) -> ContextId {
        let id = ContextId(self.contexts.len());
        let async_this_and_arguments_context = if context_root_node.is_async_function(compiler) {
            Some(id)
        } else {
            None
        };
        self.add_context(LexicalContext {
            context_root_node,
            unique_id,
            kind: LexicalContextKind::FunctionContext(FunctionContext::new(
                async_this_and_arguments_context,
            )),
        })
    }

    // port: RewriteAsyncFunctions.FunctionContext#FunctionContext(FunctionContext,Node,String)
    fn new_function_context_with_outer(
        &mut self,
        compiler: &AbstractCompiler,
        outer: ContextId,
        context_root_node: NodeId,
        unique_id: String,
    ) -> ContextId {
        check_state!(
            context_root_node.is_function(compiler),
            &context_root_node.to_string(compiler)
        );
        let this = ContextId(self.contexts.len());
        let outer_async = self
            .function_context(outer)
            .async_this_and_arguments_context;
        let async_this_and_arguments_context;
        if context_root_node.is_async_function(compiler) {
            if context_root_node.is_arrow_function(compiler) {
                // An async arrow function context points to outer.asyncThisAndArgumentsContext
                // if non-null, otherwise to itself.
                async_this_and_arguments_context = match outer_async {
                    None => Some(this),
                    Some(outer_async) => Some(outer_async),
                };
            } else {
                // An async non-arrow function context always points to itself
                async_this_and_arguments_context = Some(this);
            }
        } else if context_root_node.is_arrow_function(compiler) {
            // A non-async arrow function context always points to
            // outer.asyncThisAndArgumentsContext
            async_this_and_arguments_context = outer_async;
        } else {
            // A non-async, non-arrow function has no async context.
            async_this_and_arguments_context = None;
        }
        self.add_context(LexicalContext {
            context_root_node,
            unique_id,
            kind: LexicalContextKind::FunctionContext(FunctionContext::new(
                async_this_and_arguments_context,
            )),
        })
    }

    // port: RewriteAsyncFunctions.LexicalContext#getContextRootNode
    fn get_context_root_node(&self, context: ContextId) -> NodeId {
        self.contexts[context.0].context_root_node
    }

    fn unique_id_for(t: &mut NodeTraversal<'_>) -> String {
        let input = t
            .get_input()
            .cloned()
            .expect("java.lang.NullPointerException");
        t.get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input)
    }

    /// Returns the LexicalContext to use for visiting a node.
    ///
    /// `n`: This context's root node or one of its descendents. Returns this context or a new
    /// one for a child context.
    // port: RewriteAsyncFunctions.LexicalContext#getContextForNode
    // port: RewriteAsyncFunctions.RootContext#getContextForNode
    // port: RewriteAsyncFunctions.ParameterListContext#getContextForNode
    // port: RewriteAsyncFunctions.FunctionContext#getContextForNode
    fn get_context_for_node(
        &mut self,
        context: ContextId,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
    ) -> ContextId {
        match self.contexts[context.0].kind {
            LexicalContextKind::RootContext => {
                if n.is_function(t) {
                    let unique_id = Self::unique_id_for(t);
                    self.new_function_context(t.get_compiler(), n, unique_id)
                } else {
                    context
                }
            }
            LexicalContextKind::ParameterListContext { function_context } => {
                if n.is_function(t) {
                    // Function defined within a parameter list.
                    // e.g. `() => something`
                    // function someFunc(callback = () => something) {}
                    let unique_id = Self::unique_id_for(t);
                    self.new_function_context_with_outer(
                        t.get_compiler(),
                        function_context,
                        n,
                        unique_id,
                    )
                } else {
                    context
                }
            }
            LexicalContextKind::FunctionContext(_) => {
                if n == self.contexts[context.0].context_root_node {
                    context
                } else if n.is_function(t) {
                    let unique_id = Self::unique_id_for(t);
                    self.new_function_context_with_outer(t.get_compiler(), context, n, unique_id)
                } else if n.is_param_list(t) {
                    let unique_id = Self::unique_id_for(t);
                    self.new_parameter_list_context(context, n, unique_id)
                } else {
                    context
                }
            }
        }
    }

    // port: RewriteAsyncFunctions.LexicalContext#visit
    // port: RewriteAsyncFunctions.RootContext#visit
    // port: RewriteAsyncFunctions.ParameterListContext#visit
    fn visit_context(&mut self, context: ContextId, t: &mut NodeTraversal<'_>, n: NodeId) {
        match self.contexts[context.0].kind {
            LexicalContextKind::RootContext => {
                // In root context we haven't entered an async function yet, so there's nothing
                // to do.
            }
            LexicalContextKind::ParameterListContext { function_context } => {
                let async_context = self
                    .function_context(function_context)
                    .async_this_and_arguments_context;
                if async_context.is_some() && async_context != Some(function_context) {
                    // e.g.
                    // async function outer(outerT = this) {
                    //   // `this` in outer parameter list must remain unchanged
                    //   // but inner parameter list must be aliased
                    //   const inner = async (t = this) => t;
                    // }
                    self.function_context_visit(function_context, t, n);
                }
            }
            LexicalContextKind::FunctionContext(_) => self.function_context_visit(context, t, n),
        }
    }

    fn async_context(&self, context: ContextId) -> ContextId {
        self.function_context(context)
            .async_this_and_arguments_context
            .expect("java.lang.NullPointerException")
    }

    // port: RewriteAsyncFunctions.FunctionContext#recordAsyncThisReplacementWasDone
    fn record_async_this_replacement_was_done(&mut self, context: ContextId, type_of_this: Type) {
        let async_context = self.async_context(context);
        let f = self.function_context_mut(async_context);
        f.must_add_async_this_variable = true;
        f.type_of_this = Some(type_of_this);
    }

    // port: RewriteAsyncFunctions.FunctionContext#getOrCreateSuperPropertyWrapperInfo
    fn get_or_create_super_property_wrapper_info(
        &mut self,
        compiler: &mut AbstractCompiler,
        context: ContextId,
        super_dot_property_node: NodeId,
    ) -> &SuperPropertyWrapperInfo {
        let async_context = self.async_context(context);
        self.function_context_mut(async_context)
            .super_property_wrappers
            .get_or_create_super_property_wrapper_info(compiler, super_dot_property_node)
    }

    // port: RewriteAsyncFunctions.FunctionContext#recordAsyncArgumentsReplacementWasDone
    fn record_async_arguments_replacement_was_done(&mut self, context: ContextId) {
        let async_context = self.async_context(context);
        self.function_context_mut(async_context)
            .must_add_async_arguments_variable = true;
    }

    // port: RewriteAsyncFunctions.FunctionContext#recordAsyncNewTargetReplacementWasDone
    fn record_async_new_target_replacement_was_done(
        &mut self,
        context: ContextId,
        type_of_new_target: Type,
    ) {
        let async_context = self.async_context(context);
        let f = self.function_context_mut(async_context);
        f.must_add_async_new_target_variable = true;
        f.type_of_new_target = Some(type_of_new_target);
    }

    /// Creates a new reference to the variable used to hold the value of `this` for async
    /// functions.
    // port: RewriteAsyncFunctions.FunctionContext#createThisVariableReference
    fn create_this_variable_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        context: ContextId,
        type_of_this: Type,
    ) -> NodeId {
        self.record_async_this_replacement_was_done(context, type_of_this.clone());
        let async_context = self.async_context(context);
        let name = format!("{ASYNC_THIS}{}", self.contexts[async_context.0].unique_id);
        self.ast_factory.create_name(compiler, name, type_of_this)
    }

    // port: RewriteAsyncFunctions.FunctionContext#createNewTargetVariableReference
    fn create_new_target_variable_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        context: ContextId,
        type_of_new_target: Type,
    ) -> NodeId {
        self.record_async_new_target_replacement_was_done(context, type_of_new_target.clone());
        let async_context = self.async_context(context);
        let name = format!(
            "{ASYNC_NEW_TARGET}{}",
            self.contexts[async_context.0].unique_id
        );
        self.ast_factory
            .create_name(compiler, name, type_of_new_target)
    }

    // port: RewriteAsyncFunctions.FunctionContext#createWrapperArrowFunction
    fn create_wrapper_arrow_function(
        &self,
        compiler: &mut AbstractCompiler,
        wrapper_info: &SuperPropertyWrapperInfo,
    ) -> NodeId {
        // super.propertyName
        let super_dot_property = wrapper_info
            .first_instance_of_super_dot_property
            .clone_tree(compiler);

        // () => { return super.propertyName; };
        let ret = self.ast_factory.create_return(compiler, super_dot_property);
        let block = self.ast_factory.create_block(compiler, &[ret]);
        self.ast_factory
            .create_zero_arg_arrow_function_for_expression(compiler, block)
    }

    // port: RewriteAsyncFunctions.FunctionContext#visit
    #[allow(clippy::collapsible_match)] // Java nests the `if` inside `case NAME`
    fn function_context_visit(&mut self, context: ContextId, t: &mut NodeTraversal<'_>, n: NodeId) {
        let context_root_node = self.contexts[context.0].context_root_node;
        let async_this_and_arguments_context = self
            .function_context(context)
            .async_this_and_arguments_context;
        if context_root_node == n && context_root_node.is_async_function(t) {
            // We're visiting an async function.
            // All of its descendent nodes will have been updated as necessary, so now we just
            // need to convert the function itself.
            self.convert_async_function(t, context);
        } else if let Some(async_context) = async_this_and_arguments_context {
            // We're in the context of an async function's body, so we need to do some
            // replacements.
            let compiler = t.get_compiler();
            match n.get_token(compiler) {
                Token::NAME => {
                    if n.matches_name(compiler, "arguments") {
                        n.set_string(
                            compiler,
                            format!(
                                "{ASYNC_ARGUMENTS}{}",
                                self.contexts[async_context.0].unique_id
                            ),
                        );
                        if compiler.get_life_cycle_stage().is_normalized() {
                            // TODO: b/322009741 - Stop depending on lifeCycleStage.isNormalized()
                            // to decide constness
                            n.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
                        }
                        self.record_async_arguments_replacement_was_done(async_context);
                        compiler.report_change_to_change_scope(context_root_node);
                    }
                }
                Token::THIS => {
                    let reference = self.create_this_variable_reference(
                        compiler,
                        async_context,
                        AstFactory::type_node(n),
                    );
                    n.replace_with(compiler, reference);
                    compiler.report_change_to_change_scope(context_root_node);
                }
                Token::NEW_TARGET => {
                    let reference = self.create_new_target_variable_reference(
                        compiler,
                        async_context,
                        AstFactory::type_node(n),
                    );
                    n.replace_with(compiler, reference);
                    compiler.report_change_to_change_scope(context_root_node);
                }
                Token::SUPER => {
                    let parent = n.get_parent(compiler).unwrap();
                    if !parent.is_get_prop(compiler) {
                        compiler.report(JSError::make(
                            compiler,
                            parent,
                            &transpilation_util::CANNOT_CONVERT_YET,
                            &["super expression"],
                        ));
                    } else if NodeUtil::is_l_value(compiler, parent) {
                        // NOTE: `super.prop = x` is valid, and can be useful in overridden
                        // setters, but these cannot be async. For now, we don't support this
                        // construct in async methods.
                        let grandparent = parent.get_parent(compiler).unwrap();
                        compiler.report(JSError::make(
                            compiler,
                            grandparent,
                            &transpilation_util::CANNOT_CONVERT_YET,
                            &["assignment to super property"],
                        ));
                    }
                    // different name for parent for better readability
                    let super_dot_property = parent;

                    let super_property_wrapper_info = self
                        .get_or_create_super_property_wrapper_info(
                            compiler,
                            async_context,
                            super_dot_property,
                        )
                        .clone();

                    // super.x   =>   $jscomp$super$get$x()
                    let mut get_prop_replacement = super_property_wrapper_info
                        .create_wrapper_function_call_node(&self.ast_factory, compiler);
                    let grandparent = super_dot_property.get_parent(compiler).unwrap();
                    if grandparent.is_call(compiler)
                        && grandparent.get_first_child(compiler) == Some(super_dot_property)
                    {
                        // $jscomp$super$get$x(args) =>
                        // $jscomp$super$get$x().call($jscomp$async$this, args)
                        get_prop_replacement = self.ast_factory.create_get_prop_with_unknown_type(
                            compiler,
                            get_prop_replacement,
                            "call",
                        );
                        let async_context_root = self.get_context_root_node(async_context);
                        let enclosing_class =
                            NodeUtil::get_enclosing_class(compiler, async_context_root)
                                .expect("java.lang.NullPointerException");
                        let this_alias = self
                            .ast_factory
                            .create_this_alias_reference_for_es6_class(
                                compiler,
                                format!("{ASYNC_THIS}{}", self.contexts[async_context.0].unique_id),
                                enclosing_class,
                            )
                            .srcref(compiler, super_dot_property);
                        this_alias.insert_after(compiler, super_dot_property);
                        self.record_async_this_replacement_was_done(
                            async_context,
                            AstFactory::type_node(this_alias),
                        );
                    } else if grandparent.is_tagged_template_lit(compiler)
                        && grandparent.get_first_child(compiler) == Some(super_dot_property)
                    {
                        // $jscomp$super$get$x`template` =>
                        // $jscomp$super$get$x().bind($jscomp$async$this)`template`
                        let async_context_root = self.get_context_root_node(async_context);
                        let enclosing_class =
                            NodeUtil::get_enclosing_class(compiler, async_context_root)
                                .expect("java.lang.NullPointerException");
                        let this_alias = self
                            .ast_factory
                            .create_this_alias_reference_for_es6_class(
                                compiler,
                                format!("{ASYNC_THIS}{}", self.contexts[async_context.0].unique_id),
                                enclosing_class,
                            )
                            .srcref(compiler, super_dot_property);
                        self.record_async_this_replacement_was_done(
                            async_context,
                            AstFactory::type_node(this_alias),
                        );
                        let bind = self.ast_factory.create_get_prop_with_unknown_type(
                            compiler,
                            get_prop_replacement,
                            "bind",
                        );
                        get_prop_replacement = self.ast_factory.create_call_with_unknown_type(
                            compiler,
                            bind,
                            &[this_alias],
                        );
                        grandparent.put_boolean_prop(compiler, Prop::FREE_CALL, true);
                    }
                    let get_prop_replacement =
                        get_prop_replacement.srcref_tree(compiler, super_dot_property);
                    super_dot_property.replace_with(compiler, get_prop_replacement);
                    compiler.report_change_to_change_scope(context_root_node);
                }
                Token::AWAIT => {
                    // Awaits become yields in the converted async function's inner generator
                    // function.
                    let value = n.remove_first_child(compiler).unwrap();
                    let yield_node =
                        self.ast_factory
                            .create_yield(compiler, AstFactory::type_node(n), value);
                    n.replace_with(compiler, yield_node);
                }
                _ => {}
            }
        }
    }

    // port: RewriteAsyncFunctions#convertAsyncFunction
    fn convert_async_function(&mut self, t: &mut NodeTraversal<'_>, function_context: ContextId) {
        let original_function = self.get_context_root_node(function_context);
        original_function.set_is_async_function(t, false);
        let mut original_body = original_function.get_last_child(t).unwrap();
        if original_function.is_from_externs(t) {
            // A function defined in externs will never be executed, so we don't need to
            // transpile it. Make sure it has an empty body though so later passes won't trip over
            // uses of `await` or anything like that.
            if !NodeUtil::is_empty_block(t, original_body) {
                // TODO(b/119685646): Maybe we should warn for non-empty functions in externs?
                let block = self.ast_factory.create_block(t.get_compiler(), &[]);
                original_body.replace_with(t, block);
                NodeUtil::mark_functions_deleted(t.get_compiler(), original_body);
            }
            return;
        }
        let new_body = self.ast_factory.create_block(t.get_compiler(), &[]);
        original_body.replace_with(t, new_body);

        let async_unique_id = {
            let async_context = self.async_context(function_context);
            self.contexts[async_context.0].unique_id.clone()
        };
        if self
            .function_context(function_context)
            .must_add_async_this_variable
        {
            // const this$ = this;
            let type_of_this = self
                .function_context(function_context)
                .type_of_this
                .clone()
                .expect("java.lang.NullPointerException");
            let this_node = self.ast_factory.create_this(t.get_compiler(), type_of_this);
            let declaration = self.ast_factory.create_single_const_name_declaration(
                t.get_compiler(),
                format!("{ASYNC_THIS}{async_unique_id}"),
                this_node,
            );
            new_body.add_child_to_back(t, declaration);
            let script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::CONST_DECLARATIONS);
        }
        if self
            .function_context(function_context)
            .must_add_async_arguments_variable
        {
            // const arguments$ = arguments;
            let declaration = self.ast_factory.create_arguments_alias_declaration(
                t.get_compiler(),
                format!("{ASYNC_ARGUMENTS}{async_unique_id}"),
            );
            new_body.add_child_to_back(t, declaration);
            let script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::CONST_DECLARATIONS);
        }
        if self
            .function_context(function_context)
            .must_add_async_new_target_variable
        {
            // const newTarget$ = new.target;
            let type_of_new_target = self
                .function_context(function_context)
                .type_of_new_target
                .clone()
                .expect("java.lang.NullPointerException");
            let new_target = self
                .ast_factory
                .create_new_target(t.get_compiler(), type_of_new_target);
            let declaration = self.ast_factory.create_single_const_name_declaration(
                t.get_compiler(),
                format!("{ASYNC_NEW_TARGET}{async_unique_id}"),
                new_target,
            );
            new_body.add_child_to_back(t, declaration);
            let script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::CONST_DECLARATIONS);
        }
        let wrapper_count = self
            .function_context(function_context)
            .super_property_wrappers
            .property_name_to_type_map
            .len();
        for i in 0..wrapper_count {
            let super_property_wrapper_info = self
                .function_context(function_context)
                .super_property_wrappers
                .as_collection()
                .nth(i)
                .unwrap();
            let arrow_function =
                self.create_wrapper_arrow_function(t.get_compiler(), super_property_wrapper_info);
            let wrapper_function_name = super_property_wrapper_info.wrapper_function_name.clone();
            // const super$get$x = () => super.x;
            let arrow_function_declaration_statement =
                self.ast_factory.create_single_const_name_declaration(
                    t.get_compiler(),
                    wrapper_function_name,
                    arrow_function,
                );
            new_body.add_child_to_back(t, arrow_function_declaration_statement);

            // Make sure the compiler knows about the new arrow function's scope
            t.get_compiler()
                .report_change_to_change_scope(arrow_function);
            // Record that we've added arrow functions and const declarations to this script,
            // so later transpilations of those features will run, if needed.
            let enclosing_script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(
                t.get_compiler(),
                enclosing_script,
                Feature::ARROW_FUNCTIONS,
            );
            NodeUtil::add_feature_to_script(
                t.get_compiler(),
                enclosing_script,
                Feature::CONST_DECLARATIONS,
            );
        }

        // Normalize arrow function short body to block body
        if !original_body.is_block(t) {
            let ret = self
                .ast_factory
                .create_return(t.get_compiler(), original_body);
            original_body = self
                .ast_factory
                .create_block(t.get_compiler(), &[ret])
                .srcref_tree_if_missing(t, original_body);
        }
        // NOTE: visit() will already have made appropriate replacements in originalBody so it may
        // be used as the generator function body.
        let generator_function = self.ast_factory.create_zero_arg_generator_function(
            t.get_compiler(),
            "",
            original_body,
            /* returnType= */ None,
        );
        t.get_compiler()
            .report_change_to_change_scope(generator_function);
        let script = t.get_current_script().unwrap();
        NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::GENERATORS);

        // return $jscomp.asyncExecutePromiseGeneratorFunction(function* () { ... });
        let call = self
            .ast_factory
            .create_jscomp_async_execute_promise_generator_function_call(
                t.get_compiler(),
                &self.namespace,
                generator_function,
            );
        let ret = self.ast_factory.create_return(t.get_compiler(), call);
        new_body.add_child_to_back(t, ret);

        new_body.srcref_tree_if_missing(t, original_body);
        t.get_compiler().report_change_to_enclosing_scope(new_body);
    }
}

impl CompilerPass for RewriteAsyncFunctions {
    // port: RewriteAsyncFunctions#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, transpiled_features(), &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
    }
}

impl Callback for RewriteAsyncFunctions {
    // port: RewriteAsyncFunctions#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if parent.is_none() {
            check_state!(self.context_stack.is_empty());
            let unique_id = Self::unique_id_for(t);
            let root_context = self.new_root_context(n, unique_id);
            self.context_stack.push(root_context);
        } else {
            let parent_context = *self
                .context_stack
                .last()
                .expect("java.lang.NullPointerException");
            let node_context = self.get_context_for_node(parent_context, t, n);
            if node_context != parent_context {
                self.context_stack.push(node_context);
            }
        }
        true
    }

    // port: RewriteAsyncFunctions#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let context = *self
            .context_stack
            .last()
            .expect("java.lang.NullPointerException");
        self.visit_context(context, t, n);
        if self.get_context_root_node(context) == n {
            self.context_stack.pop();
        }
    }
}
