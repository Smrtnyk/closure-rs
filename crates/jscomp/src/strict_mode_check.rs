/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/StrictModeCheck.java.

//! Port of `StrictModeCheck.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::check_level::CheckLevel;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_jstype::prelude::*;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use indexmap::IndexSet;

// port: StrictModeCheck#USE_OF_WITH
pub static USE_OF_WITH: DiagnosticType = DiagnosticType::error(
    "JSC_USE_OF_WITH",
    "The 'with' statement cannot be used in strict mode.",
);

// port: StrictModeCheck#EVAL_DECLARATION
pub static EVAL_DECLARATION: DiagnosticType = DiagnosticType::error(
    "JSC_EVAL_DECLARATION",
    "\"eval\" cannot be redeclared in strict mode",
);

// port: StrictModeCheck#EVAL_ASSIGNMENT
pub static EVAL_ASSIGNMENT: DiagnosticType = DiagnosticType::error(
    "JSC_EVAL_ASSIGNMENT",
    "the \"eval\" object cannot be reassigned in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_DECLARATION
pub static ARGUMENTS_DECLARATION: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_DECLARATION",
    "\"arguments\" cannot be redeclared in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_ASSIGNMENT
pub static ARGUMENTS_ASSIGNMENT: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_ASSIGNMENT",
    "the \"arguments\" object cannot be reassigned in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_CALLEE_FORBIDDEN
pub static ARGUMENTS_CALLEE_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_CALLEE_FORBIDDEN",
    "\"arguments.callee\" cannot be used in strict mode",
);

// port: StrictModeCheck#ARGUMENTS_CALLER_FORBIDDEN
pub static ARGUMENTS_CALLER_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_ARGUMENTS_CALLER_FORBIDDEN",
    "\"arguments.caller\" cannot be used in strict mode",
);

// port: StrictModeCheck#FUNCTION_CALLER_FORBIDDEN
pub static FUNCTION_CALLER_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_FUNCTION_CALLER_FORBIDDEN",
    "A function''s \"caller\" property cannot be used in strict mode",
);

// port: StrictModeCheck#FUNCTION_ARGUMENTS_PROP_FORBIDDEN
pub static FUNCTION_ARGUMENTS_PROP_FORBIDDEN: DiagnosticType = DiagnosticType::error(
    "JSC_FUNCTION_ARGUMENTS_PROP_FORBIDDEN",
    "A function''s \"arguments\" property cannot be used in strict mode",
);

// port: StrictModeCheck#DELETE_VARIABLE
pub static DELETE_VARIABLE: DiagnosticType = DiagnosticType::error(
    "JSC_DELETE_VARIABLE",
    "variables, functions, and arguments cannot be deleted in strict mode",
);

// port: StrictModeCheck#DUPLICATE_MEMBER
pub static DUPLICATE_MEMBER: DiagnosticType = DiagnosticType::warning(
    "JSC_DUPLICATE_MEMBER",
    "Class or object literal contains duplicate member \"{0}\". In non-strict code, the last duplicate will overwrite the others.",
);

/// Checks that the code obeys the static restrictions of strict mode:
///
/// 1. No use of "with".
/// 2. No deleting variables, functions, or arguments.
/// 3. No re-declarations or assignments of "eval" or arguments.
/// 4. No use of arguments.callee
/// 5. No use of arguments.caller
/// 6. Class: Always under strict mode
/// 7. In addition, no duplicate class method names
pub struct StrictModeCheck {
    default_level: CheckLevel,
}

impl StrictModeCheck {
    // port: StrictModeCheck#StrictModeCheck
    pub fn new(_compiler: &AbstractCompiler, default_level: CheckLevel) -> Self {
        Self { default_level }
    }

    /// Reports a warning for with statements.
    // port: StrictModeCheck#checkWith
    fn check_with(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let info = n.get_jsdoc_info(compiler);
        let allow_with = info
            .as_ref()
            .is_some_and(|info| info.get_suppressions().contains(&JsString::from("with")));
        if !allow_with {
            self.report(compiler, n, &USE_OF_WITH, &[]);
        }
    }

