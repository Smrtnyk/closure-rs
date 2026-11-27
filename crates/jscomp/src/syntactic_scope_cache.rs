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

//! Rust-only, not in Java (crates/DESIGN.md, "Performance divergences"): syntactic scopes are
//! reused across passes.
//!
//! Java's `SyntacticScopeCreator` scans a scope root anew every time a pass asks for its scope,
//! and the garbage collector frees the scopes after the pass; a compilation of a large bundle
//! creates hundreds of thousands of scopes this way, most of them for code no pass has changed
//! since the previous scan. Here a scope made by a `SyntacticScopeCreator` with the default
//! redeclaration handler (which has no effects), or with another handler that the scan did not
//! call, is remembered by its root node and handed out again (to a creator with another handler
//! only when its scan called no handler, so that the handler would not have been called) while
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
//! - it had a parent and one is requested (a scope of an unchanged function whose enclosing scope
//!   was scanned anew, or that lies at another depth now), or neither; it is moved to the
//!   requested parent, at the depth below it. A kept scope is handed out only through `reuse`,
//!   after its parent, so no pass sees a depth from before such a move.
//!
//! In a function's change scope, a change makes only the scopes stale whose scan could see it
//! (`stamp_declaration_change`): a scan reads the statement lists of blocks and control
//! structures, declarations with their binding patterns, parameter lists, and the names of
//! functions and classes, never what lies inside an expression. In a script's change scope every
//! change counts.
//!
//! The scope's contents are then exactly what a new scan would produce. A scope's vars hold no
//! state that passes change (their constness, types and references are read from the AST or kept
//! by the passes themselves), so a reused scope behaves as a new one. Setting
//! `CLOSURE_RS_SCOPE_CACHE=off` makes every request scan anew; `CLOSURE_RS_SCOPE_CACHE=verify`
//! scans anew on every reuse too and panics when the two scopes differ.
//! `CLOSURE_RS_SCOPE_DECLARATION_STAMPS=off` makes every change in a function's change scope count
//! and moves a scope only to a parent at the depth of its old one;
//! `CLOSURE_RS_SCOPE_REDECLARATION_FREE_REUSE=off` keeps creators with a handler of their own out
//! of the cache.

use crate::{abstract_compiler::AbstractCompiler, node_util::NodeUtil, scope::ScopeId, var::VarId};
use closure_rhino::{
    fast_hash::{IndexMap, IndexSet},
    js_string::JsString,
    node::{Ast, AstChangeCursor, NodeId},
    token::Token,
};
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

/// Whether creators with a redeclaration handler of their own reuse and keep scopes whose scan
/// called no handler (`SyntacticScopeCreator::default_handler`).
pub(crate) static REDECLARATION_FREE_REUSE: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("CLOSURE_RS_SCOPE_REDECLARATION_FREE_REUSE").as_deref() != Ok("off")
});

/// Whether only the changes a scan can see make the scopes of a function stale (see
/// `stamp_declaration_change`), and a scope may move to a parent at another depth.
static DECLARATION_STAMPS: LazyLock<bool> =
    LazyLock::new(|| std::env::var("CLOSURE_RS_SCOPE_DECLARATION_STAMPS").as_deref() != Ok("off"));

