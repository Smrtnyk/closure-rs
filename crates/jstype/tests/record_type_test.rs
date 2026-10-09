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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/rhino/jstype/RecordTypeTest.java.

use closure_jstype::{
    function_type::FunctionTypeBuilder,
    js_type::Nullability,
    prelude::*,
    record_type_builder::RecordTypeBuilder,
    testing::{
        asserts::Asserts, base_js_type_test_case::BaseJSTypeTestCase, type_subject::TypeSubject,
    },
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
// port: RecordTypeTest#testRecursiveRecord
#[test]
fn test_recursive_record() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::proxy_object_type::create(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
        None,
    );
    let loop_ = _expr1;
    let mut _expr2 = RecordTypeBuilder::new();
    _expr2.add_property("loop", loop_, None);
    _expr2.add_property("number", f.base.number_type, None);
    _expr2.add_property("string", f.base.string_type, None);
    let _expr3 = _expr2.build(&mut f.base.reg, &f.base.ast);
    let record = _expr3;
    let _expr4 = record.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(
        _expr4,
        "{\n  loop: number,\n  number: number,\n  string: string\n}"
    );
    loop_.set_referenced_type(&mut f.base.reg, record);
    let _expr6 = record.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(
        _expr6,
        "{\n  loop: {...},\n  number: number,\n  string: string\n}"
    );
    let _expr7 = record.to_annotation_string(&mut f.base.reg, &f.base.ast, Nullability::EXPLICIT);
    assert_eq!(_expr7, "{loop: ?, number: number, string: string}");
    Asserts::assert_equivalence_operations(&mut f.base.reg, &f.base.ast, record, loop_);
}

// port: RecordTypeTest#testToString_largeRecord
#[test]
fn test_to_string_large_record() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    _expr1.add_property("a01", f.base.number_type, None);
    _expr1.add_property("a02", f.base.number_type, None);
    _expr1.add_property("a03", f.base.number_type, None);
    _expr1.add_property("a04", f.base.number_type, None);
    _expr1.add_property("a05", f.base.number_type, None);
    _expr1.add_property("a06", f.base.number_type, None);
    _expr1.add_property("a07", f.base.number_type, None);
    _expr1.add_property("a08", f.base.number_type, None);
    _expr1.add_property("a09", f.base.number_type, None);
    _expr1.add_property("a10", f.base.number_type, None);
    _expr1.add_property("a11", f.base.number_type, None);
    let _expr2 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record = _expr2;
    let _expr3 = record.to_string(&mut f.base.reg, &f.base.ast);
    let _expr4 = BaseJSTypeTestCase::lines(&[
        "{",
        "  a01: number,",
        "  a02: number,",
        "  a03: number,",
        "  a04: number,",
        "  a05: number,",
        "  a06: number,",
        "  a07: number,",
        "  a08: number,",
        "  a09: number,",
        "  a10: number,",
        "  ...",
        "}",
    ]);
    assert_eq!(_expr3, _expr4);
    let _expr5 = record.to_annotation_string(&mut f.base.reg, &f.base.ast, Nullability::EXPLICIT);
    let _expr6 = format!(
        "{}{}",
        "{a01: number, a02: number, a03: number, a04: number, a05: number, a06: number,",
        " a07: number, a08: number, a09: number, a10: number, a11: number}"
    );
    assert_eq!(_expr5, _expr6);
}

// port: RecordTypeTest#testToString_nestedRecordIndentation
#[test]
fn test_to_string_nested_record_indentation() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    let mut _expr2 = RecordTypeBuilder::new();
    let mut _expr3 = RecordTypeBuilder::new();
    _expr3.add_property("aaa", f.base.number_type, None);
    _expr3.add_property("aab", f.base.number_type, None);
    _expr3.add_property("aac", f.base.number_type, None);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    _expr2.add_property("aa", _expr4, None);
    _expr2.add_property("ab", f.base.number_type, None);
    let _expr5 = _expr2.build(&mut f.base.reg, &f.base.ast);
    _expr1.add_property("a", _expr5, None);
    _expr1.add_property("b", f.base.number_type, None);
    let _expr6 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record = _expr6;
    let _expr7 = record.to_string(&mut f.base.reg, &f.base.ast);
    let _expr8 = BaseJSTypeTestCase::lines(&[
        "{",
        "  a: {",
        "    aa: {",
        "      aaa: number,",
        "      aab: number,",
        "      aac: number",
        "    },",
        "    ab: number",
        "  },",
        "  b: number",
        "}",
    ]);
    assert_eq!(_expr7, _expr8);
    let _expr9 = record.to_annotation_string(&mut f.base.reg, &f.base.ast, Nullability::EXPLICIT);
    assert_eq!(
        _expr9,
        "{a: {aa: {aaa: number, aab: number, aac: number}, ab: number}, b: number}"
    );
}

