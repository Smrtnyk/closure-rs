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
//   src/com/google/javascript/jscomp/DevirtualizeMethods.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    js_chunk::JSChunk,
    node_util::NodeUtil,
    optimize_calls::{CallGraphCompilerPass, ReferenceMap},
};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId, Prop},
    token::Token,
};

/// Rewrites prototype and static methods as global, free functions that take the receiver as
/// their first argument.
///
/// This transformation simplifies the call graph so smart name removal, cross chunk code motion
/// and other passes can do more.
///
/// To work effectively, this pass depends on DisambiguateProperties running first to do a lot of
/// heavy-lifting. It assumes that different methods will have unique names which in general isn't
/// true for source JavaScript.
///
/// This pass should only be used in production code if property and variable renaming are turned
/// on. Resulting code may also benefit from `--collapse_anonymous_functions` and
/// `--collapse_variable_declarations`
///
/// This pass only rewrites functions that are part of a type's prototype or are statics on a
/// ctor/interface function. A host of other preconditions must also be met. Functions that access
/// the "arguments" variable arguments object are not eligible for this optimization, for example.
///
/// For example:
///
/// ```text
///     A.prototype.accumulate = function(value) {
///       this.total += value; return this.total
///     }
///     var total = a.accumulate(2)
/// ```
///
/// will be rewritten as:
///
/// ```text
///     var accumulate = function(self, value) {
///       self.total += value; return self.total
///     }
///     var total = accumulate(a, 2)
/// ```
///
/// A similar transformation occurs for:
///
/// ```text
///     /** @constructor */
///     function A() { }
///
///     A.accumulate = function(value) {
///       this.total += value; return this.total
///     }
///     var total = a.accumulate(2)
/// ```
#[derive(Default)]
pub struct DevirtualizeMethods {
    // Java: `private ReferenceMap refMap;`, set once by process. The map is borrowed for the
    // duration of process only, so it is passed to the methods that read it; this flag keeps
    // Java's "called once" check.
    ref_map: bool,
}

impl DevirtualizeMethods {
    // port: DevirtualizeMethods#DevirtualizeMethods
    pub fn new() -> Self {
        Self { ref_map: false }
    }

    // port: DevirtualizeMethods#processReferenceList
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn process_reference_list(
        &mut self,
        compiler: &mut AbstractCompiler,
        ref_map: &ReferenceMap,
        name: &JsString,
        sites: &[NodeId],
    ) {
        let functions_by_site = ReferenceMap::get_function_nodes(compiler, sites);
        if functions_by_site.is_empty() {
            return; // We can't devirtualize without a definition.
        }

        // Use the first definition to ensure that all invocations are after the definition
        // (temporally). It's  possible that if class `A` is defined after class `B` there are
        // calls to `A::foo` before `B::foo` is defined.
        let canonical_definition_site = *functions_by_site.keys().next().unwrap();

        let call_sites: Vec<NodeId> = sites
            .iter()
            .copied()
            // If a site has no associated functions, it must be a call site.
            .filter(|s| !functions_by_site.contains_key(s))
            .collect();
        if call_sites.is_empty() {
            return; // No need to devirtualize without calls.
        }

        // ImmutableListMultimap#values(): every value, keys in order.
        let functions: Vec<NodeId> = functions_by_site.values().flatten().copied().collect();

        // Check that this method is safe for devirtualization. These are in (estimated)
        // increasing order of cost.
        if !functions_by_site
            .keys()
            .all(|&s| self.is_eligible_definition_site(compiler, ref_map, name, s))
        {
            return;
        } else if !functions
            .iter()
            .all(|&f| Self::is_eligible_definition_function(compiler, f))
        {
            return;
        } else if !call_sites
            .iter()
            .all(|&c| Self::is_eligible_call_site(compiler, c, canonical_definition_site))
        {
            return;
        } else if !Self::all_definitions_equivalent(compiler, &functions) {
            // Remember that this is only valid because we already checked the scoping of the
            // definitions.
            return;
        }

        let devirtualized_name = Self::rewritten_method_name_of(name);
        let best_js_doc = NodeUtil::get_best_jsdoc_info(compiler, canonical_definition_site);
        let is_constant_name = best_js_doc.is_some_and(|info| info.is_constant());

        for call_site in call_sites {
            Self::rewrite_call(compiler, call_site, &devirtualized_name, is_constant_name);
        }
        // We only have to rewrite one definition. We've checked they're all identical so any of
        // them can replace the others. The un-rewritten ones will be dead-code eliminated.
        Self::rewrite_definition(
            compiler,
            canonical_definition_site,
            &devirtualized_name,
            is_constant_name,
        );
    }

