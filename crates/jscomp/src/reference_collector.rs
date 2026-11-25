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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/ReferenceCollector.java.

//! Port of `ReferenceCollector.java`.

mod function_body;

use function_body::SiteKind;
pub(crate) use function_body::{FunctionReferenceCache, Site};

use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::reference_collection::ReferenceCollection;
use crate::reference_map::ReferenceMap;
use crate::scope::ScopeId;
use crate::scope_creator::ScopeCreator;
use crate::syntactic_scope_cache::SyntacticScopeCache;
use crate::var::VarId;
use closure_rhino::check_state;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::LazyLock;

// Diagnostic comparison with the full ReferenceCollector walk, like CLOSURE_RS_SCOPE_CACHE.
static REUSE_REFERENCE_SITES: LazyLock<bool> =
    LazyLock::new(|| std::env::var("CLOSURE_RS_REFERENCE_SITES").as_deref() != Ok("off"));

/// A helper class for passes that want to access all information about where a variable is
/// referenced and declared at once and then make a decision as to how it should be handled,
/// possibly inlining, reordering, or generating warnings. Callers do this by providing
/// [`Behavior`] and then calling `process(Node, Node)`.
///
/// Java's inner `CollectorCallback` reads the outer instance's fields; here the traversal state
/// lives in [`CollectorCallback`] and the scope creator beside it, so a traversal can borrow
/// both (DESIGN §6: the compiler is not stored).
///
/// Rust-only (performance divergence): the collector is generic over its behavior `B` (Java: a
/// `Behavior` field), so that the per-node hooks a behavior does not override cost nothing.
pub struct ReferenceCollector<'a, B: Behavior> {
    callback: CollectorCallback<'a, B>,
    scope_creator: &'a mut dyn ScopeCreator,
}

/// Java's `Predicate<Var>`.
pub type VarFilter<'a> = Box<dyn Fn(&AbstractCompiler, VarId) -> bool + 'a>;

/// Rust-only (performance divergence): per-local-variable tracking state for `InferConsts`'s
/// zero-allocation leaf-function fast path.
struct InferConstLocal {
    decl_node: NodeId,
    decl_token: Token,
    assign_count: u8,
    assign_in_loop: bool,
    seen_first_ref: bool,
    first_is_init_decl: bool,
    init_block_depth: u32,
    init_block_id: u32,
    well_defined: bool,
}

const SLOT_UNASSIGNED: u32 = u32::MAX;

/// The fields of `ReferenceCollector` that its inner `CollectorCallback` reads and writes.
pub struct CollectorCallback<'a, B: Behavior> {
    /// Maps a given variable to a collection of references to that name. Note that Var objects
    /// are not stable across multiple traversals (unlike scope root or name). A scope-exit
    /// snapshot shares its collection until a later visit adds another reference.
    reference_map: IndexMap<VarId, Rc<ReferenceCollection>>,

    /// The stack of basic blocks and scopes the current traversal is in.
    block_stack: VecDeque<Rc<BasicBlock>>,

    /// Source of behavior at various points in the traversal.
    behavior: B,

    /// Only collect references for filtered variables.
    var_filter: VarFilter<'a>,

    /// If true, hoisted function declarations are traversed when the function is first
    /// referenced. If false, hoisted functions are traversed in statement order.
    collect_hoisted_functions_in_reference_order: bool,

    collected_hoisted_functions: IndexSet<NodeId>,

    narrow_scope: Option<ScopeId>,

    /// Internal users with a pure filter and no per-node hooks can replay plain blocks.
    reuse_sites: bool,

    /// Scratch for one single-scope body's current bindings and reference-map indices.
    body_bindings: Vec<(Option<VarId>, Option<usize>)>,
    /// Scratch stack of nested empty block scopes lazily materialized during cached site replay.
    replay_scope_stack: Vec<(NodeId, Option<ScopeId>)>,
    /// Scratch stack of nested basic blocks lazily materialized during cached site replay.
    replay_block_stack: Vec<(NodeId, Option<Rc<BasicBlock>>)>,
    /// Scratch list of resolved `(slot, var)` pairs for leaf-function fast paths.
    free_vars_scratch: Vec<(u32, VarId)>,
    /// Scratch buffers for `InferConsts`'s zero-allocation leaf-function fast path.
    infer_locals: Vec<InferConstLocal>,
    infer_slot_to_local: Vec<u32>,
    infer_block_stack: Vec<(u32, bool)>,
    /// Rust-only (performance divergence): lazily computed set of hoisted function declaration
    /// names in the global scope so skipped top-level functions can check for pending hoisted
    /// jumps in O(1) per name (or zero cost when empty).
    hoisted_global_fn_names: Option<IndexSet<JsString>>,

    /// Rust-only (performance divergence): `process(externs, root)` may skip the externs (see
    /// `skip_externs_when_unread`).
    may_skip_externs: bool,
    /// Rust-only: the externs the current traversal skips.
    skipped_externs: Option<NodeId>,
    /// Rust-only: while the externs are traversed to make an `ExternsReferenceSummary`, whether
    /// every NAME seen in them resolved to a var declared in the externs.
    externs_refer_only_to_externs: Option<bool>,
}

/// Rust-only (crates/DESIGN.md, "Performance divergences"): whether the NAMEs of the externs
/// resolve only to vars declared in the externs, for the externs as they were when it was made. The
/// externs are the same while no change is recorded inside them
/// (`ChangeTracker::get_externs_change_count`) and they keep their scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternsReferenceSummary {
    externs_version: (NodeId, u64, i32),
    refers_only_to_externs: bool,
}

impl ExternsReferenceSummary {
    pub(crate) fn can_skip(compiler: &AbstractCompiler, externs: NodeId) -> bool {
        compiler.get_externs_root() == Some(externs)
            && compiler.externs_reference_summary.is_some_and(|summary| {
                summary.refers_only_to_externs
                    && summary.externs_version == Self::externs_version(compiler, externs)
            })
    }

    fn externs_version(compiler: &AbstractCompiler, externs: NodeId) -> (NodeId, u64, i32) {
        (
            externs,
            compiler.get_change_tracker_ref().get_externs_change_count(),
            externs.get_child_count(compiler),
        )
    }
}

impl ReferenceCollector<'_, DoNothingBehavior> {
    pub const DO_NOTHING_BEHAVIOR: DoNothingBehavior = DoNothingBehavior;
}

impl<'a, B: Behavior> ReferenceCollector<'a, B> {
    /// Constructor initializes block stack.
    // port: ReferenceCollector#ReferenceCollector(AbstractCompiler,Behavior,ScopeCreator)
    pub fn new(
        compiler: &mut AbstractCompiler,
        behavior: B,
        creator: &'a mut dyn ScopeCreator,
    ) -> Self {
        Self::new_with_filter(compiler, behavior, creator, Box::new(|_, _| true))
    }

    /// Constructor only collects references that match the given variable.
    ///
    /// The test for Var equality uses reference equality, so it's necessary to inject a scope
    /// when you traverse.
    // port: ReferenceCollector#ReferenceCollector(AbstractCompiler,Behavior,ScopeCreator,Predicate)
    pub fn new_with_filter(
        compiler: &mut AbstractCompiler,
        behavior: B,
        creator: &'a mut dyn ScopeCreator,
        var_filter: VarFilter<'a>,
    ) -> Self {
        Self::new_with_hoisted_function_order(
            compiler, behavior, creator, var_filter,
            /* collectHoistedFunctionsInReferenceOrder= */ true,
        )
    }

