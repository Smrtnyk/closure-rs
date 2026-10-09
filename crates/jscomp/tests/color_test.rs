/*
 * Copyright 2020 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/colors/ColorTest.java.

use closure_jscomp::{
    colors::{Color, ColorId, color::Builder, standard_colors as sc},
    testing::assert_that,
};
use closure_rhino::fast_hash::IndexSet;
use std::panic::catch_unwind;

// port: ColorTest#numberOrString
fn number_or_string() -> Color {
    Color::create_union(&IndexSet::<_>::from_iter([
        sc::STRING.clone(),
        sc::NUMBER.clone(),
    ]))
}

// port: ColorTest#unionsReportIsUnion
#[test]
fn unions_report_is_union() {
    assert_that(number_or_string()).is_union();
}
// port: ColorTest#unionsThrowWhenAskedIsPrimitive
#[test]
fn unions_throw_when_asked_is_primitive() {
    assert!(catch_unwind(|| number_or_string().is_primitive()).is_err());
}
// port: ColorTest#primitivesReportIsPrimitiveButNotUnion
#[test]
fn primitives_report_is_primitive_but_not_union() {
    let string = sc::STRING.clone();
    assert_that(string.clone()).is_primitive();
    assert!(!string.is_union());
}
// port: ColorTest#objectsReportIsObjectButNotUnionOrPrimitive
#[test]
fn objects_report_is_object_but_not_union_or_primitive() {
    let foo = Color::single_builder()
        .set_id(ColorId::from_ascii("Bar"))
        .build();
    assert!(!foo.is_primitive());
    assert!(!foo.is_union());
}
// port: ColorTest#getAlternatesReturnsAlternatesList
#[test]
fn get_alternates_returns_alternates_list() {
    assert_that(number_or_string()).has_alternates(&[sc::NUMBER.clone(), sc::STRING.clone()]);
}
// port: ColorTest#alternatesMayContainOtherUnion
#[test]
fn alternates_may_contain_other_union() {
    let new_union = Color::create_union(&IndexSet::<_>::from_iter([
        number_or_string(),
        sc::BIGINT.clone(),
    ]));
    assert_that(new_union).has_alternates(&[
        sc::STRING.clone(),
        sc::BIGINT.clone(),
        sc::NUMBER.clone(),
    ]);
}
// port: ColorTest#alternatesAreDeduplicatedFromOtherUnion
#[test]
fn alternates_are_deduplicated_from_other_union() {
    let union = Color::create_union(&IndexSet::<_>::from_iter([
        sc::NUMBER.clone(),
        number_or_string(),
    ]));
    assert_that(union).has_alternates(&[sc::NUMBER.clone(), sc::STRING.clone()]);
}
// port: ColorTest#createUnionAllowsSingleAlternate
#[test]
fn create_union_allows_single_alternate() {
    assert!(catch_unwind(|| Color::create_union(&IndexSet::<_>::default())).is_err());
    assert!(!Color::create_union(&IndexSet::<_>::from_iter([sc::NUMBER.clone()])).is_union());
}
// port: ColorTest#unknownTypeIsNotSpecialCased
#[test]
fn unknown_type_is_not_special_cased() {
    let union = Color::create_union(&IndexSet::<_>::from_iter([
        sc::UNKNOWN.clone(),
        sc::STRING.clone(),
    ]));
    assert_that(union.clone()).is_union();
    assert_eq!(union.get_union_elements().len(), 2);
}
// port: ColorTest#nullableUnionDoesNotInvalidate
#[test]
fn nullable_union_does_not_invalidate() {
    let non_invalidating_object = Color::single_builder()
        .set_id(ColorId::from_ascii("Bar"))
        .build();
    let objects = Color::create_union(&IndexSet::<_>::from_iter([
        sc::NULL_OR_VOID.clone(),
        non_invalidating_object,
    ]));
    assert_that(objects).is_not_invalidating();
}
// port: ColorTest#union_isInvalidating_ored
#[test]
fn union_is_invalidating_ored() {
    let a = create_union_setting_bool_props(Builder::set_invalidating, true, false);
    assert!(a.is_invalidating());
    let b = create_union_setting_bool_props(Builder::set_invalidating, false, false);
    assert!(!b.is_invalidating());
}
// port: ColorTest#union_propertiesKeepOriginalName_ored
#[test]
fn union_properties_keep_original_name_ored() {
    let a =
        create_union_setting_bool_props(Builder::set_properties_keep_original_name, true, false);
    assert!(a.get_properties_keep_original_name());
    let b =
        create_union_setting_bool_props(Builder::set_properties_keep_original_name, false, false);
    assert!(!b.get_properties_keep_original_name());
}
// port: ColorTest#union_isClosureAssert_anded
#[test]
fn union_is_closure_assert_anded() {
    let a = create_union_setting_bool_props(Builder::set_closure_assert, true, true);
    assert!(a.is_closure_assert());
    let b = create_union_setting_bool_props(Builder::set_closure_assert, true, false);
    assert!(!b.is_closure_assert());
}
// port: ColorTest#union_isConstructor_anded
#[test]
fn union_is_constructor_anded() {
    let a = create_union_setting_bool_props(Builder::set_constructor, true, true);
    assert!(a.is_constructor());
    let b = create_union_setting_bool_props(Builder::set_constructor, true, false);
    assert!(!b.is_constructor());
}
// port: ColorTest#createUnionSettingBoolProps
fn create_union_setting_bool_props(
    setter: fn(Builder, bool) -> Builder,
    first: bool,
    second: bool,
) -> Color {
    Color::create_union(&IndexSet::<_>::from_iter([
        setter(
            Color::single_builder().set_id(ColorId::from_ascii("Foo")),
            first,
        )
        .build(),
        setter(
            Color::single_builder().set_id(ColorId::from_ascii("Bar")),
            second,
        )
        .build(),
    ]))
}
// port: ColorTest#primitivesAreInvalidatingBasedOnConstantData
#[test]
fn primitives_are_invalidating_based_on_constant_data() {
    assert_that(sc::UNKNOWN.clone()).is_invalidating();
    assert_that(sc::TOP_OBJECT.clone()).is_invalidating();
    assert_that(sc::NUMBER.clone()).is_not_invalidating();
    assert_that(sc::STRING.clone()).is_not_invalidating();
}
// port: ColorTest#objectEqualityBasedOnClassAndFileName
#[test]
fn object_equality_based_on_class_and_file_name() {
    assert_that(
        Color::single_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .build(),
    )
    .is_equal_to(
        &Color::single_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .build(),
    );
}
// port: ColorTest#objectEqualityFalseIfInvalidatingMismatch
#[test]
fn object_equality_false_if_invalidating_mismatch() {
    assert_that(
        Color::single_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .set_invalidating(true)
            .build(),
    )
    .is_not_equal_to(
        &Color::single_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .build(),
    );
}
