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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/rhino/jstype/TemplatizedTypeTest.java.

use closure_jstype::{
    TypeId,
    function_type::FunctionType,
    js_type::JSType,
    object_type::ObjectType,
    proxy_object_type::ProxyObjectType,
    testing::{base_js_type_test_case::BaseJSTypeTestCase, type_subject::TypeSubject},
};

// port: TemplatizedTypeTest#createCustomTemplatizedType
fn create_custom_templatized_type(f: &mut BaseJSTypeTestCase, raw_name: &str) -> TypeId {
    let mut closer = f.reg.get_resolver().open_for_definition();
    let t = f.reg.create_template_type(&f.ast, "T");
    let u = f.reg.create_template_type(&f.ast, "U");
    let ctor = f.reg.create_constructor_type(
        &f.ast,
        Some(raw_name.into()),
        None,
        None,
        None,
        Some(vec![t, u]),
        false,
    );
    let raw = ctor.get_instance_type(&f.reg).unwrap();
    closer.close(&mut f.reg, &f.ast);
    raw
}

// port: TemplatizedTypeTest#assertTypeCanAssignToItself
fn assert_type_can_assign_to_itself(f: &mut BaseJSTypeTestCase, type_: TypeId) {
    assert!(type_.is_subtype_of(&mut f.reg, &f.ast, type_));
}

// port: TemplatizedTypeTest#testTemplatizedType
#[test]
fn test_templatized_type() {
    let mut f = BaseJSTypeTestCase::new();
    let arr_of_string = f.create_templatized_type(f.array_type, &[f.string_type]);
    assert_type_can_assign_to_itself(&mut f, arr_of_string);
    assert!(arr_of_string.is_subtype_of(&mut f.reg, &f.ast, f.array_type));
    assert!(
        f.array_type
            .is_subtype_of(&mut f.reg, &f.ast, arr_of_string)
    );
    let arr_of_number = f.create_templatized_type(f.array_type, &[f.number_type]);
    assert_type_can_assign_to_itself(&mut f, arr_of_number);
    assert!(arr_of_number.is_subtype_of(&mut f.reg, &f.ast, f.array_type));
    assert!(
        f.array_type
            .is_subtype_of(&mut f.reg, &f.ast, arr_of_number)
    );
    let other_arr_of_string = f.create_templatized_type(f.array_type, &[f.string_type]);
    assert!(arr_of_string.equals(&mut f.reg, &f.ast, other_arr_of_string));
    assert!(!arr_of_string.equals(&mut f.reg, &f.ast, f.array_type));
    assert!(!arr_of_string.equals(&mut f.reg, &f.ast, f.array_type));
    assert!(!arr_of_string.equals(&mut f.reg, &f.ast, arr_of_number));
    assert!(!arr_of_number.equals(&mut f.reg, &f.ast, arr_of_string));
}

// port: TemplatizedTypeTest#testPrint1
#[test]
fn test_print1() {
    let mut f = BaseJSTypeTestCase::new();
    let arr_of_string = f.create_templatized_type(f.array_type, &[f.string_type]);
    assert_eq!(arr_of_string.to_string(&mut f.reg, &f.ast), "Array<string>");
}
// port: TemplatizedTypeTest#testPrint2
#[test]
fn test_print2() {
    let mut f = BaseJSTypeTestCase::new();
    let t = f.reg.create_template_type(&f.ast, "T");
    let arr_of_template_type = f.create_templatized_type(f.array_type, &[t]);
    assert_eq!(
        arr_of_template_type.to_string(&mut f.reg, &f.ast),
        "Array<T>"
    );
}
// port: TemplatizedTypeTest#testPrint3
#[test]
fn test_print3() {
    let mut f = BaseJSTypeTestCase::new();
    let arr_of_unknown = f.create_templatized_type(f.array_type, &[f.unknown_type]);
    assert_eq!(arr_of_unknown.to_string(&mut f.reg, &f.ast), "Array<?>");
}
// port: TemplatizedTypeTest#testPrintingRawType
#[test]
fn test_printing_raw_type() {
    let mut f = BaseJSTypeTestCase::new();
    let raw_type = create_custom_templatized_type(&mut f, "Foo");
    assert_eq!(raw_type.to_string(&mut f.reg, &f.ast), "Foo");
}
// port: TemplatizedTypeTest#testDifferentRawTypes
#[test]
fn test_different_raw_types() {
    let mut f = BaseJSTypeTestCase::new();
    let arr_of_number = f.create_templatized_type(f.array_type, &[f.number_type]);
    let obj_type = f.create_templatized_type(f.object_type, &[f.unknown_type]);
    assert!(arr_of_number.is_subtype_of(&mut f.reg, &f.ast, obj_type));
    assert!(!obj_type.is_subtype_of(&mut f.reg, &f.ast, arr_of_number));
}

