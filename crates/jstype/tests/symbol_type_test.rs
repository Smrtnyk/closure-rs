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
//   test/com/google/javascript/rhino/jstype/SymbolTypeTest.java.

use closure_jstype::{
    JSTypeNative,
    function_type::FunctionTypeBuilder,
    prelude::*,
    property::PropertyKey,
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
}
// port: SymbolTypeTest#symbolType_isSymbolValueType
#[test]
fn symbol_type_is_symbol_value_type() {
    let f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::SYMBOL_TYPE);
    let symbol_type = _expr1;
    let _expr2 = symbol_type.is_symbol_value_type(&f.base.reg);
    assert!(_expr2);
}

// port: SymbolTypeTest#symbolType_isNotKnownSymbolValueType
#[test]
fn symbol_type_is_not_known_symbol_value_type() {
    let f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::SYMBOL_TYPE);
    let symbol_type = _expr1;
    let _expr2 = symbol_type.is_known_symbol_value_type(&f.base.reg);
    assert!(!_expr2);
}

// port: SymbolTypeTest#symbolType_autoBoxesToSymbolObject
#[test]
fn symbol_type_auto_boxes_to_symbol_object() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::SYMBOL_TYPE);
    let symbol_type = _expr1;
    let _expr2 = symbol_type.autoboxes_to(&f.base.reg);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::SYMBOL_OBJECT_TYPE);
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, _expr3);
}

// port: SymbolTypeTest#knownSymbolType_isKnownSymbolValueType
#[test]
fn known_symbol_type_is_known_symbol_value_type() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.iterator",
    );
    let known_symbol_type = _expr1;
    let _expr2 = known_symbol_type.is_known_symbol_value_type(&f.base.reg);
    assert!(_expr2);
}

// port: SymbolTypeTest#knownSymbolType_isSubtypeOfSymbolType
#[test]
fn known_symbol_type_is_subtype_of_symbol_type() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.iterator",
    );
    let known_symbol_type = _expr1;
    let _expr2 = known_symbol_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(_expr2);
}

// port: SymbolTypeTest#knownSymbolType_reflexiveEqualityAndSubtyping
#[test]
fn known_symbol_type_reflexive_equality_and_subtyping() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.iterator",
    );
    let known_symbol_type = _expr1;
    TypeSubject::assert_type(known_symbol_type).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        known_symbol_type,
    );
    TypeSubject::assert_type(known_symbol_type).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        known_symbol_type,
    );
}

// port: SymbolTypeTest#knownSymbolType_notSubtypeOfOtherKnownSymbolTypes
#[test]
fn known_symbol_type_not_subtype_of_other_known_symbol_types() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let foo1 = _expr1;
    let _expr2 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let foo2 = _expr2;
    let _expr3 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "bar",
    );
    let bar = _expr3;
    TypeSubject::assert_type(foo1).is_not_subtype_of(&mut f.base.reg, &f.base.ast, foo2);
    TypeSubject::assert_type(foo1).is_not_subtype_of(&mut f.base.reg, &f.base.ast, bar);
    TypeSubject::assert_type(foo2).is_not_subtype_of(&mut f.base.reg, &f.base.ast, bar);
    TypeSubject::assert_type(bar).is_not_subtype_of(&mut f.base.reg, &f.base.ast, foo1);
    TypeSubject::assert_type(bar).is_not_subtype_of(&mut f.base.reg, &f.base.ast, foo2);
}

// port: SymbolTypeTest#defineSymbolProperty
#[test]
fn define_symbol_property() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let _expr2 = PropertyKey::Symbol(_expr1);
    let foo = _expr2;
    let _expr3 = f.base.array_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo.clone(),
        f.base.string_type,
        None,
    );
    let _expr4 = foo.symbol();
    TypeSubject::assert_type(f.base.array_type)
        .with_type_of_prop(&mut f.base.reg, &f.base.ast, _expr4)
        .is_string(&f.base.reg);
}

// port: SymbolTypeTest#defineSymbolProperty_structuralType
#[test]
fn define_symbol_property_structural_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let structural = _expr1;
    let _expr2 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let _expr3 = PropertyKey::Symbol(_expr2);
    let foo = _expr3;
    let _expr4 = structural.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo.clone(),
        f.base.string_type,
        None,
    );
    let _expr5 = structural.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "bar",
        f.base.number_type,
        None,
    );
    let _expr6 = structural.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "raz",
        f.base.number_type,
        None,
    );
    let _expr7 = foo.symbol();
    TypeSubject::assert_type(structural)
        .with_type_of_prop(&mut f.base.reg, &f.base.ast, _expr7)
        .is_string(&f.base.reg);
    TypeSubject::assert_type(structural).to_string_is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        "{\n  bar: number,\n  [foo]: string,\n  raz: number\n}",
    );
}

