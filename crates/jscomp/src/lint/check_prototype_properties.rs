/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckPrototypeProperties.java.

//! Checks for prototype properties that are Arrays or Objects, which are shared by every
//! instance.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::node::NodeId;

// port: CheckPrototypeProperties#ILLEGAL_PROTOTYPE_MEMBER
pub static ILLEGAL_PROTOTYPE_MEMBER: DiagnosticType = DiagnosticType::disabled(
    "JSC_ILLEGAL_PROTOTYPE_MEMBER",
    "Prototype property {0} should be a primitive, not an Array or Object.",
);

pub struct CheckPrototypeProperties;

impl CheckPrototypeProperties {
    // port: CheckPrototypeProperties#CheckPrototypeProperties
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }
}

impl CompilerPass for CheckPrototypeProperties {
    // port: CheckPrototypeProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckPrototypeProperties {
    // port: CheckPrototypeProperties#visit
    fn visit(&mut self, unused: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if NodeUtil::is_prototype_property_declaration(unused, n) {
            let assign = n.get_first_child(unused).unwrap();
            let rhs = assign.get_last_child(unused).unwrap();
            if rhs.is_array_lit(unused) || rhs.is_object_lit(unused) {
                let js_doc = NodeUtil::get_best_jsdoc_info(unused, rhs);
                if js_doc.is_some_and(|js_doc| js_doc.has_enum_parameter_type()) {
                    // Don't report for @enum's on the prototype. Sometimes this is necessary, for
                    // example, to expose the enum values to an Angular template.
                    return;
                }
                let prop_name = assign
                    .get_first_child(unused)
                    .unwrap()
                    .get_last_child(unused)
                    .unwrap()
                    .get_string(unused);
                let compiler = unused.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    assign,
                    &ILLEGAL_PROTOTYPE_MEMBER,
                    &[&prop_name.to_string()],
                ));
            }
        }
    }

    // port: CheckPrototypeProperties#shouldTraverse
    fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
}
