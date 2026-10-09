/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckJSDocStyle.java.

//! Checks for various JSDoc-related style issues, such as function definitions without JsDoc,
//! params with no corresponding @param annotation, coding conventions not being respected, etc.

#![allow(clippy::if_same_then_else)] // Java reports `isOverride() ? "" : ""` (identical arms).

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_group::DiagnosticGroup,
    diagnostic_type::DiagnosticType,
    export_test_functions::ExportTestFunctions,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_not_null, check_state, js_string::JsString, js_type_expression::JSTypeExpression,
    jsdoc_info::JSDocInfo, node::NodeId, token::Token,
};
use std::sync::{Arc, LazyLock};

// port: CheckJSDocStyle#CLASS_DISALLOWED_JSDOC
pub static CLASS_DISALLOWED_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_CLASS_DISALLOWED_JSDOC",
    "@constructor annotations are redundant on classes.",
);

// port: CheckJSDocStyle#MISSING_JSDOC
pub static MISSING_JSDOC: DiagnosticType =
    DiagnosticType::disabled("JSC_MISSING_JSDOC", "Function must have JSDoc.");

// port: CheckJSDocStyle#INCORRECT_ANNOTATION_ON_GETTER_SETTER
pub static INCORRECT_ANNOTATION_ON_GETTER_SETTER: DiagnosticType = DiagnosticType::disabled(
    "JSC_TYPE_ON_GETTER_SETTER",
    "Getters and setters must not have @type annotations. Did you mean @return or @param instead?",
);

// port: CheckJSDocStyle#MISSING_PARAMETER_JSDOC
pub static MISSING_PARAMETER_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_PARAMETER_JSDOC",
    "Parameter must have JSDoc.{0}",
);

// port: CheckJSDocStyle#MIXED_PARAM_JSDOC_STYLES
pub static MIXED_PARAM_JSDOC_STYLES: DiagnosticType = DiagnosticType::disabled(
    "JSC_MIXED_PARAM_JSDOC_STYLES",
    "Functions may not use both @param annotations and inline JSDoc",
);

// port: CheckJSDocStyle#MISSING_RETURN_JSDOC
pub static MISSING_RETURN_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_RETURN_JSDOC",
    "Function that returns a value must have JSDoc indicating the return type.{0}",
);

// port: CheckJSDocStyle#OPTIONAL_PARAM_NOT_MARKED_OPTIONAL
pub static OPTIONAL_PARAM_NOT_MARKED_OPTIONAL: DiagnosticType = DiagnosticType::disabled(
    "JSC_OPTIONAL_PARAM_NOT_MARKED_OPTIONAL",
    "Parameter {0} is optional so it must have a JSDoc type ending with ''=''",
);

// port: CheckJSDocStyle#WRONG_NUMBER_OF_PARAMS
pub static WRONG_NUMBER_OF_PARAMS: DiagnosticType = DiagnosticType::disabled(
    "JSC_WRONG_NUMBER_OF_PARAMS",
    "Wrong number of @param annotations",
);

// port: CheckJSDocStyle#INCORRECT_PARAM_NAME
pub static INCORRECT_PARAM_NAME: DiagnosticType = DiagnosticType::disabled(
    "JSC_INCORRECT_PARAM_NAME",
    "Incorrect param name. Are your @param annotations in the wrong order?",
);

// port: CheckJSDocStyle#EXTERNS_FILES_SHOULD_BE_ANNOTATED
pub static EXTERNS_FILES_SHOULD_BE_ANNOTATED: DiagnosticType = DiagnosticType::disabled(
    "JSC_EXTERNS_FILES_SHOULD_BE_ANNOTATED",
    "Externs files should be annotated with @externs in the @fileoverview block.",
);

// port: CheckJSDocStyle#LICENSE_CONTAINS_AT_EXTERNS
pub static LICENSE_CONTAINS_AT_EXTERNS: DiagnosticType = DiagnosticType::disabled(
    "JSC_LICENSE_CONTAINS_AT_EXTERNS",
    "@license block contains an @externs annotation, which will be parsed as plain license text instead of an actual @externs annotation. You probably meant to put @externs in a separate @fileoverview block.",
);

