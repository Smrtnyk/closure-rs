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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit b2f69b3:
//   test/com/google/javascript/jscomp/PolyfillUsageFinderTest.java.

//! Port of PolyfillUsageFinderTest.java: unit tests for PolyfillUsageFinder.
use closure_jscomp::{
    Compiler,
    colors::{Color, ColorId, ColorRegistry, standard_colors},
    compiler_options::CompilerOptions,
    polyfill_usage_finder::{Kind, Polyfill, PolyfillUsage, PolyfillUsageFinder, Polyfills},
    source_file::SourceFile,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    ir::IR,
    node::{Ast, NodeId},
    token::Token,
};
use closure_testing::testing::test_externs_builder::TestExternsBuilder;
use std::sync::Arc;

/// Covers testing of most common cases.
///
/// <ul>
///   <li>guarded & unguarded references
///   <li>global symbols & prototype methods
///   <li>non-polyfill references
///   <li>references via a named global object
/// </ul>
// port: PolyfillUsageFinderTest#basicCoverageTest
#[test]
fn basic_coverage_test() {
    let mut test_polyfill_usage_finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines(
            "Array.prototype.fill es6 es3 es6/array/fill\n\
             Array.from es6 es3 es6/array/from\n\
             Map es6 es3 es6/map\n",
        )
        .for_source_lines(
            "use(new Map()); // 1\n\
             globalThis.Array.from // 2\n\
             \x20 ? globalThis.Array.from([]) // 3\n\
             \x20 : []; // 4\n\
             [1, 2, 3].fill(0); // 5\n\
             notAPolyfill(); // 6\n",
        )
        .build();

    let unguarded_polyfill_usages = test_polyfill_usage_finder.get_unguarded_usages();

    // Map() and [].fill() references
    assert_eq!(unguarded_polyfill_usages.len(), 2);
    let ast: &Ast = &test_polyfill_usage_finder.compiler;

    // Source line 1: `use(new Map())`
    let map_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &unguarded_polyfill_usages[0]);
    map_usage_subject
        .has_node_that()
        .is_name("Map")
        .has_lineno(1);
    map_usage_subject.has_name("Map").is_not_explicit_global();
    map_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Map")
        .has_native_version_str("es6")
        .has_polyfill_version_str("es3")
        .has_library("es6/map")
        .has_kind(Kind::STATIC);

    // Source line 5: `[1, 2, 3].fill(0);`
    let array_fill_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &unguarded_polyfill_usages[1]);
    array_fill_usage_subject
        .has_node_that()
        .is_get_prop()
        .has_lineno(5);
    array_fill_usage_subject
        .has_name("fill")
        .is_not_explicit_global();
    array_fill_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Array.prototype.fill")
        .has_native_version_str("es6")
        .has_polyfill_version_str("es3")
        .has_library("es6/array/fill")
        .has_kind(Kind::METHOD);

    // The 2 references to `Array.from` on line 2 and 3
    // Note that the guard expression itself counts as a guarded use.
    // Source line 2: `globalThis.Array.from ? globalThis.Array.from([]) : [];`
    let guarded_polyfill_usages = test_polyfill_usage_finder.get_guarded_usages();
    assert_eq!(guarded_polyfill_usages.len(), 2);
    let ast: &Ast = &test_polyfill_usage_finder.compiler;

    let hook_condition_use_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &guarded_polyfill_usages[0]);
    hook_condition_use_subject
        .has_node_that()
        .is_get_prop()
        .has_lineno(2);

    let array_dot_from_use_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &guarded_polyfill_usages[1]);
    array_dot_from_use_subject
        .has_node_that()
        .is_get_prop()
        .has_lineno(3);

    // Other than the node they reference, both usage objects are the same
    for array_dot_from_usage_subject in [hook_condition_use_subject, array_dot_from_use_subject] {
        array_dot_from_usage_subject.has_name("Array.from");
        // reference via `globalThis` makes these explicit global references
        array_dot_from_usage_subject.is_explicit_global();
        array_dot_from_usage_subject
            .has_polyfill_that()
            .has_native_symbol("Array.from")
            .has_native_version_str("es6")
            .has_polyfill_version_str("es3")
            .has_library("es6/array/from")
            .has_kind(Kind::STATIC);
    }
}