// port: RecordTypeTest#testToString_nestedRecordIndentation_throughUnion
#[test]
fn test_to_string_nested_record_indentation_through_union() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    _expr1.add_property("a", f.base.number_type, None);
    let mut _expr2 = RecordTypeBuilder::new();
    _expr2.add_property("ba", f.base.number_type, None);
    _expr2.add_property("bb", f.base.number_type, None);
    let _expr3 = _expr2.build(&mut f.base.reg, &f.base.ast);
    let _expr4 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.number_type, _expr3]);
    _expr1.add_property("b", _expr4, None);
    let _expr5 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record = _expr5;
    let _expr6 = record.to_string(&mut f.base.reg, &f.base.ast);
    let _expr7 = BaseJSTypeTestCase::lines(&[
        "{",
        "  a: number,",
        "  b: (number|{",
        "    ba: number,",
        "    bb: number",
        "  })",
        "}",
    ]);
    assert_eq!(_expr6, _expr7);
    let _expr8 = record.to_annotation_string(&mut f.base.reg, &f.base.ast, Nullability::EXPLICIT);
    assert_eq!(_expr8, "{a: number, b: (number|{ba: number, bb: number})}");
}

// port: RecordTypeTest#testToString_nestedRecordIndentation_throughTemplateParam
#[test]
fn test_to_string_nested_record_indentation_through_template_param() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    _expr1.add_property("a", f.base.number_type, None);
    let mut _expr2 = RecordTypeBuilder::new();
    _expr2.add_property("ba", f.base.number_type, None);
    _expr2.add_property("bb", f.base.number_type, None);
    let _expr3 = _expr2.build(&mut f.base.reg, &f.base.ast);
    let _expr4 = vec![_expr3];
    let _expr5 = f
        .base
        .reg
        .create_templatized_type(&f.base.ast, f.base.array_type, &_expr4);
    _expr1.add_property("b", _expr5, None);
    let _expr6 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record = _expr6;
    let _expr7 = record.to_string(&mut f.base.reg, &f.base.ast);
    let _expr8 = BaseJSTypeTestCase::lines(&[
        "{",
        "  a: number,",
        "  b: Array<{",
        "    ba: number,",
        "    bb: number",
        "  }>",
        "}",
    ]);
    assert_eq!(_expr7, _expr8);
    let _expr9 = record.to_annotation_string(&mut f.base.reg, &f.base.ast, Nullability::EXPLICIT);
    assert_eq!(_expr9, "{a: number, b: !Array<{ba: number, bb: number}>}");
}

// port: RecordTypeTest#testSupAndInf
#[test]
fn test_sup_and_inf() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    _expr1.add_property("a", f.base.number_type, None);
    _expr1.add_property("b", f.base.number_type, None);
    let _expr2 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record_a = _expr2;
    let mut _expr3 = RecordTypeBuilder::new();
    _expr3.add_property("b", f.base.number_type, None);
    _expr3.add_property("c", f.base.number_type, None);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let record_c = _expr4;
    let _expr5 =
        closure_jstype::proxy_object_type::create(&mut f.base.reg, &f.base.ast, record_a, None);
    let proxy_record_a = _expr5;
    let _expr6 =
        closure_jstype::proxy_object_type::create(&mut f.base.reg, &f.base.ast, record_c, None);
    let proxy_record_c = _expr6;
    let mut _expr7 = RecordTypeBuilder::new();
    _expr7.add_property("a", f.base.number_type, None);
    _expr7.add_property("b", f.base.number_type, None);
    _expr7.add_property("c", f.base.number_type, None);
    let _expr8 = _expr7.build(&mut f.base.reg, &f.base.ast);
    let a_inf_c = _expr8;
    let _expr9 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[record_a, record_c]);
    let a_sup_c = _expr9;
    let _expr10 = record_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_c);
    TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, a_inf_c);
    let _expr11 = record_a.get_least_supertype(&mut f.base.reg, &f.base.ast, record_c);
    TypeSubject::assert_type(_expr11).is_equal_to(&mut f.base.reg, &f.base.ast, a_sup_c);
    let _expr12 = proxy_record_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, proxy_record_c);
    TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, a_inf_c);
    let _expr13 = proxy_record_a.get_least_supertype(&mut f.base.reg, &f.base.ast, proxy_record_c);
    TypeSubject::assert_type(_expr13).is_equal_to(&mut f.base.reg, &f.base.ast, a_sup_c);
}