// port: CheckJSDocStyle#PREFER_BACKTICKS_TO_AT_SIGN_CODE
pub static PREFER_BACKTICKS_TO_AT_SIGN_CODE: DiagnosticType = DiagnosticType::disabled(
    "JSC_PREFER_BACKTICKS_TO_AT_SIGN_CODE",
    "Use `some_code` instead of '{'@code some_code'}'.",
);

// port: CheckJSDocStyle#LINT_DIAGNOSTICS
pub static LINT_DIAGNOSTICS: LazyLock<Arc<DiagnosticGroup>> = LazyLock::new(|| {
    Arc::new(DiagnosticGroup::new(&[
        &CLASS_DISALLOWED_JSDOC,
        &MISSING_JSDOC,
        &INCORRECT_ANNOTATION_ON_GETTER_SETTER,
        &MISSING_PARAMETER_JSDOC,
        &MIXED_PARAM_JSDOC_STYLES,
        &MISSING_RETURN_JSDOC,
        &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
        &WRONG_NUMBER_OF_PARAMS,
        &INCORRECT_PARAM_NAME,
        &EXTERNS_FILES_SHOULD_BE_ANNOTATED,
        &LICENSE_CONTAINS_AT_EXTERNS,
        &PREFER_BACKTICKS_TO_AT_SIGN_CODE,
    ]))
});

// port: CheckJSDocStyle#ALL_DIAGNOSTICS
pub static ALL_DIAGNOSTICS: LazyLock<Arc<DiagnosticGroup>> = LazyLock::new(|| {
    Arc::new(DiagnosticGroup::new_from_groups(std::slice::from_ref(
        &*LINT_DIAGNOSTICS,
    )))
});

pub struct CheckJSDocStyle;

impl CheckJSDocStyle {
    // port: CheckJSDocStyle#CheckJSDocStyle
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: CheckJSDocStyle#checkForAtSignCodePresence
    fn check_for_at_sign_code_presence(
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        js_doc: Option<&JSDocInfo>,
    ) {
        let Some(js_doc) = js_doc else {
            return;
        };
        if js_doc.is_at_sign_code_present() {
            t.report(n, &PREFER_BACKTICKS_TO_AT_SIGN_CODE, &[]);
        }
    }

    // port: CheckJSDocStyle#visitNonFunction
    fn visit_non_function(t: &mut NodeTraversal<'_>, n: NodeId) {
        let js_doc = n.get_jsdoc_info(t);
        Self::check_for_at_sign_code_presence(t, n, js_doc.as_deref());
    }

    // port: CheckJSDocStyle#checkStyleForPrivateProperties
    fn check_style_for_private_properties(t: &mut NodeTraversal<'_>, n: NodeId) {
        let js_doc = NodeUtil::get_best_jsdoc_info(t, n);
        Self::check_for_at_sign_code_presence(t, n, js_doc.as_deref());
    }

    // port: CheckJSDocStyle#checkNoTypeOnGettersAndSetters
    fn check_no_type_on_getters_and_setters(
        t: &mut NodeTraversal<'_>,
        function: NodeId,
        js_doc: Option<&JSDocInfo>,
    ) {
        if function.get_grandparent(t).unwrap().is_class_members(t) {
            let member_node = function.get_parent(t).unwrap();
            if (member_node.is_setter_def(t) || member_node.is_getter_def(t))
                && js_doc.is_some_and(|js_doc| js_doc.has_type())
            {
                t.report(function, &INCORRECT_ANNOTATION_ON_GETTER_SETTER, &[]);
            }
        }
    }

    // port: CheckJSDocStyle#visitFunction
    fn visit_function(t: &mut NodeTraversal<'_>, function: NodeId) {
        let js_doc = NodeUtil::get_best_jsdoc_info(t, function);
        Self::check_for_at_sign_code_presence(t, function, js_doc.as_deref());
        if js_doc.is_none() && !Self::has_any_inline_js_doc(t, function) {
            Self::check_missing_js_doc(t, function);
        } else {
            if t.in_global_scope()
                || Self::has_any_inline_js_doc(t, function)
                || !js_doc.as_ref().unwrap().get_parameter_names().is_empty()
                || js_doc.as_ref().unwrap().has_return_type()
                || js_doc.as_ref().unwrap().is_override()
            {
                Self::check_params(t, function, js_doc.as_deref());
            }
            Self::check_no_type_on_getters_and_setters(t, function, js_doc.as_deref());
            Self::check_return(t, function, js_doc.as_deref());
        }
    }