// port: PolyfillUsageFinderTest#shadowedPolyfillTest
#[test]
fn shadowed_polyfill_test() {
    let mut test_polyfill_usage_finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines(
            "Promise es6 es3 es6/promise/promise\n\
             Promise.allSettled es_2020 es3 es6/promise/allsettled\n",
        )
        .for_source_lines(
            "// usage of Promise and of Promise.allSettled\n\
             Promise.allSettled([])(result => console.log(x));\n\
             function foo() {\n\
             \x20 // These aren't references to Promise or Promise.allSettled because of the\n\
             \x20 // shadowing variable\n\
             \x20 let Promise = { allSettled: 'I solemnly swear that I am up to no good.' };\n\
             \x20 console.log(Promise.allSettled);\n\
             }\n",
        )
        .build();

    let all_usages = test_polyfill_usage_finder.get_all_usages();
    assert_eq!(all_usages.len(), 2);
    let ast: &Ast = &test_polyfill_usage_finder.compiler;

    let promise_usage_subject = PolyfillUsageSubject::assert_polyfill_usage(ast, &all_usages[0]);
    promise_usage_subject //
        .has_node_that()
        .is_name("Promise")
        .has_lineno(2);
    promise_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Promise")
        .has_native_version_str("es6")
        .has_polyfill_version_str("es3")
        .has_library("es6/promise/promise")
        .has_kind(Kind::STATIC);

    let all_settled_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &all_usages[1]);

    all_settled_usage_subject //
        .has_node_that()
        .matches_qualified_name("Promise.allSettled")
        .has_lineno(2);
    all_settled_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Promise.allSettled")
        .has_native_version_str("es_2020")
        .has_polyfill_version_str("es3")
        .has_library("es6/promise/allsettled")
        .has_kind(Kind::STATIC);
}

// port: PolyfillUsageFinderTest#optChainCoverageTest
#[test]
fn opt_chain_coverage_test() {
    let mut test_polyfill_usage_finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines("Array.from es6 es3 es6/array/from")
        .for_source_lines(
            "// Checks for the existence of `Array`, but not `.from`, so not guarded\n\
             Array?.from([]) ?? [];\n\
             // Correctly guarded on existence of `Array.from`\n\
             Array.from?.([]) ?? [];\n",
        )
        .build();

    // First usage is unguarded
    let unguarded_polyfill_usages = test_polyfill_usage_finder.get_unguarded_usages();
    assert_eq!(unguarded_polyfill_usages.len(), 1);
    // Second usage is guarded
    let guarded_polyfill_usages = test_polyfill_usage_finder.get_guarded_usages();
    let ast: &Ast = &test_polyfill_usage_finder.compiler;
    let unguarded_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &unguarded_polyfill_usages[0]);
    unguarded_usage_subject
        .has_node_that()
        .has_token(Token::OPTCHAIN_GETPROP)
        .has_lineno(2);

    assert_eq!(guarded_polyfill_usages.len(), 1);
    let guarded_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &guarded_polyfill_usages[0]);
    guarded_usage_subject
        .has_node_that()
        .has_token(Token::GETPROP)
        .has_lineno(4);

    // Other than the node they reference, both usage objects are the same
    for array_dot_from_usage_subject in [unguarded_usage_subject, guarded_usage_subject] {
        array_dot_from_usage_subject.has_name("Array.from");
        // reference via `globalThis` makes these explicit global references
        array_dot_from_usage_subject.is_not_explicit_global();
        array_dot_from_usage_subject
            .has_polyfill_that()
            .has_native_symbol("Array.from")
            .has_native_version_str("es6")
            .has_polyfill_version_str("es3")
            .has_library("es6/array/from")
            .has_kind(Kind::STATIC);
    }
}

// port: PolyfillUsageFinderTest#createFinderWithRegistry
fn create_finder_with_registry(registry: ColorRegistry) -> PolyfillUsageFinder {
    let mut compiler = new_test_compiler();
    set_test_color_registry(&mut compiler, registry);
    PolyfillUsageFinder::new(
        &compiler,
        Arc::new(Polyfills::from_table(
            "Iterator.prototype.toArray es_next es3 es6/iterator/toarray",
        )),
        /* disambiguate_properties= */ true,
    )
}

/// `ColorRegistry.builder().setDefaultNativeColorsForTesting().build()`.
fn default_native_registry() -> ColorRegistry {
    ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .build()
}

/// `Color.singleBuilder().setId(id).build()`.
fn single(id: ColorId) -> Color {
    Color::single_builder().set_id(id).build()
}