    /// Like [`Self::new_with_filter`], but if `collect_hoisted_functions_in_reference_order` is
    /// false, hoisted function declarations are traversed in statement order instead of when the
    /// function is first referenced.
    // port: ReferenceCollector#ReferenceCollector(AbstractCompiler,Behavior,ScopeCreator,Predicate,boolean)
    pub fn new_with_hoisted_function_order(
        _compiler: &mut AbstractCompiler,
        behavior: B,
        creator: &'a mut dyn ScopeCreator,
        var_filter: VarFilter<'a>,
        collect_hoisted_functions_in_reference_order: bool,
    ) -> Self {
        Self {
            callback: CollectorCallback {
                reference_map: IndexMap::<_, _>::default(),
                block_stack: VecDeque::new(),
                behavior,
                var_filter,
                collect_hoisted_functions_in_reference_order,
                collected_hoisted_functions: IndexSet::<_>::default(),
                narrow_scope: None,
                body_bindings: Vec::new(),
                replay_scope_stack: Vec::new(),
                replay_block_stack: Vec::new(),
                free_vars_scratch: Vec::new(),
                infer_locals: Vec::new(),
                infer_slot_to_local: Vec::new(),
                infer_block_stack: Vec::new(),
                hoisted_global_fn_names: None,
                reuse_sites: false,
                may_skip_externs: false,
                skipped_externs: None,
                externs_refer_only_to_externs: None,
            },
            scope_creator: creator,
        }
    }

    // port: ReferenceCollector#process(Node)
    pub fn process(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        let Self {
            callback,
            scope_creator,
        } = self;
        create_traversal_builder(compiler, callback, &mut **scope_creator).traverse(root);
    }

    /// Targets reference collection to a particular scope.
    // port: ReferenceCollector#processScope
    pub fn process_scope(&mut self, compiler: &mut AbstractCompiler, scope: ScopeId) {
        let should_add_to_block_stack = !scope.is_hoist_scope(compiler);
        self.callback.narrow_scope = Some(scope);
        if should_add_to_block_stack {
            self.callback
                .push_new_block(compiler, scope.get_root_node(compiler));
        }
        {
            let Self {
                callback,
                scope_creator,
            } = self;
            create_traversal_builder(compiler, callback, &mut **scope_creator)
                .traverse_at_scope(scope);
        }
        if should_add_to_block_stack {
            self.callback
                .pop_last_block(compiler, scope.get_root_node(compiler));
        }
        self.callback.narrow_scope = None;
    }

    /// Rust-only (crates/DESIGN.md, "Performance divergences"): lets `process(externs, root)` leave
    /// out the externs when that cannot change what the behavior sees, for a behavior that reads
    /// neither the references nor the scopes of vars declared in the externs (Java traverses them
    /// on every run). That holds while every NAME in the externs resolves to a var declared in the
    /// externs: no reference collection of another var then has a reference in the externs, and
    /// the externs jump to no hoisted function outside them, so the root is traversed in the
    /// same order. This is found out on a traversal of the externs and kept in the compiler
    /// until the externs change.
    pub fn skip_externs_when_unread(&mut self) {
        self.callback.may_skip_externs = true;
    }

    /// Opt in for an internal traversal whose behavior only observes scope exits and whose
    /// filter is pure. Public collectors can pass a predicate that observes every occurrence.
    pub(crate) fn reuse_reference_sites(&mut self) {
        self.callback.reuse_sites = true;
    }

    /// Gets the variables that were referenced in this callback.
    // port: ReferenceCollector#getAllSymbols
    pub fn get_all_symbols(&self) -> Vec<VarId> {
        self.callback.reference_map.keys().copied().collect()
    }

    // port: ReferenceCollector#getScope
    pub fn get_scope(&self, compiler: &AbstractCompiler, var: VarId) -> ScopeId {
        var.get_scope(compiler)
    }

    /// Gets the reference collection for the given variable.
    // port: ReferenceCollector#getReferences
    pub fn get_references(&self, v: VarId) -> Option<&ReferenceCollection> {
        self.callback.reference_map.get(&v).map(Rc::as_ref)
    }

    /// Rust-only: the collected references, for a caller that lends this collector its behavior and
    /// scope creator only while it runs and keeps the references afterwards
    /// (`CrossChunkReferenceCollector`).
    pub fn into_reference_map(self) -> IndexMap<VarId, ReferenceCollection> {
        self.callback
            .reference_map
            .into_iter()
            .map(|(v, refs)| (v, Rc::unwrap_or_clone(refs)))
            .collect()
    }
}

impl<B: Behavior> CompilerPass for ReferenceCollector<'_, B> {
    /// Convenience method for running this pass over a tree with this class as a callback.
    // port: ReferenceCollector#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let Self {
            callback,
            scope_creator,
        } = self;
        if *REUSE_REFERENCE_SITES
            && (callback.reuse_sites
                || callback.behavior.can_reuse_function_reference_sites()
                || callback.behavior.use_inline_vars_clean_cache()
                || callback.behavior.infer_consts_leaf_fast_path())
        {
            function_body::FunctionReferenceCache::precompute_parallel(compiler, root);
        }
        if callback.may_skip_externs && compiler.get_externs_root() == Some(externs) {
            // Rust-only (performance divergence): see `skip_externs_when_unread`.
            let version = ExternsReferenceSummary::externs_version(compiler, externs);
            let cached_only_externs = match compiler.externs_reference_summary {
                Some(summary) if summary.externs_version == version => {
                    Some(summary.refers_only_to_externs)
                }
                _ => {
                    if try_verify_bundled_externs_refer_only_to_externs(
                        compiler,
                        &mut **scope_creator,
                        externs,
                    ) {
                        compiler.externs_reference_summary = Some(ExternsReferenceSummary {
                            externs_version: version,
                            refers_only_to_externs: true,
                        });
                        Some(true)
                    } else {
                        None
                    }
                }
            };
            match cached_only_externs {
                Some(true) => {
                    callback.skipped_externs = Some(externs);
                    create_traversal_builder(compiler, callback, &mut **scope_creator)
                        .traverse_roots_skipping_externs(externs, root);
                    callback.skipped_externs = None;
                    return;
                }
                Some(false) => {}
                None => {
                    callback.externs_refer_only_to_externs = Some(true);
                    create_traversal_builder(compiler, callback, &mut **scope_creator)
                        .traverse_roots(externs, root);
                    let refers_only_to_externs =
                        callback.externs_refer_only_to_externs.take().unwrap();
                    compiler.externs_reference_summary = Some(ExternsReferenceSummary {
                        externs_version: version,
                        refers_only_to_externs,
                    });
                    return;
                }
            }
        }
        create_traversal_builder(compiler, callback, &mut **scope_creator)
            .traverse_roots(externs, root);
    }
}

impl<B: Behavior> ReferenceMap for ReferenceCollector<'_, B> {
    fn get_references(&self, var: VarId) -> Option<&ReferenceCollection> {
        ReferenceCollector::get_references(self, var)
    }

    fn shared_references(&self, var: VarId) -> Option<Rc<ReferenceCollection>> {
        self.callback.reference_map.get(&var).cloned()
    }
}

