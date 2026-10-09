/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/disambiguate/AmbiguateProperties.java.

//! Port of `disambiguate/AmbiguateProperties.java`.

use super::{
    color_graph_builder::{ColorGraph, ColorGraphBuilder, EdgeReason},
    color_graph_node::{ColorGraphNodeId, DisambiguateArena},
    color_graph_node_factory::ColorGraphNodeFactory,
};
use crate::{
    AbstractCompiler,
    colors::{Color, color_registry::ColorRegistry, standard_colors},
    compiler_pass::CompilerPass,
    default_name_generator::DefaultNameGenerator,
    dot_formatter::DotFormatter,
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    graph::{
        adjacency_graph::AdjacencyGraph,
        annotatable::Annotatable,
        annotation::Annotation,
        fixed_point_graph_traversal::FixedPointGraphTraversal,
        graph_coloring::{GraphColoring, GreedyGraphColoring},
        graph_node::GraphNode,
        lowest_common_ancestor_finder::LowestCommonAncestorFinder,
        sub_graph::SubGraph,
    },
    name_generator::NameGenerator,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::java_util::bit_set::BitSet;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::{
    cmp::Ordering,
    sync::{Arc, RwLock},
};

/// Renames unrelated properties to the same name, using {@link Color}s provided by the
/// typechecker. This allows better compression as more properties can be given short names.
///
/// Properties are considered unrelated if they are never referenced from the same color or from a
/// subtype of each others' colors, thus this pass is only effective if type checking is enabled.
///
/// Example: <code>
///   Foo.fooprop = 0;
///   Foo.fooprop2 = 0;
///   Bar.barprop = 0;
/// </code> becomes: <code>
///   Foo.a = 0;
///   Foo.b = 0;
///   Bar.a = 0;
/// </code>
pub struct AmbiguateProperties {
    string_nodes_to_rename: Vec<NodeId>,
    // Can't use these to start property names.
    reserved_first_characters: IndexSet<u16>,
    // Can't use these at all in property names.
    reserved_non_first_characters: IndexSet<u16>,

    /// Map from property name to Property object
    property_map: IndexMap<JsString, Property>,

    /// Property names that don't get renamed
    externed_names: IndexSet<JsString>,

    /// Names to which properties shouldn't be renamed, to avoid name conflicts
    quoted_names: IndexSet<JsString>,

    color_registry: Arc<ColorRegistry>,

    /// Map from original property name to new name. Only used by tests.
    renaming_map: Option<IndexMap<JsString, JsString>>,

    graph_node_factory: Option<ColorGraphNodeFactory>,

    /// Rust-only: owns the `ColorGraphNode`s the factory creates.
    arena: DisambiguateArena,
}

/// Sorts Property objects by their count, breaking ties alphabetically to ensure a deterministic
/// total ordering.
// port: AmbiguateProperties#FREQUENCY_COMPARATOR
fn frequency_comparator(p1: &PropertyKey, p2: &PropertyKey) -> Ordering {
    if p1.num_occurrences != p2.num_occurrences {
        return p2.num_occurrences.wrapping_sub(p1.num_occurrences).cmp(&0);
    }
    p1.old_name.cmp(&p2.old_name)
}

impl AmbiguateProperties {
    // port: AmbiguateProperties#AmbiguateProperties
    pub fn new(
        compiler: &AbstractCompiler,
        reserved_first_characters: IndexSet<u16>,
        reserved_non_first_characters: IndexSet<u16>,
        extern_properties: &IndexSet<JsString>,
    ) -> Self {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        let mut externed_names = IndexSet::<_>::default();
        externed_names.insert(JsString::from("prototype"));
        externed_names.extend(extern_properties.iter().cloned());
        Self {
            string_nodes_to_rename: Vec::new(),
            reserved_first_characters,
            reserved_non_first_characters,
            property_map: IndexMap::<_, _>::default(),
            externed_names,
            quoted_names: IndexSet::<_>::default(),
            color_registry: Arc::clone(compiler.get_color_registry()),
            renaming_map: None,
            graph_node_factory: None,
            arena: DisambiguateArena::new(),
        }
    }

    // port: AmbiguateProperties#makePassForTesting
    pub fn make_pass_for_testing(
        compiler: &AbstractCompiler,
        reserved_first_characters: IndexSet<u16>,
        reserved_non_first_characters: IndexSet<u16>,
        extern_properties: &IndexSet<JsString>,
    ) -> AmbiguateProperties {
        let mut ap = AmbiguateProperties::new(
            compiler,
            reserved_first_characters,
            reserved_non_first_characters,
            extern_properties,
        );
        ap.renaming_map = Some(IndexMap::<_, _>::default());
        ap
    }

    // port: AmbiguateProperties#getRenamingMap
    pub fn get_renaming_map(&self) -> &IndexMap<JsString, JsString> {
        check_not_null!(self.renaming_map.as_ref())
    }

    /// Rust-only replay access to the `renamingMap` field (UnitRecorder reads it reflectively).
    pub fn replay_renaming_map(&self) -> Option<&IndexMap<JsString, JsString>> {
        self.renaming_map.as_ref()
    }
}

impl CompilerPass for AmbiguateProperties {
    // port: AmbiguateProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.graph_node_factory = Some(ColorGraphNodeFactory::create_factory(
            &mut self.arena,
            Arc::clone(&self.color_registry),
        ));

        // Find all property references and record the types on which they occur.
        // Populate stringNodesToRename, propertyMap, quotedNames.
        NodeTraversal::traverse(
            compiler,
            root,
            &mut ProcessPropertiesAndConstructors { outer: self },
        );

        let graph_node_factory = self.graph_node_factory.as_mut().unwrap();
        let mut graph_builder = ColorGraphBuilder::new(
            graph_node_factory,
            &mut self.arena,
            |_graph| LowestCommonAncestorFinder::new(),
            Arc::clone(&self.color_registry),
        );
        let all_known_types = graph_node_factory.get_all_known_types();
        graph_builder.add_all(graph_node_factory, &mut self.arena, all_known_types);
        let mut color_graph = graph_builder.build(graph_node_factory, &mut self.arena);
        {
            let mut chunk_graph_log =
                compiler.create_or_reopen_log("AmbiguateProperties", "color_graph.dot", &[]);
            chunk_graph_log.log_string(&DotFormatter::to_dot_graph(&color_graph));
            chunk_graph_log.close();
        }
        for node in graph_node_factory.get_all_known_types() {
            let index = node.get_index(&self.arena);
            node.get_subtype_indices_mut(&mut self.arena).set(index); // Init subtyping as reflexive.
        }

        let arena = &mut self.arena;
        FixedPointGraphTraversal::new_reverse_traversal(
            |_graph: &mut ColorGraph,
             subtype: ColorGraphNodeId,
             _e: EdgeReason,
             supertype: ColorGraphNodeId| {
                /*
                 * Cheap path for when we're sure there's going to be a change.
                 *
                 * <p>Since bits only ever turn on, using more bits means there are definitely more
                 * elements. This prevents of from needing to check cardinality or equality, which
                 * would otherwise dominate the cost of computing the fixed point.
                 *
                 * <p>We're guaranteed to converge because the sizes will be euqal after the OR
                 * operation.
                 */
                let (subtype_indices, supertype_indices) =
                    arena.subtype_indices_pair_mut(subtype, supertype);
                let Some(supertype_indices) = supertype_indices else {
                    // Java's BitSet#or returns at once for `this == set`; a node is never its
                    // own supertype here (ColorGraphBuilder adds no self-edges).
                    return false;
                };
                if subtype_indices.size() > supertype_indices.size() {
                    supertype_indices.or(subtype_indices);
                    return true;
                }

                let start_size = supertype_indices.cardinality();
                supertype_indices.or(subtype_indices);
                supertype_indices.cardinality() > start_size
            },
        )
        .compute_fixed_point(&mut color_graph);

        // Fill in all transitive edges in subtyping graph per property
        for prop in self.property_map.values_mut() {
            let Some(related_colors_seeds) = &prop.related_colors_seeds else {
                continue;
            };
            for color in related_colors_seeds.keys() {
                prop.related_colors
                    .or(color.get_subtype_indices(&self.arena));
            }
            prop.related_colors_seeds = None;
        }

        let mut reserved_names = IndexSet::<_>::default();
        reserved_names.extend(self.externed_names.iter().cloned());
        reserved_names.extend(self.quoted_names.iter().cloned());
        let mut num_renamed_property_names = 0;
        let mut num_skipped_property_names = 0;
        let mut nodes = Vec::with_capacity(self.property_map.len());
        for (index, prop) in self.property_map.values().enumerate() {
            if prop.skip_ambiguating {
                num_skipped_property_names += 1;
                reserved_names.insert(prop.old_name.clone());
            } else {
                num_renamed_property_names += 1;
                nodes.push(PropertyGraphNode::new(PropertyKey {
                    index,
                    num_occurrences: prop.num_occurrences,
                    old_name: prop.old_name.clone(),
                }));
            }
        }
        let final_num_renamed_property_names = num_renamed_property_names;
        let final_num_skipped_property_names = num_skipped_property_names;

        let mut property_graph = PropertyGraph::new(nodes, &self.property_map);
        let mut coloring: GreedyGraphColoring<PropertyKey, ()> =
            GreedyGraphColoring::with_tie_breaker(Some(Box::new(frequency_comparator)));
        let num_new_property_names = coloring.color(&mut property_graph);

        // Generate new names for the properties that will be renamed.
        let mut name_gen = DefaultNameGenerator::with_first_and_non_first_characters(
            Arc::new(RwLock::new(reserved_names)),
            JsString::from(""),
            &self.reserved_first_characters,
            &self.reserved_non_first_characters,
        );
        let mut color_map: Vec<JsString> = Vec::with_capacity(num_new_property_names as usize);
        for _i in 0..num_new_property_names {
            color_map.push(name_gen.generate_next_name());
        }

        // Translate the color of each Property instance to a name.
        let new_names = property_graph
            .get_nodes()
            .into_iter()
            .map(|node| {
                let color = node.get_annotation(&property_graph).unwrap().hash_code();
                (node.get_value(&property_graph).index, color)
            })
            .collect::<Vec<_>>();
        drop(property_graph);
        for (index, color) in new_names {
            let (_, prop) = self.property_map.get_index_mut(index).unwrap();
            prop.new_name = Some(color_map[color as usize].clone());
            if let Some(renaming_map) = &mut self.renaming_map {
                renaming_map.insert(prop.old_name.clone(), prop.new_name.clone().unwrap());
            }
        }

        // Actually assign the new names to the relevant STRING nodes in the AST.
        for n in self.string_nodes_to_rename.iter().copied() {
            let old_name = n.get_string(compiler);
            let p = self.property_map.get(&old_name);
            if let Some(p) = p
                && let Some(new_name) = &p.new_name
            {
                check_state!(old_name == p.old_name);
                if *new_name != old_name {
                    n.set_string(compiler, new_name.clone());
                    compiler.report_change_to_enclosing_scope(n);
                }
            }
        }

        // We may have renamed getter / setter properties.
        // TODO(b/161947315): this shouldn't be the responsibility of AmbiguateProperties
        GatherGetterAndSetterProperties::update(compiler, externs, root);

        let summary_supplier = || {
            format!(
                "Collapsed {final_num_renamed_property_names} properties into \
                 {num_new_property_names} and skipped renaming \
                 {final_num_skipped_property_names} properties."
            )
        };
        // logger.isLoggable(Level.FINE) is false: java.util.logging's default level is INFO.
        compiler.report_ambiguate_properties_summary(summary_supplier);
    }
}

