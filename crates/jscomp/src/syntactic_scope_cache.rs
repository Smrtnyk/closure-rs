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

//! Rust-only, not in Java (DECISIONS.md D-025): syntactic scopes are reused across passes.
//!
//! Java's `SyntacticScopeCreator` scans a scope root anew every time a pass asks for its scope,
//! and the garbage collector frees the scopes after the pass; a compilation of a large bundle
//! creates hundreds of thousands of scopes this way, most of them for code no pass has changed
//! since the previous scan. Here a scope made by a `SyntacticScopeCreator` with the default
//! redeclaration handler (which has no effects) is remembered by its root node and handed out
//! again while
//!
//! - nothing in the code it was scanned from has changed: the AST records every node whose
//!   children, token or string change (`Ast::track_changes`), whether or not a pass reports the
//!   change to the compiler, and no such node lies in the root's change scope (the function or
//!   script around it, or every script for a global scope) since the scan; the root is still in
//!   the same script, of the same kind (a function or class expression or not, a function body
//!   or not), and each declared name is still that of its NAME node, which is still in the tree
//!   (a function declaration's name lies in the function's change scope, not in the declaring
//!   scope's);
//! - nobody has declared or undeclared a name in it since (`note_mutation` evicts it);
//! - it was not handed out yet in the current pass: within a pass every request makes a new
//!   scope, as in Java, so a pass never sees one scope object where Java has two;
//! - its parent is the requested parent, or a scope at the same depth (a scope of an unchanged
//!   function whose enclosing scope was scanned anew), to which it is moved.
//!
//! The scope's contents are then exactly what a new scan would produce. A scope's vars hold no
//! state that passes change (their constness, types and references are read from the AST or kept
//! by the passes themselves), so a reused scope behaves as a new one. Setting
//! `CLOSURE_RS_SCOPE_CACHE=off` makes every request scan anew; `CLOSURE_RS_SCOPE_CACHE=verify`
//! scans anew on every reuse too and panics when the two scopes differ.

use crate::{abstract_compiler::AbstractCompiler, node_util::NodeUtil, scope::ScopeId};
use closure_rhino::{fast_hash::IndexMap, node::NodeId};
use std::{
    num::NonZeroU32,
    sync::{Arc, LazyLock},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Off,
    On,
    Verify,
}

static MODE: LazyLock<Mode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_SCOPE_CACHE").as_deref() {
            Ok("off") => Mode::Off,
            Ok("verify") => Mode::Verify,
            _ => Mode::On,
        },
    );

/// Where a scope root is, which decides the scope's contents together with the code inside the
/// root's change scope.
pub(crate) struct ScopeContext {
    /// The script holding the root (the root itself for a script); None for a ROOT node.
    script: Option<NodeId>,
    /// The function or script whose change time covers the code scanned for the scope; None for
    /// a ROOT node, whose scope covers the top level of every script below it.
    change_root: Option<NodeId>,
    /// FUNCTION: a function expression; CLASS: a class expression; BLOCK: a function body or
    /// class static block.
    kind: bool,
    /// For a ROOT node: the scripts below it.
    scripts: Option<Box<[NodeId]>>,
}

impl ScopeContext {
    /// Whether the scope is a global scope rooted at a ROOT node.
    pub(crate) fn is_global_root(&self) -> bool {
        self.scripts.is_some()
    }

    /// The context of a scope rooted at `root`, or None when scopes of `root` are not kept (the
    /// cache is off, or the root is not in a script).
    pub(crate) fn of(compiler: &AbstractCompiler, root: NodeId) -> Option<Self> {
        if *MODE == Mode::Off {
            return None;
        }
        if root.is_root(compiler) {
            let mut scripts = Vec::new();
            let mut child = root.get_first_child(compiler);
            while let Some(current) = child {
                if current.is_root(compiler) {
                    let mut script = current.get_first_child(compiler);
                    while let Some(s) = script {
                        scripts.push(s);
                        script = s.get_next(compiler);
                    }
                } else {
                    scripts.push(current);
                }
                child = current.get_next(compiler);
            }
            return Some(Self {
                script: None,
                change_root: None,
                kind: false,
                scripts: Some(scripts.into_boxed_slice()),
            });
        }
        let mut change_root = None;
        let mut node = Some(root);
        while let Some(current) = node {
            if current.is_script(compiler) {
                break;
            }
            if change_root.is_none() && current.is_function(compiler) {
                change_root = Some(current);
            }
            node = current.get_parent(compiler);
        }
        let script = node?;
        let kind = if root.is_function(compiler) {
            NodeUtil::is_function_expression(compiler, root)
        } else if root.is_class(compiler) {
            NodeUtil::is_class_expression(compiler, root)
        } else if root.is_block(compiler) {
            NodeUtil::is_function_block(compiler, root)
                || NodeUtil::is_class_static_block(compiler, root)
        } else {
            false
        };
        Some(Self {
            script: Some(script),
            change_root: Some(change_root.unwrap_or(script)),
            kind,
            scripts: None,
        })
    }
}

