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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CheckJSDoc.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

#![allow(
    clippy::collapsible_match,
    clippy::if_same_then_else,
    clippy::nonminimal_bool
)] // Retain Java control flow.
use crate::{
    abstract_compiler::AbstractCompiler,
    closure_rewrite_module::ClosureRewriteModule,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::{NodeUtil, Visitor},
    type_check::ILLEGAL_IMPLICIT_CAST,
};
use closure_parsing::parser::trees::comment;
use closure_rhino::{
    check_state,
    java_lang::pattern::Pattern,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::{Arc, LazyLock};

// port: CheckJSDoc#MISPLACED_MSG_ANNOTATION
pub static MISPLACED_MSG_ANNOTATION: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISPLACED_MSG_ANNOTATION",
    "Misplaced message annotation. @desc, @meaning, and @alternateMessageId annotations should be only on message nodes.\nMessage constants must be prefixed with 'MSG_'.",
);

// port: CheckJSDoc#MISPLACED_ANNOTATION
pub static MISPLACED_ANNOTATION: DiagnosticType =
    DiagnosticType::warning("JSC_MISPLACED_ANNOTATION", "Misplaced {0} annotation. {1}");

// port: CheckJSDoc#MISPLACED_TS_TYPE_ANNOTATION
pub static MISPLACED_TS_TYPE_ANNOTATION: DiagnosticType = DiagnosticType::warning(
    "JSC_MISPLACED_TS_TYPE_ANNOTATION",
    "Misplaced @tsType annotation. {0}",
);

// port: CheckJSDoc#ANNOTATION_DEPRECATED
pub static ANNOTATION_DEPRECATED: DiagnosticType = DiagnosticType::warning(
    "JSC_ANNOTATION_DEPRECATED",
    "The {0} annotation is deprecated. {1}",
);

// port: CheckJSDoc#DISALLOWED_MEMBER_JSDOC
pub static DISALLOWED_MEMBER_JSDOC: DiagnosticType = DiagnosticType::warning(
    "JSC_DISALLOWED_MEMBER_JSDOC",
    "Class level JSDocs (@interface, @extends, etc.) are not allowed on class members",
);

// port: CheckJSDoc#ARROW_FUNCTION_AS_CONSTRUCTOR
pub static ARROW_FUNCTION_AS_CONSTRUCTOR: DiagnosticType = DiagnosticType::error(
    "JSC_ARROW_FUNCTION_AS_CONSTRUCTOR",
    "Arrow functions cannot be used as constructors",
);

// port: CheckJSDoc#BAD_REST_PARAMETER_ANNOTATION
pub static BAD_REST_PARAMETER_ANNOTATION: DiagnosticType = DiagnosticType::warning(
    "JSC_BAD_REST_PARAMETER_ANNOTATION",
    "Missing \"...\" in type annotation for rest parameter.",
);

// port: CheckJSDoc#DEFAULT_PARAM_MUST_BE_MARKED_OPTIONAL
pub static DEFAULT_PARAM_MUST_BE_MARKED_OPTIONAL: DiagnosticType = DiagnosticType::error(
    "JSC_DEFAULT_PARAM_MUST_BE_MARKED_OPTIONAL",
    "Inline JSDoc on default parameters must be marked as optional",
);

// port: CheckJSDoc#INVALID_NO_SIDE_EFFECT_ANNOTATION
pub static INVALID_NO_SIDE_EFFECT_ANNOTATION: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_NO_SIDE_EFFECT_ANNOTATION",
    "@nosideeffects may only appear in externs files.",
);

// port: CheckJSDoc#INVALID_MODIFIES_ANNOTATION
pub static INVALID_MODIFIES_ANNOTATION: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_MODIFIES_ANNOTATION",
    "@modifies may only appear in externs files.",
);

// port: CheckJSDoc#INVALID_DEFINE_ON_LET
pub static INVALID_DEFINE_ON_LET: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DEFINE_ON_LET",
    "variables annotated with @define may only be declared with VARs, ASSIGNs, or CONSTs",
);

// port: CheckJSDoc#MISPLACED_SUPPRESS
pub static MISPLACED_SUPPRESS: DiagnosticType = DiagnosticType::warning(
    "JSC_MISPLACED_SUPPRESS",
    "@suppress annotation not allowed here. See https://github.com/google/closure-compiler/wiki/@suppress-annotations",
);

// port: CheckJSDoc#JSDOC_IN_BLOCK_COMMENT
pub static JSDOC_IN_BLOCK_COMMENT: DiagnosticType = DiagnosticType::warning(
    "JSC_JSDOC_IN_BLOCK_COMMENT",
    "Non-JSDoc comment has annotations. Did you mean to start it with '/**'?",
);

