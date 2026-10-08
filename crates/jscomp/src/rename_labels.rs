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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/RenameLabels.java.

//! Port of `com.google.javascript.jscomp.RenameLabels`.
//!
//! RenameLabels renames all the labels so that they have short names, to reduce code size and
//! also to obfuscate the code.
//!
//! Label names have a unique namespace, so variable or function names clashes are not a concern,
//! but keywords clashes are.
//!
//! Additionally, labels names are only within the statements include in the label and do not
//! cross function boundaries. This means that it is possible to create one label name that is used
//! for labels at any given depth of label nesting. Typically, the name "a" will be used for all
//! top-level labels, "b" for the next nested label, and so on.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    default_name_generator::DefaultNameGenerator,
    name_generator::NameGenerator,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
};
use closure_rhino::{check_state, js_string::JsString, node::NodeId, token::Token};
use indexmap::{IndexMap, IndexSet};
use std::sync::{Arc, Mutex, RwLock};

pub struct RenameLabels {
    name_supplier: Arc<dyn Fn() -> String + Send + Sync>,
    remove_unused: bool,
    mark_changes: bool,
}

impl RenameLabels {
    // port: RenameLabels#RenameLabels(AbstractCompiler)
    pub fn new() -> Self {
        let supplier = DefaultNameSupplier::new();
        Self::with_supplier(Arc::new(move || supplier.get()), true, true)
    }

    // port: RenameLabels#RenameLabels(AbstractCompiler,Supplier,boolean,boolean)
    pub fn with_supplier(
        supplier: Arc<dyn Fn() -> String + Send + Sync>,
        remove_unused: bool,
        mark_changes: bool,
    ) -> Self {
        Self {
            name_supplier: supplier,
            remove_unused,
            mark_changes,
        }
    }

    // port: RenameLabels#process
    //
    // Java callers may pass a null externs node (FunctionToBlockMutator#makeLocalNamesUnique);
    // the CompilerPass entry point forwards here.
    pub fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        _externs: Option<NodeId>,
        root: NodeId,
    ) {
        // Do variable reference counting.
        let mut callback = ProcessLabels::new(self, self.mark_changes);
        NodeTraversal::traverse(compiler, root, &mut callback);
    }
}

impl Default for RenameLabels {
    // port: RenameLabels#RenameLabels(AbstractCompiler)
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for RenameLabels {
    // port: RenameLabels#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        RenameLabels::process(self, compiler, Some(externs), root);
    }
}

pub struct DefaultNameSupplier {
    // DefaultNameGenerator is used to create safe label names.
    name_generator: Mutex<Box<dyn NameGenerator>>,
}

impl DefaultNameSupplier {
    // port: RenameLabels.DefaultNameSupplier#DefaultNameSupplier
    fn new() -> Self {
        Self {
            name_generator: Mutex::new(Box::new(DefaultNameGenerator::with_reserved_characters(
                Arc::new(RwLock::new(IndexSet::<JsString>::new())),
                JsString::from(""),
                &IndexSet::new(),
            ))),
        }
    }

    // port: RenameLabels.DefaultNameSupplier#get
    pub fn get(&self) -> String {
        self.name_generator
            .lock()
            .unwrap()
            .generate_next_name()
            .to_string_lossy()
    }
}

/// Iterate through the nodes, renaming all the labels.
struct ProcessLabels<'a> {
    outer: &'a RenameLabels,
    mark_changes: bool,
    // A stack of labels namespaces. Labels in an outer scope aren't part of an
    // inner scope, so a new namespace is created each time a scope is entered.
    namespace_stack: Vec<LabelNamespace>,
    // The list of generated names. Typically, the first name will be "a",
    // the second "b", etc.
    names: Vec<JsString>,
}

impl<'a> ProcessLabels<'a> {
    // port: RenameLabels.ProcessLabels#ProcessLabels
    fn new(outer: &'a RenameLabels, mark_changes: bool) -> Self {
        let mut result = Self {
            outer,
            mark_changes,
            namespace_stack: Vec::new(),
            names: Vec::new(),
        };
        // Create a entry for global scope.
        result.namespace_stack.push(LabelNamespace::default());
        result
    }

