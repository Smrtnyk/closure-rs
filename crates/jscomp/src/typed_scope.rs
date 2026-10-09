/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/TypedScope.java,
//   src/com/google/javascript/jscomp/TypedVar.java.

//! TypedScope contains information about variables and their types. Scopes can be nested; a scope
//! points back to its parent scope.
//!
//! Scopes are handles into the compiler-owned `TypedScopeArena` (DESIGN "Scopes and
//! NodeTraversal"): handle equality is Java object identity, and the common AbstractScope bodies run
//! through the `AbstractScope` trait exactly as for syntactic scopes.
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::{AbstractScope, AbstractScopeData, ImplicitVar, abstract_scope_get_var},
    abstract_var::AbstractVarData,
    compiler_input::CompilerInput,
    modules::module::Module,
    node_util::NodeUtil,
    scope::ScopeId,
    typed_var::{TypedVar, TypedVarData},
    var::VarId,
};
use closure_jstype::{
    JSTypeRegistry, TypeId,
    prelude::{FunctionType, JSType, ObjectType},
    static_typed_scope::StaticTypedScope,
    static_typed_slot::StaticTypedSlot,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_state,
    java_lang::JavaHashCode,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::{
    num::NonZeroU32,
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypedScope(pub(crate) NonZeroU32);

#[derive(Debug)]
pub(crate) struct TypedScopeData {
    pub(crate) abstract_scope: AbstractScopeData<TypedVar>,
    pub(crate) parent: Option<TypedScope>,
    pub(crate) depth: i32,
    pub(crate) module: Option<Arc<Module>>,
    /// Whether this is a bottom scope for the purposes of type inference.
    pub(crate) is_bottom: bool,
    // Will shrink over time. Java swaps in ImmutableSet.of() once empty.
    pub(crate) reserved_names: IndexSet<JsString>,
    /// Rust-only: the canonical closure-jstype view (Java: this object as a StaticTypedScope),
    /// created on first use and never freed (PORT_NOTES, "TypedScope and TypedVar views").
    pub(crate) view: OnceLock<&'static Arc<TypedScopeView>>,
}

/// Rust-only (D-025): lock-free copies of a typed scope's fields that never change after
/// construction (see `scope::ScopeMirror`), in `Compiler::typed_scope_mirror`, index-aligned with
/// `TypedScopeArena::scopes`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TypedScopeMeta {
    root_node: NodeId,
    parent: Option<TypedScope>,
    depth: i32,
}

/// Rust-only owner for Java TypedScope and TypedVar object identities; slots are never removed.
///
/// The compiler owns it as `Arc<RwLock<..>>` (`TypedScopeArena::shared`) because the canonical
/// closure-jstype views of a scope or var (Java: the same object) read it after the registry
/// captured them. Compiler-side code reaches it only through `TypedScope`/`TypedVar` methods that
/// take the whole compiler, so a guard never outlives a statement that could lock it again.
#[derive(Debug, Default)]
pub struct TypedScopeArena {
    pub(crate) scopes: Vec<TypedScopeData>,
    pub(crate) vars: Vec<TypedVarData>,
}

impl TypedScopeArena {
    /// Rust-only: the compiler's shared arena.
    pub(crate) fn shared() -> Arc<RwLock<TypedScopeArena>> {
        Arc::new(RwLock::new(TypedScopeArena::default()))
    }

    pub(crate) fn read(compiler: &AbstractCompiler) -> RwLockReadGuard<'_, TypedScopeArena> {
        compiler
            .typed_scope_arena
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn write(compiler: &AbstractCompiler) -> RwLockWriteGuard<'_, TypedScopeArena> {
        compiler
            .typed_scope_arena
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Rust-only read guard projected onto one arena entry (Java reads the object's fields). `A` is
/// the shared arena: the typed arena, or the syntactic `ScopeArena`.
pub struct TypedArenaRef<'a, T: ?Sized, A = TypedScopeArena> {
    guard: RwLockReadGuard<'a, A>,
    index: usize,
    project: fn(&A, usize) -> &T,
}

impl<'a, T: ?Sized, A> TypedArenaRef<'a, T, A> {
    pub(crate) fn new(
        guard: RwLockReadGuard<'a, A>,
        index: usize,
        project: fn(&A, usize) -> &T,
    ) -> Self {
        Self {
            guard,
            index,
            project,
        }
    }
}

impl<T: ?Sized, A> Deref for TypedArenaRef<'_, T, A> {
    type Target = T;
    fn deref(&self) -> &T {
        (self.project)(&self.guard, self.index)
    }
}

/// Rust-only write guard projected onto one arena entry.
pub struct TypedArenaMut<'a, T: ?Sized, A = TypedScopeArena> {
    guard: RwLockWriteGuard<'a, A>,
    index: usize,
    project: fn(&A, usize) -> &T,
    project_mut: fn(&mut A, usize) -> &mut T,
}

impl<'a, T: ?Sized, A> TypedArenaMut<'a, T, A> {
    pub(crate) fn new(
        guard: RwLockWriteGuard<'a, A>,
        index: usize,
        project: fn(&A, usize) -> &T,
        project_mut: fn(&mut A, usize) -> &mut T,
    ) -> Self {
        Self {
            guard,
            index,
            project,
            project_mut,
        }
    }
}

