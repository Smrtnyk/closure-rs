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
//   test/com/google/javascript/jscomp/serialization/ColorPoolTest.java.

//! Port of serialization/ColorPoolTest.java.
use closure_jscomp::serialization::color_pool::{ColorPool, default_type_pool};
use closure_jscomp::serialization::malformed_typed_ast_exception::MalformedTypedAstException;
use closure_jscomp::serialization::string_pool::StringPool;
use closure_jscomp::serialization::type_pointers::TypePointers;
use closure_jscomp::serialization::types_proto::{
    ObjectTypeProto, PrimitiveType, SubtypingEdge, TypePool, TypePoolDebugInfo,
    TypePoolDebugInfoMismatch, TypeProto, UnionTypeProto,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_colors::color::{Builder as ColorBuilder, Color};
use closure_rhino::jscomp_colors::color_id::ColorId;
use closure_rhino::jscomp_colors::color_registry::{self, ColorRegistry};
use closure_rhino::jscomp_colors::standard_colors;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

const TEST_ID: ColorId = ColorId::from_unsigned(100);

fn create_object_color_builder() -> ColorBuilder {
    Color::single_builder().set_id(ColorId::from_ascii(""))
}

fn single_object_pool(builder: ObjectTypeProto) -> Arc<TypePool> {
    Arc::new(
        TypePool::new_builder()
            .add_type(TypeProto::new_builder().set_object(builder.build()))
            .build(),
    )
}

fn pool_pointer(offset: i32) -> i32 {
    TypePointers::untrim_offset(offset)
}

fn utf8(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

fn object(uuid: Vec<u8>) -> TypeProto {
    TypeProto::new_builder().set_object(ObjectTypeProto::new_builder().set_uuid(uuid))
}

fn assert_throws_malformed<R>(f: impl FnOnce() -> R) {
    let result = catch_unwind(AssertUnwindSafe(f));
    match result {
        Ok(_) => panic!("expected MalformedTypedAstException"),
        Err(payload) => assert!(
            payload
                .downcast_ref::<MalformedTypedAstException>()
                .is_some(),
            "expected MalformedTypedAstException, got {:?}",
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        ),
    }
}

fn assert_throws<R>(f: impl FnOnce() -> R) {
    assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
}

fn set_of(colors: &[&Color]) -> IndexSet<Color> {
    colors.iter().map(|c| (*c).clone()).collect()
}

fn assert_contains_exactly(actual: &IndexSet<Color>, expected: &[&Color]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
    assert_eq!(actual, &set_of(expected));
}

fn supertypes(registry: &ColorRegistry, color: &Color) -> IndexSet<Color> {
    registry.get_disambiguation_supertypes(color).clone()
}

// port: ColorPoolTest#deserializesJavaScriptPrimitivesFromEmptyTypePool
#[test]
fn deserializes_java_script_primitives_from_empty_type_pool() {
    let pool = ColorPool::from_only_shard_for_testing(default_type_pool(), StringPool::empty());
    let color_pool = pool.get_only_shard_for_testing();

    assert_eq!(
        color_pool.get_color(PrimitiveType::NUMBER_TYPE.get_number()),
        *standard_colors::NUMBER
    );
    assert!(
        !color_pool
            .get_color(PrimitiveType::NUMBER_TYPE.get_number())
            .is_invalidating()
    );
    assert_eq!(
        color_pool.get_color(PrimitiveType::STRING_TYPE.get_number()),
        *standard_colors::STRING
    );
    assert_eq!(
        color_pool.get_color(PrimitiveType::UNKNOWN_TYPE.get_number()),
        *standard_colors::UNKNOWN
    );
}

// port: ColorPoolTest#deserializesTopObjectTypeFromEmptyTypePool
#[test]
fn deserializes_top_object_type_from_empty_type_pool() {
    let pool = ColorPool::from_only_shard_for_testing(default_type_pool(), StringPool::empty());
    let color_pool = pool.get_only_shard_for_testing();

    let object = color_pool.get_color(PrimitiveType::TOP_OBJECT.get_number());
    assert!(object.is_invalidating());
    assert!(object.get_instance_colors().is_empty());
    assert!(object.get_prototypes().is_empty());
}

// port: ColorPoolTest#deserializesNativeObjectTableIntoNativeColor
#[test]
fn deserializes_native_object_table_into_native_color() {
    let string_pool = StringPool::builder();
    let type_pool = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder()
                .set_object(
                    ObjectTypeProto::new_builder()
                        .set_uuid(standard_colors::NUMBER_OBJECT_ID.as_byte_string())
                        .set_is_invalidating(true),
                )
                .build(),
        )
        .add_type(object(utf8("Num.pro")).build())
        .add_disambiguation_edges(
            // Number is a subtype of Number.prototype
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(0))
                .set_supertype(pool_pointer(1)),
        )
        .build();

    let color_pool =
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), Arc::new(string_pool.build()));
    let view = color_pool.get_only_shard_for_testing();

    let number_object = color_pool
        .get_color(standard_colors::NUMBER_OBJECT_ID)
        .unwrap();
    assert!(number_object.is_invalidating());
    assert_contains_exactly(
        &supertypes(color_pool.get_registry(), &number_object),
        &[&view.get_color(pool_pointer(1))],
    );
    assert!(number_object.ptr_eq(&view.get_color(pool_pointer(0))));
    assert_eq!(number_object.get_id(), standard_colors::NUMBER_OBJECT_ID);
}