// port: CheckJSDoc#JSDOC_ON_RETURN
pub static JSDOC_ON_RETURN: DiagnosticType = DiagnosticType::warning(
    "JSC_JSDOC_ON_RETURN",
    "JSDoc annotations are not supported on return.",
);

// port: CheckJSDoc#COMMENT_PATTERN
static COMMENT_PATTERN: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("(/|(\n[ \t]*))\\*[ \t]*@[a-zA-Z]+[ \t\n{]"));

/// Checks for misplaced, misused or deprecated JSDoc annotations.
pub struct CheckJSDoc {
    in_externs: bool,
    check_js_doc_types_visitor: CheckJsdocTypes,
}

impl CheckJSDoc {
    // port: CheckJSDoc#CheckJSDoc
    pub fn new() -> Self {
        Self {
            in_externs: false,
            check_js_doc_types_visitor: CheckJsdocTypes,
        }
    }

    // port: CheckJSDoc#checkJsDocInBlockComments
    /// Checks for block comments (e.g. starting with /*) that look like they are JsDoc, and thus
    /// should start with /**.
    fn check_js_doc_in_block_comments(compiler: &mut AbstractCompiler, file_name: &str) {
        if !compiler.get_options().preserves_detailed_source_info() {
            // Comments only available if preservesDetailedSourceInfo is true.
            return;
        }

        let mut errors = Vec::new();
        for comment in compiler.get_comments(file_name).unwrap() {
            if comment.type_ == comment::Type::BLOCK
                && COMMENT_PATTERN.matcher(&comment.value.to_string()).find()
            {
                errors.push(JSError::make_with_source_location(
                    file_name,
                    comment.location.start.line + 1,
                    comment.location.start.column,
                    &JSDOC_IN_BLOCK_COMMENT,
                    &[],
                ));
            }
        }
        for error in errors {
            compiler.report(error);
        }
    }

    // port: CheckJSDoc#validateSuppress
    fn validate_suppress(compiler: &mut AbstractCompiler, n: NodeId, info: Option<&JSDocInfo>) {
        let Some(info) = info else {
            return;
        };
        if info.get_suppressions().is_empty() {
            return;
        }
        match n.get_token(compiler) {
            Token::FUNCTION
            | Token::CLASS
            | Token::VAR
            | Token::LET
            | Token::CONST
            | Token::SCRIPT
            | Token::MEMBER_FUNCTION_DEF
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_FIELD_DEF => {
                // Suppressions are always valid here.
                return;
            }
            Token::COMPUTED_PROP => {
                if n.get_last_child(compiler).unwrap().is_function(compiler) {
                    return; // Suppressions are valid on computed properties that declare functions.
                }
            }
            Token::STRING_KEY => {
                if n.get_parent(compiler).unwrap().is_object_lit(compiler) {
                    return;
                }
            }
            Token::WITH => {
                if Self::contains_only_suppression_for(info, "with") {
                    return;
                }
            }
            _ => {}
        }
        if Self::contains_only_suppression_for(info, "missingRequire") {
            return;
        }
        if n.get_parent(compiler).unwrap().is_expr_result(compiler) {
            return;
        }
        let error = JSError::make(compiler, n, &MISPLACED_SUPPRESS, &[]);
        compiler.report(error);
    }

    // port: CheckJSDoc#containsOnlySuppressionFor
    fn contains_only_suppression_for(jsdoc: &JSDocInfo, allowed_suppression: &str) -> bool {
        let suppressions = jsdoc.get_suppressions();
        suppressions.len() == 1 && *suppressions.iter().next().unwrap() == allowed_suppression
    }

    // port: CheckJSDoc#validateTypedefs
    fn validate_typedefs(compiler: &mut AbstractCompiler, n: NodeId, info: Option<&JSDocInfo>) {
        let Some(info) = info else {
            return;
        };
        if !info.has_typedef_type() {
            return;
        }
        if Self::is_class_decl(compiler, n) {
            Self::report_misplaced(
                compiler,
                n,
                "typedef",
                "@typedef is not allowed on a class declaration.",
            );
            return;
        }
        let lvalue = if NodeUtil::is_name_declaration(compiler, Some(n)) || n.is_assign(compiler) {
            n.get_first_child(compiler).unwrap()
        } else {
            n
        };
        if !lvalue.is_qualified_name(compiler) {
            Self::report_misplaced(
                compiler,
                n,
                "typedef",
                "@typedef is only allowed on qualified name declarations. Did you mean @type?",
            );
        } else if Self::is_prototype_or_instance_decl(compiler, lvalue) {
            Self::report_misplaced(
                compiler,
                n,
                "typedef",
                "@typedef is not allowed on instance or prototype properties. Did you mean @type?",
            );
        }
    }