/// Rust-only (performance divergence): verifies `externs_refer_only_to_externs` on the first run
/// using precomputed `BundledScript::extern_referenced_names` plus a shallow scan of any appended
/// or synthetic global-only extern statements, avoiding a full 250K-node extern AST traversal.
fn try_verify_bundled_externs_refer_only_to_externs(
    compiler: &mut AbstractCompiler,
    scope_creator: &mut dyn ScopeCreator,
    externs: NodeId,
) -> bool {
    use crate::source_file::bundled_script::BundledScript;
    use closure_rhino::node::SnapshotUse;

    fn collect_global_only_names(ast: &Ast, n: NodeId, out: &mut Vec<NodeId>) -> bool {
        let token = n.get_token(ast);
        match token {
            Token::FUNCTION
            | Token::CLASS
            | Token::BLOCK
            | Token::MODULE_BODY
            | Token::CATCH
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY
            | Token::DESTRUCTURING_LHS
            | Token::IMPORT_STAR
            | Token::IMPORT_SPEC
            | Token::EXPORT_SPEC => return false,
            Token::NAME if !n.get_string_ref(ast).is_empty() => {
                out.push(n);
            }
            _ => {}
        }
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            if !collect_global_only_names(ast, c, out) {
                return false;
            }
            child = c.get_next(ast);
        }
        true
    }

    let Some(global_root) = externs.get_parent(compiler) else {
        return false;
    };
    let mut bundled_slices: Vec<&'static [&'static str]> = Vec::new();
    let mut extra_name_nodes: Vec<NodeId> = Vec::new();
    let mut saw_bundled = false;

    let mut script = externs.get_first_child(compiler);
    while let Some(s) = script {
        if !s.is_script(compiler) {
            return false;
        }
        if let Some((facts, suffix)) =
            compiler.parsed_script_record::<BundledScript>(s, SnapshotUse::SyntaxAndNonParamNames)
            && let Some(names) = facts.extern_referenced_names
        {
            saw_bundled = true;
            if !names.is_empty() {
                bundled_slices.push(names);
            }
            let mut stmt = suffix;
            while let Some(st) = stmt {
                if !collect_global_only_names(compiler, st, &mut extra_name_nodes) {
                    return false;
                }
                stmt = st.get_next(compiler);
            }
        } else {
            let mut stmt = s.get_first_child(compiler);
            while let Some(st) = stmt {
                if !collect_global_only_names(compiler, st, &mut extra_name_nodes) {
                    return false;
                }
                stmt = st.get_next(compiler);
            }
        }
        script = s.get_next(compiler);
    }

    if !saw_bundled {
        return false;
    }

    let global_scope = scope_creator.create_scope(compiler, global_root, None);
    let mut checked: IndexSet<&'static str> = IndexSet::default();
    for slice in bundled_slices {
        for &name in slice {
            if !checked.insert(name) {
                continue;
            }
            let v = global_scope.get_var(compiler, closure_rhino::js_string::JsString::from(name));
            if !v.is_some_and(|v| v.get_input(compiler).is_some_and(|i| i.is_extern())) {
                return false;
            }
        }
    }
    for name_node in extra_name_nodes {
        let v = global_scope.get_var_of_node(compiler, name_node);
        if !v.is_some_and(|v| v.get_input(compiler).is_some_and(|i| i.is_extern())) {
            return false;
        }
    }
    true
}

/// Same as process but only runs on a part of AST associated to one script.
// port: ReferenceCollector#createTraversalBuilder
fn create_traversal_builder<'b>(
    compiler: &'b mut AbstractCompiler,
    callback: &'b mut dyn Callback,
    scope_creator: &'b mut dyn ScopeCreator,
) -> crate::node_traversal::Builder<'b> {
    let mut builder = NodeTraversal::builder();
    builder
        .set_compiler(compiler)
        .set_callback(callback)
        .set_scope_creator(scope_creator)
        .set_obey_destructuring_and_default_value_execution_order(true);
    builder
}

