/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/serialization/ColorSerializerTest.java.

//! Port of serialization/ColorSerializerTest.java.
use closure_jscomp::serialization::color_serializer::ColorSerializer;
use closure_jscomp::serialization::serialization_options::SerializationOptions;
use closure_jscomp::serialization::type_pointers::{OFFSET_TO_AXIOMATIC_COLOR, TypePointers};
use closure_jscomp::serialization::types_proto::{
    ObjectTypeProto, SubtypingEdge, TypePool, TypePoolDebugInfo, TypePoolDebugInfoMismatch,
    TypeProto, UnionTypeProto,
};
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_colors::color::Color;
use closure_rhino::jscomp_colors::color_id::ColorId;
use indexmap::{IndexMap, IndexSet};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn generate_empty_type_pool() {
    // Expect an empty TypePool when no colors are added.
    Tester::new()
        .init()
        // don't add anything
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(
            TypePool::new_builder()
                // DebugInfo is present, but empty
                .set_debug_info(TypePoolDebugInfo::get_default_instance())
                .build(),
        );

    Tester::new()
        .set_serialization_mode(
            SerializationOptions::builder()
                .set_include_debug_info(false)
                .build(),
        )
        .init()
        // don't add anything
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(TypePool::get_default_instance());
}

#[test]
fn only_add_axiomatic_colors() {
    // These are the colors that are never actually serialized, even when requested.
    // They are always just assumed to be there when deserializing and to have fixed indices as
    // indicated by their index in TypePointers.OFFSET_TO_AXIOMATIC_COLOR.
    let axiomatic_test_colors: Vec<TestColor> = OFFSET_TO_AXIOMATIC_COLOR
        .iter()
        .map(get_axiomatic_test_color)
        .collect();

    // Expect an empty TypePool when only axiomatic colors are added.
    Tester::new()
        .init()
        // add all the axiomatic colors, but only those
        .add_colors(&axiomatic_test_colors)
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(
            TypePool::new_builder()
                // No `type` values, because axiomatic colors are never serialized.
                // DebugInfo is empty, but present.
                .set_debug_info(TypePoolDebugInfo::get_default_instance())
                .build(),
        );
}

#[test]
fn add_a_super_type_relationship() {
    let super_type_color = TestObjectColorBuilder::new()
        .set_color_id("sup_type")
        .set_trimmed_pool_offset(0)
        .build();
    let sub_type_color = TestObjectColorBuilder::new()
        .set_color_id("sub_type")
        .set_trimmed_pool_offset(1)
        .build();
    let expected_type_pool = TypePool::new_builder()
        .add_type(super_type_color.get_expected_type_proto())
        .add_type(sub_type_color.get_expected_type_proto())
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(sub_type_color.expected_type_pointer)
                .set_supertype(super_type_color.expected_type_pointer)
                .build(),
        )
        // empty DebugInfo
        .set_debug_info(TypePoolDebugInfo::get_default_instance())
        .build();

    Tester::new()
        .init()
        .link_sub_color_to_super_color(&sub_type_color, &super_type_color)
        .add_color(&super_type_color)
        .add_color(&sub_type_color)
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(expected_type_pool);
}

#[test]
fn implicit_addition_of_super_type() {
    let sub_type_color = TestObjectColorBuilder::new()
        .set_color_id("sub_type")
        // The subtype will be serialized first, because it is explicitly added.
        .set_trimmed_pool_offset(0)
        .build();
    let super_type_color = TestObjectColorBuilder::new()
        .set_color_id("sup_type")
        // The supertype will be serialized second, because it gets added implicitly
        // by its relationship to the subtype.
        .set_trimmed_pool_offset(1)
        .build();
    let expected_type_pool = TypePool::new_builder()
        .add_type(sub_type_color.get_expected_type_proto())
        .add_type(super_type_color.get_expected_type_proto())
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(sub_type_color.expected_type_pointer)
                .set_supertype(super_type_color.expected_type_pointer)
                .build(),
        )
        // DebugInfo is present but empty.
        .set_debug_info(TypePoolDebugInfo::get_default_instance())
        .build();

    Tester::new()
        .init()
        .link_sub_color_to_super_color(&sub_type_color, &super_type_color)
        // Only explicitly add the subtype
        .add_color(&sub_type_color)
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(expected_type_pool);
}

