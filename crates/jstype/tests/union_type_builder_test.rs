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
//   test/com/google/javascript/rhino/jstype/UnionTypeBuilderTest.java.

use closure_jstype::{
    TypeId,
    function_type::FunctionTypeBuilder,
    prelude::*,
    record_type_builder::RecordTypeBuilder,
    testing::{
        base_js_type_test_case::BaseJSTypeTestCase, map_based_scope::MapBasedScope,
        type_subject::TypeSubject,
    },
    union_type::UnionTypeBuilder,
};
use closure_rhino::js_string::JsString;
use std::sync::Arc;
struct Fixture {
    base: BaseJSTypeTestCase,
    base_type: TypeId,
    sub: TypeId,
}
impl Fixture {
    // port: UnionTypeBuilderTest#setUp
    fn set_up() -> Self {
        let mut f = Self {
            base: BaseJSTypeTestCase::new(),
            base_type: TypeId(0),
            sub: TypeId(0),
        };
        {
            let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
            let _expr2 = FunctionTypeBuilder::new();
            let _expr3 = _expr2.for_constructor();
            let _expr4 = _expr3.with_name("Base");
            let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
            let base_ctor = _expr5;
            let _expr6 = base_ctor.get_instance_type(&f.base.reg);
            f.base_type = _expr6.unwrap();
            let _expr7 = FunctionTypeBuilder::new();
            let _expr8 = _expr7.for_constructor();
            let _expr9 = _expr8.with_name("Sub");
            let _expr10 = _expr9.with_prototype_based_on(f.base_type);
            let _expr11 = _expr10.build(&mut f.base.reg, &f.base.ast);
            let sub_ctor = _expr11;
            let _expr12 = sub_ctor.get_instance_type(&f.base.reg);
            f.sub = _expr12.unwrap();
            _expr1.close(&mut f.base.reg, &f.base.ast);
        }
        f
    }

    // port: UnionTypeBuilderTest#addRecordType
    fn add_record_type(&mut self, builder: &mut UnionTypeBuilder, inferred: bool) {
        let f = self;
        let mut _expr1 = RecordTypeBuilder::new();
        let mut rec_builder = _expr1;
        rec_builder.set_synthesized(inferred);
        rec_builder.add_property("prop", f.base.number_type, None);
        let _expr3 = rec_builder.build(&mut f.base.reg, &f.base.ast);
        builder.add_alternate(&mut f.base.reg, &f.base.ast, _expr3);
    }

    // port: UnionTypeBuilderTest#assertUnion
    fn assert_union(&mut self, expected: &str, types: &[TypeId]) {
        let f = self;
        let mut _expr1 = UnionTypeBuilder::new();
        let mut builder = _expr1;
        for &type_ in types {
            builder.add_alternate(&mut f.base.reg, &f.base.ast, type_);
        }
        let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
        let _expr3 = _expr2.to_string(&mut f.base.reg, &f.base.ast);
        assert_eq!(_expr3, expected);
    }

    // port: UnionTypeBuilderTest#createFunctionWithReturn
    fn create_function_with_return(&mut self, type_: TypeId) -> TypeId {
        let f = self;
        let _expr1 = FunctionTypeBuilder::new();
        let _expr2 = f.base.reg.create_parameters(&[]);
        let _expr3 = _expr1.with_parameters(_expr2);
        let _expr4 = _expr3.with_return_type(type_);

        _expr4.build(&mut f.base.reg, &f.base.ast)
    }
}
// port: UnionTypeBuilderTest#testWildcardType_allType
#[test]
fn test_wildcard_type_all_type() {
    let mut f = Fixture::set_up();
    f.assert_union("*", &[f.base.all_type]);
    f.assert_union("*", &[f.base.all_type, f.base.all_type]);
    f.assert_union("*", &[f.base.number_type, f.base.all_type]);
    f.assert_union("*", &[f.base.all_type, f.base.number_type]);
    f.assert_union("*", &[f.base.all_type, f.base.number_type, f.base.no_type]);
    f.assert_union("(*|undefined)", &[f.base.all_type, f.base.void_type]);
    f.assert_union("*", &[f.base.all_type, f.base.unknown_type]);
    f.assert_union("*", &[f.base.all_type, f.base.checked_unknown_type]);
}

