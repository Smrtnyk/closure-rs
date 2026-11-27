/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CombinedCompilerPass.java.

//! Rust-only, not in Java (crates/DESIGN.md, "Performance divergences"): check passes that only
//! read the AST and report diagnostics, run over the top-level statements of the scripts on
//! worker threads.
//!
//! Java traverses the whole AST with the check's callback on the compiler thread and reports each
//! diagnostic when the callback finds it. Here the statements below each SCRIPT (or its
//! MODULE_BODY) are walked one by one, each with a callback of its own in the state the callback
//! has at a top-level statement, in the order `NodeTraversal` visits nodes; the diagnostics are
//! collected with the event (the `shouldTraverse` or `visit` call) that found them. The ROOT,
//! SCRIPT and MODULE_BODY nodes themselves are visited on the compiler thread with the pass's own
//! callback. The compiler then reports the collected diagnostics in traversal order, so it gets
//! the same diagnostics in the same order as from the Java traversal, whatever the number of
//! threads. For a combined pass (`CombinedCompilerPass`, which stops visiting once the compiler
//! has halting errors) the reports stop at the first event that starts with halting errors.
//!
//! A callback that needs what a worker cannot read (a scope, the type registry) gives up its
//! walk (`CheckContext::give_up`); the pass is then traversed as in Java, from the start, on the
//! compiler thread. Nothing has been reported by then.
//!
//! `CLOSURE_RS_PARALLEL_CHECKS=off` traverses as in Java; `=verify` collects the diagnostics as
//! above, then traverses as in Java (which reports them) and panics unless the compiler was given
//! the collected diagnostics, in order.

use crate::abstract_compiler::AbstractCompiler;
use crate::coding_convention::CodingConvention;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::NodeTraversal;
use closure_jstype::prelude::*;
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_base::tri::Tri;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::ops::Deref;
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_PARALLEL_CHECKS`.
pub(crate) static MODE: LazyLock<Mode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_PARALLEL_CHECKS").as_deref() {
            Ok("off") => Mode::Off,
            Ok("verify") => Mode::Verify,
            _ => Mode::On,
        },
    );

/// Top-level statements per worker thread below which a pass runs on fewer threads.
const MIN_UNITS_PER_THREAD: usize = 32;
const MAX_THREADS: usize = 8;

/// What a check callback asks of its traversal, so that the same code runs in a `NodeTraversal`
/// and on a worker (`Collector`).
pub(crate) trait CheckContext: Deref<Target = Ast> {
    /// `NodeTraversal#report(Node, DiagnosticType, String...)`.
    fn report(&mut self, n: NodeId, diagnostic: &'static DiagnosticType, args: &[&str]);
    /// `compiler.report(error)` for an error the callback built itself.
    fn report_error(&mut self, error: JSError);
    /// Whether `t.getScope().getVar(name)` finds a var, or None on a worker (which then gives up
    /// its walk).
    fn has_var(&mut self, name: &JsString) -> Option<bool>;
    /// Whether this context gives up for what a worker cannot do. A `NodeTraversal` never does;
    /// a worker's walk ends.
    fn give_up(&mut self) -> bool;
    /// `n.getJSType().getPossibleToBooleanOutcomes().toTri()` for a node with a JSType, or None
    /// on a worker (which then defers the check, see `defer`).
    fn type_boolean_outcomes(&mut self, n: NodeId) -> Option<Tri>;
    /// `compiler.toSource(n)`, or None on a worker (which then defers the check).
    fn source_utf16(&mut self, n: NodeId) -> Option<JsString>;
    /// On a worker: the check `kind` of `n` is left to the compiler thread, which makes it at this
    /// point of the reports (`Walked::report`'s resolver).
    fn defer(&mut self, kind: u8, n: NodeId);
    /// The AST to change, or None on a worker (which gives up its walk if it has to change it).
    fn ast_mut(&mut self) -> Option<&mut Ast>;
    /// `compiler.getCodingConvention().isClassFactoryCall(n)`.
    fn is_class_factory_call(&mut self, n: NodeId) -> bool;
}