/// The bookkeeping of a kept scope.
#[derive(Clone, Copy, Default)]
struct Entry {
    /// The `scan_round` when the scope was scanned.
    scanned_at: u32,
    /// The pass (`Compiler::current_pass_index`) that last received the scope.
    handed_out_in: i32,
    script: Option<NodeId>,
    change_root: Option<NodeId>,
    kind: bool,
    /// Whether the scope declares the name of a function declaration, which lies in the
    /// function's change scope rather than in the scope's (`ScopeId::declared_names_unchanged`).
    declares_functions: bool,
}

/// A declaration that a scan of the top level of a script makes in a global scope (see
/// `ScopeScanner::scan_script`).
pub(crate) enum ScriptDeclaration {
    /// `declareVar(scope, name)`; `parent` is the name node's parent when scanned.
    Name {
        name: NodeId,
        parent: Option<NodeId>,
    },
    /// `declareImplicitGoogNamespaceFromCall(scope, exprCall)`.
    GoogNamespace(NodeId),
}

/// The scopes kept for reuse.
#[derive(Default)]
pub(crate) struct SyntacticScopeCache {
    /// The declarations of the top level of each script that a global scope was scanned from,
    /// with the `scan_round` of that scan.
    script_declarations: IndexMap<NodeId, (u32, Arc<[ScriptDeclaration]>)>,
    /// Per node: the last round of `record_changes` that walked through it, or for a FUNCTION or
    /// SCRIPT, that found a change in its change scope.
    stamps: Vec<u32>,
    /// The number of `record_changes` rounds that found changed nodes.
    round: u32,
    /// Scratch list of changed nodes.
    changed: Vec<NodeId>,
    /// The kept scope of each root node, by node: 0 for none, else the scope's index + 1.
    by_root: Vec<u32>,
    /// The bookkeeping of each kept scope, by scope.
    entries: Vec<Entry>,
    /// The scripts below the root of each kept scope rooted at a ROOT node.
    global_scripts: IndexMap<ScopeId, Box<[NodeId]>>,
}

impl SyntacticScopeCache {
    /// Stamps the change scope (function or script) of every node the AST recorded as changed
    /// since the last call with a new round number. Every change to the tree goes through the
    /// AST, so unlike the compiler's change reports this misses none.
    fn record_changes(compiler: &mut AbstractCompiler) {
        let mut changed = std::mem::take(&mut compiler.syntactic_scope_cache.changed);
        compiler.ast.take_changed_nodes(&mut changed);
        if !changed.is_empty() {
            let cache = &mut compiler.syntactic_scope_cache;
            cache.round += 1;
            let round = cache.round;
            for &n in &changed {
                let mut node = Some(n);
                while let Some(current) = node {
                    let index = current.arena_index();
                    if cache.stamps.len() <= index {
                        cache.stamps.resize(index + 1, 0);
                    }
                    if cache.stamps[index] == round {
                        break;
                    }
                    cache.stamps[index] = round;
                    if current.is_function(&compiler.ast) || current.is_script(&compiler.ast) {
                        break;
                    }
                    node = current.get_parent(&compiler.ast);
                }
            }
            changed.clear();
        }
        compiler.syntactic_scope_cache.changed = changed;
    }

    /// Whether the change scope rooted at `change_root` changed in a round at or after `round`.
    fn changed_since(&self, change_root: NodeId, round: u32) -> bool {
        self.stamps
            .get(change_root.arena_index())
            .is_some_and(|stamp| *stamp >= round)
    }

    /// The round from which a change makes a scope scanned now stale.
    pub(crate) fn scan_round(compiler: &mut AbstractCompiler) -> u32 {
        compiler.ast.track_changes();
        Self::record_changes(compiler);
        compiler.syntactic_scope_cache.round + 1
    }

    /// A kept scope for `root` that may be handed out with parent `parent`, moved to that parent
    /// if needed.
    pub(crate) fn reuse(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        parent: Option<ScopeId>,
        context: &ScopeContext,
    ) -> Option<ScopeId> {
        Self::record_changes(compiler);
        let cache = &compiler.syntactic_scope_cache;
        let slot = *cache.by_root.get(root.arena_index())?;
        let scope = ScopeId(NonZeroU32::new(slot)?);
        let entry = cache.entries[scope.index()];
        if entry.handed_out_in == compiler.current_pass_index()
            || entry.script != context.script
            || entry.change_root != context.change_root
            || entry.kind != context.kind
        {
            return None;
        }
        let changed = match (&context.scripts, context.change_root) {
            (Some(scripts), _) => {
                cache.global_scripts.get(&scope) != Some(scripts)
                    || scripts
                        .iter()
                        .any(|script| cache.changed_since(*script, entry.scanned_at))
            }
            (None, Some(change_root)) => cache.changed_since(change_root, entry.scanned_at),
            (None, None) => true,
        };
        if changed || (entry.declares_functions && !scope.declared_names_unchanged(compiler)) {
            return None;
        }
        let current_parent = scope.get_parent(compiler);
        if current_parent != parent {
            let (Some(_), Some(parent)) = (current_parent, parent) else {
                return None;
            };
            if parent.get_depth(compiler).wrapping_add(1) != scope.get_depth(compiler) {
                return None;
            }
            scope.set_parent(compiler, parent);
        }
        compiler.syntactic_scope_cache.entries[scope.index()].handed_out_in =
            compiler.current_pass_index();
        Some(scope)
    }

