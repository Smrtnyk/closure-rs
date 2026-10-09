/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ConvertToDottedProperties.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of ConvertToDottedProperties.java.
//!
//! Converts property accesses from quoted string or bracket access syntax to dot or unquoted
//! string syntax, where possible. Dot syntax is more compact.
#![allow(clippy::collapsible_if, clippy::collapsible_match)] // Retain Java control flow.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{ir::IR, node::NodeId, token::Token};

pub struct ConvertToDottedProperties;

impl ConvertToDottedProperties {
    // port: ConvertToDottedProperties#ConvertToDottedProperties
    pub fn new() -> Self {
        Self
    }

    // port: ConvertToDottedProperties#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        GatherGetterAndSetterProperties::update(compiler, externs, root);
    }
}

impl Default for ConvertToDottedProperties {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for ConvertToDottedProperties {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        ConvertToDottedProperties::process(self, compiler, externs, root);
    }
}

impl Callback for ConvertToDottedProperties {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ConvertToDottedProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::COMPUTED_PROP | Token::COMPUTED_FIELD_DEF => {
                let left_elem = n.get_first_child(t).unwrap();
                // Java: null for a computed field without an initializer (`[x];`), only
                // dereferenced once the key is known to be a valid string property name.
                let right_elem = left_elem.get_next(t);

                // not convert property named constructor or __proto__ in object literals.
                // ['constructor']() and constructor() are different.
                // {['__proto__']: v} defines an own property; {__proto__: v} sets prototype.
                if left_elem.is_string_lit(t)
                    && NodeUtil::is_valid_property_name(FeatureSet::ES3, &left_elem.get_string(t))
                    && left_elem.get_string_ref(t) != "constructor"
                    && !(parent.unwrap().is_object_lit(t)
                        && left_elem.get_string_ref(t) == "__proto__")
                {
                    left_elem.detach(t);
                    let right_elem = right_elem.expect("NullPointerException");
                    right_elem.detach(t);
                    let temp;
                    if n.is_computed_prop(t) {
                        if right_elem.is_function(t) {
                            if n.get_boolean_prop(t, NodeId::COMPUTED_PROP_GETTER) {
                                let name = left_elem.get_string(t);
                                temp = IR::getter_def(t, name, right_elem);
                            } else if n.get_boolean_prop(t, NodeId::COMPUTED_PROP_SETTER) {
                                let name = left_elem.get_string(t);
                                temp = IR::setter_def(t, name, right_elem);
                            } else if n.get_parent(t).unwrap().is_class_members(t) {
                                let name = left_elem.get_string(t);
                                temp = IR::member_function_def(t, name, right_elem);
                                let script = t.get_current_script().unwrap();
                                NodeUtil::add_feature_to_script(
                                    t.get_compiler(),
                                    script,
                                    Feature::MEMBER_DECLARATIONS,
                                );
                            } else {
                                // TODO - further optimize this code to a member function
                                let name = left_elem.get_string(t);
                                temp = IR::string_key_with_value(t, name, right_elem);
                            }
                        } else {
                            let name = left_elem.get_string(t);
                            temp = IR::string_key_with_value(t, name, right_elem);
                        }
                    } else {
                        let name = left_elem.get_string(t);
                        temp = IR::member_field_def(t, name, right_elem);
                    }

                    let is_static_member = n.is_static_member(t);
                    temp.set_static_member(t, is_static_member);
                    n.replace_with(t, temp);
                    t.get_compiler().report_change_to_enclosing_scope(temp);
                }
            }
            Token::GETTER_DEF | Token::SETTER_DEF | Token::STRING_KEY => {
                if NodeUtil::is_valid_property_name(FeatureSet::ES3, &n.get_string(t)) {
                    if n.get_boolean_prop(t, NodeId::QUOTED_PROP) {
                        n.put_boolean_prop(t, NodeId::QUOTED_PROP, false);
                        t.get_compiler().report_change_to_enclosing_scope(n);
                    }
                }
            }
            Token::OPTCHAIN_GETELEM | Token::GETELEM => {
                let left = n.get_first_child(t).unwrap();
                let right = left.get_next(t).unwrap();
                if right.is_string_lit(t)
                    && NodeUtil::is_valid_property_name(FeatureSet::ES3, &right.get_string(t))
                {
                    left.detach(t);
                    right.detach(t);
                    let prop = right.get_string(t);
                    let new_get_prop = if n.is_get_elem(t) {
                        IR::getprop(t, left, prop)
                    } else if n.is_optional_chain_start(t) {
                        IR::start_opt_chain_getprop(t, left, prop)
                    } else {
                        IR::continue_opt_chain_getprop(t, left, prop)
                    };
                    n.replace_with(t, new_get_prop);
                    t.get_compiler()
                        .report_change_to_enclosing_scope(new_get_prop);
                }
            }
            _ => {}
        }
    }
}