/// `Color.createUnion(ImmutableSet.of(..))`.
fn union(elements: &[&Color]) -> Color {
    Color::create_union(
        &elements
            .iter()
            .map(|c| (*c).clone())
            .collect::<IndexSet<Color>>(),
    )
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_unknownAndTopObject_returnsFalse
#[test]
fn is_disjoint_from_iterator_unknown_and_top_object_returns_false() {
    let registry = default_native_registry();
    let finder = create_finder_with_registry(registry);

    assert!(!finder.is_disjoint_from_iterator_color(&standard_colors::UNKNOWN));
    assert!(!finder.is_disjoint_from_iterator_color(&standard_colors::TOP_OBJECT));
    assert!(!finder.is_disjoint_from_iterator_color(&standard_colors::TOP_FUNCTION));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_sameColor_returnsFalse
#[test]
fn is_disjoint_from_iterator_same_color_returns_false() {
    let registry = default_native_registry();
    let iterator = registry.get(standard_colors::ITERATOR_ID);
    let finder = create_finder_with_registry(registry);

    assert!(!finder.is_disjoint_from_iterator_color(&iterator));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_unrelatedColors_returnsTrue
#[test]
fn is_disjoint_from_iterator_unrelated_colors_returns_true() {
    let registry = default_native_registry();
    let array = registry.get(standard_colors::ARRAY_ID);
    let finder = create_finder_with_registry(registry);
    let custom_foo = single(ColorId::from_ascii("Foo"));

    assert!(finder.is_disjoint_from_iterator_color(&array));
    assert!(finder.is_disjoint_from_iterator_color(&custom_foo));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_generatorAndIterator_returnsFalse
#[test]
fn is_disjoint_from_iterator_generator_and_iterator_returns_false() {
    let registry = default_native_registry();
    let generator = registry.get(standard_colors::GENERATOR_ID);
    let finder = create_finder_with_registry(registry);

    // Generator is a sibling of Iterator in externs, but inherits from %IteratorPrototype%
    // at runtime in ES2025, so Generator and Iterator are known not to be disjoint.
    assert!(!finder.is_disjoint_from_iterator_color(&generator));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_subtypesAndSupertypes_returnsFalse
#[test]
fn is_disjoint_from_iterator_subtypes_and_supertypes_returns_false() {
    let iterator = single(standard_colors::ITERATOR_ID);
    let sub_iterator = single(ColorId::from_ascii("SubIter"));
    let sub_sub_iterator = single(ColorId::from_ascii("SubSub"));
    let super_iterable = single(ColorId::from_ascii("SuperIt"));
    let unrelated = single(ColorId::from_ascii("Unrel"));

    let registry = ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .add_disambiguation_edge(sub_iterator.clone(), iterator.clone())
        .add_disambiguation_edge(sub_sub_iterator.clone(), sub_iterator.clone())
        .add_disambiguation_edge(iterator.clone(), super_iterable.clone())
        .build();
    let finder = create_finder_with_registry(registry);

    // Direct subtype: SubIterator extends Iterator
    assert!(!finder.is_disjoint_from_iterator_color(&sub_iterator));

    // Indirect subtype: SubSubIterator extends SubIterator extends Iterator
    assert!(!finder.is_disjoint_from_iterator_color(&sub_sub_iterator));

    // Supertype: Iterator extends SuperIterable (receiver is supertype)
    assert!(!finder.is_disjoint_from_iterator_color(&super_iterable));

    // Unrelated type with no edges
    assert!(finder.is_disjoint_from_iterator_color(&unrelated));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_branchingAndSiblingHierarchies
#[test]
fn is_disjoint_from_iterator_branching_and_sibling_hierarchies() {
    let iterator = single(standard_colors::ITERATOR_ID);
    let sub_iterator_a = single(ColorId::from_ascii("SubItA"));
    let sub_iterator_b = single(ColorId::from_ascii("SubItB"));
    let base_class = single(ColorId::from_ascii("Base"));
    let child_class = single(ColorId::from_ascii("Child"));

    let registry = ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .add_disambiguation_edge(sub_iterator_a.clone(), iterator.clone())
        .add_disambiguation_edge(sub_iterator_b.clone(), iterator.clone())
        .add_disambiguation_edge(child_class.clone(), base_class.clone())
        .build();
    let finder = create_finder_with_registry(registry);

    // Siblings under Iterator are both not disjoint from Iterator
    assert!(!finder.is_disjoint_from_iterator_color(&sub_iterator_a));
    assert!(!finder.is_disjoint_from_iterator_color(&sub_iterator_b));

    // Separate class hierarchy is disjoint from Iterator
    assert!(finder.is_disjoint_from_iterator_color(&child_class));
    assert!(finder.is_disjoint_from_iterator_color(&base_class));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_cyclesInSupertypeGraph_terminatesGracefully
#[test]
fn is_disjoint_from_iterator_cycles_in_supertype_graph_terminates_gracefully() {
    let iterator = single(standard_colors::ITERATOR_ID);
    let class_a = single(ColorId::from_ascii("ClassA"));
    let class_b = single(ColorId::from_ascii("ClassB"));

    // Cycle A <-> B with no connection to Iterator
    let registry_with_cycle = ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .add_disambiguation_edge(class_a.clone(), class_b.clone())
        .add_disambiguation_edge(class_b.clone(), class_a.clone())
        .build();
    let finder_with_cycle = create_finder_with_registry(registry_with_cycle);

    assert!(finder_with_cycle.is_disjoint_from_iterator_color(&class_a));
    assert!(finder_with_cycle.is_disjoint_from_iterator_color(&class_b));

    // Cycle A <-> B where B also extends Iterator
    let registry_with_cycle_and_iterator = ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .add_disambiguation_edge(class_a.clone(), class_b.clone())
        .add_disambiguation_edge(class_b.clone(), class_a.clone())
        .add_disambiguation_edge(class_b.clone(), iterator.clone())
        .build();
    let finder_with_cycle_and_iterator =
        create_finder_with_registry(registry_with_cycle_and_iterator);

    assert!(!finder_with_cycle_and_iterator.is_disjoint_from_iterator_color(&class_a));
    assert!(!finder_with_cycle_and_iterator.is_disjoint_from_iterator_color(&class_b));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_unionSupertypesInHierarchy
#[test]
fn is_disjoint_from_iterator_union_supertypes_in_hierarchy() {
    let iterator = single(standard_colors::ITERATOR_ID);
    let class_foo = single(ColorId::from_ascii("Foo"));
    let class_bar = single(ColorId::from_ascii("Bar"));
    let class_sub1 = single(ColorId::from_ascii("Sub1"));
    let class_sub2 = single(ColorId::from_ascii("Sub2"));

    let union_with_iterator = union(&[&class_foo, &iterator]);
    let union_without_iterator = union(&[&class_foo, &class_bar]);

    let registry = ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .add_disambiguation_edge(class_sub1.clone(), union_with_iterator)
        .add_disambiguation_edge(class_sub2.clone(), union_without_iterator)
        .build();
    let finder = create_finder_with_registry(registry);

    // Sub1 has a union supertype that includes Iterator
    assert!(!finder.is_disjoint_from_iterator_color(&class_sub1));

    // Sub2 has a union supertype that only includes disjoint types
    assert!(finder.is_disjoint_from_iterator_color(&class_sub2));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_primitives_returnsTrue
#[test]
fn is_disjoint_from_iterator_primitives_returns_true() {
    let registry = default_native_registry();
    let finder = create_finder_with_registry(registry);

    // Primitives are disjoint from Iterator
    assert!(finder.is_disjoint_from_iterator_color(&standard_colors::STRING));
    assert!(finder.is_disjoint_from_iterator_color(&standard_colors::NUMBER));
    assert!(finder.is_disjoint_from_iterator_color(&standard_colors::BOOLEAN));
    assert!(finder.is_disjoint_from_iterator_color(&standard_colors::BIGINT));
    assert!(finder.is_disjoint_from_iterator_color(&standard_colors::SYMBOL));
    assert!(finder.is_disjoint_from_iterator_color(&standard_colors::NULL_OR_VOID));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_nullAndUndefinedUnions
#[test]
fn is_disjoint_from_iterator_null_and_undefined_unions() {
    let registry = default_native_registry();
    let iterator = registry.get(standard_colors::ITERATOR_ID);
    let array = registry.get(standard_colors::ARRAY_ID);
    let finder = create_finder_with_registry(registry);

    let nullable_array = union(&[&array, &standard_colors::NULL_OR_VOID]);
    let nullable_iterator = union(&[&iterator, &standard_colors::NULL_OR_VOID]);

    // Nullable/undef Array is still disjoint from Iterator after subtraction
    assert!(finder.is_disjoint_from_iterator_color(&nullable_array));

    // Nullable/undef Iterator is not disjoint from Iterator
    assert!(!finder.is_disjoint_from_iterator_color(&nullable_iterator));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_unionReceivers
#[test]
fn is_disjoint_from_iterator_union_receivers() {
    let iterator = single(standard_colors::ITERATOR_ID);
    let sub_iterator = single(ColorId::from_ascii("SubIter"));
    let class_foo = single(ColorId::from_ascii("Foo"));
    let class_bar = single(ColorId::from_ascii("Bar"));

    let registry = ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .add_disambiguation_edge(sub_iterator.clone(), iterator.clone())
        .build();
    let finder = create_finder_with_registry(registry);

    // All elements disjoint -> disjoint
    let union_all_disjoint = union(&[&class_foo, &class_bar]);
    assert!(finder.is_disjoint_from_iterator_color(&union_all_disjoint));

    // One element is Iterator -> not disjoint
    let union_with_iterator = union(&[&class_foo, &iterator]);
    assert!(!finder.is_disjoint_from_iterator_color(&union_with_iterator));

    // One element is SubIterator -> not disjoint
    let union_with_sub_iterator = union(&[&class_foo, &sub_iterator]);
    assert!(!finder.is_disjoint_from_iterator_color(&union_with_sub_iterator));

    // One element is UNKNOWN -> not disjoint
    let union_with_unknown = union(&[&class_foo, &standard_colors::UNKNOWN]);
    assert!(!finder.is_disjoint_from_iterator_color(&union_with_unknown));

    // One element is TOP_OBJECT -> not disjoint
    let union_with_top_object = union(&[&class_foo, &standard_colors::TOP_OBJECT]);
    assert!(!finder.is_disjoint_from_iterator_color(&union_with_top_object));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_invalidatingColors_structuralRecordsVsNativeArrays
#[test]
fn is_disjoint_from_iterator_invalidating_colors_structural_records_vs_native_arrays() {
    let registry = default_native_registry();
    let finder = create_finder_with_registry(registry);

    let invalidating = |id: ColorId, invalidating: bool| {
        Color::single_builder()
            .set_id(id)
            .set_invalidating(invalidating)
            .build()
    };
    let structural_record = invalidating(ColorId::from_ascii("Struct"), true);
    let normal_class = invalidating(ColorId::from_ascii("Normal"), false);

    let invalidating_array = invalidating(standard_colors::ARRAY_ID, true);
    let invalidating_readonly_array = invalidating(standard_colors::READONLY_ARRAY_ID, true);
    let invalidating_template_array = invalidating(standard_colors::I_TEMPLATE_ARRAY_ID, true);

    let invalidating_map = invalidating(standard_colors::MAP_ID, true);
    let invalidating_map_prototype = invalidating(standard_colors::MAP_PROTOTYPE_ID, true);
    let invalidating_readonly_map = invalidating(standard_colors::READONLY_MAP_ID, true);
    let invalidating_set = invalidating(standard_colors::SET_ID, true);
    let invalidating_set_prototype = invalidating(standard_colors::SET_PROTOTYPE_ID, true);
    let invalidating_readonly_set = invalidating(standard_colors::READONLY_SET_ID, true);

    // Structural interface / record: isInvalidating causes disjoint to return false (b/571244650)
    assert!(!finder.is_disjoint_from_iterator_color(&structural_record));

    // Normal non-invalidating class: is disjoint
    assert!(finder.is_disjoint_from_iterator_color(&normal_class));

    // Native Array/Map/Set types marked invalidating are exempted
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_array));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_readonly_array));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_template_array));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_map));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_map_prototype));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_readonly_map));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_set));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_set_prototype));
    assert!(finder.is_disjoint_from_iterator_color(&invalidating_readonly_set));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_nodeAndOwnerChecks