    // port: CheckJSDoc#validateTemplates
    fn validate_templates(compiler: &mut AbstractCompiler, n: NodeId, info: Option<&JSDocInfo>) {
        if let Some(info) = info
            && !info.get_template_type_names().is_empty()
            && !info.is_constructor_or_interface()
            && !Self::is_class_decl(compiler, n)
            && !info.contains_function_declaration(compiler)
            && Self::get_function_decl(compiler, n).is_none()
        {
            Self::report_misplaced(
                compiler,
                n,
                "template",
                "@template is only allowed in class, constructor, interface, function or method declarations",
            );
        }
    }

    // port: CheckJSDoc#getFunctionDecl
    /// Returns the function node associated with the function declaration associated with the
    /// specified node, no null if no such function exists.
    fn get_function_decl(ast: &Ast, n: NodeId) -> Option<NodeId> {
        if n.is_function(ast) {
            return Some(n);
        }
        if n.is_member_function_def(ast) {
            return n.get_first_child(ast);
        }
        if NodeUtil::is_name_declaration(ast, Some(n))
            && n.get_first_first_child(ast).is_some()
            && n.get_first_first_child(ast).unwrap().is_function(ast)
        {
            return n.get_first_first_child(ast);
        }

        if n.is_assign(ast)
            && n.get_first_child(ast).unwrap().is_qualified_name(ast)
            && n.get_last_child(ast).unwrap().is_function(ast)
        {
            return n.get_last_child(ast);
        }

        if n.is_getter_def(ast) || n.is_setter_def(ast) {
            return n.get_first_child(ast);
        }

        if n.is_computed_prop(ast) && n.get_last_child(ast).unwrap().is_function(ast) {
            return n.get_last_child(ast);
        }

        None
    }

    // port: CheckJSDoc#isClassDecl
    fn is_class_decl(compiler: &AbstractCompiler, n: NodeId) -> bool {
        Self::is_class(compiler, n)
            || (n.is_assign(compiler)
                && Self::is_class(compiler, n.get_last_child(compiler).unwrap()))
            || (NodeUtil::is_name_declaration(compiler, Some(n))
                && Self::is_name_initialize_with_class(compiler, n.get_first_child(compiler)))
            || Self::is_name_initialize_with_class(compiler, Some(n))
    }

    // port: CheckJSDoc#isNameInitializeWithClass
    fn is_name_initialize_with_class(compiler: &AbstractCompiler, n: Option<NodeId>) -> bool {
        n.is_some_and(|n| {
            n.is_name(compiler)
                && n.has_children(compiler)
                && Self::is_class_decl(compiler, n.get_first_child(compiler).unwrap())
        })
    }

    // port: CheckJSDoc#isClass
    fn is_class(compiler: &AbstractCompiler, n: NodeId) -> bool {
        n.is_class(compiler)
            || (n.is_call(compiler)
                && compiler
                    .get_coding_convention()
                    .is_class_factory_call(compiler, n))
    }

    // port: CheckJSDoc#isPrototypeOrInstanceDecl
    fn is_prototype_or_instance_decl(ast: &Ast, n: NodeId) -> bool {
        if n.is_string_key(ast) {
            return false;
        }
        if NodeUtil::is_prototype_property(ast, n) {
            return true;
        }
        let receiver = NodeUtil::get_root_of_qualified_name(ast, n);
        receiver.is_this(ast) || receiver.is_super(ast)
    }

    // port: CheckJSDoc#validateClassLevelJsDoc
    /// Checks that class-level annotations like @interface/@extends are not used on member
    /// functions.
    fn validate_class_level_js_doc(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        if let Some(info) = info
            && n.is_member_function_def(compiler)
            && Self::has_class_level_js_doc(info)
        {
            Self::report(compiler, n, &DISALLOWED_MEMBER_JSDOC, &[]);
        }
    }

