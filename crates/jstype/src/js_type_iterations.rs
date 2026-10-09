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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/jstype/JSTypeIterations.java.

use crate::{JSTypeRegistry, TypeId, union_type::UnionTypeBuilder};
use closure_rhino::node::Ast;
pub struct JSTypeIterations;
impl JSTypeIterations {
    // port: JSTypeIterations#JSTypeIterations
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }
    // port: JSTypeIterations#anyTypeMatches
    pub fn any_type_matches(mut predicate: impl FnMut(TypeId) -> bool, types: &[TypeId]) -> bool {
        for &t in types {
            if predicate(t) {
                return true;
            }
        }
        false
    }
    // port: JSTypeIterations#anyTypeMatches
    pub fn any_type_matches_union(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mut predicate: impl FnMut(&mut JSTypeRegistry, TypeId) -> bool,
        union: TypeId,
    ) -> bool {
        for t in crate::union_type::get_alternates(union, reg, ast)
            .iter()
            .copied()
        {
            if predicate(reg, t) {
                return true;
            }
        }
        false
    }
    // port: JSTypeIterations#allTypesMatch
    pub fn all_types_match(mut predicate: impl FnMut(TypeId) -> bool, types: &[TypeId]) -> bool {
        for &t in types {
            if !predicate(t) {
                return false;
            }
        }
        true
    }
    // port: JSTypeIterations#allTypesMatch
    pub fn all_types_match_union(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mut predicate: impl FnMut(&mut JSTypeRegistry, TypeId) -> bool,
        union: TypeId,
    ) -> bool {
        for t in crate::union_type::get_alternates(union, reg, ast)
            .iter()
            .copied()
        {
            if !predicate(reg, t) {
                return false;
            }
        }
        true
    }
    // port: JSTypeIterations#mapTypes
    pub fn map_types(mut mapper: impl FnMut(TypeId) -> TypeId, types: &[TypeId]) -> Vec<TypeId> {
        let mut builder = Vec::new();
        for &t in types {
            builder.push(mapper(t));
        }
        builder
    }
    // port: JSTypeIterations#mapTypes
    pub fn map_types_union(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mut mapper: impl FnMut(&mut JSTypeRegistry, TypeId) -> TypeId,
        union: TypeId,
    ) -> TypeId {
        let mut mapped = Vec::new();
        for t in crate::union_type::get_alternates(union, reg, ast)
            .iter()
            .copied()
        {
            mapped.push(mapper(reg, t));
        }
        let mut builder = UnionTypeBuilder::new();
        builder.add_alternates(reg, ast, &mapped);
        builder.build(reg, ast)
    }
}
