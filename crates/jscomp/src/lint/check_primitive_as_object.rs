/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/lint/CheckPrimitiveAsObject.java.

//! Check for explicit creation of the object equivalents of primitive types (e.g. `new String`)
//! and for declarations of those object types in JSDoc.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};

// port: CheckPrimitiveAsObject#NEW_PRIMITIVE_OBJECT
pub static NEW_PRIMITIVE_OBJECT: DiagnosticType =
    DiagnosticType::warning("JSC_PRIMITIVE_OBJECT", "Explicit creation of a {0} object.");

// port: CheckPrimitiveAsObject#PRIMITIVE_OBJECT_DECLARATION
pub static PRIMITIVE_OBJECT_DECLARATION: DiagnosticType = DiagnosticType::warning(
    "JSC_PRIMITIVE_OBJECT_DECLARATION",
    "Declaration of {0} object instead of primitive type.",
);

// port: CheckPrimitiveAsObject#PRIMITIVE_OBJECT_CONSTRUCTORS
const PRIMITIVE_OBJECT_CONSTRUCTORS: [&str; 3] = ["Boolean", "Number", "String"];

fn is_primitive_object_constructor(name: &JsString) -> bool {
    PRIMITIVE_OBJECT_CONSTRUCTORS.iter().any(|c| name == *c)
}

pub struct CheckPrimitiveAsObject;

impl CheckPrimitiveAsObject {
    // port: CheckPrimitiveAsObject#CheckPrimitiveAsObject
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: CheckPrimitiveAsObject#checkForPrimitiveObjectDeclaration
    fn check_for_primitive_object_declaration(t: &mut NodeTraversal<'_>, n: NodeId) {
        let js_doc_info = n.get_jsdoc_info(t);
        if let Some(js_doc_info) = js_doc_info {
            for type_root in js_doc_info.get_type_nodes() {
                Self::check_type_node_for_primitive_object_declaration(t, type_root);
            }
        }
    }

    // port: CheckPrimitiveAsObject#checkTypeNodeForPrimitiveObjectDeclaration
    fn check_type_node_for_primitive_object_declaration(
        t: &mut NodeTraversal<'_>,
        type_root: NodeId,
    ) {
        // The visitor cannot report through `t` while `visitPreOrder` borrows its AST, so the
        // reports are collected in visit order and made right after the (read-only) walk.
        let mut reports: Vec<(NodeId, JsString)> = Vec::new();
        NodeUtil::visit_pre_order(t, type_root, &mut |ast: &mut Ast, node: NodeId| {
            if node.is_string_lit(ast) {
                let type_name = node.get_string(ast);
                if is_primitive_object_constructor(&type_name) {
                    reports.push((node, type_name));
                }
            }
        });
        for (node, type_name) in reports {
            t.report(
                node,
                &PRIMITIVE_OBJECT_DECLARATION,
                &[&type_name.to_string()],
            );
        }
    }

    // port: CheckPrimitiveAsObject#checkForPrimitiveObjectConstructor
    fn check_for_primitive_object_constructor(t: &mut NodeTraversal<'_>, n: NodeId) {
        if n.is_new(t) {
            let constructor_function = n.get_first_child(t).unwrap();
            if constructor_function.is_name(t) {
                let constructor_name = constructor_function.get_string(t);
                if is_primitive_object_constructor(&constructor_name) {
                    t.report(n, &NEW_PRIMITIVE_OBJECT, &[&constructor_name.to_string()]);
                }
            }
        }
    }
}

impl CompilerPass for CheckPrimitiveAsObject {
    // port: CheckPrimitiveAsObject#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckPrimitiveAsObject {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckPrimitiveAsObject#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        Self::check_for_primitive_object_constructor(t, n);
        Self::check_for_primitive_object_declaration(t, n);
    }
}
