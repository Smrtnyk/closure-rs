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
use crate::node_traversal::traverses_children_in_order_without_scope;
use crate::syntactic_scope_cache::SyntacticScopeCache;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use std::sync::Arc;

pub(super) enum Site {
    Reference(NodeId, u32),
    EnterBlock(NodeId),
    ExitBlock(NodeId),
}

pub(super) struct FunctionBodySites {
    /// First occurrence of each spelling in the order in which the collector visits it.
    pub(super) first_names: Box<[NodeId]>,
    pub(super) sites: Box<[Site]>,
}

impl FunctionBodySites {
    fn collect(ast: &Ast, body: NodeId) -> Option<Self> {
        let mut names = IndexMap::default();
        let mut sites = Vec::new();
        let mut child = body.get_first_child(ast);
        while let Some(n) = child {
            if !Self::collect_node(ast, n, body, &mut names, &mut sites) {
                return None;
            }
            child = n.get_next(ast);
        }
        Some(Self {
            first_names: names.into_values().collect(),
            sites: sites.into_boxed_slice(),
        })
    }

    fn collect_node<'a>(
        ast: &'a Ast,
        n: NodeId,
        parent: NodeId,
        names: &mut IndexMap<&'a JsString, NodeId>,
        sites: &mut Vec<Site>,
    ) -> bool {
        if !traverses_children_in_order_without_scope(n.get_token(ast)) {
            return false;
        }
        let boundary = BasicBlock::is_block_boundary(ast, n, Some(parent));
        let start = sites.len();
        if boundary {
            sites.push(Site::EnterBlock(n));
        }
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            if !Self::collect_node(ast, c, n, names, sites) {
                return false;
            }
            child = c.get_next(ast);
        }
        if (n.is_name(ast) || n.is_import_star(ast))
            && !is_non_local_module_specifier_name(ast, n, parent)
        {
            let Some(spelling) = n.string_payload(ast) else {
                // The ordinary walk validates malformed public NAME nodes at their visit.
                return false;
            };
            let name = names.entry(spelling);
            let slot = u32::try_from(name.index()).unwrap();
            name.or_insert(n);
            sites.push(Site::Reference(n, slot));
        }
        if boundary {
            if sites.len() == start + 1 {
                sites.truncate(start);
            } else {
                sites.push(Site::ExitBlock(n));
            }
        }
        true
    }
}

#[derive(Default)]
pub(crate) struct FunctionReferenceCache {
    bodies: IndexMap<NodeId, (u32, Option<Arc<FunctionBodySites>>)>,
}

impl FunctionReferenceCache {
    pub(super) fn sites(
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
