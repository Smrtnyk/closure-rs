/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckArrayWithGoogObject.java.

//! Port of lint/CheckArrayWithGoogObject.java: lints against passing arrays to goog.object
//! methods with the intention of iterating over them as though with a for-in loop, which is
//! discouraged with arrays.
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use closure_jstype::prelude::*;
use closure_rhino::node::{Ast, NodeId};

// port: CheckArrayWithGoogObject#GOOG_OBJECT_METHODS
static GOOG_OBJECT_METHODS: [&str; 19] = [
    "goog.object.forEach",
    "goog.object.filter",
    "goog.object.map",
    "goog.object.some",
    "goog.object.every",
    "goog.object.getCount",
    "goog.object.getAnyKey",
    "goog.object.getAnyValue",
    "goog.object.contains",
    "goog.object.getValues",
    "goog.object.getKeys",
    "goog.object.findKey",
    "goog.object.findValue",
    "goog.object.isEmpty",
    "goog.object.clear",
    "goog.object.remove",
    "goog.object.equals",
    "goog.object.clone",
    "goog.object.transpose",
];

// port: CheckArrayWithGoogObject#ARRAY_PASSED_TO_GOOG_OBJECT
pub static ARRAY_PASSED_TO_GOOG_OBJECT: DiagnosticType = DiagnosticType::warning(
    "JSC_ARRAY_PASSED_TO_GOOG_OBJECT",
    "{0} expects an object, not an array. Did you mean to use goog.array?",
);

// port: CheckArrayWithGoogObject
pub struct CheckArrayWithGoogObject;

impl CheckArrayWithGoogObject {
    // port: CheckArrayWithGoogObject#CheckArrayWithGoogObject
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: CheckArrayWithGoogObject#isGoogObjectIterationOverArray
    pub fn is_goog_object_iteration_over_array(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> bool {
        if !n.is_call(compiler) {
            return false;
        }
        let callee = n.get_first_child(compiler).unwrap();
        if !callee.is_qualified_name(compiler) {
            return false;
        }

        let name = callee.get_qualified_name(compiler).unwrap();
        if !GOOG_OBJECT_METHODS.iter().any(|method| name == *method) {
            return false;
        }

        let Some(first_arg) = n.get_second_child(compiler) else {
            return false;
        };
        let type_ = first_arg.get_jstype(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        type_.is_some_and(|type_| self.contains_array(reg, ast, type_))
    }

    // port: CheckArrayWithGoogObject#containsArray
    fn contains_array(&self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        // Check if type is itself an array
        if type_.is_array_type(reg) {
            return true;
        }
        let templatized_type = type_.to_maybe_templatized_type(reg);
        if let Some(templatized_type) = templatized_type
            && TemplatizedType::get_referenced_type(templatized_type, reg).is_array_type(reg)
        {
            return true;
        }

        // Check if this is a union that contains an array
        if type_.is_union_type(reg) {
            let array_type = reg.get_native_type(JSTypeNative::ARRAY_TYPE);
            let alternates = type_
                .to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast);
            for alternate in alternates.iter() {
                if alternate.is_subtype_of(reg, ast, array_type) {
                    return true;
                }
            }
        }
        false
    }
}

impl Callback for CheckArrayWithGoogObject {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckArrayWithGoogObject#visit
    fn visit(&mut self, unused: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = unused.get_compiler();
        if self.is_goog_object_iteration_over_array(compiler, n) {
            let name = n
                .get_first_child(compiler)
                .unwrap()
                .get_qualified_name(compiler)
                .unwrap()
                .to_string();
            compiler.report(JSError::make(
                compiler,
                n,
                &ARRAY_PASSED_TO_GOOG_OBJECT,
                &[&name],
            ));
        }
    }
}

impl CompilerPass for CheckArrayWithGoogObject {
    // port: CheckArrayWithGoogObject#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}