// port: SymbolTypeTest#defineSymbolProperty_overlappingSymbolAndStringKeyNames
#[test]
fn define_symbol_property_overlapping_symbol_and_string_key_names() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let structural = _expr1;
    let _expr2 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let _expr3 = PropertyKey::Symbol(_expr2);
    let foo1 = _expr3;
    let _expr4 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let _expr5 = PropertyKey::Symbol(_expr4);
    let foo2 = _expr5;
    let _expr6 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let _expr7 = PropertyKey::Symbol(_expr6);
    let foo3 = _expr7;
    let _expr8 = structural.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo1.clone(),
        f.base.string_type,
        None,
    );
    let _expr9 = structural.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo2.clone(),
        f.base.number_type,
        None,
    );
    let _expr10 = structural.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo3.clone(),
        structural,
        None,
    );
    let _expr11 = structural.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
        f.base.string_type,
        None,
    );
    TypeSubject::assert_type(structural).to_string_is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        "{\n  foo: string,\n  [foo]: number,\n  [foo]: string,\n  [foo]: {...}\n}",
    );
}

// port: SymbolTypeTest#defineSymbolProperty_templatized
#[test]
fn define_symbol_property_templatized() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.foo",
    );
    let _expr2 = PropertyKey::Symbol(_expr1);
    let foo = _expr2;
    let _expr3 = f.base.reg.get_array_element_key();
    let array_key = _expr3;
    let _expr4 = f
        .base
        .reg
        .get_native_object_type(JSTypeNative::PROMISE_TYPE);
    let promise_type = _expr4;
    let _expr5 = f
        .base
        .reg
        .create_templatized_type(&f.base.ast, promise_type, &[array_key]);
    let promise_of_array_key = _expr5;
    let _expr6 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[array_key, promise_of_array_key]);
    let foo_prop_type = _expr6;
    let _expr7 =
        f.base
            .reg
            .create_templatized_type(&f.base.ast, f.base.array_type, &[f.base.string_type]);
    let string_array = _expr7;
    let _expr8 = f.base.array_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo.clone(),
        foo_prop_type,
        None,
    );
    let _expr9 = foo.symbol();
    TypeSubject::assert_type(f.base.array_type)
        .with_type_of_prop(&mut f.base.reg, &f.base.ast, _expr9)
        .is_union_of(
            &mut f.base.reg,
            &f.base.ast,
            &[array_key, promise_of_array_key],
        );
    let _expr10 = foo.symbol();
    let subject = TypeSubject::assert_type(string_array).with_type_of_prop(
        &mut f.base.reg,
        &f.base.ast,
        _expr10,
    );
    let promise_of_string =
        f.base
            .reg
            .create_templatized_type(&f.base.ast, promise_type, &[f.base.string_type]);
    subject.is_union_of(
        &mut f.base.reg,
        &f.base.ast,
        &[f.base.string_type, promise_of_string],
    );
    let _expr11 = string_array.find_property_type(&mut f.base.reg, &f.base.ast, foo.clone());
    let _expr12 =
        f.base
            .reg
            .create_templatized_type(&f.base.ast, promise_type, &[f.base.string_type]);
    TypeSubject::assert_type(_expr11).is_union_of(
        &mut f.base.reg,
        &f.base.ast,
        &[f.base.string_type, _expr12],
    );
}

// port: SymbolTypeTest#defineSymbolProperty_symbolsWithSameNameAreStillUnique
#[test]
fn define_symbol_property_symbols_with_same_name_are_still_unique() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.foo",
    );
    let _expr2 = PropertyKey::Symbol(_expr1);
    let foo1 = _expr2;
    let _expr3 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.foo",
    );
    let _expr4 = PropertyKey::Symbol(_expr3);
    let foo2 = _expr4;
    let _expr5 = f.base.reg.get_native_type(JSTypeNative::STRING_TYPE);
    let _expr6 = f.base.array_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo1.clone(),
        _expr5,
        None,
    );
    let _expr7 = foo1.symbol();
    TypeSubject::assert_type(f.base.array_type)
        .with_type_of_prop(&mut f.base.reg, &f.base.ast, _expr7)
        .is_string(&f.base.reg);
    let _expr8 = f
        .base
        .array_type
        .has_property(&mut f.base.reg, &f.base.ast, foo2.clone());
    assert!(!_expr8);
}

