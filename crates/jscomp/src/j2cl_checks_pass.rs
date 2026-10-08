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
//   src/com/google/javascript/jscomp/J2clChecksPass.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `J2clChecksPass.java`.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_jstype::{JSTypeRegistry, TypeId, prelude::*};
use closure_rhino::node::{Ast, NodeId};

// port: J2clChecksPass#J2CL_REFERENCE_EQUALITY
pub static J2CL_REFERENCE_EQUALITY: DiagnosticType = DiagnosticType::warning(
    "JSC_J2CL_REFERENCE_EQUALITY",
    "Reference equality may not be used with the specified type: {0}",
);

/// Types for which using reference equality is an error. Mapped from name to filename.
// port: J2clChecksPass#REFERENCE_EQUALITY_TYPE_PATTERNS
pub static REFERENCE_EQUALITY_TYPE_PATTERNS: [(&str, &str); 3] = [
    ("java.lang.Integer", "java/lang/Integer.impl.java.js"),
    ("java.lang.Float", "java/lang/Float.impl.java.js"),
    ("goog.math.Long", "javascript/closure/math/long.js"),
];

/// Performs correctness checks which are specific to J2CL-generated patterns.
#[derive(Debug, Default)]
pub struct J2clChecksPass;

impl J2clChecksPass {
    // port: J2clChecksPass#J2clChecksPass
    pub fn new() -> Self {
        Self
    }

    /// Reports an error if the node is a reference equality check of the specified type.
    // port: J2clChecksPass#checkReferenceEquality
    fn check_reference_equality(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_name: &str,
        file_name: &str,
    ) {
        if n.is_sheq(compiler) || n.is_eq(compiler) || n.is_shne(compiler) || n.is_ne(compiler) {
            let first_js_type = n.get_first_child(compiler).unwrap().get_jstype(compiler);
            let last_js_type = n.get_last_child(compiler).unwrap().get_jstype(compiler);
            let has_type = self.is_type(compiler, first_js_type, file_name)
                || self.is_type(compiler, last_js_type, file_name);
            let has_null_type = self.is_null_type(compiler, first_js_type)
                || self.is_null_type(compiler, last_js_type);
            if has_type && !has_null_type {
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &J2CL_REFERENCE_EQUALITY,
                    &[type_name],
                ));
            }
        }
    }

    // port: J2clChecksPass#isNullType
    fn is_null_type(&self, compiler: &mut AbstractCompiler, js_type: Option<TypeId>) -> bool {
        let Some(js_type) = js_type else {
            return false;
        };
        let (reg, _ast) = compiler.get_type_registry_and_ast();
        js_type.is_null_type(reg) || js_type.is_void_type(reg)
    }

    // port: J2clChecksPass#isType
    fn is_type(
        &self,
        compiler: &mut AbstractCompiler,
        js_type: Option<TypeId>,
        file_name: &str,
    ) -> bool {
        let Some(js_type) = js_type else {
            return false;
        };
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let js_type = js_type.restrict_by_not_null_or_undefined(reg, ast);
        if js_type.to_maybe_object_type(reg).is_none() {
            return false;
        }
        let source_name = self.get_source_name(reg, ast, js_type);
        source_name.is_some_and(|source_name| source_name.ends_with(file_name))
    }

    // port: J2clChecksPass#getSourceName
    fn get_source_name(&self, reg: &JSTypeRegistry, ast: &Ast, js_type: TypeId) -> Option<String> {
        let constructor = js_type
            .to_maybe_object_type(reg)
            .unwrap()
            .get_constructor(reg);
        let Some(constructor) = constructor else {
            return Some(String::new());
        };
        NodeUtil::get_source_name(ast, FunctionType::get_source(constructor, reg)?)
    }
}

impl Callback for J2clChecksPass {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: J2clChecksPass#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        for (type_name, file_name) in REFERENCE_EQUALITY_TYPE_PATTERNS {
            self.check_reference_equality(t.get_compiler(), n, type_name, file_name);
        }
    }
}

impl CompilerPass for J2clChecksPass {
    // port: J2clChecksPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}
