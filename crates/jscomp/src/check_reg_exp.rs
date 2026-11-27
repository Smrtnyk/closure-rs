/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckRegExp.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    parallel_checks::{self, CheckContext, Collector, ReadOnlyCallback},
};
use closure_regex::reg_exp_tree::RegExpTree;
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};

// port: CheckRegExp#REGEXP_REFERENCE
pub static REGEXP_REFERENCE: DiagnosticType = DiagnosticType::warning(
    "JSC_REGEXP_REFERENCE",
    "References to the global RegExp object prevents optimization of regular expressions.",
);

// port: CheckRegExp#MALFORMED_REGEXP
pub static MALFORMED_REGEXP: DiagnosticType =
    DiagnosticType::warning("JSC_MALFORMED_REGEXP", "Malformed Regular Expression: {0}");

// port: CheckRegExp#REGEXP_PROPERTY_SKIPLIST
const REGEXP_PROPERTY_SKIPLIST: [&str; 21] = [
    "$1",
    "$2",
    "$3",
    "$4",
    "$5",
    "$6",
    "$7",
    "$8",
    "$9",
    "$_",
    "$input",
    // The following would also be skiplisted, but they aren't valid
    // identifiers, so can't be accessed with the '.' operator anyway.
    // "$*", "$&", "$+", "$`", "$'",
    "input",
    "lastMatch",
    "lastParen",
    "leftContext",
    "rightContext",
    "global",
    "ignoreCase",
    "lastIndex",
    "multiline",
    "source",
];

/// Look for references to the global RegExp object that would cause regular expressions to be
/// unoptimizable, and checks that regular expressions are syntactically valid.
pub struct CheckRegExp {
    report_errors: bool,
    global_reg_exp_properties_used: bool,
    assume_all_global_regexp_usages_visible: bool,
}

impl CheckRegExp {
    // port: CheckRegExp#isGlobalRegExpPropertiesUsed
    pub fn is_global_reg_exp_properties_used(&self) -> bool {
        if self.assume_all_global_regexp_usages_visible {
            return self.global_reg_exp_properties_used;
        }
        true
    }

    // port: CheckRegExp#CheckRegExp
    pub fn new(assume_all_global_regexp_usages_visible: bool, report_errors: bool) -> Self {
        Self {
            assume_all_global_regexp_usages_visible,
            report_errors,
            global_reg_exp_properties_used: false,
        }
    }
}

/// Typed borrowing view for java.lang.reflect.Field replay; the instance fields remain private.
pub struct CheckRegExpReplayFields<'a> {
    pub report_errors: &'a bool,
    pub global_reg_exp_properties_used: &'a bool,
    pub assume_all_global_regexp_usages_visible: &'a bool,
}

impl CheckRegExp {
    // port: java.lang.reflect.Field#get (native replay access)
    pub fn replay_fields(&self) -> CheckRegExpReplayFields<'_> {
        CheckRegExpReplayFields {
            report_errors: &self.report_errors,
            global_reg_exp_properties_used: &self.global_reg_exp_properties_used,
            assume_all_global_regexp_usages_visible: &self.assume_all_global_regexp_usages_visible,
        }
    }
}

impl CheckRegExp {
    /// Rust-only (performance divergence): `process` with the statements walked on worker
    /// threads (see `parallel_checks`); false when a walk gave up (a reference to `RegExp` whose
    /// outcome depends on the scope), before anything was reported.
    fn process_in_parallel(&mut self, compiler: &mut AbstractCompiler, root: NodeId) -> bool {
        let fresh = || {
            CheckRegExp::new(
                self.assume_all_global_regexp_usages_visible,
                self.report_errors,
            )
        };
        let mut main = fresh();
        let no_suffix = |_: &AbstractCompiler, _: NodeId| None;
        let Some(mut walked) = (parallel_checks::Run {
            main: &mut main,
            script_suffix: &no_suffix,
        })
        .walk(compiler, &[root], |_| fresh()) else {
            return false;
        };
        let used = main.global_reg_exp_properties_used
            || walked
                .take_unit_callbacks()
                .iter()
                .any(|unit| unit.global_reg_exp_properties_used);
        if *parallel_checks::MODE == parallel_checks::Mode::Verify {
            walked.verify(compiler, "CheckRegExp", false, |compiler| {
                self.process_serially(compiler, root);
            });
            assert_eq!(
                self.global_reg_exp_properties_used, used,
                "CLOSURE_RS_PARALLEL_CHECKS=verify: CheckRegExp globalRegExpPropertiesUsed: \
                 traversal {}, parallel walk {used}",
                self.global_reg_exp_properties_used
            );
        } else {
            walked.report(compiler, false);
            self.global_reg_exp_properties_used |= used;
        }
        true
    }

    fn process_serially(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }

    // port: CheckRegExp#visit
    /// Rust-only: the scope is asked whether it has `RegExp` only when the use of the name is one
    /// that counts (Java asks first); the lookup has no effect on the result.
    #[allow(clippy::nonminimal_bool)] // Retain Java's condition.
    fn visit_impl<C: CheckContext>(&mut self, t: &mut C, n: NodeId, parent: Option<NodeId>) {
        if NodeUtil::is_reference_name(t, n) {
            let name = n.get_string(t);
            if name == "RegExp" {
                let parent = parent.unwrap();
                let parent_type = parent.get_token(t);
                let first = Some(n) == parent.get_first_child(t);
                if !((parent_type == Token::NEW && first)
                    || (parent_type == Token::CALL && first)
                    || (parent_type == Token::INSTANCEOF && !first)
                    || parent_type == Token::EQ
                    || parent_type == Token::NE
                    || parent_type == Token::SHEQ
                    || parent_type == Token::SHNE
                    || parent_type == Token::CASE
                    || (parent_type == Token::GETPROP && first && {
                        let parent_string = parent.get_string(t);
                        !REGEXP_PROPERTY_SKIPLIST
                            .iter()
                            .any(|skipped| parent_string == *skipped)
                    }))
                    && t.has_var(&name) == Some(false)
                {
                    if self.report_errors {
                        t.report(n, &REGEXP_REFERENCE, &[]);
                    }
                    self.global_reg_exp_properties_used = true;
                }
            }

            // Check the syntax of regular expression patterns.
        } else if self.report_errors && n.is_reg_exp(t) {
            let pattern = n.get_first_child(t).unwrap().get_string(t);
            let flags = if n.has_two_children(t) {
                n.get_last_child(t).unwrap().get_string(t)
            } else {
                JsString::from("")
            };
            if let Err(ex) = RegExpTree::parse_reg_exp(&pattern, &flags) {
                // Throwable#getMessage may be null; MessageFormat prints it as "null".
                let message = ex
                    .get_message()
                    .map_or_else(|| "null".to_owned(), ToString::to_string);
                t.report(n, &MALFORMED_REGEXP, &[message.as_str()]);
            }
        }
    }
}

impl CompilerPass for CheckRegExp {
    // port: CheckRegExp#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        if *parallel_checks::MODE != parallel_checks::Mode::Off
            && self.process_in_parallel(compiler, root)
        {
            return;
        }
        self.process_serially(compiler, root);
    }
}

impl Callback for CheckRegExp {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckRegExp#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.visit_impl(t, n, parent);
    }
}

impl ReadOnlyCallback for CheckRegExp {
    fn should_traverse(&mut self, _cx: &mut Collector<'_>, _n: NodeId, _: Option<NodeId>) -> bool {
        true
    }

    fn visit(&mut self, cx: &mut Collector<'_>, n: NodeId, parent: Option<NodeId>) {
        self.visit_impl(cx, n, parent);
    }
}
