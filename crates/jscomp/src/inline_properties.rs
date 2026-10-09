/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InlineProperties.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    colors::{Color, ColorRegistry, standard_colors},
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{check_state, ir::IR, js_string::JsString, node::Ast, node::NodeId};
use std::sync::Arc;

/// InlineProperties attempts to find references to properties that are known to be constants and
/// inline the known value.
///
/// This pass relies on type information to find these property references and properties are
/// assumed to be constant if they are assigned exactly once, unconditionally, in either of the
/// following contexts: (1) statically on a constructor, or (2) on a class's prototype.
///
/// The current implementation only inlines immutable values (as defined by
/// NodeUtil.isImmutableValue).
pub struct InlineProperties {
    registry: Arc<ColorRegistry>,
    // Java: Map<String, PropertyInfo> with the shared INVALIDATED instance; `None` here is
    // INVALIDATED (the only PropertyInfo whose color and value are null).
    props: IndexMap<JsString, Option<PropertyInfo>>,
}

// port: InlineProperties.PropertyInfo
struct PropertyInfo {
    color: Color,
    value: NodeId,
}

impl InlineProperties {
    // port: InlineProperties#InlineProperties
    pub fn new(compiler: &AbstractCompiler) -> Self {
        let mut this = Self {
            registry: compiler.get_color_registry().clone(),
            props: IndexMap::<_, _>::default(),
        };
        this.invalidate_extern_properties(compiler);
        this
    }

    // port: InlineProperties#invalidateExternProperties
    fn invalidate_extern_properties(&mut self, compiler: &AbstractCompiler) {
        // Invalidate properties defined in externs.
        for name in compiler.get_extern_properties().expect("") {
            self.props.insert(JsString::from(name.as_str()), None);
        }
    }

    /// This method gets the JSType from the Node argument and verifies that it is present.
    // port: InlineProperties#getColor
    fn get_color(ast: &Ast, n: NodeId) -> Color {
        let color = n.get_color(ast);

        match color {
            None => standard_colors::UNKNOWN.clone(),
            Some(color) => color,
        }
    }

    // port: InlineProperties#removeNullAndUndefinedIfUnion
    fn remove_null_and_undefined_if_union(original: Color) -> Color {
        if original.is_union() {
            original.subtract_null_or_void()
        } else {
            original
        }
    }
}

impl CompilerPass for InlineProperties {
    // port: InlineProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // Find and replace the properties in non-extern AST.
        NodeTraversal::traverse(compiler, root, &mut GatherCandidates { outer: self });
        NodeTraversal::traverse(
            compiler,
            root,
            &mut ReplaceCandidates {
                outer: self,
                has_in_supertypes_list_seen_set: IndexSet::<_>::default(),
            },
        );
    }
}

struct GatherCandidates<'a> {
    outer: &'a mut InlineProperties,
}

