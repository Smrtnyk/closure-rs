/*
 * Copyright 2019 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/disambiguate/ClusterPropagatorTest.java,
//   test/com/google/javascript/jscomp/disambiguate/ColorGraphBuilderTest.java,
//   test/com/google/javascript/jscomp/disambiguate/ColorGraphNodeFactoryTest.java,
//   test/com/google/javascript/jscomp/disambiguate/UseSiteRenamerTest.java.

//! Ports of the JUnit tests of `com.google.javascript.jscomp.disambiguate` that do not run
//! through `CompilerTestCase` (those run as corpus unit records): `ClusterPropagatorTest`,
//! `ColorGraphNodeFactoryTest`, `UseSiteRenamerTest`, and the one `ColorGraphBuilderTest` method
//! that has no corpus record.

mod cluster_propagator_test {
    use closure_jscomp::{
        colors::{Color, standard_colors},
        disambiguate::{
            cluster_propagator::ClusterPropagator,
            color_graph_node::{
                ColorGraphNode, ColorGraphNodeId, DisambiguateArena, PropAssociation,
                PropertyClusteringId,
            },
            invalidation::Invalidation,
            property_clustering::PropertyClustering,
        },
        graph::union_find::UnionFind,
    };
    use closure_rhino::fast_hash::IndexSet;
    use closure_rhino::js_string::JsString;

    fn test_color() -> Color {
        standard_colors::NUMBER.clone()
    }

    struct ClusterPropagatorTest {
        propagator: ClusterPropagator,
        arena: DisambiguateArena,
        src: ColorGraphNodeId,
        dest: ColorGraphNodeId,
        prop: PropertyClusteringId,
        result: bool,
    }

    impl ClusterPropagatorTest {
        fn new() -> Self {
            let mut arena = DisambiguateArena::new();
            let src = ColorGraphNode::create_for_testing_with_color(&mut arena, test_color(), -1);
            let dest = ColorGraphNode::create_for_testing_with_color(&mut arena, test_color(), -2);
            let prop = PropertyClustering::new(&mut arena, JsString::from("prop"));
            Self {
                propagator: ClusterPropagator::new(),
                arena,
                src,
                dest,
                prop,
                result: false,
            }
        }

        // port: ClusterPropagatorTest#verifyPropertyFlow
        fn verify_property_flow(&self) {
            let valid_src_props = self
                .src
                .get_associated_props(&self.arena)
                .keys()
                .copied()
                .filter(|p| !p.is_invalidated(&self.arena))
                .collect::<IndexSet<_>>();

            let dest_props = self.dest.get_associated_props(&self.arena);
            for p in &valid_src_props {
                assert!(dest_props.contains_key(p));
            }
        }

        // port: ClusterPropagatorTest#verifyClusterMerging
        fn verify_cluster_merging(&mut self) {
            if self.prop.is_invalidated(&self.arena) {
                return;
            }

            if self.are_associated(self.prop, self.src) && self.are_associated(self.prop, self.dest)
            {
                let (src, dest) = (self.src, self.dest);
                assert!(
                    self.prop
                        .get_clusters_mut(&mut self.arena)
                        .are_equivalent(&src, &dest)
                );
            }
        }

        /// JUnit runs both `@After` methods after each test.
        fn after(mut self) {
            self.verify_property_flow();
            self.verify_cluster_merging();
        }

        // port: ClusterPropagatorTest#areAssociated
        fn are_associated(&self, prop: PropertyClusteringId, flat: ColorGraphNodeId) -> bool {
            if prop.get_clusters(&self.arena).elements().contains(&flat) {
                assert!(flat.get_associated_props(&self.arena).contains_key(&prop));
                return true;
            }

            assert!(!flat.get_associated_props(&self.arena).contains_key(&prop));
            false
        }

        // port: ClusterPropagatorTest#associate
        fn associate(
            arena: &mut DisambiguateArena,
            prop: PropertyClusteringId,
            node: ColorGraphNodeId,
        ) {
            node.get_associated_props_mut(arena)
                .insert(prop, PropAssociation::AST);
            prop.get_clusters_mut(arena).add(node);
        }

        // port: ClusterPropagatorTest#propagateFromSrcToDest
        fn propagate_from_src_to_dest(&mut self) {
            self.result = self
                .propagator
                .traverse_edge(&mut self.arena, self.src, None, self.dest);
        }
    }

    // port: ClusterPropagatorTest#propagation_mergesProperties_fromSrcToDest
    #[test]
    fn propagation_merges_properties_from_src_to_dest() {
        let mut t = ClusterPropagatorTest::new();
        // Given
        ClusterPropagatorTest::associate(&mut t.arena, t.prop, t.src);

        // When
        t.propagate_from_src_to_dest();

        // Then
        assert!(t.result);
        t.after();
    }

    // port: ClusterPropagatorTest#propagation_mergesProperties_fromSrc_thatExistOnDest
    #[test]
    fn propagation_merges_properties_from_src_that_exist_on_dest() {
        let mut t = ClusterPropagatorTest::new();
        // Given
        ClusterPropagatorTest::associate(&mut t.arena, t.prop, t.src);
        ClusterPropagatorTest::associate(&mut t.arena, t.prop, t.dest);

        // When
        t.propagate_from_src_to_dest();

        // Then
        assert!(!t.result);
        t.after();
    }

    // port: ClusterPropagatorTest#propagation_doesNotMergeProperties_fromDestToSrc
    #[test]
    fn propagation_does_not_merge_properties_from_dest_to_src() {
        let mut t = ClusterPropagatorTest::new();
        // Given
        ClusterPropagatorTest::associate(&mut t.arena, t.prop, t.dest);

        // When
        t.propagate_from_src_to_dest();

        // Then
        assert!(!t.result);
        assert!(t.src.get_associated_props(&t.arena).is_empty());
        t.after();
    }

    // port: ClusterPropagatorTest#propagation_doesNotMergeProperties_thatAreInvalidated
    #[test]
    fn propagation_does_not_merge_properties_that_are_invalidated() {
        let mut t = ClusterPropagatorTest::new();
        // Given
        ClusterPropagatorTest::associate(&mut t.arena, t.prop, t.src);
        t.prop
            .invalidate(&mut t.arena, Invalidation::well_known_property());

        // When
        t.propagate_from_src_to_dest();

        // Then
        assert!(!t.result);
        assert!(t.dest.get_associated_props(&t.arena).is_empty());
        t.after();
    }
}

mod color_graph_node_factory_test {
    use closure_jscomp::{
        colors::{
            Color, ColorId,
            color_registry::ColorRegistry,
            standard_colors::{self, NUMBER_OBJECT_ID, STRING_OBJECT_ID},
        },
        disambiguate::{
            color_graph_node::{ColorGraphNodeId, DisambiguateArena},
            color_graph_node_factory::ColorGraphNodeFactory,
        },
    };
    use closure_rhino::fast_hash::IndexSet;
    use std::sync::Arc;

    // port: ColorGraphNodeFactoryTest#initRegistry
    fn init_registry() -> Arc<ColorRegistry> {
        Arc::new(
            ColorRegistry::builder()
                .set_default_native_colors_for_testing()
                .build(),
        )
    }

    fn set_of(colors: &[Color]) -> IndexSet<Color> {
        colors.iter().cloned().collect()
    }

    // port: ColorGraphNodeFactoryTest#topLikeTypes_flattenToTop
    #[test]
    fn top_like_types_flatten_to_top() {
        let color_registry = init_registry();
        let mut arena = DisambiguateArena::new();
        // Given
        let mut factory = ColorGraphNodeFactory::create_factory(&mut arena, color_registry);
        let flat_top = factory.create_node(&mut arena, Some(&standard_colors::UNKNOWN));

        // When & Then
        assert_eq!(factory.create_node(&mut arena, None), flat_top);
        assert_eq!(
            factory.create_node(&mut arena, Some(&standard_colors::NULL_OR_VOID)),
            flat_top
        );
    }

    // port: ColorGraphNodeFactoryTest#primitiveTypes_flattenToBoxedType
    #[test]
    fn primitive_types_flatten_to_boxed_type() {
        let color_registry = init_registry();
        let mut arena = DisambiguateArena::new();
        let mut factory =
            ColorGraphNodeFactory::create_factory(&mut arena, Arc::clone(&color_registry));
        let flat_string = factory.create_node(&mut arena, Some(&standard_colors::STRING));
        let flat_string_object =
            factory.create_node(&mut arena, Some(&color_registry.get(STRING_OBJECT_ID)));
        let flat_top = factory.create_node(&mut arena, Some(&standard_colors::UNKNOWN));

        assert_ne!(flat_string, flat_top);
        assert_eq!(flat_string, flat_string_object);

        let primitives = factory.create_node(
            &mut arena,
            Some(&Color::create_union(&set_of(&[
                standard_colors::STRING.clone(),
                standard_colors::NUMBER.clone(),
            ]))),
        );
        let boxed = factory.create_node(
            &mut arena,
            Some(&Color::create_union(&set_of(&[
                color_registry.get(STRING_OBJECT_ID),
                color_registry.get(NUMBER_OBJECT_ID),
            ]))),
        );
        assert_eq!(primitives, boxed);
    }

    // port: ColorGraphNodeFactoryTest#unionTypes_whenFlattened_dropNullAndUndefined
    #[test]
    fn union_types_when_flattened_drop_null_and_undefined() {
        let color_registry = init_registry();
        let mut arena = DisambiguateArena::new();
        // Given
        let mut factory = ColorGraphNodeFactory::create_factory(&mut arena, color_registry);

        let null_or_void_or_number_type = Color::create_union(&set_of(&[
            standard_colors::NULL_OR_VOID.clone(),
            standard_colors::NUMBER.clone(),
        ]));

        let flat_number = factory.create_node(&mut arena, Some(&standard_colors::NUMBER));

        // Given
        let flat_null_or_void_or_number =
            factory.create_node(&mut arena, Some(&null_or_void_or_number_type));

        // Then
        assert_eq!(flat_null_or_void_or_number, flat_number);
    }

    // port: ColorGraphNodeFactoryTest#colorGraphNodes_areCached
    #[test]
    fn color_graph_nodes_are_cached() {
        let color_registry = init_registry();
        let mut arena = DisambiguateArena::new();
        // Given
        let mut factory = ColorGraphNodeFactory::create_factory(&mut arena, color_registry);

        let foo_type = Color::single_builder()
            .set_id(ColorId::from_ascii("Foo"))
            .build();
        let flat_foo = factory.create_node(&mut arena, Some(&foo_type));

        // When
        let flat_foo_again = factory.create_node(&mut arena, Some(&foo_type));

        // Then
        assert_eq!(flat_foo_again, flat_foo);
    }

    fn sample_types() -> IndexSet<Color> {
        set_of(&[
            standard_colors::NUMBER.clone(),
            Color::single_builder()
                .set_id(ColorId::from_ascii("Foo"))
                .build(),
            Color::single_builder()
                .set_id(ColorId::from_ascii("Bar"))
                .build(),
        ])
    }

    // port: ColorGraphNodeFactoryTest#colorGraphNodes_areTracked
    #[test]
    fn color_graph_nodes_are_tracked() {
        let color_registry = init_registry();
        let mut arena = DisambiguateArena::new();
        // Given
        let mut factory = ColorGraphNodeFactory::create_factory(&mut arena, color_registry);

        let sample_types = sample_types();

        // When
        let flat_samples = sample_types
            .iter()
            .map(|t| factory.create_node(&mut arena, Some(t)))
            .collect::<IndexSet<_>>();
        let all_known_types = factory.get_all_known_types();

        // Then
        for flat in &flat_samples {
            assert!(all_known_types.contains(flat));
        }
    }

    // port: ColorGraphNodeFactoryTest#colorGraphNodeIndices_areUnique
    #[test]
    fn color_graph_node_indices_are_unique() {
        let color_registry = init_registry();
        let mut arena = DisambiguateArena::new();
        // Given

        let mut factory = ColorGraphNodeFactory::create_factory(&mut arena, color_registry);

        let sample_types = sample_types();

        // When
        let flat_samples = sample_types
            .iter()
            .map(|t| factory.create_node(&mut arena, Some(t)))
            .collect::<IndexSet<_>>();

        // Then
        let indices = flat_samples
            .iter()
            .map(|n| n.get_index(&arena))
            .collect::<Vec<_>>();
        let unique = indices.iter().copied().collect::<IndexSet<_>>();
        assert_eq!(unique.len(), indices.len());
    }

    // port: ColorGraphNodeFactoryTest#colorGraphNodeIndices_areDeterministic
    #[test]
    fn color_graph_node_indices_are_deterministic() {
        let color_registry = init_registry();
        let mut arena = DisambiguateArena::new();
        // Given
        let sample_a = create_sample_color_graph_nodes_for_determinism_test(
            &mut arena,
            Arc::clone(&color_registry),
        );
        let sample_b = create_sample_color_graph_nodes_for_determinism_test(
            &mut arena,
            Arc::clone(&color_registry),
        );

        // Then
        // comparingElementsUsing(ID_MATCH).containsExactlyElementsIn(sampleB).inOrder()
        assert_eq!(
            sample_a
                .iter()
                .map(|n| n.get_index(&arena))
                .collect::<Vec<_>>(),
            sample_b
                .iter()
                .map(|n| n.get_index(&arena))
                .collect::<Vec<_>>()
        );
        // comparingElementsUsing(referenceEquality()).containsNoneIn(sampleB)
        for a in &sample_a {
            assert!(!sample_b.contains(a));
        }
    }

    // port: ColorGraphNodeFactoryTest#createSampleColorGraphNodesForDeterminismTest
    fn create_sample_color_graph_nodes_for_determinism_test(
        arena: &mut DisambiguateArena,
        color_registry: Arc<ColorRegistry>,
    ) -> IndexSet<ColorGraphNodeId> {
        let mut factory = ColorGraphNodeFactory::create_factory(arena, color_registry);

        let sample_types = sample_types();

        sample_types
            .iter()
            .map(|t| factory.create_node(arena, Some(t)))
            .collect()
    }
}

mod use_site_renamer_test {
    use closure_jscomp::{
        disambiguate::{
            color_graph_node::{ColorGraphNode, DisambiguateArena, PropertyClusteringId},
            invalidation::Invalidation,
            property_clustering::PropertyClustering,
            use_site_renamer::{RenamingIndex, UseSiteRenamer},
        },
        graph::union_find::UnionFind,
    };
    use closure_rhino::fast_hash::IndexSet;
    use closure_rhino::{
        ir::IR,
        js_string::JsString,
        node::{Ast, NodeId},
    };

    const PROP_NAME: &str = "prop";

    struct UseSiteRenamerTest {
        ast: Ast,
        arena: DisambiguateArena,
        reported_mutations: IndexSet<NodeId>,
        prop: PropertyClusteringId,
        renaming_index: RenamingIndex,
    }

    impl UseSiteRenamerTest {
        fn new() -> Self {
            let mut arena = DisambiguateArena::new();
            let prop = PropertyClustering::new(&mut arena, JsString::from(PROP_NAME));
            Self {
                ast: Ast::new(),
                arena,
                reported_mutations: IndexSet::<_>::default(),
                prop,
                renaming_index: RenamingIndex::default(),
            }
        }

        fn name(&mut self) -> NodeId {
            IR::name(&mut self.ast, PROP_NAME)
        }

        // port: UseSiteRenamerTest#verifyRenamingIndex_containsExactlyNewNames
        fn verify_renaming_index_contains_exactly_new_names(&self) {
            if self.prop.is_invalidated(&self.arena) {
                // containsExactly(PROP_NAME, "<INVALIDATED>")
                assert_eq!(self.renaming_index.len(), 1);
                let values = &self.renaming_index[&JsString::from(PROP_NAME)];
                assert_eq!(
                    values.iter().cloned().collect::<Vec<_>>(),
                    vec![JsString::from("<INVALIDATED>")]
                );
                return;
            }

            // assertThat(renamingIndex.values()).containsExactlyElementsIn(namesOf(useSites))
            let mut values = self
                .renaming_index
                .values()
                .flat_map(|names| names.iter().cloned())
                .collect::<Vec<_>>();
            let mut expected = self
                .names_of(self.prop.get_use_sites(&self.arena).keys().copied())
                .into_iter()
                .collect::<Vec<_>>();
            values.sort();
            expected.sort();
            assert_eq!(values, expected);
        }

        // port: UseSiteRenamerTest#verifyNewNames_areReportedAsMutations
        fn verify_new_names_are_reported_as_mutations(&self) {
            if self.prop.is_invalidated(&self.arena) {
                return;
            }

            let mutated_nodes = self
                .prop
                .get_use_sites(&self.arena)
                .keys()
                .copied()
                .filter(|n| n.get_string(&self.ast) != PROP_NAME)
                .collect::<IndexSet<_>>();

            // containsExactlyElementsIn
            assert_eq!(self.reported_mutations.len(), mutated_nodes.len());
            for n in &mutated_nodes {
                assert!(self.reported_mutations.contains(n));
            }
        }

        /// JUnit runs both `@After` methods after each test.
        fn after(&self) {
            self.verify_renaming_index_contains_exactly_new_names();
            self.verify_new_names_are_reported_as_mutations();
        }

        // port: UseSiteRenamerTest#runRename
        fn run_rename(&mut self) {
            let reported_mutations = &mut self.reported_mutations;
            let mut renamer: UseSiteRenamer<'_, Ast> =
                UseSiteRenamer::new(Box::new(|_ast: &mut Ast, n: NodeId| {
                    reported_mutations.insert(n);
                }));
            renamer.rename_uses(&mut self.ast, &mut self.arena, self.prop);

            self.renaming_index = renamer.get_renaming_index();
        }

        // port: UseSiteRenamerTest#namesOf
        fn names_of(&self, use_sites: impl Iterator<Item = NodeId>) -> IndexSet<JsString> {
            use_sites.map(|n| n.get_string(&self.ast)).collect()
        }
    }

    // port: UseSiteRenamerTest#renameUses_renamesConsistently_withinEachCluster
    #[test]
    fn rename_uses_renames_consistently_within_each_cluster() {
        let mut t = UseSiteRenamerTest::new();
        // Given
        let name1a = t.name();
        let name1b = t.name();
        let name1c = t.name();
        let name2 = t.name();

        let type1a = ColorGraphNode::create_for_testing(&mut t.arena, -1);
        let type1b = ColorGraphNode::create_for_testing(&mut t.arena, -2);
        let type1c = ColorGraphNode::create_for_testing(&mut t.arena, -3);
        let type2 = ColorGraphNode::create_for_testing(&mut t.arena, -4);

        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(name1a, type1a);
        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(name1b, type1b);
        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(name1c, type1c);
        t.prop.get_use_sites_mut(&mut t.arena).insert(name2, type2);

        t.prop.get_clusters_mut(&mut t.arena).union(type1a, type1b);
        t.prop.get_clusters_mut(&mut t.arena).union(type1a, type1c);
        t.prop.get_clusters_mut(&mut t.arena).add(type2);

        // When
        t.run_rename();

        // Then
        assert_eq!(name1b.get_string(&t.ast), name1a.get_string(&t.ast));
        assert_eq!(name1c.get_string(&t.ast), name1a.get_string(&t.ast));
        assert_ne!(name1a.get_string(&t.ast), PROP_NAME);
        t.after();
    }

    // port: UseSiteRenamerTest#renameUses_renamesDistinctly_betweenEachCluster
    #[test]
    fn rename_uses_renames_distinctly_between_each_cluster() {
        let mut t = UseSiteRenamerTest::new();
        // Given
        let name1 = t.name();
        let name2 = t.name();
        let name3 = t.name();

        let type1 = ColorGraphNode::create_for_testing(&mut t.arena, -1);
        let type2 = ColorGraphNode::create_for_testing(&mut t.arena, -2);
        let type3 = ColorGraphNode::create_for_testing(&mut t.arena, -3);

        t.prop.get_use_sites_mut(&mut t.arena).insert(name1, type1);
        t.prop.get_use_sites_mut(&mut t.arena).insert(name2, type2);
        t.prop.get_use_sites_mut(&mut t.arena).insert(name3, type3);

        t.prop.get_clusters_mut(&mut t.arena).add(type1);
        t.prop.get_clusters_mut(&mut t.arena).add(type2);
        t.prop.get_clusters_mut(&mut t.arena).add(type3);

        // When
        t.run_rename();

        // Then
        assert_ne!(name1.get_string(&t.ast), name2.get_string(&t.ast));
        assert_ne!(name1.get_string(&t.ast), name3.get_string(&t.ast));
        assert_ne!(name2.get_string(&t.ast), name3.get_string(&t.ast));
        t.after();
    }

    // port: UseSiteRenamerTest#renameUses_doesntRename_externsCluster
    #[test]
    fn rename_uses_doesnt_rename_externs_cluster() {
        let mut t = UseSiteRenamerTest::new();
        // Given
        let extern_name1 = t.name();
        let extern_name2 = t.name();
        let extern_name3 = t.name();
        let src_name = t.name();

        let extern_type1 = ColorGraphNode::create_for_testing(&mut t.arena, -1);
        let extern_type2 = ColorGraphNode::create_for_testing(&mut t.arena, -2);
        let extern_type3 = ColorGraphNode::create_for_testing(&mut t.arena, -3);
        let src_type = ColorGraphNode::create_for_testing(&mut t.arena, -4);

        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(extern_name1, extern_type1);
        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(extern_name2, extern_type2);
        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(extern_name3, extern_type3);
        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(src_name, src_type);

        t.prop
            .register_original_name_type(&mut t.arena, extern_type1);
        t.prop
            .register_original_name_type(&mut t.arena, extern_type2);
        t.prop
            .register_original_name_type(&mut t.arena, extern_type3);

        t.prop.get_clusters_mut(&mut t.arena).add(src_type);

        // When
        t.run_rename();

        // Then
        assert_eq!(extern_name1.get_string(&t.ast), PROP_NAME);
        assert_eq!(extern_name2.get_string(&t.ast), PROP_NAME);
        assert_eq!(extern_name3.get_string(&t.ast), PROP_NAME);
        assert_ne!(src_name.get_string(&t.ast), PROP_NAME);
        t.after();
    }

    // port: UseSiteRenamerTest#renameUses_doesntRename_invalidatedProps
    #[test]
    fn rename_uses_doesnt_rename_invalidated_props() {
        let mut t = UseSiteRenamerTest::new();
        // Given
        let name1 = t.name();
        let name2 = t.name();
        let name3 = t.name();

        let type1 = ColorGraphNode::create_for_testing(&mut t.arena, -1);
        let type2 = ColorGraphNode::create_for_testing(&mut t.arena, -2);
        let type3 = ColorGraphNode::create_for_testing(&mut t.arena, -3);

        t.prop.get_use_sites_mut(&mut t.arena).insert(name1, type1);
        t.prop.get_use_sites_mut(&mut t.arena).insert(name2, type2);
        t.prop.get_use_sites_mut(&mut t.arena).insert(name3, type3);

        t.prop
            .invalidate(&mut t.arena, Invalidation::well_known_property());

        // When
        t.run_rename();

        // Then
        assert_eq!(name1.get_string(&t.ast), PROP_NAME);
        assert_eq!(name1.get_string(&t.ast), PROP_NAME);
        assert_eq!(name2.get_string(&t.ast), PROP_NAME);
        t.after();
    }

    // port: UseSiteRenamerTest#renameUses_doesntRenameProp_whenThereIsOnlyOneCluster
    #[test]
    fn rename_uses_doesnt_rename_prop_when_there_is_only_one_cluster() {
        let mut t = UseSiteRenamerTest::new();
        // Given
        let name1a = t.name();
        let name1b = t.name();
        let name1c = t.name();

        let type1a = ColorGraphNode::create_for_testing(&mut t.arena, -1);
        let type1b = ColorGraphNode::create_for_testing(&mut t.arena, -2);
        let type1c = ColorGraphNode::create_for_testing(&mut t.arena, -3);

        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(name1a, type1a);
        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(name1b, type1b);
        t.prop
            .get_use_sites_mut(&mut t.arena)
            .insert(name1c, type1c);

        t.prop.get_clusters_mut(&mut t.arena).union(type1a, type1b);
        t.prop.get_clusters_mut(&mut t.arena).union(type1a, type1c);

        // When
        t.run_rename();

        // Then
        assert_eq!(name1b.get_string(&t.ast), PROP_NAME);
        assert_eq!(name1c.get_string(&t.ast), PROP_NAME);
        assert_eq!(name1a.get_string(&t.ast), PROP_NAME);
        t.after();
    }
}

/// `ColorGraphBuilderTest` runs as corpus unit records, except
/// `disambiguationSupertypes_createConnection`, which builds its registry by hand and never calls
/// `test(...)`, so the corpus has no record of it.
mod color_graph_builder_test {
    use closure_jscomp::{
        colors::{Color, ColorId, color_registry::ColorRegistry},
        disambiguate::{
            color_graph_builder::{ColorGraph, ColorGraphBuilder, EdgeReason},
            color_graph_node::DisambiguateArena,
            color_graph_node_factory::ColorGraphNodeFactory,
        },
        graph::{
            AdjacencyGraph,
            graph::{Graph, GraphEdge},
            graph_node::GraphNode,
            lowest_common_ancestor_finder::LowestCommonAncestorFinder,
        },
    };
    use closure_rhino::fast_hash::IndexMap;
    use std::sync::Arc;

    struct ColorGraphBuilderTest {
        arena: DisambiguateArena,
        /// This registry is only used to lookup box colors so it doesn't have to be the same one
        /// used by the graph builder.
        graph_node_factory: ColorGraphNodeFactory,
        result: Option<ColorGraph>,
    }

    impl ColorGraphBuilderTest {
        fn new() -> Self {
            let mut arena = DisambiguateArena::new();
            let graph_node_factory = ColorGraphNodeFactory::create_factory(
                &mut arena,
                Arc::new(
                    ColorRegistry::builder()
                        .set_default_native_colors_for_testing()
                        .build(),
                ),
            );
            Self {
                arena,
                graph_node_factory,
                result: None,
            }
        }

        // port: ColorGraphBuilderTest#createBuilder
        //
        // Java passes `new StubLcaFinder()` (no stubs) when `optLcaFinder` is null. The Rust
        // builder takes the concrete LowestCommonAncestorFinder, so the stub's precondition (no
        // `findAll` call without a stub; the builder only calls it for union nodes) is checked on
        // the result by `assert_no_lca_queries`.
        fn create_builder(&mut self, registry: ColorRegistry) -> ColorGraphBuilder {
            ColorGraphBuilder::new(
                &mut self.graph_node_factory,
                &mut self.arena,
                |_graph| LowestCommonAncestorFinder::new(),
                Arc::new(registry),
            )
        }

        /// StubLcaFinder#findAll without stubs fails `assertThat(this.stubs).containsKey(..)`;
        /// ColorGraphBuilder#build queries the finder once per union node.
        fn assert_no_lca_queries(&self) {
            let result = self.result.as_ref().unwrap();
            for node in result.get_nodes() {
                assert!(
                    !node.get_value(result).get_color(&self.arena).is_union(),
                    "StubLcaFinder has no stub for the union {:?}",
                    node.get_value(result).get_color(&self.arena).get_id()
                );
            }
        }

        // port: ColorGraphBuilderTest#assertThatResultAsTable
        fn result_as_table(&self) -> IndexMap<(ColorId, ColorId), EdgeReason> {
            let result = self.result.as_ref().unwrap();
            let mut table = IndexMap::<_, _>::default();
            for edge in result.get_edges() {
                let key = (
                    name_of(&self.arena, *edge.get_source(result).get_value(result)),
                    name_of(&self.arena, *edge.get_destination(result).get_value(result)),
                );
                let value = *edge.get_value(result);
                // ImmutableTable.Builder#buildOrThrow rejects duplicate cells.
                assert!(table.insert(key, value).is_none(), "duplicate cell {key:?}");
            }
            table
        }
    }

    // port: ColorGraphBuilderTest#nameOf(ColorGraphNode)
    fn name_of(
        arena: &DisambiguateArena,
        flat: closure_jscomp::disambiguate::color_graph_node::ColorGraphNodeId,
    ) -> ColorId {
        flat.get_color(arena).get_id()
    }

    // port: ColorGraphBuilderTest#colorWithId
    fn color_with_id(id: ColorId) -> closure_jscomp::colors::color::Builder {
        Color::single_builder().set_id(id)
    }

    // port: ColorGraphBuilderTest#disambiguationSupertypes_createConnection
    #[test]
    fn disambiguation_supertypes_create_connection() {
        let mut t = ColorGraphBuilderTest::new();
        // Given
        let parent = color_with_id(ColorId::from_unsigned(100)).build();
        let child = color_with_id(ColorId::from_unsigned(101)).build();

        let mut builder = t.create_builder(
            ColorRegistry::builder()
                .set_default_native_colors_for_testing()
                .add_disambiguation_edge(child.clone(), parent.clone())
                .build(),
        );

        let child_node = t.graph_node_factory.create_node(&mut t.arena, Some(&child));
        builder.add(&mut t.graph_node_factory, &mut t.arena, child_node);

        // When
        t.result = Some(builder.build(&mut t.graph_node_factory, &mut t.arena));

        // Then
        t.assert_no_lca_queries();
        assert_eq!(
            t.result_as_table().get(&(parent.get_id(), child.get_id())),
            Some(&EdgeReason::CAN_HOLD)
        );
    }
}