impl<B: Behavior> CollectorCallback<'_, B> {
    fn needs_name(&self, ast: &Ast, node: NodeId) -> bool {
        // Building the extern summary needs every binding even if this behavior ignores it.
        self.externs_refer_only_to_externs == Some(true) || self.behavior.needs_name(ast, node)
    }

    /// If this reference would recursively collect a hoisted function, return that function
    /// and the scope containing its declaration. Even a filtered-out reference can cause a jump.
    fn pending_hoisted_function(
        &self,
        compiler: &AbstractCompiler,
        var: VarId,
    ) -> Option<(NodeId, ScopeId)> {
        let function = var.get_parent_node(compiler)?;
        if !NodeUtil::is_hoisted_function_declaration(compiler, function)
            || self.collected_hoisted_functions.contains(&function)
        {
            return None;
        }
        let scope = var.get_scope(compiler);
        if self
            .narrow_scope
            .is_some_and(|narrow| narrow.get_depth(compiler) > scope.get_depth(compiler))
            || self
                .skipped_externs
                .is_some_and(|externs| is_inside(compiler, function, externs))
        {
            return None;
        }
        Some((function, scope))
    }

    // port: ReferenceCollector#maybeJumpToHoistedFunction
    fn maybe_jump_to_hoisted_function(
        &mut self,
        var: VarId,
        t: &mut NodeTraversal<'_>,
        reference_scope: ScopeId,
    ) {
        let Some((fn_node, var_scope)) = self.pending_hoisted_function(t.get_compiler(), var)
        else {
            return;
        };

        // Replace the block stack with a new one matching the hoist position.
        //
        // This algorithm only works because we know hoisted functions cannot be inside loops. It
        // will have to change if we ever do general function continuations.
        //
        // This is tricky to compute because of the weird traverseAtScope call for
        // AggressiveInlineAliases.
        let old_block_stack = std::mem::take(&mut self.block_stack);
        if var_scope.is_global(t.get_compiler()) {
            self.block_stack
                .push_back(old_block_stack.front().expect("getFirst").clone());
        } else {
            let var_scope_root = var_scope.get_root_node(t.get_compiler());
            for b in &old_block_stack {
                self.block_stack.push_back(b.clone());
                if b.get_root() == var_scope_root {
                    break;
                }
            }
        }

        // Record the function declaration reference explicitly because it is a bleeding name.
        // The reference must be recorded with the containing scope, and traverseAtScope skips
        // bleeding function NAME nodes
        let fn_name = fn_node.get_first_child(t.get_compiler()).unwrap();
        self.add_reference(var, fn_name, t, reference_scope);

        let (compiler, scope_creator) = t.get_compiler_and_scope_creator();
        let fn_scope = scope_creator.create_scope(compiler, fn_node, Some(var_scope));
        create_traversal_builder(compiler, self, scope_creator).traverse_at_scope(fn_scope);
        self.block_stack = old_block_stack;
    }

    /// Rebind every spelling against this traversal's actual scope. A pending jump can invoke a
    /// mutating scope-exit callback in the middle of this body. In that case collect the whole
    /// body normally, before adding any reference or changing the block stack.
    fn collect_body_from_sites(&mut self, t: &mut NodeTraversal<'_>, body: NodeId) -> bool {
        let Some(body) = FunctionReferenceCache::sites(t.get_compiler(), body) else {
            return false;
        };
        if body.first_names.is_empty() {
            return true;
        }
        let mut scope = None;
        self.body_bindings.clear();
        for &name in &body.first_names {
            t.position_at_node(name);
            if !self.needs_name(t, name) {
                self.body_bindings.push((None, None));
                continue;
            }
            let scope = *scope.get_or_insert_with(|| t.get_scope());
            let var = scope.get_var_of_node(t.get_compiler(), name);
            if self.collect_hoisted_functions_in_reference_order
                && var.is_some_and(|v| self.pending_hoisted_function(t.get_compiler(), v).is_some())
            {
                return false;
            }
            self.body_bindings.push((var, None));
        }

        // Each spelling now has one stable binding until the end of this body. Insertion at its
        // first occurrence preserves the reference map's insertion order in all inlining modes.
        let mut has_references = false;
        for ((var, index), &name) in self.body_bindings.iter_mut().zip(&body.first_names) {
            t.position_at_node(name);
            if let Some(v) = var.filter(|&v| {
                (self.var_filter)(t.get_compiler(), v)
                    && self.behavior.needs_variable(t.get_compiler(), v)
            }) {
                let entry = self.reference_map.entry(v);
                *index = Some(entry.index());
                entry.or_default();
                has_references = true;
            }
        }
        if has_references {
            let root_scope = scope.expect("collected references have a scope");
            self.replay_body_sites(t, &body.sites, root_scope);
        }
        true
    }

    /// Rust-only (performance divergence): replays `sites` using `self.body_bindings`,
    /// lazily materializing both nested `BasicBlock`s and nested empty `ScopeId`s only when an
    /// active reference actually fires inside them.
    fn replay_body_sites(
        &mut self,
        t: &mut NodeTraversal<'_>,
        sites: &[Site],
        root_scope: ScopeId,
    ) {
        self.replay_scope_stack.clear();
        self.replay_block_stack.clear();
        for site in sites {
            match site.decode() {
                SiteKind::Reference(name, binding) => {
                    if let Some(index) = self.body_bindings[binding as usize].1 {
                        let ref_scope = if self.replay_scope_stack.is_empty() {
                            root_scope
                        } else {
                            Self::materialize_replay_scope(
                                t,
                                root_scope,
                                &mut self.replay_scope_stack,
                            )
                        };
                        t.position_at_node(name);
                        let block = Self::materialize_replay_block(
                            t,
                            &self.block_stack,
                            &mut self.replay_block_stack,
                        );
                        Rc::make_mut(&mut self.reference_map[index])
                            .add(Reference::in_scope(name, t, block, ref_scope));
                    }
                }
                SiteKind::EnterBlock(n) => self.replay_block_stack.push((n, None)),
                SiteKind::ExitBlock(n) => {
                    let (root, _) = self.replay_block_stack.pop().expect("removeLast");
                    check_state!(root == n, &n.to_string(t));
                }
                SiteKind::EnterEmptyScope(n) => self.replay_scope_stack.push((n, None)),
                SiteKind::ExitEmptyScope => {
                    self.replay_scope_stack.pop();
                }
            }
        }
    }

    fn materialize_replay_block(
        ast: &Ast,
        base_stack: &VecDeque<Rc<BasicBlock>>,
        replay_stack: &mut [(NodeId, Option<Rc<BasicBlock>>)],
    ) -> Rc<BasicBlock> {
        let Some(last) = replay_stack.last() else {
            return base_stack.back().expect("getLast").clone();
        };
        if let Some(b) = &last.1 {
            return b.clone();
        }
        let mut parent = base_stack.back().cloned();
        for (root_node, cached) in replay_stack.iter_mut() {
            let block = match cached {
                Some(b) => b.clone(),
                None => {
                    let b = BasicBlock::new(ast, parent, *root_node);
                    *cached = Some(b.clone());
                    b
                }
            };
            parent = Some(block);
        }
        parent.expect("non-empty replay block stack")
    }

    /// Rust-only (performance divergence): fast-path skip for unchanged leaf functions recorded
    /// in `InlineVarsCleanCache`. Records any outer-scope function-declaration name and any
    /// references to free variables resolving outside `fn_node` while skipping `fn_node`'s own
    /// local variables and `InliningBehavior::do_inlines_for_scope`.
    fn try_skip_clean_leaf_function(
        &mut self,
        t: &mut NodeTraversal<'_>,
        fn_node: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if !t
            .get_compiler()
            .inline_vars_clean_cache
            .clean_leaves
            .contains_key(&fn_node)
        {
            return false;
        }
        let stamp = SyntacticScopeCache::subtree_stamp_with_flags(t.get_compiler(), fn_node);
        let free = match t
            .get_compiler()
            .inline_vars_clean_cache
            .clean_leaves
            .get(&fn_node)
        {
            Some(entry) if entry.fn_stamp == stamp => entry.free.clone(),
            _ => {
                t.get_compiler()
                    .inline_vars_clean_cache
                    .clean_leaves
                    .swap_remove(&fn_node);
                return false;
            }
        };
        let enclosing_scope = t.get_scope();
        self.free_vars_scratch.clear();
        if let Some(free) = &free {
            for (slot, &name) in free.first_names.iter().enumerate() {
                t.position_at_node(name);
                if !self.needs_name(t, name) {
                    continue;
                }
                let var = enclosing_scope.get_var_of_node(t.get_compiler(), name);
                if self.collect_hoisted_functions_in_reference_order
                    && var.is_some_and(|v| {
                        self.pending_hoisted_function(t.get_compiler(), v).is_some()
                    })
                {
                    return false;
                }
                if let Some(v) = var
                    && (self.var_filter)(t.get_compiler(), v)
                    && self.behavior.needs_variable(t.get_compiler(), v)
                {
                    self.free_vars_scratch.push((slot as u32, v));
                }
            }
        }

        let is_fn_decl = parent.is_some() && NodeUtil::is_function_declaration(t, fn_node);
        let boundary = BasicBlock::is_block_boundary(t, fn_node, parent);
        let has_free_refs = !self.free_vars_scratch.is_empty();
        if boundary && (is_fn_decl || has_free_refs) {
            self.push_new_block(t, fn_node);
        }
        if is_fn_decl {
            let fn_name = fn_node.get_first_child(t).unwrap();
            t.position_at_node(fn_name);
            if self.needs_name(t, fn_name)
                && let Some(v) = enclosing_scope.get_var_of_node(t.get_compiler(), fn_name)
            {
                self.add_reference(v, fn_name, t, enclosing_scope);
            }
        }
        if has_free_refs {
            let free = free.as_ref().expect("free refs imply free sites");
            let body = fn_node.get_last_child(t).expect("function body");
            self.body_bindings.clear();
            self.body_bindings
                .resize(free.first_names.len(), (None, None));
            for i in 0..self.free_vars_scratch.len() {
                let (slot, v) = self.free_vars_scratch[i];
                let entry = self.reference_map.entry(v);
                self.body_bindings[slot as usize] = (Some(v), Some(entry.index()));
                entry.or_default();
            }
            let (compiler, scope_creator) = t.get_compiler_and_scope_creator();
            let fn_scope = scope_creator.create_scope(compiler, fn_node, Some(enclosing_scope));
            let body_scope = scope_creator.create_scope(compiler, body, Some(fn_scope));
            self.push_new_block(t, fn_node);
            self.replay_body_sites(t, &free.sites, body_scope);
            self.pop_last_block(t, fn_node);
        }
        if boundary && (is_fn_decl || has_free_refs) {
            self.pop_last_block(t, fn_node);
        }
        true
    }

    /// Rust-only (performance divergence): evaluates `InferConsts` directly for simple-param leaf
    /// functions using cached `FunctionBodySites`, avoiding allocating `ScopeId`, `VarId`,
    /// `BasicBlock`, and `Reference` objects for all local variables and parameters while still
    /// recording any active outer-scope references identically to normal traversal.
    fn try_infer_consts_leaf_function(
        &mut self,
        t: &mut NodeTraversal<'_>,
        fn_node: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let fn_name = fn_node.get_first_child(t).unwrap();
        let args = fn_name.get_next(t).unwrap();
        let body = args.get_next(t).unwrap();
        if !body.is_block(t) {
            return false;
        }
        let is_fn_decl = parent.is_some() && NodeUtil::is_function_declaration(t, fn_node);
        let is_fn_expr_named =
            !NodeUtil::is_function_declaration(t, fn_node) && !fn_name.get_string_ref(t).is_empty();
        if is_fn_expr_named
            && (!NodeUtil::is_function_expression(t, fn_node)
                || fn_name.get_string_ref(t) == "arguments")
        {
            return false;
        }

        // Validate simple NAME-only parameter list before building local state.
        let mut param = args.get_first_child(t);
        while let Some(p) = param {
            if !p.is_name(t) {
                return false;
            }
            let Some(spelling) = p.string_payload(t) else {
                return false;
            };
            if spelling.is_empty() || spelling == "arguments" {
                return false;
            }
            param = p.get_next(t);
        }

        let Some(body_sites) = FunctionReferenceCache::sites(t.get_compiler(), body) else {
            return false;
        };

        // Populate `fn_scope` locals (parameters first, then bleeding function expression name,
        // matching `SyntacticScopeCreator#populate`).
        self.infer_locals.clear();
        let mut param = args.get_first_child(t);
        while let Some(p) = param {
            let spelling = p.get_string_ref(t);
            if let Some(existing) = self
                .infer_locals
                .iter_mut()
                .find(|l| l.decl_node.get_string_ref(t) == spelling)
            {
                existing.assign_count = existing.assign_count.saturating_add(1);
            } else {
                self.infer_locals.push(InferConstLocal {
                    decl_node: p,
                    decl_token: Token::PARAM_LIST,
                    assign_count: 1,
                    assign_in_loop: false,
                    seen_first_ref: true,
                    first_is_init_decl: true,
                    init_block_depth: 0,
                    init_block_id: 0,
                    well_defined: true,
                });
            }
            param = p.get_next(t);
        }
        if is_fn_expr_named {
            let spelling = fn_name.get_string_ref(t);
            if let Some(existing) = self
                .infer_locals
                .iter_mut()
                .find(|l| l.decl_node.get_string_ref(t) == spelling)
            {
                existing.assign_count = existing.assign_count.saturating_add(1);
            } else {
                self.infer_locals.push(InferConstLocal {
                    decl_node: fn_name,
                    decl_token: Token::FUNCTION,
                    assign_count: 1,
                    assign_in_loop: false,
                    seen_first_ref: true,
                    first_is_init_decl: true,
                    init_block_depth: 0,
                    init_block_id: 0,
                    well_defined: true,
                });
            }
        }
        let fn_scope_local_count = self.infer_locals.len();

        // Discover `body_scope` declarations from `body_sites` in exact AST order.
        self.infer_slot_to_local.clear();
        self.infer_slot_to_local
            .resize(body_sites.first_names.len(), SLOT_UNASSIGNED);
        for &site in &body_sites.sites {
            let SiteKind::Reference(node, slot) = site.decode() else {
                continue;
            };
            if node.is_import_star(t) {
                return false;
            }
            let p = node.get_parent(t).unwrap();
            let pt = p.get_token(t);
            match pt {
                Token::VAR | Token::LET | Token::CONST => {
                    let spelling = node.get_string_ref(t);
                    if spelling.is_empty() || spelling == "arguments" {
                        return false;
                    }
                    if self.infer_slot_to_local[slot as usize] != SLOT_UNASSIGNED {
                        continue;
                    }
                    if let Some((idx, existing)) = self.infer_locals[..fn_scope_local_count]
                        .iter()
                        .enumerate()
                        .find(|(_, l)| l.decl_node.get_string_ref(t) == spelling)
                    {
                        if existing.decl_token == Token::PARAM_LIST {
                            // `isShadowingAllowed` is false for function parameters: the var
                            // redeclaration resolves to the parameter in `fn_scope`.
                            self.infer_slot_to_local[slot as usize] = idx as u32;
                        } else {
                            // Bleeding function expression name shadowed in body (rare): fall back.
                            return false;
                        }
                    } else {
                        let idx = self.infer_locals.len() as u32;
                        self.infer_locals.push(InferConstLocal {
                            decl_node: node,
                            decl_token: pt,
                            assign_count: 0,
                            assign_in_loop: false,
                            seen_first_ref: false,
                            first_is_init_decl: false,
                            init_block_depth: 0,
                            init_block_id: 0,
                            well_defined: true,
                        });
                        self.infer_slot_to_local[slot as usize] = idx;
                    }
                }
                Token::ARRAY_PATTERN
                | Token::OBJECT_PATTERN
                | Token::STRING_KEY
                | Token::COMPUTED_PROP
                | Token::ITER_REST
                | Token::OBJECT_REST
                | Token::DEFAULT_VALUE
                | Token::DESTRUCTURING_LHS
                | Token::IMPORT
                | Token::IMPORT_SPEC
                | Token::CLASS
                | Token::FUNCTION
                | Token::CATCH
                    if Reference::is_declaration_node(t, node) =>
                {
                    return false;
                }
                _ => {}
            }
        }

        // Resolve all remaining slots: `fn_scope` bindings, implicit `arguments`, or free outer vars.
        let enclosing_scope = t.get_scope();
        if is_fn_decl {
            t.position_at_node(fn_name);
            if self.collect_hoisted_functions_in_reference_order
                && self.needs_name(t, fn_name)
                && let Some(v) = enclosing_scope.get_var_of_node(t.get_compiler(), fn_name)
                && self.pending_hoisted_function(t.get_compiler(), v).is_some()
            {
                return false;
            }
        }
        let is_arrow = fn_node.is_arrow_function(t);
        self.free_vars_scratch.clear();
        for (slot, &first_name) in body_sites.first_names.iter().enumerate() {
            if self.infer_slot_to_local[slot] != SLOT_UNASSIGNED {
                continue;
            }
            let spelling = first_name.get_string_ref(t);
            if spelling.is_empty() {
                return false;
            }
            if let Some(idx) = self.infer_locals[..fn_scope_local_count]
                .iter()
                .position(|l| l.decl_node.get_string_ref(t) == spelling)
            {
                self.infer_slot_to_local[slot] = idx as u32;
                continue;
            }
            if !is_arrow && spelling == "arguments" {
                // Resolves to `fn_scope`'s implicit `arguments` var (`name_node = None`, no-op).
                continue;
            }
            t.position_at_node(first_name);
            if !self.needs_name(t, first_name) {
                continue;
            }
            let var = enclosing_scope.get_var_of_node(t.get_compiler(), first_name);
            if self.collect_hoisted_functions_in_reference_order
                && var.is_some_and(|v| self.pending_hoisted_function(t.get_compiler(), v).is_some())
            {
                return false;
            }
            if let Some(v) = var
                && (self.var_filter)(t.get_compiler(), v)
                && self.behavior.needs_variable(t.get_compiler(), v)
            {
                self.free_vars_scratch.push((slot as u32, v));
            }
        }

        // Single linear pass over `body_sites.sites` to evaluate all local variables.
        self.infer_block_stack.clear();
        self.infer_block_stack.push((0, false));
        let mut next_block_id: u32 = 1;
        let mut loop_depth: u32 = 0;
        for &site in &body_sites.sites {
            match site.decode() {
                SiteKind::Reference(node, slot) => {
                    let local_idx = self.infer_slot_to_local[slot as usize];
                    if local_idx != SLOT_UNASSIGNED {
                        let local = &mut self.infer_locals[local_idx as usize];
                        let is_init_decl = Reference::is_initializing_declaration_node(t, node);
                        let is_assign = is_init_decl || Reference::is_lvalue_node(t, node);
                        if !local.seen_first_ref {
                            local.seen_first_ref = true;
                            local.first_is_init_decl = is_init_decl;
                            let depth = (self.infer_block_stack.len() - 1) as u32;
                            local.init_block_depth = depth;
                            local.init_block_id = self.infer_block_stack[depth as usize].0;
                        } else if local.well_defined {
                            let depth = local.init_block_depth as usize;
                            if self.infer_block_stack.get(depth).map(|&(id, _)| id)
                                != Some(local.init_block_id)
                            {
                                local.well_defined = false;
                            }
                        }
                        if is_assign {
                            local.assign_count = local.assign_count.saturating_add(1);
                            if loop_depth > 0 {
                                local.assign_in_loop = true;
                            }
                        }
                    }
                }
                SiteKind::EnterBlock(n) => {
                    let is_loop = BasicBlock::is_loop_root(t, n);
                    let id = next_block_id;
                    next_block_id += 1;
                    self.infer_block_stack.push((id, is_loop));
                    if is_loop {
                        loop_depth += 1;
                    }
                }
                SiteKind::ExitBlock(_) => {
                    let (_, is_loop) = self.infer_block_stack.pop().expect("balanced block");
                    if is_loop {
                        loop_depth -= 1;
                    }
                }
                SiteKind::EnterEmptyScope(_) | SiteKind::ExitEmptyScope => {}
            }
        }

        // Apply `declared_constant_var` and `inferred_constant_var` flags directly on each local.
        let compiler = t.get_compiler();
        for local in &self.infer_locals {
            let name_node = local.decl_node;
            if !name_node.is_declared_constant_var(compiler)
                && (local.decl_token == Token::CONST
                    || NodeUtil::get_best_jsdoc_info(compiler, name_node)
                        .is_some_and(|d| d.is_constant()))
            {
                name_node.set_declared_constant_var(compiler, true);
            }
            if !name_node.is_inferred_constant_var(compiler)
                && local.assign_count == 1
                && !local.assign_in_loop
            {
                let is_inferred = match local.decl_token {
                    Token::CONST | Token::PARAM_LIST | Token::FUNCTION => true,
                    Token::LET => {
                        !name_node.get_parent(compiler).unwrap().is_let(compiler)
                            || name_node.has_children(compiler)
                    }
                    Token::VAR => local.first_is_init_decl && local.well_defined,
                    _ => false,
                };
                if is_inferred {
                    name_node.set_inferred_constant_var(compiler, true);
                }
            }
        }

        // Record outer-scope function declaration name and any active free outer references.
        let boundary = BasicBlock::is_block_boundary(t, fn_node, parent);
        let has_free_refs = !self.free_vars_scratch.is_empty();
        if boundary && (is_fn_decl || has_free_refs) {
            self.push_new_block(t, fn_node);
        }
        if is_fn_decl {
            t.position_at_node(fn_name);
            if self.needs_name(t, fn_name)
                && let Some(v) = enclosing_scope.get_var_of_node(t.get_compiler(), fn_name)
            {
                self.add_reference(v, fn_name, t, enclosing_scope);
            }
        }
        if has_free_refs {
            self.body_bindings.clear();
            self.body_bindings
                .resize(body_sites.first_names.len(), (None, None));
            for i in 0..self.free_vars_scratch.len() {
                let (slot, v) = self.free_vars_scratch[i];
                let entry = self.reference_map.entry(v);
                self.body_bindings[slot as usize] = (Some(v), Some(entry.index()));
                entry.or_default();
            }
            // For any outer variable referenced inside `fn_node`, `fn_node`'s single function
            // BasicBlock and empty child ScopeId give identical `is_assigned_once_in_lifetime`
            // (always false for assignments inside `fn_node`) and `is_well_defined` (every block
            // inside `fn_node` is a descendant of `fn_block`).
            let empty_fn_scope =
                ScopeId::create_child_scope(t.get_compiler(), enclosing_scope, fn_node);
            let fn_block = BasicBlock::new(t, self.block_stack.back().cloned(), fn_node);
            for &site in &body_sites.sites {
                if let SiteKind::Reference(name, slot) = site.decode()
                    && let Some(index) = self.body_bindings[slot as usize].1
                {
                    t.position_at_node(name);
                    Rc::make_mut(&mut self.reference_map[index]).add(Reference::in_scope(
                        name,
                        t,
                        fn_block.clone(),
                        empty_fn_scope,
                    ));
                }
            }
        }
        if boundary && (is_fn_decl || has_free_refs) {
            self.pop_last_block(t, fn_node);
        }
        true
    }

    /// Rust-only (performance divergence): skips a top-level (global-scope) function when the
    /// behavior guarantees no local variable inside it needs action and global variables are
    /// ignored, provided no `NAME` inside it references a pending hoisted global function.
    fn try_skip_global_function_if_no_pending_hoist(
        &mut self,
        t: &mut NodeTraversal<'_>,
        fn_node: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if !self.collect_hoisted_functions_in_reference_order {
            return true;
        }
        let enclosing_scope = t.get_scope();
        let hoisted_names = self.hoisted_global_fn_names.get_or_insert_with(|| {
            enclosing_scope.collect_hoisted_function_names(t.get_compiler())
        });
        if hoisted_names.is_empty() {
            return true;
        }
        let fn_name = fn_node.get_first_child(t).unwrap();
        let args = fn_name.get_next(t).unwrap();
        let body = args.get_next(t).unwrap();
        let is_fn_decl = parent.is_some() && NodeUtil::is_function_declaration(t, fn_node);

        // Collect candidate NAME nodes in `fn_node` whose spelling is a hoisted global function.
        // We borrow `hoisted_global_fn_names` immutably during the AST scan, then resolve matches.
        let cached_body_sites = if body.is_block(t)
            && t.get_compiler()
                .function_reference_sites
                .has_cached_sites(body)
        {
            let stamp = SyntacticScopeCache::subtree_stamp(t.get_compiler(), body);
            t.get_compiler()
                .function_reference_sites
                .cached_sites_with_stamp(body, stamp)
        } else {
            None
        };
        let mut hits: Vec<NodeId> = Vec::new();
        {
            let ast: &Ast = &t.get_compiler().ast;
            let hoisted_ref = self.hoisted_global_fn_names.as_ref().unwrap();
            fn scan_hoisted_hits(
                ast: &Ast,
                n: NodeId,
                hoisted: &IndexSet<JsString>,
                out: &mut Vec<NodeId>,
            ) {
                if n.is_name(ast)
                    && let Some(s) = n.string_payload(ast)
                    && hoisted.contains(s)
                {
                    out.push(n);
                }
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    scan_hoisted_hits(ast, c, hoisted, out);
                    child = c.get_next(ast);
                }
            }
            if !is_fn_decl
                && let Some(s) = fn_name.string_payload(ast)
                && hoisted_ref.contains(s)
            {
                hits.push(fn_name);
            }
            scan_hoisted_hits(ast, args, hoisted_ref, &mut hits);
            // If `body` already has cached `FunctionBodySites`, check only its deduplicated names.
            if let Some(cached) = &cached_body_sites {
                for &name in &cached.first_names {
                    if let Some(s) = name.string_payload(ast)
                        && hoisted_ref.contains(s)
                    {
                        hits.push(name);
                    }
                }
            } else {
                scan_hoisted_hits(ast, body, hoisted_ref, &mut hits);
            }
        }
        for name in hits {
            if let Some(v) = enclosing_scope.get_var_of_node(t.get_compiler(), name)
                && self.pending_hoisted_function(t.get_compiler(), v).is_some()
            {
                return false;
            }
        }
        true
    }

    /// Rust-only (performance divergence): skips entering a simple-param leaf function when the
    /// behavior guarantees no local variable declared inside it can be needed and none of the
    /// names in its parameters or cached `FunctionBodySites` pass `needs_name`.
    fn try_skip_function_with_no_needed_names(
        &mut self,
        t: &mut NodeTraversal<'_>,
        fn_node: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let fn_name = fn_node.get_first_child(t).unwrap();
        let args = fn_name.get_next(t).unwrap();
        let body = args.get_next(t).unwrap();
        if !body.is_block(t) {
            return false;
        }
        let mut param = args.get_first_child(t);
        while let Some(p) = param {
            if !p.is_name(t) || self.needs_name(t, p) {
                return false;
            }
            param = p.get_next(t);
        }
        let is_fn_decl = parent.is_some() && NodeUtil::is_function_declaration(t, fn_node);
        if !is_fn_decl && !fn_name.get_string_ref(t).is_empty() && self.needs_name(t, fn_name) {
            return false;
        }
        let Some(body_sites) = FunctionReferenceCache::sites(t.get_compiler(), body) else {
            return false;
        };
        for &name in &body_sites.first_names {
            if self.needs_name(t, name) {
                return false;
            }
        }
        if is_fn_decl && self.needs_name(t, fn_name) {
            let enclosing_scope = t.get_scope();
            t.position_at_node(fn_name);
            let var = enclosing_scope.get_var_of_node(t.get_compiler(), fn_name);
            if self.collect_hoisted_functions_in_reference_order
                && var.is_some_and(|v| self.pending_hoisted_function(t.get_compiler(), v).is_some())
            {
                return false;
            }
            if let Some(v) = var {
                let boundary = BasicBlock::is_block_boundary(t, fn_node, parent);
                if boundary {
                    self.push_new_block(t, fn_node);
                }
                self.add_reference(v, fn_name, t, enclosing_scope);
                if boundary {
                    self.pop_last_block(t, fn_node);
                }
            }
        }
        true
    }

    fn materialize_replay_scope(
        t: &mut NodeTraversal<'_>,
        root_scope: ScopeId,
        scope_stack: &mut [(NodeId, Option<ScopeId>)],
    ) -> ScopeId {
        if let Some(s) = scope_stack.last().unwrap().1 {
            return s;
        }
        let (compiler, scope_creator) = t.get_compiler_and_scope_creator();
        let mut parent = root_scope;
        for (root_node, cached) in scope_stack.iter_mut() {
            parent = match *cached {
                Some(s) => s,
                None => {
                    let s = scope_creator.create_scope(compiler, *root_node, Some(parent));
                    *cached = Some(s);
                    s
                }
            };
        }
        parent
    }

    // port: ReferenceCollector#pushNewBlock
    fn push_new_block(&mut self, ast: &Ast, root: NodeId) {
        let block = BasicBlock::new(ast, self.block_stack.back().cloned(), root);
        self.block_stack.push_back(block);
    }

    // port: ReferenceCollector#popLastBlock
    fn pop_last_block(&mut self, ast: &Ast, root: NodeId) {
        let last = self.block_stack.pop_back().expect("removeLast");
        // Verfiy that the stack is unwound correctly.
        check_state!(root == last.get_root(), &root.to_string(ast));
    }

    /// Java returns the added `Reference`; here it is named by the index of its var's entry in
    /// `reference_map` (entries are never removed) and its index in that collection
    /// (`added_reference`), so it is neither copied nor looked up by var again.
    // port: ReferenceCollector#addReference
    fn add_reference(
        &mut self,
        v: VarId,
        n: NodeId,
        t: &mut NodeTraversal<'_>,
        scope: ScopeId,
    ) -> Option<(usize, usize)> {
        if !(self.var_filter)(t.get_compiler(), v)
            || !self.behavior.needs_variable(t.get_compiler(), v)
        {
            return None;
        }

        // Create collection if none already
        let block = self.block_stack.back().expect("getLast").clone();
        let reference = Reference::in_scope(n, t, block, scope);
        let entry = self.reference_map.entry(v);
        let entry_index = entry.index();
        let collection = Rc::make_mut(entry.or_default());

        // Add this particular reference
        collection.add(reference);
        Some((entry_index, collection.references.len() - 1))
    }
}

