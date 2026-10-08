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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/rhino/jstype/UnionTypeTest.java.

use closure_jstype::{
    TypeId,
    function_type::FunctionTypeBuilder,
    named_type::NamedTypeBuilder,
    prelude::*,
    testing::{
        asserts::Asserts, base_js_type_test_case::BaseJSTypeTestCase,
        map_based_scope::MapBasedScope, type_subject::TypeSubject,
    },
    union_type::UnionTypeBuilder,
};
use closure_rhino::jscomp_base::Tri;
use std::sync::Arc;
struct Fixture {
    base: BaseJSTypeTestCase,
    base_type: TypeId,
    sub1: TypeId,
    sub2: TypeId,
    sub3: TypeId,
    base_name_conflict: TypeId,
}
impl Fixture {
    // port: UnionTypeTest#setUp
    fn set_up() -> Self {
        let mut f = Self {
            base: BaseJSTypeTestCase::new(),
            base_type: TypeId(0),
            sub1: TypeId(0),
            sub2: TypeId(0),
            sub3: TypeId(0),
            base_name_conflict: TypeId(0),
        };
        {
            let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
            let _expr2 = FunctionTypeBuilder::new();
            let _expr3 = _expr2.for_constructor();
            let _expr4 = _expr3.with_name("Base");
            let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
            let _expr6 = _expr5.get_instance_type(&f.base.reg);
            f.base_type = _expr6.unwrap();
            let _expr7 = FunctionTypeBuilder::new();
            let _expr8 = _expr7.for_constructor();
            let _expr9 = _expr8.with_name("Base");
            let _expr10 = _expr9.build(&mut f.base.reg, &f.base.ast);
            let _expr11 = _expr10.get_instance_type(&f.base.reg);
            f.base_name_conflict = _expr11.unwrap();
            let _expr12 = FunctionTypeBuilder::new();
            let _expr13 = _expr12.for_constructor();
            let _expr14 = _expr13.with_name("Sub1");
            let _expr15 = _expr14.with_prototype_based_on(f.base_type);
            let _expr16 = _expr15.build(&mut f.base.reg, &f.base.ast);
            let _expr17 = _expr16.get_instance_type(&f.base.reg);
            f.sub1 = _expr17.unwrap();
            let _expr18 = FunctionTypeBuilder::new();
            let _expr19 = _expr18.for_constructor();
            let _expr20 = _expr19.with_name("Sub2");
            let _expr21 = _expr20.with_prototype_based_on(f.base_type);
            let _expr22 = _expr21.build(&mut f.base.reg, &f.base.ast);
            let _expr23 = _expr22.get_instance_type(&f.base.reg);
            f.sub2 = _expr23.unwrap();
            let _expr24 = FunctionTypeBuilder::new();
            let _expr25 = _expr24.for_constructor();
            let _expr26 = _expr25.with_name("Sub3");
            let _expr27 = _expr26.with_prototype_based_on(f.base_type);
            let _expr28 = _expr27.build(&mut f.base.reg, &f.base.ast);
            let _expr29 = _expr28.get_instance_type(&f.base.reg);
            f.sub3 = _expr29.unwrap();
            _expr1.close(&mut f.base.reg, &f.base.ast);
        }
        f
    }

