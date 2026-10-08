/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Var.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_var::{AbstractVar, AbstractVarData},
    compiler_input::CompilerInput,
    scope::{ScopeArena, ScopeId},
    scoped_name::ScopedName,
    typed_scope::{TypedArenaMut, TypedArenaRef},
};
use closure_rhino::{
    check_argument, check_not_null, js_string::JsString, node::NodeId, token::Token,
};
use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VarId(pub(crate) NonZeroU32);

pub type Var = VarId;

impl VarId {
    pub const ARGUMENTS: &'static str = "arguments";
    pub const EXPORTS: &'static str = "exports";

    // port: Var#Var
    pub fn new(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        name_node: Option<NodeId>,
        scope: ScopeId,
        index: i32,
        input: Option<CompilerInput>,
        implicit_goog_namespace_definition: Option<NodeId>,
    ) -> Self {
        let data = AbstractVarData::new(
            compiler,
            name.into(),
            name_node,
            Some(scope),
            index,
            input,
            implicit_goog_namespace_definition,
        );
        if let Some(name_node) = name_node {
            check_argument!(
                matches!(
                    name_node.get_token(compiler),
                    Token::MODULE_BODY | Token::NAME | Token::IMPORT_STAR
                ),
                "Invalid name node %s",
                name_node.to_string(compiler)
            );
        }
        let mut arena = crate::scope::ScopeArena::write(compiler);
        let var = Self::push(&mut arena, data);
        drop(arena);
        var
    }

    /// Rust-only: the arena half of `Var#Var` (the new var's slot), shared with the scope view
    /// that creates implicit vars (`ScopeView::get_slot_creating_implicit_vars`).
    pub(crate) fn push(
        arena: &mut crate::scope::ScopeArena,
        data: crate::abstract_var::AbstractVarData<ScopeId>,
    ) -> Self {
        let var = Self(NonZeroU32::new(u32::try_from(arena.vars.len() + 1).unwrap()).unwrap());
        arena.vars.push(data);
        arena.var_views.push(std::sync::OnceLock::new());
        var
    }

    pub(crate) fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    // port: Var#createImplicitGoogNamespace
    pub fn create_implicit_goog_namespace(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        scope: ScopeId,
        definition: NodeId,
    ) -> Self {
        Self::new(compiler, name, None, scope, -1, None, Some(definition))
    }

    // port: Var#toString
    pub fn to_string(self, compiler: &AbstractCompiler) -> String {
        format!(
            "Var {} @ {}",
            self.get_name(compiler),
            self.get_name_node(compiler)
                .map_or_else(|| "null".to_owned(), |node| node.to_string(compiler))
        )
    }
}

impl AbstractVar for VarId {
    type Scope = ScopeId;

    type DataRef<'a> = TypedArenaRef<'a, AbstractVarData<ScopeId>, ScopeArena>;
    type DataMut<'a> = TypedArenaMut<'a, AbstractVarData<ScopeId>, ScopeArena>;

    fn var_data(self, compiler: &AbstractCompiler) -> Self::DataRef<'_> {
        TypedArenaRef::new(ScopeArena::read(compiler), self.index(), |a, i| &a.vars[i])
    }

    fn var_data_mut(self, compiler: &mut AbstractCompiler) -> Self::DataMut<'_> {
        TypedArenaMut::new(
            ScopeArena::write(compiler),
            self.index(),
            |a, i| &a.vars[i],
            |a, i| &mut a.vars[i],
        )
    }

    fn to_string(self, compiler: &AbstractCompiler) -> String {
        VarId::to_string(self, compiler)
    }
}

impl ScopedName for VarId {
    fn get_name(&self, compiler: &AbstractCompiler) -> JsString {
        VarId::get_name(*self, compiler)
    }

    fn get_scope_root(&self, compiler: &AbstractCompiler) -> Option<NodeId> {
        Some(VarId::get_scope_root(*self, compiler))
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

impl VarId {
    pub fn equals(self, compiler: &AbstractCompiler, other: VarId) -> bool {
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
    var_reader!(get_symbol, VarId);
    var_reader!(get_declaration, Option<VarId>);
    var_reader!(get_parent_node, Option<NodeId>);
    var_reader!(is_bleeding_function, bool);
    pub fn get_scope(self, compiler: &AbstractCompiler) -> ScopeId {
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
    pub fn add_implicit_goog_namespace_definition(
        self,
        compiler: &mut AbstractCompiler,
        definition: NodeId,
    ) {
        <Self as AbstractVar>::add_implicit_goog_namespace_definition(self, compiler, definition);
    }
    var_reader!(
        get_implicit_goog_namespace_strength,
        closure_rhino::static_source_file::SourceKind
    );
}
