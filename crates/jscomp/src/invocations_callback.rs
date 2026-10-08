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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/InvocationsCallback.java.

use crate::node_traversal::NodeTraversal;
use closure_rhino::{js_string::JsString, node::NodeId};

/// Traversal callback that finds method invocations of the form
///
/// ```text
/// call
///   getprop
///     ...
///     string
///   ...
/// ```
///
/// and invokes a method defined by subclasses for processing these invocations.
///
/// Java's abstract class extends `AbstractPostOrderCallback`; an implementor also implements
/// `Callback` with `should_traverse` returning true and `visit` calling
/// `InvocationsCallback::visit`.
pub trait InvocationsCallback {
    // port: InvocationsCallback#visit(NodeTraversal, Node, Node)
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if !n.is_call(t) {
            return;
        }

        let callee = n.get_first_child(t).unwrap();
        if !callee.is_get_prop(t) {
            return;
        }

        let call_name = callee.get_string(t);
        self.visit_with_call_name(t, n, parent, call_name);
    }

    /// Called for each callnode that is a method invocation.
    ///
    /// `call_node` is a node of type call, `parent` the parent of callNode and `call_name` the
    /// name of method invoked by first child of call.
    // port: InvocationsCallback#visit(NodeTraversal, Node, Node, String)
    fn visit_with_call_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call_node: NodeId,
        parent: Option<NodeId>,
        call_name: JsString,
    );
}
