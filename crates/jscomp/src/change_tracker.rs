/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ChangeTracker.java,
//   test/com/google/javascript/jscomp/ChangeTrackerTest.java.

use crate::{
    code_change_handler::CodeChangeHandler, recent_change::RecentChange, timeline::Timeline,
};
use closure_rhino::{
    check_state,
    node::{Ast, NodeId},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};

pub struct ChangeTracker {
    change_stamp: Arc<AtomicI32>,
    change_timeline: Timeline<NodeId>,
    recent_change: Arc<Mutex<RecentChange>>,
    code_change_handlers: Vec<Arc<Mutex<dyn CodeChangeHandler>>>,
}

impl ChangeTracker {
    pub fn new() -> Self {
        Self {
            change_stamp: Arc::new(AtomicI32::new(1)),
            change_timeline: Timeline::new(),
            recent_change: Arc::new(Mutex::new(RecentChange::default())),
            code_change_handlers: Vec::new(),
        }
    }

    pub(crate) fn get_change_stamp_source(&self) -> Arc<AtomicI32> {
        Arc::clone(&self.change_stamp)
    }

    // port: ChangeTracker#addChangeHandler
    pub fn add_change_handler(&mut self, handler: Arc<Mutex<dyn CodeChangeHandler>>) {
        self.code_change_handlers.push(handler);
    }

    // port: ChangeTracker#removeChangeHandler
    pub fn remove_change_handler(&mut self, handler: &Arc<Mutex<dyn CodeChangeHandler>>) {
        if let Some(index) = self
            .code_change_handlers
            .iter()
            .position(|h| Arc::ptr_eq(h, handler))
        {
            self.code_change_handlers.remove(index);
        }
    }

    // port: ChangeTracker#getRecentChange
    pub fn get_recent_change(&self) -> Arc<Mutex<RecentChange>> {
        Arc::clone(&self.recent_change)
    }

    /// Rust-only: the RecentChange this tracker holds for the test harness is the test case's own
    /// object in Java, which CompilerTestCaseUtils#multistageSerializeAndDeserialize hands from the
    /// saved compiler to the restored one.
    pub fn set_recent_change(&mut self, recent_change: Arc<Mutex<RecentChange>>) {
        self.recent_change = recent_change;
    }

    // port: ChangeTracker#reportChangeToEnclosingScope
    pub fn report_change_to_enclosing_scope(&mut self, ast: &mut Ast, n: NodeId) {
        let change_scope = self.get_change_scope_for_node(ast, n);
        self.record_change(ast, change_scope);
        self.notify_change_handlers();
    }

    // port: ChangeTracker#reportChangeToChangeScope
    pub fn report_change_to_change_scope(&mut self, ast: &mut Ast, change_scope_root: NodeId) {
        check_state!(change_scope_root.is_script(ast) || change_scope_root.is_function(ast));
        self.record_change(ast, change_scope_root);
        self.notify_change_handlers();
    }

    // port: ChangeTracker#markNewScopesChanged
    pub fn mark_new_scopes_changed(&mut self, ast: &mut Ast, node: NodeId) {
        if node.is_function(ast) {
            self.report_change_to_change_scope(ast, node);
        }
        let mut child = node.get_first_child(ast);
        while let Some(current) = child {
            self.mark_new_scopes_changed(ast, current);
            child = current.get_next(ast);
        }
    }

    // port: ChangeTracker#reportFunctionDeleted
    pub fn report_function_deleted(&mut self, ast: &mut Ast, n: NodeId) {
        check_state!(n.is_function(ast));
        n.set_deleted(ast, true);
        self.change_timeline.remove(&n);
    }

    // port: ChangeTracker#getChangedScopeNodesForPass
    pub fn get_changed_scope_nodes_for_pass(&mut self, pass_name: &str) -> Option<Vec<NodeId>> {
        let changed_scope_nodes = self.change_timeline.get_since(pass_name);
        self.change_timeline.mark(pass_name);
        changed_scope_nodes
    }

    // port: ChangeTracker#getChangeStamp
    pub fn get_change_stamp(&self) -> i32 {
        self.change_stamp.load(Ordering::Relaxed)
    }

    // port: ChangeTracker#incrementChangeStamp
    pub fn increment_change_stamp(&mut self) {
        self.change_stamp.fetch_add(1, Ordering::Relaxed);
    }

    // port: ChangeTracker#resetChangeStamp
    pub fn reset_change_stamp(&mut self) {
        self.change_stamp.store(1, Ordering::Relaxed);
    }

    // port: ChangeTracker#isChangeScopeRoot
    pub fn is_change_scope_root(ast: &Ast, n: NodeId) -> bool {
        n.is_script(ast) || n.is_function(ast)
    }

    // port: ChangeTracker#getEnclosingChangeScopeRoot
    pub fn get_enclosing_change_scope_root(ast: &Ast, mut n: Option<NodeId>) -> Option<NodeId> {
        while let Some(current) = n {
            if Self::is_change_scope_root(ast, current) {
                break;
            }
            n = current.get_parent(ast);
        }
        n
    }

    // port: ChangeTracker#notifyChangeHandlers
    fn notify_change_handlers(&mut self) {
        for handler in &self.code_change_handlers {
            handler.lock().unwrap().report_change();
        }
    }

    // port: ChangeTracker#getChangeScopeForNode
    fn get_change_scope_for_node(&self, ast: &Ast, n: NodeId) -> NodeId {
        if n.is_script(ast) {
            return n;
        }
        Self::get_enclosing_change_scope_root(ast, n.get_parent(ast)).unwrap_or_else(|| {
            panic!(
                "An enclosing scope is required for change reports but node {} doesn't have one.",
                n.to_string(ast)
            )
        })
    }

