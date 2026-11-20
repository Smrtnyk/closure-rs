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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/rhino/jstype/StaticTypedScope.java.

//! `StaticTypedScope#lookupQualifiedName`, the default method a TypedScope inherits, as
//! [`TypedScopeCreator`](super::TypedScopeCreator) calls it.

use crate::abstract_compiler::AbstractCompiler;
use crate::typed_scope::TypedScope;
use closure_jstype::{TypeId, prelude::*};
use closure_rhino::{js_string::JsString, qualified_name::QualifiedName};

/// `scope.lookupQualifiedName(qname)` on a TypedScope, which inherits the StaticTypedScope
/// default method. It runs over the mutable TypedScope API, so `getSlot` goes through
/// AbstractScope#getSlot and materialises implicit vars (`this`) as Java's computeIfAbsent does.
// port: StaticTypedScope#lookupQualifiedName
pub(super) fn lookup_qualified_name(
    compiler: &mut AbstractCompiler,
    scope: TypedScope,
    qname: &QualifiedName,
) -> Option<TypeId> {
    let joined = qname.join(compiler);
    lookup_name(compiler, scope, &joined, || qname.clone())
}

/// Rust-only (performance divergence): the full name is often already a declared slot.
/// Split it into interned components only when walking the owner's properties.
pub(super) fn lookup_dotted_name(
    compiler: &mut AbstractCompiler,
    scope: TypedScope,
    name: &JsString,
) -> Option<TypeId> {
    lookup_name(compiler, scope, name, || QualifiedName::of(name.clone()))
}

fn lookup_name(
    compiler: &mut AbstractCompiler,
    scope: TypedScope,
    joined: &JsString,
    qname: impl FnOnce() -> QualifiedName,
) -> Option<TypeId> {
    if let Some(slot) = scope.get_slot(compiler, joined)
        && !slot.is_type_inferred(compiler)
    {
        return slot.get_type(compiler);
    }
    let qname = qname();
    let owner = qname.get_owner(compiler)?;
    let owner_type = lookup_qualified_name(compiler, scope, &owner)?;
    let (reg, ast) = compiler.get_type_registry_and_ast();
    if owner_type.is_unknown_type(reg, ast) {
        None
    } else {
        owner_type.find_property_type(reg, ast, qname.get_component(ast))
    }
}