// port: RecordTypeTest#testSubtypeWithUnknowns
#[test]
fn test_subtype_with_unknowns() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    _expr1.add_property("a", f.base.number_type, None);
    let _expr2 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record_a = _expr2;
    let mut _expr3 = RecordTypeBuilder::new();
    _expr3.add_property("a", f.base.unknown_type, None);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let record_b = _expr4;
    let _expr5 = record_a.is_subtype_of(&mut f.base.reg, &f.base.ast, record_b);
    assert!(_expr5);
    let _expr6 = record_b.is_subtype_of(&mut f.base.reg, &f.base.ast, record_a);
    assert!(_expr6);
}

// port: RecordTypeTest#testSubtypeWithUnknowns2
#[test]
fn test_subtype_with_unknowns2() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    let _expr2 = FunctionTypeBuilder::new();
    let _expr3 = _expr2.with_return_type(f.base.number_type);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    _expr1.add_property("a", _expr4, None);
    let _expr5 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record_a = _expr5;
    let mut _expr6 = RecordTypeBuilder::new();
    let _expr7 = FunctionTypeBuilder::new();
    let _expr8 = _expr7.with_return_type(f.base.unknown_type);
    let _expr9 = _expr8.build(&mut f.base.reg, &f.base.ast);
    _expr6.add_property("a", _expr9, None);
    let _expr10 = _expr6.build(&mut f.base.reg, &f.base.ast);
    let record_b = _expr10;
    let _expr11 = record_a.is_subtype_of(&mut f.base.reg, &f.base.ast, record_b);
    assert!(_expr11);
    let _expr12 = record_b.is_subtype_of(&mut f.base.reg, &f.base.ast, record_a);
    assert!(_expr12);
}

// port: RecordTypeTest#testSubtypeWithFunctionProps
#[test]
fn test_subtype_with_function_props() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    let _expr2 = FunctionTypeBuilder::new();
    let _expr3 = _expr2.with_return_type(f.base.number_type);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    _expr1.add_property("a", _expr4, None);
    let _expr5 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record_a = _expr5;
    let mut _expr6 = RecordTypeBuilder::new();
    let _expr7 = FunctionTypeBuilder::new();
    let _expr8 = _expr7.with_return_type(f.base.string_type);
    let _expr9 = _expr8.build(&mut f.base.reg, &f.base.ast);
    _expr6.add_property("a", _expr9, None);
    let _expr10 = _expr6.build(&mut f.base.reg, &f.base.ast);
    let record_b = _expr10;
    let _expr11 = record_a.is_subtype_of(&mut f.base.reg, &f.base.ast, record_b);
    assert!(!_expr11);
    let _expr12 = record_b.is_subtype_of(&mut f.base.reg, &f.base.ast, record_a);
    assert!(!_expr12);
}

// port: RecordTypeTest#testSubtypeWithManyProps
#[test]
fn test_subtype_with_many_props() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    _expr1.add_property("a", f.base.number_type, None);
    _expr1.add_property("b", f.base.number_type, None);
    let _expr2 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record_a = _expr2;
    let mut _expr3 = RecordTypeBuilder::new();
    _expr3.add_property("a", f.base.number_type, None);
    _expr3.add_property("b", f.base.string_type, None);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let record_b = _expr4;
    let mut _expr5 = RecordTypeBuilder::new();
    _expr5.add_property("a", f.base.number_type, None);
    let _expr6 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.number_type, f.base.string_type]);
    _expr5.add_property("b", _expr6, None);
    let _expr7 = _expr5.build(&mut f.base.reg, &f.base.ast);
    let record_c = _expr7;
    let _expr8 = record_a.is_subtype_of(&mut f.base.reg, &f.base.ast, record_b);
    assert!(!_expr8);
    let _expr9 = record_b.is_subtype_of(&mut f.base.reg, &f.base.ast, record_a);
    assert!(!_expr9);
    let _expr10 = record_c.is_subtype_of(&mut f.base.reg, &f.base.ast, record_b);
    assert!(!_expr10);
    let _expr11 = record_b.is_subtype_of(&mut f.base.reg, &f.base.ast, record_c);
    assert!(_expr11);
    let _expr12 = record_a.is_subtype_of(&mut f.base.reg, &f.base.ast, record_c);
    assert!(_expr12);
}

// port: RecordTypeTest#testEquality_onlyCompares_ownProperties
#[test]
fn test_equality_only_compares_own_properties() {
    let mut f = Fixture::set_up();
    let mut _expr1 = RecordTypeBuilder::new();
    _expr1.add_property("a", f.base.number_type, None);
    let _expr2 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let record_a = _expr2;
    let mut _expr3 = RecordTypeBuilder::new();
    _expr3.add_property("a", f.base.number_type, None);
    let _expr4 = f
        .base
        .object_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    _expr3.add_property("toString", _expr4, None);
    let _expr5 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let record_b = _expr5;
    let _expr6 = record_a.get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    let _expr7 = record_b.get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, _expr7);
    TypeSubject::assert_type(record_a).is_not_equal_to(&mut f.base.reg, &f.base.ast, record_b);
}