/// What a walk on a worker found, in order.
pub(crate) enum Item {
    Error(JSError),
    /// A check left to the compiler thread (`CheckContext::defer`).
    Deferred(u8, NodeId),
}

impl CheckContext for NodeTraversal<'_> {
    #[inline]
    fn report(&mut self, n: NodeId, diagnostic: &'static DiagnosticType, args: &[&str]) {
        NodeTraversal::report(self, n, diagnostic, args);
    }
    #[inline]
    fn report_error(&mut self, error: JSError) {
        self.get_compiler().report(error);
    }
    fn has_var(&mut self, name: &JsString) -> Option<bool> {
        let scope = self.get_scope();
        Some(scope.get_var(self.get_compiler(), name.clone()).is_some())
    }
    #[inline]
    fn give_up(&mut self) -> bool {
        false
    }
    fn type_boolean_outcomes(&mut self, n: NodeId) -> Option<Tri> {
        let type_ = n.get_jstype(self).unwrap();
        let registry = self.get_compiler().get_type_registry();
        Some(type_.get_possible_to_boolean_outcomes(registry).to_tri())
    }
    fn source_utf16(&mut self, n: NodeId) -> Option<JsString> {
        Some(self.get_compiler().to_source_for_node_utf16(n))
    }
    fn defer(&mut self, _kind: u8, _n: NodeId) {
        unreachable!("a NodeTraversal makes every check itself")
    }
    fn ast_mut(&mut self) -> Option<&mut Ast> {
        Some(self.get_compiler())
    }
    fn is_class_factory_call(&mut self, n: NodeId) -> bool {
        let compiler = self.get_compiler();
        compiler
            .get_coding_convention()
            .is_class_factory_call(compiler, n)
    }
}

/// Rust-only: the context of a deferred check on the compiler thread (see `Item::Deferred`):
/// collects the diagnostics it reports.
pub(crate) struct MainContext<'a> {
    pub(crate) compiler: &'a mut AbstractCompiler,
    pub(crate) errors: Vec<JSError>,
}

impl Deref for MainContext<'_> {
    type Target = Ast;
    #[inline]
    fn deref(&self) -> &Ast {
        self.compiler
    }
}

impl CheckContext for MainContext<'_> {
    fn report(&mut self, n: NodeId, diagnostic: &'static DiagnosticType, args: &[&str]) {
        let error = JSError::make(self.compiler, n, diagnostic, args);
        self.errors.push(error);
    }
    fn report_error(&mut self, error: JSError) {
        self.errors.push(error);
    }
    fn has_var(&mut self, _name: &JsString) -> Option<bool> {
        unreachable!("deferred checks need no scope")
    }
    fn give_up(&mut self) -> bool {
        unreachable!("deferred checks do not give up")
    }
    fn type_boolean_outcomes(&mut self, n: NodeId) -> Option<Tri> {
        let type_ = n.get_jstype(self.compiler).unwrap();
        let registry = self.compiler.get_type_registry();
        Some(type_.get_possible_to_boolean_outcomes(registry).to_tri())
    }
    fn source_utf16(&mut self, n: NodeId) -> Option<JsString> {
        Some(self.compiler.to_source_for_node_utf16(n))
    }
    fn defer(&mut self, _kind: u8, _n: NodeId) {
        unreachable!("a deferred check is made on the compiler thread")
    }
    fn ast_mut(&mut self) -> Option<&mut Ast> {
        Some(self.compiler)
    }
    fn is_class_factory_call(&mut self, n: NodeId) -> bool {
        self.compiler
            .get_coding_convention()
            .is_class_factory_call(self.compiler, n)
    }
}

/// The context of a walk on a worker: collects the diagnostics with their event.
pub(crate) struct Collector<'a> {
    ast: &'a Ast,
    convention: &'a dyn CodingConvention,
    /// The number of `shouldTraverse` and `visit` calls of this walk so far.
    event: u32,
    items: Vec<(u32, Item)>,
    gave_up: bool,
}

