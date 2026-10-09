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
/*
 * Copyright (C) 2008 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/disambiguate/DisambiguateProperties.java.
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/Gson.java.

//! Port of `disambiguate/DisambiguateProperties.java`.

use super::{
    cluster_propagator::ClusterPropagator,
    color_find_property_references::ColorFindPropertyReferences,
    color_graph_builder::{ColorGraph, ColorGraphBuilder, EdgeReason},
    color_graph_node::{
        ColorGraphNodeId, DisambiguateArena, PropAssociation, PropertyClusteringId,
    },
    color_graph_node_factory::ColorGraphNodeFactory,
    invalidation::Invalidation,
    property_clustering::PropertyClustering,
    use_site_renamer::{RenameUsesResult, UseSiteRenamer},
};
use crate::{
    AbstractCompiler,
    colors::{Color, color_registry::ColorRegistry},
    compiler_pass::CompilerPass,
    diagnostic::{log_file::StreamedJsonProducer, logs_gson::LogsGsonObject},
    diagnostic_type::DiagnosticType,
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    graph::{
        adjacency_graph::AdjacencyGraph,
        di_graph::{DiGraph, DiGraphEdge, DiGraphNode},
        fixed_point_graph_traversal::FixedPointGraphTraversal,
        graph::GraphEdge,
        graph_node::GraphNode,
        lowest_common_ancestor_finder::LowestCommonAncestorFinder,
        union_find::UnionFind,
    },
    js_error::JSError,
    node_traversal::NodeTraversal,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{check_argument, js_string::JsString, node::NodeId};
use closure_sourcemap::gson::stream::json_writer::JsonWriter;
use std::{cmp::Ordering, sync::Arc};

// port: DisambiguateProperties#PROPERTY_INVALIDATION
pub static PROPERTY_INVALIDATION: DiagnosticType = DiagnosticType::error(
    "JSC_DISAMBIGUATE2_PROPERTY_INVALIDATION",
    "Property ''{0}'' was required to be disambiguated but was invalidated.{1}",
);

/// Assembles the various parts of the diambiguator to execute them as a compiler pass.
pub struct DisambiguateProperties {
    properties_that_must_disambiguate: IndexSet<JsString>,
    registry: Arc<ColorRegistry>,
    may_have_property_seen_set: IndexSet<Color>,
}

impl DisambiguateProperties {
    // port: DisambiguateProperties#DisambiguateProperties
    pub fn new(
        compiler: &AbstractCompiler,
        properties_that_must_disambiguate: IndexSet<JsString>,
    ) -> Self {
        Self {
            properties_that_must_disambiguate,
            registry: Arc::clone(compiler.get_color_registry()),
            may_have_property_seen_set: IndexSet::<_>::default(),
        }
    }
}

impl CompilerPass for DisambiguateProperties {
    // port: DisambiguateProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        check_argument!(externs.get_parent(compiler) == root.get_parent(compiler));

        let mut arena = DisambiguateArena::new();
        let mut flattener =
            ColorGraphNodeFactory::create_factory(&mut arena, Arc::clone(&self.registry));
        // Java constructs `findRefs` here. Its constructor has no effects, and in Rust it borrows
        // the factory and the arena until the traversal ends, so it is created right before it.
        let mut graph_builder = ColorGraphBuilder::new(
            &mut flattener,
            &mut arena,
            |_graph| LowestCommonAncestorFinder::new(),
            Arc::clone(&self.registry),
        );
        let mut propagator = ClusterPropagator::new();
        let mut renamer: UseSiteRenamer<'_, AbstractCompiler> = UseSiteRenamer::new(Box::new(
            /* mutationCb= */
            |compiler: &mut AbstractCompiler, n: NodeId| {
                compiler.report_change_to_enclosing_scope(n)
            },
        ));

        let mut prop_index = {
            let mut find_refs = ColorFindPropertyReferences::new(
                &mut flattener,
                &mut arena,
                Box::new(|compiler: &AbstractCompiler, n: NodeId| {
                    compiler
                        .get_coding_convention()
                        .is_property_rename_function(compiler, n)
                }),
            );
            let externs_parent = externs.get_parent(compiler).unwrap();
            NodeTraversal::traverse(compiler, externs_parent, &mut find_refs);
            find_refs.get_property_index()
        };

        Self::invalidate_well_known_properties(&mut arena, &mut prop_index);
        self.log_for_diagnostics_streamed(
            compiler,
            "prop_refs",
            // use a StreamedJsonProducer instead of building up the entire json string at once to
            // prevent OOMs for very large projects.
            &PropRefsStreamedJsonProducer {
                compiler,
                arena: &arena,
                prop_index: &prop_index,
            },
        );

        let all_known_types = flattener.get_all_known_types();
        graph_builder.add_all(&mut flattener, &mut arena, all_known_types);
        let mut graph = graph_builder.build(&mut flattener, &mut arena);

        // Model legacy behavior from the old (pre-January 2021) disambiguator.
        // TODO(b/177695515): delete this section.
        for color_graph_node in flattener.get_all_known_types() {
            if graph.get_out_edges(&color_graph_node).is_empty() {
                // Skipping leaf types improves code size, especially as "namespace" types are all
                // leaf types and will often have their declared properties collapsed into
                // variables.
                continue;
            }
            if color_graph_node.get_color(&arena).is_union() {
                // Only need this step for SINGLE ColorGraphNodes because
                // and we will add properties of each alternate elsewhere in this loop.
                continue;
            }
            Self::register_own_declared_properties(&mut arena, color_graph_node, &prop_index);
        }

        self.log_for_diagnostics(compiler, "graph", &mut || {
            let mut nodes = graph
                .get_nodes()
                .into_iter()
                .map(|n| TypeNodeJson::new(&graph, &arena, n))
                .collect::<Vec<_>>();
            nodes.sort_by_key(|x| x.index);
            Some(Box::new(nodes))
        });

        self.invalidate_based_on_type(&flattener, &mut arena);

        FixedPointGraphTraversal::new_traversal(
            |_graph: &mut ColorGraph,
             src: ColorGraphNodeId,
             e: EdgeReason,
             dest: ColorGraphNodeId| {
                propagator.traverse_edge(&mut arena, src, Some(e), dest)
            },
        )
        .compute_fixed_point(&mut graph);

        let mut tracker_summary_generator = TrackerSummaryGenerator::default();
        for prop in prop_index.values().copied() {
            let rename_uses_result = renamer.rename_uses(compiler, &mut arena, prop);
            tracker_summary_generator.add_rename_uses_result(rename_uses_result);
            if prop.is_invalidated(&arena)
                && self
                    .properties_that_must_disambiguate
                    .contains(prop.get_name(&arena))
            {
                compiler.report(self.create_invalidation_error(&arena, prop));
            }
        }
        compiler.report_disambiguate_properties_summary(|| {
            tracker_summary_generator.build_summary_string()
        });

        self.log_for_diagnostics(compiler, "renaming_index", &mut || {
            Some(Box::new(Self::build_renaming_index(
                &arena,
                &prop_index,
                &renamer,
            )))
        });
        let registry = Arc::clone(&self.registry);
        self.log_for_diagnostics(compiler, "mismatches", &mut || {
            Some(Box::new(mismatch_locations_inverse(&registry)))
        });

        GatherGetterAndSetterProperties::update(compiler, externs, root);
    }
}