#[test]
fn simulated_classes() {
    // Simulate Colors for this class:
    //
    // class Base {
    //   static baseStaticField;
    //   baseField;
    // }
    //
    // We will explicitly add the color representing `typeof Base`.
    // The colors for `Base.prototype` and the instances of `Base` will be added implicitly
    // in that order.
    let base_field_pooled_string = PooledString::create("baseField", 1);
    let base_pooled_string = PooledString::create("Base", 2);
    let typeof_base_prototype_pooled_string = PooledString::create("typeof Base.prototype", 3);
    let typeof_base_pooled_string = PooledString::create("typeof Base", 4);
    let base_class_instance_test_color = TestObjectColorBuilder::new()
        .set_color_id("Base.ins")
        // 0 => explicitly added base class constructor
        // 1 => explicitly added child class constructor
        // Then this instance color gets added implicitly
        .set_trimmed_pool_offset(2)
        .add_own_property(base_field_pooled_string.clone())
        .build();
    let base_class_prototype_test_color = TestObjectColorBuilder::new()
        .set_color_id("Base.pro")
        // This is implicitly added after the instance color above.
        .set_trimmed_pool_offset(3)
        .build();
    let base_static_field_pooled_string = PooledString::create("baseStaticField", 0);
    let base_class_constructor_test_color = TestObjectColorBuilder::new()
        .set_color_id("Base.con")
        // This is the first explicitly-added type.
        .set_trimmed_pool_offset(0)
        .set_constructor(true)
        .add_prototype_test_color(base_class_prototype_test_color.clone())
        .add_instance_test_color(base_class_instance_test_color.clone())
        .add_own_property(base_static_field_pooled_string.clone())
        .build();

    // class Child extends Base {
    //   static childStaticField;
    //   childField;
    // }
    // We will explicitly add the color representing `typeof Child`.
    // The colors for `Child.prototype` and the instances of `Child` will be added implicitly
    // in that order.
    let child_field_pooled_string = PooledString::create("childField", 5);
    let child_pooled_string = PooledString::create("Child", 6);
    let typeof_child_prototype_pooled_string = PooledString::create("typeof Child.prototype", 7);
    let typeof_child_pooled_string = PooledString::create("typeof Child", 8);
    let child_class_instance_test_color = TestObjectColorBuilder::new()
        .set_color_id("Child.in")
        // 0 => explicitly added base class constructor color
        // 1 => explicitly added child class constructor color
        // 2 => implicitly added base class instance color
        // 3 => implicitly added base class prototype color
        // Then this instance color gets added implicitly
        .set_trimmed_pool_offset(4)
        .add_own_property(child_field_pooled_string.clone())
        .build();
    let child_class_prototype_test_color = TestObjectColorBuilder::new()
        .set_color_id("Child.pr")
        // This is implicitly added after the instance color above.
        .set_trimmed_pool_offset(5)
        .build();
    let child_static_field_pooled_string = PooledString::create("childStaticField", 2);
    let child_class_constructor_test_color = TestObjectColorBuilder::new()
        .set_color_id("Child.co")
        // This is the second explicitly-added type
        .set_trimmed_pool_offset(1)
        .set_constructor(true)
        .add_prototype_test_color(child_class_prototype_test_color.clone())
        .add_instance_test_color(child_class_instance_test_color.clone())
        .add_own_property(child_static_field_pooled_string.clone())
        .build();

    let expected_type_pool = TypePool::new_builder()
        .add_type(base_class_constructor_test_color.get_expected_type_proto())
        .add_type(child_class_constructor_test_color.get_expected_type_proto())
        .add_type(base_class_instance_test_color.get_expected_type_proto())
        .add_type(base_class_prototype_test_color.get_expected_type_proto())
        .add_type(child_class_instance_test_color.get_expected_type_proto())
        .add_type(child_class_prototype_test_color.get_expected_type_proto())
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(child_class_constructor_test_color.expected_type_pointer)
                .set_supertype(base_class_constructor_test_color.expected_type_pointer)
                .build(),
        )
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(child_class_instance_test_color.expected_type_pointer)
                .set_supertype(base_class_instance_test_color.expected_type_pointer)
                .build(),
        )
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(child_class_prototype_test_color.expected_type_pointer)
                .set_supertype(base_class_prototype_test_color.expected_type_pointer)
                .build(),
        )
        // DebugInfo is present but empty.
        .set_debug_info(TypePoolDebugInfo::get_default_instance())
        .build();

    Tester::new()
        .init()
        .add_pooled_string(&base_static_field_pooled_string)
        .add_pooled_string(&base_field_pooled_string)
        .add_pooled_string(&child_static_field_pooled_string)
        .add_pooled_string(&child_field_pooled_string)
        .add_pooled_string(&base_pooled_string)
        .add_pooled_string(&typeof_base_prototype_pooled_string)
        .add_pooled_string(&typeof_base_pooled_string)
        .add_pooled_string(&child_pooled_string)
        .add_pooled_string(&typeof_child_prototype_pooled_string)
        .add_pooled_string(&typeof_child_pooled_string)
        // Only explicitly add the constructor types
        .add_color(&base_class_constructor_test_color)
        .add_color(&child_class_constructor_test_color)
        .link_sub_color_to_super_color(
            &child_class_constructor_test_color,
            &base_class_constructor_test_color,
        )
        .link_sub_color_to_super_color(
            &child_class_instance_test_color,
            &base_class_instance_test_color,
        )
        .link_sub_color_to_super_color(
            &child_class_prototype_test_color,
            &base_class_prototype_test_color,
        )
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(expected_type_pool);
}