    // port: CheckJSDocStyle#visitClass
    fn visit_class(t: &mut NodeTraversal<'_>, cls: NodeId) {
        let js_doc = NodeUtil::get_best_jsdoc_info(t, cls);
        Self::check_for_at_sign_code_presence(t, cls, js_doc.as_deref());
        let Some(js_doc) = js_doc else {
            return;
        };
        if js_doc.is_constructor() {
            t.report(cls, &CLASS_DISALLOWED_JSDOC, &[]);
        }
    }

    // port: CheckJSDocStyle#checkMissingJsDoc
    fn check_missing_js_doc(t: &mut NodeTraversal<'_>, function: NodeId) {
        if Self::is_function_that_should_have_js_doc(t, function)
            && !Self::is_test_method(t, function)
        {
            t.report(function, &MISSING_JSDOC, &[]);
        }
    }

    /// Whether the given function should have JSDoc. True if it's a function declared in the
    /// global scope, or a method on a class which is declared in the global scope.
    // port: CheckJSDocStyle#isFunctionThatShouldHaveJsDoc
    fn is_function_that_should_have_js_doc(t: &mut NodeTraversal<'_>, function: NodeId) -> bool {
        if !(t.in_global_hoist_scope() || t.in_module_scope()) {
            // TODO(b/233631820): this should check for the module hoist scope instead
            return false;
        }
        if NodeUtil::is_function_declaration(t, function) {
            return true;
        }
        if NodeUtil::is_name_declaration(t, function.get_grandparent(t))
            || function.get_parent(t).unwrap().is_assign(t)
        {
            return true;
        }

        if function.get_parent(t).unwrap().is_export(t) {
            return true;
        }

        if function.get_grandparent(t).unwrap().is_class_members(t) {
            let member_node = function.get_parent(t).unwrap();
            if member_node.is_member_function_def(t) {
                // A constructor with no parameters doesn't need JSDoc,
                // but all other member functions do.
                return !Self::is_constructor_without_parameters(t, function);
            } else if member_node.is_getter_def(t) || member_node.is_setter_def(t) {
                return true;
            }
        }

        let grandparent = function.get_grandparent(t).unwrap();
        grandparent.is_object_lit(t)
            && NodeUtil::is_call_to(t, grandparent.get_parent(t).unwrap(), "Polymer")
    }

    /// Whether this is a test method (test* or setup/teardown) that does not require JSDoc
    // port: CheckJSDocStyle#isTestMethod
    fn is_test_method(t: &NodeTraversal<'_>, function: NodeId) -> bool {
        let best_l_value = NodeUtil::get_best_l_value(t, function);
        let name: Option<JsString> = if best_l_value.is_some() {
            NodeUtil::get_best_l_value_name(t, best_l_value)
        } else {
            None
        };
        name.is_some_and(|name| ExportTestFunctions::is_test_function(Some(&name)))
    }

    // port: CheckJSDocStyle#isConstructorWithoutParameters
    fn is_constructor_without_parameters(t: &NodeTraversal<'_>, function: NodeId) -> bool {
        NodeUtil::is_es6_constructor(t, function)
            && !NodeUtil::get_function_parameters(t, function).has_children(t)
    }

    // port: CheckJSDocStyle#checkParams
    fn check_params(t: &mut NodeTraversal<'_>, function: NodeId, js_doc: Option<&JSDocInfo>) {
        if js_doc.is_some_and(|js_doc| js_doc.get_type().is_some()) {
            // Sometimes functions are declared with @type {function(Foo, Bar)} instead of
            //   @param {Foo} foo
            //   @param {Bar} bar
            // which is fine.
            return;
        }

        let params_from_js_doc: Vec<JsString> = match js_doc {
            None => Vec::new(),
            Some(js_doc) => js_doc.get_parameter_names().into_iter().collect(),
        };
        if params_from_js_doc.is_empty() {
            Self::check_inline_params(t, function, js_doc);
        } else {
            let js_doc = js_doc.unwrap();
            let param_list = NodeUtil::get_function_parameters(t, function);
            if !param_list.has_x_children(t, params_from_js_doc.len() as i32) {
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    param_list,
                    &WRONG_NUMBER_OF_PARAMS,
                    &[if js_doc.is_override() { "" } else { "" }],
                ));
                return;
            }

