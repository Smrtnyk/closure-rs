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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/OptimizeCalls.java.

//! Port of `OptimizeCalls.java`: a root pass that is a container for other passes that should
//! run with a single call graph (`OptimizeParameters`, `OptimizeReturns`, `DevirtualizeMethods`,
//! `PureFunctionIdentifier`).

use crate::abstract_compiler::AbstractCompiler;
use crate::coding_convention::CodingConvention;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::NodeTraversal;
use crate::node_util::NodeUtil;
use crate::parallel_parse::parallel_thread_count;
use crate::scope::ScopeId;
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

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
    #[allow(dead_code)] // ReferenceMapBuildingCallback builds a set of JS strings instead.
    fn safe_set(set: Option<&IndexSet<String>>) -> IndexSet<String> {
        match set {
            Some(set) => set.clone(),
            None => IndexSet::<_>::default(),
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
        ReferenceMapBuilder::collect(
            compiler,
            externs,
            root,
            self.consider_externs,
            &mut references,
        );
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
        let mut result: IndexMap<NodeId, Vec<NodeId>> = IndexMap::<_, _>::default();
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

        // Java dereferences grandParent only once parent.isGetProp() holds: a detached
        // parent (no grandparent) answers false.
        let grand_parent = parent.get_parent(compiler);
        parent.is_get_prop(compiler)
            && grand_parent.unwrap().is_call(compiler)
            && parent.is_first_child_of(compiler, grand_parent)
            && parent.get_string_ref(compiler) == "call"
    }

    /// Whether the provided node acts as the target function in an optional chain call
    /// expression including .call expressions. For example, returns true for 'x' in 'x?.call()'.
    // port: OptimizeCalls.ReferenceMap#isOptChainCallTarget
    pub fn is_opt_chain_call_target(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let parent = n.get_parent(compiler).unwrap();
        if parent.is_opt_chain_call(compiler) && n.is_first_child_of(compiler, Some(parent)) {
            return true;
        }

        // As isCallTarget: grandParent is dereferenced only after parent.isOptChainGetProp().
        let grand_parent = parent.get_parent(compiler);
        parent.is_opt_chain_get_prop(compiler)
            && grand_parent.unwrap().is_opt_chain_call(compiler)
            && parent.is_first_child_of(compiler, grand_parent)
            && parent.get_string_ref(compiler) == "call"
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

/// Rust-only (performance divergence): direct read-only `&Ast` walker for
/// `ReferenceMapBuildingCallback`. Only builds `global_scope` (reusing `SyntacticScopeCache`,
/// avoiding creating local scopes across the whole AST) and fans out independent script chunks
/// across worker threads on large multi-script inputs while merging in exact AST order.
struct ReferenceMapBuilder<'a> {
    ast: &'a Ast,
    coding_convention: &'a dyn CodingConvention,
    extern_props: &'a IndexSet<JsString>,
    global_names: &'a IndexSet<JsString>,
    global_name_bloom: [u64; 64],
    consider_externs: bool,
    current_script: Option<NodeId>,
    current_node: Option<NodeId>,
    names: IndexMap<JsString, Vec<NodeId>>,
    props: IndexMap<JsString, Vec<NodeId>>,
}

struct WalkFailure {
    script: Option<NodeId>,
    node: Option<NodeId>,
    payload: Box<dyn Any + Send>,
}

impl ReferenceMapBuilder<'_> {
    // port: OptimizeCalls.ReferenceMapBuildingCallback#ReferenceMapBuildingCallback
    // port: OptimizeCalls.ReferenceMapBuildingCallback#enterScope
    // port: OptimizeCalls.ReferenceMapBuildingCallback#exitScope
    fn collect(
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
        consider_externs: bool,
        references: &mut ReferenceMap,
    ) {
        let scope_root = check_not_null!(externs.get_parent(compiler));
        check_state!(root.get_parent(compiler) == Some(scope_root));

        let global_scope = SyntacticScopeCreator::new().create_scope(compiler, scope_root, None);
        references.global_scope = Some(global_scope);

        let mut global_names = IndexSet::<JsString>::default();
        let mut global_name_bloom = [0u64; 64];
        for var in global_scope.get_var_iterable(compiler) {
            if consider_externs || !var.is_extern(compiler) {
                let name = var.get_name(compiler);
                let h = name.hash_code() as usize;
                global_name_bloom[(h >> 6) & 63] |= 1u64 << (h & 63);
                global_names.insert(name);
            }
        }
        let extern_props = compiler
            .get_extern_property_set_js()
            .cloned()
            .unwrap_or_default();

        // Collect ordered top-level units (scripts under `externs` and `root`, or top-level
        // statements of a single large non-module script) so multi-script and single-bundle
        // compilations scan chunks in parallel while merging in exact AST order.
        let mut units: Vec<(NodeId, Option<NodeId>)> = Vec::new();
        for (is_externs, tree) in [(true, externs), (false, root)] {
            if tree.is_root(compiler) {
                let mut child = tree.get_first_child(compiler);
                while let Some(c) = child {
                    if !c.is_script(compiler) || consider_externs || !c.is_from_externs(compiler) {
                        if !is_externs
                            && c.is_script(compiler)
                            && let Some(first) = c.get_first_child(compiler)
                            && !first.is_module_body(compiler)
                            && c.get_next(compiler).is_none()
                            && units.is_empty()
                        {
                            let mut st = Some(first);
                            while let Some(s) = st {
                                units.push((s, Some(c)));
                                st = s.get_next(compiler);
                            }
                        } else {
                            units.push((c, None));
                        }
                    }
                    child = c.get_next(compiler);
                }
            } else if !tree.is_script(compiler)
                || consider_externs
                || !tree.is_from_externs(compiler)
            {
                units.push((tree, None));
            }
        }

        let threads = if compiler.ast.node_count() >= 20_000 {
            parallel_thread_count(units.len(), 2, 8)
        } else {
            1
        };

        if threads <= 1 {
            let err = {
                let ast: &Ast = &compiler.ast;
                let coding_convention = compiler.get_coding_convention();
                let mut builder = ReferenceMapBuilder {
                    ast,
                    coding_convention,
                    extern_props: &extern_props,
                    global_names: &global_names,
                    global_name_bloom,
                    consider_externs,
                    current_script: None,
                    current_node: None,
                    names: std::mem::take(&mut references.names),
                    props: std::mem::take(&mut references.props),
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    for &(unit, script) in &units {
                        if script.is_some() {
                            builder.current_script = script;
                        }
                        builder.walk_node(unit, true);
                    }
                }));
                references.names = builder.names;
                references.props = builder.props;
                result.err().map(|payload| WalkFailure {
                    script: builder.current_script,
                    node: builder.current_node,
                    payload,
                })
            };
            if let Some(fail) = err {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    fail.script,
                    fail.node,
                    fail.payload,
                );
            }
            return;
        }

        let num_buckets = (threads * 2).min(units.len());
        let bucket_size = units.len().div_ceil(num_buckets);
        let slices: Vec<&[(NodeId, Option<NodeId>)]> = units.chunks(bucket_size).collect();
        type BucketMaps = (
            IndexMap<JsString, Vec<NodeId>>,
            IndexMap<JsString, Vec<NodeId>>,
        );
        let mut results: Vec<Option<Result<BucketMaps, WalkFailure>>> =
            (0..slices.len()).map(|_| None).collect();

        {
            let ast: &Ast = &compiler.ast;
            let coding_convention = compiler.get_coding_convention();
            let extern_props_ref = &*extern_props;
            let global_names_ref = &global_names;
            let worker_batches = crate::parallel_parse::run_scoped_indexed(
                "closure-optcalls",
                threads,
                &slices,
                |_idx, slice| {
                    let mut builder = ReferenceMapBuilder {
                        ast,
                        coding_convention,
                        extern_props: extern_props_ref,
                        global_names: global_names_ref,
                        global_name_bloom,
                        consider_externs,
                        current_script: None,
                        current_node: None,
                        names: IndexMap::default(),
                        props: IndexMap::default(),
                    };
                    let res = catch_unwind(AssertUnwindSafe(|| {
                        for &(unit, script) in *slice {
                            if script.is_some() {
                                builder.current_script = script;
                            }
                            builder.walk_node(unit, true);
                        }
                    }));
                    let out = match res {
                        Ok(()) => Ok((builder.names, builder.props)),
                        Err(payload) => Err(WalkFailure {
                            script: builder.current_script,
                            node: builder.current_node,
                            payload,
                        }),
                    };
                    Some(out)
                },
            );
            for batch in worker_batches {
                for (idx, out) in batch {
                    results[idx] = Some(out);
                }
            }
        }

        for slot in results {
            match slot.expect("every bucket processed") {
                Ok((names, props)) => {
                    for (k, mut v) in names {
                        match references.names.entry(k) {
                            indexmap::map::Entry::Vacant(e) => {
                                e.insert(v);
                            }
                            indexmap::map::Entry::Occupied(mut e) => {
                                e.get_mut().append(&mut v);
                            }
                        }
                    }
                    for (k, mut v) in props {
                        match references.props.entry(k) {
                            indexmap::map::Entry::Vacant(e) => {
                                e.insert(v);
                            }
                            indexmap::map::Entry::Occupied(mut e) => {
                                e.get_mut().append(&mut v);
                            }
                        }
                    }
                }
                Err(fail) => {
                    NodeTraversal::throw_unexpected_exception_at(
                        compiler,
                        fail.script,
                        fail.node,
                        fail.payload,
                    );
                }
            }
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#shouldTraverse
    fn walk_node(&mut self, n: NodeId, in_global_scope: bool) {
        let ast = self.ast;
        let token = n.get_token(ast);
        match token {
            Token::SCRIPT => {
                if !((self.consider_externs && in_global_scope) || !n.is_from_externs(ast)) {
                    return;
                }
                let prev_script = self.current_script;
                self.current_script = Some(n);
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    self.walk_node(c, in_global_scope);
                    child = c.get_next(ast);
                }
                self.current_script = prev_script;
                return;
            }
            Token::CLASS => {
                let class_name = n.get_first_child(ast).unwrap();
                let extends_clause = class_name.get_next(ast).unwrap();
                let body = extends_clause.get_next(ast).unwrap();
                self.walk_node(extends_clause, in_global_scope);
                self.walk_node(class_name, false);
                self.walk_node(body, false);
                return;
            }
            Token::CLASS_MEMBERS => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if c.is_computed_prop(ast) || c.is_computed_field_def(ast) {
                        self.walk_node(c.get_first_child(ast).unwrap(), false);
                    }
                    child = c.get_next(ast);
                }
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    match c.get_token(ast) {
                        Token::COMPUTED_PROP => {
                            self.walk_node(c.get_last_child(ast).unwrap(), false);
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            if c.has_two_children(ast) {
                                self.walk_node(c.get_last_child(ast).unwrap(), false);
                            }
                        }
                        _ => self.walk_node(c, false),
                    }
                    child = c.get_next(ast);
                }
                return;
            }
            _ => {
                let child_global = in_global_scope
                    && !matches!(
                        token,
                        Token::FUNCTION
                            | Token::MODULE_BODY
                            | Token::BLOCK
                            | Token::FOR
                            | Token::FOR_IN
                            | Token::FOR_OF
                            | Token::FOR_AWAIT_OF
                            | Token::SWITCH_BODY
                    );
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    self.walk_node(c, child_global);
                    child = c.get_next(ast);
                }
            }
        }
        self.visit(n, token);
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#visitSuper
    fn visit_super(&mut self, super_node: NodeId) {
        let ast = self.ast;
        // Determine whether this is a super() constructor call.
        // If it is, identify the super class and record this as a reference to that.
        let parent = super_node.get_parent(ast).unwrap();
        if parent.is_call(ast) && super_node.is_first_child_of(ast, Some(parent)) {
            let enclosing_class = check_not_null!(NodeUtil::get_enclosing_class(ast, parent));
            let extends_node = enclosing_class.get_second_child(ast).unwrap();
            check_state!(
                !extends_node.is_empty(ast),
                "super call appears in class without extends clause"
            );
            if extends_node.is_name(ast) {
                self.maybe_add_name_reference(extends_node, super_node);
            } else if extends_node.is_get_prop(ast) {
                // NOTE: Theoretically we could also include an optional chain getprop here, but
                // A) it's a runtime error if the value ends up being undefined, so that's bad code
                // B) the author is indicating uncertainty, so we should be cautious.
                self.maybe_add_prop_reference(extends_node.get_string_ref(ast), super_node);
            } // else we cannot tell what super() is referencing (e.g. `class extends getMixin() {`)
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#maybeAddNameReference
    #[inline]
    fn maybe_add_name_reference(&mut self, name_node: NodeId, n: NodeId) {
        let name = name_node.get_string_ref(self.ast);
        let h = name.hash_code() as usize;
        if (self.global_name_bloom[(h >> 6) & 63] & (1u64 << (h & 63))) == 0 {
            return;
        }
        if self.global_names.contains(name) {
            ReferenceMap::add_reference(&mut self.names, name.clone(), n);
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#maybeAddPropReference
    #[inline]
    fn maybe_add_prop_reference(&mut self, name: &JsString, n: NodeId) {
        if self.consider_externs || !self.extern_props.contains(name) {
            ReferenceMap::add_reference(&mut self.props, name.clone(), n);
        }
    }

    // port: OptimizeCalls.ReferenceMapBuildingCallback#visit
    #[inline]
    fn visit(&mut self, n: NodeId, token: Token) {
        let ast = self.ast;
        match token {
            Token::NAME => {
                self.maybe_add_name_reference(n, n);
            }
            Token::OPTCHAIN_GETPROP | Token::GETPROP => {
                self.maybe_add_prop_reference(n.get_string_ref(ast), n);
            }
            Token::CALL => {
                // If we are using goog.reflect.objectProperty on this symbol, we will assume that
                // it gets referenced.
                let fn_name = n.get_first_child(ast).unwrap();
                if self
                    .coding_convention
                    .is_property_rename_function(ast, fn_name)
                {
                    let prop_name = NodeUtil::get_argument_for_call_or_new(ast, n, 0);
                    if let Some(prop_name) = prop_name {
                        self.maybe_add_prop_reference(prop_name.get_string_ref(ast), n);
                    }
                }
            }

            Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
                // ignore quoted keys.
                if !n.is_quoted_string_key(ast) =>
            {
                self.maybe_add_prop_reference(n.get_string_ref(ast), n);
            }

            Token::SUPER => {
                self.current_node = Some(n);
                self.visit_super(n);
            }
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
}
