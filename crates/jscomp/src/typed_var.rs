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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/AbstractVar.java,
//   src/com/google/javascript/jscomp/TypedVar.java.

//! `AbstractVar` subclass for use with `TypedScope`.
//!
//! Note that this class inherits its `equals` and `hashCode` implementations from `ScopedName`,
//! which does not include any type information. This is necessary because `Var`-keyed maps are used
//! across multiple top scopes, but it comes with the caveat that if `TypedVar` instances are stored
//! in a set, the type information is at risk of disappearing if an untyped (or differently typed) var
//! is added for the same symbol.
//!
//! Handle equality is Java object identity; `equals`/`hash_code` keep ScopedName's semantics.
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_var::{AbstractVar, AbstractVarData},
    compiler_input::CompilerInput,
    scoped_name::ScopedName,
    typed_scope::{TypedArenaMut, TypedArenaRef, TypedScope, TypedScopeArena},
};
use closure_jstype::{
    JSTypeRegistry, TypeId, prelude::JSType, static_typed_ref::StaticTypedRef,
    static_typed_scope::StaticTypedScope, static_typed_slot::StaticTypedSlot,
};
use closure_rhino::{
    check_argument, check_not_null,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
    token::Token,
};
use std::{
    num::NonZeroU32,
    sync::{Arc, OnceLock, PoisonError, RwLock, Weak},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypedVar(pub(crate) NonZeroU32);

#[derive(Debug)]
pub(crate) struct TypedVarData {
    pub(crate) abstract_var: AbstractVarData<TypedScope>,
    pub(crate) type_: Option<TypeId>,
    // The next two fields and the associated methods are only used by
    // TypeInference.java. Maybe there is a way to avoid having them in all typed variable instances.
    pub(crate) marked_escaped: bool,
    pub(crate) marked_assigned_exactly_once: bool,
    /// Whether the variable's type has been inferred or is declared. An inferred type may change
    /// over time (as more code is discovered), whereas a declared type is a static contract that
    /// must be matched.
    pub(crate) type_inferred: bool,
    /// Rust-only: the canonical closure-jstype view (Java: this object as a StaticTypedSlot),
    /// created on first use and never freed (PORT_NOTES, "TypedScope and TypedVar views").
    pub(crate) view: OnceLock<&'static TypedVarView>,
}

// includes nodes that in plain JS semantics are not 'declarations', but that the type system
// & compiler treat as declarations.
// port: TypedVar#NAME_NODE_TYPES
const NAME_NODE_TYPES: [Token; 12] = [
    Token::NAME,
    Token::THIS, // `if ('prop' in this) {` creates a TypedVar `this.prop` requiring a node
    Token::IMPORT_STAR,
    Token::EXPR_RESULT, // implicit variables from goog.provide
    Token::EXPORT,      // tracks the *default* export
    Token::GETPROP,
    Token::FUNCTION,
    Token::STRING_KEY,
    Token::SETTER_DEF,
    Token::GETTER_DEF,
    Token::MEMBER_FUNCTION_DEF,
    Token::MODULE_BODY, // for the implicit exports object in a module
];

impl TypedVar {
    pub(crate) fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    pub(crate) fn data(self, compiler: &AbstractCompiler) -> TypedArenaRef<'_, TypedVarData> {
        TypedArenaRef::new(TypedScopeArena::read(compiler), self.index(), |a, i| {
            &a.vars[i]
        })
    }

    pub(crate) fn data_mut(
        self,
        compiler: &mut AbstractCompiler,
    ) -> TypedArenaMut<'_, TypedVarData> {
        TypedArenaMut::new(
            TypedScopeArena::write(compiler),
            self.index(),
            |a, i| &a.vars[i],
            |a, i| &mut a.vars[i],
        )
    }

    // port: TypedVar#TypedVar
    #[allow(clippy::too_many_arguments)] // Java constructor arity.
    pub fn new(
        compiler: &mut AbstractCompiler,
        inferred: bool,
        name: impl Into<JsString>,
        name_node: Option<NodeId>,
        type_: Option<TypeId>,
        scope: TypedScope,
        index: i32,
        input: Option<CompilerInput>,
    ) -> Self {
        let abstract_var = AbstractVarData::new(
            compiler,
            name.into(),
            name_node,
            Some(scope),
            index,
            input,
            /* implicitGoogNamespaceDefinition= */ None,
        );
        if let Some(name_node) = name_node {
            check_argument!(
                NAME_NODE_TYPES.contains(&name_node.get_token(compiler)),
                "Invalid name node token %s",
                name_node.get_token(compiler)
            );
        }
        let mut arena = TypedScopeArena::write(compiler);
        Self::push(&mut arena, inferred, abstract_var, type_)
    }

    /// Rust-only: the arena half of `TypedVar#TypedVar` (the new var's slot), shared with the
    /// scope views that create implicit vars (`TypedScopeView::get_slot_creating_implicit_vars`).
    pub(crate) fn push(
        arena: &mut TypedScopeArena,
        inferred: bool,
        abstract_var: AbstractVarData<TypedScope>,
        type_: Option<TypeId>,
    ) -> Self {
        let vars = &mut arena.vars;
        let var = Self(NonZeroU32::new(u32::try_from(vars.len() + 1).unwrap()).unwrap());
        vars.push(TypedVarData {
            abstract_var,
            type_,
            marked_escaped: false,
            marked_assigned_exactly_once: false,
            type_inferred: inferred,
            view: OnceLock::new(),
        });
        var
    }

    /// Gets this variable's type. To know whether this type has been inferred, see
    /// `#isTypeInferred()`.
    // port: TypedVar#getType
    pub fn get_type(self, compiler: &AbstractCompiler) -> Option<TypeId> {
        self.data(compiler).type_
    }

    // port: TypedVar#setType
    pub fn set_type(self, compiler: &mut AbstractCompiler, type_: Option<TypeId>) {
        self.data_mut(compiler).type_ = type_;
    }

    // port: TypedVar#resolveType
    /// The registry owns its error reporter (DESIGN §8), so Java's `errorReporter` argument is the
    /// registry's.
    pub fn resolve_type(self, compiler: &mut AbstractCompiler) {
        let type_ = self.data(compiler).type_;
        if let Some(type_) = type_ {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let resolved = type_.resolve(reg, ast);
            self.data_mut(compiler).type_ = Some(resolved);
        }
    }

    /// Returns whether this variable's type is inferred. To get the variable's type, see
    /// `#getType()`.
    // port: TypedVar#isTypeInferred
    pub fn is_type_inferred(self, compiler: &AbstractCompiler) -> bool {
        self.data(compiler).type_inferred
    }

    // port: TypedVar#getInputName
    pub fn get_input_name(self, compiler: &AbstractCompiler) -> String {
        match self.get_input(compiler) {
            None => "<non-file>".to_owned(),
            Some(input) => input.get_name().to_owned(),
        }
    }

    // port: TypedVar#toString
    pub fn to_string(self, compiler: &mut AbstractCompiler) -> String {
        let name = self.get_name(compiler);
        let type_ = self.data(compiler).type_;
        let type_string = match type_ {
            None => "null".to_owned(),
            Some(t) => {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                t.to_string(reg, ast)
            }
        };
        format!("Var {name}{{{type_string}}}")
    }

    // port: TypedVar#markEscaped
    pub fn mark_escaped(self, compiler: &mut AbstractCompiler) {
        self.data_mut(compiler).marked_escaped = true;
    }

    // port: TypedVar#isMarkedEscaped
    pub fn is_marked_escaped(self, compiler: &AbstractCompiler) -> bool {
        self.data(compiler).marked_escaped
    }

    // port: TypedVar#markAssignedExactlyOnce
    pub fn mark_assigned_exactly_once(self, compiler: &mut AbstractCompiler) {
        self.data_mut(compiler).marked_assigned_exactly_once = true;
    }

    // port: TypedVar#isMarkedAssignedExactlyOnce
    pub fn is_marked_assigned_exactly_once(self, compiler: &AbstractCompiler) -> bool {
        self.data(compiler).marked_assigned_exactly_once
    }
}

