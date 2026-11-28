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
    accessor_summary::AccessorSummary,
    compiler_pass::CompilerPass,
    node_traversal::NodeTraversal,
    node_util::NodeUtil,
    platform::Platform,
    syntactic_scope_cache::SyntacticScopeCache,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, LazyLock},
};

pub struct PeepholeOptimizationsPass {
    pass_name: String,
    // NOTE: Use a native array rather than a List to avoid creating iterators for every node in
    // the AST.
    peephole_optimizations: Vec<Box<dyn AbstractPeepholeOptimization>>,
    retraverse_on_change: bool,
    candidates: Option<Box<CandidateRules>>,
    fixpoint_rules: Option<Box<FixpointRules>>,
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
        let fixpoint_rules = FixpointRules::new(&optimizations);
        Self {
            candidates,
            fixpoint_rules,
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
            let fixpoint = match self.fixpoint_rules.as_deref() {
                Some(rules) => FixpointRun::start(compiler, rules, &self.pass_name),
                None => FixpointRun::default(),
            };
            let mut walker = PeepWalker {
                candidates: self.candidates.as_deref(),
                peephole_optimizations: &mut self.peephole_optimizations,
                has_pending_features: false,
                last_script: None,
                last_node: None,
                fixpoint,
            };
            let result = catch_unwind(AssertUnwindSafe(|| match &changed_scope_nodes {
                None => {
                    if walker.fixpoint.active {
                        walker.traverse_full::<true>(compiler, root, None, false);
                    } else {
                        walker.traverse_full::<false>(compiler, root, None, false);
                    }
                    walker.flush_features(compiler, None);
                }
                Some(nodes) => {
                    for &scope_node in nodes {
                        let script = find_enclosing_script(compiler, scope_node);
                        if walker.fixpoint.active {
                            walker.traverse_scope_root::<true>(
                                compiler, scope_node, scope_node, script, false,
                            );
                        } else {
                            walker.traverse_scope_root::<false>(
                                compiler, scope_node, scope_node, script, false,
                            );
                        }
                        walker.flush_features(compiler, script);
                    }
                }
            }));
            let (last_script, last_node) = (walker.last_script, walker.last_node);
            walker
                .fixpoint
                .finish(compiler, &self.pass_name, changed_scope_nodes.is_none());
            if let Err(unexpected_exception) = result {
                NodeTraversal::throw_unexpected_exception_at(
                    compiler,
                    last_script,
                    last_node,
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

/// Rust-only (performance divergence): `CLOSURE_RS_PEEPHOLE_FIXPOINT=off` keeps no
/// `PeepholeFixpoint` records (every statement is traversed, as in Java); `=verify` takes the same
/// decisions as the default but also traverses each statement it skips and panics if that changes
/// anything. `CLOSURE_RS_PEEPHOLE_FIXPOINT_STATS=1` prints per traversal how many statements were
/// skipped.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FixpointMode {
    Off,
    On,
    Verify,
}

static FIXPOINT_MODE: LazyLock<FixpointMode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_PEEPHOLE_FIXPOINT").as_deref() {
            Ok("off") => FixpointMode::Off,
            Ok("verify") => FixpointMode::Verify,
            _ => FixpointMode::On,
        },
    );

static FIXPOINT_STATS: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("CLOSURE_RS_PEEPHOLE_FIXPOINT_STATS").is_ok_and(|value| value != "0")
});

