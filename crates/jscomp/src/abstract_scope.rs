/*
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/AbstractScope.java.

use crate::{
    abstract_compiler::AbstractCompiler, abstract_var::AbstractVar, node_util::NodeUtil,
    scope::ScopeId, scoped_name::ScopedName, typed_scope::TypedScope,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

/// Fields shared by syntactic and future typed scope handles.
#[derive(Debug)]
pub struct AbstractScopeData<V> {
    // Java LinkedHashMap (the fast hasher keeps the insertion order, D-025).
    pub(crate) vars: closure_rhino::fast_hash::IndexMap<JsString, V>,
    pub(crate) implicit_vars: BTreeMap<ImplicitVar, V>,
    pub(crate) root_node: NodeId,
}

impl<V> AbstractScopeData<V> {
    // port: AbstractScope#AbstractScope
    pub(crate) fn new(root_node: NodeId) -> Self {
        Self {
            vars: Default::default(),
            implicit_vars: BTreeMap::new(),
            root_node,
        }
    }
}

// The StaticScope#getTopmostScopeOfEventualDeclaration default, ported from Closure's
// Rhino-derived StaticScope (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "abstract_scope_rhino.rs"]
mod rhino;

/// Shared AbstractScope bodies; concrete syntactic and future typed handles supply only storage
/// access and the Java abstract methods.
pub trait AbstractScope: Copy + Eq {
    type Var: AbstractVar<Scope = Self>;
    /// Rust-only storage access: a plain reference into the syntactic arena, or a read guard into
    /// the shared typed arena (TypedScope views read it for closure-jstype).
    type DataRef<'a>: Deref<Target = AbstractScopeData<Self::Var>>;
    /// Rust-only mutable storage access, see `DataRef`.
    type DataMut<'a>: DerefMut<Target = AbstractScopeData<Self::Var>>;

    fn scope_data(self, compiler: &AbstractCompiler) -> Self::DataRef<'_>;
    fn scope_data_mut(self, compiler: &mut AbstractCompiler) -> Self::DataMut<'_>;

    // port: AbstractScope#getDepth
    fn get_depth(self, compiler: &AbstractCompiler) -> i32;

    // port: AbstractScope#getParent
    fn get_parent(self, compiler: &AbstractCompiler) -> Option<Self>;

    // port: AbstractScope#toString
    fn to_string(self, compiler: &AbstractCompiler) -> String {
        format!(
            "Scope@{}",
            self.scope_data(compiler).root_node.to_string(compiler)
        )
    }

    // port: AbstractScope#untyped
    fn untyped(self, _compiler: &AbstractCompiler) -> ScopeId {
        panic!("untyped() called, but not an untyped scope.")
    }

    // port: AbstractScope#typed
    fn typed(self, _compiler: &AbstractCompiler) -> TypedScope {
        panic!("typed() called, but not a typed scope.")
    }

    // port: AbstractScope#contains
    fn contains(self, compiler: &AbstractCompiler, other: Self) -> bool {
        let mut s = Some(other);
        while let Some(current) = s {
            if current == self {
                return true;
            }
            s = current.get_parent(compiler);
        }
        false
    }

    // port: AbstractScope#getRootNode
    fn get_root_node(self, compiler: &AbstractCompiler) -> NodeId {
        self.scope_data(compiler).root_node
    }

    // port: AbstractScope#getGlobalScope
    fn get_global_scope(self, compiler: &AbstractCompiler) -> Self {
        let mut result = self.this_scope();
        while let Some(parent) = result.get_parent(compiler) {
            result = parent;
        }
        result
    }

    // port: AbstractScope#getParentScope
    fn get_parent_scope(self, compiler: &AbstractCompiler) -> Option<Self> {
        self.get_parent(compiler)
    }

    // port: AbstractScope#makeImplicitVar
    // Java's result is nullable (TypedScope returns null for the global scope and `exports`).
    fn make_implicit_var(
        self,
        compiler: &mut AbstractCompiler,
        var: ImplicitVar,
    ) -> Option<Self::Var>;

    // port: AbstractScope#undeclare
    fn undeclare(self, compiler: &mut AbstractCompiler, var: Self::Var) {
        check_state!(var.get_scope(compiler) == Some(self));
        let declared = check_not_null!(
            self.scope_data(compiler)
                .vars
                .get(&var.get_name(compiler))
                .copied()
        );
        check_state!((&declared as &dyn ScopedName).equals(compiler, Some(&var)));
        self.undeclare_interal(compiler, var);
    }

    /// Rust-only (D-025): called before a declaration is added to or removed from this scope
    /// (see `SyntacticScopeCache`).
    fn note_mutation(self, _compiler: &mut AbstractCompiler) {}

    // port: AbstractScope#undeclareInteral
    fn undeclare_interal(self, compiler: &mut AbstractCompiler, var: Self::Var) {
        self.note_mutation(compiler);
        let name = var.get_name(compiler);
        self.scope_data_mut(compiler).vars.shift_remove(&name);
    }

    // port: AbstractScope#declareInternal
    fn declare_internal(self, compiler: &mut AbstractCompiler, name: JsString, var: Self::Var) {
        check_state!(
            self.has_own_slot(compiler, &name) || self.can_declare(compiler, &name),
            "Illegal shadow: %s",
            var.get_node(compiler)
                .map_or_else(|| "null".to_owned(), |n| n.to_string(compiler))
        );
        self.note_mutation(compiler);
        self.scope_data_mut(compiler).vars.insert(name, var);
    }

    // port: AbstractScope#clearVarsInternal
    fn clear_vars_internal(self, compiler: &mut AbstractCompiler) {
        self.note_mutation(compiler);
        if !self.scope_data(compiler).vars.is_empty() {
            self.scope_data_mut(compiler).vars.clear();
        }
    }

    // port: AbstractScope#hasOwnImplicitSlot
    fn has_own_implicit_slot(self, compiler: &AbstractCompiler, name: Option<ImplicitVar>) -> bool {
        name.is_some_and(|name| name.is_made_by_scope(compiler, self))
    }

    // port: AbstractScope#hasOwnSlot
    fn has_own_slot(self, compiler: &AbstractCompiler, name: &JsString) -> bool {
        self.scope_data(compiler).vars.contains_key(name)
            || self.has_own_implicit_slot(compiler, ImplicitVar::of(name))
    }

    // port: AbstractScope#hasSlot
    fn has_slot(self, compiler: &AbstractCompiler, name: &JsString) -> bool {
        let mut scope = Some(self.this_scope());
        while let Some(current) = scope {
            if current.has_own_slot(compiler, name) {
                return true;
            }
            scope = current.get_parent(compiler);
        }
        false
    }

    // port: AbstractScope#getOwnImplicitSlot
    fn get_own_implicit_slot(
        self,
        compiler: &mut AbstractCompiler,
        name: Option<ImplicitVar>,
    ) -> Option<Self::Var> {
        if !self.has_own_implicit_slot(compiler, name) {
            return None;
        }
        let name = check_not_null!(name);
        if let Some(var) = self.scope_data(compiler).implicit_vars.get(&name).copied() {
            return Some(var);
        }
        // computeIfAbsent stores nothing when makeImplicitVar returns null.
        let var = self.make_implicit_var(compiler, name)?;
        self.scope_data_mut(compiler)
            .implicit_vars
            .insert(name, var);
        Some(var)
    }

    // port: AbstractScope#getOwnSlot
    fn get_own_slot(self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<Self::Var> {
        if let Some(var) = self.scope_data(compiler).vars.get(name).copied() {
            return Some(var);
        }
        self.get_own_implicit_slot(compiler, ImplicitVar::of(name))
    }

    // port: AbstractScope#getSlot
    fn get_slot(self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<Self::Var> {
        self.get_var(compiler, name)
    }

    // port: AbstractScope#getVar
    fn get_var(self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<Self::Var> {
        abstract_scope_get_var(self, compiler, name)
    }

    /// Returns the scope that declares `name`, or will declare it once scope building is
    /// complete (Java's StaticScope default; TypedScope overrides it). The default body is in
    /// `abstract_scope_rhino.rs`.
    fn get_topmost_scope_of_eventual_declaration(
        self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
    ) -> Option<Self> {
        rhino::get_topmost_scope_of_eventual_declaration(self, compiler, name)
    }

    // port: AbstractScope#getArgumentsVar
    fn get_arguments_var(self, compiler: &mut AbstractCompiler) -> Option<Self::Var> {
        let mut scope = Some(self.this_scope());
        while let Some(current) = scope {
            if let Some(arguments) =
                current.get_own_implicit_slot(compiler, Some(ImplicitVar::ARGUMENTS))
            {
                return Some(arguments);
            }
            scope = current.get_parent(compiler);
        }
        None
    }

    // port: AbstractScope#canDeclare
    fn can_declare(self, compiler: &mut AbstractCompiler, name: &JsString) -> bool {
        !self.has_own_slot(compiler, name)
            && (!self.is_function_block_scope(compiler)
                || !check_not_null!(self.get_parent(compiler)).has_own_slot(compiler, name)
                || self.is_bleeding_function_name(compiler, name))
    }

    // port: AbstractScope#isBleedingFunctionName
    fn is_bleeding_function_name(self, compiler: &mut AbstractCompiler, name: &JsString) -> bool {
        self.get_var(compiler, name)
            .and_then(|var| var.get_node(compiler))
            .is_some_and(|n| check_not_null!(n.get_parent(compiler)).is_function(compiler))
    }

    // port: AbstractScope#getVarIterable
    fn get_var_iterable(self, compiler: &AbstractCompiler) -> Vec<Self::Var> {
        self.scope_data(compiler).vars.values().copied().collect()
    }

    // port: AbstractScope#getAllAccessibleVariables
    fn get_all_accessible_variables(self, compiler: &AbstractCompiler) -> Vec<Self::Var> {
        let mut accessible_vars = IndexMap::<_, _>::default();
        let mut s = Some(self.this_scope());
        while let Some(scope) = s {
            for var in scope.get_var_iterable(compiler) {
                accessible_vars.entry(var.get_name(compiler)).or_insert(var);
            }
            s = scope.get_parent(compiler);
        }
        accessible_vars.values().copied().collect()
    }

    // port: AbstractScope#getAllSymbols
    fn get_all_symbols(self, compiler: &AbstractCompiler) -> Vec<Self::Var> {
        self.scope_data(compiler).vars.values().copied().collect()
    }

    // port: AbstractScope#getVarCount
    fn get_var_count(self, compiler: &AbstractCompiler) -> i32 {
        i32::try_from(self.scope_data(compiler).vars.len()).unwrap()
    }

    // port: AbstractScope#isGlobal
    fn is_global(self, compiler: &AbstractCompiler) -> bool {
        self.get_parent(compiler).is_none()
    }

    // port: AbstractScope#isLocal
    fn is_local(self, compiler: &AbstractCompiler) -> bool {
        self.get_parent(compiler).is_some()
    }

    // port: AbstractScope#isBlockScope
    fn is_block_scope(self, compiler: &AbstractCompiler) -> bool {
        NodeUtil::creates_block_scope(compiler, self.scope_data(compiler).root_node)
    }

    // port: AbstractScope#isStaticBlockScope
    fn is_static_block_scope(self, compiler: &AbstractCompiler) -> bool {
        NodeUtil::is_class_static_block(compiler, self.get_root_node(compiler))
    }

    // port: AbstractScope#isFunctionBlockScope
    fn is_function_block_scope(self, compiler: &AbstractCompiler) -> bool {
        NodeUtil::is_function_block(compiler, self.get_root_node(compiler))
    }

    // port: AbstractScope#isFunctionScope
    fn is_function_scope(self, compiler: &AbstractCompiler) -> bool {
        self.get_root_node(compiler).is_function(compiler)
    }

    // port: AbstractScope#isModuleScope
    fn is_module_scope(self, compiler: &AbstractCompiler) -> bool {
        self.get_root_node(compiler).is_module_body(compiler)
    }

    // port: AbstractScope#isMemberFieldDefScope
    fn is_member_field_def_scope(self, compiler: &AbstractCompiler) -> bool {
        self.get_root_node(compiler).is_member_field_def(compiler)
    }

    // port: AbstractScope#isComputedFieldDefRhsScope
    fn is_computed_field_def_rhs_scope(self, compiler: &AbstractCompiler) -> bool {
        self.get_root_node(compiler).is_computed_field_def(compiler)
    }

    // port: AbstractScope#isCatchScope
    fn is_catch_scope(self, compiler: &AbstractCompiler) -> bool {
        let root = self.get_root_node(compiler);
        root.is_block(compiler)
            && root.has_one_child(compiler)
            && check_not_null!(root.get_first_child(compiler)).is_catch(compiler)
    }

    // port: AbstractScope#isCfgRootScope
    fn is_cfg_root_scope(self, compiler: &AbstractCompiler) -> bool {
        NodeUtil::is_valid_cfg_root(compiler, self.scope_data(compiler).root_node)
    }

    // port: AbstractScope#isHoistScope
    fn is_hoist_scope(self, compiler: &AbstractCompiler) -> bool {
        self.is_function_scope(compiler)
            || self.is_function_block_scope(compiler)
            || self.is_global(compiler)
            || self.is_module_scope(compiler)
            || self.is_static_block_scope(compiler)
    }

    // port: AbstractScope#getClosestHoistScope
    fn get_closest_hoist_scope(self, compiler: &AbstractCompiler) -> Option<Self> {
        let mut current = Some(self.this_scope());
        while let Some(scope) = current {
            if scope.is_hoist_scope(compiler) {
                return Some(scope);
            }
            current = scope.get_parent(compiler);
        }
        None
    }

    // port: AbstractScope#getClosestCfgRootScope
    fn get_closest_cfg_root_scope(self, compiler: &AbstractCompiler) -> Self {
        let mut current = self.this_scope();
        while !current.is_cfg_root_scope(compiler) {
            current = check_not_null!(current.get_parent(compiler));
        }
        current
    }

    // port: AbstractScope#getClosestContainerScope
    fn get_closest_container_scope(self, compiler: &AbstractCompiler) -> Self {
        let mut scope = check_not_null!(self.get_closest_hoist_scope(compiler));
        if scope.is_function_block_scope(compiler) {
            scope = check_not_null!(scope.get_parent(compiler));
            check_state!(!scope.is_block_scope(compiler), &scope.to_string(compiler));
        }
        scope
    }

    // port: AbstractScope#thisScope
    fn this_scope(self) -> Self {
        self
    }

    // port: AbstractScope#checkChildScope
    fn check_child_scope(self, compiler: &AbstractCompiler, parent: Self) {
        let root_node = self.get_root_node(compiler);
        check_argument!(
            NodeUtil::creates_scope(compiler, root_node),
            &root_node.to_string(compiler)
        );
        check_argument!(
            root_node != parent.get_root_node(compiler),
            "rootNode should not be the parent's root node: %s",
            root_node.to_string(compiler)
        );
    }

    // port: AbstractScope#checkRootScope
    fn check_root_scope(self, compiler: &AbstractCompiler) {
        let root_node = self.get_root_node(compiler);
        check_argument!(
            NodeUtil::creates_scope(compiler, root_node)
                || root_node.is_script(compiler)
                || root_node.is_root(compiler),
            &root_node.to_string(compiler)
        );
    }

    // port: AbstractScope#getCommonParent
    fn get_common_parent(self, compiler: &AbstractCompiler, other: Self) -> Self {
        let mut left = Some(self.this_scope());
        let mut right = Some(other);
        while let (Some(left_scope), Some(right_scope)) = (left, right) {
            if left_scope == right_scope {
                break;
            }
            let left_depth = left_scope.get_depth(compiler);
            let right_depth = right_scope.get_depth(compiler);
            if left_depth >= right_depth {
                left = left_scope.get_parent(compiler);
            }
            if left_depth <= right_depth {
                right = right_scope.get_parent(compiler);
            }
        }
        check_state!(left.is_some() && left == right);
        check_not_null!(left)
    }

    // port: AbstractScope#hasSameContainerScope
    fn has_same_container_scope(self, compiler: &AbstractCompiler, other: Self) -> bool {
        self == other
            || self.get_closest_container_scope(compiler)
                == other.get_closest_container_scope(compiler)
    }

    // port: AbstractScope#getScopeOfThis
    fn get_scope_of_this(self, compiler: &AbstractCompiler) -> Self {
        let mut scope = self.get_closest_container_scope(compiler);
        while !scope.is_global(compiler)
            && !NodeUtil::is_non_arrow_function(compiler, scope.get_root_node(compiler))
        {
            scope =
                check_not_null!(scope.get_parent(compiler)).get_closest_container_scope(compiler);
        }
        scope
    }
}

// port: AbstractScope#getVar
/// The AbstractScope body, callable as `super.getVar` by an overriding scope (TypedScope#getVar).
pub(crate) fn abstract_scope_get_var<S: AbstractScope>(
    this: S,
    compiler: &mut AbstractCompiler,
    name: &JsString,
) -> Option<S::Var> {
    let mut scope = Some(this.this_scope());
    while let Some(current) = scope {
        if let Some(var) = current.get_own_slot(compiler, name) {
            return Some(var);
        }
        scope = current.get_parent(compiler);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImplicitVar {
    ARGUMENTS,
    EXPORTS,
    SUPER,
    THIS,
}

// port: AbstractScope.ImplicitVar#isMadeByScope
/// Rust-only twin of `ImplicitVar::is_made_by_scope` on the scope's root node, for the
/// closure-jstype scope views (`ScopeView`, `TypedScopeView`), which hold the AST but no compiler.
/// The scope predicates are inlined: isModuleScope, isStaticBlockScope, isMemberFieldDefScope and
/// isComputedFieldDefRhsScope read only the root node.
pub(crate) fn view_is_made_by_scope(ast: &Ast, root: NodeId, var: ImplicitVar) -> bool {
    match var {
        ImplicitVar::EXPORTS => {
            root.is_module_body(ast)
                && check_not_null!(root.get_parent(ast)).get_boolean_prop(ast, NodeId::GOOG_MODULE)
        }
        ImplicitVar::SUPER | ImplicitVar::THIS => {
            NodeUtil::is_class_static_block(ast, root)
                || NodeUtil::is_non_arrow_function(ast, root)
                || root.is_member_field_def(ast)
                || root.is_computed_field_def(ast)
        }
        ImplicitVar::ARGUMENTS => NodeUtil::is_non_arrow_function(ast, root),
    }
}

impl ImplicitVar {
    // port: AbstractScope.ImplicitVar#ImplicitVar
    pub const fn name(self) -> &'static str {
        match self {
            Self::ARGUMENTS => "arguments",
            Self::EXPORTS => "exports",
            Self::SUPER => "super",
            Self::THIS => "this",
        }
    }

    /// Rust-only: `name` as a JS string, made once per process (implicit vars are declared in
    /// every function scope).
    pub fn js_name(self) -> JsString {
        static NAMES: std::sync::OnceLock<[JsString; 4]> = std::sync::OnceLock::new();
        let names = NAMES.get_or_init(|| {
            [
                ImplicitVar::ARGUMENTS,
                ImplicitVar::EXPORTS,
                ImplicitVar::SUPER,
                ImplicitVar::THIS,
            ]
            .map(|var| JsString::from(var.name()))
        });
        names[self as usize].clone()
    }

    // port: AbstractScope.ImplicitVar#isMadeByScope
    pub fn is_made_by_scope<S: AbstractScope>(self, compiler: &AbstractCompiler, scope: S) -> bool {
        match self {
            Self::EXPORTS => {
                scope.is_module_scope(compiler)
                    && check_not_null!(scope.get_root_node(compiler).get_parent(compiler))
                        .get_boolean_prop(compiler, NodeId::GOOG_MODULE)
            }
            Self::SUPER | Self::THIS => {
                scope.is_static_block_scope(compiler)
                    || NodeUtil::is_non_arrow_function(compiler, scope.get_root_node(compiler))
                    || scope.is_member_field_def_scope(compiler)
                    || scope.is_computed_field_def_rhs_scope(compiler)
            }
            Self::ARGUMENTS => {
                NodeUtil::is_non_arrow_function(compiler, scope.get_root_node(compiler))
            }
        }
    }

    // port: AbstractScope.ImplicitVar#of
    pub fn of(name: &JsString) -> Option<Self> {
        // Rust-only fast path (D-025): the four names have distinct lengths.
        if !matches!(name.length(), 4 | 5 | 7 | 9) {
            return None;
        }
        if name == "arguments" {
            Some(Self::ARGUMENTS)
        } else if name == "super" {
            Some(Self::SUPER)
        } else if name == "this" {
            Some(Self::THIS)
        } else if name == "exports" {
            Some(Self::EXPORTS)
        } else {
            None
        }
    }
}

/// Rust-only: Java's `AbstractScope<?, ?>` as NodeTraversal and ScopeCreator pass it around
/// (a syntactic `Scope` or a `TypedScope`); each method dispatches to the scope's own body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbstractScopeHandle {
    Untyped(ScopeId),
    Typed(TypedScope),
}

impl From<ScopeId> for AbstractScopeHandle {
    fn from(scope: ScopeId) -> Self {
        Self::Untyped(scope)
    }
}

impl From<TypedScope> for AbstractScopeHandle {
    fn from(scope: TypedScope) -> Self {
        Self::Typed(scope)
    }
}

impl AbstractScopeHandle {
    // port: AbstractScope#untyped
    pub fn untyped(self, compiler: &AbstractCompiler) -> ScopeId {
        match self {
            Self::Untyped(scope) => AbstractScope::untyped(scope, compiler),
            Self::Typed(scope) => AbstractScope::untyped(scope, compiler),
        }
    }

    // port: AbstractScope#typed
    pub fn typed(self, compiler: &AbstractCompiler) -> TypedScope {
        match self {
            Self::Untyped(scope) => AbstractScope::typed(scope, compiler),
            Self::Typed(scope) => AbstractScope::typed(scope, compiler),
        }
    }

    // port: AbstractScope#getRootNode
    pub fn get_root_node(self, compiler: &AbstractCompiler) -> NodeId {
        match self {
            Self::Untyped(scope) => AbstractScope::get_root_node(scope, compiler),
            Self::Typed(scope) => AbstractScope::get_root_node(scope, compiler),
        }
    }

    // port: AbstractScope#getParent
    pub fn get_parent(self, compiler: &AbstractCompiler) -> Option<Self> {
        match self {
            Self::Untyped(scope) => AbstractScope::get_parent(scope, compiler).map(Self::Untyped),
            Self::Typed(scope) => AbstractScope::get_parent(scope, compiler).map(Self::Typed),
        }
    }

    // port: AbstractScope#isGlobal
    pub fn is_global(self, compiler: &AbstractCompiler) -> bool {
        match self {
            Self::Untyped(scope) => AbstractScope::is_global(scope, compiler),
            Self::Typed(scope) => AbstractScope::is_global(scope, compiler),
        }
    }

    // port: AbstractScope#isModuleScope
    pub fn is_module_scope(self, compiler: &AbstractCompiler) -> bool {
        match self {
            Self::Untyped(scope) => AbstractScope::is_module_scope(scope, compiler),
            Self::Typed(scope) => AbstractScope::is_module_scope(scope, compiler),
        }
    }

    // port: AbstractScope#toString
    pub fn to_string(self, compiler: &AbstractCompiler) -> String {
        match self {
            Self::Untyped(scope) => AbstractScope::to_string(scope, compiler),
            Self::Typed(scope) => AbstractScope::to_string(scope, compiler),
        }
    }
}