    // port: CheckJSDoc#validateAbstractJsDoc
    fn validate_abstract_js_doc(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        let Some(info) = info else {
            return;
        };
        if !info.is_abstract() {
            return;
        }
        if Self::is_class_decl(compiler, n) {
            return;
        }

        // @abstract annotation on a function written as a class field.
        if n.is_get_prop(compiler) {
            if !(n.get_first_child(compiler).unwrap().is_this(compiler)
                && info.get_return_type().is_some())
            {
                Self::report(
                    compiler,
                    n,
                    &MISPLACED_ANNOTATION,
                    &[
                        "@abstract",
                        "abstract undeclared methods can only be written as class fields",
                    ],
                );
            } else if !(n.get_parent(compiler).unwrap().is_expr_result(compiler)
                && n.get_grandparent(compiler).unwrap().is_block(compiler)
                && n.get_parent(compiler)
                    .unwrap()
                    .get_grandparent(compiler)
                    .unwrap()
                    .is_function(compiler))
            {
                Self::report(
                    compiler,
                    n,
                    &MISPLACED_ANNOTATION,
                    &[
                        "@abstract",
                        "abstract methods without an initializer must be declared in a constructor function.",
                    ],
                );
                return;
            }
            return;
        }

        // @abstract annotation on a non-function
        let function_node = Self::get_function_decl(compiler, n);
        let Some(function_node) = function_node else {
            Self::report(
                compiler,
                n,
                &MISPLACED_ANNOTATION,
                &[
                    "@abstract",
                    "only functions or non-static methods can be abstract",
                ],
            );
            return;
        };

        if !info.is_constructor()
            && NodeUtil::get_function_body(compiler, function_node).has_children(compiler)
        {
            // @abstract annotation on a function with a non-empty body
            Self::report(
                compiler,
                n,
                &MISPLACED_ANNOTATION,
                &[
                    "@abstract",
                    "function with a non-empty body cannot be abstract",
                ],
            );
            return;
        }

        if NodeUtil::is_es6_constructor_member_function_def(compiler, n) {
            // @abstract annotation on an ES6 constructor
            Self::report(
                compiler,
                n,
                &MISPLACED_ANNOTATION,
                &["@abstract", "constructors cannot be abstract"],
            );
            return;
        }

        if !info.is_constructor()
            && !n.is_member_function_def(compiler)
            && !n.is_string_key(compiler)
            && !n.is_computed_prop(compiler)
            && !n.is_getter_def(compiler)
            && !n.is_setter_def(compiler)
            && !NodeUtil::is_prototype_method(compiler, function_node)
        {
            // @abstract annotation on a non-method (or static method) in ES5
            Self::report(
                compiler,
                n,
                &MISPLACED_ANNOTATION,
                &[
                    "@abstract",
                    "only functions or non-static methods can be abstract",
                ],
            );
            return;
        }

        if n.is_static_member(compiler) {
            // @abstract annotation on a static method in ES6
            Self::report(
                compiler,
                n,
                &MISPLACED_ANNOTATION,
                &["@abstract", "static methods cannot be abstract"],
            );
        }
    }

    // port: CheckJSDoc#hasClassLevelJsDoc
    fn has_class_level_js_doc(info: &JSDocInfo) -> bool {
        info.is_constructor_or_interface()
            || info.has_base_type()
            || info.get_implemented_interface_count() != 0
            || info.get_extended_interfaces_count() != 0
    }

    // port: CheckJSDoc#validateNoCollapse
    /// Warns when nocollapse annotations are present on nodes which are not eligible for property
    /// collapsing.
    fn validate_no_collapse(compiler: &mut AbstractCompiler, n: NodeId, info: Option<&JSDocInfo>) {
        let Some(info) = info else {
            return;
        };
        if !info.is_no_collapse() {
            return;
        }
        if n.is_from_externs(compiler) {
            // @nocollapse has no effect in externs
            Self::report_misplaced(
                compiler,
                n,
                "nocollapse",
                "This JSDoc has no effect in externs.",
            );
            return;
        }
        if NodeUtil::is_prototype_property_declaration(compiler, n.get_parent(compiler).unwrap())
            || (n.get_parent(compiler).unwrap().is_class_members(compiler)
                && !n.is_static_member(compiler))
        {
            Self::report_misplaced(
                compiler,
                n,
                "nocollapse",
                "This JSDoc has no effect on prototype properties and non-static fields.",
            );
        }
        if n.is_assign(compiler) {
            let assignee = n.get_first_child(compiler).unwrap();
            if assignee.is_qualified_name(compiler) {
                let root_of_qname = NodeUtil::get_root_of_qualified_name(compiler, assignee);
                if !root_of_qname.is_name(compiler) {
                    Self::report_misplaced(compiler, n, "nocollapse", "This JSDoc has no effect.");
                }
            }
        }
    }