impl<'a> Collector<'a> {
    fn new(ast: &'a Ast, convention: &'a dyn CodingConvention) -> Self {
        Self {
            ast,
            convention,
            event: 0,
            items: Vec::new(),
            gave_up: false,
        }
    }
}

impl Deref for Collector<'_> {
    type Target = Ast;
    #[inline]
    fn deref(&self) -> &Ast {
        self.ast
    }
}

impl CheckContext for Collector<'_> {
    fn report(&mut self, n: NodeId, diagnostic: &'static DiagnosticType, args: &[&str]) {
        let error = JSError::make(self.ast, n, diagnostic, args);
        self.items.push((self.event, Item::Error(error)));
    }
    fn report_error(&mut self, error: JSError) {
        self.items.push((self.event, Item::Error(error)));
    }
    fn has_var(&mut self, _name: &JsString) -> Option<bool> {
        self.gave_up = true;
        None
    }
    fn give_up(&mut self) -> bool {
        self.gave_up = true;
        true
    }
    fn type_boolean_outcomes(&mut self, _n: NodeId) -> Option<Tri> {
        None
    }
    fn source_utf16(&mut self, _n: NodeId) -> Option<JsString> {
        None
    }
    fn defer(&mut self, kind: u8, n: NodeId) {
        self.items.push((self.event, Item::Deferred(kind, n)));
    }
    fn ast_mut(&mut self) -> Option<&mut Ast> {
        None
    }
    fn is_class_factory_call(&mut self, n: NodeId) -> bool {
        self.convention.is_class_factory_call(self.ast, n)
    }
}