    // port: DevirtualizeMethods#isPrototypeOrStaticMethodDefinition
    fn is_prototype_or_static_method_definition(
        &self,
        compiler: &mut AbstractCompiler,
        ref_map: &ReferenceMap,
        node: NodeId,
    ) -> bool {
        let parent = node.get_parent(compiler);
        let grandparent = node.get_grandparent(compiler);
        let (Some(parent), Some(grandparent)) = (parent, grandparent) else {
            return false;
        };

        match node.get_token(compiler) {
            Token::MEMBER_FUNCTION_DEF => {
                if NodeUtil::is_es6_constructor_member_function_def(compiler, node) {
                    return false; // Constructors aren't methods.
                }

                true
            }
            Token::GETPROP => {
                // Case: `Foo.prototype.bar = function() { };
                if !node.is_first_child_of(compiler, Some(parent))
                    || !NodeUtil::is_expr_assign(compiler, grandparent)
                    || !parent
                        .get_last_child(compiler)
                        .unwrap()
                        .is_function(compiler)
                {
                    return false;
                }

                if NodeUtil::is_prototype_property(compiler, node) {
                    return true;
                }

                let receiver = node.get_first_child(compiler).unwrap();
                if self.is_definitely_ctor_or_interface(compiler, ref_map, receiver) {
                    return true;
                }

                false
            }
            Token::STRING_KEY => {
                // Case: `Foo.prototype = {
                //          bar: function() { },
                //        }`
                check_argument!(
                    parent.is_object_lit(compiler),
                    "%s",
                    parent.to_string(compiler)
                );

                if !parent.is_second_child_of(compiler, Some(grandparent))
                    || !NodeUtil::is_prototype_assignment(
                        compiler,
                        grandparent.get_first_child(compiler).unwrap(),
                    )
                    || !node
                        .get_first_child(compiler)
                        .unwrap()
                        .is_function(compiler)
                {
                    return false;
                }

                true
            }
            _ => false,
        }
    }

    // port: DevirtualizeMethods#isDefinitelyCtorOrInterface
    fn is_definitely_ctor_or_interface(
        &self,
        compiler: &mut AbstractCompiler,
        ref_map: &ReferenceMap,
        receiver: NodeId,
    ) -> bool {
        let Some(qname) = receiver.get_qualified_name(compiler) else {
            return false;
        };

        // It's safe to rely on the global scope because normalization has made all names unique.
        // No local will shadow a global name, and we're ok with missing some statics.
        let Some(var) = ref_map.get_global_scope().unwrap().get_var(compiler, qname) else {
            return false;
        };

        if var.is_class(compiler) {
            return true;
        }

        let jsdoc = var.get_jsdoc_info(compiler);
        match jsdoc {
            None => return false,
            Some(jsdoc) if jsdoc.is_constructor_or_interface() => {
                return true; // Case: `@constructor`, `@interface`, `@record`.
            }
            Some(_) => {}
        }

        false
    }

    /// Determines if a method definition site is eligible for rewrite as a global.
    ///
    /// In order to be eligible for rewrite, the definition site must:
    ///
    /// - Not be exported
    /// - Be for a prototype method
    // port: DevirtualizeMethods#isEligibleDefinitionSite
    fn is_eligible_definition_site(
        &self,
        compiler: &mut AbstractCompiler,
        ref_map: &ReferenceMap,
        name: &JsString,
        definition_site: NodeId,
    ) -> bool {
        match definition_site.get_token(compiler) {
            Token::GETPROP
            | Token::MEMBER_FUNCTION_DEF
            | Token::STRING_KEY
            | Token::MEMBER_FIELD_DEF => {}
            _ => {
                // No other node types are supported.
                panic!("{}", definition_site.to_string(compiler))
            }
        }

        // Exporting a method prevents rewrite.
        let coding_convention = compiler.get_coding_convention();
        if coding_convention.is_exported(name, /* local= */ false) {
            return false;
        }

        if !self.is_prototype_or_static_method_definition(compiler, ref_map, definition_site) {
            return false;
        }

        true
    }