    // port: CheckJSDoc#validateFunctionJsDoc
    /// Checks that JSDoc intended for a function is actually attached to a function.
    fn validate_function_js_doc(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        let Some(info) = info else {
            return;
        };

        if info.contains_function_declaration(compiler)
            && !info.has_type()
            && !Self::is_js_doc_on_function_node(compiler, n, info)
        {
            // This JSDoc should be attached to a FUNCTION node, or an assignment
            // with a function as the RHS, etc.

            Self::report_misplaced(
                compiler,
                n,
                "function",
                "This JSDoc is not attached to a function node. Are you missing parentheses?",
            );
        }
    }

    // port: CheckJSDoc#isJSDocOnFunctionNode
    /// Whether this node's JSDoc may apply to a function
    ///
    /// This has some false positive cases, to allow for patterns like goog.abstractMethod.
    fn is_js_doc_on_function_node(
        compiler: &AbstractCompiler,
        n: NodeId,
        info: &JSDocInfo,
    ) -> bool {
        match n.get_token(compiler) {
            Token::FUNCTION
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::STRING_KEY
            | Token::COMPUTED_PROP
            | Token::EXPORT
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_FIELD_DEF => true,
            Token::GETELEM | Token::GETPROP => {
                if n.get_first_child(compiler)
                    .unwrap()
                    .is_qualified_name(compiler)
                {
                    // assume qualified names may be function declarations
                    return true;
                }
                false
            }
            Token::VAR | Token::LET | Token::CONST | Token::ASSIGN => {
                let lhs = n.get_first_child(compiler).unwrap();
                let rhs = NodeUtil::get_r_value_of_l_value(compiler, lhs);
                if let Some(rhs) = rhs
                    && Self::is_class(compiler, rhs)
                    && !info.is_constructor()
                {
                    return false;
                }

                // TODO(b/124081098): Check that the RHS of the assignment is a
                // function. Note that it can be a FUNCTION node, but it can also be
                // a call to goog.abstractMethod, goog.functions.constant, etc.
                true
            }
            _ => false,
        }
    }

    // port: CheckJSDoc#validateMsgJsDoc
    /// Checks that annotations for messages (`@desc`, `@meaning` and `@alternateMessageId`) are
    /// in the proper place, namely on names starting with MSG_ which indicates they should be
    /// extracted for translation. A later pass checks that the right side is a call to
    /// goog.getMsg.
    fn validate_msg_js_doc(compiler: &mut AbstractCompiler, n: NodeId, info: Option<&JSDocInfo>) {
        let Some(info) = info else {
            return;
        };

        let has_non_desc_msg_tag =
            info.get_meaning().is_some() || info.get_alternate_message_id().is_some();

        if has_non_desc_msg_tag
            // Don't error on TS gencode using @desc on a non-message. There's a lot of code that
            // uses @desc as a general purpose "@desc" tag
            || (info.get_description().is_some() && !Self::is_from_ts(compiler, n))
        {
            let mut desc_okay = false;
            match n.get_token(compiler) {
                Token::ASSIGN | Token::VAR | Token::LET | Token::CONST => {
                    desc_okay =
                        Self::is_valid_msg_name(compiler, n.get_first_child(compiler).unwrap());
                }
                Token::STRING_KEY => desc_okay = Self::is_valid_msg_name(compiler, n),
                Token::GETPROP => {
                    if n.is_from_externs(compiler) && n.is_qualified_name(compiler) {
                        desc_okay = Self::is_valid_msg_name(compiler, n);
                    }
                }
                _ => {}
            }
            if !desc_okay {
                Self::report(compiler, n, &MISPLACED_MSG_ANNOTATION, &[]);
            }
        }
    }

    // port: CheckJSDoc#isValidMsgName
    /// Returns whether of not the given name is valid target for the result of goog.getMsg
    fn is_valid_msg_name(ast: &Ast, name_node: NodeId) -> bool {
        if name_node.is_name(ast) || name_node.is_string_key(ast) {
            name_node.get_string(ast).starts_with("MSG_")
        } else if name_node.is_qualified_name(ast) {
            name_node.get_string(ast).starts_with("MSG_")
        } else {
            false
        }
    }

