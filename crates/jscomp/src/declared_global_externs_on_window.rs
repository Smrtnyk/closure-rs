/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DeclaredGlobalExternsOnWindow.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `DeclaredGlobalExternsOnWindow.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::{Builder, JSDocInfo};
use closure_rhino::node::{Ast, NodeId};

// port: DeclaredGlobalExternsOnWindow#WINDOW_NAME
const WINDOW_NAME: &str = "window";

/// A compiler pass to normalize externs by declaring global names on the "window" object, if it
/// is declared in externs. The new declarations are added to the window instance, not to
/// Window.prototype.
pub struct DeclaredGlobalExternsOnWindow {
    nodes: IndexSet<NodeId>,

    // Whether there is a "var window" declaration in the externs.
    window_in_externs: bool,
}

impl DeclaredGlobalExternsOnWindow {
    // port: DeclaredGlobalExternsOnWindow#DeclaredGlobalExternsOnWindow
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            nodes: IndexSet::<_>::default(),
            window_in_externs: false,
        }
    }

    // port: DeclaredGlobalExternsOnWindow#addWindowProperties
    fn add_window_properties(&mut self, compiler: &mut AbstractCompiler) {
        if !self.nodes.is_empty() {
            for node in self.nodes.iter().copied() {
                Self::add_extern(compiler, node, self.window_in_externs);
                compiler.report_change_to_enclosing_scope(node);
            }
        }
    }

    // port: DeclaredGlobalExternsOnWindow#addExtern
    fn add_extern(ast: &mut Ast, node: NodeId, define_on_window: bool) {
        let name = node.get_string(ast);
        let old_jsdoc_info = NodeUtil::get_best_jsdoc_info(ast, node);

        let global_ref = if define_on_window {
            IR::name(ast, WINDOW_NAME)
        } else {
            IR::this_node(ast)
        };
        let getprop = IR::getprop(ast, global_ref, name.clone());
        let mut new_node = getprop;

        if let Some(old_jsdoc_info) = old_jsdoc_info {
            let mut builder: Builder;

            if old_jsdoc_info.is_constructor_or_interface()
                || old_jsdoc_info.has_enum_parameter_type()
            {
                let name_node = IR::name(ast, name.clone());
                new_node = IR::assign(ast, getprop, name_node);

                builder = JSDocInfo::builder();
                if old_jsdoc_info.is_constructor() {
                    builder.record_constructor();
                }
                if old_jsdoc_info.is_interface() {
                    builder.record_interface();
                }
                if old_jsdoc_info.uses_implicit_match() {
                    builder.record_implicit_match();
                }
                if old_jsdoc_info.has_enum_parameter_type() {
                    builder.record_enum_parameter_type(old_jsdoc_info.get_enum_parameter_type());
                }
            } else {
                if NodeUtil::is_namespace_decl(ast, node) {
                    let name_node = IR::name(ast, name.clone());
                    new_node = IR::assign(ast, getprop, name_node);
                } else {
                    let rhs = NodeUtil::get_r_value_of_l_value(ast, node);
                    // Type-aliasing definition
                    if old_jsdoc_info.has_const_annotation()
                        && let Some(rhs) = rhs
                        && rhs.is_qualified_name(ast)
                    {
                        let rhs_clone = rhs.clone_tree(ast);
                        new_node = IR::assign(ast, getprop, rhs_clone);
                    }
                }
                builder = Builder::copy_from(&old_jsdoc_info);
            }

            // TODO(blickly): Remove these suppressions when all externs declarations on window are
            // gone.
            let suppressions: IndexSet<JsString> =
                [JsString::from("const"), JsString::from("duplicate")]
                    .into_iter()
                    .collect();
            builder.record_suppressions(&suppressions);
            let jsdoc_info = builder.build();
            new_node.set_jsdoc_info(ast, jsdoc_info);
        }

        new_node.srcref_tree(ast, node);
        new_node.set_original_name(ast, Some(name));
        new_node.make_non_indexable_recursive(ast);
        let expr_result = IR::expr_result(ast, new_node);
        node.get_grandparent(ast)
            .unwrap()
            .add_child_to_back(ast, expr_result);
    }
}

impl CompilerPass for DeclaredGlobalExternsOnWindow {
    // port: DeclaredGlobalExternsOnWindow#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, _root: NodeId) {
        NodeTraversal::traverse(compiler, externs, self);
        self.add_window_properties(compiler);
    }
}

impl Callback for DeclaredGlobalExternsOnWindow {
    // port: NodeTraversal.AbstractShallowStatementCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        _n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        NodeUtil::is_shallow_statement_tree(t, parent)
    }

    // port: DeclaredGlobalExternsOnWindow#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_function(t) {
            self.nodes.insert(n.get_first_child(t).unwrap());
        } else if n.is_var(t) {
            let mut c = n.get_first_child(t);
            while let Some(cur) = c {
                if cur.get_string(t) == WINDOW_NAME {
                    self.window_in_externs = true;
                    c = cur.get_next(t);
                    continue;
                }
                // Skip 'location' since there is an existing definition
                // for window.location which conflicts with the "var location" one.
                if cur.get_string_ref(t) != "location" {
                    self.nodes.insert(cur);
                }
                c = cur.get_next(t);
            }
        }
    }
}
