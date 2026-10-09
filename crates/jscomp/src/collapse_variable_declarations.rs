/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CollapseVariableDeclarations.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of CollapseVariableDeclarations.java.
//!
//! Collapses multiple variable declarations into a single one. i.e the following:
//!
//! ```text
//! var a;
//! var b = 1;
//! var c = 2;
//! ```
//!
//! becomes:
//!
//! ```text
//! var a, b = 1, c = 2;
//! ```
//!
//! This reduces the generated code size. More optimizations are possible:
//! - Group all variable declarations inside a function into one such variable. declaration block.
//! - Re-use variables instead of declaring a new one if they are used for only part of a
//!   function. Similarly, also collapses assigns like:
//!
//!   ```text
//!   a = true;
//!   b = true;
//!   var c = true;
//!   ```
//!
//!   becomes:
//!
//!   ```text
//!   var c = b = a = true;
//!   ```
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{check_state, node::NodeId};

/// Encapsulation of information about a variable declaration collapse
struct Collapse {
    /// Variable declaration that any following var nodes should be collapsed into
    start_node: NodeId,
}

impl Collapse {
    // port: CollapseVariableDeclarations.Collapse#Collapse
    fn new(start_node: NodeId) -> Self {
        Self { start_node }
    }
}

pub struct CollapseVariableDeclarations {
    /// Collapses to do in this pass.
    collapses: Vec<Collapse>,
    /// Nodes we've already looked at for collapsing, so that we don't look at them again (we look
    /// ahead when examining what nodes can be collapsed, and the node traversal may give them to
    /// us again)
    nodes_to_collapse: IndexSet<NodeId>,
}

impl CollapseVariableDeclarations {
    // port: CollapseVariableDeclarations#CollapseVariableDeclarations
    pub fn new(compiler: &AbstractCompiler) -> Self {
        check_state!(!compiler.get_life_cycle_stage().is_normalized());
        Self {
            collapses: Vec::new(),
            nodes_to_collapse: IndexSet::<_>::default(),
        }
    }

    // port: CollapseVariableDeclarations#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        self.collapses.clear();
        self.nodes_to_collapse.clear();

        NodeTraversal::traverse(compiler, root, &mut GatherCollapses { this: self });

        if !self.collapses.is_empty() {
            self.apply_collapses(compiler);
        }
    }

    // port: CollapseVariableDeclarations#applyCollapses
    fn apply_collapses(&mut self, compiler: &mut AbstractCompiler) {
        for collapse in &self.collapses {
            let var = collapse.start_node;
            compiler.report_change_to_enclosing_scope(var);

            while let Some(var_next) = var.get_next(compiler)
                && var_next.get_token(compiler) == var.get_token(compiler)
            {
                let next = var_next.detach(compiler);

                // Move all children of the next var node into the first one.
                let children = next.remove_children(compiler);
                var.add_children_to_back(compiler, children);
            }
        }
    }
}

impl CompilerPass for CollapseVariableDeclarations {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        CollapseVariableDeclarations::process(self, compiler, externs, root);
    }
}

/// Gathers all of the variable declarations that should be collapsed into one.
///
/// We do not do the collapsing as we go since node traversal would be affected by the changes we
/// are making to the parse tree.
struct GatherCollapses<'a> {
    this: &'a mut CollapseVariableDeclarations,
}

impl Callback for GatherCollapses<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CollapseVariableDeclarations.GatherCollapses#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // If we've already looked at this node, skip it
        if self.this.nodes_to_collapse.contains(&n) {
            return;
        }

        if !NodeUtil::is_name_declaration(t, Some(n)) {
            return;
        }

        // Adjacent VAR children of an IF node are the if and else parts and can't
        // be collapsed
        if parent.unwrap().is_if(t) {
            return;
        }

        let var_node = n;
        let n_type = n.get_token(t);

        // Find variable declarations that follow this one (if any)
        let mut n = n.get_next(t);
        let mut has_nodes_to_collapse = false;

        // we only want to collapse lets with lets, vars with vars, and consts with consts so we
        // check to make sure the declaration types match
        while let Some(cur) = n
            && n_type == cur.get_token(t)
        {
            self.this.nodes_to_collapse.insert(cur);
            has_nodes_to_collapse = true;

            n = cur.get_next(t);
        }

        if has_nodes_to_collapse {
            self.this.nodes_to_collapse.insert(var_node);
            self.this.collapses.push(Collapse::new(var_node));
        }
    }
}