    // port: CheckJSDoc#validateTypeAnnotations
    /// Check that JSDoc with a `@type` annotation is in a valid place.
    fn validate_type_annotations(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        if let Some(info) = info
            && info.has_type()
        {
            let mut valid = false;
            match n.get_token(compiler) {
                // Function declarations are valid
                Token::FUNCTION => valid = NodeUtil::is_function_declaration(compiler, n),
                // Object literal properties, catch declarations and variable
                // initializers are valid.
                Token::NAME => valid = Self::is_type_annotation_allowed_for_name(compiler, n),
                // allow JSDoc like
                //   function f(/** !Object */ {x}) {}
                //   function f(/** !Array */ [x]) {}
                Token::ARRAY_PATTERN | Token::OBJECT_PATTERN => {
                    valid = n.get_parent(compiler).unwrap().is_param_list(compiler)
                }
                // Casts, exports, and Object literal properties are valid.
                Token::CAST
                | Token::EXPORT
                | Token::STRING_KEY
                | Token::GETTER_DEF
                | Token::SETTER_DEF
                | Token::MEMBER_FIELD_DEF
                | Token::COMPUTED_FIELD_DEF => valid = true,
                // Declarations are valid iff they only contain simple names
                //   /** @type {number} */ var x = 3; // ok
                //   /** @type {number} */ var {x} = obj; // forbidden
                Token::VAR | Token::LET | Token::CONST => {
                    valid = !NodeUtil::is_destructuring_declaration(compiler, n)
                }
                // Property assignments are valid, if at the root of an expression.
                Token::ASSIGN => {
                    let lvalue = n.get_first_child(compiler).unwrap();
                    valid = n.get_parent(compiler).unwrap().is_expr_result(compiler)
                        && (lvalue.is_get_prop(compiler)
                            || lvalue.is_get_elem(compiler)
                            || lvalue.matches_name(compiler, "exports"));
                }
                Token::GETPROP => {
                    valid = n.get_parent(compiler).unwrap().is_expr_result(compiler)
                        && n.is_qualified_name(compiler)
                }
                Token::CALL => valid = info.is_define(),
                _ => {}
            }

            if !valid {
                Self::report_misplaced(
                    compiler,
                    n,
                    "type",
                    "Type annotations are not allowed here. Are you missing parentheses?",
                );
            }
        }
    }

    // port: CheckJSDoc#isTypeAnnotationAllowedForName
    /// Is it valid to have a type annotation on the given NAME node?
    fn is_type_annotation_allowed_for_name(ast: &Ast, n: NodeId) -> bool {
        check_state!(n.is_name(ast), "%s", n.to_string(ast));
        // Only allow type annotations on nodes used as an lvalue.
        if !NodeUtil::is_l_value(ast, n) {
            return false;
        }
        // Don't allow JSDoc on a name in an assignment. Simple names should only have JSDoc on them
        // when originally declared.
        let root_target = NodeUtil::get_root_target(ast, n);
        !NodeUtil::is_lhs_of_assign(ast, root_target)
    }

    // port: CheckJSDoc#reportMisplaced
    fn report_misplaced(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        annotation_name: &str,
        note: &str,
    ) {
        let error = JSError::make(compiler, n, &MISPLACED_ANNOTATION, &[annotation_name, note]);
        compiler.report(error);
    }

    // port: CheckJSDoc#report
    fn report(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: &'static DiagnosticType,
        arguments: &[&str],
    ) {
        let error = JSError::make(compiler, n, type_, arguments);
        compiler.report(error);
    }

    // port: CheckJSDoc#validateArrowFunction
    /// Check that an arrow function is not annotated with {@constructor}.
    fn validate_arrow_function(compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_arrow_function(compiler) {
            let info = NodeUtil::get_best_jsdoc_info(compiler, n);
            if let Some(info) = info
                && info.is_constructor_or_interface()
            {
                Self::report(compiler, n, &ARROW_FUNCTION_AS_CONSTRUCTOR, &[]);
            }
        }
    }

    // port: CheckJSDoc#validateRestParameter
    /// Check that a rest parameter has JSDoc marked as variadic.
    fn validate_rest_parameter(compiler: &mut AbstractCompiler, rest_param: NodeId) {
        if !rest_param.is_rest(compiler)
            || !rest_param
                .get_parent(compiler)
                .unwrap()
                .is_param_list(compiler)
        {
            return;
        }
        let param_list = rest_param.get_parent(compiler).unwrap();
        let inline_info = rest_param
            .get_first_child(compiler)
            .unwrap()
            .get_jsdoc_info(compiler);
        let function_info =
            NodeUtil::get_best_jsdoc_info(compiler, param_list.get_parent(compiler).unwrap());
        let param_type_annotation: Option<Arc<JSTypeExpression>> =
            if let Some(inline_info) = &inline_info {
                inline_info.get_type()
            } else if let Some(function_info) = &function_info {
                if rest_param
                    .get_first_child(compiler)
                    .unwrap()
                    .is_name(compiler)
                {
                    let param_name = rest_param
                        .get_first_child(compiler)
                        .unwrap()
                        .get_string(compiler);
                    function_info.get_parameter_type(param_name)
                } else {
                    // destructuring rest param. use the nth JSDoc parameter if present. the name
                    // will not match
                    let index_of_rest = param_list.get_index_of_child(compiler, rest_param);
                    if function_info.get_parameter_count() >= index_of_rest {
                        // JSDocInfo#getParameterType(null) finds no parameter.
                        function_info
                            .get_parameter_name_at(index_of_rest)
                            .and_then(|name| function_info.get_parameter_type(name))
                    } else {
                        None
                    }
                }
            } else {
                None
            };

        if let Some(param_type_annotation) = param_type_annotation
            && param_type_annotation.get_root().get_token(compiler) != Token::ITER_REST
        {
            let error = JSError::make(compiler, rest_param, &BAD_REST_PARAMETER_ANNOTATION, &[]);
            compiler.report(error);
        }
    }