/// No-op class to use when tracer mode is not enabled.
#[derive(Default)]
struct TrackerSummaryGenerator {
    total: i32,
    num_invalidated: i32,
    num_disambiguated: i32,
    num_only_one_cluster: i32,
}

impl TrackerSummaryGenerator {
    // port: DisambiguateProperties.TrackerSummaryGenerator#addRenameUsesResult
    fn add_rename_uses_result(&mut self, rename_uses_result: RenameUsesResult) {
        self.total += 1;
        match rename_uses_result {
            RenameUsesResult::INVALIDATED => self.num_invalidated += 1,
            RenameUsesResult::ONLY_ONE_CLUSTER => self.num_only_one_cluster += 1,
            RenameUsesResult::DISAMBIGUATED => self.num_disambiguated += 1,
        }
    }

    // port: DisambiguateProperties.TrackerSummaryGenerator#buildSummaryString
    fn build_summary_string(&self) -> String {
        format!(
            "{} property names, {} disambiguated, {} invalidated, {} had a single cluster",
            self.total, self.num_disambiguated, self.num_invalidated, self.num_only_one_cluster
        )
    }
}

impl DisambiguateProperties {
    // port: DisambiguateProperties#createInvalidationError
    fn create_invalidation_error(
        &self,
        arena: &DisambiguateArena,
        prop: PropertyClusteringId,
    ) -> JSError {
        let additional_context = "";

        // Java: JSError.make(null, -1, -1, ...): no source name, line -1 and column -1, which are
        // the builder's defaults.
        JSError::make_without_location(
            &PROPERTY_INVALIDATION,
            &[&prop.get_name(arena).to_string(), additional_context],
        )
    }