    /// Determines if a method definition function is eligible for rewrite as a global function.
    ///
    /// In order to be eligible for rewrite, the definition function must:
    ///
    /// - Be instantiated exactly once
    /// - Be the only possible implementation at a given site
    /// - Not refer to its `arguments`; no implicit varags
    /// - Not be an arrow function
    // port: DevirtualizeMethods#isEligibleDefinitionFunction
    fn is_eligible_definition_function(
        compiler: &AbstractCompiler,
        definition_function: NodeId,
    ) -> bool {
        let ast: &Ast = compiler;
        check_argument!(
            definition_function.is_function(ast),
            "%s",
            definition_function.to_string(ast)
        );

        if definition_function.is_arrow_function(ast) {
            return false;
        }

        let mut ancestor = definition_function.get_parent(ast);
        while let Some(a) = ancestor {
            // The definition must be made exactly once. (i.e. not in a loop, conditional, or
            // function)
            if Self::is_scoping_or_branching_construct(ast, a) {
                return false;
            }

            // TODO(nickreid): Support this so long as the definition doesn't reference the name.
            // We can't allow this in general because references to the local name:
            //  - won't be rewritten correctly
            //  - won't be the same across multiple definitions, even if they are node-wise
            //    identical.
            if a.is_class(ast) && Self::local_name_is_declared_by_class(ast, a) {
                return false;
            }
            ancestor = a.get_parent(ast);
        }

        if NodeUtil::has(
            ast,
            definition_function,
            &|ast: &Ast, n: NodeId| n.is_super(ast),
            &|_: &Ast, _: NodeId| true,
        ) {
            // TODO(b/120452418): Remove this when we have a rewrite for `super`. We punted
            // initially due to complexity.
            return false;
        }

        if NodeUtil::does_function_reference_own_arguments_object(ast, definition_function) {
            // Functions that access "arguments" are not eligible since rewriting changes the
            // structure of the function params.
            return false;
        }

        true
    }

    /// Determines if a method call is eligible for rewrite as a global function.
    ///
    /// In order to be eligible for rewrite, the call must:
    ///
    /// - Property is never accessed outside a function call context.
    // port: DevirtualizeMethods#isEligibleCallSite
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn is_eligible_call_site(
        compiler: &AbstractCompiler,
        access: NodeId,
        definition_site: NodeId,
    ) -> bool {
        let invocation = access.get_parent(compiler).unwrap();
        if !NodeUtil::is_invocation_target(compiler, access) || !invocation.is_call(compiler) {
            // TODO(nickreid): Use the same definition of "a call" as
            // `OptimizeCalls::ReferenceMap::isCallTarget`.
            //
            // Accessing the property in any way besides CALL has issues:
            //   - tear-off: invocations can't be tracked
            //   - as constructor: unsupported rewrite
            //   - as tagged template string: unsupported rewrite
            //   - as optional chaining call: original invocation target may be null/undefined
            // and rewriting creates an unconditional call to a well-defined global function
            return false;
        }

        // We can't rewrite functions called in chunks that do not depend on the defining chunk.
        // This is due to a subtle execution order change introduced by rewriting. Example:
        //
        //     `x.foo().bar()` => `JSCompiler_StaticMethods_bar(x.foo())`
        //
        // Note how `JSCompiler_StaticMethods_bar` will be resolved before `x.foo()` is executed.
        // In the case that `x.foo()` defines `JSCompiler_StaticMethods_bar` (e.g. by dynamically
        // loading the defining chunk) this change in ordering will cause a `ReferenceError`. No
        // error would be thrown by the original code because `bar` would be resolved later.
        //
        // We choose to use chunk ordering to avoid this issue because:
        //   - The other eligibility checks for devirtualization prevent any other dangerous cases
        //     that JSCompiler supports.
        //   - Rewriting all call-sites in a way that preserves exact ordering (e.g. using
        //     `ExpressionDecomposer`) has a significant code-size impact (circa 2018-11-19).

        let chunk_graph = compiler.get_chunk_graph();
        let definition_chunk: Option<JSChunk> = Self::chunk_for_node(compiler, definition_site);
        let call_chunk: Option<JSChunk> = Self::chunk_for_node(compiler, access);
        if definition_chunk == call_chunk {
            // Do nothing.
        } else if call_chunk.is_none() {
            return false;
        } else if !chunk_graph.unwrap().depends_on(
            call_chunk.as_ref().unwrap(),
            definition_chunk.as_ref().unwrap(),
        ) {
            return false;
        }

        true
    }

    /// Given a set of method definitions, verify they are the same.
    // port: DevirtualizeMethods#allDefinitionsEquivalent
    fn all_definitions_equivalent(compiler: &AbstractCompiler, definitions: &[NodeId]) -> bool {
        if definitions.is_empty() {
            return true;
        }

        let definition = definitions[0];
        check_argument!(
            definition.is_function(compiler),
            "%s",
            definition.to_string(compiler)
        );

        definitions
            .iter()
            .all(|&d| compiler.are_nodes_equal_for_inlining(d, definition))
    }

