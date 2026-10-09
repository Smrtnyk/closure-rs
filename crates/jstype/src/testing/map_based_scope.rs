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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/testing/MapBasedScope.java.

use super::abstract_static_scope::AbstractStaticScope;
use crate::TypeId;
use crate::simple_slot::SimpleSlot;
use crate::static_typed_slot::StaticTypedSlot;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;

#[derive(Clone, Debug)]
pub struct MapBasedScope {
    slots: IndexMap<JsString, SimpleSlot>,
}
impl MapBasedScope {
    // port: MapBasedScope#MapBasedScope
    pub fn new(names_to_types: impl IntoIterator<Item = (JsString, TypeId)>) -> Self {
        let mut slots = IndexMap::<_, _>::default();
        for (name, type_) in names_to_types {
            slots.insert(name.clone(), SimpleSlot::new(name, Some(type_), false));
        }
        Self { slots }
    }
    // port: MapBasedScope#emptyScope
    pub fn empty_scope() -> Self {
        Self::new(Vec::new())
    }
}
impl AbstractStaticScope for MapBasedScope {
    // port: MapBasedScope#getSlot
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.slots
            .get(name)
            .map(|slot| slot as &dyn StaticTypedSlot)
    }
}
