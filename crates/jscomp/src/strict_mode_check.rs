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
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::parallel_checks::{self, CheckContext, Collector, ReadOnlyCallback};
use closure_jstype::prelude::*;
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;

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
}

impl StrictModeCheck {
    // port: StrictModeCheck#StrictModeCheck
    pub fn new(_compiler: &AbstractCompiler, default_level: CheckLevel) -> Self {
        Self { default_level }
    }

    /// Reports a warning for with statements.
    // port: StrictModeCheck#checkWith
    fn check_with<C: CheckContext>(&self, cx: &mut C, n: NodeId) {
        let info = n.get_jsdoc_info(cx);
        let allow_with = info
            .as_ref()
            .is_some_and(|info| info.get_suppressions().contains(&JsString::from("with")));
        if !allow_with {
            self.report(cx, n, &USE_OF_WITH, &[]);
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
    fn check_assignment<C: CheckContext>(&self, cx: &mut C, n: NodeId) {
        let first = n.get_first_child(cx).unwrap();
        if first.is_name(cx) {
            if first.get_string_ref(cx) == "arguments" {
                self.report(cx, n, &ARGUMENTS_ASSIGNMENT, &[]);
            } else if first.get_string_ref(cx) == "eval" {
                // Note that assignment to eval is already illegal because any use of
                // that name is illegal.
                self.report(cx, n, &EVAL_ASSIGNMENT, &[]);
            }
        }
    }

    /// Checks that variables, functions, and arguments are not deleted.
    // port: StrictModeCheck#checkDelete
    fn check_delete<C: CheckContext>(&self, cx: &mut C, n: NodeId) {
        let first = n.get_first_child(cx).unwrap();
        if first.is_name(cx) {
            let name = first.get_string(cx);
            if cx.has_var(&name) == Some(true) {
                self.report(cx, n, &DELETE_VARIABLE, &[]);
            }
        }
    }

    /// Checks that object literal keys or class method names are valid.
    // port: StrictModeCheck#checkObjectLiteralOrClass
    fn check_object_literal_or_class<C: CheckContext>(&self, cx: &mut C, n: NodeId) {
        let mut getters: IndexSet<JsString> = IndexSet::<_>::default();
        let mut setters: IndexSet<JsString> = IndexSet::<_>::default();
        let mut static_getters: IndexSet<JsString> = IndexSet::<_>::default();
        let mut static_setters: IndexSet<JsString> = IndexSet::<_>::default();

        /*
         * Iterate backwards because the last duplicate is the one that will be used in sloppy or
         * ES6 code. The earlier duplicates are the ones that should be removed.
         */
        let mut key = n.get_last_child(cx);
        while let Some(k) = key {
            if k.is_empty(cx)
                || k.is_computed_prop(cx)
                || k.is_spread(cx)
                || k.is_computed_field_def(cx)
                // Computed properties cannot be computed at compile time
                || NodeUtil::is_class_static_block(cx, k)
            // Will not check since whether duplicates are declared/assigned cannot be determined
            // at compile time since we do not know which code will be run
            {
                key = k.get_previous(cx);
                continue;
            }

            let key_name = k.get_string(cx);
            if !k.is_setter_def(cx) {
                // normal property and getter cases
                let set = if k.is_static_member(cx) {
                    &mut static_getters
                } else {
                    &mut getters
                };
                if !set.insert(key_name.clone()) {
                    self.report(cx, k, &DUPLICATE_MEMBER, &[&key_name.to_string()]);
                }
            }
            if !k.is_getter_def(cx) {
                // normal property and setter cases
                let set = if k.is_static_member(cx) {
                    &mut static_setters
                } else {
                    &mut setters
                };
                if !set.insert(key_name.clone()) {
                    self.report(cx, k, &DUPLICATE_MEMBER, &[&key_name.to_string()]);
                }
            }
            key = k.get_previous(cx);
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
    fn report<C: CheckContext>(
        &self,
        cx: &mut C,
        n: NodeId,
        diagnostic: &'static DiagnosticType,
        args: &[&str],
    ) {
        let error = JSError::builder(diagnostic, args)
            .set_level(self.default_level)
            .set_node(cx, n)
            .build();
        cx.report_error(error);
    }

    // port: StrictModeCheck#visit
    fn visit_impl<C: CheckContext>(&self, cx: &mut C, n: NodeId) {
        if n.is_assign(cx) {
            self.check_assignment(cx, n);
        } else if n.is_del_prop(cx) {
            self.check_delete(cx, n);
        } else if n.is_object_lit(cx) {
            self.check_object_literal_or_class(cx, n);
        } else if n.is_class(cx) {
            let last = n.get_last_child(cx).unwrap();
            self.check_object_literal_or_class(cx, last);
        } else if n.is_with(cx) {
            self.check_with(cx, n);
        }
    }

    /// Rust-only (performance divergence): both traversals of `process` on worker threads (see
    /// `parallel_checks`); false when a walk gave up, before anything was reported.
    fn process_in_parallel(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) -> bool {
        let skip_bundled = *crate::variable_reference_check::SKIP_BUNDLED_EXTERNS
            // `NodeTraversal::bundled_script_prefix`: no records with synthetic block markers.
            && compiler
                .get_options_opt()
                .is_some_and(|options| options.get_synthetic_block_start_marker().is_none());
        let bundled_suffix = |compiler: &AbstractCompiler, script: NodeId| {
            if !skip_bundled {
                return None;
            }
            let (facts, first) = compiler
                .parsed_script_record::<crate::source_file::bundled_script::BundledScript>(
                    script,
                    closure_rhino::node::SnapshotUse::SyntaxAndNames,
                )?;
            facts.normalize_clean.then_some(first)
        };
        let default_level = self.default_level;
        let mut main = StrictModeCheck { default_level };
        let Some(all_code) = (parallel_checks::Run {
            main: &mut main,
            script_suffix: &bundled_suffix,
        })
        .walk(compiler, &[externs, root], |_| StrictModeCheck {
            default_level,
        }) else {
            return false;
        };
        let mut main_non_extern = NonExternChecks { outer: self };
        let no_suffix = |_: &AbstractCompiler, _: NodeId| None;
        let Some(non_extern) = (parallel_checks::Run {
            main: &mut main_non_extern,
            script_suffix: &no_suffix,
        })
        .walk(compiler, &[root], |_| NonExternChecks { outer: self }) else {
            return false;
        };
        let mut errors = all_code.ordered_errors();
        errors.extend(non_extern.ordered_errors());
        if *parallel_checks::MODE == parallel_checks::Mode::Verify {
            parallel_checks::verify_reports(compiler, "StrictModeCheck", &errors, |compiler| {
                self.process_serially(compiler, externs, root);
            });
        } else {
            parallel_checks::report_all(compiler, errors);
        }
        true
    }

    fn process_serially(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse_roots(compiler, self, externs, root);
        NodeTraversal::traverse(compiler, root, &mut NonExternChecks { outer: self });
    }
}

impl CompilerPass for StrictModeCheck {
    // port: StrictModeCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        if *parallel_checks::MODE != parallel_checks::Mode::Off
            && self.process_in_parallel(compiler, externs, root)
        {
            return;
        }
        self.process_serially(compiler, externs, root);
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
        self.visit_impl(t, n);
    }
}

impl ReadOnlyCallback for StrictModeCheck {
    fn should_traverse(&mut self, _cx: &mut Collector<'_>, _n: NodeId, _: Option<NodeId>) -> bool {
        true
    }

    fn visit(&mut self, cx: &mut Collector<'_>, n: NodeId, _parent: Option<NodeId>) {
        self.visit_impl(cx, n);
    }
}

/// Checks that are performed on non-extern code only.
struct NonExternChecks<'a> {
    outer: &'a StrictModeCheck,
}

impl NonExternChecks<'_> {
    /// Checks for illegal declarations.
    // port: StrictModeCheck.NonExternChecks#checkDeclaration
    fn check_declaration<C: CheckContext>(&self, cx: &mut C, n: NodeId) {
        if n.get_string_ref(cx) == "eval" {
            self.outer.report(cx, n, &EVAL_DECLARATION, &[]);
        } else if n.get_string_ref(cx) == "arguments" {
            self.outer.report(cx, n, &ARGUMENTS_DECLARATION, &[]);
        }
    }

    /// Checks that the arguments.callee is not used.
    // port: StrictModeCheck.NonExternChecks#checkGetProp
    fn check_get_prop<C: CheckContext>(
        &self,
        cx: &mut C,
        n: NodeId,
        is_function_type: &mut impl FnMut(&mut C, NodeId) -> Option<bool>,
    ) {
        let target = n.get_first_child(cx).unwrap();
        let name = n.get_string(cx);
        if name == "callee" {
            if target.is_name(cx) && target.get_string_ref(cx) == "arguments" {
                self.outer.report(cx, n, &ARGUMENTS_CALLEE_FORBIDDEN, &[]);
            }
        } else if name == "caller" {
            if target.is_name(cx) && target.get_string_ref(cx) == "arguments" {
                self.outer.report(cx, n, &ARGUMENTS_CALLER_FORBIDDEN, &[]);
            } else if is_function_type(cx, target) == Some(true) {
                self.outer.report(cx, n, &FUNCTION_CALLER_FORBIDDEN, &[]);
            }
        } else if name == "arguments" && is_function_type(cx, target) == Some(true) {
            self.outer
                .report(cx, n, &FUNCTION_ARGUMENTS_PROP_FORBIDDEN, &[]);
        }
    }
}

impl Callback for NonExternChecks<'_> {
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
            self.check_declaration(t, n);
        } else if n.is_get_prop(t) {
            self.check_get_prop(t, n, &mut |t, target| {
                Some(StrictModeCheck::is_function_type(t.get_compiler(), target))
            });
        }
    }
}

impl ReadOnlyCallback for NonExternChecks<'_> {
    fn should_traverse(&mut self, _cx: &mut Collector<'_>, _n: NodeId, _: Option<NodeId>) -> bool {
        true
    }

    /// `visit` on a worker, which gives up when `isFunctionType` needs the type registry (a node
    /// with a JSType).
    fn visit(&mut self, cx: &mut Collector<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(cx) && StrictModeCheck::is_declaration(cx, n) {
            self.check_declaration(cx, n);
        } else if n.is_get_prop(cx) {
            self.check_get_prop(cx, n, &mut |cx, target| {
                if target.get_jstype(cx).is_none() {
                    Some(false)
                } else {
                    cx.give_up();
                    None
                }
            });
        }
    }
}
