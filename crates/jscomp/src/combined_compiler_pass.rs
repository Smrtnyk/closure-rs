/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CombinedCompilerPass.java.

//! Port of CombinedCompilerPass.java: combines several NodeTraversal callbacks into a single
//! traversal.
//!
//! The combined pass runs the callbacks in a single traversal; a callback that returns false from
//! shouldTraverse is inactive (neither shouldTraverse nor visit is called for it) until the
//! traversal leaves the node it declined.
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use closure_rhino::node::NodeId;

// port: CombinedCompilerPass
pub struct CombinedCompilerPass<'a> {
    /// The callbacks that this pass combines.
    callbacks: Vec<CallbackWrapper<'a>>,
}

impl<'a> CombinedCompilerPass<'a> {
    /// Java keeps the compiler; the Rust pass receives it in `process` and from the traversal.
    // port: CombinedCompilerPass#CombinedCompilerPass(AbstractCompiler,List)
    pub fn new(_compiler: &AbstractCompiler, callbacks: Vec<&'a mut dyn Callback>) -> Self {
        let mut wrappers = Vec::with_capacity(callbacks.len());
        for callback in callbacks {
            wrappers.push(CallbackWrapper::new(callback));
        }
        Self {
            callbacks: wrappers,
        }
    }

    // port: CombinedCompilerPass#traverse
    pub fn traverse(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        mut callbacks: Vec<&'a mut dyn Callback>,
    ) {
        if callbacks.len() == 1 {
            NodeTraversal::traverse(compiler, root, callbacks.remove(0));
        } else {
            CombinedCompilerPass::new(compiler, callbacks).process(compiler, None, root);
        }
    }

    // port: CombinedCompilerPass#process
    pub fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        _externs: Option<NodeId>,
        root: NodeId,
    ) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

/// Maintains information about a callback in order to simulate it being the exclusive client of
/// traversal. The scoped callback is Java's `callback instanceof ScopedCallback`, resolved through
/// `Callback::as_scoped_callback` when needed.
// port: CombinedCompilerPass.CallbackWrapper
struct CallbackWrapper<'a> {
    /// The callback being wrapped.
    callback: &'a mut dyn Callback,
    /// The node that this callback is waiting for (it declined to traverse it), or None if the
    /// callback is active.
    waiting: Option<NodeId>,
}

impl<'a> CallbackWrapper<'a> {
    // port: CombinedCompilerPass.CallbackWrapper#CallbackWrapper
    fn new(callback: &'a mut dyn Callback) -> Self {
        Self {
            callback,
            waiting: None,
        }
    }

    /// Visits the node unless the wrapped callback is inactive. Activates the callback if
    /// appropriate.
    // port: CombinedCompilerPass.CallbackWrapper#visitOrMaybeActivate
    fn visit_or_maybe_activate(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        if self.is_active() {
            self.callback.visit(t, n, parent);
        } else if self.waiting == Some(n) {
            self.waiting = None;
        }
    }

    // port: CombinedCompilerPass.CallbackWrapper#shouldTraverseIfActive
    fn should_traverse_if_active(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        if self.is_active() && !self.callback.should_traverse(t, n, parent) {
            self.waiting = Some(n);
        }
    }

    // port: CombinedCompilerPass.CallbackWrapper#enterScopeIfActive
    fn enter_scope_if_active(&mut self, t: &mut NodeTraversal<'_>) {
        if self.is_active()
            && let Some(scoped_callback) = self.callback.as_scoped_callback()
        {
            scoped_callback.enter_scope(t);
        }
    }

    // port: CombinedCompilerPass.CallbackWrapper#exitScopeIfActive
    fn exit_scope_if_active(&mut self, t: &mut NodeTraversal<'_>) {
        if self.is_active()
            && let Some(scoped_callback) = self.callback.as_scoped_callback()
        {
            scoped_callback.exit_scope(t);
        }
    }

    // port: CombinedCompilerPass.CallbackWrapper#isActive
    fn is_active(&self) -> bool {
        self.waiting.is_none()
    }
}

impl CompilerPass for CombinedCompilerPass<'_> {
    // port: CombinedCompilerPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        CombinedCompilerPass::process(self, compiler, Some(externs), root);
    }
}

impl Callback for CombinedCompilerPass<'_> {
    // port: CombinedCompilerPass#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if t.get_compiler().has_halting_errors() {
            return false;
        }
        for callback in &mut self.callbacks {
            callback.should_traverse_if_active(t, n, parent);
        }
        // Note that this method could return false if all callbacks are inactive.
        // This apparent optimization would make this method more expensive
        // in the typical case where not all nodes are inactive. It is
        // very unlikely that many all callbacks would be inactive at the same
        // time (indeed, there are several checking passes that never return false).
        true
    }

    // port: CombinedCompilerPass#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if t.get_compiler().has_halting_errors() {
            return;
        }
        for callback in &mut self.callbacks {
            callback.visit_or_maybe_activate(t, n, parent);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for CombinedCompilerPass<'_> {
    // port: CombinedCompilerPass#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        for callback in &mut self.callbacks {
            callback.enter_scope_if_active(t);
        }
    }

    // port: CombinedCompilerPass#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        for callback in &mut self.callbacks {
            callback.exit_scope_if_active(t);
        }
    }
}
