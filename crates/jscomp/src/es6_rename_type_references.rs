/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Es6RenameTypeReferences.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Renames references in JSDoc.
use crate::{
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{check_state, js_string::JsString, node::NodeId};

/// Rust-only: Java's `Table<Node, String, String>` (a Guava `HashBasedTable`, used only through
/// `put`, `get` and `isEmpty`): (scope root, original name) -> new name.
pub type RenameTable = IndexMap<(NodeId, JsString), JsString>;

/// Renames references in JSDoc.
// port: Es6RenameTypeReferences
pub struct Es6RenameTypeReferences<'a> {
    /// Map from the root node of a scope + the original variable name to a new name for the
    /// variable.
    ///
    /// An entry in this map means we should rename a variable that was originally declared in the
    /// scope with the given root and had the original variable name to the new name.
    rename_table: &'a RenameTable,
}

impl<'a> Es6RenameTypeReferences<'a> {
    // port: Es6RenameTypeReferences#Es6RenameTypeReferences
    pub fn new(rename_table: &'a RenameTable) -> Self {
        Self { rename_table }
    }

    // port: Es6RenameTypeReferences#renameTypeNodeRecursive
    fn rename_type_node_recursive(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if n.is_string_lit(t) {
            self.rename_type_reference(t, n);
        }

        let mut child = n.get_first_child(t);
        while let Some(c) = child {
            self.rename_type_node_recursive(t, c);
            child = c.get_next(t);
        }
    }

    // port: Es6RenameTypeReferences#renameTypeReference
    fn rename_type_reference(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        check_state!(n.is_string_lit(t));
        let full_name = n.get_string(t);
        let root_name;
        let rest;
        let end_pos = full_name.index_of_char(u16::from(b'.'));
        if end_pos == -1 {
            root_name = full_name;
            rest = None;
        } else {
            root_name = full_name.substring(0, end_pos as usize);
            rest = Some(full_name.substring_from(end_pos as usize));
        }
        self.rename_reference(t, n, &root_name, rest.as_ref());
    }

    /// `old_name` is the root name of a qualified name; `rest` the rest of the qualified name or
    /// null.
    // port: Es6RenameTypeReferences#renameReference
    fn rename_reference(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        old_name: &JsString,
        rest: Option<&JsString>,
    ) {
        let mut current = Some(t.get_scope());

        // You should be wondering:
        //
        // Why are we searching up the stack of scopes here instead of just looking up the Var for
        // oldName and getting the scope from that?
        //
        // The answer is:  The Var no longer exists.
        //
        // Es6RewriteModules (the only client for this class) actually deletes the module-level
        // declaration for the old name but still uses the scope rooted at the MODULE_BODY (for
        // goog.module() files), SCRIPT, or function body (for wrapped goog.loadModule()
        // goog.module()s) in the rename map.
        //
        // If we're in some local scope where there's a shadowing local variable, we don't want to
        // do the renaming.
        //
        // Otherwise, we need to use the MODULE_BODY or SCRIPT or goog.loadModule body scope to
        // look up the new name.
        // - bradfordcsmith@google.com
        while let Some(cur) = current {
            let current_root_node = cur.get_root_node(t.get_compiler());
            let new_name = self
                .rename_table
                .get(&(current_root_node, old_name.clone()));
            if let Some(new_name) = new_name {
                check_state!(
                    current_root_node.is_module_body(t)
                        || current_root_node.is_script(t)
                        || NodeUtil::is_function_block(t, current_root_node),
                    "Not a MODULE_BODY or SCRIPT or goog.loadModule root: %s",
                    current_root_node.to_string(t)
                );
                let new_full_name = match rest {
                    None => new_name.clone(),
                    Some(rest) => new_name.concat(rest),
                };
                n.set_string(t, new_full_name);
                return;
            } else if cur.has_own_slot(t.get_compiler(), old_name.clone()) {
                // This is a reference to some local variable whose name shadows the module-scope
                // variable we actually want to rename.
                return;
            } else {
                current = cur.get_parent(t.get_compiler());
            }
        }
    }
}

impl Callback for Es6RenameTypeReferences<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RenameTypeReferences#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        // Remove this branch after module rewriting is always done
        // after type checking.
        let info = n.get_jsdoc_info(t);
        if let Some(info) = info {
            for root in info.get_type_nodes() {
                self.rename_type_node_recursive(t, root);
            }
        }
    }
}