    // port: CheckJSDoc#validateDefaultValue
    /// Check that a parameter with a default value is marked as optional. TODO(bradfordcsmith):
    /// This is redundant. We shouldn't require it.
    fn validate_default_value(compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_default_value(compiler) && n.get_parent(compiler).unwrap().is_param_list(compiler) {
            let target_node = n.get_first_child(compiler).unwrap();
            let info = target_node.get_jsdoc_info(compiler);
            let Some(info) = info else {
                return;
            };

            let type_expr = info.get_type();
            let Some(type_expr) = type_expr else {
                return;
            };

            let type_node = type_expr.get_root();
            if type_node.get_token(compiler) != Token::EQUALS {
                Self::report(
                    compiler,
                    type_node,
                    &DEFAULT_PARAM_MUST_BE_MARKED_OPTIONAL,
                    &[],
                );
            }
        }
    }

    // port: CheckJSDoc#validateNoSideEffects
    /// Check that @modifies annotations are only present in externs.
    fn validate_no_side_effects(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        // Cannot have @modifies in regular (non externs) js. Report errors.
        let Some(info) = info else {
            return;
        };

        if n.is_from_externs(compiler) {
            return;
        }

        if info.has_side_effects_arguments_annotation() || info.modifies_this() {
            Self::report(compiler, n, &INVALID_MODIFIES_ANNOTATION, &[]);
        }
    }

    // port: CheckJSDoc#validateDefinesDeclaration
    /// Check that a let declaration is not used with {@defines}
    fn validate_defines_declaration(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        if let Some(info) = info
            && info.is_define()
            && n.is_let(compiler)
        {
            Self::report(compiler, n, &INVALID_DEFINE_ON_LET, &[]);
        }
    }

    // port: CheckJSDoc#validateImplicitCast
    /// Checks that an @implicitCast annotation is in the externs
    fn validate_implicit_cast(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        if !self.in_externs
            && let Some(info) = info
            && info.is_implicit_cast()
        {
            Self::report(compiler, n, &ILLEGAL_IMPLICIT_CAST, &[]);
        }
    }

    // port: CheckJSDoc#validateClosurePrimitive
    /// Checks that a @closurePrimitive {id} is on a function
    fn validate_closure_primitive(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        let Some(info) = info else {
            return;
        };
        if !info.has_closure_primitive_id() {
            return;
        }

        if !Self::is_js_doc_on_function_node(compiler, n, info) {
            Self::report(
                compiler,
                n,
                &MISPLACED_ANNOTATION,
                &["closurePrimitive", "must be on a function node"],
            );
        }
    }

    // port: CheckJSDoc#validateReturnJsDoc
    /// Checks that there are no annotations on return.
    fn validate_return_js_doc(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        if !n.is_return(compiler) {
            return;
        }
        let Some(info) = info else {
            return;
        };
        // @type and @typedef are handled separately
        if info.contains_declaration() && !info.has_type() && !info.has_typedef_type() {
            Self::report(compiler, n, &JSDOC_ON_RETURN, &[]);
        }
    }

    // port: CheckJSDoc#validateTsType
    /// Checks that a @tsType is on a function in a supported file
    fn validate_ts_type(compiler: &mut AbstractCompiler, n: NodeId, info: Option<&JSDocInfo>) {
        let Some(info) = info else {
            return;
        };
        if info.get_ts_types().is_empty() {
            return;
        }

        if !Self::is_js_doc_on_function_node(compiler, n, info) {
            Self::report(
                compiler,
                n,
                &MISPLACED_TS_TYPE_ANNOTATION,
                &["must be on a function node"],
            );
        }
    }

    // port: CheckJSDoc#isFromTs
    fn is_from_ts(ast: &Ast, n: NodeId) -> bool {
        n.get_static_source_file(ast)
            .unwrap()
            .is_type_script_source()
    }

