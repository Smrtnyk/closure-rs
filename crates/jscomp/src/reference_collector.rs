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

mod clean_subtree;
mod function_body;
mod leaf_screen;

pub use leaf_screen::LeafScreen;

pub(crate) use clean_subtree::{CacheOf, CleanSubtreeCache as KeptCleanSubtrees};
use clean_subtree::{CleanSubtreeCache, Frame, FreeSites, FunctionFrame};
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

/// `CLOSURE_RS_DELEGATING_SITES=off`: a block holding functions, classes, catch clauses or
/// destructuring is collected by the ordinary visitor (`FunctionBodySites::collect_delegating`).
static DELEGATING_SITES: LazyLock<bool> =
    LazyLock::new(|| std::env::var("CLOSURE_RS_DELEGATING_SITES").as_deref() != Ok("off"));

/// `CLOSURE_RS_REPLAY_JUMPS=off`: a function body whose names include a pending hoisted function is
/// collected by the ordinary visitor instead of `replay_body_sites_with_jumps`.
static REPLAY_JUMPS: LazyLock<bool> =
    LazyLock::new(|| std::env::var("CLOSURE_RS_REPLAY_JUMPS").as_deref() != Ok("off"));

/// `CLOSURE_RS_EXTERN_STUBS`: whether the scan that makes the summary of the externs without a
/// traversal (`try_verify_bundled_externs_refer_only_to_externs`) takes function stubs; `verify`
/// traverses the externs whenever it did and checks that the traversal makes the same summary.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExternStubs {
    Off,
    On,
    Verify,
}

static EXTERN_STUBS: LazyLock<ExternStubs> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_EXTERN_STUBS").as_deref() {
            Ok("off") => ExternStubs::Off,
            Ok("verify") => ExternStubs::Verify,
            _ => ExternStubs::On,
        },
    );

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
    /// Per spelling of a body replayed by `replay_scoped_body_sites`: whether its first site has
    /// been reached.
    body_names_seen: Vec<bool>,
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
    /// Rust-only (performance divergence): see `skip_bundled_extern_prefixes`.
    bundled_prefixes: BundledPrefixes,
    /// Rust-only (performance divergence): see `order_free_global_functions`.
    order_free_global_functions: bool,
    /// Rust-only (performance divergence): where the behavior keeps clean function subtrees
    /// (`clean_subtree`), unless `CLOSURE_RS_CLEAN_SUBTREES=off`.
    subtree_cache: Option<CacheOf>,
    /// Rust-only: the frames of the functions being traversed (`clean_subtree`).
    frames: Vec<Frame>,
    /// Rust-only: with `CLOSURE_RS_CLEAN_SUBTREES=verify`, a kept function about to be traversed
    /// anyway and its kept free references.
    verify_next: Option<(NodeId, std::sync::Arc<FreeSites>)>,
}

/// Rust-only (performance divergence): a free spelling of a kept clean function where it is skipped:
/// its variable, whether the filter accepts it, its entry in the reference map once made and
/// whether it may be a hoisted function the traversal jumps to (`try_skip_clean_subtree`).
type KeptBinding = (VarId, bool, Option<usize>, bool);

/// Rust-only (performance divergence): what a behavior's scope exit did, for the collector's
/// clean function subtrees (`Behavior::take_scope_exit_report`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ScopeExitReport {
    /// Whether the scope exit changed the tree.
    pub changed: bool,
    /// The depth of the least deep scope declaring a variable that the scope exit looked up by
    /// name, 0 when a name resolved to no variable.
    pub read_depth: Option<i32>,
}

/// Rust-only: whether the traversal leaves out the original statements of the bundled externs
/// (`ReferenceCollector::skip_bundled_extern_prefixes`).
enum BundledPrefixes {
    Traverse,
    Undecided,
    Skip(IndexSet<NodeId>),
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
        let subtree_cache = if *clean_subtree::MODE == clean_subtree::Mode::Off {
            None
        } else {
            behavior.clean_subtree_cache()
        };
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
                body_names_seen: Vec::new(),
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
                bundled_prefixes: BundledPrefixes::Traverse,
                order_free_global_functions: false,
                subtree_cache,
                frames: Vec::new(),
                verify_next: None,
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

    /// Rust-only (crates/DESIGN.md, "Performance divergences"): lets `process(externs, root)` leave
    /// out the original statements of the unchanged bundled extern scripts (Java traverses them),
    /// for a behavior that observes only scope exits and treats a var those statements declare
    /// as declared before every other reference (`Behavior::skip_bundled_extern_prefixes`). This
    /// is decided when the first extern script is reached. It needs every bundled script
    /// recorded, unchanged in syntax and names, ahead of every other script; every global name
    /// they use (`extern_referenced_names`) declared by one of them; and no other script
    /// declaring such a name. Their other names are parameters and function expression names
    /// bound in their own extern scopes, and their functions have no names in their bodies, so
    /// no other var has a reference in them. A behavior opting in must show, for the bundled
    /// externs alone, that their references report nothing.
    pub(crate) fn skip_bundled_extern_prefixes(&mut self) {
        self.callback.bundled_prefixes = BundledPrefixes::Undecided;
    }

    /// Rust-only (performance divergence): for a traversal whose behavior ignores global variables
    /// and whose scope exits change nothing (a dry run, see `InlineObjectLiterals`), so that only
    /// the references of local variables count and the order in which global functions are
    /// traversed does not matter. A reference to a hoisted global function does not jump to it
    /// (it is traversed at its declaration, as without `collectHoistedFunctionsInReferenceOrder`),
    /// and a global function the behavior can skip is skipped whatever it references
    /// (`Behavior::can_skip_global_function_if_no_pending_hoist`). The references of a variable
    /// declared in a function come from that function's subtree in the same order: jumps to the
    /// hoisted functions declared inside it are kept.
    pub(crate) fn order_free_global_functions(&mut self) {
        self.callback.order_free_global_functions = true;
    }