#[test]
fn is_disjoint_from_iterator_node_and_owner_checks() {
    let registry = default_native_registry();
    let array = registry.get(standard_colors::ARRAY_ID);
    let iterator = registry.get(standard_colors::ITERATOR_ID);

    let finder_with_registry = create_finder_with_registry(registry);

    let compiler_without_registry = new_test_compiler();
    let finder_without_registry = PolyfillUsageFinder::new(
        &compiler_without_registry,
        Arc::new(Polyfills::from_table(
            "Iterator.prototype.toArray es_next es3 es6/iterator/toarray",
        )),
        /* disambiguate_properties= */ true,
    );

    let mut ast = Ast::new();
    let receiver_node = IR::name(&mut ast, "x");
    let get_prop_node = IR::getprop(&mut ast, receiver_node, "toArray");

    // Receiver has Array color, owner is Iterator -> disjoint
    receiver_node.set_color(&mut ast, Some(array.clone()));
    assert!(finder_with_registry.is_disjoint_from_iterator(&ast, get_prop_node, Some("Iterator")));

    // Receiver has Iterator color, owner is Iterator -> not disjoint
    receiver_node.set_color(&mut ast, Some(iterator));
    assert!(!finder_with_registry.is_disjoint_from_iterator(&ast, get_prop_node, Some("Iterator")));

    // Receiver has null color -> not disjoint
    receiver_node.set_color(&mut ast, None);
    assert!(!finder_with_registry.is_disjoint_from_iterator(&ast, get_prop_node, Some("Iterator")));

    // Owner is null or not Iterator -> not disjoint
    receiver_node.set_color(&mut ast, Some(array.clone()));
    assert!(!finder_with_registry.is_disjoint_from_iterator(&ast, get_prop_node, None));
    assert!(!finder_with_registry.is_disjoint_from_iterator(&ast, get_prop_node, Some("Array")));
    assert!(!finder_with_registry.is_disjoint_from_iterator(
        &ast,
        get_prop_node,
        Some("UnknownType")
    ));

    // Compiler without registry -> always not disjoint
    receiver_node.set_color(&mut ast, Some(array));
    assert!(!finder_without_registry.is_disjoint_from_iterator(
        &ast,
        get_prop_node,
        Some("Iterator")
    ));
}