/// Rust-only (performance divergence): calls `stamp` with every scope root whose scan can see the
/// change recorded at `n` (to its children, token or string), so that only the scopes of these
/// roots become stale.
///
/// A scan (`ScopeScanner`) starts at a scope root and reads the children of statement blocks and
/// control structures, declarations (`var`, `let`, `const`, `import`, `export`, `catch`) with the
/// targets of their binding patterns, parameter lists, and the first child (the name) of functions
/// and classes; of other nodes it reads only the token, and the string of a name only where it is
/// declared. A change at `n` is seen when the scan reads `n`'s children or string and reaches `n`
/// from its root through nodes it reads;
/// every scope root on that path is stamped, which also covers the hoisting of `var` into a
/// function body from nested blocks. A token change is recorded at the parent too, so it needs no
/// reading of `n`'s token. When an edit moves `n` out of what a scan reads, the edit is recorded at
/// a node on the old path, and the topmost such node is still reached from the root. A function's
/// body reads the function's parameters (`ScopeScanner::is_shadowing_allowed`), so a stamp of a
/// function stamps its body. A function or class declaration's name is read by the enclosing scope
/// too, which is checked on reuse instead (`ScopeId::declared_names_unchanged`) for a function,
/// whose name is in its own change scope.
fn stamp_declaration_change(ast: &Ast, n: NodeId, mut stamp: impl FnMut(NodeId)) {
    fn is_scope_root_kind(token: Token) -> bool {
        matches!(
            token,
            Token::FUNCTION
                | Token::CLASS
                | Token::BLOCK
                | Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::SWITCH_BODY
                | Token::MODULE_BODY
                | Token::SCRIPT
                | Token::ROOT
                | Token::MEMBER_FIELD_DEF
                | Token::COMPUTED_FIELD_DEF
        )
    }
    // Statement blocks, control structures and switch bodies
    // (`NodeUtil::is_shallow_statement_tree`): the scan reads the token of each child.
    fn is_statement_list(token: Token) -> bool {
        matches!(
            token,
            Token::ROOT
                | Token::SCRIPT
                | Token::BLOCK
                | Token::MODULE_BODY
                | Token::SWITCH_BODY
                | Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::DO
                | Token::WHILE
                | Token::WITH
                | Token::IF
                | Token::LABEL
                | Token::TRY
                | Token::CATCH
                | Token::SWITCH
                | Token::CASE
                | Token::DEFAULT_CASE
        )
    }
    // Declarations and binding patterns (`NodeUtil::visit_lhs_nodes_in_node`): the scan reads each
    // child as a target.
    fn is_declaration_list(token: Token) -> bool {
        matches!(
            token,
            Token::VAR
                | Token::LET
                | Token::CONST
                | Token::IMPORT
                | Token::IMPORT_SPECS
                | Token::EXPORT
                | Token::ARRAY_PATTERN
                | Token::OBJECT_PATTERN
                | Token::PARAM_LIST
        )
    }
    fn reads_all_children(token: Token) -> bool {
        is_statement_list(token) || is_declaration_list(token)
    }
    // Whether a scan that reads `parent` reads its child `child`.
    fn reads_child(ast: &Ast, parent: NodeId, child: NodeId) -> bool {
        let token = parent.get_token(ast);
        if reads_all_children(token) {
            return true;
        }
        match token {
            Token::DESTRUCTURING_LHS
            | Token::DEFAULT_VALUE
            | Token::ITER_REST
            | Token::OBJECT_REST
            | Token::CAST
            | Token::CLASS => parent.get_first_child(ast) == Some(child),
            Token::STRING_KEY | Token::COMPUTED_PROP | Token::IMPORT_SPEC => {
                parent.get_last_child(ast) == Some(child)
            }
            // The name and the parameters; the body is a scope root of its own.
            Token::FUNCTION => parent.get_last_child(ast) != Some(child),
            _ => false,
        }
    }
    let token = n.get_token(ast);
    let reads_into_n = is_scope_root_kind(token)
        || reads_all_children(token)
        || matches!(
            token,
            Token::NAME
                | Token::IMPORT_STAR
                | Token::DESTRUCTURING_LHS
                | Token::DEFAULT_VALUE
                | Token::ITER_REST
                | Token::OBJECT_REST
                | Token::CAST
                | Token::STRING_KEY
                | Token::COMPUTED_PROP
                | Token::IMPORT_SPEC
        );
    if !reads_into_n {
        return;
    }
    if matches!(token, Token::NAME | Token::IMPORT_STAR)
        && let Some(parent) = n.get_parent(ast)
    {
        // A name directly in a statement (an `if` condition, a `for-in` target) declares nothing;
        // a `catch` declares its first child.
        let parent_token = parent.get_token(ast);
        if is_statement_list(parent_token)
            && (parent_token != Token::CATCH || parent.get_first_child(ast) != Some(n))
        {
            return;
        }
    }
    let mut current = n;
    loop {
        let token = current.get_token(ast);
        if is_scope_root_kind(token) {
            stamp(current);
            if token == Token::FUNCTION {
                if let Some(body) = current.get_last_child(ast) {
                    stamp(body);
                }
                // Above lies another change scope.
                return;
            }
        }
        let Some(parent) = current.get_parent(ast) else {
            return;
        };
        if !reads_child(ast, parent, current) {
            return;
        }
        current = parent;
    }
}

