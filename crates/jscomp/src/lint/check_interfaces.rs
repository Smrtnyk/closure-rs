/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/lint/CheckInterfaces.java.

//! Checks for errors related to interfaces.
//!
//! 1. Non-declaration statements in `@interface` / `@record` constructors.
//! 2. Interface constructors with arguments, non-empty interface methods, static members and
//!    `extends` on interface classes.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_state,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    token::Token,
};

/// Placeholder class name for error reporting on anonymous classes.
// port: CheckInterfaces#ANONYMOUS_CLASSNAME
const ANONYMOUS_CLASSNAME: &str = "<anonymous>";

// port: CheckInterfaces#NON_DECLARATION_STATEMENT_IN_INTERFACE
pub static NON_DECLARATION_STATEMENT_IN_INTERFACE: DiagnosticType = DiagnosticType::disabled(
    "JSC_NON_DECLARATION_STATEMENT_IN_INTERFACE",
    "@interface or @record functions should not contain statements other than field declarations",
);

// port: CheckInterfaces#MISSING_JSDOC_IN_DECLARATION_STATEMENT
pub static MISSING_JSDOC_IN_DECLARATION_STATEMENT: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_JSDOC_IN_DECLARATION_STATEMENT",
    "@interface or @record functions must contain JSDoc for each field declaration.",
);

// port: CheckInterfaces#INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY
pub static INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY",
    "interface methods must have an empty body",
);

// port: CheckInterfaces#INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS
pub static INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS: DiagnosticType = DiagnosticType::disabled(
    "JSC_INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS",
    "Interface constructors should not take any arguments",
);

// port: CheckInterfaces#STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS
pub static STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS: DiagnosticType = DiagnosticType::disabled(
    "JSC_STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS",
    "Interface class should not have static member functions. Consider pulling out the static method into a flat name as {0}_{1}",
);

// port: CheckInterfaces#INTERFACE_DEFINED_WITH_EXTENDS
pub static INTERFACE_DEFINED_WITH_EXTENDS: DiagnosticType = DiagnosticType::disabled(
    "JSC_INTERFACE_DEFINED_WITH_EXTENDS",
    "Interface/Record class should use the `@extends` annotation instead of extends keyword.",
);

pub struct CheckInterfaces;

impl CheckInterfaces {
    // port: CheckInterfaces#CheckInterfaces
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    /// Whether jsDoc is present and has an `@interface` or `@record` annotation
    // port: CheckInterfaces#isInterface
    fn is_interface(js_doc: Option<&JSDocInfo>) -> bool {
        js_doc.is_some_and(|js_doc| js_doc.is_interface())
    }

    // port: CheckInterfaces#checkInterfaceConstructorArgs
    fn check_interface_constructor_args(t: &mut NodeTraversal<'_>, func_node: NodeId) {
        let args = func_node.get_second_child(t).unwrap();
        if args.has_children(t) {
            let first = args.get_first_child(t).unwrap();
            t.report(first, &INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS, &[]);
        }
    }

    /// Non-static class methods must be empty for `@record` and `@interface` as per the style
    /// guide.
    // port: CheckInterfaces#checkClassMethods
    fn check_class_methods(
        t: &mut NodeTraversal<'_>,
        class_node: NodeId,
        ctor_def: Option<NodeId>,
    ) {
        let class_members = class_node.get_last_child(t).unwrap();
        check_state!(
            class_members.is_class_members(t),
            "%s",
            class_members.to_string(t)
        );
        let mut member_func_def = class_members.get_first_child(t);
        while let Some(m) = member_func_def {
            member_func_def = m.get_next(t);
            if Some(m) == ctor_def {
                continue; // constructor was already checked; don't check here.
            }
            if m.is_static_member(t) {
                // `static foo() {...}`
                let class_name = NodeUtil::get_name(t, class_node)
                    .unwrap_or_else(|| JsString::from(ANONYMOUS_CLASSNAME));
                let func_name = m.get_string(t);
                t.report(
                    m,
                    &STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS,
                    &[&class_name.to_string_lossy(), &func_name.to_string_lossy()],
                );
            } else {
                let block = m.get_last_child(t).unwrap().get_last_child(t).unwrap();
                if block.has_children(t) {
                    let first = block.get_first_child(t).unwrap();
                    t.report(first, &INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY, &[]);
                }
            }
        }
    }

    /// `@record` constructors can be non-empty, check that only field declarations exist in them.
    // port: CheckInterfaces#checkConstructorBlock
    fn check_constructor_block(t: &mut NodeTraversal<'_>, func_node: NodeId) {
        let block = func_node.get_last_child(t).unwrap();
        if !block.has_children(t) {
            return;
        }
        let mut stmt = block.get_first_child(t);
        while let Some(s) = stmt {
            if !Self::is_this_prop_access(t, s) {
                // Only field declarations are expected inside @record and @interface.
                t.report(s, &NON_DECLARATION_STATEMENT_IN_INTERFACE, &[]);
                break;
            } else if s.get_first_child(t).unwrap().get_jsdoc_info(t).is_none() {
                // A field declaration that's missing a JSDoc.
                t.report(s, &MISSING_JSDOC_IN_DECLARATION_STATEMENT, &[]);
                break;
            }
            stmt = s.get_next(t);
        }
    }

    // port: CheckInterfaces#isThisPropAccess
    fn is_this_prop_access(ast: &Ast, stmt: NodeId) -> bool {
        stmt.is_expr_result(ast)
            && stmt.get_first_child(ast).unwrap().is_get_prop(ast)
            && stmt.get_first_first_child(ast).unwrap().is_this(ast)
    }
}

impl CompilerPass for CheckInterfaces {
    // port: CheckInterfaces#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckInterfaces {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckInterfaces#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::FUNCTION => {
                let jsdoc = NodeUtil::get_best_jsdoc_info(t, n);
                if Self::is_interface(jsdoc.as_deref()) {
                    Self::check_interface_constructor_args(t, n);
                    Self::check_constructor_block(t, n);
                }
            }
            Token::CLASS => {
                let jsdoc = NodeUtil::get_best_jsdoc_info(t, n);
                if Self::is_interface(jsdoc.as_deref()) {
                    if n.get_second_child(t).is_some_and(|c| c.is_name(t)) {
                        t.report(n, &INTERFACE_DEFINED_WITH_EXTENDS, &[]);
                    }
                    let ctor_def = NodeUtil::get_es6_class_constructor_member_function_def(t, n);
                    if let Some(ctor_def) = ctor_def {
                        let ctor = ctor_def.get_first_child(t).unwrap();
                        Self::check_interface_constructor_args(t, ctor);
                        Self::check_constructor_block(t, ctor);
                    }
                    Self::check_class_methods(t, n, ctor_def);
                }
            }
            _ => {}
        }
    }
}