impl GatherCandidates<'_> {
    /// Returns whether this is a valid definition for a candidate class field.
    // port: InlineProperties.GatherCandidates#maybeRecordCandidateClassFieldDefinition
    fn maybe_record_candidate_class_field_definition(&mut self, ast: &Ast, n: NodeId) -> bool {
        check_state!(n.is_member_field_def(ast), "%s", n.to_string(ast));
        let value = n.get_first_child(ast);
        let prop_name = n.get_string(ast);
        let class_node = n.get_grandparent(ast).unwrap();
        let c: Color = if n.is_static_member(ast) {
            InlineProperties::get_color(ast, class_node)
        } else {
            let class_color = InlineProperties::get_color(ast, class_node);
            let possible_instances = class_color.get_instance_colors();
            if possible_instances.is_empty() {
                standard_colors::UNKNOWN.clone()
            } else {
                Color::create_union(possible_instances)
            }
        };

        self.maybe_store_candidate_value(ast, c, prop_name, value)
    }

    /// Returns whether this is a valid definition for a candidate property.
    // port: InlineProperties.GatherCandidates#maybeRecordCandidateGetpropDefinition
    fn maybe_record_candidate_getprop_definition(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
    ) -> bool {
        check_state!(
            n.is_get_prop(t) && parent.is_assign(t),
            "%s",
            n.to_string(t)
        );
        let src = n.get_first_child(t).unwrap();
        let prop_name = n.get_string(t);
        let value = parent.get_last_child(t);

        if src.is_this(t) {
            // This is a simple assignment like:
            //    this.foo = 1;
            if Self::in_constructor(t) {
                // This may be a valid assignment.
                let color = InlineProperties::get_color(t, src);
                return self.maybe_store_candidate_value(t, color, prop_name, value);
            }
            return false;
        } else if t.in_global_hoist_scope()
            && src.is_get_prop(t)
            && src.get_string_ref(t) == "prototype"
        {
            // This is a prototype assignment like:
            //    x.prototype.foo = 1;
            let instance_type = InlineProperties::get_color(t, src);
            return self.maybe_store_candidate_value(t, instance_type, prop_name, value);
        } else if t.in_global_hoist_scope() {
            // This is a static assignment like:
            //    x.foo = 1;
            let target_type = InlineProperties::get_color(t, src);
            if target_type.is_constructor() {
                return self.maybe_store_candidate_value(t, target_type, prop_name, value);
            }
        }
        false
    }

    // port: InlineProperties.GatherCandidates#invalidateProperty
    fn invalidate_property(&mut self, prop_name: JsString) {
        self.outer.props.insert(prop_name, None);
    }

    /// Adds the candidate property to the map if it meets all constness and immutability
    /// criteria, and is not already present in the map. If the property was already present, it
    /// is invalidated. Returns true if the property was successfully added.
    // port: InlineProperties.GatherCandidates#maybeStoreCandidateValue
    fn maybe_store_candidate_value(
        &mut self,
        ast: &Ast,
        color: Color,
        prop_name: JsString,
        value: Option<NodeId>,
    ) -> bool {
        let value = closure_rhino::check_not_null!(value);
        if !self.outer.props.contains_key(&prop_name)
            && !color.is_invalidating()
            && NodeUtil::is_immutable_value(ast, value)
            && NodeUtil::is_executed_exactly_once(ast, value)
        {
            self.outer
                .props
                .insert(prop_name, Some(PropertyInfo { color, value }));
            return true;
        }
        false
    }

    /// Returns whether the traversal is directly in an ES6 class constructor or an @constructor
    /// function
    ///
    /// This returns false for nested functions inside ctors, including arrow functions (even
    /// though the `this` is the same). This pass only cares about property definitions executed
    /// once per ctor invocation, and in general we don't know how many times an arrow fn will be
    /// executed. In the future, we could special case arrow fn IIFEs in this pass if it becomes
    /// useful.
    // port: InlineProperties.GatherCandidates#inConstructor
    fn in_constructor(t: &mut NodeTraversal<'_>) -> bool {
        let root = t.get_enclosing_function();
        if root.is_none() {
            // we might be in the global scope
            return false;
        }
        let (types, ast) = t.get_compiler().get_type_registry_field_and_ast();
        match types {
            Some(types) => NodeUtil::is_constructor(ast, root, types),
            None => is_constructor_without_type_registry(ast, root.unwrap()),
        }
    }
}

impl Callback for GatherCandidates<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: InlineProperties.GatherCandidates#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // These are assigned at most once in the branches below
        let invalidating_prop_ref: bool;
        let prop_name: JsString;
        if n.is_get_prop(t) {
            let parent = parent.unwrap();
            prop_name = n.get_string(t);
            if parent.is_assign(t) {
                invalidating_prop_ref =
                    !self.maybe_record_candidate_getprop_definition(t, n, parent);
            } else if NodeUtil::is_l_value(t, n) {
                // Other LValue references invalidate
                // e.g. in an enhanced for loop or a destructuring statement
                invalidating_prop_ref = true;
            } else if parent.is_del_prop(t) {
                // Deletes invalidate
                invalidating_prop_ref = true;
            } else {
                // A property read doesn't invalidate
                invalidating_prop_ref = false;
            }
        } else if (n.is_string_key(t) && !n.get_parent(t).unwrap().is_object_pattern(t))
            || n.is_getter_def(t)
            || n.is_setter_def(t)
            || n.is_member_function_def(t)
        {
            prop_name = n.get_string(t);
            // For now, any object literal key invalidates
            // TODO(johnlenz): support prototype properties like:
            //   foo.prototype = { a: 1, b: 2 };
            // TODO(johnlenz): Object.create(), Object.createProperty
            // and getter/setter defs and member functions also invalidate
            // since we do not inline functions in this pass
            // Note that string keys in destructuring patterns are fine, since they just access the
            // prop
            invalidating_prop_ref = true;
        } else if n.is_member_field_def(t) {
            prop_name = n.get_string(t);
            if n.has_children(t) {
                // class field with initialization
                invalidating_prop_ref = !self.maybe_record_candidate_class_field_definition(t, n);
            } else {
                // class field with no initialization
                invalidating_prop_ref = true;
            }
        } else {
            return;
        }

        if invalidating_prop_ref {
            // checkNotNull(propName): a JsString is never null.
            self.invalidate_property(prop_name);
        }
    }
}