    // port: UnionTypeTest#assertTypeCanAssignToItself
    fn assert_type_can_assign_to_itself(&mut self, type_: TypeId) {
        let f = self;
        let _expr1 = type_.is_subtype_of(&mut f.base.reg, &f.base.ast, type_);
        assert!(_expr1);
    }
}
fn assert_type_multiset(
    reg: &mut closure_jstype::JSTypeRegistry,
    ast: &closure_rhino::node::Ast,
    actual: &[TypeId],
    expected: &[TypeId],
) {
    assert_eq!(actual.len(), expected.len());
    let mut remaining = expected.to_vec();
    for actual in actual {
        let i = remaining
            .iter()
            .position(|expected| actual.equals(reg, ast, *expected))
            .expect("missing collection value");
        remaining.remove(i);
    }
    assert!(remaining.is_empty());
}
// port: UnionTypeTest#testUnionType
#[test]
fn test_union_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.null_type, f.base.string_object_type]);
    let null_or_string = _expr1;
    let _expr2 = f
        .base
        .create_union_type(&[f.base.string_object_type, f.base.null_type]);
    let string_or_null = _expr2;
    TypeSubject::assert_type(string_or_null).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        null_or_string,
    );
    TypeSubject::assert_type(null_or_string).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        string_or_null,
    );
    let _expr3 = f
        .base
        .create_union_type(&[f.base.void_type, f.base.number_type]);
    f.assert_type_can_assign_to_itself(_expr3);
    let _expr5 =
        f.base
            .create_union_type(&[f.base.number_type, f.base.string_type, f.base.object_type]);
    f.assert_type_can_assign_to_itself(_expr5);
    let _expr7 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.boolean_type]);
    f.assert_type_can_assign_to_itself(_expr7);
    let _expr9 = f.base.create_union_type(&[f.base.void_type]);
    f.assert_type_can_assign_to_itself(_expr9);
    let _expr11 = null_or_string.find_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr11).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr12 = null_or_string.find_property_type(&mut f.base.reg, &f.base.ast, "lengthx");
    assert!(_expr12.is_none());
    let _expr13 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, null_or_string);
}

// port: UnionTypeTest#testUnionOfNullAndUnresolvedNamedType
#[test]
fn test_union_of_null_and_unresolved_named_type() {
    let mut f = Fixture::set_up();
    f.base
        .error_reporter
        .lock()
        .unwrap()
        .expect_all_warnings(&["Bad type annotation. Unknown type not.resolved.named.type"]);
    let _expr1 = -1;
    let _expr2 = -1;
    let _expr3 = f.base.reg.create_named_type(
        &f.base.ast,
        Some(Arc::new(MapBasedScope::empty_scope()).clone()),
        "not.resolved.named.type",
        "",
        _expr1,
        _expr2,
    );
    let unresolved_named_type = _expr3;
    let _expr4 = f
        .base
        .create_union_type(&[f.base.null_type, unresolved_named_type]);
    let null_or_unknown = _expr4;
    let _expr5 = null_or_unknown.is_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(_expr5);
    let _expr6 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, null_or_unknown);
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, null_or_unknown);
    let _expr7 =
        null_or_unknown.get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, null_or_unknown);
    let _expr8 =
        f.base
            .null_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, null_or_unknown);
    TypeSubject::assert_type(_expr8).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr9 =
        null_or_unknown.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    TypeSubject::assert_type(_expr9).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr10 = f
        .base
        .null_type
        .differs_from(&mut f.base.reg, &f.base.ast, null_or_unknown);
    assert!(_expr10);
    let _expr11 = null_or_unknown.differs_from(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(_expr11);
    let _expr12 = null_or_unknown.differs_from(&mut f.base.reg, &f.base.ast, unresolved_named_type);
    assert!(!_expr12);
    let _expr13 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, null_or_unknown);
    assert!(_expr13);
    let _expr14 = unresolved_named_type.is_subtype(&mut f.base.reg, &f.base.ast, null_or_unknown);
    assert!(_expr14);
    let _expr15 = null_or_unknown.is_subtype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(_expr15);
    let _expr16 = null_or_unknown.restrict_by_not_null_or_undefined(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr16).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        unresolved_named_type,
    );
}

// port: UnionTypeTest#testGreatestSubtypeUnionTypes1
#[test]
fn test_greatest_subtype_union_types1() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.create_nullable_type(f.base.string_type);
    let _expr2 = f.base.create_nullable_type(f.base.number_type);
    let _expr3 = _expr1.get_greatest_subtype(&mut f.base.reg, &f.base.ast, _expr2);
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
}

// port: UnionTypeTest#testGreatestSubtypeUnionTypes2
#[test]
fn test_greatest_subtype_union_types2() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.create_union_type(&[f.sub1, f.sub2]);
    let sub_union = _expr1;
    let _expr2 = sub_union.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base_type);
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, sub_union);
}

