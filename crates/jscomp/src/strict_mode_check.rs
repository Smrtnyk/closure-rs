/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/StrictModeCheck.java.

//! Port of `StrictModeCheck.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::check_level::CheckLevel;
use crate::check_reg_exp::CheckRegExp;
use crate::check_side_effects::CheckSideEffects;
use crate::check_super::CheckSuper;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::syntactic_scope_cache::{ChangeMark, SyntacticScopeCache};
use closure_jstype::prelude::*;
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::sync::LazyLock;

// port: StrictModeCheck#USE_OF_WITH
pub static USE_OF_WITH: DiagnosticType = DiagnosticType::error(
    "JSC_USE_OF_WITH",
    "The 'with' statement cannot be used in strict mode.",
);

// port: StrictModeCheck#EVAL_DECLARATION
pub static EVAL_DECLARATION: DiagnosticType = DiagnosticType::error(
    "JSC_EVAL_DECLARATION",
    "\"eval\" cannot be redeclared in strict mode",
);

// port: StrictModeCheck#EVAL_ASSIGNMENT
pub static EVAL_ASSIGNMENT: DiagnosticType = DiagnosticType::error(
    "JSC_EVAL_ASSIGNMENT",
    "the \"eval\" object cannot be reassigned in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_DECLARATION
pub static ARGUMENTS_DECLARATION: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_DECLARATION",
    "\"arguments\" cannot be redeclared in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_ASSIGNMENT
pub static ARGUMENTS_ASSIGNMENT: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_ASSIGNMENT",
    "the \"arguments\" object cannot be reassigned in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_CALLEE_FORBIDDEN
pub static ARGUMENTS_CALLEE_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_CALLEE_FORBIDDEN",
    "\"arguments.callee\" cannot be used in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_CALLER_FORBIDDEN
pub static ARGUMENTS_CALLER_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_CALLER_FORBIDDEN",
    "\"arguments.caller\" cannot be used in strict mode",
);

// port: StrictModeCheck#FUNCTION_CALLER_FORBIDDEN
pub static FUNCTION_CALLER_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_FUNCTION_CALLER_FORBIDDEN",
    "A function''s \"caller\" property cannot be used in strict mode",
);

// port: StrictModeCheck#FUNCTION_ARGUMENTS_PROP_FORBIDDEN
pub static FUNCTION_ARGUMENTS_PROP_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_FUNCTION_ARGUMENTS_PROP_FORBIDDEN",
    "A function''s \"arguments\" property cannot be used in strict mode",
);

// port: StrictModeCheck#DELETE_VARIABLE
pub static DELETE_VARIABLE: DiagnosticType = DiagnosticType::error(
    "JSC_DELETE_VARIABLE",
    "variables, functions, and arguments cannot be deleted in strict mode",
);

// port: StrictModeCheck#DUPLICATE_MEMBER
pub static DUPLICATE_MEMBER: DiagnosticType = DiagnosticType::warning(
    "JSC_DUPLICATE_MEMBER",
    "Class or object literal contains duplicate member \"{0}\". In non-strict code, the last duplicate will overwrite the others.",
);

/// Checks that the code obeys the static restrictions of strict mode:
///
/// 1. No use of "with".
/// 2. No deleting variables, functions, or arguments.
/// 3. No re-declarations or assignments of "eval" or arguments.
/// 4. No use of arguments.callee
/// 5. No use of arguments.caller
/// 6. Class: Always under strict mode
/// 7. In addition, no duplicate class method names
pub struct StrictModeCheck {
    default_level: CheckLevel,
    /// Rust-only (performance divergence): see `CheckSuper::capture`.
    capture: Option<Vec<JSError>>,
}

impl StrictModeCheck {
    // port: StrictModeCheck#StrictModeCheck
    pub fn new(_compiler: &AbstractCompiler, default_level: CheckLevel) -> Self {
        Self {
            default_level,
            capture: None,
        }
    }

    /// Reports a warning for with statements.
    // port: StrictModeCheck#checkWith
    fn check_with(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let info = n.get_jsdoc_info(compiler);
        let allow_with = info
            .as_ref()
            .is_some_and(|info| info.get_suppressions().contains(&JsString::from("with")));
        if !allow_with {
            self.report(compiler, n, &USE_OF_WITH, &[]);
        }
    }

