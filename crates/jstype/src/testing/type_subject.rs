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
//   src/com/google/javascript/rhino/testing/TypeSubject.java.

use crate::TypeId;
use crate::function_type::FunctionType;
use crate::js_type::{JSType, Nullability};
use crate::js_type_registry::JSTypeRegistry;
use crate::object_type::ObjectType;
use crate::property::PropertyKey;
use crate::union_type::UnionType;
use closure_rhino::{closure_primitive::ClosurePrimitive, js_string::JsString, node::Ast};

#[derive(Clone, Copy, Debug)]
pub struct TypeSubject {
    actual: Option<TypeId>,
}
impl TypeSubject {
    // port: TypeSubject#assertType
    pub fn assert_type(type_: impl Into<Option<TypeId>>) -> Self {
        Self::new(type_.into())
    }
    // port: TypeSubject#types
    pub fn types() -> impl Fn(Option<TypeId>) -> Self {
        Self::new
    }
    // port: TypeSubject#TypeSubject
    fn new(type_: Option<TypeId>) -> Self {
        Self { actual: type_ }
    }
    // port: TypeSubject#isEqualTo
    pub fn is_equal_to(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        provided: impl Into<Option<TypeId>>,
    ) {
        self.check_equality_against(reg, ast, provided.into(), true);
    }
    // port: TypeSubject#isNotEqualTo
    pub fn is_not_equal_to(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        provided: impl Into<Option<TypeId>>,
    ) {
        self.check_equality_against(reg, ast, provided.into(), false);
    }
    // port: TypeSubject#isNumber
    pub fn is_number(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_number_value_type(reg));
    }
    // port: TypeSubject#isOnlyBigInt
    pub fn is_only_big_int(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_only_big_int(reg));
    }
    // port: TypeSubject#isNotOnlyBigInt
    pub fn is_not_only_big_int(&self, reg: &JSTypeRegistry) {
        assert!(!self.actual_non_null().is_only_big_int(reg));
    }
    // port: TypeSubject#isString
    pub fn is_string(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_string_value_type(reg));
    }
    // port: TypeSubject#isBoolean
    pub fn is_boolean(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_boolean_value_type(reg));
    }
    // port: TypeSubject#isVoid
    pub fn is_void(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_void_type(reg));
    }
    // port: TypeSubject#isNoType
    pub fn is_no_type(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_no_type(reg));
    }
    // port: TypeSubject#isUnknown
    pub fn is_unknown(&self, reg: &mut JSTypeRegistry, ast: &Ast) {
        assert!(self.actual_non_null().is_unknown_type(reg, ast));
    }
    // port: TypeSubject#isNotUnknown
    pub fn is_not_unknown(&self, reg: &mut JSTypeRegistry, ast: &Ast) {
        assert!(!self.actual_non_null().is_unknown_type(reg, ast));
    }
    // port: TypeSubject#isNoResolvedType
    pub fn is_no_resolved_type(&self, reg: &JSTypeRegistry, reference_name: impl Into<JsString>) {
        assert!(self.actual_non_null().is_no_resolved_type(reg));
        self.get_reference_name_is_equal_to(reg, reference_name);
    }
    // port: TypeSubject#isNotEmpty
    pub fn is_not_empty(&self, reg: &JSTypeRegistry) {
        assert!(!self.actual_non_null().is_empty_type(reg));
    }
    // port: TypeSubject#isLiteralObject
    pub fn is_literal_object(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_literal_object(reg));
    }
    // port: TypeSubject#isFunctionTypeThat
    pub fn is_function_type_that(&self, reg: &JSTypeRegistry) -> FunctionTypeSubject {
        assert!(self.actual_non_null().is_function_type(reg));
        FunctionTypeSubject { actual: *self }
    }
    // port: TypeSubject#isObjectTypeWithProperty
    pub fn is_object_type_with_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prop_name: impl Into<PropertyKey>,
    ) -> &Self {
        self.is_literal_object(reg);
        self.with_type_of_prop(reg, ast, prop_name).is_not_null();
        self
    }
    // port: TypeSubject#isUnionType
    pub fn is_union_type(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_union_type(reg));
    }
    // port: TypeSubject#isUnionOf
    pub fn is_union_of(&self, reg: &mut JSTypeRegistry, ast: &Ast, alternates: &[TypeId]) {
        self.is_union_type(reg);
        let actual = self.actual_non_null().get_alternates(reg, ast);
        assert_eq!(actual.len(), alternates.len());
        let mut remaining = alternates.to_vec();
        for actual in actual.iter().copied() {
            let index = remaining
                .iter()
                .position(|expected| actual.equals(reg, ast, *expected))
                .expect("missing union alternate");
            remaining.remove(index);
        }
        assert!(remaining.is_empty());
    }
    // port: TypeSubject#isResolved
    pub fn is_resolved(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_non_null().is_resolved(reg));
    }
    // port: TypeSubject#isUnresolved
    pub fn is_unresolved(&self, reg: &JSTypeRegistry) {
        assert!(!self.actual_non_null().is_resolved(reg));
    }
    // port: TypeSubject#withTypeOfProp
    pub fn with_type_of_prop(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prop_name: impl Into<PropertyKey>,
    ) -> Self {
        let actual = self.actual_non_null();
        assert!(actual.is_object_type(reg, ast));
        Self::assert_type(actual.get_property_type(reg, ast, prop_name))
    }
    // port: TypeSubject#hasDeclaredProperty
    pub fn has_declared_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prop_name: impl Into<PropertyKey>,
    ) {
        let actual = self.actual_non_null();
        assert!(actual.is_object_type(reg, ast));
        assert!(actual.is_property_type_declared(reg, ast, prop_name));
    }
    // port: TypeSubject#hasProperty
    pub fn has_property(&self, reg: &mut JSTypeRegistry, ast: &Ast, prop_name: TypeId) {
        let actual = self.actual_non_null();
        assert!(actual.is_object_type(reg, ast));
        assert!(actual.has_property(reg, ast, PropertyKey::Symbol(prop_name)));
    }
    // port: TypeSubject#hasInferredProperty
    pub fn has_inferred_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prop_name: impl Into<PropertyKey>,
    ) {
        let actual = self.actual_non_null();
        assert!(actual.is_object_type(reg, ast));
        assert!(actual.is_property_type_inferred(reg, ast, prop_name));
    }
    // port: TypeSubject#isObjectTypeWithoutProperty
    pub fn is_object_type_without_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prop_name: impl Into<PropertyKey>,
    ) {
        self.is_literal_object(reg);
        self.with_type_of_prop(reg, ast, prop_name).is_null();
    }
    // port: TypeSubject#isSubtypeOf
    pub fn is_subtype_of(&self, reg: &mut JSTypeRegistry, ast: &Ast, super_type: TypeId) {
        assert!(self.actual_non_null().is_subtype_of(reg, ast, super_type));
    }
    // port: TypeSubject#isNotSubtypeOf
    pub fn is_not_subtype_of(&self, reg: &mut JSTypeRegistry, ast: &Ast, super_type: TypeId) {
        assert!(!self.actual_non_null().is_subtype_of(reg, ast, super_type));
    }
    // port: TypeSubject#canTestForShallowEqualityWith
    pub fn can_test_for_shallow_equality_with(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        other: TypeId,
    ) {
        assert!(
            self.actual_non_null()
                .can_test_for_shallow_equality_with(reg, ast, other)
        );
    }
    // port: TypeSubject#cannotTestForShallowEqualityWith
    pub fn cannot_test_for_shallow_equality_with(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        other: TypeId,
    ) {
        assert!(
            !self
                .actual_non_null()
                .can_test_for_shallow_equality_with(reg, ast, other)
        );
    }
    // port: TypeSubject#toStringIsEqualTo
    pub fn to_string_is_equal_to(&self, reg: &mut JSTypeRegistry, ast: &Ast, type_string: &str) {
        assert_eq!(self.actual_non_null().to_string(reg, ast), type_string);
    }
    // port: TypeSubject#getReferenceNameIsEqualTo
    pub fn get_reference_name_is_equal_to(&self, reg: &JSTypeRegistry, name: impl Into<JsString>) {
        assert_eq!(
            self.actual_non_null().get_reference_name(reg),
            Some(name.into())
        );
        assert!(self.actual_non_null().has_reference_name(reg));
    }
    // port: TypeSubject#getReferenceNameIsNull
    pub fn get_reference_name_is_null(&self, reg: &JSTypeRegistry) {
        assert_eq!(self.actual_non_null().get_reference_name(reg), None);
        assert!(!self.actual_non_null().has_reference_name(reg));
    }
    // port: TypeSubject#actualNonNull
    fn actual_non_null(&self) -> TypeId {
        self.is_not_null();
        self.actual.unwrap()
    }
    // port: TypeSubject#checkEqualityAgainst
    fn check_equality_against(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        provided: Option<TypeId>,
        expectation: bool,
    ) {
        let provided_string = Self::debug_string_of(reg, ast, provided);
        let actual_string = Self::debug_string_of(reg, ast, self.actual);
        let actual_equals_provided = match (self.actual, provided) {
            (Some(actual), Some(provided)) => actual.equals(reg, ast, provided),
            (None, None) => true,
            _ => false,
        };
        assert_eq!(
            actual_equals_provided,
            expectation,
            "Types expected to be equal: {expectation}; {}; provided: {provided_string}",
            Self::equals_expression_of(&actual_string, &provided_string)
        );
        let provided_equals_actual = match (provided, self.actual) {
            (Some(provided), Some(actual)) => provided.equals(reg, ast, actual),
            (None, None) => true,
            _ => false,
        };
        assert_eq!(
            actual_equals_provided, provided_equals_actual,
            "Equality should be symmetric; {provided_string}"
        );
        if expectation {
            assert_eq!(
                self.actual.map_or(0, |actual| actual.hash_code(reg)),
                provided.map_or(0, |provided| provided.hash_code(reg)),
                "If two types are equal their hashcodes must also be equal; provided: {provided_string}"
            );
        }
    }
    // port: TypeSubject#actualCustomStringRepresentation
    pub fn actual_custom_string_representation(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> String {
        Self::debug_string_of(reg, ast, self.actual)
    }
    // port: TypeSubject#debugStringOf
    fn debug_string_of(reg: &mut JSTypeRegistry, ast: &Ast, type_: Option<TypeId>) -> String {
        type_.map_or_else(
            || "[Java null]".into(),
            |type_| {
                format!(
                    "{} [instanceof com.google.javascript.rhino.jstype.{}]",
                    type_.to_string(reg, ast),
                    java_class_name(type_.get_type_class(reg))
                )
            },
        )
    }
    // port: TypeSubject#equalsExpressionOf
    fn equals_expression_of(receiver: &str, parameter: &str) -> String {
        format!("({receiver}).equals({parameter})")
    }
    pub fn is_null(&self) {
        assert!(self.actual.is_none());
    }
    pub fn is_not_null(&self) {
        assert!(self.actual.is_some());
    }
    pub fn is_same_instance_as(&self, other: TypeId) {
        assert_eq!(self.actual, Some(other));
    }
    pub fn is_not_same_instance_as(&self, other: TypeId) {
        assert_ne!(self.actual, Some(other));
    }
    pub fn annotation_is_equal_to(&self, reg: &mut JSTypeRegistry, ast: &Ast, annotation: &str) {
        assert_eq!(
            self.actual_non_null()
                .to_annotation_string(reg, ast, Nullability::EXPLICIT),
            annotation
        );
    }
}