/// A check callback that can run on a worker: `NodeTraversal.Callback` with a `Collector`.
pub(crate) trait ReadOnlyCallback {
    fn should_traverse(
        &mut self,
        cx: &mut Collector<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool;
    fn visit(&mut self, cx: &mut Collector<'_>, n: NodeId, parent: Option<NodeId>);
}

/// `CombinedCompilerPass` on a worker: the callbacks in order, each inactive below a node it
/// declined until the walk leaves that node. (The halting errors check is made when the
/// diagnostics are reported.)
pub(crate) struct Combined {
    callbacks: Vec<(Box<dyn ReadOnlyCallback + Send>, Option<NodeId>)>,
}

impl Combined {
    pub(crate) fn new(callbacks: Vec<Box<dyn ReadOnlyCallback + Send>>) -> Self {
        Self {
            callbacks: callbacks.into_iter().map(|c| (c, None)).collect(),
        }
    }
}

impl ReadOnlyCallback for Combined {
    // port: CombinedCompilerPass#shouldTraverse
    fn should_traverse(
        &mut self,
        cx: &mut Collector<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        for (callback, waiting) in &mut self.callbacks {
            // port: CombinedCompilerPass.CallbackWrapper#shouldTraverseIfActive
            if waiting.is_none() && !callback.should_traverse(cx, n, parent) {
                *waiting = Some(n);
            }
        }
        true
    }

    // port: CombinedCompilerPass#visit
    fn visit(&mut self, cx: &mut Collector<'_>, n: NodeId, parent: Option<NodeId>) {
        for (callback, waiting) in &mut self.callbacks {
            // port: CombinedCompilerPass.CallbackWrapper#visitOrMaybeActivate
            if waiting.is_none() {
                callback.visit(cx, n, parent);
            } else if *waiting == Some(n) {
                *waiting = None;
            }
        }
    }
}

#[inline]
fn pre<C: ReadOnlyCallback + ?Sized>(
    callback: &mut C,
    cx: &mut Collector<'_>,
    n: NodeId,
    parent: Option<NodeId>,
) -> bool {
    cx.event += 1;
    callback.should_traverse(cx, n, parent)
}

#[inline]
fn post<C: ReadOnlyCallback + ?Sized>(
    callback: &mut C,
    cx: &mut Collector<'_>,
    n: NodeId,
    parent: Option<NodeId>,
) {
    cx.event += 1;
    callback.visit(cx, n, parent);
}

/// `NodeTraversal#traverseBranch` without scopes, for a node below a SCRIPT or MODULE_BODY: the
/// same calls of `shouldTraverse` and `visit` in the same order (a class visits its extends clause
/// before its name; class members visit the keys of computed members first).
fn walk<C: ReadOnlyCallback + ?Sized>(
    callback: &mut C,
    cx: &mut Collector<'_>,
    n: NodeId,
    parent: Option<NodeId>,
) {
    if cx.gave_up {
        return;
    }
    let ast = cx.ast;
    match n.get_token(ast) {
        Token::CLASS => {
            if !pre(callback, cx, n, parent) {
                return;
            }
            let class_name = n.get_first_child(ast).unwrap();
            let extends_clause = class_name.get_next(ast).unwrap();
            let body = extends_clause.get_next(ast).unwrap();
            walk(callback, cx, extends_clause, Some(n));
            walk(callback, cx, class_name, Some(n));
            walk(callback, cx, body, Some(n));
            post(callback, cx, n, parent);
        }
        Token::CLASS_MEMBERS => {
            if !pre(callback, cx, n, parent) {
                return;
            }
            let mut child = n.get_first_child(ast);
            while let Some(c) = child {
                if c.is_computed_prop(ast) || c.is_computed_field_def(ast) {
                    walk(callback, cx, c.get_first_child(ast).unwrap(), Some(c));
                }
                child = c.get_next(ast);
            }
            let mut child = n.get_first_child(ast);
            while let Some(c) = child {
                match c.get_token(ast) {
                    Token::COMPUTED_PROP => {
                        if pre(callback, cx, c, Some(n)) {
                            walk(callback, cx, c.get_last_child(ast).unwrap(), Some(c));
                            post(callback, cx, c, Some(n));
                        }
                    }
                    Token::COMPUTED_FIELD_DEF => {
                        if pre(callback, cx, c, Some(n)) {
                            if c.has_two_children(ast) {
                                walk(callback, cx, c.get_last_child(ast).unwrap(), Some(c));
                            }
                            post(callback, cx, c, Some(n));
                        }
                    }
                    Token::MEMBER_FIELD_DEF
                    | Token::BLOCK
                    | Token::MEMBER_FUNCTION_DEF
                    | Token::MEMBER_VARIABLE_DEF
                    | Token::GETTER_DEF
                    | Token::SETTER_DEF => walk(callback, cx, c, Some(n)),
                    token => panic!("Invalid class member: {token}"),
                }
                child = c.get_next(ast);
            }
            post(callback, cx, n, parent);
        }
        Token::SCRIPT | Token::MODULE_BODY | Token::ROOT => {
            unreachable!("parallel checks walk the statements below scripts")
        }
        _ => {
            if !pre(callback, cx, n, parent) {
                return;
            }
            let mut child = n.get_first_child(ast);
            while let Some(c) = child {
                walk(callback, cx, c, Some(n));
                child = c.get_next(ast);
            }
            post(callback, cx, n, parent);
        }
    }
}

/// What a statement's walk found, and its callback.
type UnitResult<U> = (Vec<(u32, Item)>, U);

/// One part of the traversal, in order.
enum Segment {
    /// What one call on the compiler thread found.
    Main(Vec<Item>),
    /// A statement walked on a worker (an index into the units).
    Unit(usize),
}

/// The part of a pass that walks the statements of the scripts (see the module documentation).
pub(crate) struct Run<'p, M: ReadOnlyCallback> {
    /// The pass's own callback, for the ROOT, SCRIPT and MODULE_BODY nodes.
    pub(crate) main: &'p mut M,
    /// Rust-only bundled-script skip of the callback (`Callback::should_traverse_script`): the
    /// first statement to walk when the callback walks only the suffix of a bundled script (it
    /// then visits the SCRIPT without calling `shouldTraverse` on it), else None.
    pub(crate) script_suffix: &'p dyn Fn(&AbstractCompiler, NodeId) -> Option<Option<NodeId>>,
}

/// The result of walking the statements: the diagnostics and the workers' callbacks, or None
/// when a walk gave up.
pub(crate) struct Walked<U> {
    segments: Vec<Segment>,
    unit_items: Vec<Vec<(u32, Item)>>,
    /// The workers' callbacks, in the order of their statements.
    unit_callbacks: Vec<U>,
}

impl<M: ReadOnlyCallback> Run<'_, M> {
    fn main_event(
        &mut self,
        compiler: &AbstractCompiler,
        segments: &mut Vec<Segment>,
        n: NodeId,
        parent: Option<NodeId>,
        is_pre: bool,
    ) -> bool {
        let mut cx = Collector::new(compiler, compiler.get_coding_convention());
        let traverse = if is_pre {
            self.main.should_traverse(&mut cx, n, parent)
        } else {
            self.main.visit(&mut cx, n, parent);
            true
        };
        assert!(
            !cx.gave_up,
            "parallel checks: the callback gave up on a ROOT, SCRIPT or MODULE_BODY"
        );
        segments.push(Segment::Main(
            cx.items.into_iter().map(|(_, item)| item).collect(),
        ));
        traverse
    }