// port: SymbolTypeTest#defineSymbolProperty_hasOwnPropertyVsHasProperty
#[test]
fn define_symbol_property_has_own_property_vs_has_property() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .get_native_object_type(JSTypeNative::ITERABLE_TYPE);
    let iterable = _expr1;
    let _expr2 = f
        .base
        .reg
        .get_native_object_type(JSTypeNative::ITERATOR_ITERABLE_TYPE);
    let iterable_iterator = _expr2;
    let _expr3 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
    );
    let _expr4 = PropertyKey::Symbol(_expr3);
    let foo = _expr4;
    let _expr5 = iterable.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo.clone(),
        f.base.string_type,
        None,
    );
    let _expr6 = foo.symbol();
    TypeSubject::assert_type(iterable_iterator).has_property(&mut f.base.reg, &f.base.ast, _expr6);
    let _expr7 = iterable_iterator.has_own_property(&mut f.base.reg, &f.base.ast, foo.clone());
    assert!(!_expr7);
    let _expr8 = foo.symbol();
    TypeSubject::assert_type(iterable).has_property(&mut f.base.reg, &f.base.ast, _expr8);
    let _expr9 = iterable.has_own_property(&mut f.base.reg, &f.base.ast, foo.clone());
    assert!(_expr9);
}

// port: SymbolTypeTest#symbolPropertiesDoNotConflictWithStrings
#[test]
fn symbol_properties_do_not_conflict_with_strings() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.foo",
    );
    let foo = _expr1;
    let _expr2 = PropertyKey::Symbol(foo);
    let _expr3 = f.base.array_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        _expr2.clone(),
        f.base.string_type,
        None,
    );
    let _expr4 = f
        .base
        .array_type
        .has_property(&mut f.base.reg, &f.base.ast, "foo");
    assert!(!_expr4);
}

// port: SymbolTypeTest#structuralSubtypingAccountsForSymbolProps
#[test]
fn structural_subtyping_accounts_for_symbol_props() {
    let mut f = Fixture::set_up();
    let _expr1 = FunctionTypeBuilder::new();
    let _expr2 = _expr1.for_interface();
    let _expr3 = _expr2.set_name("String");
    let _expr4 = _expr3.build_and_resolve(&mut f.base.reg, &f.base.ast);
    let string_prop_ctor = _expr4;
    string_prop_ctor.set_implicit_match(&mut f.base.reg, true);
    let _expr6 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Symbol.foo",
    );
    let _expr7 = PropertyKey::Symbol(_expr6);
    let foo = _expr7;
    let _expr8 = string_prop_ctor.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr9 = _expr8.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        foo.clone(),
        f.base.string_type,
        None,
    );
    let _expr10 = string_prop_ctor.get_instance_type(&f.base.reg);
    let string_prop = _expr10;
    let _expr11 = FunctionTypeBuilder::new();
    let _expr12 = _expr11.for_interface();
    let _expr13 = _expr12.set_name("StringOrNumber");
    let _expr14 = _expr13.build_and_resolve(&mut f.base.reg, &f.base.ast);
    let string_or_number_prop_ctor = _expr14;
    string_or_number_prop_ctor.set_implicit_match(&mut f.base.reg, true);
    let _expr16 = string_or_number_prop_ctor.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr17 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let _expr18 =
        _expr16.define_declared_property(&mut f.base.reg, &f.base.ast, foo.clone(), _expr17, None);
    let _expr19 = string_or_number_prop_ctor.get_instance_type(&f.base.reg);
    let string_or_number_prop = _expr19;
    TypeSubject::assert_type(f.base.object_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        string_prop.unwrap(),
    );
    TypeSubject::assert_type(f.base.object_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        string_or_number_prop.unwrap(),
    );
    TypeSubject::assert_type(string_or_number_prop).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        string_prop.unwrap(),
    );
    TypeSubject::assert_type(string_or_number_prop).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        string_or_number_prop.unwrap(),
    );
    TypeSubject::assert_type(string_or_number_prop).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    TypeSubject::assert_type(string_prop).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        string_or_number_prop.unwrap(),
    );
    TypeSubject::assert_type(string_prop).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        string_prop.unwrap(),
    );
    TypeSubject::assert_type(string_prop).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
}
