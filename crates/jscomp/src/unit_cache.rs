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

//! Rust-only, not in Java (crates/DESIGN.md, "Performance divergences"): what a walk of a pass
//! found in one top-level unit (a statement of a script or of a module body,
//! `parallel_parse::statement_units`), kept for the pass's next run while nothing in the unit
//! changed. The optimization loop runs its passes several times, and most statements are
//! unchanged in between; a walk that reads only the unit's subtree finds the same again.

use crate::abstract_compiler::AbstractCompiler;
use crate::syntactic_scope_cache::SyntacticScopeCache;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;

/// The change stamps a unit's record is kept under: the unit's parent when it was walked and
/// the `SyntacticScopeCache::cached_subtree_stamp` of the unit and of every function in it. A
/// change anywhere in the unit advances one of them (a function is a change scope of its own; a
/// function added later changes the change scope it is added to).
pub(crate) struct UnitStamps {
    parent: Option<NodeId>,
    stamps: Box<[(NodeId, u32)]>,
}

impl UnitStamps {
    /// The stamps of `unit` now, with `functions` every function its walk reached (the caller
    /// has brought the stamps up to date with `refresh` since the last change).
    pub(crate) fn of(compiler: &AbstractCompiler, unit: NodeId, functions: &[NodeId]) -> Self {
        let stamp = |n: NodeId| (n, SyntacticScopeCache::cached_subtree_stamp(compiler, n));
        Self {
            parent: unit.get_parent(compiler),
            stamps: std::iter::once(unit)
                .chain(functions.iter().copied())
                .map(stamp)
                .collect(),
        }
    }

    /// Whether nothing in `unit` changed since the stamps were taken (after `refresh`).
    pub(crate) fn is_current(&self, compiler: &AbstractCompiler, unit: NodeId) -> bool {
        self.parent == unit.get_parent(compiler)
            && self
                .stamps
                .iter()
                .all(|&(n, stamp)| SyntacticScopeCache::cached_subtree_stamp(compiler, n) == stamp)
    }

    /// Brings the change stamps up to date (records the changes made since the last call).
    pub(crate) fn refresh(compiler: &mut AbstractCompiler, any_node: NodeId) {
        SyntacticScopeCache::subtree_stamp(compiler, any_node);
    }
}

/// The records of the units of a pass's last run, by unit node.
pub(crate) struct UnitCache<T> {
    units: IndexMap<NodeId, (UnitStamps, T)>,
}

impl<T> Default for UnitCache<T> {
    fn default() -> Self {
        Self {
            units: IndexMap::default(),
        }
    }
}

impl<T> UnitCache<T> {
    /// The record of `unit` if nothing in it changed since it was taken (after `refresh`).
    pub(crate) fn get(&self, compiler: &AbstractCompiler, unit: NodeId) -> Option<&T> {
        self.units
            .get(&unit)
            .filter(|(stamps, _)| stamps.is_current(compiler, unit))
            .map(|(_, record)| record)
    }

    /// Keeps the records of `units` (in order) and drops every other.
    pub(crate) fn replace(&mut self, units: impl IntoIterator<Item = (NodeId, UnitStamps, T)>) {
        self.units = units
            .into_iter()
            .map(|(unit, stamps, record)| (unit, (stamps, record)))
            .collect();
    }

    /// Takes the record of `unit` out (whatever its stamps).
    pub(crate) fn take(&mut self, unit: NodeId) -> Option<(UnitStamps, T)> {
        self.units.swap_remove(&unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::Compiler;
    use closure_rhino::ir::IR;
    use closure_rhino::node::Ast;

    fn functions_in(ast: &Ast, n: NodeId, out: &mut Vec<NodeId>) {
        if n.is_function(ast) {
            out.push(n);
        }
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            functions_in(ast, c, out);
            child = c.get_next(ast);
        }
    }

    /// A cache with a record (the unit itself) of every statement of `script`.
    fn cache_of(compiler: &mut Compiler, script: NodeId) -> (Vec<NodeId>, UnitCache<NodeId>) {
        let units = crate::parallel_parse::statement_units(&compiler.ast, script);
        UnitStamps::refresh(compiler, script);
        let mut cache = UnitCache::default();
        cache.replace(units.iter().map(|&unit| {
            let mut functions = Vec::new();
            functions_in(&compiler.ast, unit, &mut functions);
            (unit, UnitStamps::of(compiler, unit, &functions), unit)
        }));
        (units, cache)
    }

    #[test]
    fn a_change_in_a_nested_function_makes_its_unit_stale() {
        let mut compiler = Compiler::new();
        let script = compiler
            .parse_test_code("var a = 1; function f() { function g() { return 1; } return g; }");
        let (units, cache) = cache_of(&mut compiler, script);
        UnitStamps::refresh(&mut compiler, script);
        assert_eq!(cache.get(&compiler, units[0]), Some(&units[0]));
        assert_eq!(cache.get(&compiler, units[1]), Some(&units[1]));
        // `return 1` in g becomes `return 2`: g is a change scope of its own, below f.
        let f_body = units[1].get_last_child(&compiler.ast).unwrap();
        let g = f_body.get_first_child(&compiler.ast).unwrap();
        let one = g
            .get_last_child(&compiler.ast)
            .unwrap()
            .get_first_child(&compiler.ast)
            .unwrap()
            .get_first_child(&compiler.ast)
            .unwrap();
        let two = IR::number(&mut compiler, 2.0);
        one.replace_with(&mut compiler, two);
        UnitStamps::refresh(&mut compiler, script);
        assert_eq!(cache.get(&compiler, units[0]), Some(&units[0]));
        assert_eq!(cache.get(&compiler, units[1]), None);
    }

    #[test]
    fn a_unit_moved_to_another_parent_is_stale() {
        let mut compiler = Compiler::new();
        let script = compiler.parse_test_code("var a = 1; var b = 2;");
        let (units, cache) = cache_of(&mut compiler, script);
        let block = IR::block(&mut compiler);
        units[1].detach(&mut compiler);
        block.add_child_to_back(&mut compiler, units[1]);
        script.add_child_to_back(&mut compiler, block);
        UnitStamps::refresh(&mut compiler, script);
        assert_eq!(cache.get(&compiler, units[1]), None);
    }
}