struct ReplaceCandidates<'a> {
    outer: &'a mut InlineProperties,
    has_in_supertypes_list_seen_set: IndexSet<Color>,
}

impl ReplaceCandidates<'_> {
    // port: InlineProperties.ReplaceCandidates#isMatchingType
    fn is_matching_type(&mut self, ast: &Ast, n: NodeId, src: Color) -> bool {
        let src = InlineProperties::remove_null_and_undefined_if_union(src);
        let dest = InlineProperties::remove_null_and_undefined_if_union(
            InlineProperties::get_color(ast, n),
        );
        if dest.is_invalidating() {
            return false;
        }
        if dest.is_union() || src.is_union() {
            return false;
        }
        self.has_in_supertypes_list(&dest, &src)
    }

    // port: InlineProperties.ReplaceCandidates#hasInSupertypesList
    fn has_in_supertypes_list(&mut self, sub_ctor: &Color, super_ctor: &Color) -> bool {
        let result = (|| {
            if !self
                .has_in_supertypes_list_seen_set
                .insert(sub_ctor.clone())
            {
                return false;
            }
            // `subCtor == null || superCtor == null`: neither can be null here (callers pass
            // getColor results, PropertyInfo colors and registry supertypes).
            if sub_ctor == super_ctor {
                return true;
            }

            let registry = self.outer.registry.clone();
            for immediate_supertype in registry.get_disambiguation_supertypes(sub_ctor) {
                if !immediate_supertype.is_union()
                    && self.has_in_supertypes_list(immediate_supertype, super_ctor)
                {
                    return true;
                }
            }
            false
        })();
        // finally
        self.has_in_supertypes_list_seen_set.shift_remove(sub_ctor);
        result
    }
}

impl Callback for ReplaceCandidates<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: InlineProperties.ReplaceCandidates#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_get_prop(t) && !NodeUtil::is_l_value(t, n) {
            let target = n.get_first_child(t).unwrap();
            let prop_name = n.get_string(t);
            let info = match self.outer.props.get(&prop_name) {
                Some(Some(info)) => Some((info.color.clone(), info.value)),
                _ => None,
            };
            if let Some((info_color, info_value)) = info
                && self.is_matching_type(t, target, info_color)
            {
                let mut replacement = info_value.clone_tree(t);
                let compiler = t.get_compiler();
                if compiler
                    .get_ast_analyzer()
                    .may_have_side_effects(compiler, n.get_first_child(compiler).unwrap())
                {
                    let first = n.remove_first_child(compiler).unwrap();
                    replacement = IR::comma(compiler, first, replacement).srcref(compiler, n);
                }
                n.replace_with(compiler, replacement);
                compiler.report_change_to_enclosing_scope(replacement);
            }
        }
    }
}

/// Rust-only: `NodeUtil#isConstructor(Node)` when the compiler holds no `JSTypeRegistry` (never
/// created, or cleared after the types were converted to colors). The Rust port takes the registry
/// as an argument (DESIGN §4); Java reads it only through a JSType on the node, and a node carries a
/// JSType only while the registry exists, so the JSType clause is false here.
fn is_constructor_without_type_registry(ast: &closure_rhino::node::Ast, fn_node: NodeId) -> bool {
    if !fn_node.is_function(ast) {
        return false;
    }
    assert!(
        fn_node.get_jstype(ast).is_none(),
        "NodeUtil#isConstructor: a node has a JSType but the compiler has no JSTypeRegistry"
    );
    let js_doc_info = NodeUtil::get_best_jsdoc_info(ast, fn_node);
    let color = fn_node.get_color(ast);
    js_doc_info.is_some_and(|info| info.is_constructor())
        || color.is_some_and(|color| color.is_constructor())
        || NodeUtil::is_es6_constructor(ast, fn_node)
}