    // port: CheckJSDoc#validateJsDocTypeNames
    fn validate_js_doc_type_names(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
    ) {
        let Some(info) = info else {
            return;
        };
        for type_node in info.get_type_nodes() {
            NodeUtil::visit_pre_order(compiler, type_node, &mut self.check_js_doc_types_visitor);
        }
    }

    // port: CheckJSDoc#validateIsUsedViaDotConstructor
    /// Checks that @usedViaDotConstructor is only used on constructors.
    fn validate_is_used_via_dot_constructor(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        let Some(info) = info else {
            return;
        };
        if !info.is_used_via_dot_constructor() {
            return;
        }
        if !(n.is_function(compiler) && info.is_constructor())
            && !NodeUtil::is_es6_constructor_member_function_def(compiler, n)
        {
            Self::report(
                compiler,
                n,
                &MISPLACED_ANNOTATION,
                &["usedViaDotConstructor", "must be on a constructor"],
            );
        }
    }
}

impl Default for CheckJSDoc {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for CheckJSDoc {
    // port: CheckJSDoc#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.in_externs = true;
        NodeTraversal::traverse(compiler, externs, self);
        self.in_externs = false;
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckJSDoc {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckJSDoc#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        if n.is_script(compiler) {
            let file_name = n.get_source_file_name(compiler).unwrap();
            Self::check_js_doc_in_block_comments(compiler, &file_name);
        }
        let info = n.get_jsdoc_info(compiler);
        let info = info.as_deref();
        Self::validate_type_annotations(compiler, n, info);
        Self::validate_function_js_doc(compiler, n, info);
        Self::validate_msg_js_doc(compiler, n, info);
        Self::validate_no_collapse(compiler, n, info);
        Self::validate_class_level_js_doc(compiler, n, info);
        Self::validate_arrow_function(compiler, n);
        Self::validate_rest_parameter(compiler, n);
        Self::validate_default_value(compiler, n);
        Self::validate_templates(compiler, n, info);
        Self::validate_typedefs(compiler, n, info);
        Self::validate_no_side_effects(compiler, n, info);
        Self::validate_abstract_js_doc(compiler, n, info);
        Self::validate_defines_declaration(compiler, n, info);
        Self::validate_suppress(compiler, n, info);
        self.validate_implicit_cast(compiler, n, info);
        Self::validate_closure_primitive(compiler, n, info);
        Self::validate_return_js_doc(compiler, n, info);
        Self::validate_ts_type(compiler, n, info);
        self.validate_js_doc_type_names(compiler, info);
        Self::validate_is_used_via_dot_constructor(compiler, n, info);
    }
}

/// Ban any references to compiler internal implementation details
// port: CheckJSDoc.CheckJsdocTypes
struct CheckJsdocTypes;

impl Visitor for CheckJsdocTypes {
    // port: CheckJSDoc.CheckJsdocTypes#visit
    fn visit(&mut self, ast: &mut Ast, type_ref_node: NodeId) {
        if !type_ref_node.is_string_lit(ast) {
            return;
        }
        // A type name that might be simple like "Foo" or qualified like "foo.Bar".
        let type_name = type_ref_node.get_string(ast);
        let dot = type_name.index_of_char(b'.' as u16);
        let root_of_type = if dot == -1 {
            type_name.clone()
        } else {
            type_name.substring(0, dot as usize)
        };

        // Prevent handwritten JS from referencing a module export or module content name that's
        // synthesized by ClosureRewriteModule. Prefix the JSDoc references with
        // "UnrecognizedType_" and leave it to the typechecker to report a
        // JSC_UNRECOGNIZED_TYPE_ERROR
        //  * why not report an error here? even if we did report an error, we
        //    should still add the prefix to ensure the typechecker doesn't resolve this type.
        //    Some builds and/or files suppress type errors.
        //    TODO(lharker): consider reporting an unsuppressible error instead of doing this
        //    rewriting, if we can clean up all existing violations of this error.
        //  * why do this here instead of in the ClosureRewriteModule pass? the Es6RewriteModule
        //    runs before ClosureRewriteModule and may add references to these module export
        //    names.
        //  * note: for references in code, not JSDoc, undefined variable checks will handle this.
        // TODO(b/144593112): remove this when ClosureRewriteModule always runs after typechecking
        if ClosureRewriteModule::is_module_export(&root_of_type)
            || ClosureRewriteModule::is_module_content(&root_of_type)
        {
            type_ref_node.set_string(ast, JsString::from("UnrecognizedType_").concat(&type_name));
        }
    }
}