/// Rust-only (performance divergence): Java traverses every changed function again in each round
/// of the pass's fixed point loop and in each run of the pass, although most of its statements are
/// as they were when the last traversal changed nothing in them. A traversal records each
/// statement (a child of a BLOCK, SCRIPT or MODULE_BODY) whose visit, of the statement and of every
/// node below it, changed nothing; a later traversal of the same optimizations skips the
/// statement while it is unchanged, since each visit would read the same tree and change nothing
/// again (`AbstractPeepholeOptimization::fixpoint_context_tokens` states what the optimizations may
/// read; a skipped visit has no effect on anything else: no change report, no error, no feature,
/// no name).
///
/// A statement is unchanged while neither it nor a node below it in its change scope changed
/// since the record: the stamps of `SyntacticScopeCache` cover edits of the tree and, with
/// `Ast::track_data_changes`, every in-place change of node data a visit can read. A record is not
/// made for a statement that holds a nested function (whose body is another change scope, and
/// which `NodeUtil::is_equivalent_to`, the cost estimates, `references_own_receiver` and the
/// forward references of CollectPropertyAssignments read) or a node below it with a token an
/// optimization reads context at; a statement that has such a token itself is recorded with
/// `MARK_REVISIT`: when it is skipped, its own visit is done again (in the order of Java's
/// traversal it comes after the visits below it, which change nothing). Everything else a visit
/// reads is keyed per traversal (`FixpointContext`): the optimizations, the life cycle stage, the
/// accessor summary, `has_reg_exp_global_references`, the options the optimizations read, and the
/// stamp resets; the features of the script an optimization reads are part of each clock.
/// A statement is skipped only when no features wait to be added to the script: Java adds them
/// at the next scope exit, which may lie inside the statement.
#[derive(Default)]
pub(crate) struct PeepholeFixpoint {
    /// Per arena index of a statement: 0, or 1 + the index in `clocks` of the clock at the end of
    /// its last visit that changed nothing, plus `MARK_TRANSFERABLE` when that visit took no
    /// path of `peephole_normalized_reads`, plus `MARK_REVISIT` when the statement itself has a
    /// token of `fixpoint_context_tokens` (no node below it has).
    marks: StatementTable<u32>,
    clocks: Vec<FixpointClock>,
    contexts: Vec<FixpointContext>,
}

impl PeepholeFixpoint {
    /// The index of `context` in `contexts`, added if new.
    fn context_index(&mut self, context: FixpointContext) -> u32 {
        if let Some(index) = self.contexts.iter().rposition(|known| *known == context) {
            return index as u32;
        }
        if self.contexts.len() >= MAX_FIXPOINT_CONTEXTS || self.clocks.len() >= MAX_FIXPOINT_CLOCKS
        {
            *self = Self::default();
        }
        self.contexts.push(context);
        (self.contexts.len() - 1) as u32
    }
}

/// Rust-only (performance divergence): a value per statement, by arena index, in pages of
/// `STATEMENT_PAGE` indexes allocated when a value is first set in them: only the pages of the
/// user code's statements, not those of the bundled externs the peephole passes never visit, and
/// no copy of the whole table when it grows. An unset index reads as the default.
#[derive(Default)]
pub(crate) struct StatementTable<T> {
    pages: Vec<Option<Box<[T; STATEMENT_PAGE]>>>,
}

const STATEMENT_PAGE_BITS: u32 = 10;
const STATEMENT_PAGE: usize = 1 << STATEMENT_PAGE_BITS;

impl<T: Copy + Default + PartialEq> StatementTable<T> {
    #[inline]
    pub(crate) fn get(&self, n: NodeId) -> T {
        let index = n.arena_index();
        match self.pages.get(index >> STATEMENT_PAGE_BITS) {
            Some(Some(page)) => page[index & (STATEMENT_PAGE - 1)],
            _ => T::default(),
        }
    }

    #[inline]
    pub(crate) fn set(&mut self, n: NodeId, value: T) {
        let index = n.arena_index();
        let page = index >> STATEMENT_PAGE_BITS;
        if let Some(Some(values)) = self.pages.get_mut(page) {
            values[index & (STATEMENT_PAGE - 1)] = value;
        } else if value != T::default() {
            self.set_in_new_page(page, index & (STATEMENT_PAGE - 1), value);
        }
    }

    #[cold]
    fn set_in_new_page(&mut self, page: usize, offset: usize, value: T) {
        if self.pages.len() <= page {
            self.pages.resize_with(page + 1, || None);
        }
        let mut values = Box::new([T::default(); STATEMENT_PAGE]);
        values[offset] = value;
        self.pages[page] = Some(values);
    }