    /// Rust-only: the behavior, for a caller that wants it back after `process`.
    pub(crate) fn into_behavior(self) -> B {
        self.callback.behavior
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
            let mut scanned_only_externs = false;
            let cached_only_externs = match compiler.externs_reference_summary {
                Some(summary) if summary.externs_version == version => {
                    Some(summary.refers_only_to_externs)
                }
                _ => {
                    let scanned = try_verify_bundled_externs_refer_only_to_externs(
                        compiler,
                        &mut **scope_creator,
                        externs,
                    );
                    if scanned && *EXTERN_STUBS == ExternStubs::Verify {
                        // `CLOSURE_RS_EXTERN_STUBS=verify`: the traversal below makes the summary.
                        scanned_only_externs = true;
                        None
                    } else if scanned {
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
                    assert!(
                        !scanned_only_externs || refers_only_to_externs,
                        "CLOSURE_RS_EXTERN_STUBS=verify: the scan of the externs found that they \
                         refer only to themselves, their traversal found that they do not"
                    );
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
            // Rust-only (performance divergence): a function with plain parameters and an empty
            // body (the usual extern stub, `function F(a, b) {}` or `F.m = function(a) {};`)
            // is scanned too (Java traverses all externs). Its parameters are vars of its own
            // scope, declared in the externs; its body references nothing; the name of a
            // declaration (which sits in a script, as blocks end the scan) is a global name
            // checked like the others, the name of an expression a var of its own scope.
            // `CLOSURE_RS_EXTERN_STUBS=off` leaves such externs to the traversal, `=verify`
            // traverses them too and checks the summary it makes.
            Token::FUNCTION if *EXTERN_STUBS != ExternStubs::Off => {
                let name = n.get_first_child(ast).unwrap();
                let params = name.get_next(ast).unwrap();
                let body = params.get_next(ast).unwrap();
                let fn_name = name.get_string_ref(ast);
                if !body.is_block(ast)
                    || body.has_children(ast)
                    || params.children(ast).any(|p| {
                        !p.is_name(ast)
                            || p.has_children(ast)
                            || p.get_string_ref(ast) == fn_name
                            || p.get_string_ref(ast) == "arguments"
                    })
                {
                    return false;
                }
                if !fn_name.is_empty() && NodeUtil::is_function_declaration(ast, n) {
                    out.push(name);
                }
                return true;
            }
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
        if (self.order_free_global_functions && scope.is_global(compiler))
            || self
                .narrow_scope
                .is_some_and(|narrow| narrow.get_depth(compiler) > scope.get_depth(compiler))
            || self
                .skipped_externs
                .is_some_and(|externs| is_inside(compiler, function, externs))
        {
            return None;
        }
        // A skipped bundled script precedes every reference this traversal visits, so the full
        // traversal has collected its hoisted functions by then.
        if let BundledPrefixes::Skip(scripts) = &self.bundled_prefixes
            && NodeUtil::get_enclosing_script(compiler, function)
                .is_some_and(|script| scripts.contains(&script))
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

        // Rust-only (performance divergence): while the jumped-to function is traversed, only the
        // frames of the functions around its declaration see what happens (`clean_subtree`).
        let jump_frame = !self.frames.is_empty();
        if jump_frame {
            let depth = var_scope.get_depth(t.get_compiler());
            clean_subtree::note_jump(&mut self.frames, depth);
            self.frames.push(Frame::Jump(depth));
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
        if jump_frame {
            let frame = self.frames.pop();
            check_state!(matches!(frame, Some(Frame::Jump(_))));
        }
    }

    /// Rebind every spelling against this traversal's actual scope. A name of a pending hoisted
    /// function makes the replay jump to it at its first site, as the traversal does
    /// (`replay_body_sites_with_jumps`), for a behavior whose scope exits change only their own
    /// scopes. Otherwise a pending jump can invoke a mutating scope-exit callback in the middle of
    /// this body, and the whole body is collected normally, before adding any reference or
    /// changing the block stack (also with `CLOSURE_RS_REPLAY_JUMPS=off`).
    fn collect_body_from_sites(&mut self, t: &mut NodeTraversal<'_>, body: NodeId) -> bool {
        let body = match FunctionReferenceCache::sites(t.get_compiler(), body) {
            Some(sites) => sites,
            // Rust-only (performance divergence): a body with nested functions, classes, catch
            // clauses or destructuring is replayed with those nodes handed back to the traversal
            // (`FunctionBodySites::collect_delegating`). Disabled by
            // `CLOSURE_RS_DELEGATING_SITES=off`.
            None if *DELEGATING_SITES => {
                match FunctionReferenceCache::delegating_sites(t.get_compiler(), body) {
                    Some(sites) if sites.has_delegates => sites,
                    _ => return false,
                }
            }
            None => return false,
        };
        if body.first_names.is_empty() && !body.has_delegates {
            return true;
        }
        // A name declared in a nested scope is found by its spelling alone, which holds only
        // while every declared name is unique and no other name is spelled like it.
        if body.declares_in_nested_scopes
            && (body.nested_names_escape
                || !t.get_compiler().get_life_cycle_stage().is_normalized())
        {
            return false;
        }
        let mut scope = None;
        let mut pending_jump = false;
        self.body_bindings.clear();
        for &name in &body.first_names {
            t.position_at_node(name);
            if !self.needs_name(t, name) {
                self.body_bindings.push((None, None));
                continue;
            }
            let scope = *scope.get_or_insert_with(|| t.get_scope());
            let var = scope.get_var_of_node(t.get_compiler(), name);
            // A name declared in a nested scope is resolved at its first site.
            if var.is_none() && !body.declares_in_nested_scopes && !body.has_delegates {
                self.note_unresolved(name);
            }
            if self.collect_hoisted_functions_in_reference_order
                && var.is_some_and(|v| self.pending_hoisted_function(t.get_compiler(), v).is_some())
            {
                if !*REPLAY_JUMPS || !self.behavior.scope_exits_change_only_their_scopes() {
                    return false;
                }
                pending_jump = true;
            }
            self.body_bindings.push((var, None));
        }
        if body.declares_in_nested_scopes || body.has_delegates {
            self.replay_scoped_body_sites(t, &body, pending_jump);
            return true;
        }
        if pending_jump {
            let root_scope = scope.expect("a pending hoisted function was resolved");
            self.replay_body_sites_with_jumps(t, &body.sites, root_scope);
            return true;
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
        // The frames note the free references the filter rejects too (`clean_subtree`).
        if has_references || (!self.frames.is_empty() && scope.is_some()) {
            let root_scope = scope.expect("collected references have a scope");
            self.replay_body_sites(t, &body.sites, root_scope);
        }
        true
    }

    /// Rust-only (performance divergence): `replay_body_sites` for a body in which nested scopes
    /// declare `let`/`const` bindings (`FunctionBodySites::declares_in_nested_scopes`), in a
    /// normalized tree. The traversal is made to enter and leave every nested scope, so each
    /// scope is created with the parent the traversal would give it, when a reference in it or
    /// the behavior asks for it; a scope that declares bindings has its exit reported to the
    /// behavior where the traversal would report it. `self.body_bindings` holds the binding of
    /// every spelling visible from the body's own scope; a spelling without one is declared in
    /// a nested scope (or nowhere) and is resolved at its first site, from the scope there.
    /// Entries of the reference map are made at the first site of a spelling, as the traversal
    /// makes them.
    /// With `jumps`, a site that names a pending hoisted function jumps to it after adding the
    /// reference, as in `replay_body_sites_with_jumps`.
    fn replay_scoped_body_sites(
        &mut self,
        t: &mut NodeTraversal<'_>,
        body: &function_body::FunctionBodySites,
        jumps: bool,
    ) {
        self.replay_block_stack.clear();
        self.body_names_seen.clear();
        self.body_names_seen.resize(body.first_names.len(), false);
        let observe = self.behavior.observes_replayed_references();
        for site in &body.sites {
            match site.decode() {
                SiteKind::Reference(name, slot) => {
                    let slot = slot as usize;
                    t.position_at_node(name);
                    if !self.body_names_seen[slot] {
                        self.body_names_seen[slot] = true;
                        let mut var = self.body_bindings[slot].0;
                        if var.is_none() && self.needs_name(t, name) {
                            let scope = t.get_scope();
                            var = scope.get_var_of_node(t.get_compiler(), name);
                            if var.is_none() {
                                self.note_unresolved(name);
                            }
                        }
                        let index = var
                            .filter(|&v| {
                                (self.var_filter)(t.get_compiler(), v)
                                    && self.behavior.needs_variable(t.get_compiler(), v)
                            })
                            .map(|v| {
                                let entry = self.reference_map.entry(v);
                                let index = entry.index();
                                entry.or_default();
                                index
                            });
                        self.body_bindings[slot] = (var, index);
                    }
                    let (var, index) = self.body_bindings[slot];
                    if index.is_some() || (var.is_some() && !self.frames.is_empty()) {
                        let block = Self::materialize_replay_block(
                            t,
                            &self.block_stack,
                            &mut self.replay_block_stack,
                        );
                        if let Some(var) = var {
                            self.note_reference(t.get_compiler(), var, name, &block);
                        }
                    }
                    if let Some(index) = index {
                        let ref_scope = t.get_scope();
                        let block = Self::materialize_replay_block(
                            t,
                            &self.block_stack,
                            &mut self.replay_block_stack,
                        );
                        let references = Rc::make_mut(&mut self.reference_map[index]);
                        references.add(Reference::in_scope(name, t, block, ref_scope));
                        if observe {
                            let reference = references.references.last().expect("just added");
                            self.behavior.after_replayed_reference(t, reference);
                        }
                    }
                    if jumps
                        && let Some(var) = self.body_bindings[slot].0
                        && self
                            .pending_hoisted_function(t.get_compiler(), var)
                            .is_some()
                    {
                        let ref_scope = t.get_scope();
                        let bindings = std::mem::take(&mut self.body_bindings);
                        let seen = std::mem::take(&mut self.body_names_seen);
                        let blocks = std::mem::take(&mut self.replay_block_stack);
                        self.maybe_jump_to_hoisted_function(var, t, ref_scope);
                        self.body_bindings = bindings;
                        self.body_names_seen = seen;
                        self.replay_block_stack = blocks;
                    }
                }
                SiteKind::EnterBlock(n) => self.replay_block_stack.push((n, None)),
                SiteKind::ExitBlock(n) => {
                    let (root, _) = self.replay_block_stack.pop().expect("removeLast");
                    check_state!(root == n, &n.to_string(t));
                }
                SiteKind::EnterEmptyScope(n) | SiteKind::EnterScope(n) => {
                    t.push_scope_root_quietly(n);
                }
                SiteKind::ExitEmptyScope => t.pop_scope_root_quietly(),
                SiteKind::ExitScope => {
                    // What `exit_scope` does for a scope that is not a hoist scope.
                    let Self {
                        behavior,
                        reference_map,
                        ..
                    } = self;
                    behavior.after_exit_scope(t, &CollectedReferenceMap { reference_map });
                    if let Some(cache) = self.subtree_cache {
                        self.after_scope_exit_report(t, cache);
                    }
                    t.pop_scope_root_quietly();
                }
                SiteKind::Delegate(n) => {
                    // The traversal of the delegated node finds the scopes and basic blocks the
                    // traversal of the body would have entered around it, and may replay other
                    // bodies, so this replay's state is put aside.
                    let parent = n.get_parent(t).expect("a delegated node has a parent");
                    let depth = self.block_stack.len();
                    if !self.replay_block_stack.is_empty() {
                        Self::materialize_replay_block(
                            t,
                            &self.block_stack,
                            &mut self.replay_block_stack,
                        );
                        for (_, block) in &self.replay_block_stack {
                            self.block_stack
                                .push_back(block.clone().expect("materialized block"));
                        }
                    }
                    let bindings = std::mem::take(&mut self.body_bindings);
                    let seen = std::mem::take(&mut self.body_names_seen);
                    let blocks = std::mem::take(&mut self.replay_block_stack);
                    t.traverse_replayed_child(n, parent, self);
                    self.body_bindings = bindings;
                    self.body_names_seen = seen;
                    self.replay_block_stack = blocks;
                    self.block_stack.truncate(depth);
                }
            }
        }
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
        let observe = self.behavior.observes_replayed_references();
        for site in sites {
            match site.decode() {
                SiteKind::Reference(name, binding) => {
                    let (var, index) = self.body_bindings[binding as usize];
                    if index.is_none() && (var.is_none() || self.frames.is_empty()) {
                        continue;
                    }
                    t.position_at_node(name);
                    let block = Self::materialize_replay_block(
                        t,
                        &self.block_stack,
                        &mut self.replay_block_stack,
                    );
                    if let Some(var) = var {
                        self.note_reference(t.get_compiler(), var, name, &block);
                    }
                    if let Some(index) = index {
                        let ref_scope = if self.replay_scope_stack.is_empty() {
                            root_scope
                        } else {
                            Self::materialize_replay_scope(
                                t,
                                root_scope,
                                &mut self.replay_scope_stack,
                            )
                        };
                        let references = Rc::make_mut(&mut self.reference_map[index]);
                        references.add(Reference::in_scope(name, t, block, ref_scope));
                        if observe {
                            let reference = references.references.last().expect("just added");
                            self.behavior.after_replayed_reference(t, reference);
                        }
                    }
                }
                SiteKind::EnterBlock(n) => self.replay_block_stack.push((n, None)),
                SiteKind::ExitBlock(n) => {
                    let (root, _) = self.replay_block_stack.pop().expect("removeLast");
                    check_state!(root == n, &n.to_string(t));
                }
                SiteKind::EnterEmptyScope(n) | SiteKind::EnterScope(n) => {
                    self.replay_scope_stack.push((n, None));
                }
                SiteKind::ExitEmptyScope | SiteKind::ExitScope => {
                    self.replay_scope_stack.pop();
                }
                SiteKind::Delegate(_) => unreachable!("sites of a leaf body delegate nothing"),
            }
        }
    }

    /// Rust-only (performance divergence): `replay_body_sites` for a body (with one scope) in which
    /// a name resolves to a hoisted function the traversal has not collected yet. At each site
    /// that names a still pending one, after adding the reference, it jumps to the function as
    /// `visit` does (`maybe_jump_to_hoisted_function`), with its own state put aside, since the
    /// jump may replay other bodies. The jumped-to function is not in this body, which declares
    /// no function, and what its scope exits change lies inside it (for a behavior that opts in,
    /// `Behavior::scope_exits_change_only_their_scopes`), so the sites of this body stay
    /// valid. Entries of the reference map are made at the first site of a spelling, as the
    /// traversal makes them, so that the entries the jump makes keep their place among them.
    fn replay_body_sites_with_jumps(
        &mut self,
        t: &mut NodeTraversal<'_>,
        sites: &[Site],
        root_scope: ScopeId,
    ) {
        self.replay_scope_stack.clear();
        self.replay_block_stack.clear();
        self.body_names_seen.clear();
        self.body_names_seen.resize(self.body_bindings.len(), false);
        let observe = self.behavior.observes_replayed_references();
        for site in sites {
            match site.decode() {
                SiteKind::Reference(name, slot) => {
                    let slot = slot as usize;
                    let Some(var) = self.body_bindings[slot].0 else {
                        continue;
                    };
                    t.position_at_node(name);
                    if !self.body_names_seen[slot] {
                        self.body_names_seen[slot] = true;
                        if (self.var_filter)(t.get_compiler(), var)
                            && self.behavior.needs_variable(t.get_compiler(), var)
                        {
                            let entry = self.reference_map.entry(var);
                            self.body_bindings[slot].1 = Some(entry.index());
                            entry.or_default();
                        }
                    }
                    let ref_scope = if self.replay_scope_stack.is_empty() {
                        root_scope
                    } else {
                        Self::materialize_replay_scope(t, root_scope, &mut self.replay_scope_stack)
                    };
                    let index = self.body_bindings[slot].1;
                    if index.is_some() || !self.frames.is_empty() {
                        let block = Self::materialize_replay_block(
                            t,
                            &self.block_stack,
                            &mut self.replay_block_stack,
                        );
                        self.note_reference(t.get_compiler(), var, name, &block);
                    }
                    if let Some(index) = index {
                        let block = Self::materialize_replay_block(
                            t,
                            &self.block_stack,
                            &mut self.replay_block_stack,
                        );
                        let references = Rc::make_mut(&mut self.reference_map[index]);
                        references.add(Reference::in_scope(name, t, block, ref_scope));
                        if observe {
                            let reference = references.references.last().expect("just added");
                            self.behavior.after_replayed_reference(t, reference);
                        }
                    }
                    if self
                        .pending_hoisted_function(t.get_compiler(), var)
                        .is_some()
                    {
                        let bindings = std::mem::take(&mut self.body_bindings);
                        let seen = std::mem::take(&mut self.body_names_seen);
                        let scopes = std::mem::take(&mut self.replay_scope_stack);
                        let blocks = std::mem::take(&mut self.replay_block_stack);
                        self.maybe_jump_to_hoisted_function(var, t, ref_scope);
                        self.body_bindings = bindings;
                        self.body_names_seen = seen;
                        self.replay_scope_stack = scopes;
                        self.replay_block_stack = blocks;
                    }
                }
                SiteKind::EnterBlock(n) => self.replay_block_stack.push((n, None)),
                SiteKind::ExitBlock(n) => {
                    let (root, _) = self.replay_block_stack.pop().expect("removeLast");
                    check_state!(root == n, &n.to_string(t));
                }
                SiteKind::EnterEmptyScope(n) | SiteKind::EnterScope(n) => {
                    self.replay_scope_stack.push((n, None));
                }
                SiteKind::ExitEmptyScope | SiteKind::ExitScope => {
                    self.replay_scope_stack.pop();
                }
                SiteKind::Delegate(_) => unreachable!("sites of a leaf body delegate nothing"),
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
        // Rust-only (performance divergence): with a free name of a pending hoisted function, the
        // free sites are replayed with jumps (`replay_body_sites_with_jumps`); `jump_bindings`
        // holds the binding of every needed free name.
        let mut pending_jump = false;
        let mut jump_bindings = std::mem::take(&mut self.body_bindings);
        jump_bindings.clear();
        if let Some(free) = &free {
            for (slot, &name) in free.first_names.iter().enumerate() {
                t.position_at_node(name);
                if !self.needs_name(t, name) {
                    jump_bindings.push((None, None));
                    continue;
                }
                let var = enclosing_scope.get_var_of_node(t.get_compiler(), name);
                if var.is_none() {
                    self.note_unresolved(name);
                }
                jump_bindings.push((var, None));
                if self.collect_hoisted_functions_in_reference_order
                    && var.is_some_and(|v| {
                        self.pending_hoisted_function(t.get_compiler(), v).is_some()
                    })
                {
                    if !*REPLAY_JUMPS || !self.behavior.scope_exits_change_only_their_scopes() {
                        self.body_bindings = jump_bindings;
                        return false;
                    }
                    pending_jump = true;
                }
                if let Some(v) = var
                    && (self.var_filter)(t.get_compiler(), v)
                    && self.behavior.needs_variable(t.get_compiler(), v)
                {
                    self.free_vars_scratch.push((slot as u32, v));
                }
            }
        }
        self.body_bindings = jump_bindings;

        let is_fn_decl = parent.is_some() && NodeUtil::is_function_declaration(t, fn_node);
        let boundary = BasicBlock::is_block_boundary(t, fn_node, parent);
        let has_free_refs = !self.free_vars_scratch.is_empty() || pending_jump;
        if boundary && (is_fn_decl || has_free_refs) {
            self.push_new_block(t, fn_node);
        }
        if is_fn_decl {
            let fn_name = fn_node.get_first_child(t).unwrap();
            t.position_at_node(fn_name);
            if self.needs_name(t, fn_name) {
                match enclosing_scope.get_var_of_node(t.get_compiler(), fn_name) {
                    Some(v) => {
                        self.add_reference(v, fn_name, t, enclosing_scope);
                    }
                    None => self.note_unresolved(fn_name),
                }
            }
        }
        if pending_jump {
            let free = free.as_ref().expect("a pending jump implies free sites");
            let body = fn_node.get_last_child(t).expect("function body");
            let (compiler, scope_creator) = t.get_compiler_and_scope_creator();
            let fn_scope = scope_creator.create_scope(compiler, fn_node, Some(enclosing_scope));
            let body_scope = scope_creator.create_scope(compiler, body, Some(fn_scope));
            // The blocks of the function's scope and of its body's, both hoist scopes.
            self.push_new_block(t, fn_node);
            self.push_new_block(t, body);
            self.replay_body_sites_with_jumps(t, &free.sites, body_scope);
            self.pop_last_block(t, body);
            self.pop_last_block(t, fn_node);
        } else if has_free_refs {
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
            self.push_new_block(t, body);
            self.replay_body_sites(t, &free.sites, body_scope);
            self.pop_last_block(t, body);
            self.pop_last_block(t, fn_node);
        }
        if boundary && (is_fn_decl || has_free_refs) {
            self.pop_last_block(t, fn_node);
        }
        if !self.frames.is_empty() {
            clean_subtree::note_nested(&mut self.frames, fn_node, stamp, &[]);
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
        if body_sites.declares_in_nested_scopes {
            return false;
        }

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
                SiteKind::EnterEmptyScope(_)
                | SiteKind::ExitEmptyScope
                | SiteKind::EnterScope(_)
                | SiteKind::ExitScope => {}
                SiteKind::Delegate(_) => unreachable!("sites of a leaf body delegate nothing"),
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
                    if let Some(var) = self.body_bindings[slot as usize].0 {
                        self.note_reference(t.get_compiler(), var, name, &fn_block);
                    }
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
        if !self.collect_hoisted_functions_in_reference_order || self.order_free_global_functions {
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

    /// Rust-only (performance divergence): notes a reference to `var` in the frames of the
    /// functions being traversed that do not declare it (`clean_subtree`).
    #[inline]
    fn note_reference(
        &mut self,
        compiler: &AbstractCompiler,
        var: VarId,
        name: NodeId,
        block: &Rc<BasicBlock>,
    ) {
        if self.frames.is_empty() {
            return;
        }
        let depth = var.get_scope(compiler).get_depth(compiler);
        clean_subtree::note_reference(&mut self.frames, depth, name, block);
    }

    /// Rust-only (performance divergence): a name that resolved to no variable, which a scope
    /// around the functions being traversed may declare later (`clean_subtree`).
    #[inline]
    fn note_unresolved(&mut self, name: NodeId) {
        if !self.frames.is_empty() {
            clean_subtree::note_absent(&mut self.frames, name);
        }
    }

    /// Rust-only (performance divergence): a function skipped without noting its nested functions
    /// and free references: no function around it is kept (`clean_subtree`).
    fn note_unrecorded_skip(&mut self) {
        if !self.frames.is_empty() {
            clean_subtree::mark_dirty_deeper_than(&mut self.frames, i32::MIN, "unrecorded skip");
        }
    }

    /// Rust-only (performance divergence): starts the frame of `fn_node`, whose function scope the
    /// traversal enters (`clean_subtree`).
    fn push_function_frame(&mut self, t: &mut NodeTraversal<'_>, fn_node: NodeId) {
        let verify = match self.verify_next.take() {
            Some((kept, free)) if kept == fn_node => Some(free),
            other => {
                self.verify_next = other;
                None
            }
        };
        // Only a function of a known source input outside the externs, in a traversal of the
        // whole tree, may be kept.
        let dirty = (!t.has_cached_source_input()
            || self.narrow_scope.is_some()
            || fn_node.is_from_externs(t))
        .then_some("not keepable");
        self.frames.push(Frame::Function(FunctionFrame {
            fn_node,
            depth: t.get_scope_depth(),
            base: self.block_stack.back().expect("getLast").clone(),
            refs: Vec::new(),
            absent: Vec::new(),
            nested: Vec::new(),
            jumped: false,
            dirty,
            verify,
        }));
    }

    /// Rust-only (performance divergence): after the behavior's scope exit, marks the frames it
    /// makes dirty, and finishes the frame of a function whose function scope is left
    /// (`clean_subtree`).
    fn after_scope_exit_report(&mut self, t: &mut NodeTraversal<'_>, cache: CacheOf) {
        let report = self.behavior.take_scope_exit_report();
        if self.frames.is_empty() {
            return;
        }
        if report.changed {
            clean_subtree::mark_dirty_deeper_than(&mut self.frames, i32::MIN, "changed tree");
        } else if let Some(depth) = report.read_depth {
            clean_subtree::mark_dirty_deeper_than(&mut self.frames, depth, "outer lookup");
        }
        let root = t.get_scope_root().expect("a scope root");
        if root.is_function(t)
            && matches!(self.frames.last(), Some(Frame::Function(f)) if f.fn_node == root)
        {
            let Some(Frame::Function(frame)) = self.frames.pop() else {
                unreachable!("checked above");
            };
            let (stamp, nested) = clean_subtree::finish_frame(t.get_compiler(), cache, frame);
            clean_subtree::note_nested(&mut self.frames, root, stamp, &nested);
        }
    }

    /// Rust-only (performance divergence): per free spelling of a kept function, its variable in
    /// `enclosing_scope`, whether the filter accepts it, its entry in the reference map once made
    /// (none yet) and whether it may be a hoisted function the traversal jumps to; `None` when a
    /// spelling no longer resolves or an absent name now does (`try_skip_clean_subtree`).
    fn resolve_kept(
        &mut self,
        t: &mut NodeTraversal<'_>,
        enclosing_scope: ScopeId,
        free: &FreeSites,
    ) -> Option<Vec<KeptBinding>> {
        let mut bindings: Vec<KeptBinding> = Vec::with_capacity(free.first_names.len());
        for &name in &free.first_names {
            t.position_at_node(name);
            if !self.needs_name(t, name) {
                return None;
            }
            let var = enclosing_scope.get_var_of_node(t.get_compiler(), name)?;
            let may_jump = self.collect_hoisted_functions_in_reference_order
                && var
                    .get_parent_node(t.get_compiler())
                    .is_some_and(|f| NodeUtil::is_hoisted_function_declaration(t, f));
            bindings.push((var, false, None, may_jump));
        }
        for &name in &free.absent_names {
            if enclosing_scope
                .get_var_of_node(t.get_compiler(), name)
                .is_some()
            {
                return None;
            }
        }
        for binding in &mut bindings {
            binding.1 = (self.var_filter)(t.get_compiler(), binding.0)
                && self.behavior.needs_variable(t.get_compiler(), binding.0);
        }
        Some(bindings)
    }

    /// Rust-only (performance divergence): skips `fn_node` when it is kept as clean and unchanged
    /// (`clean_subtree`): adds its declaration's name and its free references as its traversal
    /// would, with the basic block around it if it is a block boundary.
    fn try_skip_clean_subtree(
        &mut self,
        t: &mut NodeTraversal<'_>,
        fn_node: NodeId,
        parent: Option<NodeId>,
        cache: CacheOf,
    ) -> bool {
        let kept = CleanSubtreeCache::current(t.get_compiler(), cache, fn_node).or_else(|| {
            // Rust-only (performance divergence): without jumps, a kept function of a behavior
            // whose traversal jumps serves too, if it was traversed without jumping inside it.
            self.behavior
                .clean_subtree_fallback()
                .filter(|_| !self.collect_hoisted_functions_in_reference_order)
                .and_then(|fallback| {
                    CleanSubtreeCache::current_jump_free(t.get_compiler(), fallback, fn_node)
                })
        });
        let kind = self.behavior.leaf_screen();
        if kept.is_none() && kind == LeafScreen::None {
            return false;
        }
        // Every spelling resolves in the scope around the function: no scope in it declares one.
        let enclosing_scope = t.get_scope();
        let mut bindings = kept
            .as_ref()
            .and_then(|(free, _)| self.resolve_kept(t, enclosing_scope, free));
        // Rust-only (performance divergence): a leaf function found clean without a traversal
        // (`leaf_screen`) is kept and skipped as one a traversal found clean; so is one whose kept
        // free references no longer resolve as they did (those of another behavior may have
        // resolved in other scopes).
        let mut kept = kept;
        if bindings.is_none()
            && kind != LeafScreen::None
            && leaf_screen::screen_and_keep(t.get_compiler(), cache, fn_node, enclosing_scope, kind)
        {
            kept = CleanSubtreeCache::current(t.get_compiler(), cache, fn_node);
            bindings = kept
                .as_ref()
                .and_then(|(free, _)| self.resolve_kept(t, enclosing_scope, free));
        }
        let (Some((free, nested)), Some(mut bindings)) = (kept, bindings) else {
            return false;
        };
        if *clean_subtree::MODE == clean_subtree::Mode::Verify {
            self.verify_next = Some((fn_node, free));
            return false;
        }

        // What `should_traverse` and `visit` tell the behavior about the function itself.
        self.behavior.before_traverse_node(t, fn_node, parent);
        let is_fn_decl = parent.is_some() && NodeUtil::is_function_declaration(t, fn_node);
        let boundary = BasicBlock::is_block_boundary(t, fn_node, parent);
        let has_free_refs = !free.sites.is_empty();
        if boundary && (is_fn_decl || has_free_refs) {
            self.push_new_block(t, fn_node);
        }
        let observe = self.behavior.observes_replayed_references();
        if is_fn_decl {
            // The function is collected (`should_traverse`), so its name jumps nowhere.
            let fn_name = fn_node.get_first_child(t).unwrap();
            t.position_at_node(fn_name);
            if self.needs_name(t, fn_name) {
                match enclosing_scope.get_var_of_node(t.get_compiler(), fn_name) {
                    Some(v) => {
                        if let Some((entry, index)) =
                            self.add_reference(v, fn_name, t, enclosing_scope)
                            && observe
                        {
                            let reference = &self.reference_map[entry].references[index];
                            self.behavior.after_replayed_reference(t, reference);
                        }
                    }
                    None => self.note_unresolved(fn_name),
                }
            }
        }
        // The names that resolve to nothing in the function are noted in the frames around it, as
        // its traversal notes them (the record of a function around it depends on them too).
        if !self.frames.is_empty() {
            for &name in &free.absent_names {
                self.note_unresolved(name);
            }
        }
        if has_free_refs {
            let scope = ScopeId::create_child_scope(t.get_compiler(), enclosing_scope, fn_node);
            let base = self.block_stack.back().expect("getLast").clone();
            let mut blocks: Vec<Option<Rc<BasicBlock>>> = vec![None; free.blocks.len()];
            for &(name, slot, block_index) in &free.sites {
                let (var, accepted, index, may_jump) = bindings[slot as usize];
                if accepted || !self.frames.is_empty() {
                    t.position_at_node(name);
                    let block = Self::materialize_kept_block(
                        t,
                        &free.blocks,
                        &mut blocks,
                        &base,
                        block_index,
                    );
                    self.note_reference(t.get_compiler(), var, name, &block);
                    if accepted {
                        let index = match index {
                            Some(index) => index,
                            None => {
                                let entry = self.reference_map.entry(var);
                                let index = entry.index();
                                entry.or_default();
                                bindings[slot as usize].2 = Some(index);
                                index
                            }
                        };
                        let references = Rc::make_mut(&mut self.reference_map[index]);
                        references.add(Reference::in_scope(name, t, block, scope));
                        if observe {
                            let reference = references.references.last().expect("just added");
                            self.behavior.after_replayed_reference(t, reference);
                        }
                    }
                }
                if may_jump {
                    t.position_at_node(name);
                    self.maybe_jump_to_hoisted_function(var, t, scope);
                }
            }
        }
        t.position_at_node(fn_node);
        self.behavior.after_visit_node(t, fn_node, parent, None);
        if boundary && (is_fn_decl || has_free_refs) {
            self.pop_last_block(t, fn_node);
        }
        if !self.frames.is_empty() {
            let stamp =
                SyntacticScopeCache::cached_subtree_stamp_with_flags(t.get_compiler(), fn_node);
            clean_subtree::note_nested(&mut self.frames, fn_node, stamp, &nested);
        }
        true
    }

    /// Rust-only (performance divergence): the basic block `index` of a kept function's block
    /// structure (`FreeSites::blocks`), made below `base` on first use.
    fn materialize_kept_block(
        ast: &Ast,
        kept: &[(NodeId, u32)],
        made: &mut [Option<Rc<BasicBlock>>],
        base: &Rc<BasicBlock>,
        index: u32,
    ) -> Rc<BasicBlock> {
        let Some(&(root, parent)) = kept.get(index as usize) else {
            return base.clone();
        };
        if let Some(block) = &made[index as usize] {
            return block.clone();
        }
        let parent = Self::materialize_kept_block(ast, kept, made, base, parent);
        let block = BasicBlock::new(ast, Some(parent), root);
        made[index as usize] = Some(block.clone());
        block
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
        // The frames note the free references the filter rejects too (`clean_subtree`).
        if !self.frames.is_empty() {
            let block = self.block_stack.back().expect("getLast").clone();
            self.note_reference(t.get_compiler(), v, n, &block);
        }
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

/// Rust-only (performance divergence): the bundled extern scripts that
/// `skip_bundled_extern_prefixes` may leave out, decided at the first script of the externs.
fn skippable_bundled_prefixes(
    t: &mut NodeTraversal<'_>,
    first: NodeId,
) -> Option<IndexSet<NodeId>> {
    use crate::abstract_scope::AbstractScope;
    use closure_rhino::node::SnapshotUse;

    let externs = first.get_parent(t)?;
    let compiler = t.get_compiler();
    if compiler.get_externs_root() != Some(externs)
        || externs.get_first_child(compiler) != Some(first)
    {
        return None;
    }
    let root = externs.get_next(compiler)?;
    let mut scripts = IndexSet::default();
    let mut names: Vec<&'static [&'static str]> = Vec::new();
    let mut others: Vec<NodeId> = Vec::new();
    let mut other_script = false;
    let mut child = Some(first);
    while let Some(script) = child {
        child = script.get_next(t);
        if !other_script
            && let Some((facts, suffix)) =
                t.bundled_script_prefix(script, SnapshotUse::SyntaxAndNames)
            && let Some(referenced) = facts.extern_referenced_names
        {
            scripts.insert(script);
            names.push(referenced);
            let mut statement = suffix;
            while let Some(s) = statement {
                others.push(s);
                statement = s.get_next(t);
            }
        } else if script.has_children(t) {
            other_script = true;
            others.push(script);
        }
        // An empty script (the synthetic externs at this point) has no references.
    }
    if scripts.is_empty() {
        return None;
    }
    let global = t.get_scope();
    let compiler = t.get_compiler();
    let declared_in_scripts = |name: &JsString| {
        global
            .scope_data(compiler)
            .vars
            .get(name)
            .copied()
            .and_then(|v| v.get_name_node(compiler))
            .and_then(|n| NodeUtil::get_enclosing_script(compiler, n))
            .is_some_and(|script| scripts.contains(&script))
    };
    let mut checked: IndexSet<&'static str> = IndexSet::default();
    for &name in names.iter().flat_map(|names| names.iter()) {
        if checked.insert(name) && !declared_in_scripts(&JsString::from(name)) {
            return None;
        }
    }
    others.extend(root.children(compiler));
    for n in others {
        if !n
            .get_first_child(compiler)
            .is_some_and(|c| c.is_module_body(compiler))
            && declares_any(compiler, n, &declared_in_scripts)
        {
            return None;
        }
    }
    Some(scripts)
}

/// Rust-only: whether a declaration outside functions and classes in `n` (any NAME it may bind in
/// the scope that holds `n`, or in a block or catch scope inside it) satisfies `declared`.
fn declares_any(ast: &Ast, n: NodeId, declared: &dyn Fn(&JsString) -> bool) -> bool {
    fn names_in(ast: &Ast, n: NodeId, declared: &dyn Fn(&JsString) -> bool) -> bool {
        (n.is_name(ast) && declared(n.get_string_ref(ast)))
            || n.children(ast).any(|c| names_in(ast, c, declared))
    }
    match n.get_token(ast) {
        Token::FUNCTION | Token::CLASS => {
            let name = n.get_first_child(ast).unwrap();
            name.is_name(ast) && declared(name.get_string_ref(ast))
        }
        Token::VAR | Token::LET | Token::CONST => n.children(ast).any(|target| {
            if target.is_name(ast) {
                declared(target.get_string_ref(ast))
                    || target.children(ast).any(|c| declares_any(ast, c, declared))
            } else {
                // A destructuring pattern and its initializer.
                target
                    .get_first_child(ast)
                    .is_some_and(|p| names_in(ast, p, declared))
                    || target
                        .children(ast)
                        .skip(1)
                        .any(|c| declares_any(ast, c, declared))
            }
        }),
        Token::CATCH => n.children(ast).enumerate().any(|(i, c)| {
            if i == 0 {
                names_in(ast, c, declared)
            } else {
                declares_any(ast, c, declared)
            }
        }),
        Token::IMPORT => names_in(ast, n, declared),
        Token::MODULE_BODY => false,
        _ => n.children(ast).any(|c| declares_any(ast, c, declared)),
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
    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if matches!(self.bundled_prefixes, BundledPrefixes::Undecided) {
            self.bundled_prefixes = match skippable_bundled_prefixes(t, script) {
                Some(scripts) => {
                    self.behavior.skip_bundled_extern_prefixes(&scripts);
                    BundledPrefixes::Skip(scripts)
                }
                None => BundledPrefixes::Traverse,
            };
        }
        if !self.should_traverse(t, script, parent) {
            return false;
        }
        if let BundledPrefixes::Skip(scripts) = &self.bundled_prefixes
            && scripts.contains(&script)
        {
            let (_, first) = t
                .bundled_script_prefix(script, closure_rhino::node::SnapshotUse::SyntaxAndNames)
                .expect("checked bundled script");
            t.traverse_script_suffix(script, parent, first, self);
            return false;
        }
        true
    }

    fn traverse_block_children(
        &mut self,
        t: &mut NodeTraversal<'_>,
        block: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // The summary of the externs (`externs_refer_only_to_externs`) reads only NAMEs in the
        // externs, which have no cached source input.
        (self.reuse_sites
            || (self.behavior.can_reuse_function_reference_sites()
                && parent.is_some_and(|p| p.is_function(t))))
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

        // Rust-only (performance divergence): the shortcuts below are taken only with a known
        // input. A traversal that starts at a function (the jump to a hoisted function,
        // `maybe_jump_to_hoisted_function`) has not looked up its input yet, nor has a script
        // whose names the behavior did not need so far (`Behavior::needs_name`). The lookup has
        // no other effect; without an input the ordinary visitor reports it at the first name,
        // as before. The externs have no cached source input, so a traversal that makes the summary
        // of the externs (`externs_refer_only_to_externs`), which reads only their NAMEs, takes the
        // shortcuts in the other scripts.
        if n.is_function(node_traversal)
            && (parent.is_some() || node_traversal.get_scope_root().is_some())
        {
            let _ = node_traversal.get_input();
        }
        if n.is_function(node_traversal)
            && (parent.is_some() || node_traversal.get_scope_root().is_some())
            && self.narrow_scope.is_none()
            && *REUSE_REFERENCE_SITES
            && node_traversal.has_cached_source_input()
        {
            if node_traversal.in_global_scope()
                && self
                    .behavior
                    .can_skip_global_function_if_no_pending_hoist(node_traversal.get_compiler(), n)
                && self.try_skip_global_function_if_no_pending_hoist(node_traversal, n, parent)
            {
                self.note_unrecorded_skip();
                return false;
            }
            if let Some(cache) = self.subtree_cache
                && self.try_skip_clean_subtree(node_traversal, n, parent, cache)
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
                self.note_unrecorded_skip();
                return false;
            }
            if self
                .behavior
                .can_skip_function_with_no_needed_names(node_traversal, n)
                && self.try_skip_function_with_no_needed_names(node_traversal, n, parent)
            {
                self.note_unrecorded_skip();
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
            } else if !n.get_string_ref(t).is_empty() {
                // The empty name of a function expression never resolves.
                self.note_unresolved(n);
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
            if self.subtree_cache.is_some() && root.is_function(t) {
                self.push_function_frame(t, root);
            }
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
        if let Some(cache) = self.subtree_cache {
            self.after_scope_exit_report(t, cache);
        }
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

    /// Rust-only (performance divergence): opt in when `after_exit_scope` changes the tree only
    /// inside the subtree of the scope's root, so that a replayed function body may jump to a
    /// pending hoisted function (`replay_body_sites_with_jumps`): the scope exits in that
    /// function then leave the rest of the body, whose sites were read before the jump, as it was.
    fn scope_exits_change_only_their_scopes(&self) -> bool {
        false
    }

    /// Rust-only (performance divergence): whether `after_replayed_reference` must be called for
    /// the references added from the cached sites of a function body. A behavior that reads
    /// `added_reference` in `after_visit_node` and opts into `can_reuse_function_reference_sites`
    /// needs it, because no node of a replayed body is visited.
    fn observes_replayed_references(&self) -> bool {
        false
    }
    /// Rust-only (performance divergence): called for each reference added from the cached sites
    /// of a function body, in the order `after_visit_node` would have received them.
    fn after_replayed_reference(&mut self, _t: &mut NodeTraversal<'_>, _reference: &Reference) {}
    /// Rust-only (performance divergence): opt in for `InlineVariables` to skip unchanged clean
    /// leaf functions via `AbstractCompiler::inline_vars_clean_cache`.
    fn use_inline_vars_clean_cache(&self) -> bool {
        false
    }

    /// Rust-only (performance divergence): where the collector keeps the clean functions of this
    /// behavior and skips them while they are unchanged (`clean_subtree`), for a behavior that
    /// observes only scope exits, needs every name and accepts every variable (in its filter and
    /// `needs_variable`), reads the scope of a reference to a variable declared outside a function
    /// in it only as `clean_subtree` describes, decides at a scope exit only from what lies in
    /// the scope's subtree, the references collected and the variables it looks up by name,
    /// and reports both in `take_scope_exit_report`.
    fn clean_subtree_cache(&self) -> Option<CacheOf> {
        None
    }

    /// Rust-only (performance divergence): for a behavior with `clean_subtree_cache` whose scope
    /// exits `leaf_screen::screen` knows, which: a leaf function the traversal reaches without a
    /// kept entry is then screened, and kept and skipped when found clean.
    fn leaf_screen(&self) -> LeafScreen {
        LeafScreen::None
    }

    /// Rust-only (performance divergence): for a behavior with `clean_subtree_cache` whose
    /// traversal makes no jumps to hoisted functions, the kept clean functions of another
    /// behavior whose traversal makes them, which this one may use as its own when they were
    /// traversed without a jump inside: their free references and block structure depend on the
    /// subtree alone, and the filter is applied when they are added.
    fn clean_subtree_fallback(&self) -> Option<CacheOf> {
        None
    }

    /// Rust-only (performance divergence): what the last `after_exit_scope` did, for a behavior
    /// with `clean_subtree_cache`.
    fn take_scope_exit_report(&mut self) -> ScopeExitReport {
        ScopeExitReport::default()
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

    /// Rust-only: the collector left out the original statements of these bundled extern scripts
    /// (`ReferenceCollector::skip_bundled_extern_prefixes`).
    fn skip_bundled_extern_prefixes(&mut self, _scripts: &IndexSet<NodeId>) {}

    /// Called after we finish with a scope.
    // port: ReferenceCollector.Behavior#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap);
}

// Rust-only forwarding so callers can lend a behavior they keep using afterwards.
impl<T: Behavior + ?Sized> Behavior for &mut T {
    fn needs_name(&self, ast: &Ast, node: NodeId) -> bool {
        (**self).needs_name(ast, node)
    }
    fn can_skip_function_with_no_needed_names(&self, ast: &Ast, fn_node: NodeId) -> bool {
        (**self).can_skip_function_with_no_needed_names(ast, fn_node)
    }
    fn can_skip_global_function_if_no_pending_hoist(
        &self,
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
    ) -> bool {
        (**self).can_skip_global_function_if_no_pending_hoist(compiler, fn_node)
    }
    fn needs_variable(&mut self, compiler: &AbstractCompiler, var: VarId) -> bool {
        (**self).needs_variable(compiler, var)
    }
    fn can_reuse_function_reference_sites(&self) -> bool {
        (**self).can_reuse_function_reference_sites()
    }
    fn clean_subtree_cache(&self) -> Option<CacheOf> {
        (**self).clean_subtree_cache()
    }
    fn clean_subtree_fallback(&self) -> Option<CacheOf> {
        (**self).clean_subtree_fallback()
    }
    fn leaf_screen(&self) -> LeafScreen {
        (**self).leaf_screen()
    }
    fn take_scope_exit_report(&mut self) -> ScopeExitReport {
        (**self).take_scope_exit_report()
    }
    fn scope_exits_change_only_their_scopes(&self) -> bool {
        (**self).scope_exits_change_only_their_scopes()
    }
    fn observes_replayed_references(&self) -> bool {
        (**self).observes_replayed_references()
    }
    fn after_replayed_reference(&mut self, t: &mut NodeTraversal<'_>, reference: &Reference) {
        (**self).after_replayed_reference(t, reference);
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