/// Where a scope root is, which decides the scope's contents together with the code inside the
/// root's change scope.
pub(crate) struct ScopeContext {
    /// The script holding the root (the root itself for a script); None for a ROOT node.
    script: Option<NodeId>,
    /// The function or script whose change scope holds the code scanned for the scope; None for
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
    pub(crate) fn of(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        parent: Option<ScopeId>,
    ) -> Option<Self> {
        if *MODE == Mode::Off {
            return None;
        }
        compiler.ast.track_changes();
        SyntacticScopeCache::record_changes(compiler);
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
        // Rust-only (performance divergence): when the parent scope's context was verified in
        // this exact AST change round, stop walking parents once we reach the parent scope root.
        let parent_hint = parent.and_then(|parent_scope| {
            let cache = &compiler.syntactic_scope_cache;
            let current_tag = cache.round.wrapping_add(1);
            let entry = cache.entries.get(parent_scope.index())?;
            if entry.context_tag == current_tag
                && let (Some(script), Some(change_root)) = (entry.script, entry.change_root)
            {
                Some((parent_scope.get_root_node(compiler), script, change_root))
            } else {
                None
            }
        });
        let mut change_root = None;
        let mut script = None;
        let mut node = Some(root);
        while let Some(current) = node {
            if current.is_script(compiler) {
                script = Some(current);
                break;
            }
            if let Some((parent_root, parent_script, parent_change_root)) = parent_hint
                && current == parent_root
            {
                script = Some(parent_script);
                if change_root.is_none() {
                    change_root = Some(parent_change_root);
                }
                break;
            }
            if change_root.is_none() && current.is_function(compiler) {
                change_root = Some(current);
            }
            node = current.get_parent(compiler);
        }
        let script = script?;
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
    /// The pass, or round of a pass (`Compiler::scope_handout_round`), that last received the scope.
    handed_out_in: i32,
    /// `cache.round.wrapping_add(1)` when `script` and `change_root` were last verified; 0 = none.
    context_tag: u32,
    script: Option<NodeId>,
    change_root: Option<NodeId>,
    kind: bool,
    /// Whether the scope declares the name of a function declaration, which lies in the
    /// function's change scope rather than in the scope's (`ScopeId::declared_names_unchanged`).
    declares_functions: bool,
    /// Whether the scan called the redeclaration handler.
    redeclared: bool,
}

/// A declaration that a scan of the top level of a script makes in a global scope (see
/// `ScopeScanner::scan_script`).
#[derive(PartialEq, Eq)]
pub(crate) enum ScriptDeclaration {
    /// `declareVar(scope, name)`; `parent` is the name node's parent when scanned.
    Name {
        name: NodeId,
        parent: Option<NodeId>,
    },
    /// `declareImplicitGoogNamespaceFromCall(scope, exprCall)`.
    GoogNamespace(NodeId),
}

/// Rust-only (performance divergence): the change stamps of a function and of every function
/// nested in it (`SyntacticScopeCache::deep_function_stamp`). A nested function is a change scope
/// of its own, so the stamp of the outer function alone does not advance when it changes.
pub(crate) struct DeepFunctionStamp {
    own: (u32, u32),
    nested: Box<[(NodeId, (u32, u32))]>,
}

/// The most changed functions `SyntacticScopeCache` lists between two `take_changed_functions`.
const MAX_CHANGED_FUNCTIONS: usize = 1 << 18;

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
    /// Rust-only (performance divergence): per scope root, the last round of `record_changes` that
    /// found a change a scan of it can see (`stamp_declaration_change`).
    declaration_stamps: Vec<u32>,
    /// Rust-only (performance divergence): per node, the last round of `record_changes` that found
    /// an in-place change to `JSDOC_INFO`, `SIDE_EFFECT_FLAGS`, `IS_CONSTANT_NAME`, or
    /// `CONSTANT_VAR_FLAGS` in its change scope.
    flag_stamps: Vec<u32>,
    /// The number of `record_changes` rounds that found flag-changed nodes.
    flag_round: u32,
    cursor: AstChangeCursor,
    /// Rust-only (performance divergence): the functions whose change scope a `record_changes`
    /// round stamped since `take_changed_functions` last ran, once per round; `None` when the list
    /// is not complete (never taken yet, stamps reset, or too long to keep).
    changed_functions: Option<Vec<NodeId>>,
    /// Scratch list of changed nodes.
    changed: Vec<NodeId>,
    /// Scratch list of flag-changed nodes.
    changed_flags: Vec<NodeId>,
    /// The kept scope of each root node, by node: 0 for none, else the scope's index + 1.
    by_root: Vec<u32>,
    /// The bookkeeping of each kept scope, by scope.
    entries: Vec<Entry>,
    /// The scripts below the root of each kept scope rooted at a ROOT node.
    global_scripts: IndexMap<ScopeId, Box<[NodeId]>>,
    /// Rust-only (performance divergence): for each kept scope rooted at a ROOT node, the
    /// declarations made for each of its scripts (`None` when not recorded), in script order.
    global_declarations: IndexMap<ScopeId, GlobalDeclarations>,
}

/// The declarations made for the scripts of a global scope, in script order.
type GlobalDeclarations = Box<[Option<Arc<[ScriptDeclaration]>>]>;

/// Makes the declarations a scan of a global scope makes for the top level of a script, without
/// making them (`ScopeScanner::script_declarations_now`).
pub(crate) type DeclarationsNow<'a> =
    &'a mut dyn FnMut(&mut AbstractCompiler, ScopeId, NodeId) -> Option<Vec<ScriptDeclaration>>;