/// Rust-only: whether `n` is `root` or below it.
fn is_inside(ast: &Ast, n: NodeId, root: NodeId) -> bool {
    let mut node = Some(n);
    while let Some(current) = node {
        if current == root {
            return true;
        }
        node = current.get_parent(ast);
    }
    false
}

/// Returns true if `n` is the non-local name in `import {n as x}` or `export {x as n}`, which is not
/// a reference even though it is a NAME node.
// port: ReferenceCollector#isNonLocalModuleSpecifierName
fn is_non_local_module_specifier_name(ast: &Ast, n: NodeId, parent: NodeId) -> bool {
    (parent.is_import_spec(ast) && Some(n) != parent.get_last_child(ast))
        || (parent.is_export_spec(ast) && Some(n) != parent.get_first_child(ast))
}

impl<B: Behavior> Callback for CollectorCallback<'_, B> {
    fn traverse_block_children(
        &mut self,
        t: &mut NodeTraversal<'_>,
        block: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        (self.reuse_sites
            || (self.behavior.can_reuse_function_reference_sites()
                && parent.is_some_and(|p| p.is_function(t))))
            && self.externs_refer_only_to_externs != Some(true)
            && *REUSE_REFERENCE_SITES
            && block.has_children(t)
            && t.has_cached_source_input()
            && self.collect_body_from_sites(t, block)
    }

    /// Updates block stack.
    // port: ReferenceCollector.CollectorCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // When collectHoistedFunctionsInReferenceOrder is true, we automatically traverse a hoisted
        // function body when that function is first referenced, so that the reference lists are
        // in reference order.
        //
        // TODO(nicksantos): Maybe generalize this to a continuation mechanism
        // like in RemoveUnusedCode.
        #[allow(clippy::collapsible_if)] // Retain Java control flow.
        if self.collect_hoisted_functions_in_reference_order
            && NodeUtil::is_hoisted_function_declaration(node_traversal, n)
        {
            if !self.collected_hoisted_functions.insert(n) {
                return false;
            }
        }

        if n.is_function(node_traversal)
            && (parent.is_some() || node_traversal.get_scope_root().is_some())
            && self.narrow_scope.is_none()
            && self.externs_refer_only_to_externs != Some(true)
            && *REUSE_REFERENCE_SITES
            && node_traversal.has_cached_source_input()
        {
            if node_traversal.in_global_scope()
                && self
                    .behavior
                    .can_skip_global_function_if_no_pending_hoist(node_traversal.get_compiler(), n)
                && self.try_skip_global_function_if_no_pending_hoist(node_traversal, n, parent)
            {
                return false;
            }
            if self.behavior.use_inline_vars_clean_cache()
                && self.try_skip_clean_leaf_function(node_traversal, n, parent)
            {
                return false;
            }
            if self.behavior.infer_consts_leaf_fast_path()
                && self.try_infer_consts_leaf_function(node_traversal, n, parent)
            {
                return false;
            }
            if self
                .behavior
                .can_skip_function_with_no_needed_names(node_traversal, n)
                && self.try_skip_function_with_no_needed_names(node_traversal, n, parent)
            {
                return false;
            }
        }

        self.behavior
            .before_traverse_node(node_traversal, n, parent);

        // If node is a new basic block, put on basic block stack
        if BasicBlock::is_block_boundary(node_traversal, n, parent) {
            self.push_new_block(node_traversal, n);
        }
        true
    }

    /// For each node, update the block stack and reference collection as appropriate.
    // port: ReferenceCollector.CollectorCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let mut added_reference = None;
        if (n.is_name(t) || n.is_import_star(t))
            && self.needs_name(t, n)
            && !is_non_local_module_specifier_name(t, n, parent.unwrap())
        {
            let scope = t.get_scope();
            let v = scope.get_var_of_node(t.get_compiler(), n);

            // Rust-only (performance divergence): the summary of the externs (see
            // `skip_externs_when_unread`). The empty NAME of a function expression resolves to no
            // var.
            if self.externs_refer_only_to_externs == Some(true)
                && !v.is_some_and(|v| v.get_input(t.get_compiler()).is_some_and(|i| i.is_extern()))
                && t.get_input().is_none_or(|input| input.is_extern())
                && !n.get_string_ref(t).is_empty()
            {
                self.externs_refer_only_to_externs = Some(false);
            }

            if let Some(v) = v {
                added_reference = self.add_reference(v, n, t, scope);
                if self.collect_hoisted_functions_in_reference_order {
                    self.maybe_jump_to_hoisted_function(v, t, scope);
                }
            }
        }

        let added_reference = added_reference
            .map(|(entry_index, index)| &self.reference_map[entry_index].references[index]);
        self.behavior
            .after_visit_node(t, n, parent, added_reference);

        // Rust-only (performance divergence): a block boundary pushed in should_traverse must sit
        // at the top of block_stack on exit, avoiding a second is_block_boundary check per node.
        if self.block_stack.back().is_some_and(|b| b.get_root() == n)
            && BasicBlock::is_block_boundary(t, n, parent)
        {
            self.pop_last_block(t, n);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl<B: Behavior> ScopedCallback for CollectorCallback<'_, B> {
    /// Updates block stack and invokes any additional behavior.
    // port: ReferenceCollector.CollectorCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        // Don't add all ES6 scope roots to blockStack, only those that are also scopes according
        // to the ES5 scoping rules. Other nodes that ought to be considered the root of a
        // BasicBlock are added in shouldTraverse() or processScope() and removed in visit().
        if t.is_hoist_scope() {
            let root = t.get_scope_root().unwrap();
            self.push_new_block(t, root);
        }
    }

    /// Updates block stack and invokes any additional behavior.
    // port: ReferenceCollector.CollectorCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.is_hoist_scope() {
            let root = t.get_scope_root().unwrap();
            self.pop_last_block(t, root);
        }
        // Java passes the collector itself (`ReferenceCollector.this`, a ReferenceMap); here the
        // behavior is borrowed from the same object, so a view of its reference map stands in.
        let Self {
            behavior,
            reference_map,
            ..
        } = self;
        behavior.after_exit_scope(t, &CollectedReferenceMap { reference_map });
    }
}

