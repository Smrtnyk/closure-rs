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
//   src/com/google/javascript/jscomp/disambiguate/ColorFindPropertyReferences.java.

//! Port of `disambiguate/ColorFindPropertyReferences.java`.

use super::{
    color_graph_node::{DisambiguateArena, PropAssociation, PropertyClusteringId},
    color_graph_node_factory::ColorGraphNodeFactoryMethods,
    property_clustering::PropertyClustering,
};
use crate::{
    AbstractCompiler,
    colors::{Color, standard_colors},
    graph::union_find::UnionFind,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

/// Tests whether the named JS function is a "property reflector"; a function that treats a string
/// literal as a property name.
///
/// Such a function is assumed to have the following signature:
///
/// 1. A string literal that is the name being reflected.
/// 2. An object on which the property is referenced. (Optional)
/// 3. Var args.
///
/// Java `boolean test(Node nameNode)`; the compiler the traversal runs on is passed along so a
/// reflector can consult its coding convention.
pub type IsPropertyReflector<'a> = Box<dyn Fn(&AbstractCompiler, NodeId) -> bool + 'a>;

/// Traverses the AST, collecting connections between {@link JSType}s, property accesses, and
/// their accociated {@link Node}s.
///
/// Also collects declarations of constructors even if no property accesses are visible.
///
/// This callback is intended for both source and externs.
pub struct ColorFindPropertyReferences<'a> {
    prop_index: Option<IndexMap<JsString, PropertyClusteringId>>,
    color_graph_node_factory: &'a mut dyn ColorGraphNodeFactoryMethods,
    arena: &'a mut DisambiguateArena,
    is_property_reflector: IsPropertyReflector<'a>,
}

impl<'a> ColorFindPropertyReferences<'a> {
    // port: ColorFindPropertyReferences#ColorFindPropertyReferences
    pub fn new(
        color_graph_node_factory: &'a mut dyn ColorGraphNodeFactoryMethods,
        arena: &'a mut DisambiguateArena,
        is_property_reflector: IsPropertyReflector<'a>,
    ) -> Self {
        Self {
            prop_index: Some(IndexMap::<_, _>::default()),
            color_graph_node_factory,
            arena,
            is_property_reflector,
        }
    }

    // port: ColorFindPropertyReferences#getPropertyIndex
    pub fn get_property_index(&mut self) -> IndexMap<JsString, PropertyClusteringId> {
        self.prop_index.take().unwrap()
    }

    // port: ColorFindPropertyReferences#handleObjectLit
    fn handle_object_lit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        // Object.defineProperties literals are handled at the CALL node.
        let parent = n.get_parent(t).unwrap();
        if parent.is_call(t) && NodeUtil::is_object_define_properties_definition(t, parent) {
            return;
        }

