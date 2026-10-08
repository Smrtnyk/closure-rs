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
//   src/com/google/javascript/rhino/jstype/SimpleReference.java.

use crate::js_type_registry::JSTypeRegistry;
use crate::{static_typed_ref::StaticTypedRef, static_typed_slot::StaticTypedSlot};
use closure_rhino::{
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
};
use std::sync::Arc;

pub struct SimpleReference<T: StaticTypedSlot> {
    symbol: T,
    node: Option<NodeId>,
}
impl<T: StaticTypedSlot> SimpleReference<T> {
    // port: SimpleReference#SimpleReference
    pub fn new(symbol: T, node: impl Into<Option<NodeId>>) -> Self {
        Self {
            symbol,
            node: node.into(),
        }
    }
    // port: SimpleReference#getSymbol
    pub fn get_symbol(&self, _reg: &JSTypeRegistry) -> &T {
        &self.symbol
    }
    // port: SimpleReference#getNode
    pub fn get_node(&self, _reg: &JSTypeRegistry) -> Option<NodeId> {
        self.node
    }
    // port: SimpleReference#getSourceFile
    pub fn get_source_file(
        &self,
        _reg: &JSTypeRegistry,
        ast: &Ast,
    ) -> Option<Arc<dyn StaticSourceFile>> {
        self.node.unwrap().get_static_source_file(ast)
    }
    // port: SimpleReference#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        let source_name = self.node.and_then(|n| n.get_source_file_name(ast));
        let line_no = self.node.map_or(-1, |n| n.get_lineno(ast));
        format!(
            "{}@{}:{}",
            self.node
                .unwrap()
                .get_qualified_name(ast)
                .map_or_else(|| "null".into(), |q| q.to_string_lossy()),
            source_name.as_deref().unwrap_or("null"),
            line_no
        )
    }
}
impl<T: StaticTypedSlot> StaticTypedRef for SimpleReference<T> {
    fn get_symbol(&self, _reg: &JSTypeRegistry) -> &dyn StaticTypedSlot {
        &self.symbol
    }
    fn get_node(&self, _reg: &JSTypeRegistry) -> Option<NodeId> {
        self.node
    }
    fn get_source_file(
        &self,
        _reg: &JSTypeRegistry,
        ast: &Ast,
    ) -> Option<Arc<dyn StaticSourceFile>> {
        self.get_source_file(_reg, ast)
    }
}
