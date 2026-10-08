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
//   test/com/google/javascript/rhino/jstype/EnumElementTypeTest.java.

use closure_jstype::{
    TypeId,
    enum_type::EnumTypeBuilder,
    prelude::*,
    testing::{base_js_type_test_case::BaseJSTypeTestCase, type_subject::TypeSubject},
};
struct Fixture {
    base: BaseJSTypeTestCase,
}
impl Fixture {
    fn set_up() -> Self {
        Self {
            base: BaseJSTypeTestCase::new(),
        }
    }
    // port: EnumElementTypeTest#createEnumType
    fn create_enum_type(&mut self, name: &str, element_type: TypeId) -> TypeId {
        let f = self;
        let _expr1 = EnumTypeBuilder::new();
        let _expr2 = _expr1.set_name(name);
        let _expr3 = _expr2.set_element_type(element_type);

        _expr3.build(&mut f.base.reg, &f.base.ast)
    }
}
// port: EnumElementTypeTest#testSubtypeRelation
#[test]
fn test_subtype_relation() {
    let mut f = Fixture::set_up();
    let _expr1 = f.create_enum_type("typeA", f.base.number_type);
    let _expr2 = _expr1.get_elements_type(&f.base.reg);
    let type_a = _expr2;
    let _expr3 = f.create_enum_type("typeB", f.base.number_type);
    let _expr4 = _expr3.get_elements_type(&f.base.reg);
    let type_b = _expr4;
    let _expr5 = type_a.is_subtype(&mut f.base.reg, &f.base.ast, type_b);
    assert!(!_expr5);
    let _expr6 = type_b.is_subtype(&mut f.base.reg, &f.base.ast, type_a);
    assert!(!_expr6);
    let _expr7 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, type_b);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, type_a);
    assert!(!_expr8);
    let _expr9 = type_a.is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr9);
    let _expr10 = type_b.is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr10);
}

// port: EnumElementTypeTest#testGetGreatestSubtype
#[test]
fn test_get_greatest_subtype() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let _expr2 = f.create_enum_type("typeA", _expr1);
    let _expr3 = _expr2.get_elements_type(&f.base.reg);
    let type_a = _expr3;
    let _expr4 = type_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let strings_of_a = _expr4;
    let _expr5 = strings_of_a.is_empty_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = strings_of_a.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr6, "typeA<string>");
    let _expr7 = strings_of_a.is_subtype_of(&mut f.base.reg, &f.base.ast, type_a);
    assert!(_expr7);
    let _expr8 = f
        .base
        .number_type
        .get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_a);
    let numbers_of_a = _expr8;
    let _expr9 = numbers_of_a.is_empty_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = numbers_of_a.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr10, "typeA<number>");
    let _expr11 = numbers_of_a.is_subtype_of(&mut f.base.reg, &f.base.ast, type_a);
    assert!(_expr11);
}

// port: EnumElementTypeTest#testGetGreatestSubtype_twoEnumElementTypes
#[test]
fn test_get_greatest_subtype_two_enum_element_types() {
    let mut f = Fixture::set_up();
    let _expr1 = f.create_enum_type("typeA", f.base.number_type);
    let _expr2 = _expr1.get_elements_type(&f.base.reg);
    let type_a = _expr2;
    let _expr3 = f.create_enum_type("typeB", f.base.number_type);
    let _expr4 = _expr3.get_elements_type(&f.base.reg);
    let type_b = _expr4;
    let _expr5 = closure_jstype::enum_element_type::get_greatest_subtype(
        type_a,
        &mut f.base.reg,
        &f.base.ast,
        type_b,
    );
    let greatest_subtype = _expr5;
    TypeSubject::assert_type(greatest_subtype).is_subtype_of(&mut f.base.reg, &f.base.ast, type_a);
    TypeSubject::assert_type(greatest_subtype).is_subtype_of(&mut f.base.reg, &f.base.ast, type_b);
    TypeSubject::assert_type(greatest_subtype).is_not_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        type_a,
    );
    TypeSubject::assert_type(greatest_subtype).is_not_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        type_b,
    );
    TypeSubject::assert_type(type_a).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        greatest_subtype.unwrap(),
    );
    TypeSubject::assert_type(type_b).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        greatest_subtype.unwrap(),
    );
}

// port: EnumElementTypeTest#testGetGreatestSubtype_twoEnumElementTypes_postResolution
#[test]
fn test_get_greatest_subtype_two_enum_element_types_post_resolution() {
    let mut f = Fixture::set_up();
    let _expr1 = f.create_enum_type("typeA", f.base.number_type);
    let _expr2 = _expr1.get_elements_type(&f.base.reg);
    let type_a = _expr2;
    let _expr3 = f.create_enum_type("typeB", f.base.number_type);
    let _expr4 = _expr3.get_elements_type(&f.base.reg);
    let type_b = _expr4;
    let _expr5 = closure_jstype::enum_element_type::get_greatest_subtype(
        type_a,
        &mut f.base.reg,
        &f.base.ast,
        type_b,
    );
    let greatest_subtype = _expr5;
    TypeSubject::assert_type(greatest_subtype).is_subtype_of(&mut f.base.reg, &f.base.ast, type_a);
    TypeSubject::assert_type(greatest_subtype).is_subtype_of(&mut f.base.reg, &f.base.ast, type_b);
    TypeSubject::assert_type(greatest_subtype).is_not_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        type_a,
    );
    TypeSubject::assert_type(greatest_subtype).is_not_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        type_b,
    );
    TypeSubject::assert_type(type_a).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        greatest_subtype.unwrap(),
    );
    TypeSubject::assert_type(type_b).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        greatest_subtype.unwrap(),
    );
}

// port: EnumElementTypeTest#testEqualityOfEnumTypes_withSameReferenceName
#[test]
fn test_equality_of_enum_types_with_same_reference_name() {
    let mut f = Fixture::set_up();
    let _expr1 = f.create_enum_type("Foo", f.base.number_type);
    let first_foo = _expr1;
    let _expr2 = f.create_enum_type("Foo", f.base.number_type);
    let second_foo = _expr2;
    TypeSubject::assert_type(first_foo).is_not_equal_to(&mut f.base.reg, &f.base.ast, second_foo);
    let _expr3 = first_foo.get_elements_type(&f.base.reg);
    let _expr4 = second_foo.get_elements_type(&f.base.reg);
    TypeSubject::assert_type(_expr3).is_not_equal_to(&mut f.base.reg, &f.base.ast, _expr4);
}
