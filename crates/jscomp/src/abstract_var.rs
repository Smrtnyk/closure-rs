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
//   src/com/google/javascript/jscomp/AbstractVar.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::{AbstractScope, ImplicitVar},
    compiler_input::CompilerInput,
    node_util::NodeUtil,
    scoped_name::ScopedName,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::NodeId,
    static_source_file::{SourceKind, StaticSourceFile},
    token::Token,
};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

/// Fields shared by syntactic and future typed variable handles.
#[derive(Debug)]
pub struct AbstractVarData<S> {
    pub(crate) name: JsString,
    pub(crate) name_node: Option<NodeId>,
    pub(crate) implicit_goog_namespace_strength: Option<SourceKind>,
    pub(crate) input: Option<CompilerInput>,
    pub(crate) index: i32,
    pub(crate) scope: Option<S>,
}

impl<S> AbstractVarData<S> {
    // port: AbstractVar#AbstractVar
    pub(crate) fn new(
        compiler: &AbstractCompiler,
        name: JsString,
        name_node: Option<NodeId>,
        scope: Option<S>,
        index: i32,
        input: Option<CompilerInput>,
        implicit_goog_namespace_definition: Option<NodeId>,
    ) -> Self {
        check_argument!(index >= -1, &index.to_string());
        let (name_node, implicit_goog_namespace_strength) =
            if let Some(definition) = implicit_goog_namespace_definition {
                (None, Some(strength_of(compiler, definition)))
            } else {
                (name_node, None)
            };
        Self {
            name,
            name_node,
            implicit_goog_namespace_strength,
            input,
            index,
            scope,
        }
    }
}

/// Common Java AbstractVar methods, reusable by a later typed variable handle.
pub trait AbstractVar: Copy + Eq + ScopedName {
    type Scope: AbstractScope<Var = Self> + 'static;
    /// Rust-only storage access: a plain reference into the syntactic arena, or a read guard into
    /// the shared typed arena (TypedVar views read it for closure-jstype).
    type DataRef<'a>: Deref<Target = AbstractVarData<Self::Scope>>;
    /// Rust-only mutable storage access, see `DataRef`.
    type DataMut<'a>: DerefMut<Target = AbstractVarData<Self::Scope>>;

    fn var_data(self, compiler: &AbstractCompiler) -> Self::DataRef<'_>;
    fn var_data_mut(self, compiler: &mut AbstractCompiler) -> Self::DataMut<'_>;
    fn to_string(self, compiler: &AbstractCompiler) -> String;

    // port: AbstractVar#getName
    fn get_name(self, compiler: &AbstractCompiler) -> JsString {
        self.var_data(compiler).name.clone()
    }

    /// Rust-only: `getName().equals(name)` without copying the name (D-025).
    fn name_equals(self, compiler: &AbstractCompiler, name: &str) -> bool {
        self.var_data(compiler).name == name
    }

    // port: AbstractVar#getScopeRoot
    fn get_scope_root(self, compiler: &AbstractCompiler) -> NodeId {
        check_not_null!(self.get_scope(compiler)).get_root_node(compiler)
    }

    // port: AbstractVar#getNode
    fn get_node(self, compiler: &AbstractCompiler) -> Option<NodeId> {
        self.var_data(compiler).name_node
    }

    // port: AbstractVar#getInput
    fn get_input(self, compiler: &AbstractCompiler) -> Option<CompilerInput> {
        self.var_data(compiler).input.clone()
    }

    // port: AbstractVar#getSourceFile
    fn get_source_file(self, compiler: &AbstractCompiler) -> Option<Arc<dyn StaticSourceFile>> {
        self.get_node(compiler)
            .unwrap_or_else(|| check_not_null!(self.get_scope(compiler)).get_root_node(compiler))
            .get_static_source_file(compiler)
    }

    // port: AbstractVar#getSymbol
    fn get_symbol(self, _compiler: &AbstractCompiler) -> Self {
        self.this_var()
    }

    // port: AbstractVar#getDeclaration
    fn get_declaration(self, compiler: &AbstractCompiler) -> Option<Self> {
        self.get_node(compiler).map(|_| self.this_var())
    }

    // port: AbstractVar#getParentNode
    fn get_parent_node(self, compiler: &AbstractCompiler) -> Option<NodeId> {
        self.get_node(compiler).and_then(|n| n.get_parent(compiler))
    }