    pub(crate) fn clear(&mut self) {
        self.pages.clear();
    }
}

/// The most contexts and clocks `PeepholeFixpoint` keeps before it starts over.
const MAX_FIXPOINT_CONTEXTS: usize = 1 << 10;
const MAX_FIXPOINT_CLOCKS: usize = 1 << 20;

/// The bit of a mark that says its record also holds for a later context of the same pass on the
/// denormalized AST (`FixpointContext::donates_to`).
const MARK_TRANSFERABLE: u32 = 1 << 31;

/// The bit of a mark that says the statement itself must be visited again when it is skipped:
/// its own visit may read beyond it, the visits below it do not.
const MARK_REVISIT: u32 = 1 << 30;

/// The bits of a mark that hold the clock.
const MARK_CLOCK: u32 = MARK_REVISIT - 1;

/// A point of the `SyntacticScopeCache` change clock in a traversal.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FixpointClock {
    context: u32,
    /// `FixpointRun::features` of the statement's script.
    features: u32,
    round: u32,
    flag_round: u32,
}

/// What, besides the tree, the visits of a traversal may read.
#[derive(Clone)]
struct FixpointContext {
    pass_name: String,
    optimizations: Vec<&'static str>,
    normalized: bool,
    accessor_summary: Option<Arc<AccessorSummary>>,
    reg_exp_global_references: bool,
    use_types_for_local_optimization: bool,
    assume_getters_are_pure: bool,
    assume_properties_are_statically_analyzable: bool,
    output_es5: bool,
    typed_node_equality: bool,
    resets: u32,
}

impl FixpointContext {
    /// Whether everything but the AST normalization and the optimizations is the same.
    fn same_reads(&self, other: &Self) -> bool {
        self.pass_name == other.pass_name
            // `AstAnalyzer::get_property_kind` asks the accessor summary only when getters are
            // not assumed pure; a summary gathered again is a new one, mostly with the same
            // content.
            && (self.assume_getters_are_pure
                || match (&self.accessor_summary, &other.accessor_summary) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b) || **a == **b,
                    _ => false,
                })
            && self.reg_exp_global_references == other.reg_exp_global_references
            && self.use_types_for_local_optimization == other.use_types_for_local_optimization
            && self.assume_getters_are_pure == other.assume_getters_are_pure
            && self.assume_properties_are_statically_analyzable
                == other.assume_properties_are_statically_analyzable
            && self.output_es5 == other.output_es5
            && self.typed_node_equality == other.typed_node_equality
            && self.resets == other.resets
    }

    /// Whether a statement whose visit changed nothing in `self`, without a path that bumps
    /// `peephole_normalized_reads`, changes nothing in `later` either: `later` is the same pass on
    /// the denormalized AST with some of the optimizations of `self`, all else equal. On a
    /// denormalized AST each of them does what it does on a normalized one or less
    /// (`is_ast_normalized` only allows changes), but for that one path; and the tokens `later`
    /// must not record are among those `self` must not record.
    fn donates_to(&self, later: &Self) -> bool {
        self.normalized
            && !later.normalized
            && later
                .optimizations
                .iter()
                .all(|name| self.optimizations.contains(name))
            && self.same_reads(later)
    }
}

impl PartialEq for FixpointContext {
    fn eq(&self, other: &Self) -> bool {
        self.optimizations == other.optimizations
            && self.normalized == other.normalized
            && self.same_reads(other)
    }
}

/// What a pass's optimizations say about `PeepholeFixpoint` (`None` when one of them was not
/// audited for it).
struct FixpointRules {
    /// Per token, 1 for those a record must not hold: FUNCTION and every
    /// `fixpoint_context_tokens`.
    sensitive: [u8; 256],
    /// Every `script_features_read`.
    script_features: Vec<Feature>,
    names: Vec<&'static str>,
}