#[test]
fn include_mismatches() {
    let test_color1 = TestObjectColorBuilder::new()
        .set_color_id("color001")
        .set_trimmed_pool_offset(0)
        .build();
    let test_color2 = TestObjectColorBuilder::new()
        .set_color_id("color002")
        .set_trimmed_pool_offset(1)
        .build();
    let test_mismatch1 = TestMismatch::create("location1", &[&test_color1, &test_color2]);

    let test_color3 = TestObjectColorBuilder::new()
        .set_color_id("color003")
        .set_trimmed_pool_offset(2)
        .build();
    let test_color4 = TestObjectColorBuilder::new()
        .set_color_id("color004")
        .set_trimmed_pool_offset(3)
        .build();
    let test_mismatch2 = TestMismatch::create("location2", &[&test_color3, &test_color4]);

    let expected_type_pool = TypePool::new_builder()
        .add_type(test_color1.get_expected_type_proto())
        .add_type(test_color2.get_expected_type_proto())
        .add_type(test_color3.get_expected_type_proto())
        .add_type(test_color4.get_expected_type_proto())
        .set_debug_info(
            TypePoolDebugInfo::new_builder()
                .add_mismatch(test_mismatch1.get_expected_mismatch())
                .add_mismatch(test_mismatch2.get_expected_mismatch())
                .build(),
        )
        .build();

    Tester::new()
        .init()
        .add_colors(&[
            test_color1.clone(),
            test_color2.clone(),
            test_color3.clone(),
            test_color4.clone(),
        ])
        .add_mismatch(&test_mismatch1)
        .add_mismatch(&test_mismatch2)
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(expected_type_pool);
}

