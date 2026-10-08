/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/type/FlowScope.java.

//! A symbol table for inferring types during data flow analysis.
//!
//! Java's `FlowScope extends StaticTypedScope, LatticeElement`. Flow scopes are shared immutable
//! objects compared by identity (`Arc::ptr_eq`), so they travel as `Arc<dyn FlowScope>`; the
//! StaticTypedScope methods take the compiler, which owns the typed scope arena and the registry.
use crate::{
    abstract_compiler::AbstractCompiler, graph::lattice_element::LatticeElement,
    linked_flow_scope::OverlaySlot, typed_scope::TypedScope, typed_var::TypedVar,
};
use closure_jstype::TypeId;
use closure_rhino::{js_string::JsString, jsdoc_info::JSDocInfo, node::NodeId};
use std::{any::Any, sync::Arc};

pub trait FlowScope: LatticeElement {
    // port: FlowScope#withSyntacticScope
    /// Returns a flow scope with the given syntactic scope, which may be required to be a specific
    /// subclass, such as TypedScope.
    fn with_syntactic_scope(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        scope: TypedScope,
    ) -> Arc<dyn FlowScope>;

    // port: FlowScope#inferSlotType
    /// Returns a flow scope with the type of the given `symbol` updated to `type`.
    fn infer_slot_type(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        symbol: &JsString,
        type_: Option<TypeId>,
    ) -> Arc<dyn FlowScope>;

    // port: FlowScope#inferQualifiedSlot
    /// Returns a flow scope with the type of the given `symbol` updated to `inferredType`. Updates
    /// are not performed in-place.
    fn infer_qualified_slot(
        self: Arc<Self>,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        symbol: &JsString,
        bottom_type: Option<TypeId>,
        inferred_type: TypeId,
        declare: bool,
    ) -> Arc<dyn FlowScope>;

    // port: FlowScope#getDeclarationScope
    /// Returns the underlying TypedScope.
    fn get_declaration_scope(&self, compiler: &AbstractCompiler) -> TypedScope;

    // port: StaticTypedScope#getRootNode
    fn get_root_node(&self, compiler: &AbstractCompiler) -> NodeId;

    // port: StaticTypedScope#getParentScope
    fn get_parent_scope(&self, compiler: &AbstractCompiler) -> Option<TypedScope>;

    // port: StaticTypedScope#getSlot
    fn get_slot(&self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<FlowSlot>;

    // port: StaticTypedScope#getOwnSlot
    fn get_own_slot(&self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<FlowSlot>;

    // port: StaticTypedScope#getTypeOfThis
    fn get_type_of_this(&self, compiler: &mut AbstractCompiler) -> Option<TypeId>;

    /// Rust-only: Java's casts of a FlowScope to its class (`(LinkedFlowScope) input`).
    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

/// The `StaticTypedSlot` a flow scope hands out: a TypedVar of the syntactic scope chain, or a
/// LinkedFlowScope overlay slot.
#[derive(Debug, Clone)]
pub enum FlowSlot {
    Var(TypedVar),
    Overlay(Arc<OverlaySlot>),
}

impl FlowSlot {
    // port: StaticTypedSlot#getName
    pub fn get_name(&self, compiler: &AbstractCompiler) -> JsString {
        match self {
            Self::Var(var) => var.get_name(compiler),
            Self::Overlay(slot) => slot.get_name(),
        }
    }

    // port: StaticTypedSlot#getType
    pub fn get_type(&self, compiler: &AbstractCompiler) -> Option<TypeId> {
        match self {
            Self::Var(var) => var.get_type(compiler),
            Self::Overlay(slot) => slot.get_type(),
        }
    }

    // port: StaticTypedSlot#isTypeInferred
    pub fn is_type_inferred(&self, compiler: &AbstractCompiler) -> bool {
        match self {
            Self::Var(var) => var.is_type_inferred(compiler),
            Self::Overlay(slot) => slot.is_type_inferred(),
        }
    }

    // port: StaticTypedSlot#getDeclaration
    pub fn get_declaration(&self, compiler: &AbstractCompiler) -> Option<TypedVar> {
        match self {
            Self::Var(var) => var.get_declaration(compiler),
            Self::Overlay(slot) => slot.get_declaration(),
        }
    }

    // port: StaticTypedSlot#getJSDocInfo
    pub fn get_jsdoc_info(&self, compiler: &AbstractCompiler) -> Option<Arc<JSDocInfo>> {
        match self {
            Self::Var(var) => var.get_jsdoc_info(compiler),
            Self::Overlay(slot) => slot.get_jsdoc_info(),
        }
    }

    // port: StaticTypedSlot#getScope
    pub fn get_scope(&self, compiler: &AbstractCompiler) -> TypedScope {
        match self {
            Self::Var(var) => var.get_scope(compiler),
            Self::Overlay(slot) => slot.get_scope(),
        }
    }

    /// Java `==` on slot objects.
    pub fn ptr_eq(&self, other: &FlowSlot) -> bool {
        match (self, other) {
            (Self::Var(a), Self::Var(b)) => a == b,
            (Self::Overlay(a), Self::Overlay(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}