/// The value of a [`PropertyGraph`] node: Java's `Property` object, identified by its index in
/// `propertyMap`, with the immutable fields the coloring's comparators read.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PropertyKey {
    index: usize,
    num_occurrences: i32,
    old_name: JsString,
}

// port: AmbiguateProperties.PropertyGraph
struct PropertyGraph<'a> {
    nodes: Vec<PropertyGraphNode>,
    properties: &'a IndexMap<JsString, Property>,
}

impl<'a> PropertyGraph<'a> {
    // port: AmbiguateProperties.PropertyGraph#PropertyGraph
    fn new(nodes: Vec<PropertyGraphNode>, properties: &'a IndexMap<JsString, Property>) -> Self {
        Self { nodes, properties }
    }

    fn related_colors(&self, prop: &PropertyKey) -> &BitSet {
        &self.properties[prop.index].related_colors
    }
}

/// Handle of a [`PropertyGraphNode`] in its [`PropertyGraph`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct PropertyGraphNodeId(usize);

impl<'a> AdjacencyGraph<PropertyKey, ()> for PropertyGraph<'a> {
    type Node = PropertyGraphNodeId;

    // port: AmbiguateProperties.PropertyGraph#getNodes
    fn get_nodes(&self) -> Vec<PropertyGraphNodeId> {
        (0..self.nodes.len()).map(PropertyGraphNodeId).collect()
    }

