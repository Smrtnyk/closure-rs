/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/PeepholeOptimizationsPass.java.

//! Port of `PeepholeOptimizationsPass.java`.
//!
//! A compiler pass to run various peephole optimizations (e.g. constant folding, some useless
//! code removal, some minimizations).

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::{node::NodeId, token::Token};

pub struct PeepholeOptimizationsPass {
    pass_name: String,
    // NOTE: Use a native array rather than a List to avoid creating iterators for every node in
    // the AST.
    peephole_optimizations: Vec<Box<dyn AbstractPeepholeOptimization>>,
    retraverse_on_change: bool,
    candidates: Option<Box<CandidateRules>>,
}

impl PeepholeOptimizationsPass {
    /// Creates a peephole optimization pass that runs the given optimizations.
    // port: PeepholeOptimizationsPass#PeepholeOptimizationsPass(AbstractCompiler, String, List)
    // port: PeepholeOptimizationsPass#PeepholeOptimizationsPass(AbstractCompiler, String, AbstractPeepholeOptimization...)
    // (Rust has no overloads: the varargs constructor only wraps its array in a list, so both
    // constructors are this function, which takes the list.)
    pub fn new(
        pass_name: impl Into<String>,
        optimizations: Vec<Box<dyn AbstractPeepholeOptimization>>,
    ) -> Self {
        let candidates = CandidateRules::new(&optimizations);
        Self {
            candidates,
            pass_name: pass_name.into(),
            peephole_optimizations: optimizations,
            retraverse_on_change: true,
        }
    }

    // port: PeepholeOptimizationsPass#setRetraverseOnChange
    pub fn set_retraverse_on_change(&mut self, retraverse: bool) {
        self.retraverse_on_change = retraverse;
    }

    /// Make sure that all the optimizations have the current compiler so they can report errors.
    // port: PeepholeOptimizationsPass#beginTraversal
    fn begin_traversal(&mut self, compiler: &mut AbstractCompiler) {
        for optimization in self.peephole_optimizations.iter_mut() {
            optimization.begin_traversal(compiler);
        }
    }

    /// End the traversal.
    // port: PeepholeOptimizationsPass#endTraversal
    fn end_traversal(&mut self) {
        for optimization in self.peephole_optimizations.iter_mut() {
            optimization.end_traversal();
        }
    }
}

impl CompilerPass for PeepholeOptimizationsPass {
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    // port: PeepholeOptimizationsPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        self.begin_traversal(compiler);

        // Repeat to an internal fixed point.
        let mut changed_scope_nodes = compiler
            .get_change_tracker()
            .get_changed_scope_nodes_for_pass(&self.pass_name);
        while changed_scope_nodes
            .as_ref()
            .is_none_or(|nodes| !nodes.is_empty())
        {
            match &changed_scope_nodes {
                None => {
                    // changedScopeNodes is null if this is the first run of
                    // peepholeOptimizationsPass.
                    NodeTraversal::traverse(
                        compiler,
                        root,
                        &mut PeepCallback {
                            peephole_optimizations: &mut self.peephole_optimizations,
                            candidates: self.candidates.as_deref(),
                        },
                    );
                }
                Some(nodes) => {
                    NodeTraversal::traverse_scope_roots(
                        compiler,
                        nodes,
                        &mut PeepCallback {
                            peephole_optimizations: &mut self.peephole_optimizations,
                            candidates: self.candidates.as_deref(),
                        },
                        /* traverseNested= */ false,
                    );
                }
            }

            // Cancel the fixed point if requested.
            if !self.retraverse_on_change {
                break;
            }
            changed_scope_nodes = compiler
                .get_change_tracker()
                .get_changed_scope_nodes_for_pass(&self.pass_name);
        }

        self.end_traversal();
    }
}

/// One bit per optimization in pass order. Unrestricted rules run for every token. A long
/// user-provided sequence retains the ordinary loop.
struct CandidateRules {
    tokens: [u16; Token::PLACEHOLDER3 as usize + 1],
    unrestricted: u16,
}

impl CandidateRules {
    fn new(optimizations: &[Box<dyn AbstractPeepholeOptimization>]) -> Option<Box<Self>> {
        if optimizations.len() > u16::BITS as usize {
            return None;
        }
        let mut rules = Box::new(Self {
            tokens: [0; Token::PLACEHOLDER3 as usize + 1],
            unrestricted: 0,
        });
        for (index, optimization) in optimizations.iter().enumerate() {
            let bit = 1 << index;
            match optimization.candidate_tokens() {
                Some(tokens) => {
                    for &token in tokens {
                        *rules.tokens.get_mut(token as usize)? |= bit;
                    }
                }
                None => rules.unrestricted |= bit,
            }
        }
        if rules.unrestricted.count_ones() as usize == optimizations.len() {
            return None;
        }
        for mask in &mut rules.tokens {
            *mask |= rules.unrestricted;
        }
        Some(rules)
    }

    fn for_token(&self, token: Token) -> u16 {
        self.tokens
            .get(token as usize)
            .copied()
            .unwrap_or(self.unrestricted)
    }
}

struct PeepCallback<'a> {
    candidates: Option<&'a CandidateRules>,
    peephole_optimizations: &'a mut [Box<dyn AbstractPeepholeOptimization>],
}

impl Callback for PeepCallback<'_> {
    // port: NodeTraversal.AbstractScopedCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: PeepholeOptimizationsPass.PeepCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let mut current_node = n;
        let mut next = 0;
        if let Some(rules) = self.candidates {
            let mut remaining = u16::MAX;
            while next < self.peephole_optimizations.len() {
                // A rewrite can return a different node or retag the current one in place.
                // Leave foreign IDs to the ordinary callbacks, which may replace them again.
                let Some(token) = current_node.token_if_present(t) else {
                    break;
                };
                let candidates = rules.for_token(token) & remaining;
                if candidates == 0 {
                    return;
                }
                let i = candidates.trailing_zeros() as usize;
                let Some(result) =
                    self.peephole_optimizations[i].optimize_subtree(t.get_compiler(), current_node)
                else {
                    return;
                };
                current_node = result;
                next = i + 1;
                remaining = (u16::MAX << i) << 1;
            }
        }
        for optim in &mut self.peephole_optimizations[next..] {
            match optim.optimize_subtree(t.get_compiler(), current_node) {
                Some(node) => current_node = node,
                None => return,
            }
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for PeepCallback<'_> {
    // port: PeepholeOptimizationsPass.PeepCallback#enterScope
    fn enter_scope(&mut self, _t: &mut NodeTraversal<'_>) {}

    // port: PeepholeOptimizationsPass.PeepCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        // Call updateFeatures() here, instead of immediately after
        // `optim.optimizeSubtree(currentNode)` in visit, to avoid repeating work for
        // every AST node x every peephole optimization. In practice, profiling shows a small
        // but measurable improvement for large projects. See comments on cl/856752797.
        self.update_features(t);
    }
}

impl PeepCallback<'_> {
    // port: PeepholeOptimizationsPass.PeepCallback#updateFeatures
    fn update_features(&mut self, t: &mut NodeTraversal<'_>) {
        for optim in self.peephole_optimizations.iter_mut() {
            let new_features = optim.get_new_features();
            if !new_features.is_empty() {
                let current_script = t.get_current_script().unwrap();
                NodeUtil::add_features_to_script(
                    t.get_compiler(),
                    current_script,
                    FeatureSet::BARE_MINIMUM.with_set(&new_features),
                );
            }
            optim.clear_new_features();
        }
    }
}