#[test]
fn skip_mismatches() {
    let test_color1 = TestObjectColorBuilder::new()
        .set_color_id("color001")
        .set_trimmed_pool_offset(0)
        .build();
    let test_color2 = TestObjectColorBuilder::new()
        .set_color_id("color002")
        .set_trimmed_pool_offset(1)
        .build();
    let test_mismatch1 = TestMismatch::create("location1", &[&test_color1, &test_color2]);

    let test_color3 = TestObjectColorBuilder::new()
        .set_color_id("color003")
        .set_trimmed_pool_offset(2)
        .build();
    let test_color4 = TestObjectColorBuilder::new()
        .set_color_id("color004")
        .set_trimmed_pool_offset(3)
        .build();
    let test_mismatch2 = TestMismatch::create("location2", &[&test_color3, &test_color4]);

    let expected_type_pool = TypePool::new_builder()
        .add_type(test_color1.get_expected_type_proto())
        .add_type(test_color2.get_expected_type_proto())
        .add_type(test_color3.get_expected_type_proto())
        .add_type(test_color4.get_expected_type_proto())
        // No debug info
        .build();

    Tester::new()
        .set_serialization_mode(
            SerializationOptions::builder()
                .set_include_debug_info(false)
                .build(),
        )
        .init()
        .add_colors(&[
            test_color1.clone(),
            test_color2.clone(),
            test_color3.clone(),
            test_color4.clone(),
        ])
        .add_mismatch(&test_mismatch1)
        .add_mismatch(&test_mismatch2)
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(expected_type_pool);
}

#[test]
fn filter_property_names() {
    // Tell ColorSerializer to only keep property names beginning with "kept"
    let property_name_filter = |prop_name: &JsString| prop_name.to_string().starts_with("kept");

    // Build a type that has a property name that should be kept and one that should be
    // dropped.
    let kept_property_name = PooledString::create("keptProperty", 0);
    let dropped_property_name = PooledString::create("droppedProperty", 1);
    let test_color = TestObjectColorBuilder::new()
        .set_color_id("color001")
        .set_trimmed_pool_offset(0)
        .add_own_property(kept_property_name.clone())
        .add_own_property(dropped_property_name.clone())
        .build();

    let actual_type_pool = Tester::new()
        .set_property_name_filter(Box::new(property_name_filter))
        .init()
        .add_pooled_string(&kept_property_name)
        .add_pooled_string(&dropped_property_name)
        .add_color(&test_color)
        .generate_type_pool()
        .type_pool;

    let own_property_list = actual_type_pool
        .get_type(0)
        .get_object()
        .get_own_property_list();
    assert_eq!(own_property_list, &[kept_property_name.pool_offset]);
}

#[test]
fn add_union_implicitly_adds_members() {
    let test_color1 = TestObjectColorBuilder::new()
        .set_color_id("color001")
        // We will explicitly add this color first
        .set_trimmed_pool_offset(1)
        .build();
    let test_color2 = TestObjectColorBuilder::new()
        .set_color_id("color002")
        .set_trimmed_pool_offset(2)
        .build();
    let union_test_color = TestUnionColorBuilder::new()
        .set_trimmed_pool_offset(0)
        .add_test_color(test_color1.clone())
        .add_test_color(test_color2.clone())
        .build();

    let expected_type_pool = TypePool::new_builder()
        .add_type(union_test_color.get_expected_type_proto())
        .add_type(test_color1.get_expected_type_proto())
        .add_type(test_color2.get_expected_type_proto())
        // empty DebugInfo
        .set_debug_info(TypePoolDebugInfo::get_default_instance())
        .build();

    Tester::new()
        .init()
        .add_color(&union_test_color)
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(expected_type_pool);
}

