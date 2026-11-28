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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/disambiguate/ColorFindPropertyReferences.java.

//! Port of `disambiguate/ColorFindPropertyReferences.java`.

use super::{
    color_graph_node::{
        ColorGraphNodeId, DisambiguateArena, PropAssociation, PropertyClusteringId,
    },
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

struct PropertyUse {
    property: PropertyClusteringId,
    owner: ColorGraphNodeId,
    site: NodeId,
    keep_original: bool,
}

impl PropertyUse {
    fn record(self, arena: &mut DisambiguateArena) {
        self.property.get_clusters_mut(arena).add(self.owner);
        self.property
            .get_use_sites_mut(arena)
            .insert(self.site, self.owner);
        if self.keep_original {
            self.property.register_original_name_type(arena, self.owner);
        }
    }
}

/// Traverses the AST, collecting connections between {@link JSType}s, property accesses, and
/// their accociated {@link Node}s.
///
/// Also collects declarations of constructors even if no property accesses are visible.
///
/// This callback is intended for both source and externs.
pub struct ColorFindPropertyReferences<'a> {
    prop_index: Option<IndexMap<JsString, PropertyClusteringId>>,
    pending_uses: Option<Vec<PropertyUse>>,
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
            pending_uses: None,
            color_graph_node_factory,
            arena,
            is_property_reflector,
        }
    }

    /// Rust-only (performance divergence): room for `names` property names in the index and the
    /// arena, so that they are not copied each time they double; only capacity changes.
    pub(super) fn reserve_property_names(&mut self, names: usize) {
        self.prop_index.as_mut().unwrap().reserve(names);
        self.arena.props.reserve(names);
    }

    /// The ordinary disambiguation pass only consumes renames visible in source. Defer
    /// collecting cluster members until every script has supplied its property uses.
    pub(super) fn defer_extern_only_renames(&mut self) {
        self.pending_uses = Some(Vec::new());
    }

    // port: ColorFindPropertyReferences#getPropertyIndex
    pub fn get_property_index(&mut self) -> IndexMap<JsString, PropertyClusteringId> {
        if let Some(uses) = self.pending_uses.take() {
            for usage in uses {
                if !usage.property.is_extern_only(self.arena) {
                    usage.record(self.arena);
                }
            }
        }
        self.prop_index.take().unwrap()
    }

    /// Rust-only (performance divergence): walk `root` directly in post-order without creating
    /// a `NodeTraversal`, tracking `from_externs` once per `SCRIPT`.
    pub(super) fn traverse_ast(&mut self, compiler: &AbstractCompiler, root: NodeId) {
        self.traverse_subtree(compiler, root, false);
    }

    fn traverse_subtree(&mut self, compiler: &AbstractCompiler, n: NodeId, from_externs: bool) {
        let token = n.get_token(compiler);
        let from_externs = if token == Token::SCRIPT {
            n.is_from_externs(compiler)
        } else {
            from_externs
        };
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            self.traverse_subtree(compiler, c, from_externs);
            child = c.get_next(compiler);
        }
        self.visit_node(compiler, from_externs, n, token);
    }

    #[inline]
    fn visit_node(
        &mut self,
        compiler: &AbstractCompiler,
        from_externs: bool,
        n: NodeId,
        token: Token,
    ) {
        match token {
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                // Rust-only (performance divergence): the owner's color is lent, not copied.
                let owner = n.get_first_child(compiler).unwrap().get_color_ref(compiler);
                self.register_property_use_direct(compiler, from_externs, n, owner)
            }
            Token::OBJECTLIT => self.handle_object_lit(compiler, from_externs, n),
            Token::CALL => self.handle_call(compiler, from_externs, n),
            Token::CLASS => self.handle_class(compiler, from_externs, n),
            Token::OBJECT_PATTERN => self.handle_object_pattern(compiler, from_externs, n),
            Token::FUNCTION => self.handle_function(compiler, n),
            _ => {}
        }
    }

    // port: ColorFindPropertyReferences#handleObjectLit
    fn handle_object_lit(&mut self, compiler: &AbstractCompiler, from_externs: bool, n: NodeId) {
        // Object.defineProperties literals are handled at the CALL node.
        let parent = n.get_parent(compiler).unwrap();
        if parent.is_call(compiler)
            && NodeUtil::is_object_define_properties_definition(compiler, parent)
        {
            return;
        }

        let owner = n.get_color(compiler);
        self.traverse_objectlit_like(compiler, from_externs, n, &|_ast, _m| owner.clone());
    }

    /// Examines calls in case they are Object.defineProperties calls
    // port: ColorFindPropertyReferences#handleCall
    fn handle_call(&mut self, compiler: &AbstractCompiler, from_externs: bool, call: NodeId) {
        let target = call.get_first_child(compiler).unwrap();
        if !target.is_qualified_name(compiler) {
            return;
        }

        if (self.is_property_reflector)(compiler, target) {
            self.handle_property_reflector_call(compiler, from_externs, call);
        } else if NodeUtil::is_object_define_properties_definition(compiler, call) {
            self.handle_object_define_properties(compiler, from_externs, call);
        }
    }

    // port: ColorFindPropertyReferences#handleClass
    fn handle_class(
        &mut self,
        compiler: &AbstractCompiler,
        from_externs: bool,
        class_node: NodeId,
    ) {
        let class_type = class_node.get_color(compiler);
        let class_members = NodeUtil::get_class_members(compiler, class_node);
        self.traverse_objectlit_like(compiler, from_externs, class_members, &|ast, m| {
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
        let fn_type = fn_node.get_color_ref(ast);
        if let Some(fn_type) = fn_type
            && !fn_type.get_instance_colors().is_empty()
        {
            self.color_graph_node_factory
                .create_node(self.arena, Some(fn_type));
        }
    }

    // port: ColorFindPropertyReferences#handleObjectPattern
    fn handle_object_pattern(
        &mut self,
        compiler: &AbstractCompiler,
        from_externs: bool,
        pattern: NodeId,
    ) {
        let owner = pattern.get_color(compiler);
        self.traverse_objectlit_like(compiler, from_externs, pattern, &|_ast, _m| owner.clone());
    }

    // port: ColorFindPropertyReferences#handlePropertyReflectorCall
    fn handle_property_reflector_call(
        &mut self,
        compiler: &AbstractCompiler,
        from_externs: bool,
        call: NodeId,
    ) {
        let name = call.get_second_child(compiler);
        let Some(name) = name.filter(|name| name.is_string_lit(compiler)) else {
            return;
        };

        let obj = name.get_next(compiler);
        let obj_color = obj.and_then(|obj| obj.get_color_ref(compiler));
        self.register_property_use_direct(compiler, from_externs, name, obj_color);
    }

    // port: ColorFindPropertyReferences#handleObjectDefineProperties
    fn handle_object_define_properties(
        &mut self,
        compiler: &AbstractCompiler,
        from_externs: bool,
        call: NodeId,
    ) {
        let type_obj = call.get_second_child(compiler).unwrap();
        let object_literal = type_obj.get_next(compiler).unwrap();
        if !object_literal.is_object_lit(compiler) {
            return;
        }

        let r#type = type_obj.get_color(compiler);
        self.traverse_objectlit_like(compiler, from_externs, object_literal, &|_ast, _m| {
            r#type.clone()
        });
    }

    // port: ColorFindPropertyReferences#traverseObjectlitLike
    fn traverse_objectlit_like(
        &mut self,
        compiler: &AbstractCompiler,
        from_externs: bool,
        n: NodeId,
        member_owner_fn: &dyn Fn(&Ast, NodeId) -> Option<Color>,
    ) {
        // The keys in an object pattern are r-values, not l-values, but they are still accesses.
        check_state!(
            n.is_object_lit(compiler)
                || n.is_object_pattern(compiler)
                || n.is_class_members(compiler)
        );

        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            match c.get_token(compiler) {
                Token::BLOCK
                | Token::COMPUTED_PROP
                | Token::COMPUTED_FIELD_DEF
                | Token::OBJECT_REST
                | Token::OBJECT_SPREAD => {
                    child = c.get_next(compiler);
                    continue;
                }
                Token::STRING_KEY
                | Token::MEMBER_FUNCTION_DEF
                | Token::MEMBER_FIELD_DEF
                | Token::GETTER_DEF
                | Token::SETTER_DEF => {
                    if c.is_quoted_string_key(compiler) {
                        child = c.get_next(compiler);
                        continue; // These won't be renamed due to our assumptions. Ignore them.
                    }

                    let owner = member_owner_fn(compiler, c);
                    self.register_property_use_direct(compiler, from_externs, c, owner.as_ref());
                }
                _ => panic!(
                    "Unexpected child of {}: {}",
                    n.get_token(compiler),
                    c.to_string_tree(compiler)
                ),
            }
            child = c.get_next(compiler);
        }
    }

    /// Update all datastructures as necessary to consider property use {@code site} from type
    /// {@code owner}.
    // port: ColorFindPropertyReferences#registerPropertyUse
    fn register_property_use_direct(
        &mut self,
        ast: &Ast,
        from_externs: bool,
        site: NodeId,
        owner: Option<&Color>,
    ) {
        let site_string = site.get_string_ref(ast);
        let prop_index = self.prop_index.as_mut().unwrap();
        let prop = match prop_index.get(site_string) {
            Some(&prop) => {
                if !from_externs && prop.is_extern_only(self.arena) {
                    prop.set_extern_only(self.arena, false);
                }
                prop
            }
            None => {
                let prop = PropertyClustering::new(self.arena, site_string.clone());
                if from_externs && self.pending_uses.is_some() {
                    prop.set_extern_only(self.arena, true);
                }
                prop_index.insert(site_string.clone(), prop);
                prop
            }
        };
        let flat_owner = self.color_graph_node_factory.create_node(self.arena, owner);

        // Set the initial condition for flowing this property along the graph.
        flat_owner
            .get_associated_props_mut(self.arena)
            .insert(prop, PropAssociation::AST);
        let usage = PropertyUse {
            property: prop,
            owner: flat_owner,
            site,
            keep_original: from_externs
                || owner.is_some_and(|c| c.get_properties_keep_original_name()),
        };
        if let Some(uses) = &mut self.pending_uses {
            uses.push(usage);
        } else {
            usage.record(self.arena);
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
        let token = n.get_token(t);
        if matches!(
            token,
            Token::GETPROP
                | Token::OPTCHAIN_GETPROP
                | Token::OBJECTLIT
                | Token::CALL
                | Token::CLASS
                | Token::OBJECT_PATTERN
                | Token::FUNCTION
        ) {
            let from_externs = if token == Token::FUNCTION {
                false
            } else {
                t.get_current_script().unwrap().is_from_externs(t)
            };
            self.visit_node(t.get_compiler(), from_externs, n, token);
        }
    }
}