// port: ColorPoolTest#testSynthesizesMissingStandardColors
#[test]
fn test_synthesizes_missing_standard_colors() {
    let color_pool =
        ColorPool::from_only_shard_for_testing(default_type_pool(), StringPool::empty());

    for &id in color_registry::REQUIRED_IDS.iter() {
        assert_eq!(color_pool.get_color(id).unwrap().get_id(), id);
    }
}

// port: ColorPoolTest#deserializesSimpleObject
#[test]
fn deserializes_simple_object() {
    let type_pool = TypePool::new_builder()
        .add_type(object(utf8("Foo")))
        .build();
    let pool = ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty());
    let color_pool = pool.get_only_shard_for_testing();

    assert_eq!(
        color_pool.get_color(pool_pointer(0)),
        Color::single_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .build()
    );
}

// port: ColorPoolTest#deserializesObjectWithPrototypeAndInstanceType
#[test]
fn deserializes_object_with_prototype_and_instance_type() {
    let type_pool = TypePool::new_builder()
        .add_type(object(utf8("Foo.pro")))
        .add_type(object(utf8("Foo.ins")))
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(utf8("Foo"))
                    .add_prototype(pool_pointer(0))
                    .add_instance_type(pool_pointer(1))
                    .set_marked_constructor(true),
            ),
        )
        .build();
    let pool = ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty());
    let color_pool = pool.get_only_shard_for_testing();

    assert_eq!(
        color_pool.get_color(pool_pointer(2)),
        create_object_color_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .set_instance_color(Some(
                create_object_color_builder()
                    .set_id(ColorId::from_ascii("Foo.ins"))
                    .build()
            ))
            .set_prototype(Some(
                create_object_color_builder()
                    .set_id(ColorId::from_ascii("Foo.pro"))
                    .build()
            ))
            .set_constructor(true)
            .build()
    );
}

// port: ColorPoolTest#deserializesPropertiesFromStringPool
#[test]
fn deserializes_properties_from_string_pool() {
    let type_pool = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(utf8("Foo"))
                    .add_own_property(1)
                    .add_own_property(2),
            ),
        )
        .build();
    let string_pool = StringPool::builder().put_and("x").put_and("y").build();
    let pool = ColorPool::from_only_shard_for_testing(Arc::new(type_pool), Arc::new(string_pool));
    let color_pool = pool.get_only_shard_for_testing();

    assert_eq!(
        color_pool.get_color(pool_pointer(0)),
        create_object_color_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .set_own_properties(
                [JsString::from("x"), JsString::from("y")]
                    .into_iter()
                    .collect()
            )
            .build()
    );
}