// port: UnionTypeTest#testGreatestSubtypeUnionTypes3
#[test]
fn test_greatest_subtype_union_types3() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .create_union_type(&[f.base.null_type, f.base.void_type, f.base.number_type]);
    let nullable_optional_number = _expr1;
    let _expr2 = f
        .base
        .create_union_type(&[f.base.void_type, f.base.null_type]);
    let null_undefined = _expr2;
    let _expr3 =
        null_undefined.get_greatest_subtype(&mut f.base.reg, &f.base.ast, nullable_optional_number);
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, null_undefined);
    let _expr4 =
        nullable_optional_number.get_greatest_subtype(&mut f.base.reg, &f.base.ast, null_undefined);
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, null_undefined);
}

// port: UnionTypeTest#testGreatestSubtypeUnionTypes4
#[test]
fn test_greatest_subtype_union_types4() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.null_type, f.sub1, f.sub2]);
    let union = _expr1;
    let _expr2 = union.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base_type);
    let _expr3 = f.base.create_union_type(&[f.sub1, f.sub2]);
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, _expr3);
}

// port: UnionTypeTest#testGreatestSubtypeUnionTypes5
#[test]
fn test_greatest_subtype_union_types5() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.create_union_type(&[f.sub1, f.sub2]);
    let sub_union = _expr1;
    let _expr2 =
        sub_union.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    TypeSubject::assert_type(_expr2).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
}

// port: UnionTypeTest#testSubtypingUnionTypes
#[test]
fn test_subtyping_union_types() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.boolean_type, f.base.string_type]);
    let _expr2 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr1);
    assert!(_expr2);
    let _expr3 = f
        .base
        .create_union_type(&[f.base.boolean_type, f.base.string_type]);
    let _expr4 = f
        .base
        .create_union_type(&[f.base.boolean_type, f.base.string_type]);
    let _expr5 = _expr3.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr4);
    assert!(_expr5);
    let _expr6 = f
        .base
        .create_union_type(&[f.base.boolean_type, f.base.string_type]);
    let _expr7 =
        f.base
            .create_union_type(&[f.base.boolean_type, f.base.string_type, f.base.null_type]);
    let _expr8 = _expr6.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr7);
    assert!(_expr8);
    let _expr9 = f
        .base
        .create_union_type(&[f.base.boolean_type, f.base.string_type]);
    let _expr10 =
        f.base
            .create_union_type(&[f.base.boolean_type, f.base.string_type, f.base.null_type]);
    let _expr11 = _expr9.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr10);
    assert!(_expr11);
    let _expr12 = f.base.create_union_type(&[f.base.boolean_type]);
    let _expr13 =
        f.base
            .create_union_type(&[f.base.boolean_type, f.base.string_type, f.base.null_type]);
    let _expr14 = _expr12.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr13);
    assert!(_expr14);
    let _expr15 = f.base.create_union_type(&[f.base.string_type]);
    let _expr16 =
        f.base
            .create_union_type(&[f.base.boolean_type, f.base.string_type, f.base.null_type]);
    let _expr17 = _expr15.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr16);
    assert!(_expr17);
    let _expr18 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.null_type]);
    let _expr19 = _expr18.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr19);
    let _expr20 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.regexp_type]);
    let _expr21 = _expr20.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr21);
    let _expr22 = f.base.create_union_type(&[f.sub1, f.sub2]);
    let _expr23 = _expr22.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base_type);
    assert!(_expr23);
    let _expr24 = f.base.create_union_type(&[f.sub1, f.sub2]);
    let _expr25 = _expr24.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr25);
    let _expr26 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.null_type]);
    let _expr27 = _expr26.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr27);
    let _expr28 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.null_type]);
    let _expr29 = _expr28.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .create_union_type(&[f.base.no_object_type, f.base.null_type]);
    let _expr31 = _expr30.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_number_string);
    assert!(_expr32);
    let _expr33 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_number_string);
    assert!(_expr33);
    let _expr34 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_number_string);
    assert!(_expr34);
    let _expr35 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_number_string,
    );
    assert!(_expr35);
    let _expr36 = f.base.number_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_string_boolean,
    );
    assert!(_expr36);
    let _expr37 = f.base.boolean_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_string_boolean,
    );
    assert!(_expr37);
    let _expr38 = f.base.string_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_string_boolean,
    );
    assert!(_expr38);
    let _expr39 = f.base.number_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_number_string_boolean,
    );
    assert!(_expr39);
    let _expr40 = f.base.object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_number_string_boolean,
    );
    assert!(_expr40);
    let _expr41 = f.base.string_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_number_string_boolean,
    );
    assert!(_expr41);
    let _expr42 = f.base.boolean_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_number_string_boolean,
    );
    assert!(_expr42);
    let _expr43 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_number_string_boolean,
    );
    assert!(_expr43);
}

