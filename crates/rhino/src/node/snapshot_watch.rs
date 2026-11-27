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

//! Tracks edits to contiguous, freshly loaded parser snapshots, including their detached
//! JSDoc type trees. Every child insertion or removal marks its parent. Moving a tree
//! across ranges therefore marks both its old and new owners.
use super::{Ast, NodeId};
use crate::token::Token;
use std::any::Any;

pub(super) const STRUCTURE: u8 = 1;
pub(super) const NAME: u8 = 2;
pub(super) const DOC: u8 = 4;
pub(super) const PARAM_NAME: u8 = 8;

/// Which inputs a build-time script record depends on. Syntax includes tokens, child
/// links and property spellings. An identifier rename or a JSDoc replacement affects only
/// consumers that inspect those values. `SyntaxAndNonParamNames` tolerates plain function
/// parameter renames (such as `MakeDeclaredNamesUnique` during `Normalize`) while still
/// invalidating on any global/body identifier rename.
#[derive(Clone, Copy)]
pub enum SnapshotUse {
    Syntax,
    SyntaxAndNames,
    SyntaxAndNonParamNames,
    SyntaxAndJsDoc,
    SyntaxNamesAndJsDoc,
}

#[derive(Debug)]
pub(super) struct Range {
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) root: NodeId,
    last_statement: Option<NodeId>,
    data: &'static (dyn Any + Send + Sync),
    changes: u8,
    /// The properties the range's nodes were loaded with, when the loader recorded them.
    pub(super) loaded: Option<Box<LoadedProps>>,
}

/// Rust-only (performance divergence): the property state a packed bundled snapshot gave the
/// nodes of its range (see `Ast::prop_digest_range_since_load`). Property lists are immutable and
/// replaced, never edited, and this record keeps the loaded lists alive, so a node whose list is
/// still the loaded one (by address) and whose presence bits are the loaded ones has exactly the
/// properties it was loaded with.
#[derive(Debug)]
pub(super) struct LoadedProps {
    pub(super) start: u32,
    /// The distinct lists with their presence bits; nodes refer to them by `index`.
    pub(super) lists: Vec<(Option<std::sync::Arc<super::PropListItem>>, u64)>,
    pub(super) index: Vec<u16>,
    pub(super) source_name: String,
}

#[derive(Default, Debug)]
pub(super) struct Watches {
    pub(super) ranges: Vec<Range>,
    end: u32,
    /// What the last packed bundled load recorded, until its range is watched.
    pub(super) pending_loaded: Option<Box<LoadedProps>>,
}

impl Watches {
    #[inline]
    pub(super) fn change(&mut self, n: NodeId, changes: u8) {
        self.mark(n, changes, false);
    }

    #[inline]
    pub(super) fn append_child(&mut self, parent: NodeId) {
        self.mark(parent, STRUCTURE, true);
    }

    #[inline]
    fn mark(&mut self, n: NodeId, changes: u8, append: bool) {
        let i = n.0.get() - 1;
        if i < self.end {
            let range = self.ranges.partition_point(|range| range.end <= i);
            let range = &mut self.ranges[range];
            if i >= range.start && !(append && n == range.root) {
                range.changes |= changes;
            }
        }
    }
}

impl Ast {
    /// Associate a build-time record with this exact script and its freshly loaded
    /// nodes. Detached parse nodes such as JSDoc field types belong to the same range.
    pub fn watch_parsed_nodes<T: Any + Send + Sync>(
        &mut self,
        start: u32,
        root: NodeId,
        data: &'static T,
    ) {
        let end = self.node_count();
        assert!(self.snapshot_watches.end <= start && (start..end).contains(&(root.0.get() - 1)));
        let loaded = self
            .snapshot_watches
            .pending_loaded
            .take()
            .filter(|loaded| loaded.start == start && loaded.index.len() == (end - start) as usize);
        self.snapshot_watches.ranges.push(Range {
            start,
            end,
            root,
            last_statement: root.get_last_child(self),
            data,
            changes: 0,
            loaded,
        });
        self.snapshot_watches.end = end;
    }