    // port: AmbiguateProperties.PropertyGraph#getNodeCount
    fn get_node_count(&self) -> usize {
        self.nodes.len()
    }

    // port: AmbiguateProperties.PropertyGraph#getNode
    fn get_node(&self, _property: &PropertyKey) -> Option<PropertyGraphNodeId> {
        panic!("PropertyGraph#getNode is never called.");
    }

    // port: AmbiguateProperties.PropertyGraph#newSubGraph
    fn new_sub_graph(&self) -> Box<dyn SubGraph<PropertyKey, (), Self>> {
        Box::new(PropertySubGraph::new())
    }

    // port: AmbiguateProperties.PropertyGraph#clearNodeAnnotations
    fn clear_node_annotations(&mut self) {
        for node in &mut self.nodes {
            node.annotation = None;
        }
    }

    // port: AmbiguateProperties.PropertyGraph#getWeight
    fn get_weight(&self, value: &PropertyKey) -> i32 {
        value.num_occurrences
    }

    fn node_value(&self, node: PropertyGraphNodeId) -> &PropertyKey {
        &self.nodes[node.0].property
    }

    fn node_annotation(&self, node: PropertyGraphNodeId) -> &Option<Box<dyn Annotation>> {
        &self.nodes[node.0].annotation
    }

    fn node_annotation_mut(
        &mut self,
        node: PropertyGraphNodeId,
    ) -> &mut Option<Box<dyn Annotation>> {
        &mut self.nodes[node.0].annotation
    }
}