    /// `NodeTraversal#traverseBranch(root)` for a ROOT: the calls for the ROOT, SCRIPT and
    /// MODULE_BODY nodes, and the statements to walk.
    fn plan(
        &mut self,
        compiler: &AbstractCompiler,
        root: NodeId,
        segments: &mut Vec<Segment>,
        units: &mut Vec<(NodeId, NodeId)>,
    ) {
        let ast: &Ast = compiler;
        let root_parent = root.get_parent(ast);
        if !self.main_event(compiler, segments, root, root_parent, true) {
            return;
        }
        let mut child = root.get_first_child(ast);
        while let Some(script) = child {
            child = script.get_next(ast);
            assert!(
                script.is_script(ast),
                "parallel checks: a ROOT child is not a SCRIPT"
            );
            if let Some(first) = (self.script_suffix)(compiler, script) {
                let mut statement = first;
                while let Some(s) = statement {
                    segments.push(Segment::Unit(units.len()));
                    units.push((s, script));
                    statement = s.get_next(ast);
                }
                self.main_event(compiler, segments, script, Some(root), false);
                continue;
            }
            if !self.main_event(compiler, segments, script, Some(root), true) {
                continue;
            }
            let mut statement = script.get_first_child(ast);
            while let Some(s) = statement {
                statement = s.get_next(ast);
                if s.is_module_body(ast) {
                    if self.main_event(compiler, segments, s, Some(script), true) {
                        let mut inner = s.get_first_child(ast);
                        while let Some(i) = inner {
                            segments.push(Segment::Unit(units.len()));
                            units.push((i, s));
                            inner = i.get_next(ast);
                        }
                        self.main_event(compiler, segments, s, Some(script), false);
                    }
                } else {
                    segments.push(Segment::Unit(units.len()));
                    units.push((s, script));
                }
            }
            self.main_event(compiler, segments, script, Some(root), false);
        }
        self.main_event(compiler, segments, root, root_parent, false);
    }