/// Rust-only: the `ReferenceMap` view of the collector's references that its behavior gets in
/// `afterExitScope` (see `exit_scope`).
struct CollectedReferenceMap<'m> {
    reference_map: &'m IndexMap<VarId, Rc<ReferenceCollection>>,
}

impl ReferenceMap for CollectedReferenceMap<'_> {
    // port: ReferenceCollector#getReferences
    fn get_references(&self, var: VarId) -> Option<&ReferenceCollection> {
        self.reference_map.get(&var).map(Rc::as_ref)
    }

    fn shared_references(&self, var: VarId) -> Option<Rc<ReferenceCollection>> {
        self.reference_map.get(&var).cloned()
    }
}

/// Way for callers to add specific behavior during traversal that utilizes the built-up
/// reference information.
///
/// Note: These hooks are called in AST order only when the collector is made with
/// `collect_hoisted_functions_in_reference_order = false`
/// ([`ReferenceCollector::new_with_hoisted_function_order`]). When it is true (the default),
/// hoisted functions are traversed out-of-order upon first reference via
/// `maybe_jump_to_hoisted_function`:
///
/// - Hooks fire for the hoisted function and its body before `after_visit_node` fires for the
///   referencing `NAME` node.
/// - No hooks fire when the outer traversal later reaches the hoisted function declaration.
/// - The function declaration reference added for the hoisted function does not pass through
///   `after_visit_node`.
///
/// Behaviors that rely on strict AST or statement order (such as grouping references by top-level
/// statement) should ensure `collect_hoisted_functions_in_reference_order` is false.
pub trait Behavior {
    /// Whether this NAME needs to be resolved. A behavior that narrows lookup must
    /// retain every possible hoisted target and implicit slot as well as its own references.
    fn needs_name(&self, _ast: &Ast, _node: NodeId) -> bool {
        true
    }