    /// Determines if the given name is a declaration, which can be a declaration of a variable,
    /// function, or argument.
    // port: StrictModeCheck#isDeclaration
    fn is_declaration(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::LET | Token::CONST | Token::VAR | Token::CATCH => true,
            Token::FUNCTION => Some(n) == parent.get_first_child(ast),
            Token::PARAM_LIST => n.get_grandparent(ast).unwrap().is_function(ast),
            _ => false,
        }
    }

    /// Checks that an assignment is not to the "arguments" object.
    // port: StrictModeCheck#checkAssignment
    fn check_assignment(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.get_first_child(compiler).unwrap();
        if first.is_name(compiler) {
            if first.get_string_ref(compiler) == "arguments" {
                self.report(compiler, n, &ARGUMENTS_ASSIGNMENT, &[]);
            } else if first.get_string_ref(compiler) == "eval" {
                // Note that assignment to eval is already illegal because any use of
                // that name is illegal.
                self.report(compiler, n, &EVAL_ASSIGNMENT, &[]);
            }
        }
    }

    /// Checks that variables, functions, and arguments are not deleted.
    // port: StrictModeCheck#checkDelete
    fn check_delete(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let first = n.get_first_child(t).unwrap();
        if first.is_name(t) {
            let name = first.get_string(t);
            let scope = t.get_scope();
            let v = scope.get_var(t.get_compiler(), name);
            if v.is_some() {
                self.report(t.get_compiler(), n, &DELETE_VARIABLE, &[]);
            }
        }
    }

    /// Checks that object literal keys or class method names are valid.
    // port: StrictModeCheck#checkObjectLiteralOrClass
    fn check_object_literal_or_class(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut getters: IndexSet<JsString> = IndexSet::<_>::default();
        let mut setters: IndexSet<JsString> = IndexSet::<_>::default();
        let mut static_getters: IndexSet<JsString> = IndexSet::<_>::default();
        let mut static_setters: IndexSet<JsString> = IndexSet::<_>::default();

        /*
         * Iterate backwards because the last duplicate is the one that will be used in sloppy or
         * ES6 code. The earlier duplicates are the ones that should be removed.
         */
        let mut key = n.get_last_child(compiler);
        while let Some(k) = key {
            if k.is_empty(compiler)
                || k.is_computed_prop(compiler)
                || k.is_spread(compiler)
                || k.is_computed_field_def(compiler)
                // Computed properties cannot be computed at compile time
                || NodeUtil::is_class_static_block(compiler, k)
            // Will not check since whether duplicates are declared/assigned cannot be determined
            // at compile time since we do not know which code will be run
            {
                key = k.get_previous(compiler);
                continue;
            }

            let key_name = k.get_string(compiler);
            if !k.is_setter_def(compiler) {
                // normal property and getter cases
                let set = if k.is_static_member(compiler) {
                    &mut static_getters
                } else {
                    &mut getters
                };
                if !set.insert(key_name.clone()) {
                    self.report(compiler, k, &DUPLICATE_MEMBER, &[&key_name.to_string()]);
                }
            }
            if !k.is_getter_def(compiler) {
                // normal property and setter cases
                let set = if k.is_static_member(compiler) {
                    &mut static_setters
                } else {
                    &mut setters
                };
                if !set.insert(key_name.clone()) {
                    self.report(compiler, k, &DUPLICATE_MEMBER, &[&key_name.to_string()]);
                }
            }
            key = k.get_previous(compiler);
        }
    }

    // port: StrictModeCheck#isFunctionType
    fn is_function_type(compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        let type_ = n.get_jstype(compiler);
        match type_ {
            Some(type_) => {
                let (reg, _) = compiler.get_type_registry_field_and_ast();
                type_.is_function_type(reg.expect(
                    "StrictModeCheck: a node has a JSType but the compiler has no JSTypeRegistry",
                ))
            }
            None => false,
        }
    }

    // port: StrictModeCheck#report
    fn report(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        diagnostic: &'static DiagnosticType,
        args: &[&str],
    ) {
        report_at_level(
            self.default_level,
            &mut self.capture,
            compiler,
            n,
            diagnostic,
            args,
        );
    }
}

// port: StrictModeCheck#report
fn report_at_level(
    default_level: CheckLevel,
    capture: &mut Option<Vec<JSError>>,
    compiler: &mut AbstractCompiler,
    n: NodeId,
    diagnostic: &'static DiagnosticType,
    args: &[&str],
) {
    let error = JSError::builder(diagnostic, args)
        .set_level(default_level)
        .set_node(compiler, n)
        .build();
    match capture {
        Some(errors) => errors.push(error),
        None => compiler.report(error),
    }
}

