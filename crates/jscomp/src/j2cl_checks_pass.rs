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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/J2clChecksPass.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `J2clChecksPass.java`.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
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

    // port: J2clChecksPass#isNullType
    fn is_null_type(&self, compiler: &mut AbstractCompiler, js_type: Option<TypeId>) -> bool {
        let Some(js_type) = js_type else {
            return false;
        };
        let (reg, _ast) = compiler.get_type_registry_and_ast();
        js_type.is_null_type(reg) || js_type.is_void_type(reg)
    }

    // port: J2clChecksPass#getSourceName
    fn get_source_name<'a>(
        &self,
        reg: &JSTypeRegistry,
        ast: &'a Ast,
        js_type: TypeId,
    ) -> Option<&'a str> {
        let constructor = js_type
            .to_maybe_object_type(reg)
            .unwrap()
            .get_constructor(reg);
        let Some(constructor) = constructor else {
            return Some("");
        };
        let mut current = Some(FunctionType::get_source(constructor, reg)?);
        while let Some(node) = current {
            if let Some(file) = node.get_static_source_file_ref(ast) {
                return Some(file.get_name());
            }
            current = node.get_parent(ast);
        }
        None
    }

    // port: J2clChecksPass#isType
    fn type_source_name<'a>(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &'a Ast,
        js_type: Option<TypeId>,
    ) -> Option<&'a str> {
        let js_type = js_type?;
        let js_type = js_type.restrict_by_not_null_or_undefined(reg, ast);
        js_type.to_maybe_object_type(reg)?;
        self.get_source_name(reg, ast, js_type)
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
        self.check_reference_equality(t.get_compiler(), n);
    }
}

impl J2clChecksPass {
    // port: J2clChecksPass#checkReferenceEquality
    fn check_reference_equality(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        // Rust-only (performance divergence): filter equality operators once and borrow each
        // operand's constructor source name once instead of allocating per pattern.
        if !Self::is_equality(compiler, n) {
            return;
        }
        let first_js_type = n.get_first_child(compiler).unwrap().get_jstype(compiler);
        let last_js_type = n.get_last_child(compiler).unwrap().get_jstype(compiler);
        if first_js_type.is_none() && last_js_type.is_none() {
            return;
        }
        if self.is_null_type(compiler, first_js_type) || self.is_null_type(compiler, last_js_type) {
            return;
        }
        let mut matched = [false; 3];
        {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let first_src = self.type_source_name(reg, ast, first_js_type);
            let last_src = self.type_source_name(reg, ast, last_js_type);
            for (idx, (_, file_name)) in REFERENCE_EQUALITY_TYPE_PATTERNS.iter().enumerate() {
                if first_src.is_some_and(|s| s.ends_with(file_name))
                    || last_src.is_some_and(|s| s.ends_with(file_name))
                {
                    matched[idx] = true;
                }
            }
        }
        for (idx, (type_name, _)) in REFERENCE_EQUALITY_TYPE_PATTERNS.iter().enumerate() {
            if matched[idx] {
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &J2CL_REFERENCE_EQUALITY,
                    &[type_name],
                ));
            }
        }
    }
}

impl J2clChecksPass {
    fn is_equality(ast: &Ast, n: NodeId) -> bool {
        n.is_sheq(ast) || n.is_eq(ast) || n.is_shne(ast) || n.is_ne(ast)
    }
}

/// `CLOSURE_RS_J2CL_CHECKS_WALK=off` checks the equality operators in a `NodeTraversal`.
static DIRECT_WALK: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::env::var("CLOSURE_RS_J2CL_CHECKS_WALK").as_deref() != Ok("off")
});

impl CompilerPass for J2clChecksPass {
    // port: J2clChecksPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        if !*DIRECT_WALK {
            NodeTraversal::traverse(compiler, root, self);
            return;
        }
        // Rust-only (performance divergence): the visitor acts on equality operators alone and
        // changes no tree, so a plain walk lists them in the traversal's order (a node after its
        // children, children in order) and each is checked after the walk, reporting in the same
        // order.
        let mut equalities = Vec::new();
        let mut pending = vec![(root, false)];
        while let Some((n, children_done)) = pending.pop() {
            if children_done {
                if Self::is_equality(compiler, n) {
                    equalities.push(n);
                }
                continue;
            }
            pending.push((n, true));
            let first = pending.len();
            let mut child = n.get_first_child(compiler);
            while let Some(c) = child {
                pending.push((c, false));
                child = c.get_next(compiler);
            }
            pending[first..].reverse();
        }
        for n in equalities {
            self.check_reference_equality(compiler, n);
        }
    }
}