// port: UnionTypeBuilderTest#testWildcardType_unknownType
#[test]
fn test_wildcard_type_unknown_type() {
    let mut f = Fixture::set_up();
    f.assert_union("?", &[f.base.unknown_type]);
    f.assert_union("?", &[f.base.unknown_type, f.base.unknown_type]);
    f.assert_union("?", &[f.base.number_type, f.base.unknown_type]);
    f.assert_union("?", &[f.base.unknown_type, f.base.number_type]);
    f.assert_union(
        "?",
        &[f.base.unknown_type, f.base.number_type, f.base.no_type],
    );
    f.assert_union("(?|undefined)", &[f.base.unknown_type, f.base.void_type]);
    f.assert_union("*", &[f.base.all_type, f.base.unknown_type]);
    f.assert_union("?", &[f.base.checked_unknown_type, f.base.unknown_type]);
}

// port: UnionTypeBuilderTest#testWildcardType_checkedUnknownType
#[test]
fn test_wildcard_type_checked_unknown_type() {
    let mut f = Fixture::set_up();
    f.assert_union("??", &[f.base.checked_unknown_type]);
    f.assert_union(
        "??",
        &[f.base.checked_unknown_type, f.base.checked_unknown_type],
    );
    f.assert_union("??", &[f.base.number_type, f.base.checked_unknown_type]);
    f.assert_union("??", &[f.base.checked_unknown_type, f.base.number_type]);
    f.assert_union(
        "??",
        &[
            f.base.checked_unknown_type,
            f.base.number_type,
            f.base.no_type,
        ],
    );
    f.assert_union(
        "(??|undefined)",
        &[f.base.checked_unknown_type, f.base.void_type],
    );
    f.assert_union("*", &[f.base.all_type, f.base.checked_unknown_type]);
    f.assert_union("?", &[f.base.unknown_type, f.base.checked_unknown_type]);
}

// port: UnionTypeBuilderTest#testEmptyUnion
#[test]
fn test_empty_union() {
    let mut f = Fixture::set_up();
    f.assert_union("None", &[]);
    f.assert_union("None", &[f.base.no_type, f.base.no_type]);
}

// port: UnionTypeBuilderTest#testUnionTypes
#[test]
fn test_union_types() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.string_type, f.base.object_type]);
    let union = _expr1;
    f.assert_union("*", &[f.base.all_type, union]);
    f.assert_union("(Object|string)", &[f.base.object_type, union]);
    f.assert_union("(Object|string)", &[union, f.base.object_type]);
    f.assert_union("(Object|number|string)", &[f.base.number_type, union]);
    f.assert_union("(Object|number|string)", &[union, f.base.number_type]);
    let _expr7 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.number_type, f.base.boolean_type]);
    f.assert_union("(Object|boolean|number|string)", &[union, _expr7]);
    let _expr9 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.number_type, f.base.boolean_type]);
    f.assert_union("(Object|boolean|number|string)", &[_expr9, union]);
    f.assert_union("(Object|string)", &[union, f.base.string_object_type]);
}

// port: UnionTypeBuilderTest#testUnknownTypes
#[test]
fn test_unknown_types() {
    let mut f = Fixture::set_up();
    f.assert_union("?", &[f.base.unknown_type]);
    f.assert_union("?", &[f.base.unknown_type, f.base.unknown_type]);
    f.assert_union("(?|undefined)", &[f.base.unknown_type, f.base.void_type]);
    f.assert_union("(?|undefined)", &[f.base.void_type, f.base.unknown_type]);
    f.assert_union(
        "(?|undefined)",
        &[f.base.void_type, f.base.number_type, f.base.unknown_type],
    );
    f.assert_union(
        "(*|undefined)",
        &[f.base.all_type, f.base.void_type, f.base.null_type],
    );
}