impl AbstractVar for TypedVar {
    type Scope = TypedScope;

    type DataRef<'a> = TypedArenaRef<'a, AbstractVarData<TypedScope>>;
    type DataMut<'a> = TypedArenaMut<'a, AbstractVarData<TypedScope>>;

    fn var_data(self, compiler: &AbstractCompiler) -> Self::DataRef<'_> {
        TypedArenaRef::new(TypedScopeArena::read(compiler), self.index(), |a, i| {
            &a.vars[i].abstract_var
        })
    }

    fn var_data_mut(self, compiler: &mut AbstractCompiler) -> Self::DataMut<'_> {
        TypedArenaMut::new(
            TypedScopeArena::write(compiler),
            self.index(),
            |a, i| &a.vars[i].abstract_var,
            |a, i| &mut a.vars[i].abstract_var,
        )
    }

    /// Rust-only: AbstractVar calls this only to build its precondition (internal error) messages,
    /// where it holds the compiler shared, but closure-jstype's `JSType#toString` needs the registry
    /// mutably. These messages keep Java's `Var name{type}` shape with the type's arena id; every
    /// other caller uses the inherent `TypedVar::to_string` (Java's TypedVar#toString).
    fn to_string(self, compiler: &AbstractCompiler) -> String {
        format!(
            "Var {}{{{}}}",
            self.get_name(compiler),
            self.data(compiler)
                .type_
                .map_or_else(|| "null".to_owned(), |t| format!("{t:?}"))
        )
    }
}

