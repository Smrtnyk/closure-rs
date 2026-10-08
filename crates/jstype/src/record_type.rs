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
//   src/com/google/javascript/rhino/jstype/RecordType.java.

use crate::{
    JSTypeNative, JSTypeRegistry, TypeId,
    js_type::{JSType, JSTypeKind},
    object_type::ObjectType,
    property::PropertyKey,
    prototype_object_type::{PrototypeObjectTypeBuilder, PrototypeObjectTypeData},
    record_type_builder::{RecordProperty, RecordTypeBuilder},
    union_type::UnionTypeBuilder,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::collections::BTreeMap;

pub(crate) struct RecordTypeData {
    pub(crate) prototype: PrototypeObjectTypeData,
    pub(crate) declared: bool,
    pub(crate) is_frozen: bool,
}
// port: RecordType#RecordType
pub(crate) fn new(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    properties: BTreeMap<JsString, RecordProperty>,
    declared: bool,
) -> TypeId {
    let mut prototype =
        PrototypeObjectTypeData::from_builder(reg, &PrototypeObjectTypeBuilder::new());
    prototype.pretty_print = true;
    let t = reg.alloc(
        JSTypeKind::Record(RecordTypeData {
            prototype,
            declared,
            is_frozen: false,
        }),
        None,
    );
    crate::prototype_object_type::initialize_after_allocation(t, reg);
    for (property, prop) in properties {
        if declared {
            t.define_declared_property(
                reg,
                ast,
                property,
                prop.get_type(),
                prop.get_property_node(),
            );
        } else {
            t.define_synthesized_property(
                reg,
                ast,
                property,
                prop.get_type(),
                prop.get_property_node(),
            );
        }
    }
    if let JSTypeKind::Record(d) = &mut reg.data_mut(t).kind {
        d.is_frozen = true;
    }
    reg.finish_construction(t, ast);
    t
}
pub trait RecordType {
    fn is_synthetic(self, reg: &JSTypeRegistry) -> bool;
}
impl RecordType for TypeId {
    // port: RecordType#isSynthetic
    fn is_synthetic(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Record(d) => !d.declared,
            _ => panic!("ClassCastException"),
        }
    }
}
// port: RecordType#getTypeClass
pub(crate) fn get_type_class(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::js_type_class::JSTypeClass {
    crate::js_type_class::JSTypeClass::RECORD
}
// port: RecordType#getImplicitPrototype
pub fn get_implicit_prototype(_t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(reg.get_native_object_type(JSTypeNative::OBJECT_TYPE))
}
// port: RecordType#defineProperty
#[allow(clippy::collapsible_if)] // Preserve the Java frozen-record guard.
pub(crate) fn define_property(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    property_name: &PropertyKey,
    type_: TypeId,
    inferred: bool,
    property_node: Option<NodeId>,
) -> bool {
    if let JSTypeKind::Record(d) = &reg.data(t).kind {
        if d.is_frozen {
            return false;
        }
    }
    crate::prototype_object_type::define_property(
        t,
        reg,
        ast,
        property_name,
        type_,
        inferred,
        property_node,
    )
}
// port: RecordType#getGreatestSubtypeHelper
pub(crate) fn get_greatest_subtype_helper(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> TypeId {
    if let Some(that_record) = that.to_maybe_record_type(reg) {
        let mut builder = RecordTypeBuilder::new();
        builder.set_synthesized(true);
        let no_type = reg.get_native_object_type(JSTypeNative::NO_TYPE);
        for property in t.get_own_property_names(reg) {
            let this_property_type = t.get_property_type(reg, ast, &property);
            let prop_type = if that_record.has_property(reg, ast, &property) {
                let that_property_type = that_record.get_property_type(reg, ast, &property);
                let prop_type =
                    this_property_type.get_greatest_subtype(reg, ast, that_property_type);
                if prop_type.equals(reg, ast, no_type) {
                    return no_type;
                }
                prop_type
            } else {
                this_property_type
            };
            let node = t.get_property_node(reg, ast, &property);
            builder.add_property(property, prop_type, node);
        }
        for property in that_record.get_own_property_names(reg) {
            if !t.has_property(reg, ast, &property) {
                let typ = that_record.get_property_type(reg, ast, &property);
                let node = that_record.get_property_node(reg, ast, &property);
                builder.add_property(property, typ, node);
            }
        }
        return builder.build(reg, ast);
    }
    let mut greatest_subtype = reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let obj = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let that_restricted_to_obj = obj.get_greatest_subtype(reg, ast, that);
    if !that_restricted_to_obj.is_empty_type(reg) {
        for prop_name in t.get_own_property_names(reg) {
            let prop_type = t.get_property_type(reg, ast, &prop_name);
            let mut builder = UnionTypeBuilder::new();
            for alt in reg.get_each_reference_type_with_property(&prop_name) {
                let alt_prop_type = alt.get_property_type(reg, ast, &prop_name);
                if !alt.equals(reg, ast, t)
                    && alt.is_subtype_of(reg, ast, that)
                    && alt_prop_type.is_subtype_of(reg, ast, prop_type)
                {
                    builder.add_alternate(reg, ast, alt);
                }
            }
            let union = builder.build(reg, ast);
            greatest_subtype = greatest_subtype.get_least_supertype(reg, ast, union);
        }
    }
    greatest_subtype
}
// port: RecordType#toMaybeRecordType
pub(crate) fn to_maybe_record_type(t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(t)
}
// port: RecordType#isStructuralType
pub(crate) fn is_structural_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
