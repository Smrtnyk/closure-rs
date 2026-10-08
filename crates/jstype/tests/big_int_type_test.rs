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
 *   Brock Smickley
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
//   test/com/google/javascript/rhino/jstype/BigIntTypeTest.java.

use closure_jstype::{
    TypeId,
    enum_type::EnumTypeBuilder,
    function_type::FunctionTypeBuilder,
    prelude::*,
    testing::{
        asserts::Asserts, base_js_type_test_case::BaseJSTypeTestCase,
        map_based_scope::MapBasedScope, type_subject::TypeSubject,
    },
};
use closure_rhino::js_string::JsString;
use std::sync::Arc;
struct Fixture {
    base: BaseJSTypeTestCase,
    function_type: TypeId,
    unresolved_named_type: TypeId,
    enum_type: TypeId,
    elements_type: TypeId,
}
impl Fixture {
    // port: BigIntTypeTest#setUp
    fn set_up() -> Self {
        let mut f = Self {
            base: BaseJSTypeTestCase::new(),
            function_type: TypeId(0),
            unresolved_named_type: TypeId(0),
            enum_type: TypeId(0),
            elements_type: TypeId(0),
        };
        {
            let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
            let _expr2 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
            let goog_object = _expr2;
            let _expr3 = vec![(JsString::from("goog"), goog_object)];
            let _expr4 = Arc::new(MapBasedScope::new(_expr3));
            let scope = _expr4;
            let _expr5 = FunctionTypeBuilder::new();
            let _expr6 = _expr5.with_return_type(f.base.number_type);
            let _expr7 = _expr6.build(&mut f.base.reg, &f.base.ast);
            f.function_type = _expr7;
            f.base.error_reporter.lock().unwrap().expect_all_warnings(&[
                "Bad type annotation. Unknown type not.resolved.named.type",
            ]);
            let _expr8 = -1;
            let _expr9 = -1;
            let _expr10 = f.base.reg.create_named_type(
                &f.base.ast,
                Some(scope.clone()),
                "not.resolved.named.type",
                "",
                _expr8,
                _expr9,
            );
            f.unresolved_named_type = _expr10;
            let _expr11 = EnumTypeBuilder::new();
            let _expr12 = _expr11.set_name("Enum");
            let _expr13 = _expr12.set_element_type(f.base.bigint_type);
            let _expr14 = _expr13.build(&mut f.base.reg, &f.base.ast);
            f.enum_type = _expr14;
            let _expr15 = f.enum_type.get_elements_type(&f.base.reg);
            f.elements_type = _expr15;
            _expr1.close(&mut f.base.reg, &f.base.ast);
        }
        f
    }

    // port: BigIntTypeTest#enumElementFor
    fn enum_element_for(&mut self, type_: TypeId) -> TypeId {
        let f = self;
        let _expr1 = EnumTypeBuilder::new();
        let _expr2 = _expr1.set_name("Enum");
        let _expr3 = _expr2.set_element_type(type_);
        let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);

        _expr4.get_elements_type(&f.base.reg)
    }
}
// port: BigIntTypeTest#testBigIntValueTypeIsXxx
#[test]
fn test_big_int_value_type_is_xxx() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.bigint_type.is_all_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.bigint_type.is_array_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.bigint_type.is_big_int_value_type(&f.base.reg);
    assert!(_expr3);
    let _expr4 = f.base.bigint_type.is_big_int_object_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.bigint_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.bigint_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.bigint_type.is_checked_unknown_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.bigint_type.is_constructor(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.bigint_type.is_date_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.bigint_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr10);
    let _expr11 = f.base.bigint_type.is_empty_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.bigint_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.bigint_type.is_enum_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.bigint_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.bigint_type.is_function_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.bigint_type.is_global_this_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.bigint_type.is_instance_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.bigint_type.is_named_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.bigint_type.is_native_object_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.bigint_type.is_no_object_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.bigint_type.is_no_resolved_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.bigint_type.is_no_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.bigint_type.is_null_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.bigint_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f.base.bigint_type.is_number_object_type(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.bigint_type.is_number_value_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f
        .base
        .bigint_type
        .is_object_type(&mut f.base.reg, &f.base.ast);
    assert!(!_expr27);
    let _expr28 = f.base.bigint_type.is_regexp_type(&f.base.reg);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .bigint_type
        .is_some_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(!_expr29);
    let _expr30 = f.base.bigint_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr30);
    let _expr31 = f.base.bigint_type.is_string_object_type(&f.base.reg);
    assert!(!_expr31);
    let _expr32 = f.base.bigint_type.is_string_value_type(&f.base.reg);
    assert!(!_expr32);
    let _expr33 = f.base.bigint_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr33);
    let _expr34 = f.base.bigint_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr34);
    let _expr35 = f.base.bigint_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr35);
    let _expr36 = f.base.bigint_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr36);
    let _expr37 = f.base.bigint_type.is_union_type(&f.base.reg);
    assert!(!_expr37);
    let _expr38 = f.base.bigint_type.is_void_type(&f.base.reg);
    assert!(!_expr38);
}