// port: ColorPoolTest#marksInvalidatingObject
#[test]
fn marks_invalidating_object() {
    let type_pool = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(utf8("Foo"))
                    .set_is_invalidating(true),
            ),
        )
        .build();
    let pool = ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty());
    let color_pool = pool.get_only_shard_for_testing();

    assert!(color_pool.get_color(pool_pointer(0)).is_invalidating());
    assert_eq!(
        color_pool.get_color(pool_pointer(0)).get_id(),
        ColorId::from_ascii("Foo")
    );
}

// port: ColorPoolTest#marksClosureAssert
#[test]
fn marks_closure_assert() {
    let type_pool = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(utf8("Foo"))
                    .set_closure_assert(true),
            ),
        )
        .build();
    let pool = ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty());
    let color_pool = pool.get_only_shard_for_testing();

    assert!(color_pool.get_color(pool_pointer(0)).is_closure_assert());
    assert_eq!(
        color_pool.get_color(pool_pointer(0)).get_id(),
        ColorId::from_ascii("Foo")
    );
}

// port: ColorPoolTest#recordsDisambiguationsSupertypes_multipleSupertypesPerSubtype
#[test]
fn records_disambiguations_supertypes_multiple_supertypes_per_subtype() {
    let type_pool = TypePool::new_builder()
        .add_type(object(utf8("Foo")))
        .add_type(object(utf8("Bar")))
        .add_type(object(utf8("Baz")))
        // Bar is a subtype of Foo
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(1))
                .set_supertype(pool_pointer(0)),
        )
        // Bar is also a subtype of Baz
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(1))
                .set_supertype(pool_pointer(2)),
        )
        .build();

    // When
    let mut color_pool_builder = ColorPool::builder();
    let shard = color_pool_builder.add_shard(Arc::new(type_pool), StringPool::empty());
    let color_pool = color_pool_builder.build();

    // Then
    let foo = shard.get_color(pool_pointer(0));
    let bar = shard.get_color(pool_pointer(1));
    let baz = shard.get_color(pool_pointer(2));

    assert_contains_exactly(&supertypes(color_pool.get_registry(), &bar), &[&foo, &baz]);
}

// port: ColorPoolTest#recordsDisambiguationsSupertypes_cylceInSupertypeGraph
#[test]
fn records_disambiguations_supertypes_cylce_in_supertype_graph() {
    let type_pool = TypePool::new_builder()
        .add_type(object(utf8("Foo")))
        .add_type(object(utf8("Bar")))
        // Foo is a subtype of Bar
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(0))
                .set_supertype(pool_pointer(1)),
        )
        // Bar is also a subtype of Foo
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(1))
                .set_supertype(pool_pointer(0)),
        )
        .build();

    // When
    let mut color_pool_builder = ColorPool::builder();
    let shard = color_pool_builder.add_shard(Arc::new(type_pool), StringPool::empty());
    let color_pool = color_pool_builder.build();

    // Then
    let foo = shard.get_color(pool_pointer(0));
    let bar = shard.get_color(pool_pointer(1));

    assert_contains_exactly(&supertypes(color_pool.get_registry(), &foo), &[&bar]);
    assert_contains_exactly(&supertypes(color_pool.get_registry(), &bar), &[&foo]);
}

// port: ColorPoolTest#throwsErrorIfDisambiguationEdgesContainsInvalidOffset
#[test]
fn throws_error_if_disambiguation_edges_contains_invalid_offset() {
    let type_pool = TypePool::new_builder()
        .add_type(object(utf8("Foo")))
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(0))
                .set_supertype(pool_pointer(1)),
        )
        .build();
    assert_throws_malformed(|| {
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty())
    });
}

fn union_of(members: &[i32]) -> TypeProto {
    let mut union = UnionTypeProto::new_builder();
    for &member in members {
        union = union.add_union_member(member);
    }
    TypeProto::new_builder().set_union(union)
}