// port: UnionTypeBuilderTest#testUnresolvedNamedTypes
#[test]
fn test_unresolved_named_types() {
    let mut f = Fixture::set_up();
    f.base.error_reporter.lock().unwrap().expect_all_warnings(&[
        "Bad type annotation. Unknown type not.resolved.A",
        "Bad type annotation. Unknown type not.resolved.B",
    ]);
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = -1;
        let _expr3 = -1;
        let _expr4 = f.base.reg.create_named_type(
            &f.base.ast,
            Some(Arc::new(MapBasedScope::empty_scope()).clone()),
            "not.resolved.A",
            "",
            _expr2,
            _expr3,
        );
        let unresolved_name_a1 = _expr4;
        let _expr5 = -1;
        let _expr6 = -1;
        let _expr7 = f.base.reg.create_named_type(
            &f.base.ast,
            Some(Arc::new(MapBasedScope::empty_scope()).clone()),
            "not.resolved.A",
            "",
            _expr5,
            _expr6,
        );
        let unresolved_name_a2 = _expr7;
        let _expr8 = -1;
        let _expr9 = -1;
        let _expr10 = f.base.reg.create_named_type(
            &f.base.ast,
            Some(Arc::new(MapBasedScope::empty_scope()).clone()),
            "not.resolved.B",
            "",
            _expr8,
            _expr9,
        );
        let unresolved_name_b = _expr10;
        f.assert_union("?", &[f.base.unknown_type, unresolved_name_a1]);
        f.assert_union(
            "(not.resolved.A|not.resolved.A)",
            &[unresolved_name_a1, unresolved_name_a2],
        );
        f.assert_union(
            "(not.resolved.A|not.resolved.B)",
            &[unresolved_name_a1, unresolved_name_b],
        );
        f.assert_union(
            "(Object|not.resolved.A)",
            &[unresolved_name_a1, f.base.object_type],
        );
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
}

// port: UnionTypeBuilderTest#testRemovalOfDupes
#[test]
fn test_removal_of_dupes() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.string_type, f.base.object_type]);
    let string_and_object = _expr1;
    f.assert_union(
        "(Object|string)",
        &[string_and_object, f.base.string_object_type],
    );
    f.assert_union(
        "(Object|string)",
        &[f.base.string_object_type, string_and_object],
    );
}

// port: UnionTypeBuilderTest#testRemovalOfDupes2
#[test]
fn test_removal_of_dupes2() {
    let mut f = Fixture::set_up();
    let _expr1 = f.create_function_with_return(f.base_type);
    let _expr2 = f.create_function_with_return(f.sub);
    let _expr3 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.sub, _expr1, f.base_type, _expr2]);
    let union = _expr3;
    let _expr4 = union.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr4, "(Base|function(): Base)");
}

// port: UnionTypeBuilderTest#testRemovalOfDupes3
#[test]
fn test_removal_of_dupes3() {
    let mut f = Fixture::set_up();
    let _expr1 = f.create_function_with_return(f.sub);
    let _expr2 = f.create_function_with_return(f.base_type);
    let _expr3 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base_type, _expr1, f.sub, _expr2]);
    let union = _expr3;
    let _expr4 = union.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr4, "(Base|function(): Base)");
}

// port: UnionTypeBuilderTest#testRemovalOfIdenticalRecordTypes_beforeResolution
#[test]
fn test_removal_of_identical_record_types_before_resolution() {
    let mut f = Fixture::set_up();
    let record_union;
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = vec![(JsString::from("prop"), f.base.string_type)];
        let _expr3 = f.base.reg.create_record_type(&f.base.ast, _expr2);
        let type_ = _expr3;
        let _expr4 = f
            .base
            .reg
            .create_union_type(&f.base.ast, &[type_, type_, f.base.null_type]);
        record_union = _expr4;
        let _expr5 = record_union.get_alternates(&mut f.base.reg, &f.base.ast);
        assert_eq!(_expr5.len(), 2);
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
}

