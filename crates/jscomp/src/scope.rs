/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AbstractScope.java,
//   src/com/google/javascript/jscomp/AbstractVar.java, src/com/google/javascript/jscomp/Scope.java.

use crate::typed_scope::{TypedArenaMut, TypedArenaRef};
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::{AbstractScope, AbstractScopeData, ImplicitVar},
    abstract_var::AbstractVarData,
    compiler_input::CompilerInput,
    var::VarId,
};
use closure_jstype::{
    JSTypeRegistry, TypeId, static_typed_ref::StaticTypedRef, static_typed_scope::StaticTypedScope,
    static_typed_slot::StaticTypedSlot,
};
use closure_rhino::{check_argument, check_state, js_string::JsString, node::NodeId};
use closure_rhino::{jsdoc_info::JSDocInfo, node::Ast, static_source_file::StaticSourceFile};
use std::{
    num::NonZeroU32,
    sync::{Arc, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeId(pub(crate) NonZeroU32);

pub type Scope = ScopeId;

#[derive(Debug)]
pub(crate) struct ScopeData {
    pub(crate) abstract_scope: AbstractScopeData<VarId>,
    pub(crate) parent: Option<ScopeId>,
    pub(crate) depth: i32,
    /// Rust-only: the canonical closure-jstype view (Java passes the Scope itself as a
    /// StaticScope), created on first use; see `ScopeView`.
    pub(crate) view: OnceLock<&'static Arc<ScopeView>>,
}

/// Rust-only owner for Java scope and variable object identities; slots are never removed.
/// Shared (`Arc<RwLock<..>>` in the compiler) like the typed arena, so that the canonical
/// closure-jstype views can read it while the registry is borrowed from the compiler.
#[derive(Debug, Default)]
pub struct ScopeArena {
    pub(crate) scopes: Vec<ScopeData>,
    pub(crate) vars: Vec<AbstractVarData<ScopeId>>,
    /// Rust-only: the canonical view of each var (parallel to `vars`); see `VarView`.
    pub(crate) var_views: Vec<OnceLock<&'static VarView>>,
}

/// Rust-only, not in Java (D-025): the fields of scopes and vars that never change after
/// construction (Java `final` fields), copied out of the shared `ScopeArena` so that the
/// compiler's hot readers (`getRootNode`, `getParent`, `getDepth`, `Var#getName`, `getNode`,
/// `getScope`, ...) skip the arena's lock. Index-aligned with `ScopeArena::scopes` / `vars`; a var
/// that a scope view created (implicit vars) may be missing until the next `VarId::new`
/// resynchronises, and is then read from the arena.
#[derive(Debug, Default)]
pub(crate) struct ScopeMirror {
    pub(crate) scopes: Vec<ScopeMeta>,
    pub(crate) vars: Vec<VarMeta>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScopeMeta {
    pub(crate) root_node: NodeId,
    pub(crate) parent: Option<ScopeId>,
    pub(crate) depth: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct VarMeta {
    pub(crate) name: JsString,
    pub(crate) name_node: Option<NodeId>,
    pub(crate) input: Option<CompilerInput>,
    pub(crate) index: i32,
    pub(crate) scope: Option<ScopeId>,
}

impl VarMeta {
    pub(crate) fn of(data: &AbstractVarData<ScopeId>) -> Self {
        Self {
            name: data.name.clone(),
            name_node: data.name_node,
            input: data.input.clone(),
            index: data.index,
            scope: data.scope,
        }
    }
}

impl ScopeMirror {
    /// Appends the vars the arena has and the mirror lacks.
    pub(crate) fn sync_vars(&mut self, arena: &ScopeArena) {
        for data in &arena.vars[self.vars.len()..] {
            self.vars.push(VarMeta::of(data));
        }
    }
}

impl ScopeArena {
    /// Rust-only: the compiler's shared arena.
    pub(crate) fn shared() -> Arc<RwLock<ScopeArena>> {
        Arc::new(RwLock::new(ScopeArena::default()))
    }

    pub(crate) fn read(compiler: &AbstractCompiler) -> RwLockReadGuard<'_, ScopeArena> {
        compiler
            .scope_arena
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn write(compiler: &AbstractCompiler) -> RwLockWriteGuard<'_, ScopeArena> {
        compiler
            .scope_arena
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl ScopeId {
    // port: Scope#createGlobalScope
    pub fn create_global_scope(compiler: &mut AbstractCompiler, root_node: NodeId) -> Self {
        Self::new(compiler, root_node)
    }

    // port: Scope#createChildScope
    pub fn create_child_scope(
        compiler: &mut AbstractCompiler,
        parent: Self,
        root_node: NodeId,
    ) -> Self {
        Self::new_with_parent(compiler, parent, root_node)
    }

    // port: Scope#Scope(Scope, Node)
    fn new_with_parent(compiler: &mut AbstractCompiler, parent: Self, root_node: NodeId) -> Self {
        let scope = Self::allocate(compiler, root_node);
        scope.check_child_scope(compiler, parent);
        let depth = parent.get_depth(compiler).wrapping_add(1);
        let mut arena = ScopeArena::write(compiler);
        let data = &mut arena.scopes[scope.index()];
        data.parent = Some(parent);
        data.depth = depth;
        drop(arena);
        let meta = &mut compiler.scope_mirror.scopes[scope.index()];
        meta.parent = Some(parent);
        meta.depth = depth;
        scope
    }

    // port: Scope#Scope(Node)
    fn new(compiler: &mut AbstractCompiler, root_node: NodeId) -> Self {
        let scope = Self::allocate(compiler, root_node);
        scope.check_root_scope(compiler);
        let mut arena = ScopeArena::write(compiler);
        let data = &mut arena.scopes[scope.index()];
        data.parent = None;
        data.depth = 0;
        drop(arena);
        scope
    }

    // port: Scope#untyped
    pub fn untyped(self, _compiler: &AbstractCompiler) -> Self {
        self
    }

    // port: Scope#getDepth
    pub fn get_depth(self, compiler: &AbstractCompiler) -> i32 {
        compiler.scope_mirror.scopes[self.index()].depth
    }

    // port: Scope#getParent
    pub fn get_parent(self, compiler: &AbstractCompiler) -> Option<Self> {
        compiler.scope_mirror.scopes[self.index()].parent
    }

    // port: Scope#declare
    pub fn declare(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        name_node: impl Into<Option<NodeId>>,
        input: Option<CompilerInput>,
    ) -> VarId {
        let name = name.into();
        check_argument!(!name.is_empty());
        if ImplicitVar::of(&name).is_none() {
            // Rust-only fast path (D-025): with no implicit slot for `name`, getOwnSlot,
            // hasOwnSlot and canDeclare only read declared vars; read them under one lock.
            let (declared, count) = {
                let arena = ScopeArena::read(compiler);
                let vars = &arena.scopes[self.index()].abstract_scope.vars;
                (vars.contains_key(&name), vars.len())
            };
            check_state!(!declared);
            let index = i32::try_from(count).unwrap();
            let var = VarId::new(compiler, &name, name_node.into(), self, index, input, None);
            // canDeclare: only a function block scope can refuse, when its parent has the name.
            let parent_declares = self.is_function_block_scope(compiler)
                && self.get_parent(compiler).is_some_and(|parent| {
                    ScopeArena::read(compiler).scopes[parent.index()]
                        .abstract_scope
                        .vars
                        .contains_key(&name)
                });
            if parent_declares {
                self.declare_internal(compiler, name, var);
            } else {
                ScopeArena::write(compiler).scopes[self.index()]
                    .abstract_scope
                    .vars
                    .insert(name, var);
            }
            return var;
        }
        check_state!(self.get_own_slot(compiler, &name).is_none());
        let index = self.get_var_count(compiler);
        let var = VarId::new(compiler, &name, name_node.into(), self, index, input, None);
        self.declare_internal(compiler, name, var);
        var
    }

    // port: Scope#declareImplicitGoogNamespaceIfAbsent
    pub fn declare_implicit_goog_namespace_if_absent(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        definition: NodeId,
    ) -> VarId {
        let name = name.into();
        check_argument!(!name.is_empty());
        check_state!(
            self.is_global(compiler),
            "Cannot declare implicit goog namespace in local scope %s",
            self.to_string(compiler)
        );
        match self.get_own_slot(compiler, &name) {
            None => {
                let var = VarId::create_implicit_goog_namespace(compiler, &name, self, definition);
                self.declare_internal(compiler, name, var);
                var
            }
            Some(var) => {
                if var.is_implicit_goog_namespace(compiler) {
                    var.add_implicit_goog_namespace_definition(compiler, definition);
                }
                var
            }
        }
    }

    // port: Scope#makeImplicitVar
    pub fn make_implicit_var(self, compiler: &mut AbstractCompiler, var: ImplicitVar) -> VarId {
        VarId::new(compiler, var.name(), None, self, -1, None, None)
    }

    // Rust-only allocation separates arena identity from the common Java constructor data.
    fn allocate(compiler: &mut AbstractCompiler, root_node: NodeId) -> Self {
        let mut arena = ScopeArena::write(compiler);
        let scope = Self(NonZeroU32::new(u32::try_from(arena.scopes.len() + 1).unwrap()).unwrap());
        arena.scopes.push(ScopeData {
            abstract_scope: AbstractScopeData::new(root_node),
            parent: None,
            depth: 0,
            view: OnceLock::new(),
        });
        drop(arena);
        compiler.scope_mirror.scopes.push(ScopeMeta {
            root_node,
            parent: None,
            depth: 0,
        });
        scope
    }

    pub(crate) fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

impl AbstractScope for ScopeId {
    type Var = VarId;

    type DataRef<'a> = TypedArenaRef<'a, AbstractScopeData<VarId>, ScopeArena>;
    type DataMut<'a> = TypedArenaMut<'a, AbstractScopeData<VarId>, ScopeArena>;

    fn scope_data(self, compiler: &AbstractCompiler) -> Self::DataRef<'_> {
        TypedArenaRef::new(ScopeArena::read(compiler), self.index(), |a, i| {
            &a.scopes[i].abstract_scope
        })
    }

    fn scope_data_mut(self, compiler: &mut AbstractCompiler) -> Self::DataMut<'_> {
        TypedArenaMut::new(
            ScopeArena::write(compiler),
            self.index(),
            |a, i| &a.scopes[i].abstract_scope,
            |a, i| &mut a.scopes[i].abstract_scope,
        )
    }

    // port: AbstractScope#getRootNode
    fn get_root_node(self, compiler: &AbstractCompiler) -> NodeId {
        compiler.scope_mirror.scopes[self.index()].root_node
    }

    // port: AbstractScope#getVar
    fn get_var(self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<VarId> {
        if ImplicitVar::of(name).is_some() {
            return crate::abstract_scope::abstract_scope_get_var(self, compiler, name);
        }
        // No implicit slot can match `name`, so getOwnSlot only reads each scope's declared
        // vars: walk the chain under one read lock instead of one lock per scope (D-025).
        let arena = ScopeArena::read(compiler);
        let mut scope = Some(self);
        while let Some(current) = scope {
            if let Some(var) = arena.scopes[current.index()].abstract_scope.vars.get(name) {
                return Some(*var);
            }
            scope = compiler.scope_mirror.scopes[current.index()].parent;
        }
        None
    }

    fn get_depth(self, compiler: &AbstractCompiler) -> i32 {
        ScopeId::get_depth(self, compiler)
    }
    fn get_parent(self, compiler: &AbstractCompiler) -> Option<Self> {
        ScopeId::get_parent(self, compiler)
    }
    fn untyped(self, compiler: &AbstractCompiler) -> Self {
        ScopeId::untyped(self, compiler)
    }
    fn make_implicit_var(self, compiler: &mut AbstractCompiler, var: ImplicitVar) -> Option<VarId> {
        Some(ScopeId::make_implicit_var(self, compiler, var))
    }
}

// Rust-only inherent forwarding keeps arena-handle methods usable without importing the trait;
// every common Java body occurs once in AbstractScope.
macro_rules! scope_reader {
    ($name:ident, $result:ty) => {
        pub fn $name(self, compiler: &AbstractCompiler) -> $result {
            <Self as AbstractScope>::$name(self, compiler)
        }
    };
}
macro_rules! scope_name_reader {
    ($name:ident, $result:ty) => {
        pub fn $name(self, compiler: &AbstractCompiler, name: impl Into<JsString>) -> $result {
            <Self as AbstractScope>::$name(self, compiler, &name.into())
        }
    };
}
macro_rules! scope_name_mutator {
    ($name:ident, $result:ty) => {
        pub fn $name(self, compiler: &mut AbstractCompiler, name: impl Into<JsString>) -> $result {
            <Self as AbstractScope>::$name(self, compiler, &name.into())
        }
    };
}

impl ScopeId {
    scope_reader!(to_string, String);
    scope_reader!(typed, crate::typed_scope::TypedScope);
    pub fn contains(self, compiler: &AbstractCompiler, other: ScopeId) -> bool {
        <Self as AbstractScope>::contains(self, compiler, other)
    }
    scope_reader!(get_root_node, NodeId);
    scope_reader!(get_global_scope, ScopeId);
    scope_reader!(get_parent_scope, Option<ScopeId>);
    pub fn undeclare(self, compiler: &mut AbstractCompiler, var: VarId) {
        <Self as AbstractScope>::undeclare(self, compiler, var);
    }
    pub fn undeclare_interal(self, compiler: &mut AbstractCompiler, var: VarId) {
        <Self as AbstractScope>::undeclare_interal(self, compiler, var);
    }
    pub fn declare_internal(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        var: VarId,
    ) {
        <Self as AbstractScope>::declare_internal(self, compiler, name.into(), var);
    }
    pub fn clear_vars_internal(self, compiler: &mut AbstractCompiler) {
        <Self as AbstractScope>::clear_vars_internal(self, compiler);
    }
    pub fn has_own_implicit_slot(
        self,
        compiler: &AbstractCompiler,
        name: Option<ImplicitVar>,
    ) -> bool {
        <Self as AbstractScope>::has_own_implicit_slot(self, compiler, name)
    }
    scope_name_reader!(has_own_slot, bool);
    scope_name_reader!(has_slot, bool);
    pub fn get_own_implicit_slot(
        self,
        compiler: &mut AbstractCompiler,
        name: Option<ImplicitVar>,
    ) -> Option<VarId> {
        <Self as AbstractScope>::get_own_implicit_slot(self, compiler, name)
    }
    scope_name_mutator!(get_own_slot, Option<VarId>);
    scope_name_mutator!(get_slot, Option<VarId>);
    scope_name_mutator!(get_var, Option<VarId>);
    pub fn get_arguments_var(self, compiler: &mut AbstractCompiler) -> Option<VarId> {
        <Self as AbstractScope>::get_arguments_var(self, compiler)
    }
    scope_name_mutator!(can_declare, bool);
    scope_name_mutator!(is_bleeding_function_name, bool);
    scope_reader!(get_var_iterable, Vec<VarId>);
    scope_reader!(get_all_accessible_variables, Vec<VarId>);
    scope_reader!(get_all_symbols, Vec<VarId>);
    scope_reader!(get_var_count, i32);
    scope_reader!(is_global, bool);
    scope_reader!(is_local, bool);
    scope_reader!(is_block_scope, bool);
    scope_reader!(is_static_block_scope, bool);
    scope_reader!(is_function_block_scope, bool);
    scope_reader!(is_function_scope, bool);
    scope_reader!(is_module_scope, bool);
    scope_reader!(is_member_field_def_scope, bool);
    scope_reader!(is_computed_field_def_rhs_scope, bool);
    scope_reader!(is_catch_scope, bool);
    scope_reader!(is_cfg_root_scope, bool);
    scope_reader!(is_hoist_scope, bool);
    scope_reader!(get_closest_hoist_scope, Option<ScopeId>);
    scope_reader!(get_closest_cfg_root_scope, ScopeId);
    scope_reader!(get_closest_container_scope, ScopeId);
    pub fn check_child_scope(self, compiler: &AbstractCompiler, parent: ScopeId) {
        <Self as AbstractScope>::check_child_scope(self, compiler, parent);
    }
    pub fn check_root_scope(self, compiler: &AbstractCompiler) {
        <Self as AbstractScope>::check_root_scope(self, compiler);
    }
    pub fn get_common_parent(self, compiler: &AbstractCompiler, other: ScopeId) -> ScopeId {
        <Self as AbstractScope>::get_common_parent(self, compiler, other)
    }
    pub fn has_same_container_scope(self, compiler: &AbstractCompiler, other: ScopeId) -> bool {
        <Self as AbstractScope>::has_same_container_scope(self, compiler, other)
    }
    scope_reader!(get_scope_of_this, ScopeId);
}

/// Rust-only: Java's syntactic Scope object as closure-jstype sees it (a `StaticScope`, which
/// closure-jstype's registry takes as `&dyn StaticTypedScope`). One canonical view per scope,
/// created on first use, cached in its arena entry and leaked, exactly like `TypedScopeView`
/// (PORT_NOTES, "TypedScope and TypedVar views"). The view reads the shared arena; an implicit var
/// the compiler side has not materialised yet is absent. Typed queries answer what Java's untyped
/// Scope has: no type.
#[derive(Debug)]
pub struct ScopeView {
    arena: Weak<RwLock<ScopeArena>>,
    scope: ScopeId,
}

/// Rust-only: Java's syntactic Var as closure-jstype sees it (a `StaticSlot` and `StaticRef`);
/// one canonical, leaked view per var, like `ScopeView`.
#[derive(Debug)]
pub struct VarView {
    arena: Weak<RwLock<ScopeArena>>,
    var: VarId,
}

impl ScopeArena {
    /// Rust-only: the canonical view of `scope`, created on first use.
    fn scope_view(
        &self,
        this: &Weak<RwLock<ScopeArena>>,
        scope: ScopeId,
    ) -> &'static Arc<ScopeView> {
        self.scopes[scope.index()].view.get_or_init(|| {
            Box::leak(Box::new(Arc::new(ScopeView {
                arena: this.clone(),
                scope,
            })))
        })
    }

    /// Rust-only: the canonical view of `var`, created on first use.
    fn var_view(&self, this: &Weak<RwLock<ScopeArena>>, var: VarId) -> &'static VarView {
        self.var_views[var.index()].get_or_init(|| {
            Box::leak(Box::new(VarView {
                arena: this.clone(),
                var,
            }))
        })
    }

    // port: AbstractScope#getOwnSlot
    /// Rust-only read-only twin (see `ScopeView`): implicit vars only once materialised.
    fn view_get_own_slot(&self, scope: ScopeId, name: &JsString) -> Option<VarId> {
        let data = &self.scopes[scope.index()].abstract_scope;
        if let Some(var) = data.vars.get(name) {
            return Some(*var);
        }
        ImplicitVar::of(name).and_then(|implicit| data.implicit_vars.get(&implicit).copied())
    }

    // port: AbstractScope#getVar
    /// Rust-only read-only twin (see `ScopeView`).
    fn view_get_var(&self, scope: ScopeId, name: &JsString) -> Option<VarId> {
        let mut scope = Some(scope);
        while let Some(s) = scope {
            let var = self.view_get_own_slot(s, name);
            if var.is_some() {
                return var;
            }
            scope = self.scopes[s.index()].parent;
        }
        None
    }
}

/// Rust-only: the canonical closure-jstype view of the syntactic scope `scope` (Java passes the
/// Scope itself as a StaticScope, e.g. to JSTypeRegistry#identifyNonNullableName). The `Arc`'s
/// data pointer is the same for every call on the same scope (Java identity).
pub fn as_static_scope(compiler: &AbstractCompiler, scope: ScopeId) -> Arc<dyn StaticTypedScope> {
    let this = Arc::downgrade(&compiler.scope_arena);
    let view: Arc<ScopeView> = Arc::clone(ScopeArena::read(compiler).scope_view(&this, scope));
    view
}

impl ScopeView {
    fn read<R>(&self, f: impl FnOnce(&Weak<RwLock<ScopeArena>>, &ScopeArena) -> R) -> R {
        let arena = self
            .arena
            .upgrade()
            .expect("a Scope view outlived its compiler");
        let guard = arena.read().unwrap_or_else(PoisonError::into_inner);
        f(&self.arena, &guard)
    }
}

impl ScopeView {
    // port: AbstractScope#getOwnSlot
    /// Rust-only: the implicit-var side effect of Java's `getOwnSlot` on `scope`
    /// (AbstractScope#getOwnImplicitSlot calling Scope#makeImplicitVar, which never returns
    /// null). Returns whether `scope` has an own slot for `name` afterwards.
    fn create_own_implicit_slot(&self, ast: &Ast, scope: ScopeId, name: &JsString) -> bool {
        let implicit = ImplicitVar::of(name);
        let missing = self.read(|_, arena| {
            let data = &arena.scopes[scope.index()].abstract_scope;
            if data.vars.contains_key(name) {
                return Some(false);
            }
            // AbstractScope#hasOwnImplicitSlot
            let implicit = implicit.filter(|implicit| {
                crate::abstract_scope::view_is_made_by_scope(ast, data.root_node, *implicit)
            })?;
            Some(!data.implicit_vars.contains_key(&implicit))
        });
        match missing {
            None => false,
            Some(false) => true,
            Some(true) => {
                let implicit = implicit.unwrap();
                let arena = self
                    .arena
                    .upgrade()
                    .expect("a Scope view outlived its compiler");
                let mut arena = arena.write().unwrap_or_else(PoisonError::into_inner);
                // port: Scope#makeImplicitVar
                let data = AbstractVarData {
                    name: JsString::from(implicit.name()),
                    name_node: None,
                    implicit_goog_namespace_strength: None,
                    input: None,
                    index: -1,
                    scope: Some(scope),
                };
                let var = VarId::push(&mut arena, data);
                arena.scopes[scope.index()]
                    .abstract_scope
                    .implicit_vars
                    .insert(implicit, var);
                true
            }
        }
    }
}

impl StaticTypedScope for ScopeView {
    // port: AbstractScope#getRootNode
    fn get_root_node(&self) -> Option<NodeId> {
        self.read(|_, arena| Some(arena.scopes[self.scope.index()].abstract_scope.root_node))
    }

    // port: AbstractScope#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        self.read(|this, arena| {
            arena.scopes[self.scope.index()]
                .parent
                .map(|parent| &**arena.scope_view(this, parent) as &dyn StaticTypedScope)
        })
    }

    // port: AbstractScope#getSlot
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.read(|this, arena| {
            arena
                .view_get_var(self.scope, name)
                .map(|var| arena.var_view(this, var) as &dyn StaticTypedSlot)
        })
    }

    // port: AbstractScope#getOwnSlot
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.read(|this, arena| {
            arena
                .view_get_own_slot(self.scope, name)
                .map(|var| arena.var_view(this, var) as &dyn StaticTypedSlot)
        })
    }

    // port: AbstractScope#getSlot
    /// Rust-only twin of `get_slot` that creates implicit vars like Java (see
    /// `StaticTypedScope::get_slot_creating_implicit_vars`): AbstractScope#getVar walks the
    /// scopes with getOwnSlot, which creates the implicit var in the first scope that makes it.
    fn get_slot_creating_implicit_vars(
        &self,
        _reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        let mut scope = Some(self.scope);
        while let Some(s) = scope {
            if self.create_own_implicit_slot(ast, s, name) {
                break;
            }
            scope = self.read(|_, arena| arena.scopes[s.index()].parent);
        }
        self.get_slot(name)
    }

    // port: AbstractScope#getOwnSlot
    /// Rust-only twin of `get_own_slot` that creates implicit vars like Java (see
    /// `StaticTypedScope::get_own_slot_creating_implicit_vars`).
    fn get_own_slot_creating_implicit_vars(
        &self,
        _reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        self.create_own_implicit_slot(ast, self.scope, name);
        self.get_own_slot(name)
    }

    /// Rust-only: Java's syntactic Scope is no StaticTypedScope and has no type of `this`.
    fn get_type_of_this(&self, _reg: &JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        None
    }
}