// port: UnionTypeTest#testSpecialUnionCanAssignTo
#[test]
fn test_special_union_can_assign_to() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.number_object_type]);
    let numbers = _expr1;
    let _expr2 = numbers.is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr2);
    let _expr3 = numbers.is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr3);
    let _expr4 = numbers.is_subtype(&mut f.base.reg, &f.base.ast, f.sub1);
    assert!(!_expr4);
    let _expr5 = f
        .base
        .create_union_type(&[f.base.string_object_type, f.base.string_type]);
    let strings = _expr5;
    let _expr6 = strings.is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr6);
    let _expr7 = strings.is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr7);
    let _expr8 = strings.is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr8);
    let _expr9 = f
        .base
        .create_union_type(&[f.base.boolean_object_type, f.base.boolean_type]);
    let booleans = _expr9;
    let _expr10 = booleans.is_subtype(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr10);
    let _expr11 = booleans.is_subtype(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr11);
    let _expr12 = booleans.is_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr12);
    let _expr13 = f
        .base
        .create_union_type(&[f.base.unknown_type, f.base.date_type]);
    let unknown = _expr13;
    let _expr14 = unknown.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(_expr14);
    let _expr15 = f
        .base
        .create_union_type(&[f.base.string_object_type, f.base.date_type]);
    let string_date = _expr15;
    let _expr16 = string_date.is_subtype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr16);
    let _expr17 = string_date.is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr17);
    let _expr18 = string_date.is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr18);
}

// port: UnionTypeTest#testCreateUnionType
#[test]
fn test_create_union_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.number_type, f.base.date_type]);
    let opt_number = _expr1;
    let _expr2 = opt_number.contains(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr2);
    let _expr3 = opt_number.contains(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr3);
    let _expr4 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.string_object_type, f.base.date_type]);
    let _expr5 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.regexp_type, _expr4]);
    let opt_union = _expr5;
    let _expr6 = opt_union.contains(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr6);
    let _expr7 = opt_union.contains(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(_expr7);
    let _expr8 = opt_union.contains(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(_expr8);
}

// port: UnionTypeTest#testUnionWithUnknown
#[test]
fn test_union_with_unknown() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.unknown_type, f.base.null_type]);
    let _expr2 = _expr1.is_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(_expr2);
}

// port: UnionTypeTest#testGetRestrictedUnion1
#[test]
fn test_get_restricted_union1() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let num_str = _expr1;
    let _expr2 = num_str.get_restricted_union(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
}

// port: UnionTypeTest#testGetRestrictedUnion2
#[test]
fn test_get_restricted_union2() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.null_type, f.sub1, f.sub2]);
    let num_str = _expr1;
    let _expr2 = num_str.get_restricted_union(&mut f.base.reg, &f.base.ast, f.base_type);
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
}

// port: UnionTypeTest#testEquals
#[test]
fn test_equals() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let type_ = _expr1;
    let _expr2 = type_.equals(&mut f.base.reg, &f.base.ast, None);
    assert!(!_expr2);
    let _expr3 = type_.equals(&mut f.base.reg, &f.base.ast, type_);
    assert!(_expr3);
}

