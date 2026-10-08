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
 *   Nick Santos
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
//   src/com/google/javascript/rhino/testing/AbstractStaticScope.java.

use crate::TypeId;
use crate::js_type_registry::JSTypeRegistry;
use crate::static_typed_scope::StaticTypedScope;
use crate::static_typed_slot::StaticTypedSlot;
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};

/// AbstractStaticScope supplies Java's null defaults around the one abstract slot lookup.
pub trait AbstractStaticScope: Send + Sync {
    // port: AbstractStaticScope#getSlot
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot>;
    // port: AbstractStaticScope#getRootNode
    fn get_root_node(&self) -> Option<NodeId> {
        None
    }
    // port: AbstractStaticScope#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        None
    }
    // port: AbstractStaticScope#getOwnSlot
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.get_slot(name)
    }
    // port: AbstractStaticScope#getTypeOfThis
    fn get_type_of_this(&self, _reg: &JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        None
    }
}
impl<S: AbstractStaticScope> StaticTypedScope for S {
    fn get_root_node(&self) -> Option<NodeId> {
        AbstractStaticScope::get_root_node(self)
    }
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        AbstractStaticScope::get_parent_scope(self)
    }
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        AbstractStaticScope::get_slot(self, name)
    }
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        AbstractStaticScope::get_own_slot(self, name)
    }
    fn get_type_of_this(&self, reg: &JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        AbstractStaticScope::get_type_of_this(self, reg, ast)
    }
}