// port: ColorPoolTest#deserializesMultipleUnionsOfNativeTypes
#[test]
fn deserializes_multiple_unions_of_native_types() {
    let type_pool = TypePool::new_builder()
        .add_type(union_of(&[
            PrimitiveType::NUMBER_TYPE.get_number(),
            PrimitiveType::STRING_TYPE.get_number(),
        ]))
        .add_type(union_of(&[
            PrimitiveType::NUMBER_TYPE.get_number(),
            PrimitiveType::BIGINT_TYPE.get_number(),
        ]))
        .build();

    let color_pool =
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty());
    let view = color_pool.get_only_shard_for_testing();

    assert_contains_exactly(
        view.get_color(pool_pointer(0)).get_union_elements(),
        &[&standard_colors::STRING, &standard_colors::NUMBER],
    );
    assert_contains_exactly(
        view.get_color(pool_pointer(1)).get_union_elements(),
        &[&standard_colors::BIGINT, &standard_colors::NUMBER],
    );
}

// port: ColorPoolTest#deserializesUnionReferencingOtherUnion
#[test]
fn deserializes_union_referencing_other_union() {
    let type_pool = TypePool::new_builder()
        // U0 := (number, string)
        .add_type(union_of(&[
            PrimitiveType::NUMBER_TYPE.get_number(),
            PrimitiveType::STRING_TYPE.get_number(),
        ]))
        // U1 := (U0, bigint)
        .add_type(union_of(&[
            pool_pointer(0),
            PrimitiveType::BIGINT_TYPE.get_number(),
        ]))
        .build();

    assert_throws_malformed(|| {
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty())
    });
}

// port: ColorPoolTest#deserializingUnionsInCycleThrowsErrors
#[test]
fn deserializing_unions_in_cycle_throws_errors() {
    // Create two union types U0 and U1. U0 := (U1 | NUMBER), U1 := (U0 | number)
    // These cycles are at least possible to construct in the Closure type system.
    // (Note - cycles are much more likely in object types)
    let type_pool = TypePool::new_builder()
        .add_type(union_of(&[
            PrimitiveType::NUMBER_TYPE.get_number(),
            pool_pointer(1),
        ]))
        .add_type(union_of(&[
            PrimitiveType::NUMBER_TYPE.get_number(),
            pool_pointer(0),
        ]))
        .build();

    // Eventually we may need to support this case, but for now throwing an explicit exception is
    // better than infinite recursion.

    assert_throws_malformed(|| {
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty())
            .get_only_shard_for_testing()
            .get_color(pool_pointer(0))
    });
}

// port: ColorPoolTest#throwsException_onTypeWithoutKindCase
#[test]
fn throws_exception_on_type_without_kind_case() {
    let type_pool = TypePool::new_builder()
        .add_type(TypeProto::get_default_instance())
        .build();

    assert_throws_malformed(|| {
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty())
            .get_only_shard_for_testing()
            .get_color(pool_pointer(0))
    });
}

// port: ColorPoolTest#throwsException_onSerializedUnionOfSingleElement
#[test]
fn throws_exception_on_serialized_union_of_single_element() {
    let type_pool = TypePool::new_builder()
        .add_type(union_of(&[PrimitiveType::NUMBER_TYPE.get_number()]))
        .build();

    assert_throws_malformed(|| {
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty())
            .get_only_shard_for_testing()
            .get_color(pool_pointer(0))
    });
}

// port: ColorPoolTest#throwsException_onOutOfBoundsStringPoolOffset
#[test]
fn throws_exception_on_out_of_bounds_string_pool_offset() {
    let type_pool = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(utf8("Foo"))
                    .add_own_property(1001),
            ),
        )
        .build();
    // IndexOutOfBoundsException
    assert_throws(|| {
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty())
    });
}

// port: ColorPoolTest#throwsException_onIdenticalPools
#[test]
fn throws_exception_on_identical_pools() {
    let type_pool = Arc::new(
        TypePool::new_builder()
            .add_type(object(utf8("Foo")))
            .build(),
    );
    let mut builder = ColorPool::builder();
    builder.add_shard_and(type_pool.clone(), StringPool::empty());

    assert_throws(|| builder.add_shard(type_pool.clone(), StringPool::empty()));
}

