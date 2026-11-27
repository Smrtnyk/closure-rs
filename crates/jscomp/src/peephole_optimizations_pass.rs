/*
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
//   src/com/google/javascript/jscomp/PeepholeOptimizationsPass.java.

//! Port of `PeepholeOptimizationsPass.java`.
//!
//! A compiler pass to run various peephole optimizations (e.g. constant folding, some useless
//! code removal, some minimizations).

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{AbstractPeepholeOptimization, FastTokenFilter},
    compiler_pass::CompilerPass,
    node_traversal::NodeTraversal,
    node_util::NodeUtil,
    platform::Platform,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

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
            // Rust-only (performance divergence): walk the AST directly in post-order instead of
            // building a `NodeTraversal` / `MemoizedScopeCreator` for every changed scope root, and
            // filter high-frequency `NAME` / `GETPROP` tokens inline before virtual dispatch.
            let mut walker = PeepWalker {
                candidates: self.candidates.as_deref(),
                peephole_optimizations: &mut self.peephole_optimizations,
                has_pending_features: false,
                last_script: None,
                last_node: None,
            };
            let result = catch_unwind(AssertUnwindSafe(|| match &changed_scope_nodes {
                None => {
                    walker.traverse_full(compiler, root, None);
                    walker.flush_features(compiler, None);
                }
                Some(nodes) => {
                    for &scope_node in nodes {
                        let script = find_enclosing_script(compiler, scope_node);
                        walker.traverse_scope_root(compiler, scope_node, scope_node, script);
                        walker.flush_features(compiler, script);
                    }
                }
            }));
            if let Err(unexpected_exception) = result {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    walker.last_script,
                    walker.last_node,
                    unexpected_exception,
                );
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
    /// The optimizations that may add features (`may_add_features`).
    adds_features: u16,
    unrestricted: u16,
    name_always: u16,
    name_undefined: u16,
    getprop_always: u16,
    getprop_window: u16,
    getprop_literal: u16,
}

impl CandidateRules {
    fn new(optimizations: &[Box<dyn AbstractPeepholeOptimization>]) -> Option<Box<Self>> {
        if optimizations.len() > u16::BITS as usize {
            return None;
        }
        let mut rules = Box::new(Self {
            tokens: [0; Token::PLACEHOLDER3 as usize + 1],
            adds_features: 0,
            unrestricted: 0,
            name_always: 0,
            name_undefined: 0,
            getprop_always: 0,
            getprop_window: 0,
            getprop_literal: 0,
        });
        for (index, optimization) in optimizations.iter().enumerate() {
            let bit = 1 << index;
            if optimization.may_add_features() {
                rules.adds_features |= bit;
            }
            match optimization.candidate_tokens() {
                Some(tokens) => {
                    for &token in tokens {
                        *rules.tokens.get_mut(token as usize)? |= bit;
                        match token {
                            Token::NAME => match optimization.fast_token_filter(Token::NAME) {
                                FastTokenFilter::NameIsUndefined => rules.name_undefined |= bit,
                                _ => rules.name_always |= bit,
                            },
                            Token::GETPROP => {
                                match optimization.fast_token_filter(Token::GETPROP) {
                                    FastTokenFilter::GetPropOnWindow => rules.getprop_window |= bit,
                                    FastTokenFilter::GetPropOnLiteral => {
                                        rules.getprop_literal |= bit
                                    }
                                    _ => rules.getprop_always |= bit,
                                }
                            }
                            _ => {}
                        }
                    }
                }
                None => {
                    rules.unrestricted |= bit;
                    rules.name_always |= bit;
                    rules.getprop_always |= bit;
                }
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

    #[inline]
    fn mask_for_node(&self, ast: &Ast, n: NodeId, token: Token) -> u16 {
        match token {
            Token::NAME => {
                let mut mask = self.name_always;
                if self.name_undefined != 0
                    && n.string_payload(ast).is_some_and(|s| s == "undefined")
                {
                    mask |= self.name_undefined;
                }
                mask
            }
            Token::GETPROP => {
                let mut mask = self.getprop_always;
                if (self.getprop_window | self.getprop_literal) != 0
                    && let Some(left) = n.get_first_child(ast)
                {
                    match left.get_token(ast) {
                        Token::NAME
                            if self.getprop_window != 0
                                && left.string_payload(ast).is_some_and(|s| s == "window") =>
                        {
                            mask |= self.getprop_window;
                        }
                        Token::OBJECTLIT | Token::ARRAYLIT | Token::STRINGLIT => {
                            mask |= self.getprop_literal;
                        }
                        _ => {}
                    }
                }
                mask
            }
            _ => self
                .tokens
                .get(token as usize)
                .copied()
                .unwrap_or(self.unrestricted),
        }
    }
}

fn find_enclosing_script(ast: &Ast, mut n: NodeId) -> Option<NodeId> {
    loop {
        if n.is_script(ast) {
            return Some(n);
        }
        n = n.get_parent(ast)?;
    }
}

#[inline]
fn is_scope_exit_token(token: Token) -> bool {
    matches!(
        token,
        Token::FUNCTION
            | Token::MODULE_BODY
            | Token::CLASS
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_FIELD_DEF
            | Token::BLOCK
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY
    )
}

struct PeepWalker<'a> {
    candidates: Option<&'a CandidateRules>,
    peephole_optimizations: &'a mut [Box<dyn AbstractPeepholeOptimization>],
    has_pending_features: bool,
    last_script: Option<NodeId>,
    last_node: Option<NodeId>,
}

impl PeepWalker<'_> {
    fn traverse_full(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        current_script: Option<NodeId>,
    ) {
        let token = n.get_token(compiler);
        let script = if token == Token::SCRIPT {
            if Platform::is_thread_interrupted() {
                self.last_script = Some(n);
                self.last_node = Some(n);
                panic!("java.lang.InterruptedException");
            }
            Some(n)
        } else {
            current_script
        };
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            let next = c.get_next(compiler);
            self.traverse_full(compiler, c, script);
            child = next;
        }
        if self.has_pending_features && is_scope_exit_token(token) {
            self.flush_features(compiler, script);
        }
        let current_token = n.get_token(compiler);
        self.visit_node(compiler, n, current_token, script);
        if self.has_pending_features && token == Token::SCRIPT {
            self.flush_features(compiler, script);
        }
    }

    fn traverse_scope_root(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope_node: NodeId,
        n: NodeId,
        current_script: Option<NodeId>,
    ) {
        let token = n.get_token(compiler);
        // `ChangeTracker::is_change_scope_root` of the token read once.
        if n != scope_node && matches!(token, Token::SCRIPT | Token::FUNCTION) {
            return;
        }
        let script = if token == Token::SCRIPT {
            if Platform::is_thread_interrupted() {
                self.last_script = Some(n);
                self.last_node = Some(n);
                panic!("java.lang.InterruptedException");
            }
            Some(n)
        } else {
            current_script
        };
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            let next = c.get_next(compiler);
            self.traverse_scope_root(compiler, scope_node, c, script);
            child = next;
        }
        if self.has_pending_features && is_scope_exit_token(token) {
            self.flush_features(compiler, script);
        }
        let current_token = n.get_token(compiler);
        self.visit_node(compiler, n, current_token, script);
    }

    #[inline]
    fn visit_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        initial_token: Token,
        current_script: Option<NodeId>,
    ) {
        let mut current_node = n;
        let mut next = 0;
        if let Some(rules) = self.candidates {
            let mut candidates = rules.mask_for_node(compiler, current_node, initial_token);
            if candidates == 0 {
                return;
            }
            self.last_script = current_script;
            self.last_node = Some(n);
            loop {
                let i = candidates.trailing_zeros() as usize;
                let optim = &mut self.peephole_optimizations[i];
                let result = optim.optimize_subtree(compiler, current_node);
                if rules.adds_features & (1 << i) != 0 && optim.has_new_features() {
                    self.has_pending_features = true;
                }
                let Some(result) = result else {
                    return;
                };
                current_node = result;
                next = i + 1;
                if next >= self.peephole_optimizations.len() {
                    return;
                }
                let remaining = (u16::MAX << i) << 1;
                let Some(token) = current_node.token_if_present(compiler) else {
                    break;
                };
                candidates = rules.mask_for_node(compiler, current_node, token) & remaining;
                if candidates == 0 {
                    return;
                }
            }
        }
        self.last_script = current_script;
        self.last_node = Some(n);
        for optim in &mut self.peephole_optimizations[next..] {
            match optim.optimize_subtree(compiler, current_node) {
                Some(node) => {
                    if optim.has_new_features() {
                        self.has_pending_features = true;
                    }
                    current_node = node;
                }
                None => {
                    if optim.has_new_features() {
                        self.has_pending_features = true;
                    }
                    return;
                }
            }
        }
    }

    // port: PeepholeOptimizationsPass.PeepCallback#updateFeatures
    fn flush_features(&mut self, compiler: &mut AbstractCompiler, current_script: Option<NodeId>) {
        if !self.has_pending_features {
            return;
        }
        self.has_pending_features = false;
        for optim in self.peephole_optimizations.iter_mut() {
            let new_features = optim.get_new_features();
            if !new_features.is_empty()
                && let Some(script) = current_script
            {
                NodeUtil::add_features_to_script(
                    compiler,
                    script,
                    FeatureSet::BARE_MINIMUM.with_set(&new_features),
                );
            }
            optim.clear_new_features();
        }
    }
}