impl<'a> Annotatable<PropertyKey, (), PropertyGraph<'a>> for PropertyGraphNodeId {
    // port: AmbiguateProperties.PropertyGraphNode#getAnnotation
    fn get_annotation<'g>(self, graph: &'g PropertyGraph<'a>) -> Option<&'g dyn Annotation> {
        graph.node_annotation(self).as_deref()
    }

    // port: AmbiguateProperties.PropertyGraphNode#setAnnotation
    fn set_annotation(self, graph: &mut PropertyGraph<'a>, data: Option<Box<dyn Annotation>>) {
        *graph.node_annotation_mut(self) = data;
    }

    fn take_annotation(self, graph: &mut PropertyGraph<'a>) -> Option<Box<dyn Annotation>> {
        graph.node_annotation_mut(self).take()
    }

    // port: AmbiguateProperties.PropertyGraphNode#getAnnotation
    fn get_annotation_as_mut<'g, A: Annotation>(
        self,
        graph: &'g mut PropertyGraph<'a>,
    ) -> Option<&'g mut A> {
        graph.node_annotation_mut(self).as_deref_mut().map(|a| {
            a.as_any_mut()
                .downcast_mut::<A>()
                .expect("ClassCastException")
        })
    }
}

impl<'a> GraphNode<PropertyKey, (), PropertyGraph<'a>> for PropertyGraphNodeId {
    // port: AmbiguateProperties.PropertyGraphNode#getValue
    fn get_value<'g>(self, graph: &'g PropertyGraph<'a>) -> &'g PropertyKey {
        graph.node_value(self)
    }
}

/// A {@link SubGraph} that represents properties. The related types of the properties are used
/// to efficiently calculate adjacency information.
// port: AmbiguateProperties.PropertySubGraph
struct PropertySubGraph {
    /// Types related to properties referenced in this subgraph.
    related_types: BitSet,
}

impl PropertySubGraph {
    fn new() -> Self {
        Self {
            related_types: BitSet::new_default(),
        }
    }
}

impl<'a> SubGraph<PropertyKey, (), PropertyGraph<'a>> for PropertySubGraph {
    /// Returns true if prop is in an independent set from all properties in this sub graph. That
    /// is, if none of its related types intersects with the related types for this sub graph.
    // port: AmbiguateProperties.PropertySubGraph#isIndependentOf
    fn is_independent_of(&self, graph: &PropertyGraph<'a>, prop: &PropertyKey) -> bool {
        !self.related_types.intersects(graph.related_colors(prop))
    }