impl FixpointRules {
    fn new(optimizations: &[Box<dyn AbstractPeepholeOptimization>]) -> Option<Box<Self>> {
        if *FIXPOINT_MODE == FixpointMode::Off {
            return None;
        }
        let mut rules = Box::new(Self {
            sensitive: [0; 256],
            script_features: Vec::new(),
            names: Vec::with_capacity(optimizations.len()),
        });
        rules.add_sensitive(Token::FUNCTION);
        for optimization in optimizations {
            for &token in optimization.fixpoint_context_tokens()? {
                rules.add_sensitive(token);
            }
            for &feature in optimization.script_features_read() {
                if !rules.script_features.contains(&feature) {
                    rules.script_features.push(feature);
                }
            }
            rules.names.push(optimization.get_class_name());
        }
        if rules.script_features.len() >= 31 {
            return None;
        }
        Some(rules)
    }

    fn add_sensitive(&mut self, token: Token) {
        const { assert!((Token::PLACEHOLDER3 as usize) < 256) };
        self.sensitive[token as usize] = 1;
    }
}

/// The state at the start of the visit of a statement: change stamp, errors reported, sensitive
/// tokens met, `peephole_normalized_reads`.
type FixpointBefore = (i32, u64, u64, u64);

/// What a traversal does with a node, for `PeepholeFixpoint`.
#[derive(Clone, Copy)]
enum FixpointEntry {
    /// Traverse it; it is no statement, or nothing is recorded now.
    Plain,
    /// Traverse it and record whether its visit changed nothing.
    Record(FixpointBefore),
    /// Skip it.
    Skip,
    /// Skip what is below it, then visit it.
    Revisit,
    /// `verify`: traverse what would be skipped, and check that it changes nothing (the
    /// statement's mark).
    Verify(FixpointBefore, u32),
}

/// `PeepholeFixpoint` during one traversal (inactive, the default, for a pass that keeps no
/// records).
struct FixpointRun {
    active: bool,
    records: PeepholeFixpoint,
    context: u32,
    /// Per context, whether its transferable records hold in `context`.
    donors: Vec<bool>,
    resets: u32,
    sensitive: [u8; 256],
    script_features: Vec<Feature>,
    verify: bool,
    /// Above zero while a skipped statement is traversed for `verify`: nothing is decided or
    /// recorded then.
    verifying: u32,
    /// How many tokens of `sensitive` the traversal met.
    sensitive_met: u64,
    /// A script and its `features`, while known.
    features_of: Option<(Option<NodeId>, u32)>,
    checked: u64,
    skipped: u64,
    recorded: u64,
}

impl Default for FixpointRun {
    fn default() -> Self {
        Self {
            active: false,
            records: PeepholeFixpoint::default(),
            context: 0,
            donors: Vec::new(),
            resets: 0,
            sensitive: [0; 256],
            script_features: Vec::new(),
            verify: false,
            verifying: 0,
            sensitive_met: 0,
            features_of: None,
            checked: 0,
            skipped: 0,
            recorded: 0,
        }
    }
}

impl FixpointRun {
    fn start(compiler: &mut AbstractCompiler, rules: &FixpointRules, pass_name: &str) -> Self {
        compiler.ast.track_data_changes();
        // Edits made since the last drain would otherwise get rounds after the clocks this
        // traversal records, and void its records needlessly.
        SyntacticScopeCache::drain_changes(compiler);
        let (_, _, resets) = SyntacticScopeCache::change_clock(compiler);
        let analyzer = compiler.get_ast_analyzer();
        compiler.statement_side_effects.begin(
            analyzer,
            resets,
            *FIXPOINT_MODE == FixpointMode::Verify,
        );
        let options = compiler.get_options();
        let context = FixpointContext {
            pass_name: pass_name.to_string(),
            optimizations: rules.names.clone(),
            normalized: compiler.get_life_cycle_stage().is_normalized(),
            accessor_summary: compiler.get_accessor_summary().cloned(),
            reg_exp_global_references: compiler.has_reg_exp_global_references(),
            use_types_for_local_optimization: options.should_use_types_for_local_optimization(),
            assume_getters_are_pure: options.get_assume_getters_are_pure(),
            assume_properties_are_statically_analyzable: options
                .get_assume_properties_are_statically_analyzable(),
            output_es5: options.get_output_feature_set().contains(FeatureSet::ES5),
            typed_node_equality: options.should_ambiguate_properties()
                || options.should_disambiguate_properties(),
            resets,
        };
        let mut records = std::mem::take(&mut compiler.peephole_fixpoint);
        let index = records.context_index(context);
        let current = &records.contexts[index as usize];
        let donors = records
            .contexts
            .iter()
            .map(|earlier| earlier.donates_to(current))
            .collect();
        Self {
            active: true,
            records,
            context: index,
            donors,
            resets,
            sensitive: rules.sensitive,
            script_features: rules.script_features.clone(),
            verify: *FIXPOINT_MODE == FixpointMode::Verify,
            verifying: 0,
            sensitive_met: 0,
            features_of: None,
            checked: 0,
            skipped: 0,
            recorded: 0,
        }
    }