#[test]
fn avoid_adding_duplicate_unions() {
    let test_color1 = TestObjectColorBuilder::new()
        .set_color_id("color001")
        // We will explicitly add this color first
        .set_trimmed_pool_offset(1)
        .build();
    let test_color2 = TestObjectColorBuilder::new()
        .set_color_id("color002")
        .set_trimmed_pool_offset(2)
        .build();
    let union_test_color = TestUnionColorBuilder::new()
        .set_trimmed_pool_offset(0)
        .add_test_color(test_color1.clone())
        .add_test_color(test_color2.clone())
        .build();
    // Creating a second union containing the same types will create a distinct
    // Java object, but it will have the same ColorId.
    let duplicate_union_test_color = TestUnionColorBuilder::new()
        .set_trimmed_pool_offset(0) // should end up getting the same offset
        .add_test_color(test_color1.clone())
        .add_test_color(test_color2.clone())
        .build();

    let expected_type_pool = TypePool::new_builder()
        .add_type(union_test_color.get_expected_type_proto())
        // duplicate Union color should not be serialized
        .add_type(test_color1.get_expected_type_proto())
        .add_type(test_color2.get_expected_type_proto())
        // empty DebugInfo
        .set_debug_info(TypePoolDebugInfo::get_default_instance())
        .build();

    Tester::new()
        .init()
        .add_color(&union_test_color)
        // Try to add the duplicate union color. It should be ignored.
        .add_color(&duplicate_union_test_color)
        .generate_type_pool()
        .assert_that_type_pool_is_equal_to(expected_type_pool);
}

/// Represents a string that is stored in a StringPool, recording both its value and its offset.
#[derive(Clone)]
struct PooledString {
    value: String,
    pool_offset: i32,
}

impl PooledString {
    fn create(value: &str, pool_offset: i32) -> Self {
        Self {
            value: value.to_string(),
            pool_offset,
        }
    }
}

/// Builds objects to represent both an object Color to be added and the Integer and TypeProto we
/// expect to be generated for it.
struct TestObjectColorBuilder {
    instance_test_colors: Vec<TestColor>,
    prototype_test_colors: Vec<TestColor>,
    own_properties: Vec<PooledString>,

    color_id: Option<ColorId>,
    trimmed_pool_offset: i32,
    is_constructor: bool,
}

impl TestObjectColorBuilder {
    fn new() -> Self {
        Self {
            instance_test_colors: Vec::new(),
            prototype_test_colors: Vec::new(),
            own_properties: Vec::new(),
            color_id: None,
            trimmed_pool_offset: -1,
            is_constructor: false,
        }
    }

    fn set_color_id(self, color_id_string: &str) -> Self {
        assert!(
            color_id_string.len() <= 8,
            "color ID string too long: {color_id_string}"
        );
        self.set_color_id_value(ColorId::from_ascii(color_id_string))
    }

    fn set_color_id_value(mut self, color_id: ColorId) -> Self {
        self.color_id = Some(color_id);
        self
    }

    fn set_trimmed_pool_offset(mut self, trimmed_pool_offset: i32) -> Self {
        assert!(
            trimmed_pool_offset >= 0,
            "invalid trimmedPoolOffset: {trimmed_pool_offset}"
        );
        self.trimmed_pool_offset = trimmed_pool_offset;
        self
    }

    fn set_constructor(mut self, constructor: bool) -> Self {
        self.is_constructor = constructor;
        self
    }

    fn add_instance_test_color(mut self, instance_test_color: TestColor) -> Self {
        self.instance_test_colors.push(instance_test_color);
        self
    }

    fn add_prototype_test_color(mut self, prototype_test_color: TestColor) -> Self {
        self.prototype_test_colors.push(prototype_test_color);
        self
    }

    fn add_own_property(mut self, own_property: PooledString) -> Self {
        self.own_properties.push(own_property);
        self
    }