    /// Walks the statements below `roots` (ROOT nodes, traversed in order as by
    /// `NodeTraversal#traverse` of each) with a callback `unit` makes for each statement (given
    /// the statement's SCRIPT or MODULE_BODY).
    pub(crate) fn walk<U: ReadOnlyCallback + Send>(
        &mut self,
        compiler: &AbstractCompiler,
        roots: &[NodeId],
        unit: impl Fn(NodeId) -> U + Sync,
    ) -> Option<Walked<U>> {
        let mut segments = Vec::new();
        let mut units = Vec::new();
        for &root in roots {
            self.plan(compiler, root, &mut segments, &mut units);
        }
        let ast: &Ast = compiler;
        let convention = compiler.get_coding_convention();
        let threads = crate::parallel_parse::parallel_thread_count(
            units.len(),
            MIN_UNITS_PER_THREAD,
            MAX_THREADS,
        );
        let results = crate::parallel_parse::run_scoped_indexed(
            "closure-checks",
            threads,
            &units,
            |_, &(statement, parent)| {
                let mut callback = unit(parent);
                let mut cx = Collector::new(ast, convention);
                walk(&mut callback, &mut cx, statement, Some(parent));
                Some((cx.gave_up, cx.items, callback))
            },
        );
        let mut slots: Vec<Option<UnitResult<U>>> = Vec::new();
        slots.resize_with(units.len(), || None);
        let mut gave_up = false;
        for (idx, (unit_gave_up, items, callback)) in results.into_iter().flatten() {
            gave_up |= unit_gave_up;
            slots[idx] = Some((items, callback));
        }
        if gave_up {
            return None;
        }
        let mut unit_items = Vec::with_capacity(units.len());
        let mut unit_callbacks = Vec::with_capacity(units.len());
        for slot in slots {
            let (items, callback) = slot.expect("parallel checks: a statement was not walked");
            unit_items.push(items);
            unit_callbacks.push(callback);
        }
        Some(Walked {
            segments,
            unit_items,
            unit_callbacks,
        })
    }
}

impl<U> Walked<U> {
    /// The workers' callbacks, in the order of their statements.
    pub(crate) fn take_unit_callbacks(&mut self) -> Vec<U> {
        std::mem::take(&mut self.unit_callbacks)
    }

    /// What the walks found in traversal order, each with its event (segment, event in the
    /// segment).
    fn ordered(self) -> Vec<((usize, u32), Item)> {
        let mut unit_items: Vec<Option<Vec<(u32, Item)>>> =
            self.unit_items.into_iter().map(Some).collect();
        let mut out = Vec::new();
        for (index, segment) in self.segments.into_iter().enumerate() {
            match segment {
                Segment::Main(items) => {
                    out.extend(items.into_iter().map(|item| ((index, 0), item)));
                }
                Segment::Unit(unit) => {
                    let items = unit_items[unit].take().unwrap();
                    out.extend(
                        items
                            .into_iter()
                            .map(|(event, item)| ((index, event), item)),
                    );
                }
            }
        }
        out
    }

    /// Reports the diagnostics in traversal order, making each deferred check with `resolve` at
    /// its place. With `halting` (a combined pass), the reports stop at the first event that
    /// starts with halting errors.
    pub(crate) fn report_with(
        self,
        compiler: &mut AbstractCompiler,
        halting: bool,
        resolve: &mut dyn FnMut(&mut AbstractCompiler, u8, NodeId) -> Vec<JSError>,
    ) {
        let ordered = self.ordered();
        if halting && compiler.has_halting_errors() {
            return;
        }
        let mut last_event = None;
        for (event, item) in ordered {
            if halting && last_event != Some(event) && compiler.has_halting_errors() {
                return;
            }
            last_event = Some(event);
            match item {
                Item::Error(error) => compiler.report(error),
                Item::Deferred(kind, n) => {
                    for error in resolve(compiler, kind, n) {
                        compiler.report(error);
                    }
                }
            }
        }
    }

    /// `report_with` for walks that defer nothing.
    pub(crate) fn report(self, compiler: &mut AbstractCompiler, halting: bool) {
        self.report_with(compiler, halting, &mut no_deferred_checks);
    }

    /// The diagnostics in traversal order (for a pass that is not combined and defers nothing).
    pub(crate) fn ordered_errors(self) -> Vec<JSError> {
        self.ordered()
            .into_iter()
            .map(|(_, item)| match item {
                Item::Error(error) => error,
                Item::Deferred(..) => unreachable!("this pass defers no check"),
            })
            .collect()
    }