// port: PolyfillUsageFinderTest#isDisjointFromIterator_receiverWithNullColor_returnsFalse
#[test]
fn is_disjoint_from_iterator_receiver_with_null_color_returns_false() {
    let registry = default_native_registry();
    let finder = create_finder_with_registry(registry);

    let mut ast = Ast::new();
    let receiver_node = IR::name(&mut ast, "x");
    assert!(receiver_node.get_color(&ast).is_none());
    let get_prop_node = IR::getprop(&mut ast, receiver_node, "toArray");

    assert!(!finder.is_disjoint_from_iterator(&ast, get_prop_node, Some("Iterator")));
}

// port: PolyfillUsageFinderTest#traverse_withColorRegistry_filtersDisjointUsages
#[test]
fn traverse_with_color_registry_filters_disjoint_usages() {
    let registry = default_native_registry();
    let array = registry.get(standard_colors::ARRAY_ID);
    let iterator = registry.get(standard_colors::ITERATOR_ID);

    let mut finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines(
            "Iterator.prototype.toArray es_next es3 es6/iterator/toarray\n\
             Array.prototype.fill es6 es3 es6/array/fill\n",
        )
        .with_disambiguate_properties(true)
        .for_source_lines(
            "arr.toArray(); // 1: receiver is Array, disjoint from Iterator -> filtered!\n\
             iter.toArray(); // 2: receiver is Iterator -> kept!\n\
             untyped.toArray(); // 3: untyped -> kept!\n\
             arr.fill(0); // 4: owner is Array (not Iterator) -> kept!\n",
        )
        .with_color_registry(registry)
        .build();

    // Annotate AST nodes with colors
    let root_node = finder.get_root_node();
    let arr_nodes = find_all_nodes(&finder.compiler, root_node, |ast, n| {
        n.is_name(ast) && n.get_string(ast) == "arr"
    });
    assert_eq!(arr_nodes.len(), 2);
    for arr_node in arr_nodes {
        arr_node.set_color(&mut finder.compiler, Some(array.clone()));
    }
    find_first_node(&finder.compiler, root_node, |ast, n| {
        n.is_name(ast) && n.get_string(ast) == "iter"
    })
    .expect("NullPointerException")
    .set_color(&mut finder.compiler, Some(iterator));

    let usages = finder.get_unguarded_usages();
    let ast: &Ast = &finder.compiler;

    // Should include: iter.toArray (line 2), untyped.toArray (line 3), arr.fill (line 4)
    // Should NOT include: arr.toArray (line 1)
    assert_eq!(usages.len(), 3);

    PolyfillUsageSubject::assert_polyfill_usage(ast, &usages[0])
        .has_name("toArray")
        .has_node_that()
        .has_lineno(2);

    PolyfillUsageSubject::assert_polyfill_usage(ast, &usages[1])
        .has_name("toArray")
        .has_node_that()
        .has_lineno(3);

    PolyfillUsageSubject::assert_polyfill_usage(ast, &usages[2])
        .has_name("fill")
        .has_node_that()
        .has_lineno(4);
}