    /// Adds the node to the sub graph, adding all its related types to the related types for the
    /// sub graph.
    // port: AmbiguateProperties.PropertySubGraph#addNode
    fn add_node(&mut self, graph: &PropertyGraph<'a>, prop: &PropertyKey) {
        self.related_types.or(graph.related_colors(prop));
    }
}

// port: AmbiguateProperties.PropertyGraphNode
struct PropertyGraphNode {
    property: PropertyKey,
    annotation: Option<Box<dyn Annotation>>,
}

impl PropertyGraphNode {
    // port: AmbiguateProperties.PropertyGraphNode#PropertyGraphNode
    fn new(property: PropertyKey) -> Self {
        Self {
            property,
            annotation: None,
        }
    }
}

/// Finds all property references, recording the types on which they occur, and records all
/// constructors and their instance types in the {@link ColorGraphNodeFactory}.
// port: AmbiguateProperties.ProcessPropertiesAndConstructors
struct ProcessPropertiesAndConstructors<'a> {
    outer: &'a mut AmbiguateProperties,
}

impl Callback for ProcessPropertiesAndConstructors<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                self.process_get_prop(t, n);
            }
            Token::CALL => {
                self.process_call(t, n);
            }
            // handle ES5-style classes
            Token::NAME
                if NodeUtil::is_name_declaration(t, parent) || parent.unwrap().is_function(t) =>
            {
                let color = AmbiguateProperties::get_color(t, n);
                self.outer.create_node(&color);
            }
            Token::OBJECTLIT | Token::OBJECT_PATTERN => {
                self.process_object_lit_or_pattern(t, n);
            }
            Token::GETELEM => {
                self.process_get_elem(t, n);
            }
            Token::CLASS => {
                self.process_class(t, n);
            }
            _ => {
                // Nothing to do.
            }
        }
    }
}