// port: UnionTypeTest#testProxyUnionType
#[test]
fn test_proxy_union_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let string_or_number = _expr1;
    let _expr2 = f
        .base
        .create_union_type(&[f.base.boolean_type, f.base.string_type]);
    let string_or_boolean = _expr2;
    let _expr3 =
        string_or_number.get_least_supertype(&mut f.base.reg, &f.base.ast, string_or_boolean);
    let _expr4 = _expr3.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr4, "(boolean|number|string)");
    let _expr5 =
        string_or_number.get_greatest_subtype(&mut f.base.reg, &f.base.ast, string_or_boolean);
    let _expr6 = _expr5.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr6, "string");
    let _expr7 =
        string_or_number.test_for_equality(&mut f.base.reg, &f.base.ast, string_or_boolean);
    assert_eq!(_expr7, Some(Tri::UNKNOWN));
    let _expr8 =
        string_or_number.get_types_under_equality(&mut f.base.reg, &f.base.ast, string_or_boolean);
    let _expr9 = _expr8
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr9, "(number|string)");
    let _expr10 = string_or_number.get_types_under_shallow_equality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean,
    );
    let _expr11 = _expr10
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr11, "string");
    let _expr12 = string_or_number.get_types_under_inequality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean,
    );
    let _expr13 = _expr12
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr13, "(number|string)");
    let _expr14 = string_or_number.get_types_under_shallow_inequality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean,
    );
    let _expr15 = _expr14
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr15, "(number|string)");
    let _expr16 = closure_jstype::proxy_object_type::create(
        &mut f.base.reg,
        &f.base.ast,
        string_or_number,
        None,
    );
    let string_or_number_proxy = _expr16;
    let _expr17 = closure_jstype::proxy_object_type::create(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean,
        None,
    );
    let string_or_boolean_proxy = _expr17;
    let _expr18 = string_or_number_proxy.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean_proxy,
    );
    let _expr19 = _expr18.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr19, "(boolean|number|string)");
    let _expr20 = string_or_number_proxy.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean_proxy,
    );
    let _expr21 = _expr20.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr21, "string");
    let _expr22 = string_or_number_proxy.test_for_equality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean_proxy,
    );
    assert_eq!(_expr22, Some(Tri::UNKNOWN));
    let _expr23 = string_or_number_proxy.get_types_under_equality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean_proxy,
    );
    let _expr24 = _expr23
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr24, "(number|string)");
    let _expr25 = string_or_number_proxy.get_types_under_shallow_equality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean_proxy,
    );
    let _expr26 = _expr25
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr26, "string");
    let _expr27 = string_or_number_proxy.get_types_under_inequality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean_proxy,
    );
    let _expr28 = _expr27
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr28, "(number|string)");
    let _expr29 = string_or_number_proxy.get_types_under_shallow_inequality(
        &mut f.base.reg,
        &f.base.ast,
        string_or_boolean_proxy,
    );
    let _expr30 = _expr29
        .type_a
        .unwrap()
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr30, "(number|string)");
}

// port: UnionTypeTest#testCollapseUnion1
#[test]
fn test_collapse_union1() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.number_type, f.base.string_type]);
    let _expr2 = _expr1.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr3 = _expr2.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr3, "*");
}

// port: UnionTypeTest#testCollapseUnion2
#[test]
fn test_collapse_union2() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.unknown_type, f.base.number_type]);
    let _expr2 = _expr1.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr3 = _expr2.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr3, "?");
    let _expr4 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.number_type, f.base.unknown_type]);
    let _expr5 = _expr4.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr6 = _expr5.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr6, "?");
}