// port: PolyfillUsageFinderTest#traverse_withColorRegistry_disambiguatePropertiesDisabled_doesNotFilterUsages
#[test]
fn traverse_with_color_registry_disambiguate_properties_disabled_does_not_filter_usages() {
    let registry = default_native_registry();
    let array = registry.get(standard_colors::ARRAY_ID);

    let mut finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines("Iterator.prototype.toArray es_next es3 es6/iterator/toarray\n")
        .with_disambiguate_properties(false)
        .for_source_lines("arr.toArray();\n")
        .with_color_registry(registry)
        .build();

    let root_node = finder.get_root_node();
    find_first_node(&finder.compiler, root_node, |ast, n| {
        n.is_name(ast) && n.get_string(ast) == "arr"
    })
    .expect("NullPointerException")
    .set_color(&mut finder.compiler, Some(array));

    let usages = finder.get_unguarded_usages();
    let ast: &Ast = &finder.compiler;
    assert_eq!(usages.len(), 1);
    PolyfillUsageSubject::assert_polyfill_usage(ast, &usages[0]).has_name("toArray");
}

// port: PolyfillUsageFinderTest#traverse_withoutColorRegistry_doesNotFilterUsages
#[test]
fn traverse_without_color_registry_does_not_filter_usages() {
    let registry = default_native_registry();
    let array = registry.get(standard_colors::ARRAY_ID);

    let mut finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines("Iterator.prototype.toArray es_next es3 es6/iterator/toarray\n")
        .for_source_lines("arr.toArray();\n")
        .build(); // No color registry

    // Even if arr has a color, without ColorRegistry on compiler it should not filter
    let root_node = finder.get_root_node();
    find_first_node(&finder.compiler, root_node, |ast, n| {
        n.is_name(ast) && n.get_string(ast) == "arr"
    })
    .expect("NullPointerException")
    .set_color(&mut finder.compiler, Some(array));

    let usages = finder.get_unguarded_usages();
    let ast: &Ast = &finder.compiler;
    assert_eq!(usages.len(), 1);
    PolyfillUsageSubject::assert_polyfill_usage(ast, &usages[0]).has_name("toArray");
}

// port: PolyfillUsageFinderTest#findFirstNode
fn find_first_node(
    ast: &Ast,
    root: NodeId,
    predicate: impl Fn(&Ast, NodeId) -> bool + Copy,
) -> Option<NodeId> {
    if predicate(ast, root) {
        return Some(root);
    }
    let mut child = root.get_first_child(ast);
    while let Some(c) = child {
        let found = find_first_node(ast, c, predicate);
        if found.is_some() {
            return found;
        }
        child = c.get_next(ast);
    }
    None
}