impl<T: ?Sized, A> Deref for TypedArenaMut<'_, T, A> {
    type Target = T;
    fn deref(&self) -> &T {
        (self.project)(&self.guard, self.index)
    }
}

impl<T: ?Sized, A> DerefMut for TypedArenaMut<'_, T, A> {
    fn deref_mut(&mut self) -> &mut T {
        (self.project_mut)(&mut self.guard, self.index)
    }
}

impl TypedScope {
    pub(crate) fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    pub(crate) fn data(self, compiler: &AbstractCompiler) -> TypedArenaRef<'_, TypedScopeData> {
        TypedArenaRef::new(TypedScopeArena::read(compiler), self.index(), |a, i| {
            &a.scopes[i]
        })
    }

    pub(crate) fn data_mut(
        self,
        compiler: &mut AbstractCompiler,
    ) -> TypedArenaMut<'_, TypedScopeData> {
        TypedArenaMut::new(
            TypedScopeArena::write(compiler),
            self.index(),
            |a, i| &a.scopes[i],
            |a, i| &mut a.scopes[i],
        )
    }

    // port: TypedScope#TypedScope(TypedScope, Node)
    pub fn new(compiler: &mut AbstractCompiler, parent: TypedScope, root_node: NodeId) -> Self {
        Self::new_with_reserved_names(compiler, parent, root_node, &IndexSet::<_>::default(), None)
    }

    // port: TypedScope#TypedScope(TypedScope, Node, Set, Module)
    pub fn new_with_reserved_names(
        compiler: &mut AbstractCompiler,
        parent: TypedScope,
        root_node: NodeId,
        reserved_names: &IndexSet<JsString>,
        module: Option<Arc<Module>>,
    ) -> Self {
        let scope = Self::allocate(compiler, root_node);
        scope.check_child_scope(compiler, parent);
        let depth = parent.data(compiler).depth + 1;
        let meta = &mut compiler.typed_scope_mirror[scope.index()];
        meta.parent = Some(parent);
        meta.depth = depth;
        let mut data = scope.data_mut(compiler);
        data.parent = Some(parent);
        data.depth = depth;
        data.is_bottom = false;
        data.reserved_names = if reserved_names.is_empty() {
            IndexSet::<_>::default()
        } else {
            reserved_names.clone()
        };
        data.module = module;
        scope
    }

    // port: TypedScope#TypedScope(Node, boolean)
    fn new_root(compiler: &mut AbstractCompiler, root_node: NodeId, is_bottom: bool) -> Self {
        let scope = Self::allocate(compiler, root_node);
        scope.check_root_scope(compiler);
        let mut data = scope.data_mut(compiler);
        data.parent = None;
        data.depth = 0;
        data.is_bottom = is_bottom;
        data.reserved_names = IndexSet::<_>::default();
        data.module = None;
        scope
    }

    // Rust-only allocation separates arena identity from the Java constructor bodies.
    fn allocate(compiler: &mut AbstractCompiler, root_node: NodeId) -> Self {
        let mut arena = TypedScopeArena::write(compiler);
        let scopes = &mut arena.scopes;
        let scope = Self(NonZeroU32::new(u32::try_from(scopes.len() + 1).unwrap()).unwrap());
        scopes.push(TypedScopeData {
            abstract_scope: AbstractScopeData::new(root_node),
            parent: None,
            depth: 0,
            module: None,
            is_bottom: false,
            reserved_names: IndexSet::<_>::default(),
            view: OnceLock::new(),
        });
        drop(arena);
        compiler.typed_scope_mirror.push(TypedScopeMeta {
            root_node,
            parent: None,
            depth: 0,
        });
        scope
    }

    // port: TypedScope#createGlobalScope
    pub fn create_global_scope(compiler: &mut AbstractCompiler, root_node: NodeId) -> Self {
        Self::new_root(compiler, root_node, false)
    }

    // port: TypedScope#createLatticeBottom
    pub fn create_lattice_bottom(compiler: &mut AbstractCompiler, root_node: NodeId) -> Self {
        Self::new_root(compiler, root_node, true)
    }

    // port: TypedScope#typed
    pub fn typed(self, _compiler: &AbstractCompiler) -> TypedScope {
        self
    }

    // port: TypedScope#validateCompletelyBuilt
    pub fn validate_completely_built(self, compiler: &mut AbstractCompiler) {
        let reserved_names = self.data(compiler).reserved_names.clone();
        check_state!(
            reserved_names.is_empty(),
            "Expected %s to have no reserved names, found: %s. This probably indicates a bug in TypedScopeCreator where it is failing to declare a variable.",
            self.to_string(compiler),
            format!(
                "[{}]",
                reserved_names
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        );
        // let a (16-bit) VM garbage collect 64 bytes per TypedScope. (ImmutableSet.of() returns a
        // singleton)
        self.data_mut(compiler).reserved_names = IndexSet::<_>::default();
    }

    // port: TypedScope#isBottom
    pub fn is_bottom(self, compiler: &AbstractCompiler) -> bool {
        self.data(compiler).is_bottom
    }

    // port: TypedScope#getModule
    pub fn get_module(self, compiler: &AbstractCompiler) -> Option<Arc<Module>> {
        self.data(compiler).module.clone()
    }

    // port: TypedScope#getDepth
    pub fn get_depth(self, compiler: &AbstractCompiler) -> i32 {
        compiler.typed_scope_mirror[self.index()].depth
    }

    // port: TypedScope#getParent
    pub fn get_parent(self, compiler: &AbstractCompiler) -> Option<TypedScope> {
        compiler.typed_scope_mirror[self.index()].parent
    }

    // port: TypedScope#getTypeOfThis
    pub fn get_type_of_this(self, compiler: &mut AbstractCompiler) -> Option<TypeId> {
        let root = self.get_root_node(compiler);
        if self.is_global(compiler) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            root.get_jstype(ast).and_then(|t| t.to_object_type(reg))
        } else if NodeUtil::is_non_arrow_function(compiler, root) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let node_type = root.get_jstype(ast);
            if let Some(node_type) = node_type.filter(|t| t.is_function_type(reg)) {
                node_type
                    .to_maybe_function_type(reg)
                    .unwrap()
                    .get_type_of_this(reg)
            } else {
                // Executed when the current scope has not been typechecked.
                None
            }
        } else if self.is_static_block_scope(compiler) {
            let parent_root = self.get_parent(compiler).unwrap().get_root_node(compiler);
            parent_root.get_jstype(compiler)
        } else if self.is_member_field_def_scope(compiler)
            || self.is_computed_field_def_rhs_scope(compiler)
        {
            let parent_root = self.get_parent(compiler).unwrap().get_root_node(compiler);
            let is_static_member = root.is_static_member(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let class_type = parent_root.get_jstype(ast);
            if is_static_member {
                class_type
            } else {
                class_type
                    .expect("NullPointerException")
                    .assert_function_type(reg, ast)
                    .get_instance_type(reg)
            }
        } else {
            self.get_parent(compiler)
                .unwrap()
                .get_type_of_this(compiler)
        }
    }

    // port: TypedScope#declare
    pub fn declare(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        name_node: Option<NodeId>,
        type_: Option<TypeId>,
        input: Option<CompilerInput>,
        inferred: bool,
    ) -> TypedVar {
        let name = name.into();
        check_state!(!name.is_empty());
        if !self.data(compiler).reserved_names.is_empty() {
            // (reservedNames only contains simple, non-qualified names; so it's completely normal to
            // declare qualified names that were not 'reserved')
            self.data_mut(compiler).reserved_names.shift_remove(&name);
        }
        let index = self.get_var_count(compiler);
        let var = TypedVar::new(
            compiler,
            inferred,
            name.clone(),
            name_node,
            type_,
            self,
            index,
            input,
        );
        self.declare_internal(compiler, name, var);
        var
    }

    // port: TypedScope#makeImplicitVar
    pub fn make_implicit_var(
        self,
        compiler: &mut AbstractCompiler,
        var: ImplicitVar,
    ) -> Option<TypedVar> {
        if self.is_global(compiler) {
            // TODO(sdh): This is incorrect for 'global this', but since that's currently not handled
            // by this code, it's okay to bail out now until we find the root cause.  See b/74980936.
            return None;
        } else if var == ImplicitVar::EXPORTS {
            // Instead of using the implicit 'exports' var, we want to pretend that the var is actually
            // declared.
            return None;
        }
        let type_ = self.get_implicit_var_type(compiler, var);
        Some(TypedVar::new(
            compiler,
            false,
            var.js_name(),
            None,
            type_,
            self,
            -1,
            None,
        ))
    }

    // port: TypedScope#hasOwnImplicitSlot
    pub fn has_own_implicit_slot(
        self,
        compiler: &AbstractCompiler,
        name: Option<ImplicitVar>,
    ) -> bool {
        name.is_some_and(|name| {
            name != ImplicitVar::EXPORTS && name.is_made_by_scope(compiler, self)
        })
    }

    // port: TypedScope#getImplicitVarType
    fn get_implicit_var_type(
        self,
        compiler: &mut AbstractCompiler,
        var: ImplicitVar,
    ) -> Option<TypeId> {
        match var {
            ImplicitVar::ARGUMENTS => {
                // Look for an extern named "arguments" and use its type if available.
                // TODO(sdh): consider looking for "Arguments" ctor rather than "arguments" var: this
                // could allow deleting the variable, which doesn't really belong in externs in the
                // first place.
                let global_args = self
                    .get_global_scope(compiler)
                    .get_var(compiler, VarId::ARGUMENTS);
                match global_args {
                    Some(global_args) if global_args.is_extern(compiler) => {
                        global_args.get_type(compiler)
                    }
                    _ => None,
                }
            }
            ImplicitVar::THIS => self.get_type_of_this(compiler),
            ImplicitVar::SUPER => {
                // Inside a constructor, `super` may have two different types. Calls to `super()` use
                // the super-ctor type, while property accesses use the super-instance type. This logic
                // always returns the latter case.
                let type_of_this = self.get_type_of_this(compiler);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let receiver_type = type_of_this.and_then(|t| t.to_object_type(reg))?;
                if receiver_type.is_instance_type(reg) {
                    let superclass_ctor = receiver_type.get_super_class_constructor(reg, ast);
                    superclass_ctor.and_then(|ctor| ctor.get_instance_type(reg))
                } else {
                    receiver_type.get_implicit_prototype(reg, ast)
                }
            }
            ImplicitVar::EXPORTS => {
                panic!("TypedScopes should not contain an implicit 'exports'")
            }
        }
    }

    // port: TypedScope#getDeclarativelyUnboundVarsWithoutTypes
    pub fn get_declaratively_unbound_vars_without_types(
        self,
        compiler: &AbstractCompiler,
    ) -> Vec<TypedVar> {
        self.get_var_iterable(compiler)
            .into_iter()
            .filter(|var| self.is_declaratively_unbound_var_without_type(compiler, *var))
            .collect()
    }

    // port: TypedScope#isDeclarativelyUnboundVarWithoutType
    fn is_declaratively_unbound_var_without_type(
        self,
        compiler: &AbstractCompiler,
        var: TypedVar,
    ) -> bool {
        var.get_parent_node(compiler).is_some()
            && var.get_type(compiler).is_none()
            // TODO(bradfordcsmith): update this for destructuring
            && var.get_parent_node(compiler).unwrap().is_var(compiler) // NOTE: explicitly excludes let/const
            && !var.is_extern(compiler)
    }

    // port: TypedScope#getVar
    pub fn get_var(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
    ) -> Option<TypedVar> {
        let name = name.into();
        let own_slot = self.get_own_slot(compiler, name.clone());
        if own_slot.is_some() {
            // Micro-optimization: variables declared directly in this scope cannot have been
            // shadowed.
            return own_slot;
        } else if self.get_parent(compiler).is_none() {
            return None;
        }
        // Find the root name and its slot.
        let dot = name.index_of_char(u16::from(b'.'));
        let root_name = if dot < 0 {
            name.clone()
        } else {
            name.substring(0, dot as usize)
        };
        // Use the superclass method to skip string checks.
        let root_var = abstract_scope_get_var(self, compiler, &root_name);
        match root_var {
            _ if dot < 0 => root_var,
            None => {
                // Default to the global scope because externs may have qualified names with
                // undeclared roots.
                self.get_parent(compiler)
                    .unwrap()
                    .get_global_scope(compiler)
                    .get_own_slot(compiler, name)
            }
            Some(root_var) => {
                // Qualified names 'a.b.c' are declared in the same scope as 'a', never a child
                // scope, which is why calling `getOwnSlot` is sufficient.
                root_var.get_scope(compiler).get_own_slot(compiler, name)
            }
        }
    }

    // port: TypedScope#getTypeThroughNamespace
    pub fn get_type_through_namespace(
        self,
        compiler: &mut AbstractCompiler,
        module_id: impl Into<JsString>,
    ) -> Option<TypeId> {
        let module_id = module_id.into();
        let split = module_id.last_index_of_char(u16::from(b'.'));
        if split >= 0 {
            let parent_name = module_id.substring(0, split as usize);
            let prop = module_id.substring_from(split as usize + 1);
            let parent_type = self.get_type_through_namespace(compiler, parent_name)?;
            let (reg, ast) = compiler.get_type_registry_and_ast();
            parent_type.to_maybe_object_type(reg)?;
            Some(
                parent_type
                    .assert_object_type(reg, ast)
                    .get_property_type(reg, ast, prop),
            )
        } else {
            let var = self.get_slot(compiler, module_id);
            var.and_then(|var| var.get_type(compiler))
        }
    }

    // port: TypedScope#getTopmostScopeOfEventualDeclaration
    pub fn get_topmost_scope_of_eventual_declaration(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
    ) -> Option<TypedScope> {
        let name = name.into();
        if self.get_own_slot(compiler, name.clone()).is_some()
            || self.data(compiler).reserved_names.contains(&name)
        {
            Some(self)
        } else {
            // Recurse on the parent because it, too, may be incomplete.
            self.get_parent(compiler)?
                .get_topmost_scope_of_eventual_declaration(compiler, name)
        }
    }
}

