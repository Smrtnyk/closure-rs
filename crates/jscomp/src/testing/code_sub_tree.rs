/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/testing/CodeSubTree.java.

use crate::{AbstractCompiler, node_util::NodeUtil};
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};

/// Represents a subtree of the output from a compilation.
pub struct CodeSubTree {
    root_node: NodeId,
}

impl CodeSubTree {
    // port: CodeSubTree#CodeSubTree
    fn new(root_node: NodeId) -> Self {
        Self { root_node }
    }

    // port: CodeSubTree#getRootNode
    pub fn get_root_node(&self) -> NodeId {
        self.root_node
    }

    // port: CodeSubTree#findClassDefinition(String)
    pub fn find_class_definition(
        &self,
        compiler: &mut AbstractCompiler,
        wanted_class_name: impl Into<JsString>,
    ) -> Self {
        let wanted_class_name = wanted_class_name.into();
        let class_node = Self::find_first_node(compiler, self.root_node, |ast, node| {
            node.is_class(ast) && NodeUtil::get_name(ast, node).as_ref() == Some(&wanted_class_name)
        });
        Self::new(class_node)
    }

    // port: CodeSubTree#findClassDefinition(AbstractCompiler,String)
    pub fn find_class_definition_from_compiler(
        compiler: &mut AbstractCompiler,
        wanted_class_name: impl Into<JsString>,
    ) -> Self {
        Self::new(compiler.get_root().unwrap()).find_class_definition(compiler, wanted_class_name)
    }

    // port: CodeSubTree#findMethodDefinition
    pub fn find_method_definition(
        &self,
        compiler: &mut AbstractCompiler,
        wanted_method_name: impl Into<JsString>,
    ) -> Self {
        let wanted_method_name = wanted_method_name.into();
        let method_definition_node =
            Self::find_first_node(compiler, self.root_node, |ast, node| {
                node.is_member_function_def(ast) && wanted_method_name == node.get_string(ast)
            });
        Self::new(method_definition_node)
    }

    // port: CodeSubTree#findFunctionDefinition
    pub fn find_function_definition(
        compiler: &mut AbstractCompiler,
        wanted_method_name: impl Into<JsString>,
    ) -> Self {
        let wanted_method_name = wanted_method_name.into();
        let function_definition_node =
            Self::find_first_node(compiler, compiler.get_root().unwrap(), |ast, node| {
                node.is_function(ast)
                    && node.get_first_child(ast).unwrap().is_name(ast)
                    && wanted_method_name == node.get_first_child(ast).unwrap().get_string(ast)
            });
        Self::new(function_definition_node)
    }

    // port: CodeSubTree#findMatchingQNameReferences
    pub fn find_matching_qname_references(
        &self,
        compiler: &mut AbstractCompiler,
        wanted_qname: impl Into<JsString>,
    ) -> Vec<NodeId> {
        let wanted_qname = wanted_qname.into();
        Self::find_nodes_allow_empty(compiler, self.root_node, |ast, node| {
            node.matches_qualified_name(ast, wanted_qname.clone())
        })
    }

    // port: CodeSubTree#findNodesAllowEmpty
    pub fn find_nodes_allow_empty(
        compiler: &mut AbstractCompiler,
        root_node: NodeId,
        mut predicate: impl FnMut(&Ast, NodeId) -> bool,
    ) -> Vec<NodeId> {
        let mut list_builder = Vec::new();
        NodeUtil::visit_pre_order(compiler, root_node, &mut |ast: &mut Ast, node| {
            if predicate(ast, node) {
                list_builder.push(node);
            }
        });
        list_builder
    }

    // port: CodeSubTree#findNodesNonEmpty
    pub fn find_nodes_non_empty(
        compiler: &mut AbstractCompiler,
        root_node: NodeId,
        predicate: impl FnMut(&Ast, NodeId) -> bool,
    ) -> Vec<NodeId> {
        let results = Self::find_nodes_allow_empty(compiler, root_node, predicate);
        check_state!(!results.is_empty(), "no nodes found");
        results
    }

    // port: CodeSubTree#findFirstNode
    pub fn find_first_node(
        compiler: &mut AbstractCompiler,
        root_node: NodeId,
        predicate: impl FnMut(&Ast, NodeId) -> bool,
    ) -> NodeId {
        let all_matching_nodes = Self::find_nodes_non_empty(compiler, root_node, predicate);
        all_matching_nodes[0]
    }
}