        let owner = n.get_color(t);
        self.traverse_objectlit_like(t, n, &|_ast, _m| owner.clone());
    }

    /// Examines calls in case they are Object.defineProperties calls
    // port: ColorFindPropertyReferences#handleCall
    fn handle_call(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let target = call.get_first_child(t).unwrap();
        if !target.is_qualified_name(t) {
            return;
        }

        if (self.is_property_reflector)(t.get_compiler(), target) {
            self.handle_property_reflector_call(t, call);
        } else if NodeUtil::is_object_define_properties_definition(t, call) {
            self.handle_object_define_properties(t, call);
        }
    }

    // port: ColorFindPropertyReferences#handleClass
    fn handle_class(&mut self, t: &mut NodeTraversal<'_>, class_node: NodeId) {
        let class_type = class_node.get_color(t);
        let class_members = NodeUtil::get_class_members(t, class_node);
        self.traverse_objectlit_like(t, class_members, &|ast, m| {
            if m.is_static_member(ast) {
                class_type.clone()
            } else if m.is_member_field_def(ast) {
                let class_instance_type = class_type.as_ref().unwrap().get_instance_colors();
                Some(if class_instance_type.is_empty() {
                    standard_colors::UNKNOWN.clone()
                } else {
                    Color::create_union(class_instance_type)
                })
            } else {
                check_state!(
                    m.is_member_function_def(ast) || m.is_getter_def(ast) || m.is_setter_def(ast),
                    "%s",
                    m.to_string(ast)
                );
                let class_prototype_type = class_type.as_ref().unwrap().get_prototypes();
                Some(if class_prototype_type.is_empty() {
                    standard_colors::UNKNOWN.clone()
                } else {
                    Color::create_union(class_prototype_type)
                })
            }
        });
        self.color_graph_node_factory
            .create_node(self.arena, class_type.as_ref());
    }

    // port: ColorFindPropertyReferences#handleFunction
    fn handle_function(&mut self, ast: &Ast, fn_node: NodeId) {
        // Ensure the flattener knows about the class constructor and instance. Even if we don't
        // see any direct property references off either type, it's possible that a property is
        // referenced via a superclass or implemented interface.
        // e.g. `const /** !FooInterface */ x = new Foo(); x.method();`
        let fn_type = fn_node.get_color(ast);
        if let Some(fn_type) = &fn_type
            && !fn_type.get_instance_colors().is_empty()
        {
            self.color_graph_node_factory
                .create_node(self.arena, Some(fn_type));
        }
    }

    // port: ColorFindPropertyReferences#handleObjectPattern
    fn handle_object_pattern(&mut self, t: &mut NodeTraversal<'_>, pattern: NodeId) {
        let owner = pattern.get_color(t);
        self.traverse_objectlit_like(t, pattern, &|_ast, _m| owner.clone());
    }

    // port: ColorFindPropertyReferences#handlePropertyReflectorCall
    fn handle_property_reflector_call(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let name = call.get_second_child(t);
        let Some(name) = name.filter(|name| name.is_string_lit(t)) else {
            return;
        };

        let obj = name.get_next(t);
        let obj_color = obj.and_then(|obj| obj.get_color(t));
        self.register_property_use(t, name, obj_color);
    }

    // port: ColorFindPropertyReferences#handleObjectDefineProperties
    fn handle_object_define_properties(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let type_obj = call.get_second_child(t).unwrap();
        let object_literal = type_obj.get_next(t).unwrap();
        if !object_literal.is_object_lit(t) {
            return;
        }

        let r#type = type_obj.get_color(t);
        self.traverse_objectlit_like(t, object_literal, &|_ast, _m| r#type.clone());
    }

    // port: ColorFindPropertyReferences#traverseObjectlitLike
    fn traverse_objectlit_like(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        member_owner_fn: &dyn Fn(&Ast, NodeId) -> Option<Color>,
    ) {
        // The keys in an object pattern are r-values, not l-values, but they are still accesses.
        check_state!(n.is_object_lit(t) || n.is_object_pattern(t) || n.is_class_members(t));

        let mut child = n.get_first_child(t);
        while let Some(c) = child {
            match c.get_token(t) {
                Token::BLOCK
                | Token::COMPUTED_PROP
                | Token::COMPUTED_FIELD_DEF
                | Token::OBJECT_REST
                | Token::OBJECT_SPREAD => {
                    child = c.get_next(t);
                    continue;
                }
                Token::STRING_KEY
                | Token::MEMBER_FUNCTION_DEF
                | Token::MEMBER_FIELD_DEF
                | Token::GETTER_DEF
                | Token::SETTER_DEF => {
                    if c.is_quoted_string_key(t) {
                        child = c.get_next(t);
                        continue; // These won't be renamed due to our assumptions. Ignore them.
                    }

                    let owner = member_owner_fn(t, c);
                    self.register_property_use(t, c, owner);
                }
                _ => panic!(
                    "Unexpected child of {}: {}",
                    n.get_token(t),
                    c.to_string_tree(t)
                ),
            }
            child = c.get_next(t);
        }
    }

    /// Update all datastructures as necessary to consider property use {@code site} from type
    /// {@code owner}.
    // port: ColorFindPropertyReferences#registerPropertyUse
    fn register_property_use(
        &mut self,
        t: &mut NodeTraversal<'_>,
        site: NodeId,
        owner: Option<Color>,
    ) {
        let site_string = site.get_string(t);
        let prop_index = self.prop_index.as_mut().unwrap();
        let prop = match prop_index.get(&site_string) {
            Some(prop) => *prop,
            None => {
                let prop = PropertyClustering::new(self.arena, site_string.clone());
                prop_index.insert(site_string, prop);
                prop
            }
        };
        let flat_owner = self
            .color_graph_node_factory
            .create_node(self.arena, owner.as_ref());

        // Set the initial condition for flowing this property along the graph.
        flat_owner
            .get_associated_props_mut(self.arena)
            .insert(prop, PropAssociation::AST);
        // Make sure there's a cluster for this name/type combination.
        prop.get_clusters_mut(self.arena).add(flat_owner);
        // Record the site to rename once clusters are found. If it's an extern, we won't rename
        // anyway.
        prop.get_use_sites_mut(self.arena).insert(site, flat_owner);

        // Track the cluster of types whose properties must keep their original name after
        // disambiguation. Note: an "enum type" is the type of an enum object like
        // "{STOP: 0, GO: 1}".
        // NOTE: we can't use site.isFromExterns() because sometimes nodes have source file
        // information that doesn't match the containing script. This could lead to renaming
        // properties that are in externs if the property node didn't have an externs source file.
        // Related: b/186056977.
        if t.get_current_script().unwrap().is_from_externs(t)
            || owner
                .as_ref()
                .is_some_and(|owner| owner.get_properties_keep_original_name())
        {
            prop.register_original_name_type(self.arena, flat_owner);
        }
    }
}

impl Callback for ColorFindPropertyReferences<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ColorFindPropertyReferences#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                let owner = n.get_first_child(t).unwrap().get_color(t);
                self.register_property_use(t, n, owner)
            }
            Token::OBJECTLIT => self.handle_object_lit(t, n),
            Token::CALL => self.handle_call(t, n),
            Token::CLASS => self.handle_class(t, n),
            Token::OBJECT_PATTERN => self.handle_object_pattern(t, n),
            Token::FUNCTION => self.handle_function(t, n),
            _ => {}
        }
    }
}
