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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
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
    change_tracker::ChangeTracker,
    compiler_pass::CompilerPass,
    default_name_generator::DefaultNameGenerator,
    name_generator::NameGenerator,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
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
        if *DIRECT_MODE != DirectMode::Off
            && let Some(labels) = outermost_labels(compiler, root)
        {
            if *DIRECT_MODE == DirectMode::Verify {
                callback.expected_labels = Some((labels, 0));
                NodeTraversal::traverse(compiler, root, &mut callback);
                let (labels, next) = callback.expected_labels.as_ref().unwrap();
                assert_eq!(
                    *next,
                    labels.len(),
                    "CLOSURE_RS_RENAME_LABELS_DIRECT=verify: the traversal entered {next} \
                     outermost labels, the candidates were {}",
                    labels.len()
                );
                return;
            }
            for label in labels {
                let change_scope = ChangeTracker::get_enclosing_change_scope_root(
                    compiler,
                    label.get_parent(compiler),
                )
                .unwrap();
                let parent = label.get_parent(compiler);
                callback.walk(compiler, label, parent, change_scope);
            }
            return;
        }
        NodeTraversal::traverse(compiler, root, &mut callback);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_RENAME_LABELS_DIRECT=off` traverses the whole tree as Java does; `=verify` does too
/// and checks that the labels it enters without a label around them are the candidates, in order.
static DIRECT_MODE: std::sync::LazyLock<DirectMode> = std::sync::LazyLock::new(|| {
    match std::env::var("CLOSURE_RS_RENAME_LABELS_DIRECT").as_deref() {
        Ok("off") => DirectMode::Off,
        Ok("verify") => DirectMode::Verify,
        _ => DirectMode::On,
    }
});

/// At most this many outermost labels are put in order without a traversal.
const MAX_DIRECT_LABELS: usize = 256;

/// Rust-only (performance divergence): the LABELs under `root` without a LABEL above them, in the
/// order Java's traversal reaches them, or `None` when there are more than `MAX_DIRECT_LABELS`.
/// The traversal visits children in the order of the tree, except under CLASS_MEMBERS, whose
/// computed keys it traverses first (`handle_class_members`).
fn outermost_labels(compiler: &mut AbstractCompiler, root: NodeId) -> Option<Vec<NodeId>> {
    let labels: Vec<NodeId> = compiler
        .ast
        .with_token_candidates(&[Token::LABEL], |ast, nodes| {
            let mut labels = Vec::new();
            for &n in nodes {
                if !n.is_label(ast) {
                    continue;
                }
                // Attached under `root`, with no LABEL above it.
                let mut outermost = true;
                let mut current = n.get_parent(ast);
                let attached = loop {
                    match current {
                        None => break false,
                        Some(c) if c == root => break true,
                        Some(c) => {
                            outermost &= !c.is_label(ast);
                            current = c.get_parent(ast);
                        }
                    }
                };
                if attached && outermost {
                    labels.push(n);
                }
            }
            labels
        });
    if labels.len() > MAX_DIRECT_LABELS {
        return None;
    }
    let ast: &Ast = &compiler.ast;
    // The traversal's order: by the position of each ancestor among its siblings, where a member
    // of class members is ordered after every computed key unless the label is in its key.
    const AFTER_KEYS: u32 = 1 << 31;
    let mut keyed: Vec<(Vec<u32>, NodeId)> = labels
        .into_iter()
        .map(|label| {
            let mut path = Vec::new();
            let mut below = label;
            let mut n = label;
            while n != root {
                let parent = n.get_parent(ast).unwrap();
                let mut index = 0u32;
                let mut sibling = parent.get_first_child(ast);
                while sibling != Some(n) {
                    index += 1;
                    sibling = sibling.unwrap().get_next(ast);
                }
                if parent.is_class_members(ast)
                    && !((n.is_computed_prop(ast) || n.is_computed_field_def(ast))
                        && n.get_first_child(ast) == Some(below))
                {
                    index |= AFTER_KEYS;
                }
                path.push(index);
                below = n;
                n = parent;
            }
            path.reverse();
            (path, label)
        })
        .collect();
    keyed.sort();
    Some(keyed.into_iter().map(|(_, label)| label).collect())
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
                Arc::new(RwLock::new(IndexSet::<JsString>::default())),
                JsString::from(""),
                &IndexSet::<_>::default(),
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
    /// Rust-only: under `CLOSURE_RS_RENAME_LABELS_DIRECT=verify`, the outermost labels found
    /// without a traversal and how many of them the traversal has entered.
    expected_labels: Option<(Vec<NodeId>, usize)>,
}

impl<'a> ProcessLabels<'a> {
    // port: RenameLabels.ProcessLabels#ProcessLabels
    fn new(outer: &'a RenameLabels, mark_changes: bool) -> Self {
        let mut result = Self {
            outer,
            mark_changes,
            namespace_stack: Vec::new(),
            names: Vec::new(),
            expected_labels: None,
        };
        // Create a entry for global scope.
        result.namespace_stack.push(LabelNamespace::default());
        result
    }

    // port: RenameLabels.ProcessLabels#visitBreakOrContinue
    fn visit_break_or_continue(&mut self, t: &mut NodeTraversal<'_>, node: NodeId) {
        if self.break_or_continue_changed(t, node) && self.mark_changes {
            t.report_code_change();
        }
    }

    /// `visitBreakOrContinue` without the change report: whether it changed the tree (Rust-only).
    fn break_or_continue_changed(&mut self, t: &mut Ast, node: NodeId) -> bool {
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
                    return true;
                }
            }
        }
        false
    }

    // port: RenameLabels.ProcessLabels#visitLabel
    fn visit_label(&mut self, t: &mut NodeTraversal<'_>, node: NodeId) {
        if self.label_changed(t, node) && self.mark_changes {
            t.report_code_change();
        }
    }

    /// `visitLabel` without the change report: whether it changed the tree (Rust-only).
    fn label_changed(&mut self, t: &mut Ast, node: NodeId) -> bool {
        let mut changed = false;
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
                changed = true;
            }
        } else {
            // ... and it is not referenced, just remove it.
            let new_child = node.get_last_child(t).unwrap();
            new_child.detach(t);
            node.replace_with(t, new_child);
            if new_child.is_block(t) {
                NodeUtil::try_merge_block(t, new_child, false);
            }
            changed = true;
        }

        // Remove the label from the current stack of labels.
        self.namespace_stack
            .last_mut()
            .unwrap()
            .rename_map
            .shift_remove(&name);
        changed
    }

    // port: RenameLabels.ProcessLabels#shouldTraverse (its LABEL branch)
    fn enter_label(&mut self, ast: &Ast, node: NodeId) {
        // Determine the new name for this label.
        let current = self.namespace_stack.last_mut().unwrap();
        let current_depth = current.rename_map.len() as i32 + 1;
        let name = node.get_first_child(ast).unwrap().get_string(ast);

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

    /// Rust-only (performance divergence): the traversal of the subtree of an outermost label
    /// (`outermost_labels`), which is all of Java's traversal that acts: outside labels
    /// `shouldTraverse` and `visit` do nothing (a BREAK or CONTINUE with a label name is inside
    /// its label) and a function's namespace stays empty. The callbacks run in the traversal's
    /// order (children in order, the next sibling read before descending; class members in the
    /// order of `handle_class_members`), a function gets its own namespace, and a change is reported to the
    /// innermost function or script, the traversal's change scope. Each outermost label starts
    /// in an empty namespace, as in the traversal, where the labels before it were removed from
    /// the namespace at their `visit`; the generated names are shared.
    fn walk(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        _parent: Option<NodeId>,
        change_scope: NodeId,
    ) {
        if n.is_label(compiler) {
            self.enter_label(compiler, n);
        }
        let is_function = n.is_function(compiler);
        let change_scope = if is_function {
            self.namespace_stack.push(LabelNamespace::default());
            n
        } else {
            change_scope
        };
        if n.is_class_members(compiler) {
            // The order of `NodeTraversal::handle_class_members`: the computed keys, then each
            // member (of a computed one, its value).
            let mut child = n.get_first_child(compiler);
            while let Some(c) = child {
                let next = c.get_next(compiler);
                if c.is_computed_prop(compiler) || c.is_computed_field_def(compiler) {
                    let key = c.get_first_child(compiler).unwrap();
                    self.walk(compiler, key, Some(c), change_scope);
                }
                child = next;
            }
            let mut child = n.get_first_child(compiler);
            while let Some(c) = child {
                let next = c.get_next(compiler);
                if c.is_computed_prop(compiler)
                    || (c.is_computed_field_def(compiler) && c.has_two_children(compiler))
                {
                    let value = c.get_last_child(compiler).unwrap();
                    self.walk(compiler, value, Some(c), change_scope);
                } else if !c.is_computed_field_def(compiler) {
                    self.walk(compiler, c, Some(n), change_scope);
                }
                child = next;
            }
        } else {
            let mut child = n.get_first_child(compiler);
            while let Some(c) = child {
                let next = c.get_next(compiler);
                self.walk(compiler, c, Some(n), change_scope);
                child = next;
            }
        }
        if is_function {
            self.namespace_stack.pop();
        }
        let changed = match n.get_token(compiler) {
            Token::LABEL => self.label_changed(compiler, n),
            Token::BREAK | Token::CONTINUE => self.break_or_continue_changed(compiler, n),
            _ => false,
        };
        if changed && self.mark_changes {
            compiler.report_change_to_change_scope(change_scope);
        }
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
            if let Some((labels, next)) = &mut self.expected_labels
                && self
                    .namespace_stack
                    .iter()
                    .all(|ns| ns.rename_map.is_empty())
            {
                assert_eq!(
                    labels.get(*next).copied(),
                    Some(node),
                    "CLOSURE_RS_RENAME_LABELS_DIRECT=verify: outermost label {next} of the \
                     traversal is {}, the candidate is {:?}",
                    node.to_string(node_traversal),
                    labels.get(*next).map(|l| l.to_string(node_traversal))
                );
                *next += 1;
            }
            self.enter_label(node_traversal, node);
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