// port: TemplatizedTypeTest#testSubtypingAndEquivalenceAmongCustomTemplatizedTypes
#[test]
fn test_subtyping_and_equivalence_among_custom_templatized_types() {
    let mut f = BaseJSTypeTestCase::new();
    let raw_type = create_custom_templatized_type(&mut f, "Baz");
    let templatized_string_number =
        f.create_templatized_type(raw_type, &[f.string_type, f.number_type]);
    let second_templatized_string_number =
        f.create_templatized_type(raw_type, &[f.string_type, f.number_type]);
    let templatized_string_all = f.create_templatized_type(raw_type, &[f.string_type, f.all_type]);
    let templatized_string_unknown =
        f.create_templatized_type(raw_type, &[f.string_type, f.unknown_type]);
    let templatized_unknown_unknown =
        f.create_templatized_type(raw_type, &[f.unknown_type, f.unknown_type]);
    TypeSubject::assert_type(templatized_string_number).is_subtype_of(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(templatized_string_all).is_subtype_of(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(templatized_string_unknown)
        .is_subtype_of(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(templatized_unknown_unknown)
        .is_subtype_of(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(templatized_string_number)
        .is_not_equal_to(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(templatized_string_all).is_not_equal_to(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(templatized_string_unknown)
        .is_not_equal_to(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(templatized_string_number).is_equal_to(
        &mut f.reg,
        &f.ast,
        second_templatized_string_number,
    );
    TypeSubject::assert_type(templatized_unknown_unknown).is_equal_to(&mut f.reg, &f.ast, raw_type);
    TypeSubject::assert_type(raw_type).is_subtype_of(&mut f.reg, &f.ast, templatized_string_number);
    TypeSubject::assert_type(raw_type).is_subtype_of(&mut f.reg, &f.ast, templatized_string_all);
    TypeSubject::assert_type(raw_type).is_subtype_of(
        &mut f.reg,
        &f.ast,
        templatized_string_unknown,
    );
    TypeSubject::assert_type(raw_type).is_subtype_of(
        &mut f.reg,
        &f.ast,
        templatized_unknown_unknown,
    );
    TypeSubject::assert_type(templatized_string_number).is_subtype_of(
        &mut f.reg,
        &f.ast,
        second_templatized_string_number,
    );
    TypeSubject::assert_type(templatized_string_number).is_not_subtype_of(
        &mut f.reg,
        &f.ast,
        templatized_string_all,
    );
    TypeSubject::assert_type(templatized_string_all).is_not_subtype_of(
        &mut f.reg,
        &f.ast,
        templatized_string_number,
    );
    TypeSubject::assert_type(templatized_string_all).is_subtype_of(
        &mut f.reg,
        &f.ast,
        templatized_string_unknown,
    );
    TypeSubject::assert_type(templatized_string_unknown).is_subtype_of(
        &mut f.reg,
        &f.ast,
        templatized_string_all,
    );
}

// port: TemplatizedTypeTest#testEqualityWithRawType_whenSpecializedOnUnknown_includingHashcode
#[test]
fn test_equality_with_raw_type_when_specialized_on_unknown_including_hashcode() {
    let mut f = BaseJSTypeTestCase::new();
    let raw_type = create_custom_templatized_type(&mut f, "Baz");
    let templatized_unknown_string =
        f.create_templatized_type(raw_type, &[f.unknown_type, f.string_type]);
    TypeSubject::assert_type(raw_type).is_not_equal_to(
        &mut f.reg,
        &f.ast,
        templatized_unknown_string,
    );
    let templatized_string_unknown =
        f.create_templatized_type(raw_type, &[f.string_type, f.unknown_type]);
    TypeSubject::assert_type(raw_type).is_not_equal_to(
        &mut f.reg,
        &f.ast,
        templatized_string_unknown,
    );
    let templatized_unknown_unknown =
        f.create_templatized_type(raw_type, &[f.unknown_type, f.unknown_type]);
    TypeSubject::assert_type(raw_type).is_equal_to(&mut f.reg, &f.ast, templatized_unknown_unknown);
    let templatized_checked_unknown_unknown =
        f.create_templatized_type(raw_type, &[f.checked_unknown_type, f.unknown_type]);
    TypeSubject::assert_type(raw_type).is_not_equal_to(
        &mut f.reg,
        &f.ast,
        templatized_checked_unknown_unknown,
    );
    let ramdom_template = f.reg.create_template_type(&f.ast, "X");
    let templatized_template_unknown =
        f.create_templatized_type(raw_type, &[ramdom_template, f.unknown_type]);
    TypeSubject::assert_type(ramdom_template).is_unknown(&mut f.reg, &f.ast);
    TypeSubject::assert_type(raw_type).is_not_equal_to(
        &mut f.reg,
        &f.ast,
        templatized_template_unknown,
    );
    let mut closer = f.reg.get_resolver().open_for_definition();
    let resolved_named_unknown = f.reg.create_named_type(&f.ast, None, "Y", "", -1, -1);
    resolved_named_unknown.set_referenced_type(&mut f.reg, f.unknown_type);
    closer.close(&mut f.reg, &f.ast);
    let templatized_resolved_named_unknown_unknown =
        f.create_templatized_type(raw_type, &[resolved_named_unknown, f.unknown_type]);
    TypeSubject::assert_type(resolved_named_unknown).is_unknown(&mut f.reg, &f.ast);
    TypeSubject::assert_type(raw_type).is_equal_to(
        &mut f.reg,
        &f.ast,
        templatized_resolved_named_unknown_unknown,
    );
    let mut closer = f.reg.get_resolver().open_for_definition();
    let unresolved_named_unknown = f.reg.create_named_type(&f.ast, None, "Y", "", -1, -1);
    unresolved_named_unknown.set_referenced_type(&mut f.reg, f.unknown_type);
    let templatized_unresolved_named_unknown_unknown =
        f.create_templatized_type(raw_type, &[unresolved_named_unknown, f.unknown_type]);
    TypeSubject::assert_type(resolved_named_unknown).is_unknown(&mut f.reg, &f.ast);
    TypeSubject::assert_type(raw_type).is_not_equal_to(
        &mut f.reg,
        &f.ast,
        templatized_unresolved_named_unknown_unknown,
    );
    closer.close(&mut f.reg, &f.ast);
}

// port: TemplatizedTypeTest#testGetPropertyTypeOnTemplatizedType
#[test]
fn test_get_property_type_on_templatized_type() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let template_t = f.reg.create_template_type(&f.ast, "T");
    let ctor = f.reg.create_constructor_type(
        &f.ast,
        Some("Foo".into()),
        None,
        None,
        None,
        Some(vec![template_t]),
        false,
    );
    let raw_type = ctor.get_instance_type(&f.reg).unwrap();
    raw_type.define_declared_property(&mut f.reg, &f.ast, "property", template_t, None);
    let templatized_number = f.create_templatized_type(raw_type, &[f.number_type]);
    let property = templatized_number.get_property_type(&mut f.reg, &f.ast, "property");
    TypeSubject::assert_type(property).is_equal_to(&mut f.reg, &f.ast, f.number_type);
    closer.close(&mut f.reg, &f.ast);
}
// port: TemplatizedTypeTest#testFindPropertyTypeOnTemplatizedType
#[test]
fn test_find_property_type_on_templatized_type() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let template_t = f.reg.create_template_type(&f.ast, "T");
    let ctor = f.reg.create_constructor_type(
        &f.ast,
        Some("Foo".into()),
        None,
        None,
        None,
        Some(vec![template_t]),
        false,
    );
    let raw_type = ctor.get_instance_type(&f.reg).unwrap();
    raw_type.define_declared_property(&mut f.reg, &f.ast, "property", template_t, None);
    let templatized_number = f.create_templatized_type(raw_type, &[f.number_type]);
    let property = templatized_number.find_property_type(&mut f.reg, &f.ast, "property");
    TypeSubject::assert_type(property).is_equal_to(&mut f.reg, &f.ast, f.number_type);
    closer.close(&mut f.reg, &f.ast);
}

// port: TemplatizedTypeTest#testGreatestSubtypeWithSubclass
#[test]
fn test_greatest_subtype_with_subclass() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let template_t = f.reg.create_template_type(&f.ast, "T");
    let base_ctor = f.reg.create_constructor_type(
        &f.ast,
        Some("Base".into()),
        None,
        None,
        None,
        Some(vec![template_t]),
        false,
    );
    let base_raw = base_ctor.get_instance_type(&f.reg).unwrap();
    let sub_ctor = f.reg.create_constructor_type(
        &f.ast,
        Some("Sub".into()),
        None,
        None,
        None,
        Some(vec![template_t]),
        false,
    );
    sub_ctor.set_prototype_based_on(&mut f.reg, &f.ast, base_raw);
    let sub_raw = sub_ctor.get_instance_type(&f.reg).unwrap();
    let base_number = f.create_templatized_type(base_raw, &[f.number_type]);
    let sub_number = f.create_templatized_type(sub_raw, &[f.number_type]);
    let greatest = base_number.get_greatest_subtype(&mut f.reg, &f.ast, sub_number);
    TypeSubject::assert_type(greatest).is_equal_to(&mut f.reg, &f.ast, sub_number);
    let greatest = sub_number.get_greatest_subtype(&mut f.reg, &f.ast, base_number);
    TypeSubject::assert_type(greatest).is_equal_to(&mut f.reg, &f.ast, sub_number);
    closer.close(&mut f.reg, &f.ast);
}
// port: TemplatizedTypeTest#testGreatestSubtypeWithIncompatibleGenerics
#[test]
fn test_greatest_subtype_with_incompatible_generics() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let template_t = f.reg.create_template_type(&f.ast, "T");
    let base_ctor = f.reg.create_constructor_type(
        &f.ast,
        Some("Base".into()),
        None,
        None,
        None,
        Some(vec![template_t]),
        false,
    );
    let base_raw = base_ctor.get_instance_type(&f.reg).unwrap();
    let sub_ctor = f.reg.create_constructor_type(
        &f.ast,
        Some("Sub".into()),
        None,
        None,
        None,
        Some(vec![template_t]),
        false,
    );
    sub_ctor.set_prototype_based_on(&mut f.reg, &f.ast, base_raw);
    let sub_raw = sub_ctor.get_instance_type(&f.reg).unwrap();
    let base_number = f.create_templatized_type(base_raw, &[f.number_type]);
    let sub_string = f.create_templatized_type(sub_raw, &[f.string_type]);
    let greatest = base_number.get_greatest_subtype(&mut f.reg, &f.ast, sub_string);
    TypeSubject::assert_type(greatest).is_equal_to(&mut f.reg, &f.ast, f.no_object_type);
    let greatest = sub_string.get_greatest_subtype(&mut f.reg, &f.ast, base_number);
    TypeSubject::assert_type(greatest).is_equal_to(&mut f.reg, &f.ast, f.no_object_type);
    closer.close(&mut f.reg, &f.ast);
}
