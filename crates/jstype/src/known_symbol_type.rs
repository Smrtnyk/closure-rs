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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/jstype/KnownSymbolType.java.

use crate::{
    JSTypeRegistry, TypeId,
    js_type::{JSType, JSTypeKind},
    js_type_class::JSTypeClass,
};
use closure_rhino::js_string::JsString;
use closure_rhino::node::Ast;
pub struct KnownSymbolTypeData {
    pub symbol: crate::symbol_type::SymbolTypeData,
    pub(crate) name: JsString,
}
pub struct KnownSymbolType;
impl KnownSymbolType {
    // port: KnownSymbolType#KnownSymbolType
    pub fn new(reg: &mut JSTypeRegistry, ast: &Ast, name: impl Into<JsString>) -> TypeId {
        let t = reg.alloc(
            JSTypeKind::KnownSymbol(KnownSymbolTypeData {
                symbol: crate::symbol_type::SymbolTypeData {
                    value: crate::value_type::ValueTypeData,
                },
                name: name.into(),
            }),
            None,
        );
        t.eagerly_resolve_to_self(reg, ast);
        t
    }
}
// port: KnownSymbolType#getTypeClass
pub(crate) fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::WELL_KNOWN_SYMBOL
}
// port: KnownSymbolType#getDisplayName
pub(crate) fn get_display_name(t: TypeId, reg: &JSTypeRegistry) -> Option<String> {
    match &reg.data(t).kind {
        JSTypeKind::KnownSymbol(d) => Some(d.name.to_string_lossy()),
        _ => panic!("ClassCastException"),
    }
}
// port: KnownSymbolType#isKnownSymbolValueType
pub(crate) fn is_known_symbol_value_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: KnownSymbolType#toMaybeKnownSymbolType
pub(crate) fn to_maybe_known_symbol_type(t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(t)
}