    // port: DisambiguateProperties#buildRenamingIndex
    fn build_renaming_index(
        arena: &DisambiguateArena,
        props: &IndexMap<JsString, PropertyClusteringId>,
        renamer: &UseSiteRenamer<'_, AbstractCompiler>,
    ) -> RenamingIndexJson {
        let new_names = renamer.get_renaming_index();
        // toImmutableSortedMap(naturalOrder(), PropertyClustering::getName, ...)
        let entries = to_sorted_set(props.values().copied().collect(), |a, b| {
            a.get_name(arena).cmp(b.get_name(arena))
        });
        RenamingIndexJson(
            entries
                .into_iter()
                .map(|prop| {
                    let value = if prop.is_invalidated(arena) {
                        RenamingIndexValue::Invalidation(prop.get_last_invalidation(arena))
                    } else {
                        RenamingIndexValue::NewNames(to_sorted_set(
                            new_names
                                .get(prop.get_name(arena))
                                .map(|s| s.iter().cloned().collect())
                                .unwrap_or_default(),
                            JsString::cmp,
                        ))
                    };
                    (prop.get_name(arena).clone(), value)
                })
                .collect(),
        )
    }

    // port: DisambiguateProperties#invalidateWellKnownProperties
    fn invalidate_well_known_properties(
        arena: &mut DisambiguateArena,
        prop_index: &mut IndexMap<JsString, PropertyClusteringId>,
    ) {
        /*
         * Expand this list as needed; it wasn't created exhaustively.
         *
         * <p>Good candidates are: props accessed by builtin functions, props accessed by syntax
         * sugar, props used in strange ways by the language spec, etc.
         *
         * <p>TODO(b/169899789): consider instead omitting these properties entirely from the
         * serialized colors format.
         */
        let names = ["prototype", "constructor", "then"];
        for name in names {
            let name = JsString::from(name);
            let prop = match prop_index.get(&name) {
                Some(prop) => *prop,
                None => {
                    let prop = PropertyClustering::new(arena, name.clone());
                    prop_index.insert(name, prop);
                    prop
                }
            };
            prop.invalidate(arena, Invalidation::well_known_property());
        }
    }

