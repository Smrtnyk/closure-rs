/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckTypeImportCodeReferences.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{node::NodeId, qualified_name::QualifiedName};
use std::sync::LazyLock;

// port: CheckTypeImportCodeReferences#TYPE_IMPORT_CODE_REFERENCE
pub static TYPE_IMPORT_CODE_REFERENCE: DiagnosticType = DiagnosticType::error(
    "JSC_TYPE_IMPORT_CODE_REFERENCE",
    "Cannot reference goog.requireType()''d name {0} outside of a type annotation.",
);

// port: CheckTypeImportCodeReferences#GOOG_REQUIRE_TYPE
// Java keeps the detached node IR.getprop(IR.name("goog"), "requireType") and calls
// Node#matchesQualifiedName(Node) with it. Nodes live in the compiler's arena here, so the
// constant is the equivalent QualifiedName (a GETPROP "requireType" whose receiver is the NAME
// "goog"), as in GatherModuleMetadata#GOOG_PROVIDE.
static GOOG_REQUIRE_TYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));

/// Checks for invalid code references to type-only imports (i.e., goog.requireType).
pub struct CheckTypeImportCodeReferences;

impl CheckTypeImportCodeReferences {
    // port: CheckTypeImportCodeReferences#CheckTypeImportCodeReferences
    pub fn new() -> Self {
        Self
    }
}

impl Default for CheckTypeImportCodeReferences {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for CheckTypeImportCodeReferences {
    // port: CheckTypeImportCodeReferences#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckTypeImportCodeReferences {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckTypeImportCodeReferences#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_name(t) {
            return;
        }

        let scope = t.get_scope();
        let name = n.get_string(t);
        let Some(var) = scope.get_var(t.get_compiler(), &name) else {
            return;
        };

        let decl_name_node = var.get_name_node(t.get_compiler());

        let Some(decl_name_node) = decl_name_node else {
            return;
        };
        if decl_name_node == n || !NodeUtil::is_declaration_l_value(t, decl_name_node) {
            return;
        }

        let decl = if decl_name_node.get_parent(t).unwrap().is_string_key(t) {
            decl_name_node
                .get_grandparent(t)
                .unwrap()
                .get_grandparent(t)
                .unwrap()
        } else {
            decl_name_node.get_parent(t).unwrap()
        };
        if !NodeUtil::is_name_declaration(t, Some(decl))
            || !decl.has_one_child(t)
            || !decl.get_first_child(t).unwrap().has_children(t)
            || !decl
                .get_first_child(t)
                .unwrap()
                .get_last_child(t)
                .unwrap()
                .is_call(t)
        {
            return;
        }

        let call_node = decl.get_first_child(t).unwrap().get_last_child(t).unwrap();
        if !GOOG_REQUIRE_TYPE.matches(t, call_node.get_first_child(t).unwrap()) {
            return;
        }

        let name = n.get_string(t).to_string();
        t.report(n, &TYPE_IMPORT_CODE_REFERENCE, &[name.as_str()]);
    }
}
