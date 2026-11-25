/*
 * Copyright 2026 The closure-rs Authors.
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

//! Rust-only (performance divergence): the ordered NAME sites and basic-block boundaries in a
//! block whose descendants use NodeTraversal's plain child loop. The block itself still
//! runs through NodeTraversal, including scope creation and its mutating exit callback.

use super::is_non_local_module_specifier_name;
use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::node_traversal::has_traversal_handler;
use crate::node_util::NodeUtil;
use crate::syntactic_scope_cache::SyntacticScopeCache;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::sync::Arc;

const TAG_SHIFT: u32 = 29;
const TAG_REFERENCE: u32 = 0 << TAG_SHIFT;
const TAG_ENTER_BLOCK: u32 = 1 << TAG_SHIFT;
const TAG_EXIT_BLOCK: u32 = 2 << TAG_SHIFT;
const TAG_ENTER_EMPTY_SCOPE: u32 = 3 << TAG_SHIFT;
const TAG_EXIT_EMPTY_SCOPE: u32 = 4 << TAG_SHIFT;
const TAG_ENTER_SCOPE: u32 = 5 << TAG_SHIFT;
const TAG_EXIT_SCOPE: u32 = 6 << TAG_SHIFT;
const PAYLOAD_MASK: u32 = (1 << TAG_SHIFT) - 1;

/// Compact 8-byte site record for cached reference-collection replay.
#[derive(Clone, Copy)]
pub(crate) struct Site(NodeId, u32);

pub(super) enum SiteKind {
    Reference(NodeId, u32),
    EnterBlock(NodeId),
    ExitBlock(NodeId),
    EnterEmptyScope(NodeId),
    ExitEmptyScope,
    /// A nested scope that declares `let`/`const` bindings.
    EnterScope(NodeId),
    ExitScope,
}

impl Site {
    #[inline]
    fn reference(node: NodeId, slot: u32) -> Option<Self> {
        (slot <= PAYLOAD_MASK).then_some(Self(node, TAG_REFERENCE | slot))
    }

    #[inline]
    fn enter_block(node: NodeId) -> Self {
        Self(node, TAG_ENTER_BLOCK)
    }

    #[inline]
    fn exit_block(node: NodeId) -> Self {
        Self(node, TAG_EXIT_BLOCK)
    }

    #[inline]
    fn enter_empty_scope(node: NodeId) -> Self {
        Self(node, TAG_ENTER_EMPTY_SCOPE)
    }

    #[inline]
    fn exit_empty_scope(node: NodeId) -> Self {
        Self(node, TAG_EXIT_EMPTY_SCOPE)
    }

    #[inline]
    fn enter_scope(node: NodeId) -> Self {
        Self(node, TAG_ENTER_SCOPE)
    }

    #[inline]
    fn exit_scope(node: NodeId) -> Self {
        Self(node, TAG_EXIT_SCOPE)
    }

    #[inline]
    pub(super) fn decode(self) -> SiteKind {
        match self.1 & !PAYLOAD_MASK {
            TAG_REFERENCE => SiteKind::Reference(self.0, self.1 & PAYLOAD_MASK),
            TAG_ENTER_BLOCK => SiteKind::EnterBlock(self.0),
            TAG_EXIT_BLOCK => SiteKind::ExitBlock(self.0),
            TAG_ENTER_EMPTY_SCOPE => SiteKind::EnterEmptyScope(self.0),
            TAG_EXIT_EMPTY_SCOPE => SiteKind::ExitEmptyScope,
            TAG_ENTER_SCOPE => SiteKind::EnterScope(self.0),
            _ => SiteKind::ExitScope,
        }
    }
}

pub(crate) struct FunctionBodySites {
    /// First occurrence of each spelling in the order in which the collector visits it.
    pub(crate) first_names: Box<[NodeId]>,
    pub(super) sites: Box<[Site]>,
    /// Whether a scope nested in the body declares `let`/`const` bindings (`SiteKind::EnterScope`).
    /// The names of such a body resolve in more than one scope, and the scopes that declare
    /// bindings have exits a behavior acts on: only `collect_body_from_sites` handles that.
    pub(crate) declares_in_nested_scopes: bool,
}

/// The innermost scope around a node of a function body.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nested {
    /// The scope of the body itself.
    No,
    /// A nested scope without bindings.
    Empty,
    /// A nested scope that declares `let`/`const` bindings.
    Declaring,
}

impl FunctionBodySites {
    fn collect(ast: &Ast, body: NodeId) -> Option<Self> {
        let mut names = IndexMap::default();
        let mut sites = Vec::new();
        let mut child = body.get_first_child(ast);
        while let Some(n) = child {
            if !Self::collect_node(ast, n, body, Nested::No, &mut names, &mut sites) {
                return None;
            }
            child = n.get_next(ast);
        }
        let declares_in_nested_scopes = sites
            .iter()
            .any(|site| matches!(site.decode(), SiteKind::EnterScope(_)));
        Some(Self {
            first_names: names.into_values().collect(),
            sites: sites.into_boxed_slice(),
            declares_in_nested_scopes,
        })
    }

    /// Whether the scope rooted at `n` declares `let` or `const` bindings: as statements of a
    /// block, in the head of a loop, or as statements of the cases of a switch. Classes and
    /// function declarations end the collection of a body, so they need not be looked for.
    fn declares_lexical_bindings(ast: &Ast, n: NodeId, token: Token) -> bool {
        fn any_lexical(ast: &Ast, parent: NodeId) -> bool {
            let mut child = parent.get_first_child(ast);
            while let Some(c) = child {
                if matches!(c.get_token(ast), Token::LET | Token::CONST) {
                    return true;
                }
                child = c.get_next(ast);
            }
            false
        }
        match token {
            Token::BLOCK => any_lexical(ast, n),
            Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => n
                .get_first_child(ast)
                .is_some_and(|c| matches!(c.get_token(ast), Token::LET | Token::CONST)),
            Token::SWITCH_BODY => {
                let mut case = n.get_first_child(ast);
                while let Some(c) = case {
                    if c.get_last_child(ast)
                        .is_some_and(|block| block.is_block(ast) && any_lexical(ast, block))
                    {
                        return true;
                    }
                    case = c.get_next(ast);
                }
                false
            }
            _ => false,
        }
    }

    fn collect_node<'a>(
        ast: &'a Ast,
        n: NodeId,
        parent: NodeId,
        nested: Nested,
        names: &mut IndexMap<&'a JsString, NodeId>,
        sites: &mut Vec<Site>,
    ) -> bool {
        let token = n.get_token(ast);
        if has_traversal_handler(token) || token == Token::CATCH {
            return false;
        }
        if nested != Nested::No && matches!(token, Token::IMPORT | Token::EXPORT) {
            return false;
        }
        // A `let`/`const` in a nested scope belongs to the innermost scope, which was found to
        // declare bindings when it was entered; anywhere else it is not understood here.
        if nested == Nested::Empty && matches!(token, Token::LET | Token::CONST) {
            return false;
        }
        // Rust-only (performance divergence): nested BLOCK/FOR/SWITCH_BODY subtrees that declare
        // no lexical bindings resolve every NAME identically to `body`'s scope and have no-op
        // empty scope exits, while `EnterEmptyScope`/`ExitEmptyScope` reconstruct each Reference's
        // exact ScopeId lazily.
        let creates_scope = match token {
            Token::BLOCK => {
                if n.is_synthetic_block(ast) {
                    return false;
                }
                !NodeUtil::is_switch_case(ast, parent) && !parent.is_catch(ast)
            }
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY => true,
            _ => false,
        };
        let declares = creates_scope && Self::declares_lexical_bindings(ast, n, token);
        let boundary = BasicBlock::is_block_boundary(ast, n, Some(parent));
        let start = sites.len();
        if boundary {
            sites.push(Site::enter_block(n));
        }
        if declares {
            sites.push(Site::enter_scope(n));
        } else if creates_scope {
            sites.push(Site::enter_empty_scope(n));
        }
        let content_start = sites.len();
        let child_nested = if declares {
            Nested::Declaring
        } else if creates_scope {
            Nested::Empty
        } else {
            nested
        };
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            if !Self::collect_node(ast, c, n, child_nested, names, sites) {
                return false;
            }
            child = c.get_next(ast);
        }
        if (token == Token::NAME || token == Token::IMPORT_STAR)
            && !is_non_local_module_specifier_name(ast, n, parent)
        {
            let Some(spelling) = n.string_payload(ast) else {
                // The ordinary walk validates malformed public NAME nodes at their visit.
                return false;
            };
            let name = names.entry(spelling);
            let slot = u32::try_from(name.index()).ok();
            let Some(site) = slot.and_then(|s| Site::reference(n, s)) else {
                return false;
            };
            name.or_insert(n);
            sites.push(site);
        }
        if sites.len() == content_start {
            sites.truncate(start);
        } else {
            if declares {
                sites.push(Site::exit_scope(n));
            } else if creates_scope {
                sites.push(Site::exit_empty_scope(n));
            }
            if boundary {
                sites.push(Site::exit_block(n));
            }
        }
        true
    }

    /// Rust-only (performance divergence): builds a compact replay stream containing only
    /// references whose slot is in `keep_slots` (remapped to `0..keep_slots.len()`), dropping any
    /// nested block or empty-scope frames that contain no kept references. Reuses caller scratch
    /// buffers to avoid per-function temporary allocations.
    pub(crate) fn prune_to_slots(
        &self,
        keep_slots: &[u32],
        remap: &mut Vec<u32>,
        frames: &mut Vec<usize>,
        out: &mut Vec<Site>,
    ) -> Box<[Site]> {
        if keep_slots.is_empty() {
            return Box::new([]);
        }
        remap.clear();
        remap.resize(self.first_names.len(), u32::MAX);
        for (new_slot, &old_slot) in keep_slots.iter().enumerate() {
            remap[old_slot as usize] = new_slot as u32;
        }
        out.clear();
        frames.clear();
        for &site in &self.sites {
            match site.decode() {
                SiteKind::Reference(node, old_slot) => {
                    let new_slot = remap[old_slot as usize];
                    if new_slot != u32::MAX {
                        out.push(Site::reference(node, new_slot).expect("remapped slot fits"));
                    }
                }
                SiteKind::EnterBlock(_)
                | SiteKind::EnterEmptyScope(_)
                | SiteKind::EnterScope(_) => {
                    frames.push(out.len());
                    out.push(site);
                }
                SiteKind::ExitBlock(_) | SiteKind::ExitEmptyScope | SiteKind::ExitScope => {
                    let start = frames.pop().expect("balanced enter/exit frame");
                    if out.len() == start + 1 {
                        out.truncate(start);
                    } else {
                        out.push(site);
                    }
                }
            }
        }
        debug_assert!(frames.is_empty());
        out.as_slice().into()
    }
}

#[derive(Default)]
pub(crate) struct FunctionReferenceCache {
    bodies: IndexMap<NodeId, (u32, Option<Arc<FunctionBodySites>>)>,
}

impl FunctionReferenceCache {
    pub(crate) fn has_cached_sites(&self, body: NodeId) -> bool {
        self.bodies
            .get(&body)
            .is_some_and(|(_, sites)| sites.is_some())
    }

    /// Whether `body` has cached sites and all its names resolve in the scope of the body itself.
    pub(crate) fn has_single_scope_sites(&self, body: NodeId) -> bool {
        self.bodies.get(&body).is_some_and(|(_, sites)| {
            sites
                .as_ref()
                .is_some_and(|sites| !sites.declares_in_nested_scopes)
        })
    }

    pub(crate) fn cached_sites_with_stamp(
        &self,
        body: NodeId,
        stamp: u32,
    ) -> Option<Arc<FunctionBodySites>> {
        self.bodies
            .get(&body)
            .and_then(|(old_stamp, sites)| (*old_stamp == stamp).then(|| sites.clone()).flatten())
    }

    pub(crate) fn sites(
        compiler: &mut AbstractCompiler,
        body: NodeId,
    ) -> Option<Arc<FunctionBodySites>> {
        let stamp = SyntacticScopeCache::subtree_stamp(compiler, body);
        if let Some((old_stamp, sites)) = compiler.function_reference_sites.bodies.get(&body)
            && *old_stamp == stamp
        {
            return sites.clone();
        }
        let sites = FunctionBodySites::collect(compiler, body).map(Arc::new);
        compiler
            .function_reference_sites
            .bodies
            .insert(body, (stamp, sites.clone()));
        sites
    }

    /// Rust-only (performance divergence): finds uncached or stale non-empty function bodies under
    /// `root` and computes their `FunctionBodySites` in parallel on `&Ast` before traversal begins.
    pub(crate) fn precompute_parallel(compiler: &mut AbstractCompiler, root: NodeId) {
        fn collect_dirty(compiler: &mut AbstractCompiler, n: NodeId, out: &mut Vec<(NodeId, u32)>) {
            if n.is_function(compiler)
                && let Some(body) = n.get_last_child(compiler)
                && body.is_block(compiler)
                && body.has_children(compiler)
            {
                let stamp = SyntacticScopeCache::subtree_stamp(compiler, body);
                let up_to_date = compiler
                    .function_reference_sites
                    .bodies
                    .get(&body)
                    .is_some_and(|(old_stamp, _)| *old_stamp == stamp);
                if !up_to_date {
                    out.push((body, stamp));
                }
            }
            let mut child = n.get_first_child(compiler);
            while let Some(c) = child {
                collect_dirty(compiler, c, out);
                child = c.get_next(compiler);
            }
        }

        // Without a second thread every body is collected when the traversal reaches it.
        if crate::parallel_parse::parallel_thread_count(usize::MAX, 64, 8) < 2 {
            return;
        }
        let mut dirty = Vec::new();
        match SyntacticScopeCache::take_changed_functions(compiler) {
            // Only a body in a changed function can be stale. A function made since the last call
            // may be missing from the list; the traversal collects its body.
            Some(changed) => {
                let mut seen = IndexSet::<NodeId>::default();
                for function in changed {
                    if seen.insert(function)
                        && let Some(body) = function.get_last_child(compiler)
                        && body.is_block(compiler)
                        && body.has_children(compiler)
                    {
                        let stamp = SyntacticScopeCache::subtree_stamp(compiler, body);
                        let up_to_date = compiler
                            .function_reference_sites
                            .bodies
                            .get(&body)
                            .is_some_and(|(old_stamp, _)| *old_stamp == stamp);
                        if !up_to_date {
                            dirty.push((body, stamp));
                        }
                    }
                }
            }
            None => collect_dirty(compiler, root, &mut dirty),
        }
        let threads = crate::parallel_parse::parallel_thread_count(dirty.len(), 64, 8);
        if threads < 2 {
            return;
        }
        let ast = &compiler.ast;
        let batches = crate::parallel_parse::run_scoped_indexed(
            "closure-refsites",
            threads,
            &dirty,
            |_idx, &(body, stamp)| {
                let sites = FunctionBodySites::collect(ast, body).map(Arc::new);
                Some((body, stamp, sites))
            },
        );
        for batch in batches {
            for (_idx, (body, stamp, sites)) in batch {
                compiler
                    .function_reference_sites
                    .bodies
                    .insert(body, (stamp, sites));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use closure_rhino::token::Token;

    fn block(ast: &mut Ast, left: &str, right: &str) -> (NodeId, NodeId) {
        let a = ast.new_string_with_token(Token::NAME, left);
        let b = ast.new_string_with_token(Token::NAME, right);
        (ast.new_node_with_children2(Token::BLOCK, a, b), b)
    }

    #[test]
    fn replay_tracks_public_payload_changes_missed_drains_and_arena_replacement() {
        let mut compiler = AbstractCompiler::new();
        let (root, b) = block(&mut compiler.ast, "x", "x");
        let count = |c: &mut AbstractCompiler| {
            FunctionReferenceCache::sites(c, root)
                .unwrap()
                .first_names
                .len()
        };
        assert_eq!(count(&mut compiler), 1);

        let number = compiler.ast.new_number(1.0);
        number.set_double(&mut compiler.ast, 2.0);
        let mut changes = Vec::new();
        compiler.ast.take_changed_nodes(&mut changes);
        assert!(changes.is_empty());

        let mut other = Ast::new();
        for _ in 0..compiler.ast.node_count() {
            other.new_node(Token::EMPTY);
        }
        let new = other.new_string_with_token(Token::NAME, "y");
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = &mut compiler.ast[new];
            }))
            .is_err()
        );
        assert_eq!(count(&mut compiler), 1);
        std::mem::swap(&mut compiler.ast[b], &mut other[new]);
        assert_eq!(count(&mut compiler), 2);
        b.set_string(&mut compiler.ast, "x");
        compiler.ast.take_changed_nodes(&mut changes);
        assert!(!changes.is_empty());
        assert_eq!(count(&mut compiler), 1);

        let mut replacement = Ast::new();
        let (same_id, _) = block(&mut replacement, "a", "b");
        assert_eq!(same_id, root);
        std::mem::swap(&mut compiler.ast, &mut replacement);
        assert_eq!(count(&mut compiler), 2);
        let malformed = other.new_node(Token::EMPTY);
        std::mem::swap(&mut compiler.ast[b], &mut other[malformed]);
        assert!(FunctionReferenceCache::sites(&mut compiler, root).is_none());
    }
}