    /// A behavior may select the resolved bindings it consumes and remember their scopes.
    /// Cached function bodies call this once per spelling instead of once per occurrence.
    fn needs_variable(&mut self, _compiler: &AbstractCompiler, _var: VarId) -> bool {
        true
    }

    /// Rust-only: opt in when the behavior observes only scope exits and its variable filter
    /// depends only on the binding and AST. ReferenceCollector may then reuse ordered sites in a
    /// single-scope function body and evaluate the filter once per spelling in that body.
    fn can_reuse_function_reference_sites(&self) -> bool {
        false
    }

    /// Rust-only (performance divergence): opt in for `InlineVariables` to skip unchanged clean
    /// leaf functions via `AbstractCompiler::inline_vars_clean_cache`.
    fn use_inline_vars_clean_cache(&self) -> bool {
        false
    }

    /// Rust-only (performance divergence): opt in for `InferConsts` to evaluate simple-param leaf
    /// functions directly from `FunctionBodySites` without allocating local scopes or references.
    fn infer_consts_leaf_fast_path(&self) -> bool {
        false
    }

    /// Rust-only (performance divergence): opt in when `fn_node` declares no variables needed by
    /// this behavior, allowing `ReferenceCollector` to skip entering `fn_node` if its parameters
    /// and cached `FunctionBodySites` contain no `needs_name` spellings.
    fn can_skip_function_with_no_needed_names(&self, _ast: &Ast, _fn_node: NodeId) -> bool {
        false
    }

