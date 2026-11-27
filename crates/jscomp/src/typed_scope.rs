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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/AbstractScope.java,
//   src/com/google/javascript/jscomp/TypedScope.java,
//   src/com/google/javascript/jscomp/TypedVar.java.

//! TypedScope contains information about variables and their types. Scopes can be nested; a scope
//! points back to its parent scope.
//!
//! Scopes are handles into the compiler-owned `TypedScopeArena` (DESIGN "Scopes and
//! NodeTraversal"): handle equality is Java object identity, and the common AbstractScope bodies run
//! through the `AbstractScope` trait exactly as for syntactic scopes.
use crate::scope::NameArg;
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
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypedScope(pub(crate) NonZeroU32);

#[derive(Debug)]
pub(crate) struct TypedScopeData {
    pub(crate) abstract_scope: AbstractScopeData<TypedVar>,
    pub(crate) parent: Option<TypedScope>,
    pub(crate) module: Option<Arc<Module>>,
    /// Whether this is a bottom scope for the purposes of type inference.
    pub(crate) is_bottom: bool,
    // Will shrink over time. Java swaps in ImmutableSet.of() once empty.
    pub(crate) reserved_names: ReservedNames,
    /// Rust-only (performance divergence): root prefixes (before first `.`) of qualified variable
    /// names declared in `abstract_scope.vars`, allowing `TypeInference::get_property_type` to
    /// skip allocating `get_qualified_name` and probing scopes when no qualified var exists.
    pub(crate) qualified_roots: IndexSet<JsString>,
    /// Rust-only (performance divergence): whether any qualified var starting with `"this."` was
    /// declared in this scope.
    pub(crate) has_this_qualified: bool,
    /// Rust-only: the canonical closure-jstype view (Java: this object as a StaticTypedScope),
    /// created on first use and never freed (PORT_NOTES, "TypedScope and TypedVar views").
    pub(crate) view: OnceLock<&'static Arc<TypedScopeView>>,
}

/// Java's `LinkedHashSet<String> reservedNames` of a TypedScope: filled when the scope is made,
/// afterwards only shrinks (`declare`) and answers `contains`.
///
/// Rust-only (performance divergence): Java unlinks a removed name; here the name keeps its slot
/// and is marked removed, because removing from the middle of an `IndexSet` in order moves every
/// later entry (a module scope reserves thousands of names and declares them one by one, which
/// made each declaration linear). Names are never added after construction, so the live names
/// keep Java's insertion order: membership, emptiness and the order of the names listed by
/// `validateCompletelyBuilt` are Java's.
#[derive(Debug, Default, Clone)]
pub(crate) struct ReservedNames {
    names: IndexSet<JsString>,
    removed: Vec<bool>,
    live: usize,
}

