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
 *   John Lenz
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
//   src/com/google/javascript/rhino/jstype/RelationshipVisitor.java.

use crate::{TypeId, js_type_registry::JSTypeRegistry};
use closure_rhino::node::Ast;

pub trait RelationshipVisitor<T> {
    // port: RelationshipVisitor#caseUnknownType
    fn case_unknown_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
    // port: RelationshipVisitor#caseNoType
    fn case_no_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, that_type: TypeId) -> T;
    // port: RelationshipVisitor#caseNoObjectType
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, that_type: TypeId) -> T;
    // port: RelationshipVisitor#caseAllType
    fn case_all_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, that_type: TypeId) -> T;
    // port: RelationshipVisitor#caseValueType
    fn case_value_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
    // port: RelationshipVisitor#caseObjectType
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
    // port: RelationshipVisitor#caseFunctionType
    fn case_function_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
    // port: RelationshipVisitor#caseUnionType
    fn case_union_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
    // port: RelationshipVisitor#caseTemplatizedType
    fn case_templatized_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
    // port: RelationshipVisitor#caseTemplateType
    fn case_template_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
    // port: RelationshipVisitor#caseEnumElementType
    fn case_enum_element_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> T;
}
