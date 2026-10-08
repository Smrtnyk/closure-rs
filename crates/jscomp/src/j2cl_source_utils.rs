/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/J2clSourceUtils.java.

//! Port of `J2clSourceUtils.java`.
use crate::node_util::NodeUtil;
use closure_rhino::{
    check_argument,
    node::{Ast, NodeId},
};

pub struct J2clSourceUtils;

impl J2clSourceUtils {
    // port: J2clSourceUtils#isJ2clSource(String)
    pub fn is_j2cl_source(source_file_name: Option<&str>) -> bool {
        source_file_name.is_some_and(|name| name.ends_with(".java.js"))
    }

    // port: J2clSourceUtils#isJ2clSource(Node)
    pub fn is_j2cl_source_node(ast: &Ast, script_node: NodeId) -> bool {
        check_argument!(script_node.is_script(ast));
        Self::is_j2cl_source(script_node.get_source_file_name(ast).as_deref())
    }

    // port: J2clSourceUtils#isInsideJ2clSource
    pub fn is_inside_j2cl_source(ast: &Ast, node: NodeId) -> bool {
        let script_node = if node.is_script(ast) {
            Some(node)
        } else {
            NodeUtil::get_enclosing_script(ast, node)
        };
        let Some(script_node) = script_node else {
            return false;
        };
        Self::is_j2cl_source_node(ast, script_node)
    }
}