// port: BigIntTypeTest#testBigIntObjectTypeIsXxx
#[test]
fn test_big_int_object_type_is_xxx() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.bigint_object_type.is_all_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.bigint_object_type.is_array_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.bigint_object_type.is_big_int_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f
        .base
        .bigint_object_type
        .is_big_int_object_type(&f.base.reg);
    assert!(_expr4);
    let _expr5 = f
        .base
        .bigint_object_type
        .is_boolean_object_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.bigint_object_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f
        .base
        .bigint_object_type
        .is_checked_unknown_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.bigint_object_type.is_constructor(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.bigint_object_type.is_date_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f
        .base
        .bigint_object_type
        .is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr10);
    let _expr11 = f.base.bigint_object_type.is_empty_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.bigint_object_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.bigint_object_type.is_enum_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f
        .base
        .bigint_object_type
        .is_function_prototype_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.bigint_object_type.is_function_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.bigint_object_type.is_global_this_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.bigint_object_type.is_instance_type(&f.base.reg);
    assert!(_expr17);
    let _expr18 = f.base.bigint_object_type.is_named_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.bigint_object_type.is_native_object_type(&f.base.reg);
    assert!(_expr19);
    let _expr20 = f.base.bigint_object_type.is_no_object_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.bigint_object_type.is_no_resolved_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.bigint_object_type.is_no_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.bigint_object_type.is_null_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f
        .base
        .bigint_object_type
        .is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f.base.bigint_object_type.is_number_object_type(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.bigint_object_type.is_number_value_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f
        .base
        .bigint_object_type
        .is_object_type(&mut f.base.reg, &f.base.ast);
    assert!(_expr27);
    let _expr28 = f.base.bigint_object_type.is_regexp_type(&f.base.reg);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .bigint_object_type
        .is_some_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .bigint_object_type
        .is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr30);
    let _expr31 = f.base.bigint_object_type.is_string_object_type(&f.base.reg);
    assert!(!_expr31);
    let _expr32 = f.base.bigint_object_type.is_string_value_type(&f.base.reg);
    assert!(!_expr32);
    let _expr33 = f
        .base
        .bigint_object_type
        .is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .bigint_object_type
        .is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr34);
    let _expr35 = f.base.bigint_object_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr35);
    let _expr36 = f.base.bigint_object_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr36);
    let _expr37 = f.base.bigint_object_type.is_union_type(&f.base.reg);
    assert!(!_expr37);
    let _expr38 = f.base.bigint_object_type.is_void_type(&f.base.reg);
    assert!(!_expr38);
}

// port: BigIntTypeTest#testIsOnlyBigInt
#[test]
fn test_is_only_big_int() {
    let mut f = Fixture::set_up();
    TypeSubject::assert_type(f.base.bigint_type).is_only_big_int(&f.base.reg);
    TypeSubject::assert_type(f.base.bigint_object_type).is_only_big_int(&f.base.reg);
    TypeSubject::assert_type(f.base.all_type).is_not_only_big_int(&f.base.reg);
    TypeSubject::assert_type(f.base.unknown_type).is_not_only_big_int(&f.base.reg);
    TypeSubject::assert_type(f.base.no_type).is_not_only_big_int(&f.base.reg);
    let _expr1 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.bigint_object_type]);
    TypeSubject::assert_type(_expr1).is_not_only_big_int(&f.base.reg);
    TypeSubject::assert_type(f.base.bigint_number).is_not_only_big_int(&f.base.reg);
    let _expr2 = f.enum_element_for(f.base.bigint_type);
    TypeSubject::assert_type(_expr2).is_not_only_big_int(&f.base.reg);
    let _expr3 = f.enum_element_for(f.base.number_type);
    TypeSubject::assert_type(_expr3).is_not_only_big_int(&f.base.reg);
    let _expr4 = f.enum_element_for(f.base.bigint_number);
    TypeSubject::assert_type(_expr4).is_not_only_big_int(&f.base.reg);
    let _expr5 = f.enum_element_for(f.base.number_string);
    TypeSubject::assert_type(_expr5).is_not_only_big_int(&f.base.reg);
}

