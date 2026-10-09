/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckProvidesSorted.java.

//! Checks that goog.provide statements are sorted and deduplicated, exposing the necessary
//! information to produce a suggested fix.

use crate::{
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};

// port: CheckProvidesSorted#PROVIDES_NOT_SORTED
pub static PROVIDES_NOT_SORTED: DiagnosticType = DiagnosticType::warning(
    "JSC_PROVIDES_NOT_SORTED",
    "goog.provide() statements are not sorted. The correct order is:\n\n{0}\n",
);

/// Operation modes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Collect information to determine whether a fix is required, but do not report a warning.
    COLLECT_ONLY,
    /// Additionally report a warning.
    COLLECT_AND_REPORT,
}

pub struct CheckProvidesSorted {
    mode: Mode,

    // The provided namespaces in the order they appear.
    original_provides: Vec<JsString>,

    // The provided namespaces in canonical order.
    first_node: Option<NodeId>,
    last_node: Option<NodeId>,
    finished: bool,

    replacement: Option<String>,
    needs_fix: bool,
}

impl CheckProvidesSorted {
    // port: CheckProvidesSorted#CheckProvidesSorted
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            original_provides: Vec::new(),
            first_node: None,
            last_node: None,
            finished: false,
            replacement: None,
            needs_fix: false,
        }
    }

    /// Returns the node for the first recognized provide statement.
    // port: CheckProvidesSorted#getFirstNode
    pub fn get_first_node(&self) -> Option<NodeId> {
        self.first_node
    }

    /// Returns the node for the last recognized provide statement.
    // port: CheckProvidesSorted#getLastNode
    pub fn get_last_node(&self) -> Option<NodeId> {
        self.last_node
    }

    /// Returns a textual replacement yielding a canonical version of the provides.
    // port: CheckProvidesSorted#getReplacement
    pub fn get_replacement(&self) -> Option<&str> {
        self.replacement.as_deref()
    }

    /// Returns whether the provides need to be fixed, i.e., whether they are *not* already
    /// canonical.
    // port: CheckProvidesSorted#needsFix
    pub fn needs_fix(&self) -> bool {
        self.needs_fix
    }

    // port: CheckProvidesSorted#areProvideArgumentsValid
    fn are_provide_arguments_valid(ast: &Ast, call: NodeId) -> bool {
        call.has_two_children(ast) && call.get_second_child(ast).unwrap().is_string_lit(ast)
    }

    // port: CheckProvidesSorted#getNamespace
    fn get_namespace(ast: &Ast, n: NodeId) -> JsString {
        n.get_first_child(ast)
            .unwrap()
            .get_second_child(ast)
            .unwrap()
            .get_string(ast)
    }

    /// Returns the code for a correctly formatted provide call.
    // port: CheckProvidesSorted#formatProvide
    fn format_provide(namespace: &JsString) -> String {
        let mut sb = String::new();

        sb.push_str("goog.provide('");
        sb.push_str(&namespace.to_string_lossy());
        sb.push_str("');");

        sb
    }

    // port: CheckProvidesSorted#checkCanonical
    fn check_canonical(&mut self, t: &mut NodeTraversal<'_>) {
        // stream().sorted().distinct(): String natural order (JsString's Ord is compareTo).
        let mut canonical_provides = self.original_provides.clone();
        canonical_provides.sort();
        canonical_provides.dedup();
        if self.original_provides != canonical_provides {
            self.needs_fix = true;
            let replacement = canonical_provides
                .iter()
                .map(Self::format_provide)
                .collect::<Vec<_>>()
                .join("\n");
            if self.mode == Mode::COLLECT_AND_REPORT {
                t.report(
                    self.first_node.unwrap(),
                    &PROVIDES_NOT_SORTED,
                    &[&replacement],
                );
            }
            self.replacement = Some(replacement);
        }
    }
}

impl Callback for CheckProvidesSorted {
    // port: CheckProvidesSorted#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        _n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // Traverse top-level statements until a block of contiguous provides is found.
        !self.finished
            && parent.is_none_or(|parent| {
                parent.is_root(t) || parent.is_script(t) || parent.is_module_body(t)
            })
    }

    // port: CheckProvidesSorted#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_script(t) {
            self.check_canonical(t);
            return;
        }

        if NodeUtil::is_goog_provide_call(t, n)
            && Self::are_provide_arguments_valid(t, n.get_first_child(t).unwrap())
        {
            self.original_provides.push(Self::get_namespace(t, n));
            if self.first_node.is_none() {
                self.first_node = Some(n);
                self.last_node = Some(n);
            } else {
                self.last_node = Some(n);
            }
        } else if !self.original_provides.is_empty() {
            self.finished = true;
        }
    }
}
