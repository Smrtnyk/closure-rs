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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/GatherRawExports.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of GatherRawExports.java.
//!
//! Collect known properties on the global windows object to avoid creating global variable names
//! that conflict with these names. This pass runs after property renaming but before variable
//! renaming.
//!
//! This is a best effort pass and does not guarantee that there are no conflicts.
//!
//! This is not required if the global variables are isolated from global scope
//! (isolation_mode=IIFE) or equivalent.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{check_state, js_string::JsString, node::NodeId};

pub struct GatherRawExports {
    exported_variables: IndexSet<JsString>,
}

impl GatherRawExports {
    // TODO(johnlenz): "goog$global" should be part of a coding convention.
    // Note: GatherRawExports runs after property renaming and
    // collapse properties, so the two entries here protect goog.global in the
    // two common cases "collapse properties and renaming on" or both off
    // but not the case where only property renaming is on.
    // port: GatherRawExports#GLOBAL_THIS_NAMES
    const GLOBAL_THIS_NAMES: [&'static str; 8] = [
        "window",
        "globalThis",
        "top",
        "self",
        "goog$global",
        "goog.global",
        "$jscomp.global",
        "$jscomp$global",
    ];

    // port: GatherRawExports#GatherRawExports
    pub fn new() -> Self {
        Self {
            exported_variables: IndexSet::<_>::default(),
        }
    }

    // port: GatherRawExports#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        NodeTraversal::traverse(compiler, root, self);
    }

    // port: GatherRawExports#isGlobalThisObject
    fn is_global_this_object(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        // We should consider using scopes or types to check for references to the "global this".
        if n.is_this(t) {
            return t.in_global_hoist_scope();
        } else if n.is_qualified_name(t) {
            let items = Self::GLOBAL_THIS_NAMES.len();
            for i in 0..items {
                if n.matches_qualified_name(t, Self::GLOBAL_THIS_NAMES[i]) {
                    return true;
                }
            }
        }
        false
    }

    // port: GatherRawExports#getExportedVariableNames
    pub fn get_exported_variable_names(&self) -> &IndexSet<JsString> {
        &self.exported_variables
    }
}

impl Default for GatherRawExports {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for GatherRawExports {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        GatherRawExports::process(self, compiler, externs, root);
    }
}

impl Callback for GatherRawExports {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: GatherRawExports#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if NodeUtil::is_normal_or_opt_chain_get(t, n)
            && Self::is_global_this_object(t, n.get_first_child(t).unwrap())
        {
            if NodeUtil::is_normal_or_opt_chain_get_prop(t, n) {
                self.exported_variables.insert(n.get_string(t));
            } else if NodeUtil::is_normal_or_opt_chain_get(t, n)
                && n.get_second_child(t).unwrap().is_string_lit(t)
            {
                self.exported_variables
                    .insert(n.get_second_child(t).unwrap().get_string(t));
            }
        }
    }
}