impl ReservedNames {
    pub(crate) fn new(names: IndexSet<JsString>) -> Self {
        Self {
            live: names.len(),
            removed: vec![false; names.len()],
            names,
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub(crate) fn contains(&self, name: &JsString) -> bool {
        self.live != 0
            && self
                .names
                .get_index_of(name)
                .is_some_and(|index| !self.removed[index])
    }

    pub(crate) fn remove(&mut self, name: &JsString) {
        if let Some(index) = self.names.get_index_of(name)
            && !self.removed[index]
        {
            self.removed[index] = true;
            self.live -= 1;
            if self.live == 0 {
                *self = Self::default();
            }
        }
    }

    /// The names not removed, in insertion order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &JsString> {
        self.names
            .iter()
            .zip(&self.removed)
            .filter_map(|(name, &removed)| (!removed).then_some(name))
    }

    pub(crate) fn len(&self) -> usize {
        self.live
    }
}

/// Rust-only (performance divergence): lock-free copies of a typed scope's fields that never change
/// after construction (see `scope::ScopeMirror`), in `Compiler::typed_scope_mirror`, index-aligned
/// with `TypedScopeArena::scopes`.
#[derive(Debug)]
pub(crate) struct TypedScopeMeta {
    root_node: NodeId,
    parent: Option<TypedScope>,
    depth: i32,
    view: OnceLock<&'static Arc<TypedScopeView>>,
}

/// Rust-only owner for Java TypedScope and TypedVar object identities; slots are never removed.
///
/// Rust-only (performance divergence): while the compiler has a type registry, the arena lives in
/// the registry's scope storage (`JSTypeRegistry::scope_storage`): the canonical closure-jstype
/// views of a scope or var (Java: the same object), which the registry keeps and consults without
/// the compiler, read it through the registry every lookup passes them, and the compiler reads and
/// writes it there without a lock (`Compiler::typed_scope_arena`). Before the registry exists and
/// after it is cleared, the compiler keeps it in a field. Java's views are the objects themselves.
#[derive(Debug, Default)]
pub struct TypedScopeArena {
    pub(crate) scopes: Vec<TypedScopeData>,
    pub(crate) vars: Vec<TypedVarData>,
    pub(crate) watch: VarWatch,
}

/// Rust-only (performance divergence): change detection for a cache of `TypeInference` that
/// reads vars (`InferenceTransfer::declare_module_imports`). The epoch advances whenever the
/// type of a watched var changes, a watched var is replaced in or removed from its scope's
/// declarations, a scope's declarations are cleared, or a caller reports another change
/// (`bump`). Watching never ends: a stale watch only costs a cache miss.
#[derive(Debug, Default)]
pub(crate) struct VarWatch {
    epoch: u64,
    watched: Vec<u64>,
}

impl VarWatch {
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) fn watch(&mut self, var: TypedVar) {
        let (word, bit) = (var.index() / 64, var.index() % 64);
        if self.watched.len() <= word {
            self.watched.resize(word + 1, 0);
        }
        self.watched[word] |= 1 << bit;
    }

    #[inline]
    pub(crate) fn is_watched(&self, var: TypedVar) -> bool {
        self.watched
            .get(var.index() / 64)
            .is_some_and(|word| word & (1 << (var.index() % 64)) != 0)
    }

    /// Notes a change of `var`.
    #[inline]
    pub(crate) fn touched(&mut self, var: TypedVar) {
        if self.is_watched(var) {
            self.epoch += 1;
        }
    }

    pub(crate) fn bump(&mut self) {
        self.epoch += 1;
    }
}

impl TypedScopeArena {
    #[inline]
    pub(crate) fn read(compiler: &AbstractCompiler) -> &TypedScopeArena {
        compiler.typed_scope_arena()
    }

    #[inline]
    pub(crate) fn write(compiler: &mut AbstractCompiler) -> &mut TypedScopeArena {
        compiler.typed_scope_arena_mut()
    }

    /// Rust-only: the arena a scope view reads, in the scope storage of the registry it is
    /// consulted through.
    #[inline]
    pub(crate) fn of_registry(reg: &JSTypeRegistry) -> &TypedScopeArena {
        reg.scope_storage()
            .and_then(|storage| storage.downcast_ref())
            .expect("a TypedScope view is consulted through its compiler's type registry")
    }

    /// Rust-only: `of_registry`, mutably (a view that creates an implicit var).
    fn of_registry_mut(reg: &mut JSTypeRegistry) -> &mut TypedScopeArena {
        reg.scope_storage_mut()
            .and_then(|storage| storage.downcast_mut())
            .expect("a TypedScope view is consulted through its compiler's type registry")
    }
}

impl TypedScope {
    pub(crate) fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    #[inline]
    pub(crate) fn data(self, compiler: &AbstractCompiler) -> &TypedScopeData {
        &TypedScopeArena::read(compiler).scopes[self.index()]
    }

    #[inline]
    pub(crate) fn data_mut(self, compiler: &mut AbstractCompiler) -> &mut TypedScopeData {
        &mut TypedScopeArena::write(compiler).scopes[self.index()]
    }