impl ScopedName for TypedVar {
    fn get_name(&self, compiler: &AbstractCompiler) -> JsString {
        TypedVar::get_name(*self, compiler)
    }

    fn get_scope_root(&self, compiler: &AbstractCompiler) -> Option<NodeId> {
        Some(TypedVar::get_scope_root(*self, compiler))
    }
}

// Rust-only inherent forwarding keeps arena-handle methods usable without importing the trait;
// every common Java body occurs once in AbstractVar.
macro_rules! var_reader {
    ($name:ident, $result:ty) => {
        pub fn $name(self, compiler: &AbstractCompiler) -> $result {
            <Self as AbstractVar>::$name(self, compiler)
        }
    };
}

impl TypedVar {
    pub fn equals(self, compiler: &AbstractCompiler, other: TypedVar) -> bool {
        (&self as &dyn ScopedName).equals(compiler, Some(&other))
    }

    pub fn equals_scoped_name(
        self,
        compiler: &AbstractCompiler,
        other: Option<&dyn ScopedName>,
    ) -> bool {
        (&self as &dyn ScopedName).equals(compiler, other)
    }

    pub fn hash_code(self, compiler: &AbstractCompiler) -> i32 {
        (&self as &dyn ScopedName).hash_code(compiler)
    }

    var_reader!(get_name, JsString);
    var_reader!(get_scope_root, NodeId);
    var_reader!(get_node, Option<NodeId>);
    var_reader!(get_input, Option<CompilerInput>);
    var_reader!(
        get_source_file,
        Option<std::sync::Arc<dyn closure_rhino::static_source_file::StaticSourceFile>>
    );
    var_reader!(get_symbol, TypedVar);
    var_reader!(get_declaration, Option<TypedVar>);
    var_reader!(get_parent_node, Option<NodeId>);
    var_reader!(is_bleeding_function, bool);
    pub fn get_scope(self, compiler: &AbstractCompiler) -> TypedScope {
        check_not_null!(<Self as AbstractVar>::get_scope(self, compiler))
    }
    var_reader!(get_index, i32);
    var_reader!(is_global, bool);
    var_reader!(is_local, bool);
    var_reader!(is_extern, bool);
    var_reader!(is_declared_or_inferred_const, bool);
    var_reader!(is_define, bool);
    var_reader!(get_initial_value, Option<NodeId>);
    var_reader!(get_name_node, Option<NodeId>);
    var_reader!(
        get_jsdoc_info,
        Option<std::sync::Arc<closure_rhino::jsdoc_info::JSDocInfo>>
    );
    var_reader!(is_var, bool);
    var_reader!(is_catch, bool);
    var_reader!(is_let, bool);
    var_reader!(is_const, bool);
    var_reader!(is_class, bool);
    var_reader!(is_param, bool);
    var_reader!(is_default_param, bool);
    var_reader!(is_import, bool);
    var_reader!(is_arguments, bool);
    var_reader!(is_goog_module_exports, bool);
    var_reader!(is_this, bool);
    var_reader!(is_implicit, bool);
    var_reader!(declaration_type, Option<Token>);
    var_reader!(is_implicit_goog_namespace, bool);
}