    // port: ChangeTracker#recordChange
    fn record_change(&mut self, ast: &mut Ast, n: NodeId) {
        if n.is_deleted(ast) {
            return;
        }
        n.set_change_time(ast, self.get_change_stamp());
        self.increment_change_stamp();
        self.change_timeline.add(n);
    }
}

impl Default for ChangeTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ChangeTracker;
    use closure_rhino::{
        ir::IR,
        node::{Ast, NodeId},
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn foo(ast: &mut Ast) -> NodeId {
        let name = IR::name(ast, "foo");
        let params = IR::param_list(ast, &[]);
        let body = IR::block(ast);
        IR::function(ast, name, params, body)
    }
    // port: ChangeTrackerTest#testReportChangeNoScopeFails
    #[test]
    fn test_report_change_no_scope_fails() {
        let mut change_tracker = ChangeTracker::new();
        let mut ast = Ast::new();
        let name = IR::name(&mut ast, "foo");
        let detached_node = IR::var(&mut ast, name);
        assert!(
            catch_unwind(AssertUnwindSafe(
                || change_tracker.report_change_to_enclosing_scope(&mut ast, detached_node)
            ))
            .is_err()
        );
    }
    // port: ChangeTrackerTest#testReportChangeWithScopeSucceeds
    #[test]
    fn test_report_change_with_scope_succeeds() {
        let mut change_tracker = ChangeTracker::new();
        let mut ast = Ast::new();
        let name = IR::name(&mut ast, "foo");
        let attached_node = IR::var(&mut ast, name);
        let name = IR::name(&mut ast, "bar");
        let params = IR::param_list(&mut ast, &[]);
        let block = IR::block_with_child(&mut ast, attached_node);
        IR::function(&mut ast, name, params, block);
        change_tracker.report_change_to_enclosing_scope(&mut ast, attached_node);
    }
    // port: ChangeTrackerTest#testGetChangesAndDeletions_baseline
    #[test]
    fn test_get_changes_and_deletions_baseline() {
        let mut change_tracker = ChangeTracker::new();
        assert_eq!(
            change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner"),
            None
        );
    }
    // port: ChangeTrackerTest#testGetChangesAndDeletions_changeReportsVisible
    #[test]
    fn test_get_changes_and_deletions_change_reports_visible() {
        let mut change_tracker = ChangeTracker::new();
        let mut ast = Ast::new();
        let function1 = foo(&mut ast);
        let function2 = foo(&mut ast);
        let script = IR::script_with_children(&mut ast, &[function1, function2]);
        IR::root(&mut ast, &[script]);
        change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner");
        change_tracker.report_change_to_change_scope(&mut ast, function1);
        change_tracker.report_change_to_change_scope(&mut ast, function2);
        assert_eq!(
            change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner"),
            Some(vec![function1, function2])
        );
    }
    // port: ChangeTrackerTest#testGetChangesAndDeletions_deleteOverridesChange
    #[test]
    fn test_get_changes_and_deletions_delete_overrides_change() {
        let mut change_tracker = ChangeTracker::new();
        let mut ast = Ast::new();
        let function1 = foo(&mut ast);
        let function2 = foo(&mut ast);
        let script = IR::script_with_children(&mut ast, &[function1, function2]);
        IR::root(&mut ast, &[script]);
        change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner");
        change_tracker.report_change_to_change_scope(&mut ast, function1);
        change_tracker.report_change_to_change_scope(&mut ast, function2);
        function2.detach(&mut ast);
        change_tracker.report_function_deleted(&mut ast, function2);
        assert_eq!(
            change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner"),
            Some(vec![function1])
        );
    }
    // port: ChangeTrackerTest#testGetChangesAndDeletions_changeDoesntOverrideDelete
    #[test]
    fn test_get_changes_and_deletions_change_doesnt_override_delete() {
        let mut change_tracker = ChangeTracker::new();
        let mut ast = Ast::new();
        let function1 = foo(&mut ast);
        let function2 = foo(&mut ast);
        let script = IR::script_with_children(&mut ast, &[function1, function2]);
        IR::root(&mut ast, &[script]);
        change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner");
        change_tracker.report_change_to_change_scope(&mut ast, function1);
        function2.detach(&mut ast);
        change_tracker.report_function_deleted(&mut ast, function2);
        change_tracker.report_change_to_change_scope(&mut ast, function2);
        assert_eq!(
            change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner"),
            Some(vec![function1])
        );
    }
    // port: ChangeTrackerTest#testGetChangesAndDeletions_onlySeesChangesSinceLastRequest
    #[test]
    fn test_get_changes_and_deletions_only_sees_changes_since_last_request() {
        let mut change_tracker = ChangeTracker::new();
        let mut ast = Ast::new();
        let function1 = foo(&mut ast);
        let function2 = foo(&mut ast);
        let script = IR::script_with_children(&mut ast, &[function1, function2]);
        IR::root(&mut ast, &[script]);
        change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner");
        change_tracker.report_change_to_change_scope(&mut ast, function1);
        function2.detach(&mut ast);
        change_tracker.report_function_deleted(&mut ast, function2);
        assert_eq!(
            change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner"),
            Some(vec![function1])
        );
        assert_eq!(
            change_tracker.get_changed_scope_nodes_for_pass("FunctionInliner"),
            Some(Vec::new())
        );
    }
}