impl CompilerPass for StrictModeCheck {
    // port: StrictModeCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Rust-only (performance divergence): see `CheckLookahead`.
        if let Some(kept) =
            CheckLookahead::take(compiler, root, LookaheadReads::Code, |lookahead| {
                lookahead.strict_mode.take()
            })
            && kept.default_level == self.default_level
        {
            self.capture = Some(Vec::new());
            NodeTraversal::traverse_roots(
                compiler,
                &mut ExternsOnly {
                    strict: self,
                    code_root: root,
                },
                externs,
                root,
            );
            let mut errors = self.capture.take().expect("captured");
            errors.extend(kept.code_errors);
            errors.extend(kept.non_extern_errors);
            if *LOOKAHEAD_MODE == LookaheadMode::Verify {
                self.capture = Some(Vec::new());
                self.process_as_java(compiler, externs, root);
                verify_kept("StrictModeCheck", &errors, &self.capture.take().unwrap());
            }
            report_all(compiler, errors);
            return;
        }
        self.process_as_java(compiler, externs, root);
    }
}

impl Callback for StrictModeCheck {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    /// Rust-only (performance divergence): Java visits every node of the bundled externs. An
    /// unchanged `normalize_clean` bundled prefix holds no DELPROP, WITH or class, its object
    /// literals are empty and no assignment in it targets the name `arguments` or `eval` (a test
    /// checks this), so `visit` reports nothing there. Statements appended to the script are
    /// visited.
    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if *crate::variable_reference_check::SKIP_BUNDLED_EXTERNS
            && let Some((facts, first)) =
                t.bundled_script_prefix(script, closure_rhino::node::SnapshotUse::SyntaxAndNames)
            && facts.normalize_clean
        {
            t.traverse_script_suffix(script, parent, first, self);
            return false;
        }
        true
    }

    // port: StrictModeCheck#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_assign(t) {
            self.check_assignment(t.get_compiler(), n);
        } else if n.is_del_prop(t) {
            self.check_delete(t, n);
        } else if n.is_object_lit(t) {
            self.check_object_literal_or_class(t.get_compiler(), n);
        } else if n.is_class(t) {
            let last = n.get_last_child(t).unwrap();
            self.check_object_literal_or_class(t.get_compiler(), last);
        } else if n.is_with(t) {
            self.check_with(t.get_compiler(), n);
        }
    }
}

/// Checks that are performed on non-extern code only.
struct NonExternChecks {
    default_level: CheckLevel,
    /// Rust-only (performance divergence): see `CheckSuper::capture`.
    capture: Option<Vec<JSError>>,
}

impl NonExternChecks {
    // port: StrictModeCheck#report
    fn report(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        diagnostic: &'static DiagnosticType,
        args: &[&str],
    ) {
        report_at_level(
            self.default_level,
            &mut self.capture,
            compiler,
            n,
            diagnostic,
            args,
        );
    }

    /// Checks for illegal declarations.
    // port: StrictModeCheck.NonExternChecks#checkDeclaration
    fn check_declaration(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.get_string_ref(compiler) == "eval" {
            self.report(compiler, n, &EVAL_DECLARATION, &[]);
        } else if n.get_string_ref(compiler) == "arguments" {
            self.report(compiler, n, &ARGUMENTS_DECLARATION, &[]);
        }
    }

    /// Checks that the arguments.callee is not used.
    // port: StrictModeCheck.NonExternChecks#checkGetProp
    fn check_get_prop(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let target = n.get_first_child(compiler).unwrap();
        let name = n.get_string(compiler);
        if name == "callee" {
            if target.is_name(compiler) && target.get_string_ref(compiler) == "arguments" {
                self.report(compiler, n, &ARGUMENTS_CALLEE_FORBIDDEN, &[]);
            }
        } else if name == "caller" {
            if target.is_name(compiler) && target.get_string_ref(compiler) == "arguments" {
                self.report(compiler, n, &ARGUMENTS_CALLER_FORBIDDEN, &[]);
            } else if StrictModeCheck::is_function_type(compiler, target) {
                self.report(compiler, n, &FUNCTION_CALLER_FORBIDDEN, &[]);
            }
        } else if name == "arguments" && StrictModeCheck::is_function_type(compiler, target) {
            self.report(compiler, n, &FUNCTION_ARGUMENTS_PROP_FORBIDDEN, &[]);
        }
    }
}