    /// Keeps `scope`, just scanned from `root` in `scan_round` `scanned_at`, for reuse.
    pub(crate) fn keep(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        scope: ScopeId,
        scanned_at: u32,
        context: ScopeContext,
    ) {
        let entry = Entry {
            scanned_at,
            handed_out_in: compiler.current_pass_index(),
            script: context.script,
            change_root: context.change_root,
            kind: context.kind,
            declares_functions: scope.declares_functions(compiler),
        };
        let cache = &mut compiler.syntactic_scope_cache;
        let index = root.arena_index();
        if cache.by_root.len() <= index {
            cache.by_root.resize(index + 1, 0);
        }
        let previous = std::mem::replace(
            &mut cache.by_root[index],
            u32::try_from(scope.index() + 1).unwrap(),
        );
        if cache.entries.len() <= scope.index() {
            cache.entries.resize(scope.index() + 1, Entry::default());
        }
        cache.entries[scope.index()] = entry;
        if let Some(scripts) = context.scripts {
            cache.global_scripts.insert(scope, scripts);
        }
        if let Some(previous) = NonZeroU32::new(previous) {
            let previous = ScopeId(previous);
            cache.global_scripts.swap_remove(&previous);
            previous.set_kept(compiler, false);
        }
        scope.set_kept(compiler, true);
    }

    /// Forgets the kept `scope` (rooted at `root`), whose names were changed by a pass.
    pub(crate) fn evict(compiler: &mut AbstractCompiler, root: NodeId, scope: ScopeId) {
        let cache = &mut compiler.syntactic_scope_cache;
        let slot = &mut cache.by_root[root.arena_index()];
        if *slot as usize == scope.index() + 1 {
            *slot = 0;
        }
        cache.global_scripts.swap_remove(&scope);
        scope.set_kept(compiler, false);
    }

    /// The declarations recorded for the top level of `script`, while nothing in it changed and
    /// every declared name node is still in place with a name.
    pub(crate) fn script_declarations(
        compiler: &mut AbstractCompiler,
        script: NodeId,
    ) -> Option<Arc<[ScriptDeclaration]>> {
        Self::record_changes(compiler);
        let cache = &compiler.syntactic_scope_cache;
        let (scanned_at, declarations) = cache.script_declarations.get(&script)?;
        if cache.changed_since(script, *scanned_at) {
            return None;
        }
        // A function declaration's name node lies in the function's change scope.
        let in_place = declarations.iter().all(|declaration| match *declaration {
            ScriptDeclaration::Name { name, parent } => {
                name.get_parent(compiler) == parent
                    && (!name.is_name(compiler) || !name.get_string_ref(compiler).is_empty())
            }
            ScriptDeclaration::GoogNamespace(_) => true,
        });
        in_place.then(|| Arc::clone(declarations))
    }

    /// Keeps the declarations of the top level of `script`, scanned in `scan_round` `scanned_at`.
    pub(crate) fn keep_script_declarations(
        compiler: &mut AbstractCompiler,
        script: NodeId,
        scanned_at: u32,
        declarations: Vec<ScriptDeclaration>,
    ) {
        compiler
            .syntactic_scope_cache
            .script_declarations
            .insert(script, (scanned_at, declarations.into()));
    }

    /// Whether reused scopes are checked against a new scan.
    pub(crate) fn verifying() -> bool {
        *MODE == Mode::Verify
    }

    /// Panics unless `reused` declares the same vars, in the same order, as the new scan `fresh`.
    pub(crate) fn verify(compiler: &AbstractCompiler, reused: ScopeId, fresh: ScopeId) {
        let describe = |scope: ScopeId| -> Vec<String> {
            scope
                .get_var_iterable(compiler)
                .into_iter()
                .map(|var| {
                    format!(
                        "{}@{:?}#{} in {:?} goog:{}",
                        var.get_name(compiler),
                        var.get_name_node(compiler),
                        var.get_index(compiler),
                        var.get_input(compiler)
                            .map(|input| input.get_name().to_owned()),
                        var.is_implicit_goog_namespace(compiler)
                    )
                })
                .collect()
        };
        let (a, b) = (describe(reused), describe(fresh));
        assert!(
            a == b,
            "syntactic scope cache: reused scope of {} differs from a new scan:\nreused {a:?}\nnew    {b:?}",
            reused.get_root_node(compiler).to_string(compiler)
        );
    }
}