    // port: AbstractVar#isBleedingFunction
    fn is_bleeding_function(self, compiler: &AbstractCompiler) -> bool {
        self.get_parent_node(compiler)
            .is_some_and(|parent| NodeUtil::is_function_expression(compiler, parent))
    }

    // port: AbstractVar#getScope
    fn get_scope(self, compiler: &AbstractCompiler) -> Option<Self::Scope> {
        self.var_data(compiler).scope
    }

    // port: AbstractVar#getIndex
    fn get_index(self, compiler: &AbstractCompiler) -> i32 {
        self.var_data(compiler).index
    }

    // port: AbstractVar#isGlobal
    fn is_global(self, compiler: &AbstractCompiler) -> bool {
        check_not_null!(self.get_scope(compiler)).is_global(compiler)
    }

    // port: AbstractVar#isLocal
    fn is_local(self, compiler: &AbstractCompiler) -> bool {
        check_not_null!(self.get_scope(compiler)).is_local(compiler)
    }

    // port: AbstractVar#isExtern
    fn is_extern(self, compiler: &AbstractCompiler) -> bool {
        self.get_input(compiler)
            .is_none_or(|input| input.is_extern())
    }

    // port: AbstractVar#isDeclaredOrInferredConst
    fn is_declared_or_inferred_const(self, compiler: &AbstractCompiler) -> bool {
        let Some(declaration_node) = self.get_node(compiler) else {
            return false;
        };
        declaration_node.is_declared_constant_var(compiler)
            || declaration_node.is_inferred_constant_var(compiler)
            || declaration_node.get_boolean_prop(compiler, NodeId::IS_CONSTANT_NAME)
    }

    // port: AbstractVar#isDefine
    fn is_define(self, compiler: &AbstractCompiler) -> bool {
        self.get_jsdoc_info(compiler)
            .is_some_and(|info| info.is_define())
    }

    // port: AbstractVar#getInitialValue
    fn get_initial_value(self, compiler: &AbstractCompiler) -> Option<NodeId> {
        self.get_node(compiler)
            .and_then(|n| NodeUtil::get_r_value_of_l_value(compiler, n))
    }

    // port: AbstractVar#getNameNode
    fn get_name_node(self, compiler: &AbstractCompiler) -> Option<NodeId> {
        self.get_node(compiler)
    }

    // port: AbstractVar#getJSDocInfo
    fn get_jsdoc_info(self, compiler: &AbstractCompiler) -> Option<Arc<JSDocInfo>> {
        self.get_node(compiler)
            .and_then(|n| NodeUtil::get_best_jsdoc_info(compiler, n))
    }

    // port: AbstractVar#isVar
    fn is_var(self, compiler: &AbstractCompiler) -> bool {
        self.declaration_type(compiler) == Some(Token::VAR)
    }

    // port: AbstractVar#isCatch
    fn is_catch(self, compiler: &AbstractCompiler) -> bool {
        self.declaration_type(compiler) == Some(Token::CATCH)
    }

    // port: AbstractVar#isLet
    fn is_let(self, compiler: &AbstractCompiler) -> bool {
        self.declaration_type(compiler) == Some(Token::LET)
    }

    // port: AbstractVar#isConst
    fn is_const(self, compiler: &AbstractCompiler) -> bool {
        self.declaration_type(compiler) == Some(Token::CONST)
    }

    // port: AbstractVar#isClass
    fn is_class(self, compiler: &AbstractCompiler) -> bool {
        self.declaration_type(compiler) == Some(Token::CLASS)
    }

    // port: AbstractVar#isParam
    fn is_param(self, compiler: &AbstractCompiler) -> bool {
        self.declaration_type(compiler) == Some(Token::PARAM_LIST)
    }

    // port: AbstractVar#isDefaultParam
    fn is_default_param(self, compiler: &AbstractCompiler) -> bool {
        let parent = check_not_null!(check_not_null!(self.get_node(compiler)).get_parent(compiler));
        check_not_null!(parent.get_parent(compiler)).is_param_list(compiler)
            && parent.is_default_value(compiler)
            && parent.get_first_child(compiler) == self.var_data(compiler).name_node
    }

    // port: AbstractVar#isImport
    fn is_import(self, compiler: &AbstractCompiler) -> bool {
        self.declaration_type(compiler) == Some(Token::IMPORT)
    }