/// Rust-only: HamtPMap keys (LinkedFlowScope's scope map) need Java's `hashCode`. TypedScope
/// keeps Object's identity hash, which is JVM-dependent, so the HAMT's internal layout cannot match
/// Java's anyway; the handle's arena index is a deterministic stand-in. LinkedFlowScope only gets,
/// puts, removes, compares and reconciles by key, so no output depends on the layout.
impl JavaHashCode for TypedScope {
    // port: Object#hashCode
    fn hash_code(&self) -> i32 {
        self.0.get() as i32
    }
}

impl AbstractScope for TypedScope {
    type Var = TypedVar;

    type DataRef<'a> = TypedArenaRef<'a, AbstractScopeData<TypedVar>>;
    type DataMut<'a> = TypedArenaMut<'a, AbstractScopeData<TypedVar>>;

    fn scope_data(self, compiler: &AbstractCompiler) -> Self::DataRef<'_> {
        TypedArenaRef::new(TypedScopeArena::read(compiler), self.index(), |a, i| {
            &a.scopes[i].abstract_scope
        })
    }

    fn scope_data_mut(self, compiler: &mut AbstractCompiler) -> Self::DataMut<'_> {
        TypedArenaMut::new(
            TypedScopeArena::write(compiler),
            self.index(),
            |a, i| &a.scopes[i].abstract_scope,
            |a, i| &mut a.scopes[i].abstract_scope,
        )
    }

    fn get_depth(self, compiler: &AbstractCompiler) -> i32 {
        TypedScope::get_depth(self, compiler)
    }
    fn get_parent(self, compiler: &AbstractCompiler) -> Option<Self> {
        TypedScope::get_parent(self, compiler)
    }
    // port: AbstractScope#getRootNode
    fn get_root_node(self, compiler: &AbstractCompiler) -> NodeId {
        compiler.typed_scope_mirror[self.index()].root_node
    }
    fn typed(self, compiler: &AbstractCompiler) -> TypedScope {
        TypedScope::typed(self, compiler)
    }
    fn make_implicit_var(
        self,
        compiler: &mut AbstractCompiler,
        var: ImplicitVar,
    ) -> Option<TypedVar> {
        TypedScope::make_implicit_var(self, compiler, var)
    }
    fn has_own_implicit_slot(self, compiler: &AbstractCompiler, name: Option<ImplicitVar>) -> bool {
        TypedScope::has_own_implicit_slot(self, compiler, name)
    }
    fn get_var(self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<TypedVar> {
        TypedScope::get_var(self, compiler, name.clone())
    }
    fn get_topmost_scope_of_eventual_declaration(
        self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
    ) -> Option<TypedScope> {
        TypedScope::get_topmost_scope_of_eventual_declaration(self, compiler, name.clone())
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

impl TypedScope {
    scope_reader!(to_string, String);
    pub fn untyped(self, compiler: &AbstractCompiler) -> ScopeId {
        <Self as AbstractScope>::untyped(self, compiler)
    }
    pub fn contains(self, compiler: &AbstractCompiler, other: TypedScope) -> bool {
        <Self as AbstractScope>::contains(self, compiler, other)
    }
    scope_reader!(get_root_node, NodeId);
    scope_reader!(get_global_scope, TypedScope);
    scope_reader!(get_parent_scope, Option<TypedScope>);
    pub fn undeclare(self, compiler: &mut AbstractCompiler, var: TypedVar) {
        <Self as AbstractScope>::undeclare(self, compiler, var);
    }
    pub fn undeclare_interal(self, compiler: &mut AbstractCompiler, var: TypedVar) {
        <Self as AbstractScope>::undeclare_interal(self, compiler, var);
    }
    pub fn declare_internal(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        var: TypedVar,
    ) {
        <Self as AbstractScope>::declare_internal(self, compiler, name.into(), var);
    }
    pub fn clear_vars_internal(self, compiler: &mut AbstractCompiler) {
        <Self as AbstractScope>::clear_vars_internal(self, compiler);
    }
    scope_name_reader!(has_own_slot, bool);
    scope_name_reader!(has_slot, bool);
    pub fn get_own_implicit_slot(
        self,
        compiler: &mut AbstractCompiler,
        name: Option<ImplicitVar>,
    ) -> Option<TypedVar> {
        <Self as AbstractScope>::get_own_implicit_slot(self, compiler, name)
    }
    scope_name_mutator!(get_own_slot, Option<TypedVar>);
    scope_name_mutator!(get_slot, Option<TypedVar>);
    pub fn get_arguments_var(self, compiler: &mut AbstractCompiler) -> Option<TypedVar> {
        <Self as AbstractScope>::get_arguments_var(self, compiler)
    }
    scope_name_mutator!(can_declare, bool);
    scope_name_mutator!(is_bleeding_function_name, bool);
    scope_reader!(get_var_iterable, Vec<TypedVar>);
    scope_reader!(get_all_accessible_variables, Vec<TypedVar>);
    scope_reader!(get_all_symbols, Vec<TypedVar>);
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
    scope_reader!(get_closest_hoist_scope, Option<TypedScope>);
    scope_reader!(get_closest_cfg_root_scope, TypedScope);
    scope_reader!(get_closest_container_scope, TypedScope);
    pub fn check_child_scope(self, compiler: &AbstractCompiler, parent: TypedScope) {
        <Self as AbstractScope>::check_child_scope(self, compiler, parent);
    }
    pub fn check_root_scope(self, compiler: &AbstractCompiler) {
        <Self as AbstractScope>::check_root_scope(self, compiler);
    }
    pub fn get_common_parent(self, compiler: &AbstractCompiler, other: TypedScope) -> TypedScope {
        <Self as AbstractScope>::get_common_parent(self, compiler, other)
    }
    pub fn has_same_container_scope(self, compiler: &AbstractCompiler, other: TypedScope) -> bool {
        <Self as AbstractScope>::has_same_container_scope(self, compiler, other)
    }
    scope_reader!(get_scope_of_this, TypedScope);
}

/// Rust-only: Java's TypedScope object as closure-jstype sees it (a `StaticTypedScope`).
///
/// closure-jstype's scope traits hand out `&dyn` references and NamedType compares scopes by
/// address (Java `!=`), so every TypedScope has exactly one view, created on first use and cached
/// in its arena entry. The view holds only a `Weak` to the compiler's arena; it is leaked (one
/// small allocation per scope the registry touches; PORT_NOTES, "TypedScope and TypedVar views").
/// Its methods are read-only twins of the compiler-side TypedScope methods: they read the arena,
/// and an implicit var (`this`, `arguments`, `super`, `exports`) the compiler side has not
/// materialised yet is absent (JSDoc type names never start with these words).
#[derive(Debug)]
pub struct TypedScopeView {
    arena: Weak<RwLock<TypedScopeArena>>,
    scope: TypedScope,
}

impl TypedScopeArena {
    /// Rust-only: the canonical view of `scope`, created on first use.
    pub(crate) fn scope_view(
        &self,
        this: &Weak<RwLock<TypedScopeArena>>,
        scope: TypedScope,
    ) -> &'static Arc<TypedScopeView> {
        self.scopes[scope.index()].view.get_or_init(|| {
            Box::leak(Box::new(Arc::new(TypedScopeView {
                arena: this.clone(),
                scope,
            })))
        })
    }

    // port: AbstractScope#getOwnSlot
    /// Rust-only read-only twin (see `TypedScopeView`): implicit vars only once materialised.
    fn view_get_own_slot(&self, scope: TypedScope, name: &JsString) -> Option<TypedVar> {
        let data = &self.scopes[scope.index()].abstract_scope;
        if let Some(var) = data.vars.get(name) {
            return Some(*var);
        }
        ImplicitVar::of(name).and_then(|implicit| data.implicit_vars.get(&implicit).copied())
    }

    // port: AbstractScope#getVar
    /// Rust-only read-only twin (see `TypedScopeView`).
    fn view_abstract_get_var(&self, scope: TypedScope, name: &JsString) -> Option<TypedVar> {
        let mut scope = Some(scope);
        while let Some(s) = scope {
            let var = self.view_get_own_slot(s, name);
            if var.is_some() {
                return var;
            }
            // Recurse up the parent Scope
            scope = self.scopes[s.index()].parent;
        }
        None
    }

    // port: TypedScope#getVar
    /// Rust-only read-only twin (see `TypedScopeView`).
    fn view_get_var(&self, scope: TypedScope, name: &JsString) -> Option<TypedVar> {
        let own_slot = self.view_get_own_slot(scope, name);
        let parent = self.scopes[scope.index()].parent;
        if own_slot.is_some() {
            // Micro-optimization: variables declared directly in this scope cannot have been
            // shadowed.
            return own_slot;
        } else if parent.is_none() {
            return None;
        }
        // Find the root name and its slot.
        let dot = name.index_of_char(u16::from(b'.'));
        let root_name = if dot < 0 {
            name.clone()
        } else {
            name.substring(0, dot as usize)
        };
        // Use the superclass method to skip string checks.
        let root_var = self.view_abstract_get_var(scope, &root_name);
        match root_var {
            _ if dot < 0 => root_var,
            None => {
                // Default to the global scope because externs may have qualified names with
                // undeclared roots.
                let mut global = parent.unwrap();
                while let Some(p) = self.scopes[global.index()].parent {
                    global = p;
                }
                self.view_get_own_slot(global, name)
            }
            Some(root_var) => {
                // Qualified names 'a.b.c' are declared in the same scope as 'a', never a child
                // scope, which is why calling `getOwnSlot` is sufficient.
                let var_scope = self.vars[root_var.index()].abstract_var.scope.unwrap();
                self.view_get_own_slot(var_scope, name)
            }
        }
    }
}

impl TypedScope {
    /// Rust-only: the canonical closure-jstype view of this scope (Java passes `this` as a
    /// StaticTypedScope), for registry calls that take `&dyn StaticTypedScope`.
    pub fn as_static_typed_scope(
        self,
        compiler: &AbstractCompiler,
    ) -> &'static dyn StaticTypedScope {
        &**self.static_typed_scope_view(compiler)
    }

    /// Rust-only: the same canonical view as an `Arc` (NamedType and SyntheticTemplateScope keep
    /// it); its data pointer is the address `as_static_typed_scope` returns.
    pub fn as_static_typed_scope_arc(
        self,
        compiler: &AbstractCompiler,
    ) -> Arc<dyn StaticTypedScope> {
        let view: Arc<TypedScopeView> = Arc::clone(self.static_typed_scope_view(compiler));
        view
    }

    fn static_typed_scope_view(self, compiler: &AbstractCompiler) -> &'static Arc<TypedScopeView> {
        let this = Arc::downgrade(&compiler.typed_scope_arena);
        TypedScopeArena::read(compiler).scope_view(&this, self)
    }

    /// Rust-only: the TypedScope a closure-jstype scope reference stands for, when it is one of
    /// the canonical views (Java: a downcast to TypedScope).
    pub fn from_static_typed_scope(
        compiler: &AbstractCompiler,
        scope: &dyn StaticTypedScope,
    ) -> Option<TypedScope> {
        let arena = TypedScopeArena::read(compiler);
        arena
            .scopes
            .iter()
            .filter_map(|data| data.view.get())
            .find(|view| std::ptr::addr_eq(Arc::as_ptr(view), scope))
            .map(|view| view.scope)
    }
}

