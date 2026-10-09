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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CheckRegExp.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
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

impl CompilerPass for CheckRegExp {
    // port: CheckRegExp#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
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
    #[allow(clippy::nonminimal_bool)] // Retain Java's condition.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if NodeUtil::is_reference_name(t, n) {
            let name = n.get_string(t);
            if name == "RegExp" && {
                let scope = t.get_scope();
                scope.get_var(t.get_compiler(), name.clone()).is_none()
            } {
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