    // port: AbstractVar#isArguments
    fn is_arguments(self, compiler: &AbstractCompiler) -> bool {
        self.name_equals(compiler, "arguments")
            && check_not_null!(self.get_scope(compiler)).is_function_scope(compiler)
    }

    // port: AbstractVar#isGoogModuleExports
    fn is_goog_module_exports(self, compiler: &AbstractCompiler) -> bool {
        check_not_null!(self.get_scope(compiler)).is_module_scope(compiler)
            && self.name_equals(compiler, "exports")
            && self.is_implicit(compiler)
    }

    // port: AbstractVar#isThis
    fn is_this(self, compiler: &AbstractCompiler) -> bool {
        self.name_equals(compiler, "this")
            && check_not_null!(self.get_scope(compiler)).is_function_scope(compiler)
    }

    // port: AbstractVar#isImplicit
    fn is_implicit(self, compiler: &AbstractCompiler) -> bool {
        if self.is_implicit_goog_namespace(compiler) {
            return true;
        }
        ImplicitVar::of(&self.get_name(compiler)).is_some_and(|var| {
            var.is_made_by_scope(compiler, check_not_null!(self.get_scope(compiler)))
        })
    }

    const DECLARATION_TYPES: [Token; 8] = [
        Token::VAR,
        Token::LET,
        Token::CONST,
        Token::FUNCTION,
        Token::CLASS,
        Token::CATCH,
        Token::IMPORT,
        Token::PARAM_LIST,
    ];

    // port: AbstractVar#declarationType
    fn declaration_type(self, compiler: &AbstractCompiler) -> Option<Token> {
        if self.is_implicit_goog_namespace(compiler) {
            return None;
        }
        let mut current = self.get_node(compiler);
        while let Some(n) = current {
            if Self::DECLARATION_TYPES.contains(&n.get_token(compiler)) {
                return Some(n.get_token(compiler));
            }
            current = n.get_parent(compiler);
        }
        // Guava immutableEnumSet iterates in Token ordinal order.
        let mut declaration_types = Self::DECLARATION_TYPES;
        declaration_types.sort();
        let declaration_types = format!(
            "[{}]",
            declaration_types.map(|token| token.to_string()).join(", ")
        );
        check_state!(
            self.is_implicit(compiler),
            "The nameNode for %s must be a descendant of one of: %s",
            self.to_string(compiler),
            declaration_types
        );
        None
    }

    // port: AbstractVar#thisVar
    fn this_var(self) -> Self {
        self
    }

    // port: AbstractVar#isImplicitGoogNamespace
    fn is_implicit_goog_namespace(self, compiler: &AbstractCompiler) -> bool {
        self.var_data(compiler)
            .implicit_goog_namespace_strength
            .is_some()
    }

    // port: AbstractVar#addImplicitGoogNamespaceDefinition
    fn add_implicit_goog_namespace_definition(
        self,
        compiler: &mut AbstractCompiler,
        definition: NodeId,
    ) {
        check_state!(
            self.is_implicit_goog_namespace(compiler),
            &self.to_string(compiler)
        );
        let strength = stronger_of(
            self.var_data(compiler)
                .implicit_goog_namespace_strength
                .unwrap(),
            strength_of(compiler, definition),
        );
        self.var_data_mut(compiler).implicit_goog_namespace_strength = Some(strength);
    }

    // port: AbstractVar#getImplicitGoogNamespaceStrength
    fn get_implicit_goog_namespace_strength(self, compiler: &AbstractCompiler) -> SourceKind {
        check_state!(
            self.is_implicit_goog_namespace(compiler),
            &self.to_string(compiler)
        );
        self.var_data(compiler)
            .implicit_goog_namespace_strength
            .unwrap()
    }
}
// port: AbstractVar#strengthOf
pub(crate) fn strength_of(compiler: &AbstractCompiler, n: NodeId) -> SourceKind {
    n.get_static_source_file(compiler)
        .map_or(SourceKind::EXTERN, |source| source.get_kind())
}

// port: AbstractVar#strongerOf
pub(crate) fn stronger_of(left: SourceKind, right: SourceKind) -> SourceKind {
    if left == SourceKind::STRONG || right == SourceKind::STRONG {
        SourceKind::STRONG
    } else if left == SourceKind::EXTERN || right == SourceKind::EXTERN {
        SourceKind::EXTERN
    } else {
        SourceKind::WEAK
    }
}