    // port: DisambiguateProperties#invalidateBasedOnType
    fn invalidate_based_on_type(
        &mut self,
        flattener: &ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
    ) {
        for r#type in flattener.get_all_known_types() {
            if r#type.get_color(arena).is_invalidating() {
                let props = r#type
                    .get_associated_props(arena)
                    .keys()
                    .copied()
                    .collect::<Vec<_>>();
                for prop in props {
                    let index = r#type.get_index(arena);
                    prop.invalidate(arena, Invalidation::invalidating_type(index));
                }
            } else {
                self.invalidate_non_declared_property_accesses(arena, r#type);
            }
        }
    }

    /// Invalidate all property accesses that cause "missing property" warnings.
    ///
    /// This behavior is not inherently necessary for correctness. It only exists because the
    /// older version of the disambiguator invalidated these properties and some projects began to
    /// rely on this.
    ///
    /// TODO(b/177695515): delete this method
    // port: DisambiguateProperties#invalidateNonDeclaredPropertyAccesses
    fn invalidate_non_declared_property_accesses(
        &mut self,
        arena: &mut DisambiguateArena,
        color_graph_node: ColorGraphNodeId,
    ) {
        let color = color_graph_node.get_color(arena).clone();
        check_argument!(
            !color.is_invalidating(),
            "Not applicable to invalidating types. All their properties are invalidated"
        );

        let props = color_graph_node
            .get_associated_props(arena)
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for prop in props {
            if prop.is_invalidated(arena) {
                continue; // Skip unnecessary `hasProperty` lookups which can be expensive.
            }

            let name = prop.get_name(arena).clone();
            if !self.may_have_property(&color, &name) {
                let index = color_graph_node.get_index(arena);
                prop.invalidate(arena, Invalidation::undeclared_access(index));
            }
        }
    }

    /// Returns true if the color or any of its ancestors has the given property
    ///
    /// If this is a union, returns true if /any/ union alternate has the property.
    ///
    /// TODO(b/177695515): delete this method
    // port: DisambiguateProperties#mayHaveProperty
    fn may_have_property(&mut self, color: &Color, property_name: &JsString) -> bool {
        // try
        let result = 'try_block: {
            if !self.may_have_property_seen_set.insert(color.clone()) {
                break 'try_block false;
            }

            if color.is_union() {
                break 'try_block color
                    .get_union_elements()
                    .iter()
                    .any(|element| self.may_have_property(element, property_name));
            }

            if color.get_own_properties().contains(property_name) {
                break 'try_block true;
            }
            let registry = Arc::clone(&self.registry);
            registry
                .get_disambiguation_supertypes(color)
                .iter()
                .any(|element| self.may_have_property(element, property_name))
        };
        // finally
        self.may_have_property_seen_set.shift_remove(color);
        result
    }

    // port: DisambiguateProperties#registerOwnDeclaredProperties
    fn register_own_declared_properties(
        arena: &mut DisambiguateArena,
        color_graph_node: ColorGraphNodeId,
        prop_index: &IndexMap<JsString, PropertyClusteringId>,
    ) {
        check_argument!(!color_graph_node.get_color(arena).is_union());

        // For each type, get the list of its "own properties" and add them to its clustering.
        // This is only to mimic the behavior of the old disambiguator and could be removed if we
        // were confident we could update all existing code to be compatible.
        let own_properties = color_graph_node
            .get_color(arena)
            .get_own_properties()
            .clone();
        for prop_name in &own_properties {
            let Some(prop) = prop_index.get(prop_name).copied() else {
                // Ignore declared properties without any visible references to rename.
                continue;
            };
            if prop.is_invalidated(arena) {
                continue;
            }
            prop.get_clusters_mut(arena).add(color_graph_node);
            color_graph_node
                .get_associated_props_mut(arena)
                .insert(prop, PropAssociation::TYPE_SYSTEM);
        }
    }

    // port: DisambiguateProperties#logForDiagnostics(String,Supplier)
    fn log_for_diagnostics(
        &self,
        compiler: &AbstractCompiler,
        name: &str,
        data: &mut dyn FnMut() -> Option<Box<dyn LogsGsonObject>>,
    ) {
        // try (LogFile log = ...): the log closes when the scope ends.
        let mut log =
            compiler.create_or_reopen_log("DisambiguateProperties", &format!("{name}.log"), &[]);
        log.log_json_supplier(data);
        log.close();
    }

    // port: DisambiguateProperties#logForDiagnostics(String,LogFile.StreamedJsonProducer)
    fn log_for_diagnostics_streamed(
        &self,
        compiler: &AbstractCompiler,
        name: &str,
        data: &dyn StreamedJsonProducer,
    ) {
        // try (LogFile log = ...): the log closes when the scope ends.
        let mut log =
            compiler.create_or_reopen_log("DisambiguateProperties", &format!("{name}.log"), &[]);
        log.log_json(data);
        log.close();
    }
}

/// Guava `toImmutableSortedSet(comparator)`: sorted by the comparator, and of elements comparing
/// equal only the first is kept.
fn to_sorted_set<T>(mut elements: Vec<T>, compare: impl Fn(&T, &T) -> Ordering) -> Vec<T> {
    elements.sort_by(&compare);
    elements.dedup_by(|a, b| compare(b, a) == Ordering::Equal);
    elements
}

/// `registry.getMismatchLocationsForDebugging()::inverse`.
fn mismatch_locations_inverse(registry: &ColorRegistry) -> MismatchLocationsInverseJson {
    let mut inverse: IndexMap<String, IndexSet<String>> = IndexMap::<_, _>::default();
    for (id, locations) in registry.get_mismatch_locations_for_debugging() {
        for location in locations {
            inverse
                .entry(location.clone())
                .or_default()
                .insert(id.to_logs_gson());
        }
    }
    MismatchLocationsInverseJson(inverse)
}

/// The inverted SetMultimap<String, ColorId> as LogsGson writes it.
struct MismatchLocationsInverseJson(IndexMap<String, IndexSet<String>>);

impl LogsGsonObject for MismatchLocationsInverseJson {
    /// LogsGson's `Multimap` adapter (`asMap()`; ColorIds through `LogsGson.Able#toLogsGson`).
    fn write(&self, writer: &mut JsonWriter) {
        writer.begin_object();
        for (location, ids) in &self.0 {
            writer.name(location.as_str());
            writer.begin_array();
            for id in ids {
                id.write(writer);
            }
            writer.end_array();
        }
        writer.end_object();
    }
}

/// `buildRenamingIndex`'s `ImmutableMap<String, Object>`.
struct RenamingIndexJson(Vec<(JsString, RenamingIndexValue)>);