// port: UnionTypeTest#testCollapseUnion3
#[test]
fn test_collapse_union3() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.array_type, f.base.date_type]);
    let _expr2 = _expr1.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr3 = _expr2.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr3, "Object");
    let _expr4 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.array_type, f.base.object_type]);
    let _expr5 = _expr4.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr6 = _expr5.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr6, "Object");
    let _expr7 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base_type, f.sub1]);
    let _expr8 = _expr7.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr9 = _expr8.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr9, "Base");
    let _expr10 = f.base.reg.create_union_type(&f.base.ast, &[f.sub1, f.sub2]);
    let _expr11 = _expr10.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr12 = _expr11.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr12, "Base");
    let _expr13 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.sub1, f.sub2, f.sub3]);
    let _expr14 = _expr13.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr15 = _expr14.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr15, "Base");
}

// port: UnionTypeTest#testCollapseUnion4
#[test]
fn test_collapse_union4() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.object_type, f.base.string_type]);
    let _expr2 = _expr1.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr3 = _expr2.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr3, "*");
    let _expr4 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.string_type, f.base.object_type]);
    let _expr5 = _expr4.collapse_union(&mut f.base.reg, &f.base.ast);
    let _expr6 = _expr5.unwrap().to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr6, "*");
}

// port: UnionTypeTest#testCollapseProxyUnion
#[test]
fn test_collapse_proxy_union() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::proxy_object_type::create(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
        None,
    );
    let type_ = _expr1;
    let _expr2 = type_.collapse_union(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr2, Some(type_));
}

// port: UnionTypeTest#testShallowEquality
#[test]
fn test_shallow_equality() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.array_type, f.base.string_type]);
    let _expr2 =
        _expr1.can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr2);
}

// port: UnionTypeTest#testUnionOfSingleType_equalToNonUnion
#[test]
fn test_union_of_single_type_equal_to_non_union() {
    let mut f = Fixture::set_up();
    let number_union;
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = NamedTypeBuilder::new(&f.base.reg, "NumberProxy");
        let _expr3 = _expr2.set_resolution_kind(closure_jstype::named_type::ResolutionKind::NONE);
        let _expr4 = _expr3.set_referenced_type(f.base.number_type);
        let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
        let number_proxy = _expr5;
        let _expr6 = f
            .base
            .reg
            .create_union_type(&f.base.ast, &[f.base.number_type, number_proxy]);
        number_union = _expr6;
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
    let _expr7 = number_union.get_alternates(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr7.len(), 1);
    TypeSubject::assert_type(number_union).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    TypeSubject::assert_type(f.base.number_type).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        number_union,
    );
}

// port: UnionTypeTest#testUnionOfResolvedAlternates_isNotRebuilt_duringResolution
#[test]
fn test_union_of_resolved_alternates_is_not_rebuilt_during_resolution() {
    let mut f = Fixture::set_up();
    TypeSubject::assert_type(f.base.number_type).is_resolved(&f.base.reg);
    TypeSubject::assert_type(f.base.string_type).is_resolved(&f.base.reg);
    let union;
    let original_alternates;
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = f
            .base
            .reg
            .create_union_type(&f.base.ast, &[f.base.number_type, f.base.string_type]);
        union = _expr2;
        TypeSubject::assert_type(union).is_unresolved(&f.base.reg);
        let _expr3 = union.get_alternates(&mut f.base.reg, &f.base.ast);
        original_alternates = _expr3;
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
    TypeSubject::assert_type(union).is_resolved(&f.base.reg);
    let _expr4 = union.get_alternates(&mut f.base.reg, &f.base.ast);
    assert!(Arc::ptr_eq(&original_alternates, &_expr4));
}