impl TypedScopeView {
    fn read<R>(&self, f: impl FnOnce(&Weak<RwLock<TypedScopeArena>>, &TypedScopeArena) -> R) -> R {
        let arena = self
            .arena
            .upgrade()
            .expect("a TypedScope view outlived its compiler");
        let guard = arena.read().unwrap_or_else(PoisonError::into_inner);
        f(&self.arena, &guard)
    }
}

impl TypedScopeView {
    // port: TypedScope#getVar
    /// Rust-only: the implicit-var side effect of Java's `getSlot` on this scope. TypedScope#getVar
    /// looks the name up with getOwnSlot and then AbstractScope#getVar walks the parents with
    /// getOwnSlot, and getOwnSlot creates an implicit var (`this`, `arguments`, `super`) in the
    /// first scope that makes it (AbstractScope#getOwnImplicitSlot calling
    /// TypedScope#makeImplicitVar, whose type needs the registry and the AST). This creates the var
    /// the same lookup creates in Java; the lookup itself then reads the arena.
    fn create_implicit_var_for_get_var(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) {
        let dot = name.index_of_char(u16::from(b'.'));
        let root_name = if dot < 0 {
            name.clone()
        } else {
            name.substring(0, dot as usize)
        };
        let Some(implicit) = ImplicitVar::of(&root_name) else {
            return;
        };
        let owner = self.read(|_, arena| {
            if dot >= 0 && arena.view_get_own_slot(self.scope, name).is_some() {
                return None;
            }
            arena.scopes[self.scope.index()].parent?;
            let mut scope = Some(self.scope);
            while let Some(s) = scope {
                let data = &arena.scopes[s.index()];
                if data.abstract_scope.vars.contains_key(&root_name)
                    || data.abstract_scope.implicit_vars.contains_key(&implicit)
                {
                    return None;
                }
                // TypedScope#hasOwnImplicitSlot
                if implicit != ImplicitVar::EXPORTS
                    && crate::abstract_scope::view_is_made_by_scope(
                        ast,
                        data.abstract_scope.root_node,
                        implicit,
                    )
                {
                    // TypedScope#makeImplicitVar returns null in the global scope; computeIfAbsent
                    // then stores nothing and the walk goes on (to no parent).
                    return data.parent.map(|_| s);
                }
                scope = data.parent;
            }
            None
        });
        let Some(owner) = owner else {
            return;
        };
        self.make_implicit_var_in(reg, ast, owner, implicit);
    }