// port: PolyfillUsageFinderTest#findAllNodes
fn find_all_nodes(
    ast: &Ast,
    root: NodeId,
    predicate: impl Fn(&Ast, NodeId) -> bool + Copy,
) -> Vec<NodeId> {
    let mut builder = Vec::new();
    find_all_nodes_internal(ast, root, predicate, &mut builder);
    builder
}

// port: PolyfillUsageFinderTest#findAllNodesInternal
fn find_all_nodes_internal(
    ast: &Ast,
    root: NodeId,
    predicate: impl Fn(&Ast, NodeId) -> bool + Copy,
    builder: &mut Vec<NodeId>,
) {
    if predicate(ast, root) {
        builder.push(root);
    }
    let mut child = root.get_first_child(ast);
    while let Some(c) = child {
        find_all_nodes_internal(ast, c, predicate, builder);
        child = c.get_next(ast);
    }
}

/// Java's `new TestCompiler()`. Java's `TestCompiler` subclass of `Compiler` overrides
/// `hasOptimizationColors()` and `getColorRegistry()` to answer from a test registry; Rust's
/// `Compiler` can't be subclassed, so the test registry is stored in the compiler itself by
/// [`set_test_color_registry`], and a `TestCompiler` is a plain `Compiler`.
// port: PolyfillUsageFinderTest.TestCompiler#TestCompiler
fn new_test_compiler() -> Compiler {
    Compiler::new()
}

/// Stores `registry` as the compiler's color registry, so `hasOptimizationColors()` is true and
/// `getColorRegistry()` returns it. `Compiler::set_color_registry` requires the runtime library
/// typed ASTs, so they are initialized first (with default options if the compiler has none).
// port: PolyfillUsageFinderTest.TestCompiler#setTestColorRegistry
fn set_test_color_registry(compiler: &mut Compiler, registry: ColorRegistry) {
    if compiler.get_options_opt().is_none() {
        compiler.init_options(CompilerOptions::new());
    }
    compiler.init_runtime_library_typed_asts(None);
    compiler.set_color_registry(Arc::new(registry));
}

/// Uses {@link PolyfillUsageFinder} to find polyfill usages in source code for testing.
struct TestPolyfillUsageFinder {
    polyfill_usage_finder: PolyfillUsageFinder,
    root_node: NodeId,
    compiler: Compiler,
}

#[derive(Default)]
struct Builder {
    polyfills_table: Option<Polyfills>,
    compiler: Option<Compiler>,
    polyfill_usage_finder: Option<PolyfillUsageFinder>,
    root_node: Option<NodeId>,
    color_registry: Option<ColorRegistry>,
    disambiguate_properties: bool,
}

