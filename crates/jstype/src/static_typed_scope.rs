/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/StaticScope.java,
//   src/com/google/javascript/rhino/jstype/StaticTypedScope.java.

// Preserve Java nested qualified-name lookup branches.
#![allow(clippy::collapsible_if)]
use crate::{
    TypeId, js_type::JSType, js_type_registry::JSTypeRegistry, static_typed_slot::StaticTypedSlot,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
};

pub trait StaticTypedScope: Send + Sync {
    fn get_root_node(&self) -> Option<NodeId>;
    // port: StaticTypedScope#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope>;
    // port: StaticTypedScope#getSlot
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot>;
    // port: StaticTypedScope#getOwnSlot
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot>;
    // port: StaticTypedScope#getSlot
    /// Rust-only twin of `get_slot` for callers that hold the registry and the AST (NamedType).
    /// A Java TypedScope creates an implicit var (`this`, `arguments`, `super`) on its first
    /// lookup (AbstractScope#getOwnImplicitSlot calling TypedScope#makeImplicitVar), and the var's
    /// type needs the registry and the AST; the plain `get_slot` of a scope view cannot create it.
    /// Scopes without implicit vars answer like `get_slot`.
    fn get_slot_creating_implicit_vars(
        &self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        self.get_slot(name)
    }
    // port: StaticTypedScope#getOwnSlot
    /// Rust-only twin of `get_own_slot` for callers that hold the registry and the AST (the
    /// JSTypeRegistry lookups): like `get_slot_creating_implicit_vars`, a Java scope creates its
    /// own implicit var (`this`, `arguments`, `super`) on the first `getOwnSlot`.
    /// Scopes without implicit vars answer like `get_own_slot`.
    fn get_own_slot_creating_implicit_vars(
        &self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        self.get_own_slot(name)
    }
    // port: StaticTypedScope#getTypeOfThis
    /// Rust-only parameters: Java's implementations read the root's JSType and the registry.
    fn get_type_of_this(&self, reg: &JSTypeRegistry, ast: &Ast) -> Option<TypeId>;
    // port: StaticScope#getTopmostScopeOfEventualDeclaration
    /// Rust-only parameters: Java's `getOwnSlot` may create an implicit var, whose type needs
    /// the registry and the AST (see `get_own_slot_creating_implicit_vars`).
    fn get_topmost_scope_of_eventual_declaration(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedScope> {
        if let Some(slot) = self.get_own_slot_creating_implicit_vars(reg, ast, name) {
            return slot.get_scope(reg);
        }
        self.get_parent_scope()
            .and_then(|p| p.get_topmost_scope_of_eventual_declaration(reg, ast, name))
    }
    // SyntheticTemplateScope exposes its template names before ordinary lookup.
    fn get_template_type(&self, _name: &JsString) -> Option<TypeId> {
        None
    }
    // port: StaticTypedScope#lookupQualifiedName
    fn lookup_qualified_name(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        qname: &QualifiedName,
    ) -> Option<TypeId> {
        if let Some(slot) = self.get_slot_creating_implicit_vars(reg, ast, &qname.join(ast)) {
            if !slot.is_type_inferred(reg) {
                return slot.get_type(reg);
            }
        }
        if !qname.is_simple(ast) {
            if let Some(type_) =
                self.lookup_qualified_name(reg, ast, &qname.get_owner(ast).unwrap())
            {
                if !type_.is_unknown_type(reg, ast) {
                    return type_.find_property_type(reg, ast, qname.get_component(ast));
                }
            }
        }
        None
    }
}