impl VarView {
    fn read<R>(
        &self,
        f: impl FnOnce(&Weak<RwLock<ScopeArena>>, &AbstractVarData<ScopeId>, &ScopeArena) -> R,
    ) -> R {
        let arena = self
            .arena
            .upgrade()
            .expect("a Var view outlived its compiler");
        let guard = arena.read().unwrap_or_else(PoisonError::into_inner);
        f(&self.arena, &guard.vars[self.var.index()], &guard)
    }
}

impl StaticTypedSlot for VarView {
    // port: AbstractVar#getName
    fn get_name(&self, _reg: &JSTypeRegistry) -> JsString {
        self.read(|_, data, _| data.name.clone())
    }

    /// Rust-only: Java's syntactic Var is no StaticTypedSlot and has no type.
    fn get_type(&self, _reg: &JSTypeRegistry) -> Option<TypeId> {
        None
    }

    /// Rust-only: Java's syntactic Var is no StaticTypedSlot (no type, so none inferred).
    fn is_type_inferred(&self, _reg: &JSTypeRegistry) -> bool {
        false
    }

    // port: AbstractVar#getDeclaration
    fn get_declaration(&self, _reg: &JSTypeRegistry) -> Option<&dyn StaticTypedRef> {
        self.read(|_, data, _| data.name_node)
            .map(|_| self as &dyn StaticTypedRef)
    }