impl Callback for NonExternChecks {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: StrictModeCheck.NonExternChecks#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(t) && StrictModeCheck::is_declaration(t, n) {
            self.check_declaration(t.get_compiler(), n);
        } else if n.is_get_prop(t) {
            self.check_get_prop(t.get_compiler(), n);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LookaheadMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_CHECK_LOOKAHEAD=off` traverses the code once per pass as Java does; `=verify` also
/// makes every traversal of the passes and compares their diagnostics and results (see
/// `CheckLookahead`).
pub(crate) static LOOKAHEAD_MODE: LazyLock<LookaheadMode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_CHECK_LOOKAHEAD").as_deref() {
            Ok("off") => LookaheadMode::Off,
            Ok("verify") => LookaheadMode::Verify,
            _ => LookaheadMode::On,
        },
    );

/// Rust-only (performance divergence, crates/DESIGN.md): checkJsDocAndEs6Modules shares its
/// traversal of the code with StrictModeCheck, CheckSuper, CheckSideEffects and CheckRegExp, which
/// run later in the checks. Java traverses the code once for checkJsDocAndEs6Modules (a
/// CombinedCompilerPass), twice for StrictModeCheck (its own callback over the externs and the
/// code, then `NonExternChecks` over the code), and once each for the other three, each callback
/// only reading the code and reporting diagnostics (checkJsDocAndEs6Modules may rename type names
/// in JSDoc, CheckSideEffects changes the tree after its traversal, CheckRegExp sets a compiler
/// flag). Here the traversal of checkJsDocAndEs6Modules also runs the callbacks of the later
/// passes, each in the state its own traversal would give it, keeping what they would report
/// instead of reporting it; its own callbacks run as in the CombinedCompilerPass, reporting and
/// stopping at halting errors. Each later pass, when it runs, reports what was kept for it at that
/// time and takes over the result of its traversal, if nothing it reads has changed since (the
/// structure of the code for CheckSuper and CheckRegExp, also the JSDoc and flags of the code for
/// StrictModeCheck and CheckSideEffects; `nodes_changed_since`) and its configuration is the same.
/// What reads the externs, which change in between (declaredGlobalExternsOnWindow, VarCheck), is
/// done when the pass runs: StrictModeCheck's traversal of the externs, and CheckSideEffects'
/// collection of the externs, against which it then decides the calls it kept
/// (`SideEffectEvent::ExternCall`). A pass that does not run (after halting errors) drops what
/// was kept for it, and nothing is kept if the traversal halted or changed the tree. The callbacks
/// see the code as in their own traversals of the root of the code: the same nodes in the same
/// order, scopes made from the root of the code (StrictModeCheck's own callback has scopes made
/// from the root of both, which it reads only for `delete name`: after such a DELPROP nothing is
/// kept for StrictModeCheck, which then traverses as Java does).
pub(crate) struct CheckLookahead {
    js_root: NodeId,
    /// The point up to which `code_structure_changed` is known.
    structure_watched: ChangeMark,
    /// The point up to which `code_flags_changed` is known.
    flags_watched: ChangeMark,
    /// Whether the structure of the code changed since the traversal.
    code_structure_changed: bool,
    /// Whether the JSDoc or flags of the code changed since the traversal.
    code_flags_changed: bool,
    pub(crate) strict_mode: Option<StrictModeKept>,
    pub(crate) check_super: Option<CheckSuper>,
    pub(crate) check_side_effects: Option<CheckSideEffects>,
    pub(crate) check_reg_exp: Option<CheckRegExp>,
}

/// What the shared traversal keeps for StrictModeCheck: the diagnostics of its own callback in the
/// code and those of `NonExternChecks`.
pub(crate) struct StrictModeKept {
    default_level: CheckLevel,
    code_errors: Vec<JSError>,
    non_extern_errors: Vec<JSError>,
}

/// What a kept traversal may have seen change.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LookaheadReads {
    /// The structure of the code.
    CodeStructure,
    /// The structure, the JSDoc and the flags of the code.
    Code,
}

impl CheckLookahead {
    /// Takes what was kept for the pass `take` selects, while it is still valid for a pass on
    /// `root` that reads `reads`.
    pub(crate) fn take<T>(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        reads: LookaheadReads,
        take: impl FnOnce(&mut CheckLookahead) -> Option<T>,
    ) -> Option<T> {
        let mut lookahead = compiler.check_lookahead.take()?;
        let kept = take(&mut lookahead);
        let valid = kept.is_some()
            && lookahead.js_root == root
            && !lookahead.changed(compiler, false)
            && (reads == LookaheadReads::CodeStructure || !lookahead.changed(compiler, true));
        compiler.check_lookahead = Some(lookahead);
        kept.filter(|_| valid)
    }

    /// Whether the structure (or with `flags`, the JSDoc or flags) of the code changed since the
    /// traversal.
    fn changed(&mut self, compiler: &mut AbstractCompiler, flags: bool) -> bool {
        let (watched, changed) = if flags {
            (&mut self.flags_watched, &mut self.code_flags_changed)
        } else {
            (
                &mut self.structure_watched,
                &mut self.code_structure_changed,
            )
        };
        if !*changed {
            let now = SyntacticScopeCache::change_mark(compiler);
            let round = |rounds: (u32, u32, u32)| if flags { rounds.1 } else { rounds.0 };
            if round(now.rounds) != round(watched.rounds) || now.rounds.2 != watched.rounds.2 {
                *changed = match SyntacticScopeCache::nodes_changed_since(compiler, watched, flags)
                {
                    Some(nodes) => any_below(compiler, &nodes, self.js_root),
                    None => true,
                };
                *watched = now;
            }
        }
        *changed
    }
}

/// Whether any of `nodes` (from `SyntacticScopeCache::nodes_changed_since`) is `root` or below it.
pub(crate) fn any_below(ast: &Ast, nodes: &[NodeId], root: NodeId) -> bool {
    nodes.iter().any(|&n| {
        let mut node = Some(n);
        while let Some(current) = node {
            if current == root {
                return true;
            }
            node = current.get_parent(ast);
        }
        false
    })
}

/// Reports `errors` in order.
pub(crate) fn report_all(compiler: &mut AbstractCompiler, errors: Vec<JSError>) {
    for error in errors {
        compiler.report(error);
    }
}

/// With `CLOSURE_RS_CHECK_LOOKAHEAD=verify`: panics unless the diagnostics `kept` from the shared
/// traversal are those of the pass's own traversal, `own`.
pub(crate) fn verify_kept(pass: &str, kept: &[JSError], own: &[JSError]) {
    if kept != own {
        panic!(
            "CLOSURE_RS_CHECK_LOOKAHEAD=verify: {pass} reports differ\nshared traversal:\n{kept:#?}\nown traversal:\n{own:#?}"
        );
    }
}

/// StrictModeCheck's own callback over the externs only.
struct ExternsOnly<'a> {
    strict: &'a mut StrictModeCheck,
    code_root: NodeId,
}

impl Callback for ExternsOnly<'_> {
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        n != self.code_root && self.strict.should_traverse(t, n, parent)
    }

    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        self.strict.should_traverse_script(t, script, parent)
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.strict.visit(t, n, parent);
    }
}

/// A callback of the leading CombinedCompilerPass, as its `CallbackWrapper` runs it.
// port: CombinedCompilerPass.CallbackWrapper
struct LeaderCallback<'a> {
    callback: &'a mut dyn Callback,
    waiting: Option<NodeId>,
}

/// The callbacks of the shared traversal of the code (`CheckLookahead`).
struct Shared<'a> {
    leader: Vec<LeaderCallback<'a>>,
    strict: StrictModeCheck,
    non_extern: NonExternChecks,
    check_super: CheckSuper,
    check_side_effects: CheckSideEffects,
    check_reg_exp: Option<CheckRegExp>,
    /// A `delete name` was seen.
    strict_gave_up: bool,
    /// The compiler had halting errors.
    halted: bool,
}

impl Callback for Shared<'_> {
    // port: CombinedCompilerPass#shouldTraverse (for the leader's callbacks)
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if t.get_compiler().has_halting_errors() {
            self.halted = true;
            return false;
        }
        for wrapper in &mut self.leader {
            if wrapper.waiting.is_none() && !wrapper.callback.should_traverse(t, n, parent) {
                wrapper.waiting = Some(n);
            }
        }
        // StrictModeCheck's, CheckSideEffects' and CheckRegExp's callbacks traverse every node;
        // CheckSuper's does too, after noting contexts.
        let traverse = self.check_super.should_traverse(t, n, parent);
        debug_assert!(traverse);
        true
    }

    // port: CombinedCompilerPass#visit (for the leader's callbacks)
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if t.get_compiler().has_halting_errors() {
            self.halted = true;
            return;
        }
        for wrapper in &mut self.leader {
            if wrapper.waiting.is_none() {
                wrapper.callback.visit(t, n, parent);
            } else if wrapper.waiting == Some(n) {
                wrapper.waiting = None;
            }
        }
        if n.is_del_prop(t) && n.get_first_child(t).is_some_and(|c| c.is_name(t)) {
            self.strict_gave_up = true;
        }
        self.strict.visit(t, n, parent);
        self.non_extern.visit(t, n, parent);
        self.check_super.visit(t, n, parent);
        self.check_side_effects.visit(t, n, parent);
        if let Some(check_reg_exp) = &mut self.check_reg_exp {
            check_reg_exp.visit(t, n, parent);
        }
    }
}

/// Rust-only (performance divergence): the traversal of checkJsDocAndEs6Modules, a
/// CombinedCompilerPass of `callbacks`, shared with the passes that `CheckLookahead` names.
/// Returns false when it cannot lead (a scoped callback), before anything happened.
pub(crate) fn traverse_leading(
    compiler: &mut AbstractCompiler,
    callbacks: &mut [Box<dyn Callback>],
    root: NodeId,
) -> bool {
    compiler.check_lookahead = None;
    if *LOOKAHEAD_MODE == LookaheadMode::Off
        || callbacks
            .iter_mut()
            .any(|callback| callback.as_scoped_callback().is_some())
    {
        return false;
    }
    // The later passes as their factories make them.
    let options = compiler.get_options();
    let default_level = if options.expect_strict_mode_input() {
        CheckLevel::ERROR
    } else {
        CheckLevel::OFF
    };
    let mut check_side_effects = CheckSideEffects::new(
        options.get_check_suspicious_code(),
        options.should_protect_hidden_side_effects(),
    );
    let mut check_reg_exp = options.should_compute_function_side_effects().then(|| {
        CheckRegExp::new(
            options.get_assume_properties_are_statically_analyzable(),
            /* report_errors= */ true,
        )
    });
    let mut strict = StrictModeCheck::new(compiler, default_level);
    strict.capture = Some(Vec::new());
    let mut check_super = CheckSuper::new();
    check_super.capture = Some(Vec::new());
    check_side_effects.defer_externs();
    if let Some(check_reg_exp) = &mut check_reg_exp {
        check_reg_exp.capture = Some(Vec::new());
    }
    let mark = SyntacticScopeCache::change_mark(compiler);
    let mut shared = Shared {
        leader: callbacks
            .iter_mut()
            .map(|callback| LeaderCallback {
                callback: callback.as_mut(),
                waiting: None,
            })
            .collect(),
        strict,
        non_extern: NonExternChecks {
            default_level,
            capture: Some(Vec::new()),
        },
        check_super,
        check_side_effects,
        check_reg_exp,
        strict_gave_up: false,
        halted: false,
    };
    NodeTraversal::traverse(compiler, root, &mut shared);
    let Shared {
        mut strict,
        non_extern,
        check_super,
        check_side_effects,
        check_reg_exp,
        strict_gave_up,
        halted,
        ..
    } = shared;
    if halted || SyntacticScopeCache::change_rounds(compiler) != mark.rounds {
        return true;
    }
    check_super.check_context_stack_empty();
    compiler.check_lookahead = Some(CheckLookahead {
        js_root: root,
        structure_watched: mark,
        flags_watched: mark,
        code_structure_changed: false,
        code_flags_changed: false,
        strict_mode: (!strict_gave_up).then(|| StrictModeKept {
            default_level,
            code_errors: strict.capture.take().expect("captured"),
            non_extern_errors: non_extern.capture.expect("captured"),
        }),
        check_super: Some(check_super),
        check_side_effects: Some(check_side_effects),
        check_reg_exp,
    });
    true
}

impl StrictModeCheck {
    // port: StrictModeCheck#process
    fn process_as_java(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse_roots(compiler, self, externs, root);
        let mut non_extern = NonExternChecks {
            default_level: self.default_level,
            capture: self.capture.as_ref().map(|_| Vec::new()),
        };
        NodeTraversal::traverse(compiler, root, &mut non_extern);
        if let (Some(errors), Some(more)) = (&mut self.capture, non_extern.capture) {
            errors.extend(more);
        }
    }
}
