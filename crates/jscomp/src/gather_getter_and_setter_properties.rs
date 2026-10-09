/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/GatherGetterAndSetterProperties.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `GatherGetterAndSetterProperties.java`: finds all properties defined by getters or
//! setters and records them in the compiler's `AccessorSummary`.

use crate::{
    AbstractCompiler,
    accessor_summary::{AccessorSummary, PropertyAccessKind},
    compiler_pass::CompilerPass,
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
use std::sync::Arc;

/// Finds all getters and setters in the compilation (Java `CompilerPass`).
pub struct GatherGetterAndSetterProperties;

impl GatherGetterAndSetterProperties {
    // port: GatherGetterAndSetterProperties#GatherGetterAndSetterProperties
    pub fn new() -> Self {
        Self
    }

    // port: GatherGetterAndSetterProperties#update
    pub fn update(compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // TODO(nickreid): We probably don't need to re-gather from the externs. They don't change so
        // the first collection should be good forever.
        // For now we traverse both trees every time because there's no reason we have to treat them
        // differently.
        check_state!(externs.get_parent(compiler) == root.get_parent(compiler));
        let accessor_summary = if compiler
            .get_options()
            .get_assume_properties_are_statically_analyzable()
        {
            let parent = externs.get_parent(compiler).unwrap();
            AccessorSummary::create(Self::gather(compiler, parent))
        } else {
            AccessorSummary::create_assuming_always_getter_and_setter()
        };
        compiler.set_accessor_summary(Arc::new(accessor_summary));
    }

    // port: GatherGetterAndSetterProperties#gather
    pub fn gather(
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> IndexMap<JsString, PropertyAccessKind> {
        let mut gather_callback = GatherCallback::new();
        NodeTraversal::traverse(compiler, root, &mut gather_callback);
        gather_callback.properties
    }
}

impl Default for GatherGetterAndSetterProperties {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for GatherGetterAndSetterProperties {
    // port: GatherGetterAndSetterProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::update(compiler, externs, root);
    }
}

// port: GatherGetterAndSetterProperties.GatherCallback
struct GatherCallback {
    properties: IndexMap<JsString, PropertyAccessKind>,
}

impl GatherCallback {
    fn new() -> Self {
        Self {
            properties: IndexMap::<_, _>::default(),
        }
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#record
    fn record(&mut self, property: JsString, kind: PropertyAccessKind) {
        // Java: properties.merge(property, kind, PropertyAccessKind::unionWith)
        match self.properties.get_mut(&property) {
            Some(existing) => *existing = existing.union_with(kind),
            None => {
                self.properties.insert(property, kind);
            }
        }
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#recordGetterDef
    fn record_getter_def(&mut self, ast: &Ast, getter_def: NodeId) {
        check_state!(getter_def.is_getter_def(ast));
        let name = getter_def.get_string(ast);
        self.record(name, PropertyAccessKind::GETTER_ONLY);
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#recordSetterDef
    fn record_setter_def(&mut self, ast: &Ast, setter_def: NodeId) {
        check_state!(setter_def.is_setter_def(ast));
        let name = setter_def.get_string(ast);
        self.record(name, PropertyAccessKind::SETTER_ONLY);
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visitDescriptor
    fn visit_descriptor(&mut self, ast: &Ast, property_name: &JsString, descriptor: NodeId) {
        let mut key = descriptor.get_first_child(ast);
        while let Some(k) = key {
            if k.is_string_key(ast) || k.is_member_function_def(ast) {
                if k.get_string_ref(ast) == "get" {
                    self.record(property_name.clone(), PropertyAccessKind::GETTER_ONLY);
                } else if k.get_string_ref(ast) == "set" {
                    self.record(property_name.clone(), PropertyAccessKind::SETTER_ONLY);
                }
            }
            key = k.get_next(ast);
        }
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visitDefineProperty
    fn visit_define_property(&mut self, ast: &Ast, define_property_call: NodeId) {
        let property_name_node = define_property_call.get_child_at_index(ast, 2).unwrap();
        let descriptor = define_property_call.get_child_at_index(ast, 3).unwrap();
        if !property_name_node.is_string_lit(ast) || !descriptor.is_object_lit(ast) {
            return;
        }
        let property_name = property_name_node.get_string(ast);
        self.visit_descriptor(ast, &property_name, descriptor);
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visitDefineProperties
    fn visit_define_properties(&mut self, ast: &Ast, define_properties_call: NodeId) {
        let props = define_properties_call.get_child_at_index(ast, 2).unwrap();
        if !props.is_object_lit(ast) {
            return;
        }
        let mut prop = props.get_first_child(ast);
        while let Some(p) = prop {
            if p.is_string_key(ast)
                && p.has_one_child(ast)
                && p.get_first_child(ast).unwrap().is_object_lit(ast)
            {
                let property_name = p.get_string(ast);
                let descriptor = p.get_first_child(ast).unwrap();
                self.visit_descriptor(ast, &property_name, descriptor);
            }
            prop = p.get_next(ast);
        }
    }
}

impl Callback for GatherCallback {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::GETTER_DEF => self.record_getter_def(t, n),
            Token::SETTER_DEF => self.record_setter_def(t, n),
            Token::CALL => {
                if NodeUtil::is_object_define_property_definition(t, n) {
                    self.visit_define_property(t, n);
                } else if NodeUtil::is_object_define_properties_definition(t, n) {
                    self.visit_define_properties(t, n);
                }
            }
            _ => {}
        }
    }
}