    // port: AbstractScope#getOwnSlot
    /// Rust-only: the implicit-var side effect of Java's `getOwnSlot` on this scope
    /// (AbstractScope#getOwnImplicitSlot calling TypedScope#makeImplicitVar, see
    /// `create_implicit_var_for_get_var`).
    fn create_implicit_var_for_get_own_slot(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) {
        let Some(implicit) = ImplicitVar::of(name) else {
            return;
        };
        let missing = self.read(|_, arena| {
            let data = &arena.scopes[self.scope.index()];
            !data.abstract_scope.vars.contains_key(name)
                && !data.abstract_scope.implicit_vars.contains_key(&implicit)
                // TypedScope#hasOwnImplicitSlot
                && implicit != ImplicitVar::EXPORTS
                && crate::abstract_scope::view_is_made_by_scope(ast, data.abstract_scope.root_node, implicit)
                // TypedScope#makeImplicitVar returns null in the global scope; computeIfAbsent
                // then stores nothing.
                && data.parent.is_some()
        });
        if missing {
            self.make_implicit_var_in(reg, ast, self.scope, implicit);
        }
    }

    // port: TypedScope#makeImplicitVar
    /// Rust-only: creates the implicit var `implicit` of the non-global scope `owner` and stores
    /// it like AbstractScope#getOwnImplicitSlot's computeIfAbsent. The type is computed before the
    /// arena's write lock is taken (no read guard may be held then).
    fn make_implicit_var_in(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        owner: TypedScope,
        implicit: ImplicitVar,
    ) {
        let type_ = self
            .read(|this, arena| arena.scope_view(this, owner))
            .get_implicit_var_type(reg, ast, implicit);
        let arena = self
            .arena
            .upgrade()
            .expect("a TypedScope view outlived its compiler");
        let mut arena = arena.write().unwrap_or_else(PoisonError::into_inner);
        // port: TypedScope#makeImplicitVar
        let abstract_var = AbstractVarData {
            name: implicit.js_name(),
            name_node: None,
            implicit_goog_namespace_strength: None,
            input: None,
            index: -1,
            scope: Some(owner),
        };
        // port: TypedVar#TypedVar
        let var = TypedVar::push(&mut arena, false, abstract_var, type_);
        arena.scopes[owner.index()]
            .abstract_scope
            .implicit_vars
            .insert(implicit, var);
    }