// port: UnionTypeTest#testUnionOfResolvedAlternates_isRebuilt_ifAlternatedAddedWhileUnresolved
#[test]
fn test_union_of_resolved_alternates_is_rebuilt_if_alternated_added_while_unresolved() {
    let mut f = Fixture::set_up();
    TypeSubject::assert_type(f.base.number_type).is_resolved(&f.base.reg);
    let mut _expr1 = UnionTypeBuilder::new();
    _expr1.add_alternate(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let mut builder = _expr1;
    {
        let mut _expr2 = f.base.reg.get_resolver().open_for_definition();
        let _expr3 = NamedTypeBuilder::new(&f.base.reg, "NumberProxy");
        let _expr4 = _expr3.set_resolution_kind(closure_jstype::named_type::ResolutionKind::NONE);
        let _expr5 = _expr4.set_referenced_type(f.base.number_type);
        let _expr6 = _expr5.build(&mut f.base.reg, &f.base.ast);
        let number_proxy = _expr6;
        TypeSubject::assert_type(number_proxy).is_unresolved(&f.base.reg);
        builder.add_alternate(&mut f.base.reg, &f.base.ast, number_proxy);
        _expr2.close(&mut f.base.reg, &f.base.ast);
    }
    let _expr7 = builder.build(&mut f.base.reg, &f.base.ast);
    let result = _expr7;
    let _expr8 = result.get_union_members(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr8.as_ref().unwrap().len(), 1);
    TypeSubject::assert_type(result).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
}

// port: UnionTypeTest#testUnionWithUnresolvedAlternate_isNotRebuilt_ifNoAlternateHasResolved
#[test]
fn test_union_with_unresolved_alternate_is_not_rebuilt_if_no_alternate_has_resolved() {
    let mut f = Fixture::set_up();
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = NamedTypeBuilder::new(&f.base.reg, "NumberProxy");
        let _expr3 = _expr2.set_resolution_kind(closure_jstype::named_type::ResolutionKind::NONE);
        let _expr4 = _expr3.set_referenced_type(f.base.number_type);
        let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
        let number_proxy = _expr5;
        TypeSubject::assert_type(number_proxy).is_unresolved(&f.base.reg);
        let _expr6 = f
            .base
            .reg
            .create_union_type(&f.base.ast, &[f.base.string_type, number_proxy]);
        let union = _expr6;
        TypeSubject::assert_type(union).is_unresolved(&f.base.reg);
        let _expr7 = union.get_alternates(&mut f.base.reg, &f.base.ast);
        let first_read = _expr7;
        let _expr8 = union.get_alternates(&mut f.base.reg, &f.base.ast);
        let second_read = _expr8;
        assert!(Arc::ptr_eq(&second_read, &first_read));
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
}

// port: UnionTypeTest#testUnionWithAlternateThatResolvesToUnion_isStillFlattened
#[test]
fn test_union_with_alternate_that_resolves_to_union_is_still_flattened() {
    let mut f = Fixture::set_up();
    let union;
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = NamedTypeBuilder::new(&f.base.reg, "UnionProxy");
        let _expr3 = _expr2.set_resolution_kind(closure_jstype::named_type::ResolutionKind::NONE);
        let _expr4 = f
            .base
            .reg
            .create_union_type(&f.base.ast, &[f.base.number_type, f.base.boolean_type]);
        let _expr5 = _expr3.set_referenced_type(_expr4);
        let _expr6 = _expr5.build(&mut f.base.reg, &f.base.ast);
        let union_proxy = _expr6;
        let _expr7 = f
            .base
            .reg
            .create_union_type(&f.base.ast, &[f.base.string_type, union_proxy]);
        union = _expr7;
        let _expr8 = union.get_alternates(&mut f.base.reg, &f.base.ast);
        let first_read = _expr8;
        let _expr9 = union.get_alternates(&mut f.base.reg, &f.base.ast);
        assert!(Arc::ptr_eq(&_expr9, &first_read));
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
    TypeSubject::assert_type(union).is_resolved(&f.base.reg);
    let _expr10 = union.get_alternates(&mut f.base.reg, &f.base.ast);
    assert_type_multiset(
        &mut f.base.reg,
        &f.base.ast,
        &_expr10,
        &[f.base.string_type, f.base.number_type, f.base.boolean_type],
    );
}

// port: UnionTypeTest#testToStringNameConflict
#[test]
fn test_to_string_name_conflict() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base_type, f.base_name_conflict]);
    let _expr2 = _expr1.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr2, "(Base|Base)");
}
