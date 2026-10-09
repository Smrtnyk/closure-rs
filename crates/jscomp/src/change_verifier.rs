/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ChangeVerifier.java.

use crate::{
    abstract_compiler::AbstractCompiler, change_tracker::ChangeTracker, node_printing,
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::node::NodeId;

pub struct ChangeVerifier {
    clones_by_current: IndexMap<NodeId, NodeId>,
    snapshot_change: i32,
}
impl ChangeVerifier {
    // port: ChangeVerifier#ChangeVerifier
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            clones_by_current: IndexMap::<_, _>::default(),
            snapshot_change: 0,
        }
    }
    // port: ChangeVerifier#snapshot
    pub fn snapshot(mut self, compiler: &mut AbstractCompiler, root: NodeId) -> Self {
        self.clones_by_current.clear();
        self.snapshot_change = compiler.get_change_tracker_ref().get_change_stamp();
        let snapshot = root.clone_tree(compiler);
        self.associate_clones(compiler, root, snapshot);
        self
    }
    // port: ChangeVerifier#checkRecordedChanges(Node)
    pub fn check_recorded_changes(&self, compiler: &mut AbstractCompiler, current: NodeId) {
        self.check_recorded_changes_named(compiler, "", current);
    }
    // port: ChangeVerifier#checkRecordedChanges(String, Node)
    pub fn check_recorded_changes_named(
        &self,
        compiler: &mut AbstractCompiler,
        pass_name: &str,
        root: NodeId,
    ) {
        self.verify_scope_changes_have_been_recorded(compiler, pass_name, root);
    }
    // port: ChangeVerifier#associateClones
    fn associate_clones(&mut self, compiler: &AbstractCompiler, n: NodeId, snapshot: NodeId) {
        if n.is_root(compiler) || ChangeTracker::is_change_scope_root(compiler, n) {
            self.clones_by_current.insert(n, snapshot);
        }
        let mut child = n.get_first_child(compiler);
        let mut snapshot_child = snapshot.get_first_child(compiler);
        while let Some(c) = child {
            let clone = snapshot_child.unwrap();
            self.associate_clones(compiler, c, clone);
            child = c.get_next(compiler);
            snapshot_child = clone.get_next(compiler);
        }
    }
    // port: ChangeVerifier#verifyScopeChangesHaveBeenRecorded
    fn verify_scope_changes_have_been_recorded(
        &self,
        compiler: &mut AbstractCompiler,
        pass_name: &str,
        root: NodeId,
    ) {
        let pass_name_msg = if pass_name.is_empty() {
            String::new()
        } else {
            format!("{pass_name}: ")
        };
        let mut snapshot_scope_nodes = IndexSet::<_>::default();
        // port: ChangeVerifier.<anonymous>#visit
        visit_pre_order(compiler, self.clones_by_current[&root], &mut |n| {
            if ChangeTracker::is_change_scope_root(compiler, n) {
                snapshot_scope_nodes.insert(n);
            }
            None::<Failure>
        });
        // The checks read the compiler immutably while they traverse; the first failure stops the
        // traversal (Java throws there) and its message, which prints nodes with their JSTypes
        // (the compiler's registry, borrowed mutably), is built after it.
        // port: ChangeVerifier.<anonymous>#visit
        let failure = visit_pre_order(compiler, root, &mut |n| {
            if n.is_root(compiler) {
                self.verify_root(compiler, n);
            } else if ChangeTracker::is_change_scope_root(compiler, n) {
                let clone = self.clones_by_current.get(&n).copied();
                if let Some(clone) = clone {
                    snapshot_scope_nodes.shift_remove(&clone);
                }
                if let Some(failure) = self.verify_node(compiler, n) {
                    return Some(failure);
                }
                if let Some(clone) = clone {
                    return self.verify_node_change(compiler, &pass_name_msg, n, clone);
                } else {
                    return self.verify_new_node(compiler, n);
                }
            }
            None
        });
        if let Some(failure) = failure {
            failure.throw(self, compiler, &pass_name_msg);
        }
        self.verify_deleted_scope_nodes(compiler, &pass_name_msg, &snapshot_scope_nodes);
    }
    // port: ChangeVerifier#verifyDeletedScopeNodes
    fn verify_deleted_scope_nodes(
        &self,
        compiler: &mut AbstractCompiler,
        pass_name_msg: &str,
        deleted_scope_nodes: &IndexSet<NodeId>,
    ) {
        for snapshot in deleted_scope_nodes {
            let current = *self
                .clones_by_current
                .iter()
                .find(|(_, s)| *s == snapshot)
                .unwrap()
                .0;
            if current.is_deleted(compiler) {
                continue;
            }
            panic!(
                "{pass_name_msg}deleted scope was not reported:\n{}",
                node_printing::to_string_tree(compiler, current)
            );
        }
    }
    // port: ChangeVerifier#verifyNode
    fn verify_node(&self, compiler: &AbstractCompiler, n: NodeId) -> Option<Failure> {
        if n.is_deleted(compiler) {
            return Some(Failure::ImproperlyMarkedAsDeleted(n));
        }
        None
    }
    // port: ChangeVerifier#verifyNewNode
    fn verify_new_node(&self, compiler: &AbstractCompiler, n: NodeId) -> Option<Failure> {
        let change_time = n.get_change_time(compiler);
        if change_time == 0 || change_time < self.snapshot_change {
            return Some(Failure::NewScopeNotMarkedAsChanged(n));
        }
        None
    }
    // port: ChangeVerifier#verifyRoot
    fn verify_root(&self, compiler: &AbstractCompiler, root: NodeId) {
        assert!(root.is_root(compiler));
        assert!(
            root.get_change_time(compiler) == 0,
            "Root nodes should never be marked as changed."
        );
    }
    // port: ChangeVerifier#verifyNodeChange
    fn verify_node_change(
        &self,
        compiler: &AbstractCompiler,
        pass_name_msg: &str,
        n: NodeId,
        snapshot: NodeId,
    ) -> Option<Failure> {
        if n.is_root(compiler) {
            return None;
        }
        let result = Self::get_inequivalence_reason_excluding_functions(compiler, n, snapshot);
        if n.get_change_time(compiler) > snapshot.get_change_time(compiler) {
            if result.equals {
                panic!(
                    "{pass_name_msg}unchanged scope marked as changed: {}",
                    self.get_name_for_node(compiler, n)
                );
            }
        } else if !result.equals {
            return Some(Failure::ChangedScopeNotMarkedAsChanged(n, result));
        }
        None
    }
    // port: ChangeVerifier#getNameForNode
    pub fn get_name_for_node(&self, compiler: &AbstractCompiler, n: NodeId) -> String {
        let source_name = NodeUtil::get_source_name(compiler, n).unwrap_or_else(|| "null".into());
        if n.is_script(compiler) {
            format!("SCRIPT: {source_name}")
        } else if n.is_function(compiler) {
            let fn_name = NodeUtil::get_nearest_function_name(compiler, n)
                .map(|s| s.to_string_lossy())
                .unwrap_or_else(|| {
                    format!(
                        "anonymous@{}:{}",
                        n.get_lineno(compiler),
                        n.get_charno(compiler)
                    )
                });
            format!("FUNCTION: {fn_name} in {source_name}")
        } else {
            panic!("unexpected Node type")
        }
    }
    // port: ChangeVerifier#path
    fn path(compiler: &mut AbstractCompiler, ancestor: NodeId, child: NodeId) -> String {
        let mut child_to_ancestor = vec![];
        let mut current = child;
        while current != ancestor {
            child_to_ancestor.push(node_printing::to_string(compiler, current));
            current = current.get_parent(compiler).unwrap();
        }
        child_to_ancestor.push(node_printing::to_string(compiler, ancestor));
        child_to_ancestor.reverse();
        let mut result = String::new();
        for (i, name) in child_to_ancestor.iter().enumerate() {
            result.push_str(&" ".repeat(i * 2));
            result.push_str(name);
            result.push('\n');
        }
        result
    }
    // port: ChangeVerifier#getInequivalenceReasonExcludingFunctions
    fn get_inequivalence_reason_excluding_functions(
        compiler: &AbstractCompiler,
        this_node: NodeId,
        that_node: NodeId,
    ) -> EqualsResult {
        if this_node.get_child_count(compiler) != that_node.get_child_count(compiler) {
            // Java concatenates `thisNode + ": " + getChildCount()` here; the strings are
            // printed when the message is reported (the same text: printing has no side effects),
            // with the compiler's registry for typed nodes.
            return EqualsResult::not_equal(
                this_node,
                "differing child count",
                MessageArg::NodeWithChildCount(this_node, this_node.get_child_count(compiler)),
                MessageArg::NodeWithChildCount(that_node, that_node.get_child_count(compiler)),
            );
        }
        if !this_node.is_equivalent_with_side_effects_to_shallow(compiler, that_node) {
            return EqualsResult::not_equal(
                this_node,
                "shallow inequivalence",
                MessageArg::Node(this_node),
                MessageArg::Node(that_node),
            );
        }
        if this_node.is_function(compiler)
            && that_node.is_function(compiler)
            && NodeUtil::is_function_declaration(compiler, this_node)
                != NodeUtil::is_function_declaration(compiler, that_node)
        {
            return EqualsResult::not_equal(
                this_node,
                "mismatched isFunctionDeclaration",
                MessageArg::Node(this_node),
                MessageArg::Node(that_node),
            );
        }
        let mut this_child = this_node.get_first_child(compiler);
        let mut that_child = that_node.get_first_child(compiler);
        while let (Some(a), Some(b)) = (this_child, that_child) {
            if a.is_function(compiler) || a.is_script(compiler) {
                if b.get_token(compiler) != a.get_token(compiler) {
                    return EqualsResult::not_equal(
                        this_node,
                        "different tokens",
                        MessageArg::Node(a),
                        MessageArg::Node(b),
                    );
                }
                if a.is_function(compiler) && NodeUtil::is_function_declaration(compiler, a) {
                    let this_name = a.get_first_child(compiler).unwrap().get_string(compiler);
                    let that_name = b.get_first_child(compiler).unwrap().get_string(compiler);
                    if this_name != that_name {
                        return EqualsResult::not_equal(
                            this_node,
                            "function name changed",
                            MessageArg::String(this_name.to_string_lossy()),
                            MessageArg::String(that_name.to_string_lossy()),
                        );
                    }
                }
            } else {
                let result = Self::get_inequivalence_reason_excluding_functions(compiler, a, b);
                if !result.equals {
                    return result;
                }
            }
            this_child = a.get_next(compiler);
            that_child = b.get_next(compiler);
        }
        EqualsResult::equal()
    }
}
/// Java's `Supplier<String> errorMessage`: the message is formatted only when
/// verifyNodeChange reports it (`result.errorMessage.get()`), so nodes are printed lazily.
struct EqualsResult {
    equals: bool,
    error_node: Option<NodeId>,
    error_message: Option<(&'static str, MessageArg, MessageArg)>,
}
/// The `Object after` / `Object before` arguments of EqualsResult#notEqual, stringified by
/// String.format only when the supplier runs.
enum MessageArg {
    String(String),
    Node(NodeId),
    /// `node + ": " + node.getChildCount()`.
    NodeWithChildCount(NodeId, i32),
}
impl MessageArg {
    fn format(&self, compiler: &mut AbstractCompiler) -> String {
        match self {
            MessageArg::String(s) => s.clone(),
            MessageArg::Node(n) => node_printing::to_string(compiler, *n),
            MessageArg::NodeWithChildCount(n, count) => {
                format!("{}: {}", node_printing::to_string(compiler, *n), count)
            }
        }
    }
}
/// A check that failed while ChangeVerifier traversed the AST; `throw` builds Java's
/// IllegalStateException message.
enum Failure {
    ImproperlyMarkedAsDeleted(NodeId),
    NewScopeNotMarkedAsChanged(NodeId),
    ChangedScopeNotMarkedAsChanged(NodeId, EqualsResult),
}
impl Failure {
    // port: ChangeVerifier#verifyNode
    // port: ChangeVerifier#verifyNewNode
    // port: ChangeVerifier#verifyNodeChange
    fn throw(
        self,
        verifier: &ChangeVerifier,
        compiler: &mut AbstractCompiler,
        pass_name_msg: &str,
    ) -> ! {
        match self {
            Failure::ImproperlyMarkedAsDeleted(n) => panic!(
                "{pass_name_msg}existing scope is improperly marked as deleted:\n{}",
                node_printing::to_string_tree(compiler, n)
            ),
            Failure::NewScopeNotMarkedAsChanged(n) => panic!(
                "{pass_name_msg}new scope not explicitly marked as changed:\n{}",
                node_printing::to_string_tree(compiler, n)
            ),
            Failure::ChangedScopeNotMarkedAsChanged(n, result) => {
                let name = verifier.get_name_for_node(compiler, n);
                let error_message = result.error_message(compiler);
                let path = ChangeVerifier::path(compiler, n, result.error_node.unwrap());
                panic!(
                    "\"{pass_name_msg}changed scope not marked as changed: {name}.\n{error_message}\nAncestor nodes:\n{path}\n"
                )
            }
        }
    }
}
impl EqualsResult {
    // port: ChangeVerifier.EqualsResult#equal
    fn equal() -> Self {
        Self {
            equals: true,
            error_node: None,
            error_message: None,
        }
    }
    // port: ChangeVerifier.EqualsResult#notEqual
    fn not_equal(
        error_node: NodeId,
        error: &'static str,
        after: MessageArg,
        before: MessageArg,
    ) -> Self {
        Self {
            equals: false,
            error_node: Some(error_node),
            error_message: Some((error, after, before)),
        }
    }
    // port: ChangeVerifier.EqualsResult#errorMessage (Supplier#get)
    fn error_message(&self, compiler: &mut AbstractCompiler) -> String {
        match &self.error_message {
            Some((error, after, before)) => format!(
                "{error}\nBefore: {}\nAfter:  {}\n",
                before.format(compiler),
                after.format(compiler)
            ),
            None => "null".into(),
        }
    }
}
// Read-only borrowing adaptation: NodeUtil::visit_pre_order takes &mut Ast,
// while ChangeVerifier checks an immutable compiler and cannot use that API. A visitor that
// returns a failure stops the traversal (Java's visitor throws).
fn visit_pre_order<F>(
    compiler: &AbstractCompiler,
    n: NodeId,
    visitor: &mut impl FnMut(NodeId) -> Option<F>,
) -> Option<F> {
    if let Some(failure) = visitor(n) {
        return Some(failure);
    }
    for child in n.children(compiler) {
        if let Some(failure) = visit_pre_order(compiler, child, visitor) {
            return Some(failure);
        }
    }
    None
}
