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
//   test/com/google/javascript/rhino/jstype/NamedTypeTest.java.

use closure_jstype::{
    TypeId,
    function_type::{FunctionType, FunctionTypeBuilder},
    js_type::JSType,
    named_type::{NamedType, NamedTypeBuilder, ResolutionKind},
    no_resolved_type::NoResolvedType,
    object_type::ObjectType,
    testing::{
        base_js_type_test_case::BaseJSTypeTestCase, map_based_scope::MapBasedScope,
        type_subject::TypeSubject,
    },
};
use closure_rhino::error_reporter::NullErrorReporter;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

// port: NamedTypeTest#setUp
fn set_up() -> (BaseJSTypeTestCase, TypeId, TypeId) {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let foo_ctor_type = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Foo")
        .build(&mut f.reg, &f.ast);
    let foo_type = foo_ctor_type.get_instance_type(&f.reg).unwrap();
    closer.close(&mut f.reg, &f.ast);
    (f, foo_ctor_type, foo_type)
}
// port: NamedTypeTest#namedTypeBuilder
fn named_type_builder(f: &BaseJSTypeTestCase, name: &str) -> NamedTypeBuilder {
    NamedTypeBuilder::new(&f.reg, name)
        .set_resolution_kind(ResolutionKind::TYPE_NAME)
        .set_scope(Some(Arc::new(MapBasedScope::empty_scope())))
        .set_error_reporting_location("source", 1, 0)
}
// port: NamedTypeTest#createNominalType
fn create_nominal_type(f: &mut BaseJSTypeTestCase, name: &str) -> TypeId {
    let mut closer = f.reg.get_resolver().open_for_definition();
    let ctor_type = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name(name)
        .build(&mut f.reg, &f.ast);
    let type_ = ctor_type.get_instance_type(&f.reg).unwrap();
    closer.close(&mut f.reg, &f.ast);
    type_
}
// port: NamedTypeTest#forceResolutionWith
fn force_resolution_with(
    f: &mut BaseJSTypeTestCase,
    type_: TypeId,
    proxy: NamedTypeBuilder,
) -> TypeId {
    proxy
        .set_referenced_type(type_)
        .set_resolution_kind(ResolutionKind::NONE)
        .build(&mut f.reg, &f.ast)
}
fn assert_equality_groups(f: &mut BaseJSTypeTestCase, groups: &[Vec<TypeId>]) {
    for (i, group) in groups.iter().enumerate() {
        for &a in group {
            TypeSubject::assert_type(a).is_not_equal_to(&mut f.reg, &f.ast, None);
            for (j, other) in groups.iter().enumerate() {
                for &b in other {
                    assert_eq!(a.equals(&mut f.reg, &f.ast, b), i == j);
                    assert_eq!(b.equals(&mut f.reg, &f.ast, a), i == j);
                    if i == j {
                        assert_eq!(a.hash_code(&f.reg), b.hash_code(&f.reg));
                    }
                }
            }
        }
    }
}