// port: BigIntTypeTest#testBigIntValueTypeIsSubtype
#[test]
fn test_big_int_value_type_is_subtype() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr1);
    let _expr2 =
        f.base
            .bigint_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr2);
    let _expr3 =
        f.base
            .bigint_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr3);
    let _expr4 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr4);
    let _expr5 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr5);
    let _expr6 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.bigint_type);
    assert!(_expr6);
    let _expr7 =
        f.base
            .bigint_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr8);
    let _expr9 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr9);
    let _expr10 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr10);
    let _expr11 =
        f.base
            .bigint_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr11);
    let _expr12 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.null_type]);
    let _expr13 = f
        .base
        .bigint_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr12);
    assert!(_expr13);
    let _expr14 =
        f.base
            .bigint_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr14);
}

// port: BigIntTypeTest#testBigIntValueObjectIsSubtype
#[test]
fn test_big_int_value_object_is_subtype() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr1);
    let _expr2 = f.base.bigint_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr2);
    let _expr3 = f.base.bigint_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr3);
    let _expr4 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr4);
    let _expr5 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr5);
    let _expr6 = f.base.bigint_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.bigint_object_type,
    );
    assert!(_expr6);
    let _expr7 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.bigint_type);
    assert!(!_expr7);
    let _expr8 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(!_expr8);
    let _expr9 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr9);
    let _expr10 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr10);
    let _expr11 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr11);
    let _expr12 = f.base.bigint_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.unresolved_named_type,
    );
    assert!(_expr12);
    let _expr13 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.base.null_type]);
    let _expr14 = f
        .base
        .bigint_object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr13);
    assert!(_expr14);
    let _expr15 =
        f.base
            .bigint_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr15);
}

// port: BigIntTypeTest#testBigIntValueTypeEquality
#[test]
fn test_big_int_value_type_equality() {
    let mut f = Fixture::set_up();
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.no_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.bigint_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.bigint_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.enum_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_type, f.base.symbol_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_type, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_type, f.base.void_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_type, f.base.null_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_type, f.base.unknown_type);
}

// port: BigIntTypeTest#testBigIntObjectTypeEquality
#[test]
fn test_big_int_object_type_equality() {
    let mut f = Fixture::set_up();
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.no_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.bigint_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.bigint_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_object_type, f.base.symbol_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_object_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.elements_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_object_type, f.base.void_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.bigint_object_type, f.base.null_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.bigint_object_type, f.base.unknown_type);
}

// port: BigIntTypeTest#testBigIntValueTypeShallowEquality
#[test]
fn test_big_int_value_type_shallow_equality() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr1);
    let _expr2 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr2);
    let _expr3 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr3);
    let _expr4 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr4);
    let _expr5 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr5);
    let _expr6 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr6);
    let _expr7 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr7);
    let _expr8 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr8);
    let _expr9 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.bigint_type,
    );
    assert!(_expr9);
    let _expr10 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr10);
    let _expr11 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr11);
    let _expr12 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr12);
    let _expr13 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr13);
    let _expr14 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr14);
    let _expr15 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr15);
    let _expr16 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr16);
    let _expr17 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr17);
    let _expr18 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr18);
    let _expr19 = f.base.bigint_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr19);
}

// port: BigIntTypeTest#testBigIntObjectTypeShallowEquality
#[test]
fn test_big_int_object_type_shallow_equality() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr1);
    let _expr2 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr2);
    let _expr3 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr3);
    let _expr4 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr4);
    let _expr5 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.boolean_object_type,
        );
    assert!(!_expr5);
    let _expr6 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr6);
    let _expr7 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr8);
    let _expr9 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.bigint_type);
    assert!(!_expr9);
    let _expr10 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.bigint_object_type,
        );
    assert!(_expr10);
    let _expr11 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.number_object_type,
        );
    assert!(!_expr11);
    let _expr12 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr12);
    let _expr13 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr13);
    let _expr14 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr14);
    let _expr15 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.symbol_object_type,
        );
    assert!(!_expr15);
    let _expr16 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr16);
    let _expr17 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.string_object_type,
        );
    assert!(!_expr17);
    let _expr18 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr18);
    let _expr19 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr19);
    let _expr20 = f
        .base
        .bigint_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr20);
}