    fn finish(self, compiler: &mut AbstractCompiler, pass_name: &str, full: bool) {
        if !self.active {
            return;
        }
        let (kept, computed) = compiler.statement_side_effects.end();
        if *FIXPOINT_STATS {
            eprintln!(
                "peephole fixpoint: {pass_name} {}: statements checked {}, skipped {}, recorded \
                 {}; side effects of block statements kept {kept}, computed {computed}",
                if full {
                    "full traversal"
                } else {
                    "changed scopes"
                },
                self.checked,
                self.skipped,
                self.recorded
            );
        }
        compiler.peephole_fixpoint = self.records;
    }

    /// `PeepWalker::traverse_full` reaches a node with `token`.
    #[inline]
    fn enter_full(
        &mut self,
        compiler: &mut AbstractCompiler,
        token: Token,
        in_block: bool,
    ) -> FixpointEntry {
        let entry = if in_block && self.verifying == 0 {
            FixpointEntry::Record(self.before(compiler))
        } else {
            FixpointEntry::Plain
        };
        self.note_token(token);
        entry
    }

    /// `PeepWalker::traverse_scope_root` reaches the node `n` with `token`.
    #[inline]
    fn enter(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        token: Token,
        script: Option<NodeId>,
        in_block: bool,
        pending_features: bool,
    ) -> FixpointEntry {
        if !in_block || self.verifying != 0 {
            self.note_token(token);
            return FixpointEntry::Plain;
        }
        if !pending_features && self.holds(compiler, n, script) {
            self.skipped += 1;
            let mark = self.mark(n);
            let revisit = mark & MARK_REVISIT != 0;
            if revisit {
                // For the statement around this one.
                self.note_token(token);
            }
            if !self.verify {
                return if revisit {
                    FixpointEntry::Revisit
                } else {
                    FixpointEntry::Skip
                };
            }
            let before = self.before(compiler);
            self.verifying += 1;
            compiler.statement_side_effects.set_suspended(true);
            return FixpointEntry::Verify(before, mark);
        }
        let before = self.before(compiler);
        self.note_token(token);
        FixpointEntry::Record(before)
    }

    #[inline]
    fn note_token(&mut self, token: Token) {
        self.sensitive_met += u64::from(self.sensitive[token as usize & 0xff]);
    }

    #[inline]
    fn before(&self, compiler: &mut AbstractCompiler) -> FixpointBefore {
        (
            compiler.get_change_tracker().get_change_stamp(),
            compiler.reports_made(),
            self.sensitive_met,
            compiler.peephole_normalized_reads,
        )
    }

    /// Of the features of `script` the optimizations read, which it has (bit `i` for
    /// `script_features[i]`), and whether it has features at all (bit 31).
    fn features(&mut self, compiler: &AbstractCompiler, script: Option<NodeId>) -> u32 {
        if self.script_features.is_empty() {
            return 0;
        }
        match self.features_of {
            Some((known, features)) if known == script => features,
            _ => {
                let mut features = 0;
                if let Some(set) =
                    script.and_then(|script| NodeUtil::get_feature_set_of_script(compiler, script))
                {
                    features |= 1 << 31;
                    for (i, &feature) in self.script_features.iter().enumerate() {
                        if set.has(feature) {
                            features |= 1 << i;
                        }
                    }
                }
                self.features_of = Some((script, features));
                features
            }
        }
    }