    // port: RenameLabels.ProcessLabels#visitBreakOrContinue
    fn visit_break_or_continue(&mut self, t: &mut NodeTraversal<'_>, node: NodeId) {
        let name_node = node.get_first_child(t);
        if let Some(name_node) = name_node {
            // This is a named break or continue;
            let name = name_node.get_string(t);
            check_state!(!name.is_empty());
            let id = self.get_label_info(&name).map(|li| li.id);
            if let Some(id) = id {
                let new_name = self.get_name_for_id(id);
                // Mark the label as referenced so it isn't removed.
                self.get_label_info(&name).unwrap().referenced = true;
                if name != new_name {
                    // Give it the short name.
                    name_node.set_string(t, new_name);
                    if self.mark_changes {
                        t.report_code_change();
                    }
                }
            }
        }
    }

    // port: RenameLabels.ProcessLabels#visitLabel
    fn visit_label(&mut self, t: &mut NodeTraversal<'_>, node: NodeId) {
        let name_node = node.get_first_child(t);
        check_state!(name_node.is_some());
        let name_node = name_node.unwrap();
        let name = name_node.get_string(t);
        let li = self.get_label_info(&name).unwrap();
        let (referenced, id) = (li.referenced, li.id);
        // This is a label...
        if referenced || !self.outer.remove_unused {
            let new_name = self.get_name_for_id(id);
            if name != new_name {
                // ... and it is used, give it the short name.
                name_node.set_string(t, new_name);
                if self.mark_changes {
                    t.report_code_change();
                }
            }
        } else {
            // ... and it is not referenced, just remove it.
            let new_child = node.get_last_child(t).unwrap();
            new_child.detach(t);
            node.replace_with(t, new_child);
            if new_child.is_block(t) {
                NodeUtil::try_merge_block(t, new_child, false);
            }
            if self.mark_changes {
                t.report_code_change();
            }
        }

        // Remove the label from the current stack of labels.
        self.namespace_stack
            .last_mut()
            .unwrap()
            .rename_map
            .shift_remove(&name);
    }

    // port: RenameLabels.ProcessLabels#getNameForId
    fn get_name_for_id(&self, id: i32) -> JsString {
        self.names[(id - 1) as usize].clone()
    }

    // port: RenameLabels.ProcessLabels#getLabelInfo
    fn get_label_info(&mut self, name: &JsString) -> Option<&mut LabelInfo> {
        self.namespace_stack
            .last_mut()
            .unwrap()
            .rename_map
            .get_mut(name)
    }
}

impl Callback for ProcessLabels<'_> {
    // port: RenameLabels.ProcessLabels#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        node: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if node.is_label(node_traversal) {
            // Determine the new name for this label.
            let current = self.namespace_stack.last_mut().unwrap();
            let current_depth = current.rename_map.len() as i32 + 1;
            let name = node
                .get_first_child(node_traversal)
                .unwrap()
                .get_string(node_traversal);

            // Store the context for this label name.
            let li = LabelInfo::new(current_depth);
            check_state!(!current.rename_map.contains_key(&name));
            current.rename_map.insert(name, li);

            // Create a new name, if needed, for this depth.
            if (self.names.len() as i32) < current_depth {
                self.names
                    .push(JsString::from((self.outer.name_supplier)()));
            }
        }

        true
    }

    // port: RenameLabels.ProcessLabels#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, node: NodeId, _parent: Option<NodeId>) {
        match node.get_token(t) {
            Token::LABEL => self.visit_label(t, node),
            Token::BREAK | Token::CONTINUE => self.visit_break_or_continue(t, node),
            _ => {}
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for ProcessLabels<'_> {
    // port: RenameLabels.ProcessLabels#enterScope
    fn enter_scope(&mut self, node_traversal: &mut NodeTraversal<'_>) {
        // Start a new namespace for label names.
        if node_traversal
            .get_scope_root()
            .unwrap()
            .is_function(node_traversal)
        {
            self.namespace_stack.push(LabelNamespace::default());
        }
    }

    // port: RenameLabels.ProcessLabels#exitScope
    fn exit_scope(&mut self, node_traversal: &mut NodeTraversal<'_>) {
        if node_traversal
            .get_scope_root()
            .unwrap()
            .is_function(node_traversal)
        {
            self.namespace_stack.pop();
        }
    }
}

struct LabelInfo {
    referenced: bool,
    id: i32,
}

impl LabelInfo {
    // port: RenameLabels.LabelInfo#LabelInfo
    fn new(id: i32) -> Self {
        Self {
            referenced: false,
            id,
        }
    }
}

#[derive(Default)]
struct LabelNamespace {
    rename_map: IndexMap<JsString, LabelInfo>,
}