/// Rust-only: Java's TypedVar object as closure-jstype sees it (a `StaticTypedSlot` and
/// `StaticTypedRef`); one canonical, leaked view per var, like `TypedScopeView`.
#[derive(Debug)]
pub struct TypedVarView {
    arena: Weak<RwLock<TypedScopeArena>>,
    var: TypedVar,
}

impl TypedScopeArena {
    /// Rust-only: the canonical view of `var`, created on first use.
    pub(crate) fn var_view(
        &self,
        this: &Weak<RwLock<TypedScopeArena>>,
        var: TypedVar,
    ) -> &'static TypedVarView {
        self.vars[var.index()].view.get_or_init(|| {
            Box::leak(Box::new(TypedVarView {
                arena: this.clone(),
                var,
            }))
        })
    }
}

impl TypedVar {
    /// Rust-only: the canonical closure-jstype view of this var (Java passes `this` as a
    /// StaticTypedSlot).
    pub fn as_static_typed_slot(self, compiler: &AbstractCompiler) -> &'static dyn StaticTypedSlot {
        let this = Arc::downgrade(&compiler.typed_scope_arena);
        TypedScopeArena::read(compiler).var_view(&this, self)
    }
}

impl TypedVarView {
    fn read<R>(
        &self,
        f: impl FnOnce(&Weak<RwLock<TypedScopeArena>>, &TypedVarData, &TypedScopeArena) -> R,
    ) -> R {
        let arena = self
            .arena
            .upgrade()
            .expect("a TypedVar view outlived its compiler");
        let guard = arena.read().unwrap_or_else(PoisonError::into_inner);
        f(&self.arena, &guard.vars[self.var.index()], &guard)
    }
}

impl StaticTypedSlot for TypedVarView {
    // port: AbstractVar#getName
    fn get_name(&self, _reg: &JSTypeRegistry) -> JsString {
        self.read(|_, data, _| data.abstract_var.name.clone())
    }

    // port: TypedVar#getType
    fn get_type(&self, _reg: &JSTypeRegistry) -> Option<TypeId> {
        self.read(|_, data, _| data.type_)
    }

    // port: TypedVar#isTypeInferred
    fn is_type_inferred(&self, _reg: &JSTypeRegistry) -> bool {
        self.read(|_, data, _| data.type_inferred)
    }

    // port: AbstractVar#getDeclaration
    fn get_declaration(&self, _reg: &JSTypeRegistry) -> Option<&dyn StaticTypedRef> {
        self.read(|_, data, _| data.abstract_var.name_node)
            .map(|_| self as &dyn StaticTypedRef)
    }

    // port: AbstractVar#getJSDocInfo
    /// Java reads `NodeUtil.getBestJSDocInfo(nameNode)`, which needs the AST that
    /// closure-jstype's slot method does not pass; closure-jstype never asks a scope slot for it
    /// (only properties), and compiler-side code calls `TypedVar::get_jsdoc_info`.
    fn get_jsdoc_info(&self, _reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
        unreachable!("TypedVar#getJSDocInfo needs the AST: call TypedVar::get_jsdoc_info(compiler)")
    }

    // port: AbstractVar#getScope
    fn get_scope(&self, _reg: &JSTypeRegistry) -> Option<&dyn StaticTypedScope> {
        self.read(|this, data, arena| {
            data.abstract_var
                .scope
                .map(|scope| &**arena.scope_view(this, scope) as &dyn StaticTypedScope)
        })
    }
}

impl StaticTypedRef for TypedVarView {
    // port: AbstractVar#getSymbol
    fn get_symbol(&self, _reg: &JSTypeRegistry) -> &dyn StaticTypedSlot {
        self
    }

    // port: AbstractVar#getNode
    fn get_node(&self, _reg: &JSTypeRegistry) -> Option<NodeId> {
        self.read(|_, data, _| data.abstract_var.name_node)
    }

    // port: AbstractVar#getSourceFile
    fn get_source_file(
        &self,
        _reg: &JSTypeRegistry,
        ast: &Ast,
    ) -> Option<Arc<dyn StaticSourceFile>> {
        self.read(|_, data, arena| {
            data.abstract_var
                .name_node
                .unwrap_or_else(|| {
                    let scope = check_not_null!(data.abstract_var.scope);
                    arena.scopes[scope.index()].abstract_scope.root_node
                })
                .get_static_source_file(ast)
        })
    }
}
