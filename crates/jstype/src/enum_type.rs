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
//   src/com/google/javascript/rhino/jstype/EnumType.java.

use crate::{
    JSTypeNative, JSTypeRegistry, TypeId,
    js_type::{JSType, JSTypeKind},
    object_type::ObjectType,
    prototype_object_type::{PrototypeObjectTypeBuilder, PrototypeObjectTypeData},
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    error_reporter::ErrorReporter,
    js_string::JsString,
    jscomp_base::Tri,
    node::{Ast, NodeId},
};
use indexmap::IndexSet;

pub(crate) struct EnumTypeData {
    pub(crate) prototype: PrototypeObjectTypeData,
    pub(crate) element_type: Option<TypeId>,
    pub(crate) elements: IndexSet<JsString>,
    pub(crate) source: Option<NodeId>,
    pub(crate) goog_module_id: Option<JsString>,
}
pub struct EnumTypeBuilder {
    prototype: PrototypeObjectTypeBuilder,
    element_name: Option<JsString>,
    source: Option<NodeId>,
    goog_module_id: Option<JsString>,
    element_type: Option<TypeId>,
}
pub type Builder = EnumTypeBuilder;
// port: EnumType#builder
pub fn builder() -> EnumTypeBuilder {
    EnumTypeBuilder::new()
}
impl Default for EnumTypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl EnumTypeBuilder {
    // port: EnumType.Builder#Builder
    pub fn new() -> Self {
        Self {
            prototype: PrototypeObjectTypeBuilder::new(),
            element_name: None,
            source: None,
            goog_module_id: None,
            element_type: None,
        }
    }
    // port: EnumType.Builder#setName
    pub fn set_name(self, x: impl Into<JsString>) -> Self {
        self.set_name_nullable(Some(x.into()))
    }
    // port: EnumType.Builder#setName
    /// The nullable form of `setName` (Java callers may pass null: `"enum{" + x + "}"` then
    /// concatenates `String.valueOf(null)`, "null", and the element name stays null).
    pub fn set_name_nullable(mut self, x: Option<JsString>) -> Self {
        let shown = x.clone().unwrap_or_else(|| JsString::from("null"));
        self.prototype = self
            .prototype
            .set_name(JsString::from("enum{").concat(&shown).concat(&"}".into()));
        self.element_name = x;
        self
    }
    // port: EnumType.Builder#setSource
    pub fn set_source(mut self, x: Option<NodeId>) -> Self {
        self.source = x;
        self
    }
    // port: EnumType.Builder#setGoogModuleId
    pub fn set_goog_module_id(self, x: impl Into<JsString>) -> Self {
        self.set_goog_module_id_nullable(Some(x.into()))
    }
    // port: EnumType.Builder#setGoogModuleId
    /// The nullable form of `setGoogModuleId` (null outside goog.modules).
    pub fn set_goog_module_id_nullable(mut self, x: Option<JsString>) -> Self {
        self.goog_module_id = x;
        self
    }
    // port: EnumType.Builder#setElementType
    pub fn set_element_type(mut self, x: TypeId) -> Self {
        self.element_type = Some(x);
        self
    }
    // port: EnumType.Builder#build
    pub fn build(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        new(reg, ast, self)
    }
}
// port: EnumType#EnumType
fn new(reg: &mut JSTypeRegistry, ast: &Ast, builder: EnumTypeBuilder) -> TypeId {
    let data = EnumTypeData {
        prototype: PrototypeObjectTypeData::from_builder(reg, &builder.prototype),
        element_type: None,
        elements: IndexSet::new(),
        source: builder.source,
        goog_module_id: builder.goog_module_id,
    };
    let t = reg.alloc(JSTypeKind::Enum(data), builder.prototype.template_type_map);
    crate::prototype_object_type::initialize_after_allocation(t, reg);
    let element = crate::enum_element_type::new(
        reg,
        ast,
        builder.element_type.expect("NullPointerException"),
        builder.element_name,
        t,
    );
    if let JSTypeKind::Enum(d) = &mut reg.data_mut(t).kind {
        d.element_type = Some(element);
    }
    reg.finish_construction(t, ast);
    t
}
pub trait EnumType {
    fn get_elements(self, reg: &JSTypeRegistry) -> IndexSet<JsString>;
    fn define_element(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
        defining_node: Option<NodeId>,
    ) -> bool;
    fn get_elements_type(self, reg: &JSTypeRegistry) -> TypeId;
    fn get_source(self, reg: &JSTypeRegistry) -> Option<NodeId>;
    fn get_goog_module_id(self, reg: &JSTypeRegistry) -> Option<JsString>;
}
impl EnumType for TypeId {
    // port: EnumType#getElements
    fn get_elements(self, reg: &JSTypeRegistry) -> IndexSet<JsString> {
        data(self, reg).elements.clone()
    }
    // port: EnumType#defineElement
    fn define_element(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
        defining_node: Option<NodeId>,
    ) -> bool {
        let name = name.into();
        if let JSTypeKind::Enum(d) = &mut reg.data_mut(self).kind {
            d.elements.insert(name.clone());
        }
        let element = get_elements_type(self, reg);
        self.define_declared_property(reg, ast, name, element, defining_node)
    }
    // port: EnumType#getElementsType
    fn get_elements_type(self, reg: &JSTypeRegistry) -> TypeId {
        get_elements_type(self, reg)
    }
    // port: EnumType#getSource
    fn get_source(self, reg: &JSTypeRegistry) -> Option<NodeId> {
        data(self, reg).source
    }
    // port: EnumType#getGoogModuleId
    fn get_goog_module_id(self, reg: &JSTypeRegistry) -> Option<JsString> {
        data(self, reg).goog_module_id.clone()
    }
}
pub(crate) fn data(t: TypeId, reg: &JSTypeRegistry) -> &EnumTypeData {
    match &reg.data(t).kind {
        JSTypeKind::Enum(d) => d,
        _ => panic!("ClassCastException"),
    }
}
// port: EnumType#getTypeClass
pub(crate) fn get_type_class(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::js_type_class::JSTypeClass {
    crate::js_type_class::JSTypeClass::ENUM
}
// port: EnumType#toMaybeEnumType
pub(crate) fn to_maybe_enum_type(t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(t)
}
// port: EnumType#getImplicitPrototype
pub fn get_implicit_prototype(_t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(reg.get_native_object_type(JSTypeNative::OBJECT_TYPE))
}
// port: EnumType#getElementsType
pub(crate) fn get_elements_type(t: TypeId, reg: &JSTypeRegistry) -> TypeId {
    data(t, reg).element_type.expect("NullPointerException")
}
// port: EnumType#getEnumeratedTypeOfEnumObject
pub fn get_enumerated_type_of_enum_object(t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(crate::enum_element_type::get_primitive_type(
        get_elements_type(t, reg),
        reg,
    ))
}
// port: EnumType#testForEquality
pub(crate) fn test_for_equality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<Tri> {
    if let Some(result) = crate::object_type::test_for_equality(t, reg, ast, that) {
        return Some(result);
    }
    Some(if t.equals(reg, ast, that) {
        Tri::TRUE
    } else {
        Tri::FALSE
    })
}
// port: EnumType#appendTo
pub(crate) fn append_to(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    _ast: &Ast,
    sb: &mut TypeStringBuilder,
) {
    if sb.is_for_annotations() {
        sb.append("!Object");
    } else {
        let name = t.get_reference_name(reg).unwrap().to_string_lossy();
        sb.append(&name);
    }
}
// port: EnumType#getDisplayName
pub fn get_display_name(t: TypeId, reg: &JSTypeRegistry) -> Option<String> {
    get_elements_type(t, reg).get_display_name(reg)
}
// port: EnumType#visit
pub(crate) fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    visitor.case_object_type(reg, ast, t)
}
// port: EnumType#visit
pub(crate) fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_object_type(reg, ast, t, that)
}
// port: EnumType#getConstructor
pub fn get_constructor(_t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    None
}
// port: EnumType#matchesNumberContext
pub(crate) fn matches_number_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    false
}
// port: EnumType#matchesStringContext
pub(crate) fn matches_string_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: EnumType#matchesObjectContext
pub(crate) fn matches_object_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: EnumType#resolveInternal
pub(crate) fn resolve_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) -> TypeId {
    let element = get_elements_type(t, reg).resolve_with_reporter(reg, ast, reporter);
    if let JSTypeKind::Enum(d) = &mut reg.data_mut(t).kind {
        d.element_type = Some(element);
    }
    crate::prototype_object_type::resolve_internal(t, reg, ast, reporter)
}
