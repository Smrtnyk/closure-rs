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
//   src/com/google/javascript/rhino/jstype/StaticTypedScope.java.

//! `StaticTypedScope#lookupQualifiedName`, the default method a TypedScope inherits, as
//! [`TypedScopeCreator`](super::TypedScopeCreator) calls it.

use crate::abstract_compiler::AbstractCompiler;
use crate::typed_scope::TypedScope;
use closure_jstype::{TypeId, prelude::*};
use closure_rhino::qualified_name::QualifiedName;

/// `scope.lookupQualifiedName(qname)` on a TypedScope, which inherits the StaticTypedScope
/// default method. It runs over the mutable TypedScope API, so `getSlot` goes through
/// AbstractScope#getSlot and materialises implicit vars (`this`) as Java's computeIfAbsent does.
// port: StaticTypedScope#lookupQualifiedName
pub(super) fn lookup_qualified_name(
    compiler: &mut AbstractCompiler,
    scope: TypedScope,
    qname: &QualifiedName,
) -> Option<TypeId> {
    let joined = {
        let (_, ast) = compiler.get_type_registry_and_ast();
        qname.join(ast)
    };
    let slot = scope.get_slot(compiler, joined);
    if slot.is_some() && !slot.unwrap().is_type_inferred(compiler) {
        return slot.unwrap().get_type(compiler);
    } else if !{
        let (_, ast) = compiler.get_type_registry_and_ast();
        qname.is_simple(ast)
    } {
        let owner = {
            let (_, ast) = compiler.get_type_registry_and_ast();
            qname.get_owner(ast).expect("NullPointerException")
        };
        let type_ = lookup_qualified_name(compiler, scope, &owner);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if type_.is_some() && !type_.unwrap().is_unknown_type(reg, ast) {
            return type_
                .unwrap()
                .find_property_type(reg, ast, qname.get_component(ast));
        }
    }
    None
}