impl SyntacticScopeCache {
    /// Stamps the change scope (function or script) of every node the AST recorded as changed
    /// since the last call with a new round number. Every change to the tree goes through the
    /// AST, so unlike the compiler's change reports this misses none.
    fn record_changes(compiler: &mut AbstractCompiler) {
        // Rust-only (performance divergence): most calls find nothing new to record.
        if compiler
            .ast
            .no_changes_since(&compiler.syntactic_scope_cache.cursor)
        {
            return;
        }
        let mut changed = std::mem::take(&mut compiler.syntactic_scope_cache.changed);
        let mut changed_flags = std::mem::take(&mut compiler.syntactic_scope_cache.changed_flags);
        let continuous = compiler
            .ast
            .take_changes_since(&mut compiler.syntactic_scope_cache.cursor, &mut changed);
        compiler.ast.take_changed_flag_nodes(&mut changed_flags);
        if !continuous {
            // Public callers can move whole arenas or drain the log between compiler passes.
            // In either case its old node IDs and stamps no longer identify reusable analyses.
            let old = std::mem::take(&mut compiler.syntactic_scope_cache.by_root);
            for scope in old.into_iter().filter_map(NonZeroU32::new) {
                ScopeId(scope).set_kept(compiler, false);
            }
            let cache = &mut compiler.syntactic_scope_cache;
            cache.entries.clear();
            cache.global_scripts.clear();
            cache.global_declarations.clear();
            cache.script_declarations.clear();
            cache.stamps.clear();
            cache.declaration_stamps.clear();
            cache.flag_stamps.clear();
            cache.changed_functions = None;
            changed_flags.clear();
            compiler.function_reference_sites = Default::default();
            compiler.remove_unused_code_cache = Default::default();
            compiler.flow_inline_clean_cache = Default::default();
            compiler.inline_vars_clean_cache = Default::default();
            compiler.inline_object_literals_clean_cache = Default::default();
            compiler.optimize_calls_unit_cache = Default::default();
            compiler.inline_functions_facts = Default::default();
            compiler.inline_simple_methods_cache = Default::default();
            compiler.dae_unit_cache = Default::default();
            compiler.fsiv_unit_cache = Default::default();
        }
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
                    if current.is_function(&compiler.ast) {
                        if let Some(functions) = &mut cache.changed_functions {
                            functions.push(current);
                        }
                        break;
                    }
                    if current.is_script(&compiler.ast) {
                        break;
                    }
                    node = current.get_parent(&compiler.ast);
                }
            }
            if *DECLARATION_STAMPS {
                let stamps = &mut cache.declaration_stamps;
                for &n in &changed {
                    stamp_declaration_change(&compiler.ast, n, |root| {
                        let index = root.arena_index();
                        if stamps.len() <= index {
                            stamps.resize(index + 1, 0);
                        }
                        stamps[index] = round;
                    });
                }
            }
            if cache
                .changed_functions
                .as_ref()
                .is_some_and(|functions| functions.len() > MAX_CHANGED_FUNCTIONS)
            {
                cache.changed_functions = None;
            }
            changed.clear();
        }
        if !changed_flags.is_empty() {
            let cache = &mut compiler.syntactic_scope_cache;
            cache.flag_round += 1;
            let round = cache.flag_round;
            for &n in &changed_flags {
                let mut node = Some(n);
                while let Some(current) = node {
                    let index = current.arena_index();
                    if cache.flag_stamps.len() <= index {
                        cache.flag_stamps.resize(index + 1, 0);
                    }
                    if cache.flag_stamps[index] == round {
                        break;
                    }
                    cache.flag_stamps[index] = round;
                    if current.is_function(&compiler.ast) || current.is_script(&compiler.ast) {
                        break;
                    }
                    node = current.get_parent(&compiler.ast);
                }
            }
            changed_flags.clear();
        }
        compiler.syntactic_scope_cache.changed = changed;
        compiler.syntactic_scope_cache.changed_flags = changed_flags;
    }

    /// Whether the change scope rooted at `change_root` changed in a round at or after `round`.
    fn changed_since(&self, change_root: NodeId, round: u32) -> bool {
        self.stamps
            .get(change_root.arena_index())
            .is_some_and(|stamp| *stamp >= round)
    }

    /// Whether the scope rooted at `root` in the change scope rooted at `change_root` may have
    /// changed in a round at or after `round`: in a function, by a change a scan of `root` can see.
    fn scope_changed_since(
        &self,
        ast: &Ast,
        root: NodeId,
        change_root: NodeId,
        round: u32,
    ) -> bool {
        if !*DECLARATION_STAMPS || !change_root.is_function(ast) {
            return self.changed_since(change_root, round);
        }
        self.declaration_stamps
            .get(root.arena_index())
            .is_some_and(|stamp| *stamp >= round)
    }

    /// The last round that changed this node or a descendant in its change scope. A caller
    /// relying on the whole subtree must exclude nested functions and scripts (each has its own
    /// change scope). As for syntactic scopes, this observes edits even during a running pass.
    pub(crate) fn subtree_stamp(compiler: &mut AbstractCompiler, root: NodeId) -> u32 {
        compiler.ast.track_changes();
        Self::record_changes(compiler);
        Self::cached_subtree_stamp(compiler, root)
    }

    /// Reads the recorded stamp of `root` after `subtree_stamp` has already drained pending
    /// changes.
    #[inline]
    pub(crate) fn cached_subtree_stamp(compiler: &AbstractCompiler, root: NodeId) -> u32 {
        compiler
            .syntactic_scope_cache
            .stamps
            .get(root.arena_index())
            .copied()
            .unwrap_or(0)
    }

    /// Rust-only (performance divergence): `(structural_stamp, flag_stamp)` for `root`'s change
    /// scope, advancing on either structural edits or in-place `JSDOC_INFO`, `SIDE_EFFECT_FLAGS`,
    /// `IS_CONSTANT_NAME`, or `CONSTANT_VAR_FLAGS` updates inside `root`.
    pub(crate) fn subtree_stamp_with_flags(
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> (u32, u32) {
        compiler.ast.track_changes();
        Self::record_changes(compiler);
        let cache = &compiler.syntactic_scope_cache;
        let idx = root.arena_index();
        (
            cache.stamps.get(idx).copied().unwrap_or(0),
            cache.flag_stamps.get(idx).copied().unwrap_or(0),
        )
    }

    /// Rust-only (performance divergence): the `subtree_stamp_with_flags` of `fn_node` and of
    /// every function nested in it, for a caller relying on the function's whole subtree.
    pub(crate) fn deep_function_stamp(
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
    ) -> DeepFunctionStamp {
        let own = Self::subtree_stamp_with_flags(compiler, fn_node);
        let ast = &compiler.ast;
        let cache = &compiler.syntactic_scope_cache;
        let mut nested = Vec::new();
        let mut pending = Vec::new();
        let mut child = fn_node.get_first_child(ast);
        while let Some(c) = child {
            pending.push(c);
            child = c.get_next(ast);
        }
        while let Some(n) = pending.pop() {
            if n.is_function(ast) {
                nested.push((n, cache.recorded_stamp_with_flags(n)));
            }
            let mut child = n.get_first_child(ast);
            while let Some(c) = child {
                pending.push(c);
                child = c.get_next(ast);
            }
        }
        DeepFunctionStamp {
            own,
            nested: nested.into_boxed_slice(),
        }
    }

    /// Rust-only (performance divergence): whether neither the function `stamp` was taken of nor
    /// any function nested in it at that time has changed since; `own` is the function's
    /// `subtree_stamp_with_flags` now. A function added to the subtree later changes the change
    /// scope it is added to, which is one of these.
    pub(crate) fn deep_function_stamp_is_current(
        &self,
        own: (u32, u32),
        stamp: &DeepFunctionStamp,
    ) -> bool {
        own == stamp.own
            && stamp
                .nested
                .iter()
                .all(|&(nested_fn, recorded)| self.recorded_stamp_with_flags(nested_fn) == recorded)
    }

    /// Reads what `subtree_stamp_with_flags` returns for `root` after a call of it has already
    /// drained pending changes.
    #[inline]
    pub(crate) fn cached_subtree_stamp_with_flags(
        compiler: &AbstractCompiler,
        root: NodeId,
    ) -> (u32, u32) {
        compiler
            .syntactic_scope_cache
            .recorded_stamp_with_flags(root)
    }

    /// The `(structural_stamp, flag_stamp)` of `root` as of the last `record_changes`.
    #[inline]
    fn recorded_stamp_with_flags(&self, root: NodeId) -> (u32, u32) {
        let idx = root.arena_index();
        (
            self.stamps.get(idx).copied().unwrap_or(0),
            self.flag_stamps.get(idx).copied().unwrap_or(0),
        )
    }

    /// Rust-only (performance divergence): the functions whose change scope changed since the last
    /// call, or `None` when that list is not complete and the caller has to look at every function.
    /// Either way the list starts anew. A function may be listed more than once, and one that was
    /// created since may be missing: the list serves to refresh caches early, never to trust them.
    pub(crate) fn take_changed_functions(compiler: &mut AbstractCompiler) -> Option<Vec<NodeId>> {
        compiler.ast.track_changes();
        Self::record_changes(compiler);
        compiler
            .syntactic_scope_cache
            .changed_functions
            .replace(Vec::new())
    }

    /// The round from which a change makes a scope scanned now stale.
    pub(crate) fn scan_round(compiler: &mut AbstractCompiler) -> u32 {
        compiler.ast.track_changes();
        Self::record_changes(compiler);
        compiler.syntactic_scope_cache.round + 1
    }

    /// A kept scope for `root` that may be handed out with parent `parent`, moved to that parent
    /// if needed; when `without_redeclarations`, only one whose scan called no redeclaration
    /// handler.
    pub(crate) fn reuse(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        parent: Option<ScopeId>,
        context: &ScopeContext,
        without_redeclarations: bool,
    ) -> Option<ScopeId> {
        Self::record_changes(compiler);
        let cache = &compiler.syntactic_scope_cache;
        let slot = *cache.by_root.get(root.arena_index())?;
        let scope = ScopeId(NonZeroU32::new(slot)?);
        let entry = cache.entries[scope.index()];
        if (without_redeclarations && entry.redeclared)
            || entry.handed_out_in == compiler.scope_handout_round()
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
            (None, Some(change_root)) => {
                cache.scope_changed_since(&compiler.ast, root, change_root, entry.scanned_at)
            }
            (None, None) => true,
        };
        if changed || (entry.declares_functions && !scope.declared_names_unchanged(compiler)) {
            return None;
        }
        let current_parent = scope.get_parent(compiler);
        match (current_parent, parent) {
            (None, None) => {}
            (Some(current_parent), Some(parent)) => {
                if !*DECLARATION_STAMPS
                    && parent.get_depth(compiler).wrapping_add(1) != scope.get_depth(compiler)
                {
                    return None;
                }
                if current_parent != parent {
                    scope.set_parent(compiler, parent);
                } else if parent.get_depth(compiler).wrapping_add(1) != scope.get_depth(compiler) {
                    // The parent was moved to another depth since this scope was kept (a module
                    // body that was rewritten into its script took a scope out of the chain).
                    scope.set_depth_below_parent(compiler);
                }
            }
            _ => return None,
        }
        let pass_index = compiler.scope_handout_round();
        let current_tag = compiler.syntactic_scope_cache.round.wrapping_add(1);
        let entry = &mut compiler.syntactic_scope_cache.entries[scope.index()];
        entry.handed_out_in = pass_index;
        entry.context_tag = current_tag;
        Some(scope)
    }

    /// Keeps `scope`, just scanned from `root` in `scan_round` `scanned_at`, for reuse;
    /// `redeclared`: whether the scan called the redeclaration handler.
    pub(crate) fn keep(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        scope: ScopeId,
        scanned_at: u32,
        context: ScopeContext,
        redeclared: bool,
        script_lists: Vec<Option<Arc<[ScriptDeclaration]>>>,
    ) {
        let context_tag = compiler.syntactic_scope_cache.round.wrapping_add(1);
        let entry = Entry {
            scanned_at,
            handed_out_in: compiler.scope_handout_round(),
            context_tag,
            script: context.script,
            change_root: context.change_root,
            kind: context.kind,
            declares_functions: scope.declares_functions(compiler),
            redeclared,
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
            // One list per script, unless a script was not scanned through `scan_script`.
            if script_lists.len() == scripts.len() {
                cache
                    .global_declarations
                    .insert(scope, script_lists.into_boxed_slice());
            }
            cache.global_scripts.insert(scope, scripts);
        }
        if let Some(previous) = NonZeroU32::new(previous) {
            let previous = ScopeId(previous);
            cache.global_scripts.swap_remove(&previous);
            cache.global_declarations.swap_remove(&previous);
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
        if let Some(entry) = cache.entries.get_mut(scope.index()) {
            entry.context_tag = 0;
        }
        cache.global_scripts.swap_remove(&scope);
        cache.global_declarations.swap_remove(&scope);
        scope.set_kept(compiler, false);
    }

    /// Rust-only (performance divergence): the kept global scope of `root`, updated, when the
    /// scripts that changed since it was scanned make the declarations they made then less
    /// some. A scan of the global scope makes the declarations of the top level of each script
    /// in script order (`ScopeScanner::scan_script`): the first declaration of a name makes its
    /// var, the others change nothing. When every changed script declares a subsequence of the
    /// name nodes it declared at the scan, the vars of the declarations left are those of a new
    /// scan, in the same order, except the vars made by the declarations that are gone, which
    /// are removed (`ScopeId::remove_vars_as_rescanned` numbers the vars as a scan would), as
    /// long as no declaration left has the name of one of them and every declared name node left
    /// still has its var's name and is in the tree. `declarations_now` makes the declarations of
    /// a script without making them (`None` when one would go to another scope). A script with a
    /// Closure namespace, whose name is not a NAME, is scanned anew.
    /// `CLOSURE_RS_GLOBAL_SCOPE_UPDATE=off` scans the global scope anew whenever a script changed.
    pub(crate) fn reuse_global_after_removals(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        context: &ScopeContext,
        without_redeclarations: bool,
        declarations_now: DeclarationsNow<'_>,
    ) -> Option<ScopeId> {
        static ENABLED: LazyLock<bool> = LazyLock::new(|| {
            std::env::var("CLOSURE_RS_GLOBAL_SCOPE_UPDATE").as_deref() != Ok("off")
        });
        let scripts = context.scripts.as_ref()?;
        if !*ENABLED {
            return None;
        }
        Self::record_changes(compiler);
        let cache = &compiler.syntactic_scope_cache;
        let slot = *cache.by_root.get(root.arena_index())?;
        let scope = ScopeId(NonZeroU32::new(slot)?);
        let entry = cache.entries[scope.index()];
        if entry.handed_out_in == compiler.scope_handout_round()
            || entry.script.is_some()
            || entry.change_root.is_some()
            || cache.global_scripts.get(&scope) != Some(scripts)
            || scope.get_parent(compiler).is_some()
        {
            return None;
        }
        let lists = cache.global_declarations.get(&scope)?.clone();
        let changed: Vec<usize> = (0..scripts.len())
            .filter(|&i| cache.changed_since(scripts[i], entry.scanned_at))
            .collect();
        let names = |list: &[ScriptDeclaration]| -> Option<Vec<NodeId>> {
            list.iter()
                .map(|declaration| match *declaration {
                    ScriptDeclaration::Name { name, .. } => Some(name),
                    ScriptDeclaration::GoogNamespace(_) => None,
                })
                .collect()
        };
        let mut renewed = Vec::with_capacity(changed.len());
        let mut gone: Vec<NodeId> = Vec::new();
        for i in changed {
            let kept = names(lists[i].as_deref()?)?;
            let now = declarations_now(compiler, scope, scripts[i])?;
            let now_names = names(&now)?;
            // `now_names` must be a subsequence of `kept`.
            let mut k = 0;
            for &name in &now_names {
                while k < kept.len() && kept[k] != name {
                    gone.push(kept[k]);
                    k += 1;
                }
                if k == kept.len() {
                    return None;
                }
                k += 1;
            }
            gone.extend_from_slice(&kept[k..]);
            renewed.push((i, now));
        }
        // The vars made by declarations that are gone, by name, and whether a declaration that is
        // gone found its name declared (it called the redeclaration handler).
        let mut removed_names: IndexMap<JsString, VarId> = IndexMap::default();
        let mut gone_redeclaration = false;
        for &node in &gone {
            let name = node.get_string(compiler);
            let var = scope.get_own_slot(compiler, &name)?;
            if var.get_name_node(compiler) == Some(node) {
                removed_names.insert(name, var);
            } else {
                gone_redeclaration = true;
            }
        }
        // Whether a scan now calls the redeclaration handler: a later declaration of a name finds
        // it declared (the global scope allows shadowing, and the declarations hold no Closure
        // namespace).
        let mut redeclared = entry.redeclared;
        if !removed_names.is_empty() || (redeclared && gone_redeclaration) {
            let renewed_at: IndexMap<usize, &Vec<ScriptDeclaration>> =
                renewed.iter().map(|(i, now)| (*i, now)).collect();
            let mut declared: IndexMap<&JsString, NodeId> = IndexMap::default();
            redeclared = false;
            for (i, list) in lists.iter().enumerate() {
                let declarations: &[ScriptDeclaration] = match renewed_at.get(&i) {
                    Some(now) => now,
                    None => list.as_deref()?,
                };
                for declaration in declarations {
                    let ScriptDeclaration::Name { name, .. } = *declaration else {
                        return None;
                    };
                    let spelling = name.get_string_ref(compiler);
                    if removed_names.contains_key(spelling) {
                        return None;
                    }
                    if let Some(&first) = declared.get(spelling) {
                        redeclared |= first != name;
                    } else {
                        declared.insert(spelling, name);
                    }
                }
            }
        }
        if without_redeclarations && redeclared {
            return None;
        }
        let removed: IndexSet<VarId> = removed_names.into_values().collect();
        // Every declared name left must still be the name of its node.
        if !scope.declared_names_unchanged_except(compiler, &removed) {
            return None;
        }
        if !removed.is_empty() {
            scope.remove_vars_as_rescanned(compiler, &removed);
        }
        let scanned_at = Self::scan_round(compiler);
        let pass_index = compiler.scope_handout_round();
        let cache = &mut compiler.syntactic_scope_cache;
        let current_tag = cache.round.wrapping_add(1);
        let mut lists = lists.into_vec();
        for (i, now) in renewed {
            let now: Arc<[ScriptDeclaration]> = now.into();
            cache
                .script_declarations
                .insert(scripts[i], (scanned_at, Arc::clone(&now)));
            lists[i] = Some(now);
        }
        cache
            .global_declarations
            .insert(scope, lists.into_boxed_slice());
        let entry = &mut cache.entries[scope.index()];
        entry.scanned_at = scanned_at;
        entry.handed_out_in = pass_index;
        entry.context_tag = current_tag;
        entry.redeclared = redeclared;
        Some(scope)
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
        declarations: Arc<[ScriptDeclaration]>,
    ) {
        compiler
            .syntactic_scope_cache
            .script_declarations
            .insert(script, (scanned_at, declarations));
    }

    /// Whether reused scopes are checked against a new scan.
    pub(crate) fn verifying() -> bool {
        *MODE == Mode::Verify
    }

    /// Panics unless `reused` declares the same vars, in the same order, as the new scan `fresh`,
    /// at the same depth, and its scan called a redeclaration handler as `fresh_redeclared` says.
    pub(crate) fn verify(
        compiler: &AbstractCompiler,
        reused: ScopeId,
        fresh: ScopeId,
        fresh_redeclared: bool,
    ) {
        if let Some(entry) = compiler.syntactic_scope_cache.entries.get(reused.index()) {
            assert!(
                entry.redeclared == fresh_redeclared,
                "syntactic scope cache: reused scope of {} redeclared {}, a new scan {}",
                reused.get_root_node(compiler).to_string(compiler),
                entry.redeclared,
                fresh_redeclared
            );
        }
        assert!(
            reused.get_depth(compiler) == fresh.get_depth(compiler),
            "syntactic scope cache: reused scope of {} at depth {}, a new scan at depth {}",
            reused.get_root_node(compiler).to_string(compiler),
            reused.get_depth(compiler),
            fresh.get_depth(compiler)
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stamped(ast: &Ast, n: NodeId) -> Vec<NodeId> {
        let mut roots = Vec::new();
        stamp_declaration_change(ast, n, |root| roots.push(root));
        roots
    }

    /// `function f(p = d) { var x = init(); g(); if (c) { let y; } class C { m() {} } }`
    #[test]
    fn declaration_changes_stamp_the_scope_roots_that_read_them() {
        let mut ast = Ast::new();
        let name = |ast: &mut Ast, s: &str| ast.new_string_with_token(Token::NAME, s);
        let f = name(&mut ast, "f");
        let p = name(&mut ast, "p");
        let d = name(&mut ast, "d");
        let default_value = ast.new_node_with_children2(Token::DEFAULT_VALUE, p, d);
        let params = ast.new_node_with_child(Token::PARAM_LIST, default_value);
        let init_callee = name(&mut ast, "init");
        let init = ast.new_node_with_child(Token::CALL, init_callee);
        let x = name(&mut ast, "x");
        x.add_child_to_back(&mut ast, init);
        let var = ast.new_node_with_child(Token::VAR, x);
        let g = name(&mut ast, "g");
        let call = ast.new_node_with_child(Token::CALL, g);
        let expr = ast.new_node_with_child(Token::EXPR_RESULT, call);
        let c = name(&mut ast, "c");
        let y = name(&mut ast, "y");
        let let_node = ast.new_node_with_child(Token::LET, y);
        let inner_block = ast.new_node_with_child(Token::BLOCK, let_node);
        let if_node = ast.new_node_with_children2(Token::IF, c, inner_block);
        let class_name = name(&mut ast, "C");
        let extends = ast.new_node(Token::EMPTY);
        let method_name = name(&mut ast, "");
        let method_params = ast.new_node(Token::PARAM_LIST);
        let method_body = ast.new_node(Token::BLOCK);
        let method =
            ast.new_node_with_children3(Token::FUNCTION, method_name, method_params, method_body);
        let member = ast.new_string_with_token(Token::MEMBER_FUNCTION_DEF, "m");
        member.add_child_to_back(&mut ast, method);
        let members = ast.new_node_with_child(Token::CLASS_MEMBERS, member);
        let class = ast.new_node_with_children3(Token::CLASS, class_name, extends, members);
        let body = ast.new_node_with_child(Token::BLOCK, var);
        for statement in [expr, if_node, class] {
            body.add_child_to_back(&mut ast, statement);
        }
        let function = ast.new_node_with_children3(Token::FUNCTION, f, params, body);
        let script = ast.new_node_with_child(Token::SCRIPT, function);

        // Inside expressions: no scan reads them.
        for n in [init, init_callee, call, g, expr, c, d, members, member] {
            assert_eq!(stamped(&ast, n), [], "{}", n.to_string(&ast));
        }
        // A declared name, its declaration and the statement lists up to the function body.
        assert_eq!(stamped(&ast, x), [body]);
        assert_eq!(stamped(&ast, var), [body]);
        assert_eq!(stamped(&ast, body), [body]);
        assert_eq!(stamped(&ast, if_node), [body]);
        // A nested block, and the body that hoists `var`s out of it.
        assert_eq!(stamped(&ast, y), [inner_block, body]);
        // The class scope and the scope that declares the class.
        assert_eq!(stamped(&ast, class_name), [class, body]);
        assert_eq!(stamped(&ast, class), [class, body]);
        // Parameters and the function's name: the function and its body.
        assert_eq!(stamped(&ast, p), [function, body]);
        assert_eq!(stamped(&ast, default_value), [function, body]);
        assert_eq!(stamped(&ast, f), [function, body]);
        assert_eq!(stamped(&ast, function), [function, body]);
        // A method is a change scope of its own.
        assert_eq!(stamped(&ast, method_body), [method_body]);
        assert_eq!(stamped(&ast, method), [method, method_body]);
        // The top level of a script.
        assert_eq!(stamped(&ast, script), [script]);
        // A detached node.
        let detached = name(&mut ast, "z");
        assert_eq!(stamped(&ast, detached), []);
    }
}