    /// Determines if the given name is a declaration, which can be a declaration of a variable,
    /// function, or argument.
    // port: StrictModeCheck#isDeclaration
    fn is_declaration(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::LET | Token::CONST | Token::VAR | Token::CATCH => true,
            Token::FUNCTION => Some(n) == parent.get_first_child(ast),
            Token::PARAM_LIST => n.get_grandparent(ast).unwrap().is_function(ast),
            _ => false,
        }
    }

    /// Checks that an assignment is not to the "arguments" object.
    // port: StrictModeCheck#checkAssignment
    fn check_assignment(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.get_first_child(compiler).unwrap();
        if first.is_name(compiler) {
            if first.get_string(compiler) == "arguments" {
                self.report(compiler, n, &ARGUMENTS_ASSIGNMENT, &[]);
            } else if first.get_string(compiler) == "eval" {
                // Note that assignment to eval is already illegal because any use of
                // that name is illegal.
                self.report(compiler, n, &EVAL_ASSIGNMENT, &[]);
            }
        }
    }

    /// Checks that variables, functions, and arguments are not deleted.
    // port: StrictModeCheck#checkDelete
    fn check_delete(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let first = n.get_first_child(t).unwrap();
        if first.is_name(t) {
            let name = first.get_string(t);
            let scope = t.get_scope();
            let v = scope.get_var(t.get_compiler(), name);
            if v.is_some() {
                self.report(t.get_compiler(), n, &DELETE_VARIABLE, &[]);
            }
        }
    }

    /// Checks that object literal keys or class method names are valid.
    // port: StrictModeCheck#checkObjectLiteralOrClass
    fn check_object_literal_or_class(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut getters: IndexSet<JsString> = IndexSet::new();
        let mut setters: IndexSet<JsString> = IndexSet::new();
        let mut static_getters: IndexSet<JsString> = IndexSet::new();
        let mut static_setters: IndexSet<JsString> = IndexSet::new();

        /*
         * Iterate backwards because the last duplicate is the one that will be used in sloppy or
         * ES6 code. The earlier duplicates are the ones that should be removed.
         */
        let mut key = n.get_last_child(compiler);
        while let Some(k) = key {
            if k.is_empty(compiler)
                || k.is_computed_prop(compiler)
                || k.is_spread(compiler)
                || k.is_computed_field_def(compiler)
                // Computed properties cannot be computed at compile time
                || NodeUtil::is_class_static_block(compiler, k)
            // Will not check since whether duplicates are declared/assigned cannot be determined
            // at compile time since we do not know which code will be run
            {
                key = k.get_previous(compiler);
                continue;
            }

            let key_name = k.get_string(compiler);
            if !k.is_setter_def(compiler) {
                // normal property and getter cases
                let set = if k.is_static_member(compiler) {
                    &mut static_getters
                } else {
                    &mut getters
                };
                if !set.insert(key_name.clone()) {
                    self.report(
                        compiler,
                        k,
                        &DUPLICATE_MEMBER,
                        &[&key_name.to_string_lossy()],
                    );
                }
            }
            if !k.is_getter_def(compiler) {
                // normal property and setter cases
                let set = if k.is_static_member(compiler) {
                    &mut static_setters
                } else {
                    &mut setters
                };
                if !set.insert(key_name.clone()) {
                    self.report(
                        compiler,
                        k,
                        &DUPLICATE_MEMBER,
                        &[&key_name.to_string_lossy()],
                    );
                }
            }
            key = k.get_previous(compiler);
        }
    }

    // port: StrictModeCheck#isFunctionType
    fn is_function_type(compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        let type_ = n.get_jstype(compiler);
        match type_ {
            Some(type_) => {
                let (reg, _) = compiler.get_type_registry_field_and_ast();
                type_.is_function_type(reg.expect(
                    "StrictModeCheck: a node has a JSType but the compiler has no JSTypeRegistry",
                ))
            }
            None => false,
        }
    }

    // port: StrictModeCheck#report
    fn report(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        diagnostic: &'static DiagnosticType,
        args: &[&str],
    ) {
        let error = JSError::builder(diagnostic, args)
            .set_level(self.default_level)
            .set_node(compiler, n)
            .build();
        compiler.report(error);
    }
}

impl CompilerPass for StrictModeCheck {
    // port: StrictModeCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse_roots(compiler, self, externs, root);
        NodeTraversal::traverse(compiler, root, &mut NonExternChecks { outer: self });
    }
}

impl Callback for StrictModeCheck {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: StrictModeCheck#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_assign(t) {
            self.check_assignment(t.get_compiler(), n);
        } else if n.is_del_prop(t) {
            self.check_delete(t, n);
        } else if n.is_object_lit(t) {
            self.check_object_literal_or_class(t.get_compiler(), n);
        } else if n.is_class(t) {
            let last = n.get_last_child(t).unwrap();
            self.check_object_literal_or_class(t.get_compiler(), last);
        } else if n.is_with(t) {
            self.check_with(t.get_compiler(), n);
        }
    }
}

/// Checks that are performed on non-extern code only.
struct NonExternChecks<'a> {
    outer: &'a StrictModeCheck,
}

impl NonExternChecks<'_> {
    /// Checks for illegal declarations.
    // port: StrictModeCheck.NonExternChecks#checkDeclaration
    fn check_declaration(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.get_string(compiler) == "eval" {
            self.outer.report(compiler, n, &EVAL_DECLARATION, &[]);
        } else if n.get_string(compiler) == "arguments" {
            self.outer.report(compiler, n, &ARGUMENTS_DECLARATION, &[]);
        }
    }

    /// Checks that the arguments.callee is not used.
    // port: StrictModeCheck.NonExternChecks#checkGetProp
    fn check_get_prop(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let target = n.get_first_child(compiler).unwrap();
        let name = n.get_string(compiler);
        if name == "callee" {
            if target.is_name(compiler) && target.get_string(compiler) == "arguments" {
                self.outer
                    .report(compiler, n, &ARGUMENTS_CALLEE_FORBIDDEN, &[]);
            }
        } else if name == "caller" {
            if target.is_name(compiler) && target.get_string(compiler) == "arguments" {
                self.outer
                    .report(compiler, n, &ARGUMENTS_CALLER_FORBIDDEN, &[]);
            } else if StrictModeCheck::is_function_type(compiler, target) {
                self.outer
                    .report(compiler, n, &FUNCTION_CALLER_FORBIDDEN, &[]);
            }
        } else if name == "arguments" && StrictModeCheck::is_function_type(compiler, target) {
            self.outer
                .report(compiler, n, &FUNCTION_ARGUMENTS_PROP_FORBIDDEN, &[]);
        }
    }
}

impl Callback for NonExternChecks<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: StrictModeCheck.NonExternChecks#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(t) && StrictModeCheck::is_declaration(t, n) {
            self.check_declaration(t.get_compiler(), n);
        } else if n.is_get_prop(t) {
            self.check_get_prop(t.get_compiler(), n);
        }
    }
}
