/*
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
//   src/com/google/javascript/jscomp/OptimizeCalls.java.

//! Port of `OptimizeCalls.java`: a root pass that is a container for other passes that should
//! run with a single call graph (`OptimizeParameters`, `OptimizeReturns`, `DevirtualizeMethods`,
//! `PureFunctionIdentifier`).

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::scope::ScopeId;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use indexmap::{IndexMap, IndexSet};

/// Java's `OptimizeCalls.CallGraphCompilerPass`. The compiler is passed explicitly
/// (DESIGN.md section 6) instead of being a field of the pass.
pub trait CallGraphCompilerPass {
    // port: OptimizeCalls.CallGraphCompilerPass#process
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
        references: &mut ReferenceMap,
    );
}

impl<F> CallGraphCompilerPass for F
where
    F: FnMut(&mut AbstractCompiler, NodeId, NodeId, &mut ReferenceMap),
{
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
        references: &mut ReferenceMap,
    ) {
        self(compiler, externs, root, references);
    }
}

pub struct OptimizeCalls {
    passes: Vec<Box<dyn CallGraphCompilerPass>>,
    consider_externs: bool,
}

pub struct Builder {
    // Java's `AbstractCompiler compiler` field: the pass never stores the compiler
    // (DESIGN.md section 6), so the builder records only that `setCompiler` was called, which
    // `build` checks like Java's `checkNotNull(compiler)`.
    compiler: bool,
    passes: Vec<Box<dyn CallGraphCompilerPass>>,
    consider_externs: Option<bool>, // Nullable to force users to specify a value.
}

impl Builder {
    // port: OptimizeCalls.Builder#setCompiler
    pub fn set_compiler(mut self, _compiler: &AbstractCompiler) -> Self {
        self.compiler = true;
        self
    }

    /// Sets whether or not to include references to extern names and properties in the
    /// [`ReferenceMap`] being generated.
    // port: OptimizeCalls.Builder#setConsiderExterns
    pub fn set_consider_externs(mut self, b: bool) -> Self {
        self.consider_externs = Some(b);
        self
    }

    // port: OptimizeCalls.Builder#addPass
    pub fn add_pass(mut self, pass: Box<dyn CallGraphCompilerPass>) -> Self {
        self.passes.push(pass);
        self
    }

    // port: OptimizeCalls.Builder#build
    pub fn build(self) -> OptimizeCalls {
        check_not_null!(self.compiler.then_some(()));
        let consider_externs = check_not_null!(self.consider_externs);

        OptimizeCalls::new(self.passes, consider_externs)
    }

    // port: OptimizeCalls.Builder#Builder
    fn new() -> Self {
        Self {
            compiler: false,
            passes: Vec::new(),
            consider_externs: None,
        }
    }
}

impl OptimizeCalls {
    // port: OptimizeCalls#OptimizeCalls
    fn new(passes: Vec<Box<dyn CallGraphCompilerPass>>, consider_externs: bool) -> Self {
        Self {
            passes,
            consider_externs,
        }
    }

    // port: OptimizeCalls#builder
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Delete getter and setter names from `references`.
    ///
    /// Accessor names are disqualified from being in the `ReferenceMap`. We don't intentionally
    /// collect them, but other properties may share the same names.
    // port: OptimizeCalls#eliminateAccessorsFrom
    fn eliminate_accessors_from(compiler: &AbstractCompiler, references: &mut ReferenceMap) {
        let accessor_summary = check_not_null!(compiler.get_accessor_summary());
        let accessors = accessor_summary.get_accessors();
        references
            .props
            .retain(|name, _| !accessors.contains_key(name));
    }

    // port: OptimizeCalls#safeSet
    fn safe_set(set: Option<&IndexSet<String>>) -> IndexSet<String> {
        match set {
            Some(set) => set.clone(),
            None => IndexSet::new(),
        }
    }

    /// Returns whether the provided name may be a candidate for call optimizations.
    // port: OptimizeCalls#mayBeOptimizableName
    pub fn may_be_optimizable_name(compiler: &AbstractCompiler, name: &JsString) -> bool {
        if compiler.get_coding_convention().is_exported_name(name) {
            return false;
        }

        // Avoid modifying a few special case functions. Specifically, $jscomp.inherits to
        // recognize 'inherits' calls. (b/27244988)
        if *name == NodeUtil::JSC_PROPERTY_NAME_FN
            || *name == "inherits"
            || *name == "$jscomp$inherits"
            || *name == "goog$inherits"
        {
            return false;
        }
        true
    }

    /// Returns whether the reference is a known non-aliasing reference.
    // port: OptimizeCalls#isAllowedReference
    pub fn is_allowed_reference(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        match parent.get_token(compiler) {
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                // inspecting the properties is allowed.
                return parent.get_second_child(compiler) == Some(n);
            }
            Token::INSTANCEOF | Token::TYPEOF | Token::IN => {
                return true;
            }
            Token::GETELEM | Token::GETPROP | Token::OPTCHAIN_GETPROP | Token::OPTCHAIN_GETELEM => {
                // Calls escape the "this" value. a.foo() aliases "a" as "this" but general
                // property references do not.
                let grandparent = parent.get_parent(compiler);
                if Some(n) == parent.get_first_child(compiler)
                    && grandparent.is_some_and(|grandparent| {
                        grandparent.is_call(compiler)
                            || grandparent.is_opt_chain_call(compiler)
                            || grandparent.is_tagged_template_lit(compiler)
                    })
                {
                    return false; // `a.foo()` or `a?.foo()` or `a?.[foo]()` or `a.foo`bar``
                }
                return true;
            }
            Token::CLASS => {
                if n.is_first_child_of(compiler, Some(parent)) {
                    // class Name {
                    // this is a definition, not a read reference
                    return false;
                } else {
                    // class SubClass extends Name {
                    check_state!(
                        n.is_second_child_of(compiler, Some(parent)),
                        "%s",
                        parent.to_string(compiler)
                    );
                    // find the constructor
                    if NodeUtil::get_es6_class_constructor_member_function_def(compiler, parent)
                        .is_none()
                    {
                        // The subclass has no explicit constructor, so `new SubClass()`
                        // implicitly calls `new Name(...arguments)`. This hidden call makes it
                        // harder to safely optimize the `Name` constructor, so we won't do it.
                        return false;
                    } else {
                        // We can still optimize the constructor of the class being extended as
                        // long as all child classes have explicit constructors, so we can see the
                        // `super()` calls in them and update them.
                        return true;
                    }
                }
            }
            _ => {
                if NodeUtil::is_name_declaration(compiler, Some(parent))
                    && !n.has_children(compiler)
                {
                    // allow "let x;"
                    return true;
                }
            }
        }
        false
    }
}

impl CompilerPass for OptimizeCalls {
    // port: OptimizeCalls#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Only global names are collected, which is insufficient if names have not been
        // normalized.
        check_state!(compiler.get_life_cycle_stage().is_normalized());

        if self.passes.is_empty() {
            return;
        }

        let mut references = ReferenceMap::new();
        {
            let mut callback =
                ReferenceMapBuildingCallback::new(compiler, &mut references, self.consider_externs);
            NodeTraversal::traverse_roots(compiler, &mut callback, externs, root);
        }
        Self::eliminate_accessors_from(compiler, &mut references);

        for pass in &mut self.passes {
            pass.process(compiler, externs, root, &mut references);
        }
    }
}

/// A reference map for global symbols and properties.
#[derive(Default)]
pub struct ReferenceMap {
    global_scope: Option<ScopeId>,
    names: IndexMap<JsString, Vec<NodeId>>,
    props: IndexMap<JsString, Vec<NodeId>>,
}

impl ReferenceMap {
    fn new() -> Self {
        Self::default()
    }

    // port: OptimizeCalls.ReferenceMap#addReference
    fn add_reference(data: &mut IndexMap<JsString, Vec<NodeId>>, name: JsString, n: NodeId) {
        let refs = data.entry(name).or_default();
        refs.push(n);
    }

    // port: OptimizeCalls.ReferenceMap#addNameReference
    pub fn add_name_reference(&mut self, name: JsString, n: NodeId) {
        Self::add_reference(&mut self.names, name, n);
    }

    // port: OptimizeCalls.ReferenceMap#addPropReference
    pub fn add_prop_reference(&mut self, name: JsString, n: NodeId) {
        Self::add_reference(&mut self.props, name, n);
    }

    // port: OptimizeCalls.ReferenceMap#getGlobalScope
    pub fn get_global_scope(&self) -> Option<ScopeId> {
        self.global_scope
    }

    // port: OptimizeCalls.ReferenceMap#getNameReferences
    pub fn get_name_references(&self) -> &IndexMap<JsString, Vec<NodeId>> {
        &self.names
    }

    // port: OptimizeCalls.ReferenceMap#getPropReferences
    pub fn get_prop_references(&self) -> &IndexMap<JsString, Vec<NodeId>> {
        &self.props
    }

    /// Mutable access to the entries behind `getNameReferences()`: Java hands out the live
    /// `entrySet()` view, whose `ArrayList` values callers may modify.
    pub fn get_name_references_mut(&mut self) -> &mut IndexMap<JsString, Vec<NodeId>> {
        &mut self.names
    }

    /// Mutable access to the entries behind `getPropReferences()` (see
    /// [`ReferenceMap::get_name_references_mut`]).
    pub fn get_prop_references_mut(&mut self) -> &mut IndexMap<JsString, Vec<NodeId>> {
        &mut self.props
    }

    /// Given a set of references, returns the set of known definitions; specifically, those of
    /// the form: `function x() { }` or `x = ...;`
    ///
    /// The result is Java's `ImmutableListMultimap`: keys in first-insertion order, each with its
    /// values in insertion order. A definition site with no function is not a key.
    // port: OptimizeCalls.ReferenceMap#getFunctionNodes
    pub fn get_function_nodes(
        compiler: &AbstractCompiler,
        definition_sites: &[NodeId],
    ) -> IndexMap<NodeId, Vec<NodeId>> {
        let mut result: IndexMap<NodeId, Vec<NodeId>> = IndexMap::new();
        for &def in definition_sites {
            let fns = Self::definition_function_nodes_for(compiler, def);
            if !fns.is_empty() {
                result.entry(def).or_default().extend(fns);
            }
        }
        result
    }

    /// Collects potential definition FUNCTIONs associated with a method definition site.
    // port: OptimizeCalls.ReferenceMap#definitionFunctionNodesFor
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn definition_function_nodes_for(
        compiler: &AbstractCompiler,
        definition_site: NodeId,
    ) -> Vec<NodeId> {
        if definition_site.is_getter_def(compiler) || definition_site.is_setter_def(compiler) {
            // TODO(nickreid): Support getters and setters. Ignore them for now since they aren't
            // "called".
            return Vec::new();
        }

        // Ignore detached nodes.
        let Some(parent) = definition_site.get_parent(compiler) else {
            return Vec::new();
        };

        let mut fns = Vec::new();
        match parent.get_token(compiler) {
            Token::CLASS => {
                if definition_site.is_first_child_of(compiler, Some(parent)) {
                    let constructor_fn_def =
                        NodeUtil::get_es6_class_constructor_member_function_def(compiler, parent);
                    if let Some(constructor_fn_def) = constructor_fn_def {
                        fns.push(constructor_fn_def.get_only_child(compiler));
                    }
                }
            }
            Token::FUNCTION => fns.push(parent),
            Token::CLASS_MEMBERS => {
                if definition_site.is_member_function_def(compiler) {
                    fns.push(definition_site.get_last_child(compiler).unwrap());
                } else {
                    check_argument!(
                        definition_site.is_member_field_def(compiler),
                        "%s",
                        definition_site.to_string(compiler)
                    );
                    let value = definition_site.get_first_child(compiler);
                    if let Some(value) = value {
                        Self::add_value_function_nodes(compiler, &mut fns, value);
                    }
                }
            }
            Token::OBJECTLIT => {
                check_argument!(
                    definition_site.is_string_key(compiler)
                        || definition_site.is_member_function_def(compiler),
                    "%s",
                    definition_site.to_string(compiler)
                );
                Self::add_value_function_nodes(
                    compiler,
                    &mut fns,
                    definition_site.get_last_child(compiler).unwrap(),
                );
            }
            Token::ASSIGN => {
                // Only a candidate if the assign isn't consumed.
                let target = parent.get_first_child(compiler).unwrap();
                let value = parent.get_last_child(compiler).unwrap();
                if definition_site == target {
                    Self::add_value_function_nodes(compiler, &mut fns, value);
                }
            }
            Token::CONST | Token::LET | Token::VAR => {
                if definition_site.is_name(compiler) && definition_site.has_children(compiler) {
                    Self::add_value_function_nodes(
                        compiler,
                        &mut fns,
                        definition_site.get_first_child(compiler).unwrap(),
                    );
                }
            }
            _ => {}
        }
        fns
    }

    // port: OptimizeCalls.ReferenceMap#addValueFunctionNodes
    fn add_value_function_nodes(compiler: &AbstractCompiler, fns: &mut Vec<NodeId>, n: NodeId) {
        // TODO(johnlenz): add member definitions
        match n.get_token(compiler) {
            Token::CLASS => {
                let constructor_fn_def =
                    NodeUtil::get_es6_class_constructor_member_function_def(compiler, n);
                if let Some(constructor_fn_def) = constructor_fn_def {
                    fns.push(constructor_fn_def.get_only_child(compiler));
                }
            }
            Token::FUNCTION => fns.push(n),
            Token::HOOK => {
                Self::add_value_function_nodes(
                    compiler,
                    fns,
                    n.get_second_child(compiler).unwrap(),
                );
                Self::add_value_function_nodes(compiler, fns, n.get_last_child(compiler).unwrap());
            }
            Token::OR | Token::AND | Token::COALESCE => {
                Self::add_value_function_nodes(compiler, fns, n.get_first_child(compiler).unwrap());
                Self::add_value_function_nodes(compiler, fns, n.get_last_child(compiler).unwrap());
            }
            Token::CAST | Token::COMMA => {
                Self::add_value_function_nodes(compiler, fns, n.get_last_child(compiler).unwrap())
            }
            _ => {
                // do nothing.
            }
        }
    }

    /// Whether the provided node acts as the target function in a new or call or optional chain
    /// call expression including .call expressions. For example, returns true for 'x' in
    /// 'x?.call()'.
    // port: OptimizeCalls.ReferenceMap#isNormalOrOptChainCallOrNewTarget
    pub fn is_normal_or_opt_chain_call_or_new_target(
        compiler: &AbstractCompiler,
        n: NodeId,
    ) -> bool {
        Self::is_call_target(compiler, n)
            || Self::is_new_target(compiler, n)
            || Self::is_opt_chain_call_target(compiler, n)
    }

    /// Whether the provided node acts as the target function in a call expression including
    /// .call expressions. For example, returns true for 'x' in 'x.call()'.
    // port: OptimizeCalls.ReferenceMap#isCallTarget
    pub fn is_call_target(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        if parent.is_call(compiler) && n.is_first_child_of(compiler, Some(parent)) {
            return true;
        }

        let grand_parent = parent.get_parent(compiler).unwrap();
        parent.is_get_prop(compiler)
            && grand_parent.is_call(compiler)
            && parent.is_first_child_of(compiler, Some(grand_parent))
            && parent.get_string(compiler) == "call"
    }

    /// Whether the provided node acts as the target function in an optional chain call
    /// expression including .call expressions. For example, returns true for 'x' in 'x?.call()'.
    // port: OptimizeCalls.ReferenceMap#isOptChainCallTarget
    pub fn is_opt_chain_call_target(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        if parent.is_opt_chain_call(compiler) && n.is_first_child_of(compiler, Some(parent)) {
            return true;
        }

        let grand_parent = parent.get_parent(compiler).unwrap();
        parent.is_opt_chain_get_prop(compiler)
            && grand_parent.is_opt_chain_call(compiler)
            && parent.is_first_child_of(compiler, Some(grand_parent))
            && parent.get_string(compiler) == "call"
    }

    /// Whether the provided node acts as the target function in a new expression.
    // port: OptimizeCalls.ReferenceMap#isNewTarget
    pub fn is_new_target(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        parent.is_new(compiler) && parent.get_first_child(compiler) == Some(n)
    }

    /// Finds the associated call node for a node for which isNormalOrOptChainCallOrNewTarget
    /// returns true.
    // port: OptimizeCalls.ReferenceMap#getCallOrNewNodeForTarget
    pub fn get_call_or_new_node_for_target(compiler: &AbstractCompiler, n: NodeId) -> NodeId {
        let mut maybe_call = n.get_parent(compiler).unwrap();
        check_state!(
            n.is_first_child_of(compiler, Some(maybe_call)),
            "%s\n\n%s",
            maybe_call.to_string(compiler),
            n.to_string(compiler)
        );

        if NodeUtil::is_call_or_new(compiler, maybe_call) {
            // e.g. `n` input param is the `a` in `a()` or `a?.()`
            maybe_call
        } else {
            // e.g. `n` input param is the `a` in `a.b()` or `a?.b()`.
            let child = maybe_call;
            maybe_call = child.get_parent(compiler).unwrap();

            check_state!(
                NodeUtil::is_normal_or_opt_chain_get_prop(compiler, child),
                "%s",
                child.to_string(compiler)
            );
            check_state!(
                NodeUtil::is_normal_or_opt_chain_call(compiler, maybe_call),
                "%s",
                maybe_call.to_string(compiler)
            );
            check_state!(
                child.is_first_child_of(compiler, Some(maybe_call)),
                "%s\n\n%s",
                maybe_call.to_string(compiler),
                child.to_string(compiler)
            );

            maybe_call
        }
    }

    /// Finds the call argument node matching the first parameter of the called function for a
    /// node for which isNormalOrOptChainCallOrNewTarget returns true. Specifically, corrects for
    /// the additional argument provided to .call expressions.
    // port: OptimizeCalls.ReferenceMap#getFirstArgumentForCallOrNewOrDotCall
    pub fn get_first_argument_for_call_or_new_or_dot_call(
        compiler: &AbstractCompiler,
        n: NodeId,
    ) -> Option<NodeId> {
        Self::get_argument_for_call_or_new_or_dot_call(compiler, n, 0)
    }

    /// Finds the call argument node matching the parameter at the specified index of the called
    /// function for a node for which isNormalOrOptChainCallOrNewTarget returns true.
    /// Specifically, corrects for the additional argument provided to .call expressions.
    // port: OptimizeCalls.ReferenceMap#getArgumentForCallOrNewOrDotCall
    pub fn get_argument_for_call_or_new_or_dot_call(
        compiler: &AbstractCompiler,
        n: NodeId,
        index: i32,
    ) -> Option<NodeId> {
        let mut adjusted_index = index;
        let mut parent = n.get_parent(compiler).unwrap();
        if !(parent.is_call(compiler)
            || parent.is_opt_chain_call(compiler)
            || parent.is_new(compiler))
        {
            parent = parent.get_parent(compiler).unwrap();
            if NodeUtil::is_function_object_call(compiler, parent) {
                adjusted_index += 1;
            }
        }
        NodeUtil::get_argument_for_call_or_new(compiler, parent, adjusted_index)
    }

    // port: OptimizeCalls.ReferenceMap#isSimpleAssignmentTarget
    pub fn is_simple_assignment_target(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        // `ref = value;`
        parent.is_assign(compiler)
            && n.is_first_child_of(compiler, Some(parent))
            && parent
                .get_parent(compiler)
                .unwrap()
                .is_expr_result(compiler)
    }
}

struct ReferenceMapBuildingCallback<'a> {
    extern_props: IndexSet<String>,
    references: &'a mut ReferenceMap,
    global_scope: Option<ScopeId>,
    // The enclosing OptimizeCalls instance's `considerExterns` field (Java inner class).
    consider_externs: bool,
}

impl<'a> ReferenceMapBuildingCallback<'a> {
    // port: OptimizeCalls.ReferenceMapBuildingCallback#ReferenceMapBuildingCallback
    fn new(
        compiler: &AbstractCompiler,
        references: &'a mut ReferenceMap,
        consider_externs: bool,
    ) -> Self {
        Self {
            extern_props: OptimizeCalls::safe_set(compiler.get_extern_properties()),
            references,
            global_scope: None,
            consider_externs,
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#visitSuper
    fn visit_super(&mut self, compiler: &mut AbstractCompiler, super_node: NodeId) {
        // Determine whether this is a super() constructor call.
        // If it is, identify the super class and record this as a reference to that.
        let parent = super_node.get_parent(compiler).unwrap();
        if parent.is_call(compiler) && super_node.is_first_child_of(compiler, Some(parent)) {
            let enclosing_class = check_not_null!(NodeUtil::get_enclosing_class(compiler, parent));
            let extends_node = enclosing_class.get_second_child(compiler).unwrap();
            check_state!(
                !extends_node.is_empty(compiler),
                "super call appears in class without extends clause"
            );
            if extends_node.is_name(compiler) {
                let name = extends_node.get_string(compiler);
                self.maybe_add_name_reference(compiler, name, super_node);
            } else if extends_node.is_get_prop(compiler) {
                // NOTE: Theoretically we could also include an optional chain getprop here, but
                // A) it's a runtime error if the value ends up being undefined, so that's bad code
                // B) the author is indicating uncertainty, so we should be cautious.
                let name = extends_node.get_string(compiler);
                self.maybe_add_prop_reference(name, super_node);
            } // else we cannot tell what super() is referencing (e.g. `class extends getMixin() {`)
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#maybeAddNameReference
    fn maybe_add_name_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        name: JsString,
        n: NodeId,
    ) {
        // TODO(b/129503101): Why are we limiting ourselves to global names?
        let var = check_not_null!(self.global_scope).get_slot(compiler, name.clone());
        if let Some(var) = var
            && (self.consider_externs || !var.is_extern(compiler))
        {
            // As every name declaration is unique due to normalizations, it is only necessary to
            // build the global scope and ask it if it knows about a name as it can never be
            // shadowed.
            self.references.add_name_reference(name, n);
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#maybeAddPropReference
    fn maybe_add_prop_reference(&mut self, name: JsString, n: NodeId) {
        if self.consider_externs || !self.extern_props.contains(&name.to_string_lossy()) {
            self.references.add_prop_reference(name, n);
        }
    }
}

impl Callback for ReferenceMapBuildingCallback<'_> {
    // port: OptimizeCalls.ReferenceMapBuildingCallback#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _unused: Option<NodeId>) {
        match n.get_token(t) {
            Token::NAME => {
                let name = n.get_string(t);
                self.maybe_add_name_reference(t.get_compiler(), name, n);
            }
            Token::OPTCHAIN_GETPROP | Token::GETPROP => {
                let name = n.get_string(t);
                self.maybe_add_prop_reference(name, n);
            }
            Token::CALL => {
                // If we are using goog.reflect.objectProperty on this symbol, we will assume that
                // it gets referenced.
                let fn_name = n.get_first_child(t).unwrap();
                let compiler = t.get_compiler();
                if compiler
                    .get_coding_convention()
                    .is_property_rename_function(compiler, fn_name)
                {
                    let prop_name = NodeUtil::get_argument_for_call_or_new(compiler, n, 0);
                    if let Some(prop_name) = prop_name {
                        let name = prop_name.get_string(compiler);
                        self.maybe_add_prop_reference(name, n);
                    }
                }
            }

            Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF => {
                // ignore quoted keys.
                if !n.is_quoted_string_key(t) {
                    let name = n.get_string(t);
                    self.maybe_add_prop_reference(name, n);
                }
            }

            Token::SUPER => self.visit_super(t.get_compiler(), n),
            // Ignore quoted and computed keys.
            // TODO(johnlenz): support symbols.
            Token::COMPUTED_PROP
            | Token::COMPUTED_FIELD_DEF
            | Token::OPTCHAIN_GETELEM
            | Token::GETELEM
            // Don't worry about invisible accesses using object rest/spread. To be invoked there
            // would need to be downstream references that use the actual name. We'd see those.
            | Token::OBJECT_REST
            | Token::OBJECT_SPREAD => {}
            _ => {}
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            // Even when considering externs, we only care about top-level identifiers. Dummy
            // function parameters, for example, shouldn't be considered references.
            (self.consider_externs && t.in_global_scope()) || !n.is_from_externs(t)
        } else {
            true
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for ReferenceMapBuildingCallback<'_> {
    // port: OptimizeCalls.ReferenceMapBuildingCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_global_scope() {
            self.global_scope = Some(t.get_scope());
            self.references.global_scope = self.global_scope;
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}