pub struct FunctionTypeSubject {
    actual: TypeSubject,
}
impl FunctionTypeSubject {
    // port: TypeSubject.FunctionTypeSubject#actualFunctionType
    fn actual_function_type(&self, reg: &JSTypeRegistry) -> TypeId {
        self.actual
            .actual_non_null()
            .to_maybe_function_type(reg)
            .expect("")
    }
    // port: TypeSubject.FunctionTypeSubject#hasTypeOfThisThat
    pub fn has_type_of_this_that(&self, reg: &JSTypeRegistry) -> TypeSubject {
        TypeSubject::assert_type(self.actual_function_type(reg).get_type_of_this(reg))
    }
    // port: TypeSubject.FunctionTypeSubject#hasReturnTypeThat
    pub fn has_return_type_that(&self, reg: &JSTypeRegistry) -> TypeSubject {
        TypeSubject::assert_type(self.actual_function_type(reg).get_return_type(reg))
    }
    // port: TypeSubject.FunctionTypeSubject#isConstructorFor
    pub fn is_constructor_for(&self, reg: &JSTypeRegistry, name: &str) {
        assert!(self.actual.actual_non_null().is_constructor(reg));
        assert_eq!(
            self.actual_function_type(reg)
                .get_instance_type(reg)
                .unwrap()
                .get_display_name(reg),
            Some(name.into())
        );
    }
    // port: TypeSubject.FunctionTypeSubject#isAbstract
    pub fn is_abstract(&self, reg: &JSTypeRegistry) {
        assert!(self.actual_function_type(reg).is_abstract(reg));
    }
    // port: TypeSubject.FunctionTypeSubject#hasPrimitiveId
    pub fn has_primitive_id(&self, reg: &JSTypeRegistry, id: ClosurePrimitive) {
        assert_eq!(
            self.actual_function_type(reg).get_closure_primitive(reg),
            Some(id)
        );
    }
}

fn java_class_name(type_class: crate::js_type_class::JSTypeClass) -> &'static str {
    use crate::js_type_class::JSTypeClass::*;
    match type_class {
        ALL => "AllType",
        ARROW => "ArrowType",
        BOOLEAN => "BooleanType",
        BIGINT => "BigIntType",
        ENUM => "EnumType",
        ENUM_ELEMENT => "EnumElementType",
        FUNCTION => "FunctionType",
        INSTANCE_OBJECT => "InstanceObjectType",
        NAMED => "NamedType",
        NO => "NoType",
        NO_OBJECT => "NoObjectType",
        NO_RESOLVED => "NoResolvedType",
        NULL => "NullType",
        NUMBER => "NumberType",
        PROTOTYPE_OBJECT => "PrototypeObjectType",
        PROXY_OBJECT => "ProxyObjectType",
        RECORD => "RecordType",
        STRING => "StringType",
        SYMBOL => "SymbolType",
        WELL_KNOWN_SYMBOL => "KnownSymbolType",
        TEMPLATE => "TemplateType",
        TEMPLATIZED => "TemplatizedType",
        UNION => "UnionType",
        UNKNOWN => "UnknownType",
        VOID => "VoidType",
    }
}
