/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteGlobalDeclarationsForTryCatchWrapping.java.

// Identity keys are immutable even though the associated Java objects are mutable.
#![allow(clippy::mutable_key_type)]
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    js_chunk::JSChunk,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{ir::IR, node::NodeId, token::Token};

/// Moves top-level function declarations to the top of the enclosing JSChunk and rewrites class
/// declarations.
pub struct RewriteGlobalDeclarationsForTryCatchWrapping {
    // Java's ArrayListMultimap (HashMap of JSChunk keys, identity hash). Each chunk has its own
    // insertion root, so the key order is not observable; insertion order is kept here.
    functions: IndexMap<Option<JSChunk>, Vec<NodeId>>,
    classes: Vec<NodeId>,
}

impl RewriteGlobalDeclarationsForTryCatchWrapping {
    // port: RewriteGlobalDeclarationsForTryCatchWrapping#RewriteGlobalDeclarationsForTryCatchWrapping
    pub fn new() -> Self {
        Self {
            functions: IndexMap::<_, _>::default(),
            classes: Vec::new(),
        }
    }
}

impl Default for RewriteGlobalDeclarationsForTryCatchWrapping {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for RewriteGlobalDeclarationsForTryCatchWrapping {
    // port: RewriteGlobalDeclarationsForTryCatchWrapping#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        let functions: Vec<(Option<JSChunk>, Vec<NodeId>)> = self
            .functions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, value) in functions {
            let adding_root = compiler.get_node_for_code_insertion(key.as_ref());
            let fn_nodes: Vec<NodeId> = value.into_iter().rev().collect();
            if !fn_nodes.is_empty() {
                for n in fn_nodes {
                    let parent = n.get_parent(compiler).unwrap();
                    n.detach(compiler);
                    compiler.report_change_to_enclosing_scope(parent);

                    let name_node = n.get_first_child(compiler).unwrap();
                    let name = name_node.get_string(compiler);
                    name_node.set_string(compiler, "");
                    let name_n = IR::name(compiler, name);
                    let var = IR::var_with_value(compiler, name_n, n);
                    let var = var.srcref_tree_if_missing(compiler, n);
                    adding_root.add_child_to_front(compiler, var);
                    compiler.report_change_to_enclosing_scope(name_node);
                }
                compiler.report_change_to_enclosing_scope(adding_root);
            }
        }

        let classes = self.classes.clone();
        for n in classes {
            // rewrite CLASS > NAME ... => VAR > NAME > CLASS > EMPTY ...
            let original_name_node = n.get_first_child(compiler).unwrap();
            let empty = IR::empty(compiler);
            original_name_node.replace_with(compiler, empty);
            let var = IR::var(compiler, original_name_node);
            n.replace_with(compiler, var);
            original_name_node.add_child_to_front(compiler, n);
            compiler.report_change_to_enclosing_scope(n);
        }
    }
}

impl Callback for RewriteGlobalDeclarationsForTryCatchWrapping {
    // port: RewriteGlobalDeclarationsForTryCatchWrapping#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let grandparent = n.get_ancestor(t, 2);
        grandparent.is_none() || !grandparent.unwrap().is_script(t)
    }

    // port: RewriteGlobalDeclarationsForTryCatchWrapping#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if parent.is_none() || !parent.unwrap().is_script(t) {
            return;
        }

        if NodeUtil::is_function_declaration(t, n) {
            let chunk = t.get_chunk();
            self.functions.entry(chunk).or_default().push(n);
        }

        if NodeUtil::is_class_declaration(t, n) {
            self.classes.push(n);
        }

        if n.is_const(t) || n.is_let(t) {
            n.set_token(t, Token::VAR);
            t.get_compiler().report_change_to_enclosing_scope(n);
        }
    }
}