// port: ColorPoolTest#throwsException_onIdenticalPools_exceptDefaultInstance
#[test]
fn throws_exception_on_identical_pools_except_default_instance() {
    // Given
    let mut builder = ColorPool::builder();
    let shard = builder.add_shard(default_type_pool(), StringPool::empty());

    // When
    let other_shard = builder.add_shard(default_type_pool(), StringPool::empty());

    // Then
    assert!(Arc::ptr_eq(&shard, &other_shard));
}

// port: ColorPoolTest#throwsException_onDuplicateIdsInSinglePool
#[test]
fn throws_exception_on_duplicate_ids_in_single_pool() {
    // Given
    let type_pool = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(TEST_ID.as_byte_string())
                    .build(),
            ),
        )
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(TEST_ID.as_byte_string())
                    .build(),
            ),
        )
        .build();

    // When & Then
    assert_throws_malformed(|| {
        ColorPool::from_only_shard_for_testing(Arc::new(type_pool), StringPool::empty())
    });
}

// port: ColorPoolTest#uuid_mustBeSet
#[test]
fn uuid_must_be_set() {
    // Given
    let type_pool = single_object_pool(ObjectTypeProto::new_builder());

    // When & Then
    assert_throws_malformed(|| {
        let mut builder = ColorPool::builder();
        builder.add_shard_and(type_pool, StringPool::empty());
        builder.build()
    });
}

// port: ColorPoolTest#uuid_mustNotBeAxiomatic
#[test]
fn uuid_must_not_be_axiomatic() {
    for id in standard_colors::AXIOMATIC_COLORS.keys() {
        // Given
        let type_pool =
            single_object_pool(ObjectTypeProto::new_builder().set_uuid(id.as_byte_string()));

        // When & Then
        assert_throws_malformed(|| {
            let mut builder = ColorPool::builder();
            builder.add_shard_and(type_pool, StringPool::empty());
            builder.build()
        });
    }
}

// port: ColorPoolTest#reconcile_defaultValues
#[test]
fn reconcile_default_values() {
    // Given
    let type_pool =
        single_object_pool(ObjectTypeProto::new_builder().set_uuid(TEST_ID.as_byte_string()));

    // When
    let mut builder = ColorPool::builder();
    builder.add_shard_and(type_pool, StringPool::empty());
    let color_pool = builder.build();

    // Then
    let default_color = color_pool.get_color(TEST_ID).unwrap();
    assert!(default_color.get_instance_colors().is_empty());
    assert!(default_color.get_prototypes().is_empty());
    assert!(default_color.get_own_properties().is_empty());
    assert!(!default_color.is_closure_assert());
    assert!(!default_color.is_constructor());
    assert!(!default_color.is_invalidating());
    assert!(!default_color.get_properties_keep_original_name());
}

fn build_two(
    type_pool0: Arc<TypePool>,
    string_pool0: Arc<StringPool>,
    type_pool1: Arc<TypePool>,
    string_pool1: Arc<StringPool>,
) -> Arc<ColorPool> {
    let mut builder = ColorPool::builder();
    builder
        .add_shard_and(type_pool0, string_pool0)
        .add_shard_and(type_pool1, string_pool1);
    builder.build()
}

// port: ColorPoolTest#reconcile_setDisambiguationSupertypes
#[test]
fn reconcile_set_disambiguation_supertypes() {
    // Given
    let type_pool0 = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(TEST_ID.as_byte_string())
                    .build(),
            ),
        )
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(0))
                .set_supertype(PrimitiveType::NUMBER_TYPE.get_number())
                .build(),
        )
        .build();

    let type_pool1 = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(TEST_ID.as_byte_string())
                    .build(),
            ),
        )
        .add_disambiguation_edges(
            SubtypingEdge::new_builder()
                .set_subtype(pool_pointer(0))
                .set_supertype(PrimitiveType::STRING_TYPE.get_number())
                .build(),
        )
        .build();

    // When
    let color_pool = build_two(
        Arc::new(type_pool0),
        StringPool::empty(),
        Arc::new(type_pool1),
        StringPool::empty(),
    );

    // Then
    assert_contains_exactly(
        &supertypes(
            color_pool.get_registry(),
            &color_pool.get_color(TEST_ID).unwrap(),
        ),
        &[&standard_colors::NUMBER, &standard_colors::STRING],
    );
}