    // port: TypedScope#TypedScope(TypedScope, Node)
    pub fn new(compiler: &mut AbstractCompiler, parent: TypedScope, root_node: NodeId) -> Self {
        Self::new_child(compiler, parent, root_node, ReservedNames::default(), None)
    }

    // port: TypedScope#TypedScope(TypedScope, Node, Set, Module)
    pub fn new_with_reserved_names(
        compiler: &mut AbstractCompiler,
        parent: TypedScope,
        root_node: NodeId,
        reserved_names: &IndexSet<JsString>,
        module: Option<Arc<Module>>,
    ) -> Self {
        Self::new_child(
            compiler,
            parent,
            root_node,
            ReservedNames::new(reserved_names.clone()),
            module,
        )
    }

    pub(crate) fn new_child(
        compiler: &mut AbstractCompiler,
        parent: TypedScope,
        root_node: NodeId,
        reserved_names: ReservedNames,
        module: Option<Arc<Module>>,
    ) -> Self {
        let scope = Self::allocate(
            compiler,
            root_node,
            Some(parent),
            reserved_names,
            module,
            false,
        );
        scope.check_child_scope(compiler, parent);
        scope
    }

    // port: TypedScope#TypedScope(Node, boolean)
    fn new_root(compiler: &mut AbstractCompiler, root_node: NodeId, is_bottom: bool) -> Self {
        let scope = Self::allocate(
            compiler,
            root_node,
            None,
            ReservedNames::default(),
            None,
            is_bottom,
        );
        scope.check_root_scope(compiler);
        scope
    }