// port: UnionTypeBuilderTest#testRemovalOfDuplicateRecordTypes_deferredUntilResolution
#[test]
fn test_removal_of_duplicate_record_types_deferred_until_resolution() {
    let mut f = Fixture::set_up();
    let record_union;
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = vec![(JsString::from("prop"), f.base.string_type)];
        let _expr3 = f.base.reg.create_record_type(&f.base.ast, _expr2);
        let type_a = _expr3;
        let _expr4 = vec![(JsString::from("prop"), f.base.string_type)];
        let _expr5 = f.base.reg.create_record_type(&f.base.ast, _expr4);
        let type_b = _expr5;
        let _expr6 = f.base.reg.create_union_type(&f.base.ast, &[type_a, type_b]);
        record_union = _expr6;
        let _expr7 = record_union.get_alternates(&mut f.base.reg, &f.base.ast);
        assert_eq!(_expr7.len(), 2);
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
    let _expr8 = record_union.get_alternates(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr8.len(), 1);
}

// port: UnionTypeBuilderTest#testRemovalOfDuplicateRecordTypes1
#[test]
fn test_removal_of_duplicate_record_types1() {
    let mut f = Fixture::set_up();
    let mut _expr1 = UnionTypeBuilder::new();
    let mut builder = _expr1;
    f.add_record_type(&mut builder, false);
    f.add_record_type(&mut builder, false);
    let _expr4 = builder.build(&mut f.base.reg, &f.base.ast);
    let _expr5 = _expr4.to_maybe_union_type(&f.base.reg);
    assert!(_expr5.is_none());
}

// port: UnionTypeBuilderTest#testRemovalOfDuplicateRecordTypes2
#[test]
fn test_removal_of_duplicate_record_types2() {
    let mut f = Fixture::set_up();
    let mut _expr1 = UnionTypeBuilder::new();
    let mut builder = _expr1;
    f.add_record_type(&mut builder, true);
    f.add_record_type(&mut builder, true);
    let _expr4 = builder.build(&mut f.base.reg, &f.base.ast);
    let _expr5 = _expr4.to_maybe_union_type(&f.base.reg);
    assert!(_expr5.is_none());
}

// port: UnionTypeBuilderTest#testDifferentTemplateSpecializations_whenUnioned_doNotLeakRawType
#[test]
fn test_different_template_specializations_when_unioned_do_not_leak_raw_type() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .reg
            .create_templatized_type(&f.base.ast, f.base.array_type, &[f.base.string_type]);
    let array_of_string = _expr1;
    let _expr2 =
        f.base
            .reg
            .create_templatized_type(&f.base.ast, f.base.array_type, &[f.base.number_type]);
    let array_of_number = _expr2;
    let _expr3 =
        f.base
            .reg
            .create_templatized_type(&f.base.ast, f.base.array_type, &[f.base.unknown_type]);
    let array_of_unknown = _expr3;
    let mut _expr4 = UnionTypeBuilder::new();
    _expr4.add_alternate(&mut f.base.reg, &f.base.ast, array_of_string);
    _expr4.add_alternate(&mut f.base.reg, &f.base.ast, array_of_number);
    let mut builder = _expr4;
    let _expr5 = builder.build(&mut f.base.reg, &f.base.ast);
    let result = _expr5;
    let _expr6 = result.is_raw_type_of_templatized_type(&f.base.reg);
    assert!(!_expr6);
    TypeSubject::assert_type(result).is_equal_to(&mut f.base.reg, &f.base.ast, array_of_unknown);
}

// port: UnionTypeBuilderTest#testAfterBuild_cannotRebuild
#[test]
fn test_after_build_cannot_rebuild() {
    let mut f = Fixture::set_up();
    let mut builder = UnionTypeBuilder::new();
    builder.build(&mut f.base.reg, &f.base.ast);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || builder.build(&mut f.base.reg, &f.base.ast)
        ))
        .is_err()
    );
}
// port: UnionTypeBuilderTest#testAfterBuild_cannotAdd
#[test]
fn test_after_build_cannot_add() {
    let mut f = Fixture::set_up();
    let mut builder = UnionTypeBuilder::new();
    builder.build(&mut f.base.reg, &f.base.ast);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            builder.add_alternate(&mut f.base.reg, &f.base.ast, f.base.number_type);
        }))
        .is_err()
    );
}