    // port: AbstractVar#getJSDocInfo
    /// Java reads `NodeUtil.getBestJSDocInfo(nameNode)`, which needs the AST that
    /// closure-jstype's slot method does not pass; closure-jstype never asks a scope slot for it.
    fn get_jsdoc_info(&self, _reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
        unreachable!("Var#getJSDocInfo needs the AST: call VarId::get_jsdoc_info(compiler)")
    }

    // port: AbstractVar#getScope
    fn get_scope(&self, _reg: &JSTypeRegistry) -> Option<&dyn StaticTypedScope> {
        self.read(|this, data, arena| {
            data.scope
                .map(|scope| &**arena.scope_view(this, scope) as &dyn StaticTypedScope)
        })
    }
}

impl StaticTypedRef for VarView {
    // port: AbstractVar#getSymbol
    fn get_symbol(&self, _reg: &JSTypeRegistry) -> &dyn StaticTypedSlot {
        self
    }

    // port: AbstractVar#getNode
    fn get_node(&self, _reg: &JSTypeRegistry) -> Option<NodeId> {
        self.read(|_, data, _| data.name_node)
    }

    // port: AbstractVar#getSourceFile
    fn get_source_file(
        &self,
        _reg: &JSTypeRegistry,
        ast: &Ast,
    ) -> Option<Arc<dyn StaticSourceFile>> {
        self.read(|_, data, arena| {
            data.name_node
                .unwrap_or_else(|| {
                    let scope = closure_rhino::check_not_null!(data.scope);
                    arena.scopes[scope.index()].abstract_scope.root_node
                })
                .get_static_source_file(ast)
        })
    }
}