    // port: TypedScope#getImplicitVarType
    /// Rust-only twin of `TypedScope::get_implicit_var_type` on the view.
    fn get_implicit_var_type(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        var: ImplicitVar,
    ) -> Option<TypeId> {
        match var {
            ImplicitVar::ARGUMENTS => {
                // Look for an extern named "arguments" and use its type if available.
                let arguments = JsString::from("arguments");
                self.read(|_, arena| {
                    let mut global = self.scope;
                    while let Some(p) = arena.scopes[global.index()].parent {
                        global = p;
                    }
                    let global_args = arena.view_get_own_slot(global, &arguments)?;
                    let data = &arena.vars[global_args.index()];
                    // AbstractVar#isExtern
                    let is_extern = data
                        .abstract_var
                        .input
                        .as_ref()
                        .is_none_or(|input| input.is_extern());
                    if is_extern { data.type_ } else { None }
                })
            }
            ImplicitVar::THIS => self.get_type_of_this(reg, ast),
            ImplicitVar::SUPER => {
                // Inside a constructor, `super` may have two different types. Calls to `super()` use
                // the super-ctor type, while property accesses use the super-instance type. This logic
                // always returns the latter case.
                let type_of_this = self.get_type_of_this(reg, ast);
                let receiver_type = type_of_this.and_then(|t| t.to_object_type(reg))?;
                if receiver_type.is_instance_type(reg) {
                    let superclass_ctor = receiver_type.get_super_class_constructor(reg, ast);
                    superclass_ctor.and_then(|ctor| ctor.get_instance_type(reg))
                } else {
                    receiver_type.get_implicit_prototype(reg, ast)
                }
            }
            ImplicitVar::EXPORTS => {
                panic!("TypedScopes should not contain an implicit 'exports'")
            }
        }
    }
}