// port: ColorPoolTest#reconcile_setInstanceColors
#[test]
fn reconcile_set_instance_colors() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .add_instance_type(PrimitiveType::NUMBER_TYPE.get_number()),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .add_instance_type(PrimitiveType::STRING_TYPE.get_number()),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        StringPool::empty(),
        type_pool1,
        StringPool::empty(),
    );

    // Then
    assert_contains_exactly(
        color_pool.get_color(TEST_ID).unwrap().get_instance_colors(),
        &[&standard_colors::NUMBER, &standard_colors::STRING],
    );
}

// port: ColorPoolTest#reconcile_addPrototypes
#[test]
fn reconcile_add_prototypes() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .add_prototype(PrimitiveType::NUMBER_TYPE.get_number()),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .add_prototype(PrimitiveType::STRING_TYPE.get_number()),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        StringPool::empty(),
        type_pool1,
        StringPool::empty(),
    );

    // Then
    assert_contains_exactly(
        color_pool.get_color(TEST_ID).unwrap().get_prototypes(),
        &[&standard_colors::NUMBER, &standard_colors::STRING],
    );
}

// port: ColorPoolTest#reconcile_setOwnProperties
#[test]
fn reconcile_set_own_properties() {
    // Given
    let string_pool0 = StringPool::builder().put_and("hello").build();
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .add_own_property(1),
    );

    let string_pool1 = StringPool::builder().put_and("world").build();
    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .add_own_property(1),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        Arc::new(string_pool0),
        type_pool1,
        Arc::new(string_pool1),
    );

    // Then
    let expected: IndexSet<JsString> = [JsString::from("hello"), JsString::from("world")]
        .into_iter()
        .collect();
    assert_eq!(
        color_pool.get_color(TEST_ID).unwrap().get_own_properties(),
        &expected
    );
}

// port: ColorPoolTest#reconcile_setClosureAssert_true
#[test]
fn reconcile_set_closure_assert_true() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_closure_assert(true),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_closure_assert(true),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        StringPool::empty(),
        type_pool1,
        StringPool::empty(),
    );

    // Then
    assert!(color_pool.get_color(TEST_ID).unwrap().is_closure_assert());
}

// port: ColorPoolTest#reconcile_setClosureAssert_false
#[test]
fn reconcile_set_closure_assert_false() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_closure_assert(false),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_closure_assert(false),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        StringPool::empty(),
        type_pool1,
        StringPool::empty(),
    );

    // Then
    assert!(!color_pool.get_color(TEST_ID).unwrap().is_closure_assert());
}

// port: ColorPoolTest#reconcile_setClosureAssert_mixed
#[test]
fn reconcile_set_closure_assert_mixed() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_closure_assert(true),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_closure_assert(false),
    );

    // When & Then
    assert_throws_malformed(|| {
        build_two(
            type_pool0,
            StringPool::empty(),
            type_pool1,
            StringPool::empty(),
        )
    });
}

// port: ColorPoolTest#reconcile_setConstructor
#[test]
fn reconcile_set_constructor() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_marked_constructor(false),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_marked_constructor(true),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        StringPool::empty(),
        type_pool1,
        StringPool::empty(),
    );

    // Then
    assert!(color_pool.get_color(TEST_ID).unwrap().is_constructor());
}

// port: ColorPoolTest#reconcile_setInvalidating
#[test]
fn reconcile_set_invalidating() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_is_invalidating(false),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_is_invalidating(true),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        StringPool::empty(),
        type_pool1,
        StringPool::empty(),
    );

    // Then
    assert!(color_pool.get_color(TEST_ID).unwrap().is_invalidating());
}

