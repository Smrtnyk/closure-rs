/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CollapseAnonymousFunctions.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of CollapseAnonymousFunctions.java.
//!
//! Collapses anonymous function expressions into named function declarations, i.e. the
//! following:
//!
//! ```text
//! var f = function() {}
//! ```
//!
//! becomes:
//!
//! ```text
//! function f() {}
//! ```
//!
//! This reduces the generated code size but changes the semantics because f will be defined
//! before its definition is reached. Also, in ES6+, "var f" is visible in the entire function
//! scope, whereas "function f" is block scoped, which may cause issues.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument,
    js_string::JsString,
    node::{Ast, NodeId},
};

pub struct CollapseAnonymousFunctions;

impl CollapseAnonymousFunctions {
    // port: CollapseAnonymousFunctions#CollapseAnonymousFunctions
    pub fn new(compiler: &AbstractCompiler) -> Self {
        check_argument!(compiler.get_life_cycle_stage().is_normalized());
        Self
    }

    // port: CollapseAnonymousFunctions#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }

    // port: CollapseAnonymousFunctions#isRecursiveFunction
    fn is_recursive_function(ast: &Ast, function: NodeId) -> bool {
        let name = function.get_first_child(ast).unwrap();
        if name.get_string_ref(ast).is_empty() {
            return false;
        }
        let args = name.get_next(ast).unwrap();
        let body = args.get_next(ast).unwrap();
        Self::contains_name(ast, body, &name.get_string(ast))
    }

    // port: CollapseAnonymousFunctions#containsName
    fn contains_name(ast: &Ast, n: NodeId, name: &JsString) -> bool {
        if n.is_name(ast) && n.get_string(ast) == *name {
            return true;
        }

        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            if Self::contains_name(ast, c, name) {
                return true;
            }
            child = c.get_next(ast);
        }
        false
    }
}

impl CompilerPass for CollapseAnonymousFunctions {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        CollapseAnonymousFunctions::process(self, compiler, externs, root);
    }
}

impl Callback for CollapseAnonymousFunctions {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CollapseAnonymousFunctions#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if !(n.is_var(t) || n.is_let(t) || n.is_const(t)) {
            return;
        }

        // It is only safe to collapse anonymous functions that appear
        // at top-level blocks.  In other cases the difference between
        // variable and function declarations can lead to problems or
        // expose subtle bugs in browser implementation as function
        // definitions are added to scopes before the start of execution.

        let parent = parent.unwrap();
        let grandparent = parent.get_parent(t);
        if !(parent.is_script(t)
            || (grandparent.is_some_and(|grandparent| grandparent.is_function(t))
                && parent.is_block(t)))
        {
            return;
        }

        // Need to store the next name in case the current name is removed from
        // the linked list.
        let name = n.get_only_child(t);

        // Don't collapse if the lhs is a destructuring pattern.
        if !name.is_name(t) {
            return;
        }

        let value = name.get_first_child(t);
        if let Some(value) = value
            && value.is_function(t)
            && !value.is_arrow_function(t)
            && !Self::is_recursive_function(t, value)
        {
            let fn_name = value.get_first_child(t).unwrap();
            if fn_name.get_boolean_prop(t, NodeId::IS_CONSTANT_NAME)
                && !name.get_boolean_prop(t, NodeId::IS_CONSTANT_NAME)
            {
                // When the LHS name `f` is already a const `const f = function() {};`, then we must
                // preserve its constness during this rewriting to `function f() {}`.
                // When the LHS name `f` is non-const `var f = function() {}` or
                // `let f = function() {}`, we must reset the constness as the function will no
                // longer be an RHS function expression during this rewriting to `function f() {}`
                fn_name.put_boolean_prop(t, NodeId::IS_CONSTANT_NAME, false);
            }
            let name_string = name.get_string(t);
            fn_name.set_string(t, name_string);
            NodeUtil::copy_name_annotations(t, name, fn_name);
            value.detach(t);
            n.replace_with(t, value);

            // Renormalize the code.
            if !t.in_global_scope() && NodeUtil::is_hoisted_function_declaration(t, value) {
                let detached = value.detach(t);
                parent.add_child_to_front(t, detached);
            }

            // report changes to both the change scopes
            t.get_compiler().report_change_to_change_scope(value);
            t.report_code_change();
        }
    }
}