    #[inline]
    fn mark(&self, n: NodeId) -> u32 {
        self.records.marks.get(n)
    }

    /// Whether the statement `n` was recorded in this context and is unchanged since.
    #[inline]
    fn holds(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        script: Option<NodeId>,
    ) -> bool {
        self.checked += 1;
        let mark = self.mark(n);
        if mark == 0 {
            return false;
        }
        let clock = self.records.clocks[(mark & MARK_CLOCK) as usize - 1];
        (clock.context == self.context
            || (mark & MARK_TRANSFERABLE != 0 && self.donors[clock.context as usize]))
            && clock.features == self.features(compiler, script)
            && SyntacticScopeCache::unchanged_since(
                compiler,
                n,
                (clock.round, clock.flag_round, self.resets),
            )
    }

    /// Records whether the visit of the statement `n` changed nothing.
    #[inline]
    fn record(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        script: Option<NodeId>,
        unchanged: bool,
        transferable: bool,
        revisit: bool,
    ) {
        let mark = if unchanged {
            self.recorded += 1;
            // Without draining: an edit still pending (none is in `n`, whose visit changed
            // nothing, unless an earlier one) gets a later round and so voids the record.
            let (round, flag_round, _) = SyntacticScopeCache::change_clock(compiler);
            let clock = FixpointClock {
                context: self.context,
                features: self.features(compiler, script),
                round,
                flag_round,
            };
            if self.records.clocks.last() != Some(&clock) {
                self.records.clocks.push(clock);
            }
            self.records.clocks.len() as u32
                | if transferable { MARK_TRANSFERABLE } else { 0 }
                | if revisit { MARK_REVISIT } else { 0 }
        } else {
            0
        };
        self.records.marks.set(n, mark);
    }

    /// `CLOSURE_RS_PEEPHOLE_FIXPOINT=verify`: the statement `n` the traversal would have skipped
    /// was traversed; it must have changed nothing.
    fn verify_unchanged(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mark: u32,
        before: FixpointBefore,
        pending_features: bool,
    ) {
        let after = self.before(compiler);
        let clock = self.records.clocks[(mark & MARK_CLOCK) as usize - 1];
        let tree_unchanged = SyntacticScopeCache::unchanged_since(
            compiler,
            n,
            (clock.round, clock.flag_round, self.resets),
        );
        if after.0 != before.0 || after.1 != before.1 || pending_features || !tree_unchanged {
            panic!(
                "CLOSURE_RS_PEEPHOLE_FIXPOINT=verify: a skipped statement changed when traversed: \
                 change stamp {} -> {}, errors reported {} -> {}, features pending {}, tree \
                 unchanged since the record {}, statement {}",
                before.0,
                after.0,
                before.1,
                after.1,
                pending_features,
                tree_unchanged,
                n.to_string_tree(compiler)
            );
        }
    }
}

#[inline]
fn is_statement_container(token: Token) -> bool {
    matches!(token, Token::BLOCK | Token::SCRIPT | Token::MODULE_BODY)
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
    fixpoint: FixpointRun,
}

