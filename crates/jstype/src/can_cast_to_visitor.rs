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
//   src/com/google/javascript/rhino/jstype/CanCastToVisitor.java.

use crate::{
    JSTypeNative, JSTypeRegistry, TypeId, enum_element_type::EnumElementType, js_type::JSType,
    object_type::ObjectType, proxy_object_type::ProxyObjectType,
    relationship_visitor::RelationshipVisitor,
};
use closure_rhino::node::Ast;
pub struct CanCastToVisitor;
impl CanCastToVisitor {
    // port: CanCastToVisitor#canCastToUnion
    fn can_cast_to_union(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        union_type: TypeId,
    ) -> bool {
        for t in crate::union_type::get_alternates(union_type, reg, ast)
            .iter()
            .copied()
        {
            if this_type.visit_relationship(reg, ast, self, t) {
                return true;
            }
        }
        false
    }
    // port: CanCastToVisitor#canCastToFunction
    fn can_cast_to_function(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        function_type: TypeId,
    ) -> bool {
        this_type.is_function_type(reg)
            || this_type.is_subtype_of(reg, ast, function_type)
            || function_type.is_subtype(reg, ast, this_type)
    }
    // port: CanCastToVisitor#isInterface
    fn is_interface(&self, reg: &JSTypeRegistry, type_: TypeId) -> bool {
        type_
            .to_object_type(reg)
            .and_then(|o| o.get_constructor(reg))
            .is_some_and(|c| c.is_interface(reg))
    }
    // port: CanCastToVisitor#castCastToHelper
    fn cast_cast_to_helper(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> bool {
        if this_type.is_no_resolved_type(reg)
            || that_type.is_unknown_type(reg, ast)
            || that_type.is_all_type(reg)
            || that_type.is_no_object_type(reg)
            || that_type.is_no_resolved_type(reg)
            || that_type.is_no_type(reg)
            || this_type.is_record_type(reg)
            || that_type.is_record_type(reg)
            || self.is_interface(reg, this_type)
            || self.is_interface(reg, that_type)
        {
            return true;
        }
        if let Some(element) = that_type.to_maybe_enum_element_type(reg) {
            let primitive = element.get_primitive_type(reg);
            return this_type.visit_relationship(reg, ast, self, primitive);
        }
        if let Some(union) = that_type.to_maybe_union_type(reg) {
            return self.can_cast_to_union(reg, ast, this_type, union);
        }
        if let Some(function) = that_type.to_maybe_function_type(reg) {
            return self.can_cast_to_function(reg, ast, this_type, function);
        }
        if let Some(templ) = that_type.to_maybe_templatized_type(reg) {
            let referenced = templ.get_referenced_type_internal(reg);
            return this_type.visit_relationship(reg, ast, self, referenced);
        }
        this_type.is_subtype_of(reg, ast, that_type) || that_type.is_subtype_of(reg, ast, this_type)
    }
}
impl RelationshipVisitor<bool> for CanCastToVisitor {
    // port: CanCastToVisitor#caseUnknownType
    fn case_unknown_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        _this_type: TypeId,
        _that_type: TypeId,
    ) -> bool {
        true
    }
    // port: CanCastToVisitor#caseNoType
    fn case_no_type(&mut self, _reg: &mut JSTypeRegistry, _ast: &Ast, _that_type: TypeId) -> bool {
        true
    }
    // port: CanCastToVisitor#caseNoObjectType
    fn case_no_object_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        _that_type: TypeId,
    ) -> bool {
        true
    }
    // port: CanCastToVisitor#caseAllType
    fn case_all_type(&mut self, _reg: &mut JSTypeRegistry, _ast: &Ast, _that_type: TypeId) -> bool {
        true
    }
    // port: CanCastToVisitor#caseValueType
    fn case_value_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> bool {
        self.cast_cast_to_helper(reg, ast, this_type, that_type)
    }
    // port: CanCastToVisitor#caseObjectType
    fn case_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> bool {
        self.cast_cast_to_helper(reg, ast, this_type, that_type)
    }
    // port: CanCastToVisitor#caseFunctionType
    fn case_function_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> bool {
        self.cast_cast_to_helper(reg, ast, this_type, that_type)
    }
    // port: CanCastToVisitor#caseUnionType
    fn case_union_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> bool {
        let mut visited = false;
        for t in crate::union_type::get_alternates(this_type, reg, ast)
            .iter()
            .copied()
        {
            if t.is_void_type(reg) || t.is_null_type(reg) {
                continue;
            }
            visited = true;
            if t.visit_relationship(reg, ast, self, that_type) {
                return true;
            }
        }
        if !visited {
            let null = reg.get_native_type(JSTypeNative::NULL_TYPE);
            let void = reg.get_native_type(JSTypeNative::VOID_TYPE);
            return null.visit_relationship(reg, ast, self, that_type)
                || void.visit_relationship(reg, ast, self, that_type);
        }
        false
    }
    // port: CanCastToVisitor#caseTemplatizedType
    fn case_templatized_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> bool {
        let referenced = this_type.get_referenced_type_internal(reg);
        referenced.visit_relationship(reg, ast, self, that_type)
    }
    // port: CanCastToVisitor#caseTemplateType
    fn case_template_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        _this_type: TypeId,
        _that_type: TypeId,
    ) -> bool {
        true
    }
    // port: CanCastToVisitor#caseEnumElementType
    fn case_enum_element_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        this_type: TypeId,
        that_type: TypeId,
    ) -> bool {
        let primitive = this_type.get_primitive_type(reg);
        primitive.visit_relationship(reg, ast, self, that_type)
    }
}