            let mut param = param_list.get_first_child(t).unwrap();
            for s in &params_from_js_doc {
                if param.get_jsdoc_info(t).is_some() {
                    t.report(param, &MIXED_PARAM_JSDOC_STYLES, &[]);
                }
                let name = s;
                let param_type = js_doc.get_parameter_type(name.clone());
                if Self::check_param(t, param, Some(name), param_type.as_deref()) {
                    return;
                }
                // Java: `param = param.getNext()`; a null next is only read on the next
                // iteration, which the paramList child-count check above rules out.
                if let Some(next) = param.get_next(t) {
                    param = next;
                }
            }
        }
    }

    /// Checks that the inline type annotations are correct.
    // port: CheckJSDocStyle#checkInlineParams
    fn check_inline_params(
        t: &mut NodeTraversal<'_>,
        function: NodeId,
        fn_js_doc: Option<&JSDocInfo>,
    ) {
        let param_list = NodeUtil::get_function_parameters(t, function);

        let mut param = param_list.get_first_child(t);
        while let Some(p) = param {
            let js_doc = if p.is_default_value(t) {
                p.get_first_child(t).unwrap().get_jsdoc_info(t)
            } else {
                p.get_jsdoc_info(t)
            };
            match js_doc {
                None => {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(
                        compiler,
                        p,
                        &MISSING_PARAMETER_JSDOC,
                        &[if fn_js_doc.is_some_and(|d| d.is_override()) {
                            ""
                        } else {
                            ""
                        }],
                    ));
                    return;
                }
                Some(js_doc) => {
                    let param_type = js_doc.get_type();
                    check_not_null!(
                        param_type.as_ref(),
                        "Inline JSDoc info should always have a type"
                    );
                    Self::check_param(t, p, None, param_type.as_deref());
                }
            }
            param = p.get_next(t);
        }
    }

    /// Checks that the given parameter node has the given name, and that the given type is
    /// compatible.
    ///
    /// @param param If this is a non-NAME node, such as a destructuring pattern, skip the name
    ///     check.
    /// @param name If null, skip the name check
    /// @return Whether a warning was reported
    // port: CheckJSDocStyle#checkParam
    fn check_param(
        t: &mut NodeTraversal<'_>,
        param: NodeId,
        name: Option<&JsString>,
        param_type: Option<&JSTypeExpression>,
    ) -> bool {
        let name_optional;
        let mut node_to_check = param;
        if param.is_default_value(t) {
            node_to_check = param.get_first_child(t).unwrap();
            name_optional = true;
        } else if param.is_name(t) {
            name_optional = param.get_string(t).starts_with(&JsString::from("opt_"));
        } else {
            check_state!(
                param.is_destructuring_pattern(t) || param.is_rest(t),
                "%s",
                param.to_string(t)
            );
            name_optional = false;
        }

        let name: JsString = match name {
            Some(name) if node_to_check.is_name(t) => {
                if !node_to_check.matches_qualified_name(t, name.clone()) {
                    t.report(node_to_check, &INCORRECT_PARAM_NAME, &[]);
                    return true;
                }
                name.clone()
            }
            // Skip the name check, but use "<unknown name>" for other errors that might be
            // reported.
            _ => JsString::from("<unknown name>"),
        };

        if !name_optional {
            return false;
        }

        let js_doc_optional = param_type.is_some_and(|param_type| param_type.is_optional_arg(t));
        if js_doc_optional {
            return false;
        }

        let error_source = match param_type {
            Some(param_type) => param_type.get_root(),
            None => node_to_check,
        };
        t.report(
            error_source,
            &OPTIONAL_PARAM_NOT_MARKED_OPTIONAL,
            &[&name.to_string()],
        );
        true
    }

    // port: CheckJSDocStyle#isDefaultAssignedParamWithInlineJsDoc
    fn is_default_assigned_param_with_inline_js_doc(t: &NodeTraversal<'_>, param: NodeId) -> bool {
        if param.is_default_value(t)
            && param.has_children(t)
            && param.get_first_child(t).unwrap().is_name(t)
        {
            return param
                .get_first_child(t)
                .unwrap()
                .get_jsdoc_info(t)
                .is_some();
        }
        false
    }

    // port: CheckJSDocStyle#hasAnyInlineJsDoc
    fn has_any_inline_js_doc(t: &NodeTraversal<'_>, function: NodeId) -> bool {
        if function
            .get_first_child(t)
            .unwrap()
            .get_jsdoc_info(t)
            .is_some()
        {
            // Inline return annotation.
            return true;
        }
        let mut param = NodeUtil::get_function_parameters(t, function).get_first_child(t);
        while let Some(p) = param {
            if p.get_jsdoc_info(t).is_some()
                || Self::is_default_assigned_param_with_inline_js_doc(t, p)
            {
                return true;
            }
            param = p.get_next(t);
        }
        false
    }

    // port: CheckJSDocStyle#checkReturn
    fn check_return(t: &mut NodeTraversal<'_>, function: NodeId, js_doc: Option<&JSDocInfo>) {
        if js_doc.is_some_and(|js_doc| {
            js_doc.has_type() || js_doc.is_constructor() || js_doc.has_return_type()
        }) {
            return;
        }

        if NodeUtil::is_es6_constructor(t, function) {
            // ES6 class constructors should never have "@return".
            return;
        }

        if function
            .get_first_child(t)
            .unwrap()
            .get_jsdoc_info(t)
            .is_some()
        {
            return;
        }

        let mut finder = FindNonTrivialReturn { found: false };
        let body = function.get_last_child(t).unwrap();
        NodeTraversal::traverse(t.get_compiler(), body, &mut finder);

        if finder.found {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                function,
                &MISSING_RETURN_JSDOC,
                &[if js_doc.is_some_and(|d| d.is_override()) {
                    ""
                } else {
                    ""
                }],
            ));
        }
    }

    // port: CheckJSDocStyle#checkLicenseComment
    fn check_license_comment(t: &mut NodeTraversal<'_>, n: NodeId) {
        let Some(info) = n.get_jsdoc_info(t) else {
            return;
        };
        let Some(license) = info.get_license() else {
            return;
        };
        if license.index_of(&JsString::from("@externs")) >= 0 {
            t.report(n, &LICENSE_CONTAINS_AT_EXTERNS, &[]);
        }
    }
}