// port: BigIntTypeTest#testGetLeastSupertypeForValueType
#[test]
fn test_get_least_supertype_for_value_type() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .bigint_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr1).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr2 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.string_object_type]);
    let _expr3 = f.base.bigint_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, _expr2);
    let _expr4 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.symbol_object_type]);
    let _expr5 = f.base.bigint_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, _expr4);
    let _expr6 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.symbol_type]);
    let _expr7 =
        f.base
            .bigint_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, _expr6);
    let _expr8 =
        f.base
            .bigint_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.bigint_type);
    TypeSubject::assert_type(_expr8).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.bigint_type);
    let _expr9 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.function_type]);
    let _expr10 =
        f.base
            .bigint_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, _expr9);
    let _expr11 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.object_type]);
    let _expr12 =
        f.base
            .bigint_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, _expr11);
    let _expr13 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.date_type]);
    let _expr14 =
        f.base
            .bigint_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, _expr13);
    let _expr15 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.regexp_type]);
    let _expr16 =
        f.base
            .bigint_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr16).is_equal_to(&mut f.base.reg, &f.base.ast, _expr15);
}

// port: BigIntTypeTest#testGetLeastSupertypeForObjectType
#[test]
fn test_get_least_supertype_for_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    TypeSubject::assert_type(_expr1).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr2 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.base.string_object_type]);
    let _expr3 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, _expr2);
    let _expr4 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.base.symbol_object_type]);
    let _expr5 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, _expr4);
    let _expr6 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.base.symbol_type]);
    let _expr7 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, _expr6);
    let _expr8 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.bigint_object_type,
    );
    TypeSubject::assert_type(_expr8).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.bigint_object_type,
    );
    let _expr9 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.function_type]);
    let _expr10 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, _expr9);
    let _expr11 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.base.object_type]);
    let _expr12 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, _expr11);
    let _expr13 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.base.date_type]);
    let _expr14 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, _expr13);
    let _expr15 = f
        .base
        .create_union_type(&[f.base.bigint_object_type, f.base.regexp_type]);
    let _expr16 = f.base.bigint_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    TypeSubject::assert_type(_expr16).is_equal_to(&mut f.base.reg, &f.base.ast, _expr15);
}

// port: BigIntTypeTest#testBigIntAutobox
#[test]
fn test_big_int_autobox() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.bigint_type.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(f.base.bigint_object_type).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        _expr1,
    );
}

// port: BigIntTypeTest#testCanBeCalled
#[test]
fn test_can_be_called() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .bigint_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr1);
    let _expr2 = f
        .base
        .bigint_object_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr2);
}

// port: BigIntTypeTest#testIsNullable
#[test]
fn test_is_nullable() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.bigint_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr1);
    let _expr2 = f.base.bigint_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr2);
    let _expr3 = f
        .base
        .bigint_object_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr3);
    let _expr4 = f
        .base
        .bigint_object_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr4);
}

// port: BigIntTypeTest#testMatchesXxx
#[test]
fn test_matches_xxx() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .bigint_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr1);
    let _expr2 = f
        .base
        .bigint_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr2);
    let _expr3 = f
        .base
        .bigint_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr3);
    let _expr4 = f
        .base
        .bigint_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr4);
    let _expr5 = f
        .base
        .bigint_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr5);
    let _expr6 = f
        .base
        .bigint_object_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr6);
    let _expr7 = f
        .base
        .bigint_object_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr7);
    let _expr8 = f
        .base
        .bigint_object_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr8);
}

// port: BigIntTypeTest#testToString
#[test]
fn test_to_string() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.bigint_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr1, "bigint");
    let _expr2 = f.base.bigint_type.has_display_name(&f.base.reg);
    assert!(_expr2);
    let _expr3 = f.base.bigint_type.get_display_name(&f.base.reg);
    assert_eq!(_expr3, Some("bigint".into()));
    let _expr4 = f
        .base
        .bigint_object_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr4, "BigInt");
    let _expr5 = f.base.bigint_object_type.has_display_name(&f.base.reg);
    assert!(_expr5);
    let _expr6 = f.base.bigint_object_type.get_display_name(&f.base.reg);
    assert_eq!(_expr6, Some("BigInt".into()));
}

// port: BigIntTypeTest#testIsNotNominalConstructor
#[test]
fn test_is_not_nominal_constructor() {
    let f = Fixture::set_up();
    let _expr1 = f
        .base
        .bigint_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f
        .base
        .bigint_object_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.bigint_object_type.get_constructor(&f.base.reg);
    let _expr4 = _expr3
        .unwrap()
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(_expr4);
}

// port: BigIntTypeTest#testResolvesToSame
#[test]
fn test_resolves_to_same() {
    let mut f = Fixture::set_up();
    let _expr1 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.bigint_type);
    let _expr2 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.bigint_object_type);
}