    /// `verify_with` for walks that defer nothing.
    pub(crate) fn verify(
        self,
        compiler: &mut AbstractCompiler,
        pass: &str,
        halting: bool,
        serial: impl FnOnce(&mut AbstractCompiler),
    ) {
        self.verify_with(compiler, pass, halting, &mut no_deferred_checks, serial);
    }

    /// `CLOSURE_RS_PARALLEL_CHECKS=verify`: makes the deferred checks, runs `serial` (the Java
    /// traversal, which reports) and panics unless it reported exactly the diagnostics
    /// `report_with` would have.
    pub(crate) fn verify_with(
        self,
        compiler: &mut AbstractCompiler,
        pass: &str,
        halting: bool,
        resolve: &mut dyn FnMut(&mut AbstractCompiler, u8, NodeId) -> Vec<JSError>,
        serial: impl FnOnce(&mut AbstractCompiler),
    ) {
        let mut ordered: Vec<((usize, u32), JSError)> = Vec::new();
        for (event, item) in self.ordered() {
            match item {
                Item::Error(error) => ordered.push((event, error)),
                Item::Deferred(kind, n) => {
                    ordered.extend(resolve(compiler, kind, n).into_iter().map(|e| (event, e)));
                }
            }
        }
        let reported = tap(compiler, serial);
        let same = if halting && reported.len() < ordered.len() {
            // The traversal stopped at halting errors: it reported a prefix of the diagnostics,
            // ending with the last one of an event.
            compiler.has_halting_errors()
                && reported
                    .iter()
                    .zip(&ordered)
                    .all(|(reported, (_, collected))| reported == collected)
                && (reported.is_empty()
                    || ordered[reported.len() - 1].0 != ordered[reported.len()].0)
        } else {
            reported.len() == ordered.len()
                && reported
                    .iter()
                    .zip(&ordered)
                    .all(|(reported, (_, collected))| reported == collected)
        };
        let collected: Vec<JSError> = ordered.into_iter().map(|(_, error)| error).collect();
        assert_same(pass, same, &reported, &collected);
    }
}

fn no_deferred_checks(_: &mut AbstractCompiler, _: u8, _: NodeId) -> Vec<JSError> {
    unreachable!("this pass defers no check")
}

/// Runs `serial`, returning the errors it gave to `compiler.report`, in order.
fn tap(
    compiler: &mut AbstractCompiler,
    serial: impl FnOnce(&mut AbstractCompiler),
) -> Vec<JSError> {
    let previous = compiler.report_tap.replace(Vec::new());
    assert!(previous.is_none(), "parallel checks: nested report taps");
    serial(compiler);
    compiler.report_tap.take().unwrap()
}

fn assert_same(pass: &str, same: bool, reported: &[JSError], collected: &[JSError]) {
    assert!(
        same,
        "CLOSURE_RS_PARALLEL_CHECKS=verify: {pass} reported {} diagnostics\n{reported:#?}\nbut \
         the parallel walks collected {}\n{collected:#?}",
        reported.len(),
        collected.len()
    );
}

/// `CLOSURE_RS_PARALLEL_CHECKS=verify` for a pass that is not combined: runs `serial` (the Java
/// traversals, which report) and panics unless they reported exactly `collected`, in order.
pub(crate) fn verify_reports(
    compiler: &mut AbstractCompiler,
    pass: &str,
    collected: &[JSError],
    serial: impl FnOnce(&mut AbstractCompiler),
) {
    let reported = tap(compiler, serial);
    assert_same(pass, reported == collected, &reported, collected);
}

/// Reports `errors` in order.
pub(crate) fn report_all(compiler: &mut AbstractCompiler, errors: Vec<JSError>) {
    for error in errors {
        compiler.report(error);
    }
}

/// What the twins of a combined pass share: the names each ES module imports (the local names
/// of its IMPORT statements), by MODULE_BODY.
#[derive(Default)]
pub(crate) struct TwinContext {
    pub(crate) module_imports:
        closure_rhino::fast_hash::IndexMap<NodeId, std::sync::Arc<[JsString]>>,
}