impl StaticTypedScope for TypedScopeView {
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

    // port: AbstractScope#getSlot
    /// Rust-only twin of `get_slot` that creates implicit vars like Java (see
    /// `StaticTypedScope::get_slot_creating_implicit_vars`).
    fn get_slot_creating_implicit_vars(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        self.create_implicit_var_for_get_var(reg, ast, name);
        self.get_slot(name)
    }

    // port: AbstractScope#getOwnSlot
    /// Rust-only twin of `get_own_slot` that creates implicit vars like Java (see
    /// `StaticTypedScope::get_own_slot_creating_implicit_vars`).
    fn get_own_slot_creating_implicit_vars(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        self.create_implicit_var_for_get_own_slot(reg, ast, name);
        self.get_own_slot(name)
    }

    // port: AbstractScope#getOwnSlot
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.read(|this, arena| {
            arena
                .view_get_own_slot(self.scope, name)
                .map(|var| arena.var_view(this, var) as &dyn StaticTypedSlot)
        })
    }

    // port: TypedScope#getTypeOfThis
    fn get_type_of_this(&self, reg: &JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        let (root, parent) = self.read(|_, arena| {
            let data = &arena.scopes[self.scope.index()];
            (data.abstract_scope.root_node, data.parent)
        });
        let parent_view =
            || parent.map(|parent| self.read(|this, arena| arena.scope_view(this, parent)));
        if parent.is_none() {
            root.get_jstype(ast).and_then(|t| t.to_object_type(reg))
        } else if NodeUtil::is_non_arrow_function(ast, root) {
            let node_type = root.get_jstype(ast);
            if let Some(node_type) = node_type.filter(|t| t.is_function_type(reg)) {
                node_type
                    .to_maybe_function_type(reg)
                    .unwrap()
                    .get_type_of_this(reg)
            } else {
                // Executed when the current scope has not been typechecked.
                None
            }
        } else if NodeUtil::is_class_static_block(ast, root) {
            parent_view()
                .unwrap()
                .get_root_node()
                .unwrap()
                .get_jstype(ast)
        } else if root.is_member_field_def(ast) || root.is_computed_field_def(ast) {
            let class_type = parent_view()
                .unwrap()
                .get_root_node()
                .unwrap()
                .get_jstype(ast);
            if root.is_static_member(ast) {
                class_type
            } else {
                // JSType#assertFunctionType; its failure message needs the registry mutably.
                let class_type = class_type.expect("NullPointerException");
                class_type
                    .to_maybe_function_type(reg)
                    .unwrap_or_else(|| panic!("not a FunctionType: {class_type:?}"))
                    .get_instance_type(reg)
            }
        } else {
            parent_view().unwrap().get_type_of_this(reg, ast)
        }
    }

    // port: TypedScope#getTopmostScopeOfEventualDeclaration
    fn get_topmost_scope_of_eventual_declaration(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedScope> {
        if self
            .get_own_slot_creating_implicit_vars(reg, ast, name)
            .is_some()
            || self.read(|_, arena| {
                arena.scopes[self.scope.index()]
                    .reserved_names
                    .contains(name)
            })
        {
            return Some(self.read(|this, arena| {
                &**arena.scope_view(this, self.scope) as &dyn StaticTypedScope
            }));
        }
        match self.get_parent_scope() {
            None => None,
            // Recurse on the parent because it, too, may be incomplete.
            Some(parent) => parent.get_topmost_scope_of_eventual_declaration(reg, ast, name),
        }
    }
}