impl CompilerPass for CheckJSDocStyle {
    // port: CheckJSDocStyle#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        NodeTraversal::traverse(compiler, externs, &mut ExternsCallback);
    }
}

impl Callback for CheckJSDocStyle {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckJSDocStyle#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _unused: Option<NodeId>) {
        match n.get_token(t) {
            Token::FUNCTION => Self::visit_function(t, n),
            Token::CLASS => Self::visit_class(t, n),
            Token::ASSIGN => Self::check_style_for_private_properties(t, n),
            Token::VAR | Token::LET | Token::CONST | Token::STRING_KEY => {}
            Token::MEMBER_FUNCTION_DEF | Token::GETTER_DEF | Token::SETTER_DEF => {
                // Don't need to call visitFunction because this JSDoc will be visited when the
                // function is visited.
                if NodeUtil::get_enclosing_class(t, n).is_some() {
                    Self::check_style_for_private_properties(t, n);
                }
            }
            Token::SCRIPT => Self::check_license_comment(t, n),
            _ => Self::visit_non_function(t, n),
        }
    }
}

struct FindNonTrivialReturn {
    found: bool,
}

impl Callback for FindNonTrivialReturn {
    // port: CheckJSDocStyle.FindNonTrivialReturn#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if self.found {
            return false;
        }

        // Shallow traversal, since we don't need to inspect within functions or expressions.
        if NodeUtil::is_shallow_statement_tree(t, parent) {
            if n.is_return(t) && n.has_children(t) {
                self.found = true;
                return false;
            }
            return true;
        }
        false
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

struct ExternsCallback;

impl Callback for ExternsCallback {
    // port: CheckJSDocStyle.ExternsCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        parent.is_none() || n.is_script(t)
    }

    // port: CheckJSDocStyle.ExternsCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_script(t) {
            let info = n.get_jsdoc_info(t);
            if info.is_none_or(|info| !info.is_externs()) {
                t.report(n, &EXTERNS_FILES_SHOULD_BE_ANNOTATED, &[]);
            }
        }
    }
}