    /// Rewrites object method call sites as calls to global functions that take "this" as their
    /// first argument.
    ///
    /// Before: o.foo(a, b, c)
    ///
    /// After: foo(o, a, b, c)
    // port: DevirtualizeMethods#rewriteCall
    fn rewrite_call(
        compiler: &mut AbstractCompiler,
        getprop: NodeId,
        new_method_name: &JsString,
        is_constant_name: bool,
    ) {
        check_argument!(
            getprop.is_get_prop(compiler),
            "%s",
            getprop.to_string(compiler)
        );
        let call = getprop.get_parent(compiler).unwrap();
        check_argument!(call.is_call(compiler), "%s", call.to_string(compiler));
        let receiver = getprop.get_first_child(compiler).unwrap();

        // This rewriting does not exactly preserve order of operations; the newly inserted static
        // method name will be resolved before `receiver` is evaluated. This is known to be safe
        // due to the eligibility checks earlier in the pass.
        //
        // We choose not to do a full-fidelity rewriting (e.g. using `ExpressionDecomposer`)
        // because doing so means extracting `receiver` into a new variable at each call-site.
        // This  has a significant code-size impact (circa 2018-11-19).

        receiver.detach(compiler);
        getprop.replace_with(compiler, receiver);
        let new_receiver = IR::name(compiler, new_method_name.clone())
            .copy_type_from(compiler, getprop)
            .srcref(compiler, getprop);
        if is_constant_name {
            new_receiver.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        }
        call.add_child_to_front(compiler, new_receiver);

        if receiver.is_super(compiler) {
            // Case: `super.foo(a, b)` => `foo(this, a, b)`
            receiver.set_token(compiler, Token::THIS);
        }

        call.put_boolean_prop(compiler, Prop::FREE_CALL, true);
        compiler.report_change_to_enclosing_scope(call);
    }

    /// Rewrites method definitions as global functions that take "this" as their first argument.
    ///
    /// Before: a.prototype.b = function(a, b, c) {...}
    ///
    /// After: var b = function(self, a, b, c) {...}
    // port: DevirtualizeMethods#rewriteDefinition
    fn rewrite_definition(
        compiler: &mut AbstractCompiler,
        definition_site: NodeId,
        new_method_name: &JsString,
        is_constant_name: bool,
    ) {
        let function: NodeId;
        let subtree_to_remove: NodeId;
        let name_source: NodeId;

        match definition_site.get_token(compiler) {
            Token::GETPROP => {
                function = definition_site
                    .get_parent(compiler)
                    .unwrap()
                    .get_last_child(compiler)
                    .unwrap();
                name_source = definition_site;
                subtree_to_remove =
                    NodeUtil::get_enclosing_statement(compiler, definition_site).unwrap();
            }
            Token::STRING_KEY | Token::MEMBER_FUNCTION_DEF => {
                function = definition_site.get_last_child(compiler).unwrap();
                name_source = definition_site;
                subtree_to_remove = definition_site;
            }
            _ => panic!("{}", definition_site.to_string(compiler)),
        }

        // Define a new variable after the original declaration.
        let statement = NodeUtil::get_enclosing_statement(compiler, definition_site).unwrap();
        let new_name_node =
            IR::name(compiler, new_method_name.clone()).srcref_if_missing(compiler, name_source);
        let new_var_node =
            IR::var(compiler, new_name_node).srcref_if_missing(compiler, name_source);
        new_var_node.insert_before(compiler, statement);
        if is_constant_name {
            new_name_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        }

        // Copy the JSDocInfo, if any, from the original declaration
        let original_js_doc = NodeUtil::get_best_jsdoc_info(compiler, definition_site);
        new_var_node.set_jsdoc_info(compiler, original_js_doc);

        // Attach the function to the new variable.
        function.detach(compiler);
        new_name_node.add_child_to_front(compiler, function);

        // Create the `this` param.
        let self_name = new_method_name.concat(&JsString::from("$self"));
        let param_list = function.get_second_child(compiler).unwrap();
        let self_param =
            IR::name(compiler, self_name.clone()).srcref_if_missing(compiler, function);
        param_list.add_child_to_front(compiler, self_param);
        compiler.report_change_to_enclosing_scope(param_list);

        // Eliminate `this`.
        let params = function.get_second_child(compiler).unwrap();
        Self::replace_references_to_this(compiler, params, &self_name); // In default param values.
        let body = function.get_last_child(compiler).unwrap();
        Self::replace_references_to_this(compiler, body, &self_name); // In function body.

        // Clean up dangling AST.
        NodeUtil::delete_node(compiler, subtree_to_remove);
        compiler.report_change_to_enclosing_scope(new_var_node);
    }