impl ProcessPropertiesAndConstructors<'_> {
    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#processGetProp
    fn process_get_prop(&mut self, ast: &Ast, get_prop: NodeId) {
        let r#type = AmbiguateProperties::get_color(ast, get_prop.get_first_child(ast).unwrap());
        self.maybe_mark_candidate(ast, get_prop, &r#type);
        if NodeUtil::is_lhs_of_assign(ast, get_prop)
            || NodeUtil::is_statement(ast, get_prop.get_parent(ast).unwrap())
        {
            self.outer.create_node(&r#type);
        }
    }

    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#processCall
    fn process_call(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let target = call.get_first_child(t).unwrap();
        if !target.is_qualified_name(t) {
            return;
        }

        let compiler = t.get_compiler();
        if compiler
            .get_coding_convention()
            .is_property_rename_function(compiler, target)
        {
            let prop_name = call.get_second_child(t);
            let Some(prop_name) = prop_name.filter(|p| p.is_string_lit(t)) else {
                return;
            };

            // Skip ambiguation for properties in renaming calls
            // NOTE (lharker@) - I'm not sure if this behavior is necessary, or if we could safely
            // ambiguate the property as long as we also updated the property renaming call
            let p = self.outer.get_property(prop_name.get_string(t));
            p.skip_ambiguating = true;
        } else if NodeUtil::is_object_define_properties_definition(t, call) {
            let type_obj = call.get_second_child(t).unwrap();
            let r#type = AmbiguateProperties::get_color(t, type_obj);
            let object_literal = type_obj.get_next(t).unwrap();

            if !object_literal.is_object_lit(t) {
                return;
            }

            let mut key = object_literal.get_first_child(t);
            while let Some(k) = key {
                self.process_object_property(t, object_literal, k, &r#type);
                key = k.get_next(t);
            }
        }
    }

    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#processObjectProperty
    fn process_object_property(
        &mut self,
        ast: &Ast,
        object_lit: NodeId,
        key: NodeId,
        r#type: &Color,
    ) {
        check_argument!(
            object_lit.is_object_lit(ast) || object_lit.is_object_pattern(ast),
            "%s",
            object_lit.to_string(ast)
        );
        match key.get_token(ast) {
            Token::COMPUTED_PROP => {
                if key.get_first_child(ast).unwrap().is_string_lit(ast) {
                    // If this quoted prop name is statically determinable, ensure we don't rename
                    // some other property in a way that could conflict with it.
                    //
                    // This is largely because we store quoted member functions as computed
                    // properties and want to be consistent with how other quoted properties
                    // invalidate property names.
                    self.outer
                        .quoted_names
                        .insert(key.get_first_child(ast).unwrap().get_string(ast));
                }
            }
            Token::MEMBER_FUNCTION_DEF
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::STRING_KEY => {
                if key.is_quoted_string_key(ast) {
                    // If this quoted prop name is statically determinable, ensure we don't rename
                    // some other property in a way that could conflict with it
                    self.outer.quoted_names.insert(key.get_string(ast));
                } else {
                    self.maybe_mark_candidate(ast, key, r#type);
                }
            }
            Token::OBJECT_REST | Token::OBJECT_SPREAD => {
                // Nothing to do.
            }
            _ => panic!(
                "Unexpected child of {}: {}",
                object_lit.get_token(ast),
                key.to_string_tree(ast)
            ),
        }
    }

    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#processObjectLitOrPattern
    fn process_object_lit_or_pattern(&mut self, ast: &Ast, object_lit: NodeId) {
        // Object.defineProperties literals are handled at the CALL node, as we determine the type
        // differently than for regular object literals.
        let parent = object_lit.get_parent(ast).unwrap();
        if parent.is_call(ast) && NodeUtil::is_object_define_properties_definition(ast, parent) {
            return;
        }

        // The children of an OBJECTLIT node are keys, where the values
        // are the children of the keys.
        let r#type = AmbiguateProperties::get_color(ast, object_lit);
        let mut key = object_lit.get_first_child(ast);
        while let Some(k) = key {
            self.process_object_property(ast, object_lit, k, &r#type);
            key = k.get_next(ast);
        }
    }

    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#processGetElem
    fn process_get_elem(&mut self, ast: &Ast, n: NodeId) {
        // If this is a quoted property access (e.g. x['myprop']), we need to
        // ensure that we never rename some other property in a way that
        // could conflict with this quoted name.
        let child = n.get_last_child(ast).unwrap();
        if child.is_string_lit(ast) {
            self.outer.quoted_names.insert(child.get_string(ast));
        }
    }

    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#processClass
    fn process_class(&mut self, ast: &Ast, class_node: NodeId) {
        let class_constructor_type = AmbiguateProperties::get_color(ast, class_node);
        self.outer.create_node(&class_constructor_type);
        // In theory all CLASS colors should be a function with a known prototype, but in
        // practice typecasts mean that this is not always the case.

        let possible_prototypes = class_constructor_type.get_prototypes();
        let class_prototype = if possible_prototypes.is_empty() {
            standard_colors::UNKNOWN.clone()
        } else {
            Color::create_union(possible_prototypes)
        };
        let mut member = NodeUtil::get_class_members(ast, class_node).get_first_child(ast);
        while let Some(m) = member {
            member = m.get_next(ast);
            if m.is_quoted_string_key(ast) {
                // ignore get 'foo'() {} and prevent property name collisions
                // Note that only getters/setters are represented as quoted strings, not 'foo'() {}
                // see https://github.com/google/closure-compiler/issues/3071
                self.outer.quoted_names.insert(m.get_string(ast));
                continue;
            } else if m.is_computed_prop(ast) || m.is_computed_field_def(ast) {
                // ignore ['foo']() {}
                // for simple cases, we also prevent renaming collisions
                if m.get_first_child(ast).unwrap().is_string_lit(ast) {
                    self.outer
                        .quoted_names
                        .insert(m.get_first_child(ast).unwrap().get_string(ast));
                }
                continue;
            } else if NodeUtil::is_es6_constructor_member_function_def(ast, m) {
                // don't rename `class C { constructor() {} }` !
                // This only applies for ES6 classes, not generic properties called 'constructor',
                // which is why it's handled in this method specifically.
                continue;
            } else if m.is_block(ast) {
                // ES2022 static initialization blocks don't have names so can't be renamed.
                // Example: `class C { static { alert('foo'); } }`
                continue;
            }

            let member_owner_color = if m.is_static_member(ast) {
                class_constructor_type.clone()
            } else if m.is_member_field_def(ast) {
                let possible_instances = class_constructor_type.get_instance_colors();
                if possible_instances.is_empty() {
                    standard_colors::UNKNOWN.clone()
                } else {
                    Color::create_union(possible_instances)
                }
            } else {
                check_state!(
                    m.is_member_function_def(ast) || m.is_getter_def(ast) || m.is_setter_def(ast)
                );
                class_prototype.clone()
            };
            // member could be a MEMBER_FUNCTION_DEF, MEMBER_FIELD_DEF, GETTER_DEF, or SETTER_DEF
            self.maybe_mark_candidate(ast, m, &member_owner_color);
        }
    }

    /// If a property node is eligible for renaming, stashes a reference to it and increments the
    /// property name's access count.
    ///
    /// @param n The STRING node for a property
    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#maybeMarkCandidate
    fn maybe_mark_candidate(&mut self, ast: &Ast, n: NodeId, r#type: &Color) {
        let name = n.get_string(ast);
        if !self.outer.externed_names.contains(&name) {
            self.outer.string_nodes_to_rename.push(n);
            self.record_property(name, r#type);
        }
    }

    // port: AmbiguateProperties.ProcessPropertiesAndConstructors#recordProperty
    fn record_property(&mut self, name: JsString, color: &Color) {
        let outer = &mut *self.outer;
        let prop = outer
            .property_map
            .entry(name.clone())
            .or_insert_with(|| Property::new(name));
        prop.add_related_color(
            outer.graph_node_factory.as_mut().unwrap(),
            &mut outer.arena,
            color,
        );
    }
}

impl AmbiguateProperties {
    // port: AmbiguateProperties#getProperty
    fn get_property(&mut self, name: JsString) -> &mut Property {
        self.property_map
            .entry(name.clone())
            .or_insert_with(|| Property::new(name))
    }

    /// This method gets the Color from the Node argument or UNKNOWN if not present.
    // port: AmbiguateProperties#getColor
    fn get_color(ast: &Ast, n: NodeId) -> Color {
        let r#type = n.get_color(ast);
        match r#type {
            // TODO(bradfordcsmith): This branch indicates a compiler bug. It should throw an
            // exception.
            None => standard_colors::UNKNOWN.clone(),
            Some(r#type) => r#type,
        }
    }

    /// Java `graphNodeFactory.createNode(color)` from the inner classes.
    fn create_node(&mut self, color: &Color) -> ColorGraphNodeId {
        self.graph_node_factory
            .as_mut()
            .unwrap()
            .create_node(&mut self.arena, Some(color))
    }
}

/// Encapsulates the information needed for renaming a property.
// port: AmbiguateProperties.Property
struct Property {
    old_name: JsString,
    new_name: Option<JsString>,
    num_occurrences: i32,
    skip_ambiguating: bool,
    // All colors upon which this property was directly accessed. For "a.b" this includes "a"'s
    // type
    related_colors_seeds: Option<IndexMap<ColorGraphNodeId, i32>>,
    // includes relatedTypesSeeds + all subtypes of those seed colors. For example if this property
    // was accessed off of Iterable, then this bitset will include Array as well.
    related_colors: BitSet,
}

impl Property {
    // port: AmbiguateProperties.Property#Property
    fn new(name: JsString) -> Self {
        Self {
            old_name: name,
            new_name: None,
            num_occurrences: 0,
            skip_ambiguating: false,
            related_colors_seeds: None,
            related_colors: BitSet::new_default(),
        }
    }

    /// Marks this color as related to this property
    // port: AmbiguateProperties.Property#addRelatedColor
    fn add_related_color(
        &mut self,
        graph_node_factory: &mut ColorGraphNodeFactory,
        arena: &mut DisambiguateArena,
        color: &Color,
    ) {
        if self.skip_ambiguating {
            return;
        }

        self.num_occurrences += 1;

        if color.is_invalidating() || color.get_properties_keep_original_name() {
            self.skip_ambiguating = true;
            return;
        }

        if self.related_colors_seeds.is_none() {
            self.related_colors_seeds = Some(IndexMap::<_, _>::default());
        }

        let new_color_graph_node = graph_node_factory.create_node(arena, Some(color));
        self.related_colors_seeds
            .as_mut()
            .unwrap()
            .insert(new_color_graph_node, 0);
    }
}