/// A renaming index value: the last Invalidation, or the sorted new names.
enum RenamingIndexValue {
    Invalidation(Invalidation),
    NewNames(Vec<JsString>),
}

impl LogsGsonObject for RenamingIndexJson {
    /// Gson's map adapter over Invalidation (reflective) and ImmutableSortedSet<String> values.
    fn write(&self, writer: &mut JsonWriter) {
        writer.begin_object();
        for (name, value) in &self.0 {
            writer.name(name.clone());
            match value {
                RenamingIndexValue::Invalidation(invalidation) => invalidation.write_gson(writer),
                RenamingIndexValue::NewNames(names) => {
                    writer.begin_array();
                    for name in names {
                        writer.value(Some(name.clone()));
                    }
                    writer.end_array();
                }
            }
        }
        writer.end_object();
    }
}

/// The anonymous `LogFile.StreamedJsonProducer` of `process` for "prop_refs".
struct PropRefsStreamedJsonProducer<'a> {
    compiler: &'a AbstractCompiler,
    arena: &'a DisambiguateArena,
    prop_index: &'a IndexMap<JsString, PropertyClusteringId>,
}

impl StreamedJsonProducer for PropRefsStreamedJsonProducer<'_> {
    // port: DisambiguateProperties#process.StreamedJsonProducer#writeJson
    fn write_json(&self, writer: &mut JsonWriter) -> std::io::Result<()> {
        let prop_refs_json = to_sorted_set(
            self.prop_index
                .values()
                .map(|prop| PropertyReferenceIndexJson::new(self.compiler, self.arena, *prop))
                .collect(),
            PropertyReferenceIndexJson::compare_to,
        );

        writer.begin_object();
        for prop_ref in &prop_refs_json {
            prop_ref.write_json(writer);
        }
        writer.end_object();
        Ok(())
    }
}

/// `GSON.toJson(src, typeOfSrc, writer)` with `GSON = new Gson()`: lenient, HTML-safe, nulls
/// omitted around the adapter's write; the writer's settings are restored afterwards.
// port: com.google.gson.Gson#toJson(Object,Type,JsonWriter)
fn gson_to_json(writer: &mut JsonWriter, write: impl FnOnce(&mut JsonWriter)) {
    let old_lenient = writer.is_lenient();
    writer.set_lenient(true);
    let old_html_safe = writer.is_html_safe();
    writer.set_html_safe(true);
    let old_serialize_nulls = writer.get_serialize_nulls();
    writer.set_serialize_nulls(false);
    write(writer);
    writer.set_lenient(old_lenient);
    writer.set_html_safe(old_html_safe);
    writer.set_serialize_nulls(old_serialize_nulls);
}

// port: DisambiguateProperties.PropertyReferenceIndexJson
struct PropertyReferenceIndexJson {
    name: JsString,
    refs: Vec<PropertyReferenceJson>,
}

impl PropertyReferenceIndexJson {
    // port: DisambiguateProperties.PropertyReferenceIndexJson#PropertyReferenceIndexJson
    fn new(
        compiler: &AbstractCompiler,
        arena: &DisambiguateArena,
        prop: PropertyClusteringId,
    ) -> Self {
        let name = prop.get_name(arena).clone();
        let refs = if prop.is_invalidated(arena) {
            Vec::new()
        } else {
            to_sorted_set(
                prop.get_use_sites(arena)
                    .iter()
                    .map(|(k, v)| PropertyReferenceJson::new(compiler, arena, *k, *v))
                    .collect(),
                PropertyReferenceJson::compare_to,
            )
        };
        Self { name, refs }
    }

    // port: DisambiguateProperties.PropertyReferenceIndexJson#compareTo
    fn compare_to(&self, x: &Self) -> Ordering {
        self.name.cmp(&x.name)
    }

    // port: DisambiguateProperties.PropertyReferenceIndexJson#writeJson
    fn write_json(&self, writer: &mut JsonWriter) {
        // creates an entry such as:
        //   foo: {name: 'foo', refs: [{location: 'bar.js:3:4', receiverIndex: 2}]}
        writer.name(self.name.clone());

        writer.begin_object();
        writer.name("name");
        writer.value(Some(self.name.clone()));
        writer.name("refs").begin_array();
        for r in &self.refs {
            // GSON.toJson(ref, PropertyReferenceJson.class, writer)
            gson_to_json(writer, |writer| r.write_gson(writer));
        }
        writer.end_array();
        writer.end_object();
    }
}