    /// Replaces references to "this" with references to name. Do not traverse function
    /// boundaries.
    // port: DevirtualizeMethods#replaceReferencesToThis
    fn replace_references_to_this(compiler: &mut AbstractCompiler, node: NodeId, name: &JsString) {
        if (node.is_function(compiler) && !node.is_arrow_function(compiler))
            || node.is_member_field_def(compiler)
            || NodeUtil::is_class_static_block(compiler, node)
        {
            // Functions (besides arrows), member field defs, and static blocks create a new
            // binding for `this`.
            return;
        }

        if node.is_computed_field_def(compiler) {
            // Computed field keys are evaluated in the enclosing scope, but field initializers
            // create a new binding for `this`.
            if let Some(key) = node.get_first_child(compiler) {
                if key.is_this(compiler) {
                    let new_name = IR::name(compiler, name.clone())
                        .srcref(compiler, key)
                        .copy_type_from(compiler, key);
                    key.replace_with(compiler, new_name);
                    compiler.report_change_to_enclosing_scope(new_name);
                } else {
                    Self::replace_references_to_this(compiler, key, name);
                }
            }
            return;
        }

        let mut child = node.get_first_child(compiler);
        while let Some(c) = child {
            let next = c.get_next(compiler);
            if c.is_this(compiler) {
                let new_name = IR::name(compiler, name.clone())
                    .srcref(compiler, c)
                    .copy_type_from(compiler, c);
                c.replace_with(compiler, new_name);
                compiler.report_change_to_enclosing_scope(new_name);
            } else {
                Self::replace_references_to_this(compiler, c, name);
            }
            child = next;
        }
    }

    // port: DevirtualizeMethods#chunkForNode
    fn chunk_for_node(compiler: &AbstractCompiler, node: NodeId) -> Option<JSChunk> {
        let script = NodeUtil::get_enclosing_script(compiler, node).unwrap();
        let input = compiler
            .get_input(&script.get_input_id(compiler).unwrap())
            .unwrap();
        input.get_chunk()
    }

    // port: DevirtualizeMethods#rewrittenMethodNameOf
    fn rewritten_method_name_of(original_method_name: &JsString) -> JsString {
        JsString::from("JSCompiler_StaticMethods_").concat(original_method_name)
    }

    /// Returns `true` iff a node may change the variable bindings of its subtree or cause that
    /// subtree to be executed not exactly once.
    ///
    /// This method does not include CLASS because CLASS does not always create a new binding and
    /// it is important for the success of this optimization to consider class methods.
    ///
    /// See localNameIsDeclaredByClass.
    // port: DevirtualizeMethods#isScopingOrBranchingConstruct
    fn is_scoping_or_branching_construct(ast: &Ast, node: NodeId) -> bool {
        NodeUtil::is_control_structure(ast, node) // Branching.
            || node.is_and(ast) // Branching.
            || node.is_or(ast) // Branching.
            || node.is_function(ast) // Branching & scoping.
            || node.is_block(ast) // Scoping.
    }

    /// Returns `true` iff a CLASS subtree declares a name local to the class body.
    ///
    /// Example:
    ///
    /// ```text
    /// const Foo = class Bar {
    ///   qux() { return Bar; }
    /// }
    /// ```
    // port: DevirtualizeMethods#localNameIsDeclaredByClass
    fn local_name_is_declared_by_class(ast: &Ast, clazz: NodeId) -> bool {
        check_argument!(clazz.is_class(ast), "%s", clazz.to_string(ast));

        if clazz.get_first_child(ast).unwrap().is_empty(ast) {
            return false; // There must be a name.
        } else if NodeUtil::is_statement(ast, clazz) {
            return false; // The name must be local.
        }

        true
    }
}

impl CallGraphCompilerPass for DevirtualizeMethods {
    // port: DevirtualizeMethods#process
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        _externs: NodeId,
        _root: NodeId,
        ref_map: &mut ReferenceMap,
    ) {
        check_state!(!self.ref_map, "`process` should only be called once.");

        self.ref_map = true;
        let ref_map: &ReferenceMap = ref_map;

        for (key, value) in ref_map.get_prop_references() {
            self.process_reference_list(compiler, ref_map, key, value);
        }
    }
}