    fn allocate(
        compiler: &mut AbstractCompiler,
        root_node: NodeId,
        parent: Option<TypedScope>,
        reserved_names: ReservedNames,
        module: Option<Arc<Module>>,
        is_bottom: bool,
    ) -> Self {
        let depth = parent.map_or(0, |parent| parent.get_depth(compiler) + 1);
        let scopes = &mut TypedScopeArena::write(compiler).scopes;
        let scope = Self(NonZeroU32::new(u32::try_from(scopes.len() + 1).unwrap()).unwrap());
        scopes.push(TypedScopeData {
            abstract_scope: AbstractScopeData::new(root_node),
            parent,
            module,
            is_bottom,
            reserved_names,
            qualified_roots: IndexSet::default(),
            has_this_qualified: false,
            view: OnceLock::new(),
        });
        compiler.typed_scope_mirror.push(TypedScopeMeta {
            root_node,
            parent,
            depth,
            view: OnceLock::new(),
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
        let scope = self.data_mut(compiler);
        if scope.reserved_names.is_empty() {
            scope.reserved_names = ReservedNames::default();
            return;
        }
        let remaining = scope
            .reserved_names
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        check_state!(
            false,
            "Expected %s to have no reserved names, found: %s. This probably indicates a bug in TypedScopeCreator where it is failing to declare a variable.",
            self.to_string(compiler),
            format!("[{remaining}]")
        );
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
            self.data_mut(compiler).reserved_names.remove(&name);
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
    pub fn get_var(self, compiler: &mut AbstractCompiler, name: impl NameArg) -> Option<TypedVar> {
        // Rust-only: the name is read by reference (performance divergence).
        name.with_name(|name| self.get_var_by_ref(compiler, name))
    }

    // port: TypedScope#getVar
    fn get_var_by_ref(self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<TypedVar> {
        // Rust-only (performance divergence): inspect ordinary bindings in one arena borrow.
        // getOwnSlot can create an implicit variable, so those paths take the compiler again.
        let arena = TypedScopeArena::read(compiler);
        if let Some(&var) = arena.scopes[self.index()].abstract_scope.vars.get(name) {
            return Some(var);
        }
        if ImplicitVar::of(name).is_some() {
            return abstract_scope_get_var(self, compiler, name);
        }
        let parent = self.get_parent(compiler)?;
        let dot = name.index_of_char(u16::from(b'.'));
        if dot < 0 {
            // The current scope was checked above.
            return arena.find_declared_var(parent, name).0;
        }
        let root_name = name.substring(0, dot as usize);
        if ImplicitVar::of(&root_name).is_some() {
            let owner = abstract_scope_get_var(self, compiler, &root_name).map_or_else(
                || self.get_global_scope(compiler),
                |var| var.get_scope(compiler),
            );
            return owner.get_own_slot(compiler, name);
        }

        let (root_var, last_scope) = arena.find_declared_var(self, &root_name);
        // An undeclared root uses the global scope. When found, the slot's live owner may
        // differ from the scope where the binding was encountered.
        let owner = root_var.map_or(last_scope, |var| {
            closure_rhino::check_not_null!(arena.vars[var.index()].abstract_var.scope)
        });
        arena.scopes[owner.index()]
            .abstract_scope
            .vars
            .get(name)
            .copied()
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

    /// Rust-only (performance divergence): under one arena read, check whether any scope on the
    /// enclosing chain from `self` to global (or the owning scope of `root_name`) has any declared
    /// qualified var starting with `root_name.`, and also return the `TypedVar` for `root_name` if
    /// found.
    pub(crate) fn may_have_qualified_name(
        self,
        compiler: &AbstractCompiler,
        root_name: &JsString,
    ) -> (bool, Option<TypedVar>) {
        let arena = TypedScopeArena::read(compiler);
        let mut cur = self;
        let mut root_var = None;
        let mut has_qualified = false;
        loop {
            let scope_data = &arena.scopes[cur.index()];
            if !has_qualified && scope_data.qualified_roots.contains(root_name) {
                has_qualified = true;
            }
            if root_var.is_none()
                && let Some(&var) = scope_data.abstract_scope.vars.get(root_name)
            {
                root_var = Some(var);
            }
            match scope_data.parent {
                Some(parent) => cur = parent,
                None => break,
            }
        }
        if !has_qualified && let Some(var) = root_var {
            let owner = closure_rhino::check_not_null!(arena.vars[var.index()].abstract_var.scope);
            has_qualified = arena.scopes[owner.index()]
                .qualified_roots
                .contains(root_name);
        }
        (has_qualified, root_var)
    }

    /// Rust-only (performance divergence): under one arena read, check whether any scope on the
    /// enclosing chain from `self` to global (or the `this` scope) has any declared qualified var
    /// starting with `"this."`.
    pub(crate) fn may_have_this_qualified_name(self, compiler: &AbstractCompiler) -> bool {
        let this_scope = self.get_scope_of_this(compiler);
        let arena = TypedScopeArena::read(compiler);
        if arena.scopes[this_scope.index()].has_this_qualified {
            return true;
        }
        let mut cur = self;
        loop {
            let scope_data = &arena.scopes[cur.index()];
            if scope_data.has_this_qualified {
                return true;
            }
            match scope_data.parent {
                Some(parent) => cur = parent,
                None => return false,
            }
        }
    }

    // port: TypedScope#getTopmostScopeOfEventualDeclaration
    pub fn get_topmost_scope_of_eventual_declaration(
        self,
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
    ) -> Option<TypedScope> {
        let name = name.into();
        if self.get_own_slot(compiler, &name).is_some()
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

    type DataRef<'a> = &'a AbstractScopeData<TypedVar>;
    type DataMut<'a> = &'a mut AbstractScopeData<TypedVar>;

    #[inline]
    fn scope_data(self, compiler: &AbstractCompiler) -> Self::DataRef<'_> {
        &self.data(compiler).abstract_scope
    }

    #[inline]
    fn scope_data_mut(self, compiler: &mut AbstractCompiler) -> Self::DataMut<'_> {
        &mut self.data_mut(compiler).abstract_scope
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
        TypedScope::get_var(self, compiler, name)
    }
    fn get_topmost_scope_of_eventual_declaration(
        self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
    ) -> Option<TypedScope> {
        TypedScope::get_topmost_scope_of_eventual_declaration(self, compiler, name.clone())
    }
    fn declare_internal(self, compiler: &mut AbstractCompiler, name: JsString, var: TypedVar) {
        let dot = name.index_of_char(u16::from(b'.'));
        let arena = TypedScopeArena::write(compiler);
        let scope = &mut arena.scopes[self.index()];
        if dot > 0 {
            let root = name.substring(0, dot as usize);
            if root == "this" {
                scope.has_this_qualified = true;
            }
            scope.qualified_roots.insert(root);
        }
        if let Some(replaced) = scope.abstract_scope.vars.insert(name, var) {
            arena.watch.touched(replaced);
        }
    }
    // port: AbstractScope#undeclareInteral
    fn undeclare_interal(self, compiler: &mut AbstractCompiler, var: TypedVar) {
        let name = var.get_name(compiler);
        let arena = TypedScopeArena::write(compiler);
        if let Some(removed) = arena.scopes[self.index()]
            .abstract_scope
            .vars
            .shift_remove(&name)
        {
            arena.watch.touched(removed);
        }
    }
    fn clear_vars_internal(self, compiler: &mut AbstractCompiler) {
        TypedScopeArena::write(compiler).watch.bump();
        let scope = self.data_mut(compiler);
        scope.abstract_scope.vars.clear();
        scope.abstract_scope.implicit_vars.clear();
        scope.qualified_roots.clear();
        scope.has_this_qualified = false;
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
        pub fn $name(self, compiler: &AbstractCompiler, name: impl NameArg) -> $result {
            name.with_name(|name| <Self as AbstractScope>::$name(self, compiler, name))
        }
    };
}
macro_rules! scope_name_mutator {
    ($name:ident, $result:ty) => {
        pub fn $name(self, compiler: &mut AbstractCompiler, name: impl NameArg) -> $result {
            name.with_name(|name| <Self as AbstractScope>::$name(self, compiler, name))
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
/// in its arena entry. The view holds its scope's handle and the fields fixed at construction (the
/// root, and the parent's view, which is made with it); it is leaked (one small allocation per
/// scope the registry touches; PORT_NOTES, "TypedScope and TypedVar views").
/// Its methods are read-only twins of the compiler-side TypedScope methods: they read the arena in
/// the scope storage of the registry they are consulted through (`TypedScopeArena::of_registry`),
/// and an implicit var (`this`, `arguments`, `super`, `exports`) the compiler side has not
/// materialised yet is absent (JSDoc type names never start with these words).
#[derive(Debug)]
pub struct TypedScopeView {
    scope: TypedScope,
    root: NodeId,
    parent: Option<&'static Arc<TypedScopeView>>,
}

impl TypedScopeArena {
    /// Rust-only: the canonical view of `scope` (and of its ancestors), created on first use.
    pub(crate) fn scope_view(&self, scope: TypedScope) -> &'static Arc<TypedScopeView> {
        let data = &self.scopes[scope.index()];
        if let Some(view) = data.view.get() {
            return view;
        }
        let parent = data.parent.map(|parent| self.scope_view(parent));
        data.view.get_or_init(|| {
            Box::leak(Box::new(Arc::new(TypedScopeView {
                scope,
                root: data.abstract_scope.root_node,
                parent,
            })))
        })
    }

    /// Rust-only (performance divergence): `TypedScope::get_var` of the dotted name `key` from
    /// `scope`, without making a string of the name. `root_var` is the binding that
    /// `getVar` finds for the name's first component from `scope` (`declared_var`, or
    /// `existing_this_var` for `this`). The lookup is that of `get_var_by_ref` for a dotted name:
    /// the scope's own names, then the names of the scope that owns the root's binding (the
    /// global scope when the root has none). Like that path, it creates nothing.
    pub(crate) fn dotted_var(
        &self,
        scope: TypedScope,
        root_var: Option<TypedVar>,
        key: &crate::var_map::UnitsKey<'_>,
    ) -> Option<TypedVar> {
        let data = &self.scopes[scope.index()];
        if let Some(&var) = data.abstract_scope.vars.get_units(key) {
            return Some(var);
        }
        let mut owner = data.parent?;
        match root_var {
            Some(var) => owner = self.var_scope(var),
            None => {
                while let Some(parent) = self.scopes[owner.index()].parent {
                    owner = parent;
                }
            }
        }
        self.scopes[owner.index()]
            .abstract_scope
            .vars
            .get_units(key)
            .copied()
    }

    /// Rust-only (performance divergence): the binding `TypedScope::get_var("this")` finds from
    /// `scope` when that lookup would create nothing: the first scope on the chain that declares
    /// `this` or makes it implicitly (`AbstractScope#getOwnSlot`); `Some(None)` when there is
    /// none. `None` when that scope has not made its implicit `this` yet: looking it up would
    /// make it.
    pub(crate) fn existing_this_var(
        &self,
        compiler: &AbstractCompiler,
        mut scope: TypedScope,
        this_key: &crate::var_map::UnitsKey<'_>,
    ) -> Option<Option<TypedVar>> {
        loop {
            let data = &self.scopes[scope.index()].abstract_scope;
            if let Some(&var) = data.vars.get_units(this_key) {
                return Some(Some(var));
            }
            if ImplicitVar::THIS.is_made_by_scope(compiler, scope) {
                return data
                    .implicit_vars
                    .get(&ImplicitVar::THIS)
                    .map(|&var| Some(var));
            }
            match self.scopes[scope.index()].parent {
                Some(parent) => scope = parent,
                None => return Some(None),
            }
        }
    }

    /// The binding `TypedScope::get_var` finds for the plain (not implicit, not dotted) name
    /// `name` from `scope`, read without effect.
    pub(crate) fn declared_var(&self, scope: TypedScope, name: &JsString) -> Option<TypedVar> {
        self.find_declared_var(scope, name).0
    }

    /// `TypedVar::get_scope`, read in this arena.
    pub(crate) fn var_scope(&self, var: TypedVar) -> TypedScope {
        closure_rhino::check_not_null!(self.vars[var.index()].abstract_var.scope)
    }

    /// Find an explicit lexical binding and the last scope examined (global on a miss).
    fn find_declared_var(
        &self,
        mut scope: TypedScope,
        name: &JsString,
    ) -> (Option<TypedVar>, TypedScope) {
        loop {
            let data = &self.scopes[scope.index()];
            if let Some(&var) = data.abstract_scope.vars.get(name) {
                return (Some(var), scope);
            }
            let Some(parent) = data.parent else {
                return (None, scope);
            };
            scope = parent;
        }
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
        compiler.typed_scope_mirror[self.index()]
            .view
            .get_or_init(|| TypedScopeArena::read(compiler).scope_view(self))
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
    fn read<R>(&self, reg: &JSTypeRegistry, f: impl FnOnce(&TypedScopeArena) -> R) -> R {
        f(TypedScopeArena::of_registry(reg))
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
        let owner = self.read(reg, |arena| {
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
        let missing = self.read(reg, |arena| {
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
    /// arena is borrowed mutably.
    fn make_implicit_var_in(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        owner: TypedScope,
        implicit: ImplicitVar,
    ) {
        let type_ = self
            .read(reg, |arena| arena.scope_view(owner))
            .get_implicit_var_type(reg, ast, implicit);
        let arena = TypedScopeArena::of_registry_mut(reg);
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
        let var = TypedVar::push(arena, false, abstract_var, type_);
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
                self.read(reg, |arena| {
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
        // Like the parent, the root is fixed when the view is created.
        Some(self.root)
    }

    // port: AbstractScope#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        self.parent.map(|parent| &**parent as &dyn StaticTypedScope)
    }

    // port: AbstractScope#getSlot
    fn get_slot(&self, reg: &JSTypeRegistry, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.read(reg, |arena| {
            arena
                .view_get_var(self.scope, name)
                .map(|var| arena.var_view(var) as &dyn StaticTypedSlot)
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
        self.get_slot(reg, name)
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
        self.get_own_slot(reg, name)
    }

    // port: AbstractScope#getOwnSlot
    fn get_own_slot(&self, reg: &JSTypeRegistry, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.read(reg, |arena| {
            arena
                .view_get_own_slot(self.scope, name)
                .map(|var| arena.var_view(var) as &dyn StaticTypedSlot)
        })
    }

    // port: TypedScope#getTypeOfThis
    fn get_type_of_this(&self, reg: &JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        let root = self.get_root_node().unwrap();
        let parent_view = || self.parent;
        if self.parent.is_none() {
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
        // Ordinary type names only consult declarations and reservations. Both belong to
        // this arena, so the complete scope chain is read in one pass.
        if ImplicitVar::of(name).is_none() {
            return self.read(reg, |arena| {
                let mut scope = Some(self.scope);
                while let Some(current) = scope {
                    let data = &arena.scopes[current.index()];
                    if data.abstract_scope.vars.contains_key(name)
                        || data.reserved_names.contains(name)
                    {
                        return Some(&**arena.scope_view(current) as &dyn StaticTypedScope);
                    }
                    scope = data.parent;
                }
                None
            });
        }
        if self
            .get_own_slot_creating_implicit_vars(reg, ast, name)
            .is_some()
            || self.read(reg, |arena| {
                arena.scopes[self.scope.index()]
                    .reserved_names
                    .contains(name)
            })
        {
            return Some(self.read(reg, |arena| {
                &**arena.scope_view(self.scope) as &dyn StaticTypedScope
            }));
        }
        match self.get_parent_scope() {
            None => None,
            // Recurse on the parent because it, too, may be incomplete.
            Some(parent) => parent.get_topmost_scope_of_eventual_declaration(reg, ast, name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::Compiler;
    use closure_rhino::ir::IR;

    #[test]
    fn canonical_scope_views_keep_their_root_and_observe_later_bindings() {
        let mut compiler = Compiler::new();
        let script = compiler.parse_test_code("function f() {}");
        let global = TypedScope::create_global_scope(&mut compiler, script);
        let function = script.get_only_child(&compiler);
        let local = TypedScope::new(&mut compiler, global, function);
        let view = local.as_static_typed_scope_arc(&compiler);
        assert_eq!(view.get_root_node(), Some(function));
        assert!(std::ptr::addr_eq(
            Arc::as_ptr(&view),
            local.as_static_typed_scope(&compiler)
        ));
        assert!(std::ptr::addr_eq(
            view.get_parent_scope().unwrap(),
            global.as_static_typed_scope(&compiler),
        ));
        let name = JsString::from("later");
        assert!(view.get_slot(compiler.get_type_registry(), &name).is_none());
        global.declare(&mut compiler, name.clone(), None, None, None, true);
        assert!(view.get_slot(compiler.get_type_registry(), &name).is_some());
    }

    #[test]
    fn qualified_lookup_uses_the_live_owner_of_the_root_binding() {
        let mut compiler = Compiler::new();
        let script = compiler.parse_test_code("function f() { let y; }");
        let global = TypedScope::create_global_scope(&mut compiler, script);
        let function = script.get_only_child(&compiler);
        let local = TypedScope::new(&mut compiler, global, function);
        let body = function.get_last_child(&compiler).unwrap();
        let block = TypedScope::new(&mut compiler, local, body);

        let outer = global.declare(&mut compiler, "ns", None, None, None, true);
        let global_prop = global.declare(&mut compiler, "ns.x", None, None, None, true);
        let inner = local.declare(&mut compiler, "ns", None, None, None, true);
        let local_prop = local.declare(&mut compiler, "ns.x", None, None, None, true);
        assert_eq!(Some(local_prop), block.get_var(&mut compiler, "ns.x"));

        // A binding can point to a slot owned by another scope.
        local
            .scope_data_mut(&mut compiler)
            .vars
            .insert("ns".into(), outer);
        assert_eq!(Some(outer), block.get_var(&mut compiler, "ns"));
        assert_eq!(Some(global_prop), block.get_var(&mut compiler, "ns.x"));
        assert_eq!(Some(local_prop), local.get_var(&mut compiler, "ns.x"));

        local
            .scope_data_mut(&mut compiler)
            .vars
            .insert("ns".into(), inner);
        local.undeclare(&mut compiler, inner);
        global.undeclare(&mut compiler, outer);
        assert_eq!(Some(global_prop), block.get_var(&mut compiler, "ns.x"));
    }

    #[test]
    fn declaration_view_follows_reservations_and_live_shadowing() {
        let mut compiler = Compiler::new();
        let root = IR::root(&mut compiler.ast, &[]);
        let global = TypedScope::create_global_scope(&mut compiler, root);
        let name = IR::name(&mut compiler.ast, "f");
        let params = IR::param_list(&mut compiler.ast, &[]);
        let body = IR::block(&mut compiler.ast);
        let function = IR::function(&mut compiler.ast, name, params, body);
        let reserved = IndexSet::from_iter([JsString::from("T")]);
        let child =
            TypedScope::new_with_reserved_names(&mut compiler, global, function, &reserved, None);
        let global_view = global.as_static_typed_scope(&compiler);
        let child_view = child.as_static_typed_scope(&compiler);
        assert_eq!(child_view.get_root_node(), Some(function));
        assert!(std::ptr::addr_eq(
            child_view.get_parent_scope().unwrap(),
            global_view
        ));

        let declaration = |compiler: &mut Compiler| {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            child_view.get_topmost_scope_of_eventual_declaration(reg, ast, &JsString::from("T"))
        };
        let outer = global.declare(&mut compiler, "T", None, None, None, true);
        assert!(std::ptr::addr_eq(
            declaration(&mut compiler).unwrap(),
            child_view
        ));
        let inner = child.declare(&mut compiler, "T", None, None, None, true);
        assert!(std::ptr::addr_eq(
            declaration(&mut compiler).unwrap(),
            child_view
        ));
        child.undeclare(&mut compiler, inner);
        assert!(std::ptr::addr_eq(
            declaration(&mut compiler).unwrap(),
            global_view
        ));
        global.undeclare(&mut compiler, outer);
        assert!(declaration(&mut compiler).is_none());
    }

    #[test]
    fn scope_view_reads_its_arena_only_through_its_compilers_registry() {
        let mut compiler = Compiler::new();
        let root = IR::root(&mut compiler.ast, &[]);
        let global = TypedScope::create_global_scope(&mut compiler, root);
        // Declared before the registry exists: the compiler then holds the arena itself.
        global.declare(&mut compiler, "early", None, None, None, true);
        let view = global.as_static_typed_scope_arc(&compiler);
        assert!(view.get_parent_scope().is_none());
        assert!(
            view.get_slot(compiler.get_type_registry(), &JsString::from("early"))
                .is_some()
        );
        let mut other_ast = Ast::new();
        let other = JSTypeRegistry::new(
            &mut other_ast,
            Box::new(closure_rhino::error_reporter::NullErrorReporter),
            Vec::new(),
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                view.get_slot(&other, &JsString::from("early")).is_some()
            }))
            .is_err()
        );
    }
}