    /// Rust-only (performance divergence): opt in for a behavior that ignores global variables
    /// when a top-level function `fn_node` cannot produce any local rewrites (e.g. contains no
    /// candidates or is cached clean), allowing `ReferenceCollector` to skip the entire subtree
    /// (leaf or non-leaf) if no pending hoisted global function is referenced inside it.
    fn can_skip_global_function_if_no_pending_hoist(
        &self,
        _compiler: &mut AbstractCompiler,
        _fn_node: NodeId,
    ) -> bool {
        false
    }

    /// Called before traversing a node.
    // port: ReferenceCollector.Behavior#beforeTraverseNode
    fn before_traverse_node(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) {
    }

    /// Called after visiting a node, passing any reference recorded for `n`.
    ///
    /// `added_reference` is set only if a [`Reference`] was recorded for `n`. If the variable for
    /// `n` was rejected by the `var_filter`, `added_reference` is `None`. See the trait
    /// documentation for execution order considerations when
    /// `collect_hoisted_functions_in_reference_order` is true.
    // port: ReferenceCollector.Behavior#afterVisitNode
    fn after_visit_node(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
        _added_reference: Option<&Reference>,
    ) {
    }

    /// Called after we finish with a scope.
    // port: ReferenceCollector.Behavior#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap);
}

// Rust-only forwarding so callers can lend a behavior they keep using afterwards.
impl<T: Behavior + ?Sized> Behavior for &mut T {
    fn needs_name(&self, ast: &Ast, node: NodeId) -> bool {
        (**self).needs_name(ast, node)
    }
    fn needs_variable(&mut self, compiler: &AbstractCompiler, var: VarId) -> bool {
        (**self).needs_variable(compiler, var)
    }
    fn before_traverse_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        (**self).before_traverse_node(t, n, parent);
    }
    fn after_visit_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
        added_reference: Option<&Reference>,
    ) {
        (**self).after_visit_node(t, n, parent, added_reference);
    }
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        (**self).after_exit_scope(t, reference_map);
    }
}

/// Java's anonymous `DO_NOTHING_BEHAVIOR`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DoNothingBehavior;

impl Behavior for DoNothingBehavior {
    // port: ReferenceCollector.DO_NOTHING_BEHAVIOR#afterExitScope
    fn after_exit_scope(&mut self, _t: &mut NodeTraversal<'_>, _reference_map: &dyn ReferenceMap) {}
}
