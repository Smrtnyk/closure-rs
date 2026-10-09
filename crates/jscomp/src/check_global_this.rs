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
//   src/com/google/javascript/jscomp/CheckGlobalThis.java.

use crate::{
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};

// port: CheckGlobalThis#GLOBAL_THIS
pub static GLOBAL_THIS: DiagnosticType = DiagnosticType::warning(
    "JSC_USED_GLOBAL_THIS",
    "dangerous use of the global 'this' object",
);

/// Checks for certain uses of the `this` keyword that are considered unsafe because they are
/// likely to reference the global `this` object unintentionally.
///
/// A use of `this` is considered unsafe if it's on the left side of an assignment or a property
/// access, and not inside one of the following:
///
/// 1. a class static initialization block
/// 2. a prototype method
/// 3. a function annotated with `@constructor`
/// 4. a function annotated with `@this`.
/// 5. a function where there's no logical place to put a `this` annotation.
///
/// Note that this check does not track assignments of `this` to variables or objects. The code
///
/// ```text
/// function evil() {
///   var a = this;
///   a.useful = undefined;
/// }
/// ```
///
/// will not get flagged, even though it is semantically equivalent to
///
/// ```text
/// function evil() {
///   this.useful = undefined;
/// }
/// ```
///
/// which would get flagged.
pub struct CheckGlobalThis {
    /// If `assign_lhs_child.is_some()`, then the node being traversed is a descendant of the first
    /// child of an ASSIGN node. assignLhsChild's parent is this ASSIGN node.
    assign_lhs_child: Option<NodeId>,
}

impl CheckGlobalThis {
    // port: CheckGlobalThis#CheckGlobalThis
    pub fn new() -> Self {
        Self {
            assign_lhs_child: None,
        }
    }

    // port: CheckGlobalThis#shouldReportThis
    fn should_report_this(&self, t: &NodeTraversal<'_>, n: NodeId) -> bool {
        let parent = n.get_parent(t);
        if self.assign_lhs_child.is_some() {
            // Always report a THIS on the left side of an assign.
            return true;
        }

        // Also report a THIS with a property access.
        parent.is_some_and(|parent| NodeUtil::is_normal_get(t, parent))
    }
}

impl Default for CheckGlobalThis {
    fn default() -> Self {
        Self::new()
    }
}

impl Callback for CheckGlobalThis {
    /// Since this pass reports errors only when a global `this` keyword is encountered, there is
    /// no reason to traverse non global contexts.
    // port: CheckGlobalThis#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if n.is_function(t) {
            // Arrow functions automatically get the "this" value from the
            // enclosing scope. e.g. the "this" in
            //   Foo.prototype.getBar = () => this.bar;
            // is the global "this", not an instance of Foo.
            if n.is_arrow_function(t) {
                return true;
            }

            // Don't traverse functions that are constructors or have the @this
            // or @override annotation.
            let js_doc = NodeUtil::get_best_jsdoc_info(t, n);
            if let Some(js_doc) = &js_doc
                && (js_doc.is_constructor()
                    || js_doc.is_interface()
                    || js_doc.has_this_type()
                    || js_doc.is_override())
            {
                return false;
            }

            if let Some(js_doc) = &js_doc {
                let function_type = js_doc.get_type();
                if let Some(function_type) = function_type {
                    let function_node = function_type.get_root();
                    if function_node.is_function(t) {
                        // function(this: ThisType, ...)
                        // `this:` is only allowed as the very first child of the
                        // FUNCTION node.
                        let this_node = function_node.get_first_child(t);
                        if this_node.is_some_and(|this_node| this_node.is_this(t)) {
                            // Type of `this` is specified, so no need to check further.
                            return false;
                        }
                    }
                }
            }

            // Don't traverse functions unless they would normally
            // be able to have a @this annotation associated with them. e.g.,
            // var a = function() { }; // or
            // function a() {} // or
            // a.x = function() {}; // or
            // var a = {x: function() {}};
            let parent = parent.unwrap();
            let p_type = parent.get_token(t);
            if !(p_type == Token::BLOCK
                || p_type == Token::SCRIPT
                || p_type == Token::NAME
                || p_type == Token::ASSIGN
                // object literal keys
                || p_type == Token::STRING_KEY)
            {
                return false;
            }

            // Don't traverse functions that are getting lent to a prototype.
            let grandparent = parent.get_parent(t);
            if NodeUtil::may_be_object_lit_key(t, parent) {
                let maybe_lends = grandparent.unwrap().get_jsdoc_info(t);
                if let Some(maybe_lends) = maybe_lends
                    && maybe_lends.has_lends_name()
                    && maybe_lends
                        .get_lends_name()
                        .unwrap()
                        .get_root()
                        .get_string(t)
                        .ends_with(&JsString::from(".prototype"))
                {
                    return false;
                }
            }
        }

        // Don't traverse class static blocks, 'this' is never the global 'this'
        if NodeUtil::is_class_static_block(t, n) {
            return false;
        }

        // `this` in class field definitions refers to the instance (for non-static fields) or the
        // class itself (for static fields), never the global 'this'.
        if (n.is_member_field_def(t) || n.is_computed_field_def(t))
            && n.get_parent(t).unwrap().is_class_members(t)
        {
            return false;
        }

        if let Some(parent) = parent
            && parent.is_assign(t)
        {
            let lhs = parent.get_first_child(t).unwrap();

            if n == lhs {
                // Always traverse the left side of the assignment. To handle
                // nested assignments properly (e.g., (a = this).property = c;),
                // assignLhsChild should not be overridden.
                if self.assign_lhs_child.is_none() {
                    self.assign_lhs_child = Some(lhs);
                }
            } else {
                // Only traverse the right side if it's not an assignment to a prototype
                // property or subproperty.
                if NodeUtil::is_normal_get(t, lhs) {
                    if lhs.is_get_prop(t) && lhs.get_string_ref(t) == "prototype" {
                        return false;
                    }
                    let llhs = lhs.get_first_child(t).unwrap();
                    if llhs.is_get_prop(t) && llhs.get_string_ref(t) == "prototype" {
                        return false;
                    }
                }
            }
        }

        true
    }

    // port: CheckGlobalThis#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_this(t) && self.should_report_this(t, n) {
            let error = JSError::make(t, n, &GLOBAL_THIS, &[]);
            t.get_compiler().report(error);
        }
        if Some(n) == self.assign_lhs_child {
            self.assign_lhs_child = None;
        }
    }
}