impl PeepWalker<'_> {
    /// `FIX`: whether the traversal keeps `PeepholeFixpoint` records (`self.fixpoint.active`);
    /// `in_block` then says whether the parent of `n` is a BLOCK, SCRIPT or MODULE_BODY, so that
    /// `n` is a statement.
    fn traverse_full<const FIX: bool>(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        current_script: Option<NodeId>,
        in_block: bool,
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
        let entry = if FIX {
            self.fixpoint.enter_full(compiler, token, in_block)
        } else {
            FixpointEntry::Plain
        };
        let child_in_block = FIX && is_statement_container(token);
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            let next = c.get_next(compiler);
            self.traverse_full::<FIX>(compiler, c, script, child_in_block);
            child = next;
        }
        if self.has_pending_features && is_scope_exit_token(token) {
            self.flush_features(compiler, script);
        }
        let current_token = n.get_token(compiler);
        self.visit_node(compiler, n, current_token, script);
        if FIX && let FixpointEntry::Record(before) = entry {
            self.fixpoint_record(compiler, n, token, script, before);
        }
        if self.has_pending_features && token == Token::SCRIPT {
            self.flush_features(compiler, script);
        }
    }

    /// `FIX` and `in_block` as for `traverse_full`.
    fn traverse_scope_root<const FIX: bool>(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope_node: NodeId,
        n: NodeId,
        current_script: Option<NodeId>,
        in_block: bool,
    ) {
        let token = n.get_token(compiler);
        // `ChangeTracker::is_change_scope_root` of the token read once.
        if n != scope_node && matches!(token, Token::SCRIPT | Token::FUNCTION) {
            if FIX {
                // The function's body is a change scope of its own, which the stamps of the
                // statements around it do not cover.
                self.fixpoint.note_token(Token::FUNCTION);
            }
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
        // Rust-only (performance divergence): skip a statement whose last visit with this pass's
        // optimizations changed nothing and which has not changed since (`PeepholeFixpoint`).
        let entry = if FIX {
            let pending_features = self.has_pending_features;
            let entry = self
                .fixpoint
                .enter(compiler, n, token, script, in_block, pending_features);
            match entry {
                FixpointEntry::Skip => return,
                FixpointEntry::Revisit => {
                    // No features wait (`enter`), so no scope exit adds any before the visit.
                    self.visit_node(compiler, n, token, script);
                    return;
                }
                _ => {}
            }
            entry
        } else {
            FixpointEntry::Plain
        };
        let child_in_block = FIX && is_statement_container(token);
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            let next = c.get_next(compiler);
            self.traverse_scope_root::<FIX>(compiler, scope_node, c, script, child_in_block);
            child = next;
        }
        if self.has_pending_features && is_scope_exit_token(token) {
            self.flush_features(compiler, script);
        }
        if FIX
            && let FixpointEntry::Verify(before, mark) = entry
            && mark & MARK_REVISIT != 0
        {
            // `verify` of a statement whose own visit is done again: what is below it must
            // change nothing; its own visit may.
            let pending_features = self.has_pending_features;
            self.fixpoint.verifying -= 1;
            self.fixpoint
                .verify_unchanged(compiler, n, mark, before, pending_features);
            self.fixpoint.verifying += 1;
            compiler.statement_side_effects.set_suspended(false);
        }
        let current_token = n.get_token(compiler);
        self.visit_node(compiler, n, current_token, script);
        if FIX {
            match entry {
                FixpointEntry::Plain | FixpointEntry::Skip | FixpointEntry::Revisit => {}
                FixpointEntry::Record(before) => {
                    self.fixpoint_record(compiler, n, token, script, before)
                }
                FixpointEntry::Verify(before, mark) => {
                    self.fixpoint.verifying -= 1;
                    compiler.statement_side_effects.set_suspended(false);
                    if mark & MARK_REVISIT == 0 {
                        let pending_features = self.has_pending_features;
                        self.fixpoint
                            .verify_unchanged(compiler, n, mark, before, pending_features);
                    }
                }
            }
        }
    }

    /// Records whether the visit of the statement `n` that began at `before` changed nothing.
    #[inline]
    fn fixpoint_record(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        token: Token,
        script: Option<NodeId>,
        before: FixpointBefore,
    ) {
        let pending_features = self.has_pending_features;
        let fixpoint = &mut self.fixpoint;
        let now = fixpoint.before(compiler);
        // Context tokens met: none, or only the statement's own (`token`, met first).
        let own = u64::from(fixpoint.sensitive[token as usize & 0xff]);
        let met = now.2 - before.2;
        let unchanged =
            !pending_features && now.0 == before.0 && now.1 == before.1 && (met == 0 || met == own);
        fixpoint.record(compiler, n, script, unchanged, now.3 == before.3, met != 0);
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
        self.fixpoint.features_of = None;
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