impl TwinContext {
    fn new(ast: &Ast, root: NodeId) -> Self {
        let mut module_imports = closure_rhino::fast_hash::IndexMap::default();
        let mut script = root.get_first_child(ast);
        while let Some(sc) = script {
            script = sc.get_next(ast);
            let Some(body) = sc.get_first_child(ast).filter(|b| b.is_module_body(ast)) else {
                continue;
            };
            let mut names = Vec::new();
            let mut statement = body.get_first_child(ast);
            while let Some(st) = statement {
                statement = st.get_next(ast);
                if !st.is_import(ast) {
                    continue;
                }
                let mut part = st.get_first_child(ast);
                while let Some(p) = part {
                    part = p.get_next(ast);
                    if p.is_name(ast) || p.is_import_star(ast) {
                        names.push(p.get_string(ast));
                    } else if p.is_import_specs(ast) {
                        let mut spec = p.get_first_child(ast);
                        while let Some(sp) = spec {
                            spec = sp.get_next(ast);
                            if let Some(local) = sp.get_last_child(ast) {
                                names.push(local.get_string(ast));
                            }
                        }
                    }
                }
            }
            module_imports.insert(body, names.into());
        }
        Self { module_imports }
    }
}

/// The read-only twin of one callback of a combined pass, and the deferred checks it makes.
pub(crate) struct Twin {
    /// Makes the callback for a statement below the given SCRIPT or MODULE_BODY.
    pub(crate) make: fn(&TwinContext, NodeId) -> Box<dyn ReadOnlyCallback + Send>,
    /// Makes a check this callback deferred (`Item::Deferred` with this callback's kinds).
    pub(crate) resolve: Option<fn(&mut MainContext<'_>, u8, NodeId)>,
}

/// Rust-only (performance divergence): a combined check pass (`CombinedCompilerPass`) whose
/// callbacks all have read-only twins walks the statements on worker threads (see the module
/// documentation); otherwise, or when a walk gives up, it traverses as in Java.
pub(crate) struct ParallelCombinedPass {
    pub(crate) name: &'static str,
    pub(crate) serial: Box<dyn CompilerPass>,
    /// The twins of the serial callbacks, in the same order; None when one has none.
    pub(crate) twins: Option<Vec<Twin>>,
}

impl ParallelCombinedPass {
    fn process_in_parallel(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) -> bool {
        let Some(twins) = &self.twins else {
            return false;
        };
        let context = TwinContext::new(compiler, root);
        let context = &context;
        let make_all = |parent: NodeId| {
            Combined::new(
                twins
                    .iter()
                    .map(|twin| (twin.make)(context, parent))
                    .collect(),
            )
        };
        let mut main = make_all(root);
        let no_suffix = |_: &AbstractCompiler, _: NodeId| None;
        let Some(walked) = (Run {
            main: &mut main,
            script_suffix: &no_suffix,
        })
        .walk(compiler, &[root], make_all) else {
            return false;
        };
        let resolvers: Vec<fn(&mut MainContext<'_>, u8, NodeId)> =
            twins.iter().filter_map(|twin| twin.resolve).collect();
        let mut resolve = |compiler: &mut AbstractCompiler, kind: u8, n: NodeId| {
            let mut cx = MainContext {
                compiler,
                errors: Vec::new(),
            };
            for resolver in &resolvers {
                resolver(&mut cx, kind, n);
            }
            cx.errors
        };
        if *MODE == Mode::Verify {
            let serial = &mut self.serial;
            walked.verify_with(compiler, self.name, true, &mut resolve, |compiler| {
                serial.process(compiler, externs, root);
            });
        } else {
            walked.report_with(compiler, true, &mut resolve);
        }
        true
    }
}

impl CompilerPass for ParallelCombinedPass {
    // port: CombinedCompilerPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        if *MODE != Mode::Off && self.process_in_parallel(compiler, externs, root) {
            return;
        }
        self.serial.process(compiler, externs, root);
    }
}