// port: DisambiguateProperties.PropertyReferenceJson
struct PropertyReferenceJson {
    location: String,
    receiver_index: i32,
}

impl PropertyReferenceJson {
    // port: DisambiguateProperties.PropertyReferenceJson#PropertyReferenceJson
    fn new(
        compiler: &AbstractCompiler,
        arena: &DisambiguateArena,
        location: NodeId,
        receiver: ColorGraphNodeId,
    ) -> Self {
        Self {
            location: location.get_location(compiler),
            receiver_index: receiver.get_index(arena),
        }
    }

    // port: DisambiguateProperties.PropertyReferenceJson#compareTo
    fn compare_to(&self, x: &Self) -> Ordering {
        self.receiver_index.cmp(&x.receiver_index).then_with(|| {
            JsString::from(self.location.as_str()).cmp(&JsString::from(x.location.as_str()))
        })
    }

    /// Gson's reflective adapter: the declared fields in order.
    fn write_gson(&self, writer: &mut JsonWriter) {
        writer.begin_object();
        writer.name("location");
        self.location.write(writer);
        writer.name("receiverIndex");
        self.receiver_index.write(writer);
        writer.end_object();
    }
}

// port: DisambiguateProperties.TypeNodeJson
struct TypeNodeJson {
    index: i32,
    // These fields are used reflectively via GSON.
    invalidating: bool,
    color_id: String,
    edges: Vec<TypeEdgeJson>,
    props: Vec<(JsString, PropAssociation)>,
}

impl TypeNodeJson {
    // port: DisambiguateProperties.TypeNodeJson#TypeNodeJson
    fn new(graph: &ColorGraph, arena: &DisambiguateArena, n: DiGraphNode) -> Self {
        let t = *n.get_value(graph);

        let index = t.get_index(arena);
        let color_id = t.get_color(arena).get_id().to_string();
        let invalidating = t.get_color(arena).is_invalidating();
        let edges = to_sorted_set(
            n.get_out_edges(graph)
                .iter()
                .map(|e| TypeEdgeJson::new(graph, arena, *e))
                .collect(),
            TypeEdgeJson::compare_to,
        );
        // toImmutableSortedMap(naturalOrder(), e -> e.getKey().getName(), Map.Entry::getValue)
        let props = to_sorted_set(
            t.get_associated_props(arena)
                .iter()
                .map(|(k, v)| (k.get_name(arena).clone(), *v))
                .collect(),
            |a, b| a.0.cmp(&b.0),
        );
        Self {
            index,
            invalidating,
            color_id,
            edges,
            props,
        }
    }
}

impl LogsGsonObject for TypeNodeJson {
    /// Gson's reflective adapter: the declared fields in order.
    fn write(&self, writer: &mut JsonWriter) {
        writer.begin_object();
        writer.name("index");
        self.index.write(writer);
        writer.name("invalidating");
        writer.value_boolean(self.invalidating);
        writer.name("colorId");
        self.color_id.write(writer);
        writer.name("edges");
        writer.begin_array();
        for edge in &self.edges {
            edge.write(writer);
        }
        writer.end_array();
        writer.name("props");
        writer.begin_object();
        for (name, association) in &self.props {
            writer.name(name.clone());
            association.name().write(writer);
        }
        writer.end_object();
        writer.end_object();
    }
}

// port: DisambiguateProperties.TypeEdgeJson
struct TypeEdgeJson {
    dest: i32,
    // This field is used reflectively via GSON.
    value: EdgeReason,
}

impl TypeEdgeJson {
    // port: DisambiguateProperties.TypeEdgeJson#TypeEdgeJson
    fn new(graph: &ColorGraph, arena: &DisambiguateArena, e: DiGraphEdge) -> Self {
        Self {
            dest: e.get_destination(graph).get_value(graph).get_index(arena),
            value: *e.get_value(graph),
        }
    }

    // port: DisambiguateProperties.TypeEdgeJson#compareTo
    fn compare_to(&self, x: &Self) -> Ordering {
        check_argument!(self.dest != x.dest);
        self.dest.wrapping_sub(x.dest).cmp(&0)
    }
}

impl LogsGsonObject for TypeEdgeJson {
    /// Gson's reflective adapter: the declared fields in order.
    fn write(&self, writer: &mut JsonWriter) {
        writer.begin_object();
        writer.name("dest");
        self.dest.write(writer);
        writer.name("value");
        self.value.name().write(writer);
        writer.end_object();
    }
}