    fn build(self) -> TestColor {
        assert!(
            self.trimmed_pool_offset >= 0,
            "call setTrimmedPoolOffset() first"
        );
        // There's no testing benefit to varying these values
        let properties_keep_original_name = false;
        let is_invalidating = false;
        let is_closure_assert = false;
        let color_id = self.color_id.expect("colorId");

        let color_builder = Color::single_builder()
            .set_id(color_id)
            .set_invalidating(is_invalidating)
            .set_properties_keep_original_name(properties_keep_original_name)
            .set_constructor(self.is_constructor)
            .set_closure_assert(is_closure_assert)
            .set_instance_colors(
                self.instance_test_colors
                    .iter()
                    .map(|c| c.color.clone())
                    .collect::<IndexSet<_>>(),
            )
            .set_prototypes(
                self.prototype_test_colors
                    .iter()
                    .map(|c| c.color.clone())
                    .collect::<IndexSet<_>>(),
            )
            .set_own_properties(
                self.own_properties
                    .iter()
                    .map(|p| JsString::from(p.value.as_str()))
                    .collect::<IndexSet<_>>(),
            );
        let type_pointer = TypePointers::untrim_offset(self.trimmed_pool_offset);
        let object_type_proto_builder = ObjectTypeProto::new_builder()
            .set_uuid(color_id.as_byte_string())
            .set_is_invalidating(is_invalidating)
            .set_properties_keep_original_name(properties_keep_original_name)
            .set_marked_constructor(self.is_constructor)
            .set_closure_assert(is_closure_assert)
            .add_all_instance_type(
                self.instance_test_colors
                    .iter()
                    .map(|c| c.expected_type_pointer),
            )
            .add_all_prototype(
                self.prototype_test_colors
                    .iter()
                    .map(|c| c.expected_type_pointer),
            )
            .add_all_own_property(self.own_properties.iter().map(|p| p.pool_offset));
        let type_proto = TypeProto::new_builder()
            .set_object(object_type_proto_builder)
            .build();
        TestColor::create(color_builder.build(), Some(type_proto), type_pointer)
    }
}

struct TestUnionColorBuilder {
    member_test_colors: Vec<TestColor>,
    trimmed_pool_offset: i32,
}

impl TestUnionColorBuilder {
    fn new() -> Self {
        Self {
            member_test_colors: Vec::new(),
            trimmed_pool_offset: -1,
        }
    }

    fn set_trimmed_pool_offset(mut self, trimmed_pool_offset: i32) -> Self {
        assert!(
            trimmed_pool_offset >= 0,
            "invalid trimmedPoolOffset: {trimmed_pool_offset}"
        );
        self.trimmed_pool_offset = trimmed_pool_offset;
        self
    }

    fn add_test_color(mut self, test_color: TestColor) -> Self {
        self.member_test_colors.push(test_color);
        self
    }

    fn build(self) -> TestColor {
        assert!(
            self.trimmed_pool_offset >= 0,
            "call setTrimmedPoolOffset() first"
        );
        let member_colors: IndexSet<Color> = self
            .member_test_colors
            .iter()
            .map(|c| c.color.clone())
            .collect();
        let color = Color::create_union(&member_colors);

        let type_pointer = TypePointers::untrim_offset(self.trimmed_pool_offset);

        let member_type_poiners: Vec<i32> = self
            .member_test_colors
            .iter()
            .map(|c| c.expected_type_pointer)
            .collect();
        let type_proto = TypeProto::new_builder()
            .set_union(UnionTypeProto::new_builder().add_all_union_member(member_type_poiners))
            .build();
        TestColor::create(color, Some(type_proto), type_pointer)
    }
}

fn get_axiomatic_test_color(axiomatic_color: &Color) -> TestColor {
    let pool_offset = OFFSET_TO_AXIOMATIC_COLOR
        .iter()
        .position(|c| c == axiomatic_color)
        .map_or(-1, |i| i as i32);
    assert!(
        pool_offset >= 0,
        "Not an axiomatic color: {axiomatic_color:?}"
    );
    TestColor::create(axiomatic_color.clone(), None, pool_offset)
}