// port: NamedTypeTest#testResolutionPropagatesNamedTypePropertiesToResolvedType
#[test]
fn test_resolution_propagates_named_type_properties_to_resolved_type() {
    let (mut f, foo_ctor_type, foo_type) = set_up();
    let scope = Arc::new(MapBasedScope::new(vec![("Foo".into(), foo_ctor_type)]));
    let mut closer = f.reg.get_resolver().open_for_definition();
    let named_foo_type = named_type_builder(&f, "Foo")
        .set_scope(Some(scope))
        .build(&mut f.reg, &f.ast);
    named_foo_type.define_declared_property(&mut f.reg, &f.ast, "myProperty", f.number_type, None);
    let type_ = foo_type.get_property_type(&mut f.reg, &f.ast, "myProperty");
    f.assert_type_not_equals(f.number_type, type_);
    closer.close(&mut f.reg, &f.ast);
    let type_ = foo_type.get_property_type(&mut f.reg, &f.ast, "myProperty");
    f.assert_type_equals(f.number_type, type_);
}
// port: NamedTypeTest#testStateOfForwardDeclaredType_Unresolved
#[test]
fn test_state_of_forward_declared_type_unresolved() {
    let (mut f, _, _) = set_up();
    f.reset_registry_with_forward_declared_name("forwardDeclared");
    let mut closer = f.reg.get_resolver().open_for_definition();
    let type_ = named_type_builder(&f, "forwardDeclared").build(&mut f.reg, &f.ast);
    assert!(!type_.is_resolved(&f.reg));
    assert!(!type_.is_empty_type(&f.reg));
    f.assert_type_not_equals(f.unknown_type, type_);
    f.assert_type_equals(
        f.unknown_type,
        NamedType::get_referenced_type(type_, &f.reg),
    );
    closer.close(&mut f.reg, &f.ast);
}
// port: NamedTypeTest#testStateOfForwardDeclaredType_UnsuccesfullyResolved
#[test]
fn test_state_of_forward_declared_type_unsuccesfully_resolved() {
    let (mut f, _, _) = set_up();
    f.reset_registry_with_forward_declared_name("forwardDeclared");
    let type_ = named_type_builder(&f, "forwardDeclared").build(&mut f.reg, &f.ast);
    assert!(type_.is_unsuccessfully_resolved(&f.reg));
    assert!(!type_.is_unknown_type(&mut f.reg, &f.ast));
}
// port: NamedTypeTest#testStateOfForwardDeclaredType_SuccessfullyResolved
#[test]
fn test_state_of_forward_declared_type_successfully_resolved() {
    let (mut f, foo_ctor_type, foo_type) = set_up();
    let named_foo_type = named_type_builder(&f, "Foo")
        .set_scope(Some(Arc::new(MapBasedScope::new(vec![(
            "Foo".into(),
            foo_ctor_type,
        )]))))
        .build(&mut f.reg, &f.ast);
    named_foo_type.resolve_with_reporter(&mut f.reg, &f.ast, &mut NullErrorReporter);
    assert!(named_foo_type.is_successfully_resolved(&f.reg));
    f.assert_type_equals(named_foo_type, foo_type);
}
// port: NamedTypeTest#testEquality
#[test]
fn test_equality() {
    let (mut f, _, foo_type) = set_up();
    let bar_type = create_nominal_type(&mut f, "Bar");
    let anon_type = FunctionTypeBuilder::new().build(&mut f.reg, &f.ast);
    let mut closer = f.reg.get_resolver().open_for_definition();
    let foo_builder = named_type_builder(&f, "Foo");
    let no_foo = NoResolvedType::new(&mut f.reg, &f.ast, "Foo", None);
    let foo_unresolved = force_resolution_with(&mut f, no_foo, foo_builder.clone());
    let foo_foo = force_resolution_with(&mut f, foo_type, foo_builder.clone());
    let foo_anon = force_resolution_with(&mut f, anon_type, foo_builder.clone());
    let foo_bar = force_resolution_with(&mut f, bar_type, foo_builder);
    let bar_builder = named_type_builder(&f, "Bar");
    let no_bar = NoResolvedType::new(&mut f.reg, &f.ast, "Bar", None);
    let bar_unresolved = force_resolution_with(&mut f, no_bar, bar_builder.clone());
    let bar_foo = force_resolution_with(&mut f, foo_type, bar_builder.clone());
    let bar_anon = force_resolution_with(&mut f, anon_type, bar_builder.clone());
    let bar_bar = force_resolution_with(&mut f, bar_type, bar_builder);
    closer.close(&mut f.reg, &f.ast);
    assert_equality_groups(
        &mut f,
        &[
            vec![foo_type, foo_foo, bar_foo],
            vec![bar_type, foo_bar, bar_bar],
            vec![anon_type, foo_anon, bar_anon],
            vec![foo_unresolved],
            vec![bar_unresolved],
        ],
    );
}
// port: NamedTypeTest#testEqualityOfTypesWithSameReferenceName_postResolution
#[test]
fn test_equality_of_types_with_same_reference_name_post_resolution() {
    let (mut f, _, foo_type) = set_up();
    let bar_type_a = create_nominal_type(&mut f, "Bar");
    let bar_type_b = create_nominal_type(&mut f, "Bar");
    let anon_type = FunctionTypeBuilder::new().build(&mut f.reg, &f.ast);
    let mut closer = f.reg.get_resolver().open_for_definition();
    let foo_builder = NamedTypeBuilder::new(&f.reg, "Foo");
    let no_foo = NoResolvedType::new(&mut f.reg, &f.ast, "Foo", None);
    let foo_unresolved = force_resolution_with(&mut f, no_foo, foo_builder.clone());
    let foo_foo = force_resolution_with(&mut f, foo_type, foo_builder.clone());
    let foo_anon = force_resolution_with(&mut f, anon_type, foo_builder.clone());
    let foo_bar_a = force_resolution_with(&mut f, bar_type_a, foo_builder.clone());
    let foo_bar_b = force_resolution_with(&mut f, bar_type_b, foo_builder.clone());
    let number = f.number_type;
    let string = f.string_type;
    let foo_number = force_resolution_with(&mut f, number, foo_builder.clone());
    let foo_string = force_resolution_with(&mut f, string, foo_builder);
    let bar_builder = NamedTypeBuilder::new(&f.reg, "Bar");
    let no_bar = NoResolvedType::new(&mut f.reg, &f.ast, "Bar", None);
    let bar_unresolved = force_resolution_with(&mut f, no_bar, bar_builder.clone());
    let bar_foo = force_resolution_with(&mut f, foo_type, bar_builder.clone());
    let bar_anon = force_resolution_with(&mut f, anon_type, bar_builder.clone());
    let bar_bar_a = force_resolution_with(&mut f, bar_type_a, bar_builder.clone());
    let bar_bar_b = force_resolution_with(&mut f, bar_type_b, bar_builder.clone());
    let bar_number = force_resolution_with(&mut f, number, bar_builder.clone());
    let bar_string = force_resolution_with(&mut f, string, bar_builder);
    closer.close(&mut f.reg, &f.ast);
    assert_equality_groups(
        &mut f,
        &[
            vec![foo_type, foo_foo, bar_foo],
            vec![bar_type_a, foo_bar_a, bar_bar_a],
            vec![bar_type_b, foo_bar_b, bar_bar_b],
            vec![anon_type, foo_anon, bar_anon],
            vec![number, foo_number, bar_number],
            vec![string, foo_string, bar_string],
            vec![foo_unresolved],
            vec![bar_unresolved],
        ],
    );
}
// port: NamedTypeTest#testForwardDeclaredNamedType
#[test]
fn test_forward_declared_named_type() {
    let (mut f, _, _) = set_up();
    f.error_reporter
        .lock()
        .unwrap()
        .expect_all_warnings(&["Bad type annotation. Unknown type Unresolvable"]);
    let a = named_type_builder(&f, "Unresolvable").build(&mut f.reg, &f.ast);
    let value = a.get_least_supertype(&mut f.reg, &f.ast, f.unknown_type);
    f.assert_type_equals(f.unknown_type, value);
    let value = a.get_least_supertype(&mut f.reg, &f.ast, f.checked_unknown_type);
    f.assert_type_equals(f.checked_unknown_type, value);
    let value = f.unknown_type.get_least_supertype(&mut f.reg, &f.ast, a);
    f.assert_type_equals(f.unknown_type, value);
    let value = f
        .checked_unknown_type
        .get_least_supertype(&mut f.reg, &f.ast, a);
    f.assert_type_equals(f.checked_unknown_type, value);
}
// port: NamedTypeTest#testActiveXObjectResolve
#[test]
fn test_active_x_object_resolve() {
    let (mut f, _, _) = set_up();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let scope = Arc::new(MapBasedScope::new(vec![(
        "ActiveXObject".into(),
        f.no_object_type,
    )]));
    let active_xobject = named_type_builder(&f, "ActiveXObject")
        .set_scope(Some(scope))
        .build(&mut f.reg, &f.ast);
    assert_eq!(
        active_xobject.to_string(&mut f.reg, &f.ast),
        "ActiveXObject"
    );
    closer.close(&mut f.reg, &f.ast);
    assert_eq!(active_xobject.to_string(&mut f.reg, &f.ast), "NoObject");
    f.assert_type_equals(
        f.no_object_type,
        NamedType::get_referenced_type(active_xobject, &f.reg),
    );
}
// port: NamedTypeTest#testResolveToGoogModule
#[test]
fn test_resolve_to_goog_module() {
    let (mut f, foo_ctor_type, foo_type) = set_up();
    f.reg
        .register_non_legacy_closure_namespace("mod.Foo", None, foo_ctor_type);
    let mod_dot_foo = named_type_builder(&f, "mod.Foo").build(&mut f.reg, &f.ast);
    mod_dot_foo.resolve_with_reporter(&mut f.reg, &f.ast, &mut NullErrorReporter);
    f.assert_type_equals(
        foo_type,
        NamedType::get_referenced_type(mod_dot_foo, &f.reg),
    );
}
// port: NamedTypeTest#testResolveToPropertyOnGoogModule
#[test]
fn test_resolve_to_property_on_goog_module() {
    let (mut f, foo_ctor_type, foo_type) = set_up();
    let exports = f
        .reg
        .create_record_type(&f.ast, vec![("Foo".into(), foo_ctor_type)]);
    f.reg
        .register_non_legacy_closure_namespace("mod.Bar", None, exports);
    let mod_bar_foo = named_type_builder(&f, "mod.Bar.Foo").build(&mut f.reg, &f.ast);
    mod_bar_foo.resolve_with_reporter(&mut f.reg, &f.ast, &mut NullErrorReporter);
    f.assert_type_equals(
        foo_type,
        NamedType::get_referenced_type(mod_bar_foo, &f.reg),
    );
}
// port: NamedTypeTest#testResolveToGoogModule_failsIfLegacyNamespaceIsLongerPrefix
#[test]
fn test_resolve_to_goog_module_fails_if_legacy_namespace_is_longer_prefix() {
    let (mut f, foo_ctor_type, _) = set_up();
    f.error_reporter
        .lock()
        .unwrap()
        .expect_all_warnings(&["Bad type annotation. Unknown type mod.Bar.Foo"]);
    let mut closer = f.reg.get_resolver().open_for_definition();
    let exports = f
        .reg
        .create_record_type(&f.ast, vec![("Foo".into(), foo_ctor_type)]);
    f.reg
        .register_non_legacy_closure_namespace("mod.Bar", None, exports);
    let mod_dot_foo = named_type_builder(&f, "mod.Bar.Foo").build(&mut f.reg, &f.ast);
    f.reg.register_legacy_closure_namespace("mod.Bar.Foo");
    closer.close(&mut f.reg, &f.ast);
    TypeSubject::assert_type(NamedType::get_referenced_type(mod_dot_foo, &f.reg))
        .is_unknown(&mut f.reg, &f.ast);
}
// port: NamedTypeTest#testResolveToGoogModule_usesLongestModulePrefix
#[test]
fn test_resolve_to_goog_module_uses_longest_module_prefix() {
    let (mut f, foo_ctor_type, _) = set_up();
    let bar_type = create_nominal_type(&mut f, "Bar");
    let exports = f
        .reg
        .create_record_type(&f.ast, vec![("Bar".into(), foo_ctor_type)]);
    f.reg
        .register_non_legacy_closure_namespace("mod.Foo", None, exports);
    f.reg.register_non_legacy_closure_namespace(
        "mod.Foo.Bar",
        None,
        bar_type.get_constructor(&f.reg).unwrap(),
    );
    let mod_dot_foo = named_type_builder(&f, "mod.Foo.Bar")
        .set_scope(Some(Arc::new(MapBasedScope::empty_scope())))
        .build(&mut f.reg, &f.ast);
    mod_dot_foo.resolve_with_reporter(&mut f.reg, &f.ast, &mut NullErrorReporter);
    f.assert_type_equals(
        bar_type,
        NamedType::get_referenced_type(mod_dot_foo, &f.reg),
    );
}
// port: NamedTypeTest#testReferenceGoogModuleByType_resolvesToModuleRatherThanRegistryType
#[test]
fn test_reference_goog_module_by_type_resolves_to_module_rather_than_registry_type() {
    let (mut f, foo_ctor_type, foo_type) = set_up();
    f.reg
        .register_non_legacy_closure_namespace("mod.Foo", None, foo_ctor_type);
    let scope = Arc::new(MapBasedScope::empty_scope());
    let mod_dot_foo = named_type_builder(&f, "mod.Foo")
        .set_scope(Some(scope.clone()))
        .build(&mut f.reg, &f.ast);
    let other = create_nominal_type(&mut f, "other.mod.Foo");
    f.reg
        .declare_type(&f.ast, Some(scope.as_ref()), "mod.Foo", other);
    f.assert_type_equals(
        foo_type,
        NamedType::get_referenced_type(mod_dot_foo, &f.reg),
    );
}
// port: NamedTypeTest#testGetBangType_onUnresolvedType
#[test]
fn test_get_bang_type_on_unresolved_type() {
    let (mut f, _, foo_type) = set_up();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let bar_type = named_type_builder(&f, "Bar").build(&mut f.reg, &f.ast);
    let bar_non_null = bar_type.get_bang_type(&mut f.reg, &f.ast);
    assert!(!bar_non_null.is_resolved(&f.reg));
    let nullable = f.reg.create_union_type(&f.ast, &[foo_type, f.null_type]);
    f.reg.declare_type(&f.ast, None, "Bar", nullable);
    closer.close(&mut f.reg, &f.ast);
    TypeSubject::assert_type(bar_non_null).to_string_is_equal_to(&mut f.reg, &f.ast, "Foo");
}
// port: NamedTypeTest#testGetBangType_onResolvedType
#[test]
fn test_get_bang_type_on_resolved_type() {
    let (mut f, _, foo_type) = set_up();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let bar_type = named_type_builder(&f, "Bar").build(&mut f.reg, &f.ast);
    let nullable = f.reg.create_union_type(&f.ast, &[foo_type, f.null_type]);
    f.reg.declare_type(&f.ast, None, "Bar", nullable);
    closer.close(&mut f.reg, &f.ast);
    let bar_non_null = bar_type.get_bang_type(&mut f.reg, &f.ast);
    TypeSubject::assert_type(bar_non_null).to_string_is_equal_to(&mut f.reg, &f.ast, "Foo");
}
// port: NamedTypeTest#testBuilderForTypeof_emitsUnrecognizedTypeError
#[test]
fn test_builder_for_typeof_emits_unrecognized_type_error() {
    let (mut f, _, _) = set_up();
    let builder = NamedTypeBuilder::new(&f.reg, "typeof Foo")
        .set_scope(Some(Arc::new(MapBasedScope::empty_scope())))
        .set_resolution_kind(ResolutionKind::TYPEOF);
    let type_ = builder.build(&mut f.reg, &f.ast);
    TypeSubject::assert_type(type_).is_unknown(&mut f.reg, &f.ast);
    f.error_reporter.lock().unwrap().expect_all_warnings(&[
        "Missing type for `typeof` value. The value must be declared and const.",
    ]);
}
// port: NamedTypeTest#testBuilderForTypeof_yieldsNoResolvedTypeWithForwardDeclaredName
#[test]
fn test_builder_for_typeof_yields_no_resolved_type_with_forward_declared_name() {
    let (mut f, _, _) = set_up();
    f.reset_registry_with_forward_declared_name("Foo");
    let builder = NamedTypeBuilder::new(&f.reg, "typeof Foo")
        .set_scope(Some(Arc::new(MapBasedScope::empty_scope())))
        .set_resolution_kind(ResolutionKind::TYPEOF);
    let type_ = builder.build(&mut f.reg, &f.ast);
    TypeSubject::assert_type(type_).is_no_resolved_type(&f.reg, "typeof Foo");
}
// port: NamedTypeTest#testBuilderForTypeof_requiresReferenceToStartWithTypeof
#[test]
fn test_builder_for_typeof_requires_reference_to_start_with_typeof() {
    let (mut f, _, _) = set_up();
    let builder = NamedTypeBuilder::new(&f.reg, "Foo")
        .set_scope(Some(Arc::new(MapBasedScope::empty_scope())))
        .set_resolution_kind(ResolutionKind::TYPEOF);
    assert!(catch_unwind(AssertUnwindSafe(|| builder.build(&mut f.reg, &f.ast))).is_err());
}