    /// Retrieve the record only for its own script in this arena while the specified
    /// inputs remain unchanged. Other parses and active source overrides have no record.
    pub fn parsed_script_record<T: Any>(
        &self,
        script: NodeId,
        usage: SnapshotUse,
    ) -> Option<(&'static T, Option<NodeId>)> {
        let i = script.0.get() - 1;
        if i >= self.snapshot_watches.end {
            return None;
        }
        let index = self
            .snapshot_watches
            .ranges
            .partition_point(|range| range.end <= i);
        let range = &self.snapshot_watches.ranges[index];
        if range.root != script {
            return None;
        }
        let mask = match usage {
            SnapshotUse::Syntax => STRUCTURE,
            SnapshotUse::SyntaxAndNames => STRUCTURE | NAME | PARAM_NAME,
            SnapshotUse::SyntaxAndNonParamNames => STRUCTURE | NAME,
            SnapshotUse::SyntaxAndJsDoc => {
                // Each parse may use the same out-of-range template bound. It contributes
                // no fields while it is still the ordinary leaf QMARK.
                if let Some(bound) = &self.implicit_template_bound {
                    let root = bound.get_root();
                    if root.arena_index() >= self.nodes.len()
                        || root.get_token(self) != Token::QMARK
                        || root.has_children(self)
                    {
                        return None;
                    }
                }
                STRUCTURE | DOC
            }
            SnapshotUse::SyntaxNamesAndJsDoc => STRUCTURE | NAME | PARAM_NAME | DOC,
        };
        if range.changes & mask != 0 {
            return None;
        }
        let first_new_statement = match range.last_statement {
            Some(last) => last.get_next(self),
            None => script.get_first_child(self),
        };
        Some((range.data.downcast_ref()?, first_new_statement))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{js_type_expression::JSTypeExpression, jsdoc_info::JSDocInfo};

    #[test]
    fn records_belong_to_the_original_nodes_and_track_only_their_inputs() {
        let mut ast = Ast::new();
        let bound = JSTypeExpression::implicit_template_bound(&mut ast).get_root();
        let start = ast.node_count();
        let name = ast.new_string_with_token(Token::NAME, "x");
        let script = ast.new_node_with_child(Token::SCRIPT, name);
        ast.watch_parsed_nodes(start, script, &7u8);
        let gap = ast.new_node(Token::SCRIPT);
        let start = ast.node_count();
        let empty = ast.new_node(Token::SCRIPT);
        ast.watch_parsed_nodes(start, empty, &9u8);
        assert!(
            ast.parsed_script_record::<u8>(gap, SnapshotUse::Syntax)
                .is_none()
        );
        assert!(
            ast.parsed_script_record::<usize>(script, SnapshotUse::Syntax)
                .is_none()
        );

        let added = ast.new_node(Token::EMPTY);
        script.add_child_to_back(&mut ast, added);
        let other = ast.new_node(Token::EMPTY);
        empty.add_child_to_back(&mut ast, other);
        name.set_string(&mut ast, "renamed");
        assert_eq!(
            ast.parsed_script_record::<u8>(script, SnapshotUse::Syntax),
            Some((&7, Some(added)))
        );
        assert!(
            ast.parsed_script_record::<u8>(script, SnapshotUse::SyntaxAndNames)
                .is_none()
        );
        assert!(
            ast.parsed_script_record::<u8>(script, SnapshotUse::SyntaxAndJsDoc)
                .is_some()
        );

        let mut doc = JSDocInfo::builder();
        doc.record_no_side_effects();
        name.set_jsdoc_info(&mut ast, doc.build());
        assert!(
            ast.parsed_script_record::<u8>(script, SnapshotUse::SyntaxAndJsDoc)
                .is_none()
        );
        assert!(
            ast.parsed_script_record::<u8>(script, SnapshotUse::Syntax)
                .is_some()
        );
        name.detach(&mut ast);
        assert!(
            ast.parsed_script_record::<u8>(script, SnapshotUse::Syntax)
                .is_none()
        );

        assert_eq!(
            ast.parsed_script_record::<u8>(empty, SnapshotUse::SyntaxNamesAndJsDoc),
            Some((&9, Some(other)))
        );
        bound.set_token(&mut ast, Token::LB);
        assert!(
            ast.parsed_script_record::<u8>(empty, SnapshotUse::SyntaxAndJsDoc)
                .is_none()
        );
        assert!(
            ast.parsed_script_record::<u8>(empty, SnapshotUse::Syntax)
                .is_some()
        );
    }
}