/// Represents a Color that has been or will be added to the ColorSerializer.
///
/// `nullable_expected_type_proto` is the TypeProto we expect ColorSerializer to create for this
/// Color. For an axiomatic color this will be `None`, since those are never stored into a
/// `TypeProto`. Generally test code should call `get_expected_type_proto()` instead in order to
/// get an exception if an attempt is made to serialize an axiomatic color.
#[derive(Clone)]
struct TestColor {
    // The Color that will be added to the ColorSerializer.
    color: Color,
    nullable_expected_type_proto: Option<TypeProto>,
    // The Integer we expect ColorSerializer to create for this Color.
    expected_type_pointer: i32,
}

impl TestColor {
    fn get_expected_type_proto(&self) -> TypeProto {
        self.nullable_expected_type_proto.clone().unwrap()
    }

    fn create(
        color: Color,
        expected_type_proto: Option<TypeProto>,
        nullable_expected_type_pointer: i32,
    ) -> Self {
        Self {
            color,
            nullable_expected_type_proto: expected_type_proto,
            expected_type_pointer: nullable_expected_type_pointer,
        }
    }
}

/// Represents a color/type mismatch both as it would appear in a ColorRegistry and in a TypePool.
struct TestMismatch {
    location_string: String,
    test_colors: Vec<TestColor>,
}

impl TestMismatch {
    fn get_colors(&self) -> Vec<Color> {
        self.test_colors.iter().map(|c| c.color.clone()).collect()
    }

    fn get_expected_mismatch(&self) -> TypePoolDebugInfoMismatch {
        let involved_color_type_pointers: Vec<i32> = self
            .test_colors
            .iter()
            .map(|c| c.expected_type_pointer)
            .collect();
        TypePoolDebugInfoMismatch::new_builder()
            .set_source_ref(self.location_string.clone())
            .add_all_involved_color(involved_color_type_pointers)
            .build()
    }

    /// `location_string` is used as the `source_ref` string in the Mismatch proto; `test_colors`
    /// as the `involved_color` repeated field in the Mismatch proto, in the order.
    fn create(location_string: &str, test_colors: &[&TestColor]) -> Self {
        Self {
            location_string: location_string.to_string(),
            test_colors: test_colors.iter().map(|c| (*c).clone()).collect(),
        }
    }
}

type PropertyNameFilter = Box<dyn Fn(&JsString) -> bool>;

struct Tester {
    // Simulates ColorRegistry#getDisambiguationSuperTypes()
    sub_color_to_super_colors_map: IndexMap<Color, IndexSet<Color>>,
    // Simulates ColorRegistry#getMismatchLocationsForDebugging()
    color_to_mismatch_location_strings_map: IndexMap<Color, IndexSet<String>>,
    // Simulates StringPool.Builder#put()
    string_to_pool_offset_map: Rc<RefCell<IndexMap<JsString, i32>>>,
    // Test object
    color_serializer: Option<ColorSerializer<'static>>,
    // Default to the serialization mode that includes the most information.
    // If the test case includes information that doesn't make it into the expected result,
    // then I want the test case to clearly request that the information be omitted.
    serialization_mode: SerializationOptions,
    // Tells ColorSerializer which property names to serialize
    property_name_filter: Option<PropertyNameFilter>,
}

impl Tester {
    fn new() -> Self {
        Self {
            sub_color_to_super_colors_map: IndexMap::new(),
            color_to_mismatch_location_strings_map: IndexMap::new(),
            string_to_pool_offset_map: Rc::new(RefCell::new(IndexMap::new())),
            color_serializer: None,
            serialization_mode: SerializationOptions::builder()
                .set_include_debug_info(true)
                .set_run_validation(true)
                .build(),
            property_name_filter: Some(Box::new(|_prop_name| true)),
        }
    }

    fn set_serialization_mode(mut self, serialization_mode: SerializationOptions) -> Self {
        assert!(
            self.color_serializer.is_none(),
            "call this method before init()"
        );
        self.serialization_mode = serialization_mode;
        self
    }