impl TestPolyfillUsageFinder {
    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#builder
    fn builder() -> Builder {
        Builder::default()
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#TestPolyfillUsageFinder
    fn new(builder: Builder) -> Self {
        Self {
            polyfill_usage_finder: builder.polyfill_usage_finder.unwrap(),
            root_node: builder.root_node.unwrap(),
            compiler: builder.compiler.unwrap(),
        }
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#getRootNode
    fn get_root_node(&self) -> NodeId {
        self.root_node
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#getUnguardedUsages
    fn get_unguarded_usages(&mut self) -> Vec<PolyfillUsage> {
        let mut consumer = PolyfillUsageCollectingConsumer::default();
        self.polyfill_usage_finder.traverse_excluding_guarded(
            &mut self.compiler,
            self.root_node,
            &mut |_compiler: &mut Compiler, usage| consumer.accept(usage),
        );
        consumer.get_usages()
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#getGuardedUsages
    fn get_guarded_usages(&mut self) -> Vec<PolyfillUsage> {
        let mut consumer = PolyfillUsageCollectingConsumer::default();
        self.polyfill_usage_finder.traverse_only_guarded(
            &mut self.compiler,
            self.root_node,
            &mut |_compiler: &mut Compiler, usage| consumer.accept(usage),
        );
        consumer.get_usages()
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#getAllUsages
    fn get_all_usages(&mut self) -> Vec<PolyfillUsage> {
        let mut consumer = PolyfillUsageCollectingConsumer::default();
        self.polyfill_usage_finder.traverse_including_guarded(
            &mut self.compiler,
            self.root_node,
            &mut |_compiler: &mut Compiler, usage| consumer.accept(usage),
        );
        consumer.get_usages()
    }
}

impl Builder {
    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#withPolyfillTableLines
    fn with_polyfill_table_lines(mut self, polyfill_table: &str) -> Self {
        self.polyfills_table = Some(Polyfills::from_table(polyfill_table));
        self
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#withColorRegistry
    fn with_color_registry(mut self, color_registry: ColorRegistry) -> Self {
        self.color_registry = Some(color_registry);
        self
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#withDisambiguateProperties
    fn with_disambiguate_properties(mut self, disambiguate_properties: bool) -> Self {
        self.disambiguate_properties = disambiguate_properties;
        self
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#forSourceLines
    fn for_source_lines(mut self, src: &str) -> Self {
        let src_file = SourceFile::builder()
            .with_path("src.js")
            .with_content(src)
            .build();
        let mut options = CompilerOptions::new();
        // Don't include `"use strict";` when printing the AST as source text
        options.set_emit_use_strict(false);
        let mut compiler = new_test_compiler();
        compiler.init(
            &[Arc::new(
                TestExternsBuilder::new().build_externs_file("externs.js"),
            )],
            &[Arc::new(src_file)],
            options,
        );
        compiler.parse();
        self.root_node = compiler.get_js_root();
        self.compiler = Some(compiler);
        self
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#build
    fn build(mut self) -> TestPolyfillUsageFinder {
        assert!(self.compiler.is_some());
        if let Some(color_registry) = self.color_registry.take() {
            set_test_color_registry(self.compiler.as_mut().unwrap(), color_registry);
        }
        self.polyfill_usage_finder = Some(PolyfillUsageFinder::new(
            self.compiler.as_ref().unwrap(),
            Arc::new(self.polyfills_table.take().unwrap()),
            self.disambiguate_properties,
        ));
        TestPolyfillUsageFinder::new(self)
    }
}

/// Consumes {@link PolyfillUsage} objects by storing them into a retrievable list.
#[derive(Default)]
struct PolyfillUsageCollectingConsumer {
    polyfill_usage_list_builder: Vec<PolyfillUsage>,
}

impl PolyfillUsageCollectingConsumer {
    // port: PolyfillUsageFinderTest.PolyfillUsageCollectingConsumer#accept
    fn accept(&mut self, polyfill_usage: PolyfillUsage) {
        self.polyfill_usage_list_builder.push(polyfill_usage);
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageCollectingConsumer#getUsages
    fn get_usages(self) -> Vec<PolyfillUsage> {
        self.polyfill_usage_list_builder
    }
}

struct PolyfillSubject<'a> {
    actual: &'a Polyfill,
}

impl<'a> PolyfillSubject<'a> {
    // port: PolyfillUsageFinderTest.PolyfillSubject#assertPolyfill
    fn assert_polyfill(polyfill: &'a Polyfill) -> Self {
        Self { actual: polyfill }
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasNativeSymbol
    fn has_native_symbol(&self, expected: &str) -> &Self {
        assert_eq!(self.actual.native_symbol, expected, "nativeSymbol");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasNativeVersionStr
    fn has_native_version_str(&self, expected: &str) -> &Self {
        assert_eq!(self.actual.native_version, expected, "nativeVersion");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasPolyfillVersionStr
    fn has_polyfill_version_str(&self, expected: &str) -> &Self {
        assert_eq!(self.actual.polyfill_version, expected, "polyfillVersion");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasLibrary
    fn has_library(&self, expected_library_path: &str) -> &Self {
        assert_eq!(self.actual.library, expected_library_path, "library");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasKind
    fn has_kind(&self, expected_kind: Kind) -> &Self {
        assert_eq!(self.actual.kind, expected_kind, "kind");
        self
    }
}

#[derive(Clone, Copy)]
struct PolyfillUsageSubject<'a> {
    ast: &'a Ast,
    actual: &'a PolyfillUsage,
}

impl<'a> PolyfillUsageSubject<'a> {
    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#assertPolyfillUsage
    fn assert_polyfill_usage(ast: &'a Ast, polyfill_usage: &'a PolyfillUsage) -> Self {
        Self {
            ast,
            actual: polyfill_usage,
        }
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#hasName
    fn has_name(&self, expected_name: &str) -> &Self {
        assert_eq!(*self.actual.name(), expected_name, "name()");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#isExplicitGlobal
    fn is_explicit_global(&self) -> &Self {
        assert!(self.actual.is_explicit_global(), "isExplicitGlobal()");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#isNotExplicitGlobal
    fn is_not_explicit_global(&self) -> &Self {
        assert!(!self.actual.is_explicit_global(), "isExplicitGlobal()");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#hasNodeThat
    fn has_node_that(&self) -> NodeAssertions<'a> {
        NodeAssertions {
            ast: self.ast,
            actual: self.actual.node(),
        }
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#hasPolyfillThat
    fn has_polyfill_that(&self) -> PolyfillSubject<'a> {
        PolyfillSubject::assert_polyfill(self.actual.polyfill())
    }
}

/// The NodeSubject assertions these tests use (rhino's Rust NodeSubject has no isName, isGetProp,
/// hasToken or matchesQualifiedName yet); each checks what Java's NodeSubject method checks.
struct NodeAssertions<'a> {
    ast: &'a Ast,
    actual: NodeId,
}

// The NodeSubject assertions of NodeAssertions, which name Closure's Rhino-derived NodeSubject
// (MPL-1.1 / GPL-2.0-or-later), are in their own file.
#[path = "rhino/polyfill_usage_finder_test.rs"]
mod rhino;