// port: ColorPoolTest#reconcile_setPropertiesKeepOriginalName
#[test]
fn reconcile_set_properties_keep_original_name() {
    // Given
    let type_pool0 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_properties_keep_original_name(false),
    );

    let type_pool1 = single_object_pool(
        ObjectTypeProto::new_builder()
            .set_uuid(TEST_ID.as_byte_string())
            .set_properties_keep_original_name(true),
    );

    // When
    let color_pool = build_two(
        type_pool0,
        StringPool::empty(),
        type_pool1,
        StringPool::empty(),
    );

    // Then
    assert!(
        color_pool
            .get_color(TEST_ID)
            .unwrap()
            .get_properties_keep_original_name()
    );
}

// port: ColorPoolTest#reconcile_debugInfo_mismatches
#[test]
fn reconcile_debug_info_mismatches() {
    // Given
    let type_pool0 = TypePool::new_builder()
        .set_debug_info(
            TypePoolDebugInfo::new_builder()
                .add_mismatch(
                    TypePoolDebugInfoMismatch::new_builder()
                        .set_source_ref("location_0")
                        .add_involved_color(PrimitiveType::SYMBOL_TYPE.get_number())
                        .add_involved_color(PrimitiveType::STRING_TYPE.get_number())
                        .build(),
                )
                .add_mismatch(
                    TypePoolDebugInfoMismatch::new_builder()
                        .set_source_ref("location_1")
                        .add_involved_color(PrimitiveType::NUMBER_TYPE.get_number())
                        .build(),
                ),
        )
        .build();

    let type_pool1 = TypePool::new_builder()
        .set_debug_info(
            TypePoolDebugInfo::new_builder()
                .add_mismatch(
                    TypePoolDebugInfoMismatch::new_builder()
                        .set_source_ref("location_0")
                        .add_involved_color(PrimitiveType::SYMBOL_TYPE.get_number())
                        .add_involved_color(PrimitiveType::BOOLEAN_TYPE.get_number())
                        .build(),
                )
                .add_mismatch(
                    TypePoolDebugInfoMismatch::new_builder()
                        .set_source_ref("location_2")
                        .add_involved_color(PrimitiveType::BIGINT_TYPE.get_number())
                        .build(),
                ),
        )
        .build();

    // When
    let color_pool = build_two(
        Arc::new(type_pool0),
        StringPool::empty(),
        Arc::new(type_pool1),
        StringPool::empty(),
    );

    // Then
    let mut expected: IndexMap<ColorId, IndexSet<String>> = IndexMap::<_, _>::default();
    for (id, location) in [
        (standard_colors::BOOLEAN.get_id(), "location_0"),
        (standard_colors::STRING.get_id(), "location_0"),
        (standard_colors::SYMBOL.get_id(), "location_0"),
        (standard_colors::NUMBER.get_id(), "location_1"),
        (standard_colors::BIGINT.get_id(), "location_2"),
    ] {
        expected.entry(id).or_default().insert(location.to_string());
    }
    assert_eq!(
        color_pool
            .get_registry()
            .get_mismatch_locations_for_debugging(),
        &expected
    );
}

// port: ColorPoolTest#reconcile_shards_preserveOriginalOffsets
#[test]
fn reconcile_shards_preserve_original_offsets() {
    // Given
    let other_id = ColorId::from_ascii("other");

    let type_pool0 = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(other_id.as_byte_string())
                    .build(),
            ),
        )
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(TEST_ID.as_byte_string())
                    .build(),
            ),
        )
        .build();

    let type_pool1 = TypePool::new_builder()
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(TEST_ID.as_byte_string())
                    .build(),
            ),
        )
        .add_type(
            TypeProto::new_builder().set_object(
                ObjectTypeProto::new_builder()
                    .set_uuid(other_id.as_byte_string())
                    .build(),
            ),
        )
        .build();

    // When
    let mut color_pool_builder = ColorPool::builder();
    let shard0 = color_pool_builder.add_shard(Arc::new(type_pool0), StringPool::empty());
    let shard1 = color_pool_builder.add_shard(Arc::new(type_pool1), StringPool::empty());
    color_pool_builder.build();

    // Then
    assert_eq!(shard0.get_color(pool_pointer(1)).get_id(), TEST_ID);
    assert!(
        shard0
            .get_color(pool_pointer(1))
            .ptr_eq(&shard1.get_color(pool_pointer(0)))
    );
}