    fn set_property_name_filter(mut self, property_name_filter: PropertyNameFilter) -> Self {
        assert!(
            self.color_serializer.is_none(),
            "call this method before init()"
        );
        self.property_name_filter = Some(property_name_filter);
        self
    }

    fn init(mut self) -> Self {
        assert!(
            self.color_serializer.is_none(),
            "init() called more than once"
        );
        let string_to_pool_offset_map = self.string_to_pool_offset_map.clone();
        self.color_serializer = Some(ColorSerializer::new(
            self.serialization_mode.clone(),
            move |str: &JsString| {
                *string_to_pool_offset_map
                    .borrow()
                    .get(str)
                    .unwrap_or_else(|| panic!("unexpected string: {str}"))
            },
            self.property_name_filter.take().unwrap(),
        ));
        self
    }

    /// Be prepared to respond to a request for this string.
    fn add_pooled_string(self, pooled_string: &PooledString) -> Self {
        let string = JsString::from(pooled_string.value.as_str());
        assert!(
            !self
                .string_to_pool_offset_map
                .borrow()
                .contains_key(&string),
            "duplicate string added: {string}"
        );
        self.string_to_pool_offset_map
            .borrow_mut()
            .insert(string, pooled_string.pool_offset);
        self
    }

    fn link_sub_color_to_super_color(
        mut self,
        sub_color: &TestColor,
        super_color: &TestColor,
    ) -> Self {
        self.sub_color_to_super_colors_map
            .entry(sub_color.color.clone())
            .or_default()
            .insert(super_color.color.clone());
        self
    }

    fn add_mismatch(mut self, test_mismatch: &TestMismatch) -> Self {
        let location_string = &test_mismatch.location_string;
        for color in test_mismatch.get_colors() {
            self.color_to_mismatch_location_strings_map
                .entry(color)
                .or_default()
                .insert(location_string.clone());
        }
        self
    }

    fn add_color(mut self, test_color: &TestColor) -> Self {
        let type_pointer = self
            .color_serializer
            .as_mut()
            .unwrap()
            .add_color(&test_color.color);
        assert_eq!(type_pointer, test_color.expected_type_pointer);
        self
    }

    fn add_colors(mut self, test_color_list: &[TestColor]) -> Self {
        let color_serializer = self.color_serializer.as_mut().expect("call init() first");
        let colors: Vec<Color> = test_color_list.iter().map(|c| c.color.clone()).collect();
        let type_pointers = color_serializer.add_colors(colors.iter());
        for i in 0..test_color_list.len() {
            assert_eq!(type_pointers[i], test_color_list[i].expected_type_pointer);
        }
        self
    }

    fn generate_type_pool(mut self) -> GenerateTypePoolTestResult {
        let color_serializer = self.color_serializer.as_mut().expect("call init() first");
        let sub_color_to_super_colors_map = &self.sub_color_to_super_colors_map;
        let color_to_mismatch_location_strings_map = &self.color_to_mismatch_location_strings_map;
        let type_pool = color_serializer.generate_type_pool(
            &|subtype_color: &Color| {
                sub_color_to_super_colors_map
                    .get(subtype_color)
                    .cloned()
                    .unwrap_or_default()
            },
            Some(&|color: &Color| {
                color_to_mismatch_location_strings_map
                    .get(color)
                    .cloned()
                    .unwrap_or_default()
            }),
        );
        GenerateTypePoolTestResult::create(type_pool)
    }
}

/// The result of Tester::generateTypePool()
struct GenerateTypePoolTestResult {
    type_pool: TypePool,
}

impl GenerateTypePoolTestResult {
    fn assert_that_type_pool_is_equal_to(&self, expected: TypePool) {
        assert_eq!(self.type_pool, expected);
    }

    fn create(type_pool: TypePool) -> Self {
        Self { type_pool }
    }
}
