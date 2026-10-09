/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CodeGenerator.java.

#![allow(
    clippy::collapsible_if,
    clippy::unnecessary_unwrap,
    clippy::manual_range_contains,
    clippy::assign_op_pattern,
    clippy::if_same_then_else,
    clippy::too_many_arguments,
    clippy::match_like_matches_macro,
    unused_mut,
    unused_parens
)] // Retain Java branches and local declarations.
use crate::{
    code_consumer::CodeConsumer, compiler_options::CompilerOptions,
    js_doc_info_printer::JSDocInfoPrinter, node_util::NodeUtil,
    output_charset_encoder::OutputCharsetEncoder,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_state,
    js_identifier::JSIdentifier,
    js_string::JsString,
    jsdoc_info::{JSDocInfo, Visibility},
    node::{Ast, NodeId, ObjectProp},
    non_jsdoc_comment::NonJSDocComment,
    qualified_name::QualifiedName,
    token::Token,
    token_stream::TokenStream,
};
use closure_sourcemap::util::Util;

const NUL: u16 = b'\0' as u16;
const BACKSPACE: u16 = b'\x08' as u16;
const TAB: u16 = b'\t' as u16;
const NEWLINE: u16 = b'\n' as u16;
const VERTICAL_TAB: u16 = b'\x0b' as u16;
const FORM_FEED: u16 = b'\x0c' as u16;
const CARRIAGE_RETURN: u16 = b'\r' as u16;
const DOUBLE_QUOTE: u16 = b'"' as u16;
const DOLLAR: u16 = b'$' as u16;
const AMPERSAND: u16 = b'&' as u16;
const SINGLE_QUOTE: u16 = b'\'' as u16;
const LESS_THAN: u16 = b'<' as u16;
const EQUALS: u16 = b'=' as u16;
const GREATER_THAN: u16 = b'>' as u16;
const BACKSLASH: u16 = b'\\' as u16;
const BACKTICK: u16 = b'`' as u16;

/// CodeGenerator generates codes from a parse tree, sending it to the specified CodeConsumer.
pub struct CodeGenerator<'a> {
    cc: &'a mut dyn CodeConsumer,
    output_charset_encoder: Option<OutputCharsetEncoder>,
    prefer_single_quotes: bool,
    preserve_type_annotations: bool,
    print_non_jsdoc_comments: bool,
    /// To distinguish between gents and non-gents mode so that we can turn off checking the sanity of
    /// the source location of comments, and also provide a different mode for comment printing between
    /// those two.
    gents_mode: bool,
    trusted_strings: bool,
    quote_keyword_properties: bool,
    use_original_name: bool,
    output_feature_set: FeatureSet,
    js_doc_info_printer: JSDocInfoPrinter,
}
impl<'a> CodeGenerator<'a> {
    pub const LT_ESCAPED: &'static str = "\\x3c";
    pub const GT_ESCAPED: &'static str = "\\x3e";
    const JSCOMP_SCOPE: &'static str = "$jscomp.scope";
    // port: CodeGenerator#CodeGenerator(CodeConsumer)
    fn new_for_cost_estimation(consumer: &'a mut dyn CodeConsumer) -> Self {
        Self {
            cc: consumer,
            output_charset_encoder: None,
            prefer_single_quotes: false,
            trusted_strings: true,
            preserve_type_annotations: false,
            print_non_jsdoc_comments: false,
            gents_mode: false,
            quote_keyword_properties: false,
            use_original_name: false,
            output_feature_set: FeatureSet::BARE_MINIMUM,
            js_doc_info_printer: JSDocInfoPrinter::new(false),
        }
    }

    // port: CodeGenerator#CodeGenerator(CodeConsumer,CompilerOptions)
    pub fn new(consumer: &'a mut dyn CodeConsumer, options: &CompilerOptions) -> Self {
        let use_original_name = options.get_use_original_names_in_output();
        Self {
            cc: consumer,
            output_charset_encoder: Some(OutputCharsetEncoder::new(options.get_output_charset())),
            prefer_single_quotes: options.should_prefer_single_quotes(),
            trusted_strings: options.assume_trusted_strings(),
            preserve_type_annotations: options.should_preserve_type_annotations(),
            print_non_jsdoc_comments: options.get_preserve_non_jsdoc_comments(),
            gents_mode: options.get_gents_mode(),
            quote_keyword_properties: options.should_quote_keyword_properties(),
            use_original_name,
            output_feature_set: options.get_output_feature_set(),
            js_doc_info_printer: JSDocInfoPrinter::new(use_original_name),
        }
    }

    // port: CodeGenerator#forCostEstimation
    pub fn for_cost_estimation(consumer: &'a mut dyn CodeConsumer) -> Self {
        Self::new_for_cost_estimation(consumer)
    }

    // port: CodeGenerator#isSimpleNumber
    pub fn is_simple_number(s: &JsString) -> bool {
        let len = s.length();
        if len == 0 {
            return false;
        }
        for index in 0..len {
            let c = s.char_at(index);
            if c < b'0' as u16 || c > b'9' as u16 {
                return false;
            }
        }
        len == 1 || s.char_at(0) != b'0' as u16
    }

    // port: CodeGenerator#getSimpleNumber
    pub fn get_simple_number(s: &JsString) -> f64 {
        if Self::is_simple_number(s) {
            if let Ok(l) = s.to_string_lossy().parse::<i64>() {
                if l <= NodeUtil::MAX_POSITIVE_INTEGER_NUMBER {
                    return l as f64;
                }
            }
        }
        // The number was too long to parse. Fall through to NaN.
        f64::NAN
    }
    /// @return Whether the name is an indirect eval.
    // port: CodeGenerator#isIndirectEval
    fn is_indirect_eval(ast: &Ast, n: NodeId) -> bool {
        n.is_name(ast)
            && n.get_string_ref(ast) == "eval"
            && !n.get_boolean_prop(ast, NodeId::DIRECT_EVAL)
    }
    /// @return Whether the Node is a DO or a declaration that is only allowed in restricted contexts.
    // port: CodeGenerator#isBlockDeclOrDo
    fn is_block_decl_or_do(ast: &Ast, n: NodeId) -> bool {
        if n.is_label(ast) {
            let labeled_statement = n.get_last_child(ast).unwrap();
            if !labeled_statement.is_block(ast) {
                Self::is_block_decl_or_do(ast, labeled_statement)
            // For labels with block children, we need to ensure that a
            // labeled FUNCTION or DO isn't generated when extraneous BLOCKs
            // are skipped.
            } else if Self::get_non_empty_child_count(ast, n, 2) == 1 {
                Self::is_block_decl_or_do(ast, Self::get_first_non_empty_child(ast, n).unwrap())
            } else {
                // Either a empty statement or an block with more than one child,
                // way it isn't a FUNCTION or DO.
                false
            }
        } else {
            match n.get_token(ast) {
                Token::LET | Token::CONST | Token::FUNCTION | Token::CLASS | Token::DO => true,
                _ => false,
            }
        }
    }

    // port: CodeGenerator#isLogicalANDorLogicalORChildOfNullishCoalesce
    fn is_logical_andor_logical_or_child_of_nullish_coalesce(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast);
        let logical_andor_logical_or = n.is_and(ast) || n.is_or(ast);
        let child_of_nullish_coalesce = parent.is_some_and(|p| p.is_nullish_coalesce(ast));
        logical_andor_logical_or && child_of_nullish_coalesce
    }

    // port: CodeGenerator#isNullishCoalesceChildOfLogicalANDorLogicalOR
    fn is_nullish_coalesce_child_of_logical_andor_logical_or(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast);
        let child_of_logical_andor_logical_or =
            parent.is_some_and(|p| p.is_and(ast) || p.is_or(ast));
        n.is_nullish_coalesce(ast) && child_of_logical_andor_logical_or
    }

    // port: CodeGenerator#identifierEscape
    pub fn identifier_escape(s: &JsString) -> JsString {
        // First check if escaping is needed at all -- in most cases it isn't.
        if NodeUtil::is_latin(s) {
            return s.clone();
        }
        // Now going through the string to escape non-Latin characters if needed.
        let mut sb = Vec::new();
        for i in 0..s.length() {
            let c = s.char_at(i);
            // Identifiers should always go to Latin1/ ASCII characters because
            // different browser's rules for valid identifier characters are
            // crazy.
            if c > 0x1f && c < 0x7f {
                sb.push(c);
            } else {
                append_hex(&mut sb, c);
            }
        }
        JsString::from_units(sb)
    }
    /// @param maxCount The maximum number of children to look for.
    /// @return The number of children of this node that are non empty up to maxCount.
    // port: CodeGenerator#getNonEmptyChildCount
    fn get_non_empty_child_count(ast: &Ast, n: NodeId, max_count: i32) -> i32 {
        let mut i = 0;
        let mut c = n.get_first_child(ast);
        while c.is_some() && i < max_count {
            let cur = c.unwrap();
            if cur.is_block(ast) {
                i += Self::get_non_empty_child_count(ast, cur, max_count - i);
            } else if !cur.is_empty(ast) {
                i += 1;
            }
            c = cur.get_next(ast);
        }
        i
    }
    /// Gets the first non-empty child of the given node.
    // port: CodeGenerator#getFirstNonEmptyChild
    fn get_first_non_empty_child(ast: &Ast, n: NodeId) -> Option<NodeId> {
        for c in n.children(ast) {
            if c.is_block(ast) {
                let result = Self::get_first_non_empty_child(ast, c);
                if result.is_some() {
                    return result;
                }
            } else if !c.is_empty(ast) {
                return Some(c);
            }
        }
        None
    }

    // port: CodeGenerator#getContextForNonEmptyExpression
    fn get_context_for_non_empty_expression(current_context: Context) -> Context {
        if current_context == Context::BEFORE_DANGLING_ELSE {
            Context::BEFORE_DANGLING_ELSE
        } else {
            Context::OTHER
        }
    }
    /// If we're in a IN_FOR_INIT_CLAUSE, we can't permit in operators in the expression. Pass on the
    /// IN_FOR_INIT_CLAUSE flag through subexpressions.
    // port: CodeGenerator#getContextForNoInOperator
    fn get_context_for_no_in_operator(context: Context) -> Context {
        if context.in_for_in_init_clause() {
            context
        } else {
            Context::OTHER
        }
    }
    /// If we're at the start of an arrow function body, we need parentheses around object literals and
    /// object patterns. We also must also pass the IN_FOR_INIT_CLAUSE flag into subexpressions.
    // port: CodeGenerator#getContextForArrowFunctionBody
    fn get_context_for_arrow_function_body(context: Context) -> Context {
        if context.in_for_in_init_clause() {
            Context::START_OF_ARROW_FN_IN_FOR_INIT
        } else {
            Context::START_OF_ARROW_FN_BODY
        }
    }

    // Rust receiver adapter for CodeGeneration.
    pub fn tag_as_type_summary(&mut self) {
        CodeGeneration::tag_as_type_summary(self)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn tag_as_strict(&mut self) {
        CodeGeneration::tag_as_strict(self)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn print_leading_comments_in_order(&mut self, ast: &Ast, node: NodeId) {
        CodeGeneration::print_leading_comments_in_order(self, ast, node)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn print_trailing_comment(&mut self, ast: &Ast, node: NodeId) {
        CodeGeneration::print_trailing_comment(self, ast, node)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add(&mut self, str: impl Into<JsString>) {
        CodeGeneration::add(self, &str.into())
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_node(&mut self, ast: &Ast, n: NodeId) {
        CodeGeneration::add_node(self, ast, n)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_node_with_context(&mut self, ast: &Ast, node: NodeId, context: Context) {
        CodeGeneration::add_node_with_context(self, ast, node, context)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_node_with_comments(
        &mut self,
        ast: &Ast,
        node: NodeId,
        context: Context,
        print_comments: bool,
    ) {
        CodeGeneration::add_node_with_comments(self, ast, node, context, print_comments)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_list(&mut self, ast: &Ast, first_in_list: Option<NodeId>) {
        CodeGeneration::add_list(self, ast, first_in_list)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_list_with_separator(
        &mut self,
        ast: &Ast,
        first_in_list: Option<NodeId>,
        separator: &str,
    ) {
        CodeGeneration::add_list_with_separator(self, ast, first_in_list, separator)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_list_full(
        &mut self,
        ast: &Ast,
        first_in_list: Option<NodeId>,
        is_array_or_function_argument: bool,
        lhs_context: Context,
        separator: &str,
    ) {
        CodeGeneration::add_list_full(
            self,
            ast,
            first_in_list,
            is_array_or_function_argument,
            lhs_context,
            separator,
        )
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_string_key(&mut self, ast: &Ast, n: NodeId) {
        CodeGeneration::add_string_key(self, ast, n)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_object_pattern(&mut self, ast: &Ast, n: NodeId) {
        CodeGeneration::add_object_pattern(self, ast, n)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_array_list(&mut self, ast: &Ast, first_in_list: Option<NodeId>) {
        CodeGeneration::add_array_list(self, ast, first_in_list)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_case_body(&mut self, ast: &Ast, case_body: NodeId) {
        CodeGeneration::add_case_body(self, ast, case_body)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn add_all_siblings(&mut self, ast: &Ast, n: Option<NodeId>) {
        CodeGeneration::add_all_siblings(self, ast, n)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn js_string(&self, s: &JsString) -> JsString {
        CodeGeneration::js_string(self, s)
    }
    // Rust receiver adapter for CodeGeneration.
    pub fn regexp_escape(&self, s: &JsString) -> JsString {
        CodeGeneration::regexp_escape(self, s)
    }
}

/// Java's overridable CodeGenerator methods, with the base fields reached through accessors.
/// Subclasses hold a CodeGenerator and return it from these two accessors.
pub trait CodeGeneration<'a> {
    fn code_generator(&self) -> &CodeGenerator<'a>;
    fn code_generator_mut(&mut self) -> &mut CodeGenerator<'a>;
    /// Insert a top-level identifying file as .i.js generated typing file.
    // port: CodeGenerator#tagAsTypeSummary
    fn tag_as_type_summary(&mut self) {
        self.add(&("/** @fileoverview @typeSummary */\n").into());
    }
    /// Insert a ECMASCRIPT 5 strict annotation.
    // port: CodeGenerator#tagAsStrict
    fn tag_as_strict(&mut self) {
        self.add(&("'use strict';").into());
        self.code_generator_mut().cc.end_line();
    }

    // port: CodeGenerator#printJSDocComment
    fn print_jsdoc_comment(&mut self, ast: &Ast, node: NodeId, js_doc_info: &JSDocInfo) {
        // In gents mode, we print NonJSDocInfo without special handling, as we already have
        // pre-filtered the comments properly.
        let jsdoc_as_string = if self.code_generator().gents_mode {
            js_doc_info.get_original_comment_string()
        } else {
            Some(
                self.code_generator()
                    .js_doc_info_printer
                    .print(ast, js_doc_info),
            )
        };
        // Don't print an empty jsdoc
        if let Some(jsdoc_as_string) = jsdoc_as_string {
            if jsdoc_as_string != "/** */ " {
                self.add(&(jsdoc_as_string));
                if !node.is_cast(ast) {
                    self.code_generator_mut().cc.end_line();
                }
            }
        }
    }

    // port: CodeGenerator#printNonJSDocComment
    fn print_non_jsdoc_comment(
        &mut self,
        ast: &Ast,
        node: NodeId,
        non_jsdoc_comment: &NonJSDocComment,
    ) {
        let non_jsdoc_comment_string = non_jsdoc_comment.get_comment_string();
        if !non_jsdoc_comment_string.is_empty() {
            self.add_non_js_doc_non_trailing(ast, node, non_jsdoc_comment);
        }
    }
    /// Print Leading JSDocComments or NonJSDocComments for the given node in order, depending on their
    /// source location.
    // port: CodeGenerator#printLeadingCommentsInOrder
    fn print_leading_comments_in_order(&mut self, ast: &Ast, node: NodeId) {
        let js_doc_info = node.get_jsdoc_info(ast);
        let non_jsdoc_comment = node.get_non_jsdoc_comment(ast);
        let print_jsdoc = self.code_generator().preserve_type_annotations && js_doc_info.is_some();
        let print_non_jsdoc =
            self.code_generator().print_non_jsdoc_comments && non_jsdoc_comment.is_some();
        if print_jsdoc && print_non_jsdoc {
            let js_doc_info = js_doc_info.unwrap();
            let non_jsdoc_comment = non_jsdoc_comment.unwrap();
            if js_doc_info.get_original_comment_position()
                < non_jsdoc_comment.get_start_position().offset
            {
                self.print_jsdoc_comment(ast, node, &js_doc_info);
                self.print_non_jsdoc_comment(ast, node, &non_jsdoc_comment);
            } else {
                self.print_non_jsdoc_comment(ast, node, &non_jsdoc_comment);
                self.print_jsdoc_comment(ast, node, &js_doc_info);
            }
            return;
        }
        if print_jsdoc {
            self.print_jsdoc_comment(ast, node, &js_doc_info.unwrap());
            return;
        }
        if print_non_jsdoc {
            self.print_non_jsdoc_comment(ast, node, &non_jsdoc_comment.unwrap());
        }
    }
    /// Returns true when a node has a trailing comment.
    // port: CodeGenerator#hasTrailingCommentOnSameLine
    fn has_trailing_comment_on_same_line(&self, ast: &Ast, node: NodeId) -> bool {
        if !self.code_generator().print_non_jsdoc_comments {
            return false;
        }
        !node.get_trailing_non_jsdoc_comment_string(ast).is_empty()
    }

    // port: CodeGenerator#printTrailingComment
    fn print_trailing_comment(&mut self, ast: &Ast, node: NodeId) {
        // print any trailing nonJSDoc comment attached to this node
        if !self.code_generator().print_non_jsdoc_comments {
            return;
        }
        if let Some(non_jsdoc_comment) = node.get_trailing_non_jsdoc_comment(ast) {
            let non_jsdoc_comment_string = node.get_trailing_non_jsdoc_comment_string(ast);
            if !non_jsdoc_comment_string.is_empty() {
                self.add_non_js_doc_trailing(
                    &non_jsdoc_comment,
                    self.has_trailing_comment_on_same_line(ast, node),
                );
            }
        }
    }

    // port: CodeGenerator#add(String)
    fn add(&mut self, str: &JsString) {
        self.code_generator_mut().cc.add(str);
    }

    // port: CodeGenerator#add(Node)
    fn add_node(&mut self, ast: &Ast, n: NodeId) {
        self.add_node_with_context(ast, n, Context::OTHER);
    }

    // port: CodeGenerator#add(Node,Context)
    fn add_node_with_context(&mut self, ast: &Ast, node: NodeId, context: Context) {
        self.add_node_with_comments(ast, node, context, true);
    }
    /// Generate the current node
    // port: CodeGenerator#add(Node,Context,boolean)
    fn add_node_with_comments(
        &mut self,
        ast: &Ast,
        node: NodeId,
        context: Context,
        print_comments: bool,
    ) {
        if !self.code_generator_mut().cc.continue_processing() {
            return;
        }
        // If this node is actually a shadow host node, then print the shadow content instead.
        if let Some(shadow) = node.get_closure_unaware_shadow(ast) {
            let script = shadow.get_only_child(ast);
            let sink_call = script.get_only_child(ast).get_only_child(ast);
            check_state!(sink_call.is_call(ast), "%s", sink_call.to_string(ast));
            self.add_node_with_comments(
                ast,
                sink_call.get_last_child(ast).unwrap(),
                context,
                print_comments,
            );
            return;
        }
        if print_comments {
            self.print_leading_comments_in_order(ast, node);
        }
        self.code_generator_mut().cc.track_licenses(ast, node);
        let type_ = node.get_token(ast);
        let opstr = NodeUtil::op_to_str(type_);
        let child_count = node.get_child_count(ast);
        let first = node.get_first_child(ast);
        let last = node.get_last_child(ast);
        // Handle all binary operators
        if let Some(opstr) = opstr {
            if first != last {
                check_state!(
                    child_count == 2,
                    "Bad binary operator \"%s\": expected 2 arguments but got %s",
                    opstr,
                    child_count
                );
                let p = self.precedence(ast, node);
                // For right-hand-side of operations, only pass context if it's
                // the IN_FOR_INIT_CLAUSE one.
                let rhs_context = CodeGenerator::get_context_for_no_in_operator(context);
                let needs_parens = (context == Context::START_OF_EXPR
                    || context.at_arrow_function_body())
                    && first.unwrap().is_object_pattern(ast);
                if node.is_assign(ast) && needs_parens {
                    self.add(&("(").into());
                }
                if NodeUtil::is_assignment_op(ast, node) || type_ == Token::EXPONENT {
                    // Assignment operators and '**' are the only right-associative binary operators
                    self.add_expr(ast, first.unwrap(), p + 1, context);
                    self.code_generator_mut().cc.add_op(opstr, true);
                    self.add_expr(ast, last.unwrap(), p, rhs_context);
                } else {
                    self.unroll_binary_operator(
                        ast,
                        node,
                        type_,
                        opstr,
                        context,
                        rhs_context,
                        p,
                        p + 1,
                    );
                }
                if node.is_assign(ast) && needs_parens {
                    self.add(&(")").into());
                }
                return;
            }
        }
        self.code_generator_mut().cc.start_source_mapping(ast, node);
        'token_switch: {
            match type_ {
                Token::TRY => {
                    check_state!(
                        first.unwrap().get_next(ast).unwrap().is_block(ast)
                            && !first
                                .unwrap()
                                .get_next(ast)
                                .unwrap()
                                .has_more_than_one_child(ast)
                    );
                    check_state!(child_count >= 2 && child_count <= 3);

                    self.add(&("try").into());
                    self.add_node(ast, first.unwrap());

                    // second child contains the catch block, or nothing if there
                    // isn't a catch block
                    let mut catchblock = first.unwrap().get_next(ast).unwrap().get_first_child(ast);
                    if (catchblock.is_some()) {
                        self.add_node(ast, catchblock.unwrap());
                    }

                    if (child_count == 3) {
                        self.code_generator_mut().cc.maybe_insert_space();
                        self.add(&("finally").into());
                        self.add_node(ast, last.unwrap());
                    }
                }
                Token::CATCH => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("catch").into());
                    self.code_generator_mut().cc.maybe_insert_space();

                    if (!first.unwrap().is_empty(ast)) {
                        // optional catch binding
                        self.add(&("(").into());
                        self.add_node(ast, first.unwrap());
                        self.add(&(")").into());
                    }

                    self.add_node(ast, last.unwrap());
                }
                Token::THROW => {
                    check_state!(child_count == 1, "%s", node.to_string(ast));
                    self.add(&("throw").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add_node(ast, first.unwrap());

                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        // Must have a ';' after a throw statement, otherwise safari can't
                        // parse this.
                        .cc
                        // needSemiColon=
                        .end_statement_with_semicolon(true, has_trailing_comment_on_same_line);
                }
                Token::RETURN => {
                    self.add(&("return").into());
                    if (child_count == 1) {
                        self.code_generator_mut().cc.maybe_insert_space();
                        if (self.code_generator().preserve_type_annotations
                            && first.unwrap().get_jsdoc_info(ast).is_some())
                        {
                            self.add(&("(").into());
                            self.add_node(ast, first.unwrap());
                            self.add(&(")").into());
                        } else {
                            self.add_node(ast, first.unwrap());
                        }
                    } else {
                        check_state!(child_count == 0, "%s", node.to_string(ast));
                    }
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        .end_statement(has_trailing_comment_on_same_line);
                }
                Token::VAR => {
                    self.add(&("var ").into());
                    self.add_list_full(
                        ast,
                        first,
                        false,
                        CodeGenerator::get_context_for_no_in_operator(context),
                        ",",
                    );
                    if (node.get_parent(ast).is_none() || NodeUtil::is_statement(ast, node)) {
                        let has_trailing_comment_on_same_line =
                            self.has_trailing_comment_on_same_line(ast, node);
                        self.code_generator_mut()
                            .cc
                            .end_statement(has_trailing_comment_on_same_line);
                    }
                }
                Token::CONST => {
                    self.add(&("const ").into());
                    self.add_list_full(
                        ast,
                        first,
                        false,
                        CodeGenerator::get_context_for_no_in_operator(context),
                        ",",
                    );
                    if (node.get_parent(ast).is_none() || NodeUtil::is_statement(ast, node)) {
                        let has_trailing_comment_on_same_line =
                            self.has_trailing_comment_on_same_line(ast, node);
                        self.code_generator_mut()
                            .cc
                            .end_statement(has_trailing_comment_on_same_line);
                    }
                }
                Token::LET => {
                    self.add(&("let ").into());
                    self.add_list_full(
                        ast,
                        first,
                        false,
                        CodeGenerator::get_context_for_no_in_operator(context),
                        ",",
                    );
                    if (node.get_parent(ast).is_none() || NodeUtil::is_statement(ast, node)) {
                        let has_trailing_comment_on_same_line =
                            self.has_trailing_comment_on_same_line(ast, node);
                        self.code_generator_mut()
                            .cc
                            .end_statement(has_trailing_comment_on_same_line);
                    }
                }
                Token::LABEL_NAME => {
                    check_state!(
                        !node.get_string_ref(ast).is_empty(),
                        "%s",
                        node.to_string(ast)
                    );
                    self.add_identifier(node.get_string(ast));
                }
                Token::DESTRUCTURING_LHS => {
                    self.add_node(ast, first.unwrap());
                    if (first != last) {
                        check_state!(child_count == 2, "%s", node.to_string(ast));
                        self.code_generator_mut().cc.add_op("=", true);
                        self.add_expr(
                            ast,
                            last.unwrap(),
                            NodeUtil::precedence(Token::ASSIGN),
                            CodeGenerator::get_context_for_no_in_operator(context),
                        );
                    }
                }
                Token::NAME => {
                    if (self.code_generator().use_original_name
                        && node.get_original_name(ast).is_some())
                    {
                        self.add_identifier(node.get_original_name(ast).unwrap());
                    } else {
                        self.add_identifier(node.get_string(ast));
                    }
                    self.maybe_add_optional(ast, node);
                    self.maybe_add_type_decl(ast, node);

                    if (first.is_some() && !first.unwrap().is_empty(ast)) {
                        check_state!(child_count == 1, "%s", node.to_string(ast));
                        self.code_generator_mut().cc.add_op("=", true);
                        self.add_expr(
                            ast,
                            first.unwrap(),
                            NodeUtil::precedence(Token::ASSIGN),
                            CodeGenerator::get_context_for_no_in_operator(context),
                        );
                    }
                }
                Token::ARRAYLIT => {
                    self.add(&("[").into());
                    self.add_array_list(ast, first);
                    self.add(&("]").into());
                }
                Token::ARRAY_PATTERN => {
                    self.add(&("[").into());
                    self.add_array_list(ast, first);
                    self.add(&("]").into());
                    self.maybe_add_type_decl(ast, node);
                }
                Token::PARAM_LIST => {
                    // If this is the list for a non-TypeScript arrow function with one simple name param.
                    if (node.get_parent(ast).unwrap().is_arrow_function(ast)
                        && node.has_one_child(ast)
                        && first.unwrap().is_name(ast)
                        && !self.code_generator().gents_mode)
                    {
                        self.add_node(ast, first.unwrap());
                    } else {
                        self.add(&("(").into());
                        self.add_list(ast, first);
                        self.add(&(")").into());
                    }
                }
                Token::DEFAULT_VALUE => {
                    self.add_node(ast, first.unwrap());
                    self.maybe_add_type_decl(ast, node);
                    self.code_generator_mut().cc.add_op("=", true);
                    self.add_expr(
                        ast,
                        first.unwrap().get_next(ast).unwrap(),
                        1,
                        Context::OTHER,
                    );
                }
                Token::COMMA => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    self.unroll_binary_operator(
                        ast,
                        node,
                        Token::COMMA,
                        ",",
                        context,
                        CodeGenerator::get_context_for_no_in_operator(context),
                        0,
                        0,
                    );
                }
                Token::NUMBER => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.code_generator_mut()
                        .cc
                        .add_number(node.get_double(ast), ast, Some(node));
                }
                Token::BIGINT => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.code_generator_mut()
                        .cc
                        .add_big_int(&node.get_big_int(ast));
                }
                Token::TYPEOF
                | Token::VOID
                | Token::NOT
                | Token::BITNOT
                | Token::POS
                | Token::NEG => {
                    // All of these unary operators are right-associative
                    check_state!(child_count == 1, "%s", node.to_string(ast));
                    self.code_generator_mut()
                        .cc
                        .add_op(NodeUtil::op_to_str_no_fail(type_), false);
                    self.add_expr(
                        ast,
                        first.unwrap(),
                        NodeUtil::precedence(type_),
                        Context::OTHER,
                    );
                }
                Token::HOOK => {
                    check_state!(
                        child_count == 3,
                        "%s wrong number of children: %s",
                        node.to_string(ast),
                        child_count
                    );
                    let mut p = NodeUtil::precedence(type_);
                    let rhs_context = CodeGenerator::get_context_for_no_in_operator(context);
                    self.add_expr(ast, first.unwrap(), p + 1, context);
                    self.code_generator_mut().cc.add_op("?", true);
                    self.add_expr(ast, first.unwrap().get_next(ast).unwrap(), 1, rhs_context);
                    self.code_generator_mut().cc.add_op(":", true);
                    self.add_expr(ast, last.unwrap(), 1, rhs_context);
                }
                Token::REGEXP => {
                    if (!first.unwrap().is_string_lit(ast) || !last.unwrap().is_string_lit(ast)) {
                        panic!("Expected children to be strings");
                    }

                    let mut regexp = self.regexp_escape(&first.unwrap().get_string(ast));

                    // I only use one .add because whitespace matters
                    if (child_count == 2) {
                        self.add(&(regexp.concat(&last.unwrap().get_string(ast))));
                    } else {
                        check_state!(child_count == 1, "%s", node.to_string(ast));
                        self.add(&(regexp));
                    }
                }
                Token::FUNCTION => {
                    if node.get_class(ast) != "com.google.javascript.rhino.Node" {
                        panic!("Unexpected Node subclass.");
                    }
                    check_state!(child_count == 3, "%s", node.to_string(ast));
                    if (node.is_arrow_function(ast)) {
                        self.add_arrow_function(ast, node, first.unwrap(), last.unwrap(), context);
                    } else {
                        self.add_function(ast, node, first.unwrap(), last.unwrap(), context);
                    }
                }
                Token::ITER_REST | Token::OBJECT_REST => {
                    self.add(&("...").into());
                    self.add_node(ast, first.unwrap());
                    self.maybe_add_type_decl(ast, node);
                }
                Token::ITER_SPREAD | Token::OBJECT_SPREAD => {
                    self.add(&("...").into());
                    self.add_expr(
                        ast,
                        first.unwrap(),
                        NodeUtil::precedence(type_),
                        Context::OTHER,
                    );
                }
                Token::EXPORT => {
                    self.add(&("export").into());
                    if (node.get_boolean_prop(ast, NodeId::EXPORT_DEFAULT)) {
                        self.add(&("default").into());
                    }
                    if (node.get_boolean_prop(ast, NodeId::EXPORT_ALL_FROM)) {
                        self.add(&("*").into());
                        check_state!(
                            first.is_some() && first.unwrap().is_empty(ast),
                            "%s",
                            node.to_string(ast)
                        );
                    } else {
                        self.add_node(ast, first.unwrap());
                    }
                    if (child_count == 2) {
                        self.add(&("from").into());
                        self.add_node(ast, last.unwrap());
                    }
                    self.process_end(ast, first.unwrap(), context);
                }
                Token::IMPORT => {
                    self.add(&("import").into());

                    let mut second = first.unwrap().get_next(ast).unwrap();
                    if (!first.unwrap().is_empty(ast)) {
                        self.add_node(ast, first.unwrap());
                        if (!second.is_empty(ast)) {
                            self.code_generator_mut().cc.list_separator();
                        }
                    }
                    if (!second.is_empty(ast)) {
                        self.add_node(ast, second);
                    }
                    if (!first.unwrap().is_empty(ast) || !second.is_empty(ast)) {
                        self.add(&("from").into());
                    }
                    self.add_node(ast, last.unwrap());
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        .end_statement(has_trailing_comment_on_same_line);
                }
                Token::EXPORT_SPECS | Token::IMPORT_SPECS => {
                    self.add(&("{").into());
                    for c in node.children(ast) {
                        if (Some(c) != first) {
                            self.code_generator_mut().cc.list_separator();
                        }
                        self.add_node(ast, c);
                    }
                    self.add(&("}").into());
                }
                Token::EXPORT_SPEC | Token::IMPORT_SPEC => {
                    self.add_node(ast, first.unwrap());
                    if (node.is_shorthand_property(ast)
                        && first.unwrap().get_string(ast) == last.unwrap().get_string(ast))
                    {
                        break 'token_switch;
                    }
                    self.add(&("as").into());
                    self.add_node(ast, last.unwrap());
                }
                Token::IMPORT_STAR => {
                    self.add(&("*").into());
                    self.add(&("as").into());
                    self.add(&(node.get_string(ast)));
                }
                Token::DYNAMIC_IMPORT => {
                    self.add(&("import(").into());
                    self.add_expr(ast, first.unwrap(), NodeUtil::precedence(type_), context);
                    self.add(&(")").into());
                }
                Token::IMPORT_META => self.add(&("import.meta").into()),

                // CLASS -> NAME,EXPR|EMPTY,BLOCK
                Token::CLASS => {
                    check_state!(child_count == 3, "%s", node.to_string(ast));
                    let mut class_needs_parens = (context == Context::START_OF_EXPR);
                    if (class_needs_parens) {
                        self.add(&("(").into());
                    }

                    let mut name = first.unwrap();
                    let mut super_class = first.unwrap().get_next(ast).unwrap();
                    let mut members = last.unwrap();

                    self.add(&("class").into());
                    if (!name.is_empty(ast)) {
                        self.add_node(ast, name);
                    }

                    self.maybe_add_generic_types(ast, first.unwrap());

                    if (!super_class.is_empty(ast)) {
                        self.add(&("extends").into());

                        // Parentheses are required for a comma expression or an assignment expression.
                        self.add_expr(ast, super_class, 1, Context::OTHER);
                    }

                    let mut interfaces =
                        node.get_prop(ast, NodeId::IMPLEMENTS)
                            .map(|prop| match prop {
                                ObjectProp::Node(n) => n,
                                _ => panic!("ClassCastException"),
                            });
                    if (interfaces.is_some()) {
                        self.add(&("implements").into());
                        let mut child = interfaces.unwrap().get_first_child(ast).unwrap();
                        self.add_node(ast, child);
                        while let Some(next) = child.get_next(ast) {
                            child = next;
                            self.add(&(",").into());
                            self.code_generator_mut().cc.maybe_insert_space();
                            self.add_node(ast, child);
                        }
                    }
                    self.add_node(ast, members);
                    self.code_generator_mut()
                        .cc
                        .end_class(context == Context::STATEMENT);

                    if (class_needs_parens) {
                        self.add(&(")").into());
                    }
                }
                Token::CLASS_MEMBERS | Token::INTERFACE_MEMBERS | Token::NAMESPACE_ELEMENTS => {
                    self.code_generator_mut().cc.begin_block();
                    for c in node.children(ast) {
                        self.add_node(ast, c);
                        self.process_end(ast, c, context);
                        self.code_generator_mut().cc.end_line();
                    }
                    self.code_generator_mut().cc.end_block_with_end_line(false);
                }
                Token::ENUM_MEMBERS => {
                    self.code_generator_mut().cc.begin_block();
                    for c in node.children(ast) {
                        self.add_node(ast, c);
                        if (c.get_next(ast).is_some()) {
                            self.add(&(",").into());
                        }
                        self.code_generator_mut().cc.end_line();
                    }
                    self.code_generator_mut().cc.end_block_with_end_line(false);
                }
                Token::GETTER_DEF
                | Token::SETTER_DEF
                | Token::MEMBER_FUNCTION_DEF
                | Token::MEMBER_VARIABLE_DEF => {
                    check_state!(
                        node.get_parent(ast).unwrap().is_object_lit(ast)
                            || node.get_parent(ast).unwrap().is_class_members(ast)
                            || node.get_parent(ast).unwrap().is_interface_members(ast)
                            || node.get_parent(ast).unwrap().is_record_type(ast)
                            || node.get_parent(ast).unwrap().is_index_signature(ast)
                    );

                    self.maybe_add_accessibility_modifier(ast, node);
                    if (node.is_static_member(ast)) {
                        self.add(&("static ").into());
                    }

                    if (node.is_member_function_def(ast)
                        && node.get_first_child(ast).unwrap().is_async_function(ast))
                    {
                        self.add(&("async ").into());
                    }

                    if (!node.is_member_variable_def(ast)
                        && node
                            .get_first_child(ast)
                            .unwrap()
                            .is_generator_function(ast))
                    {
                        check_state!(
                            type_ == Token::MEMBER_FUNCTION_DEF,
                            "%s",
                            node.to_string(ast)
                        );
                        self.add(&("*").into());
                    }

                    match type_ {
                        Token::GETTER_DEF => {
                            // Get methods have no parameters.
                            check_state!(
                                !first
                                    .unwrap()
                                    .get_second_child(ast)
                                    .unwrap()
                                    .has_children(ast),
                                "%s",
                                node.to_string(ast)
                            );
                            self.add(&("get ").into());
                        }
                        Token::SETTER_DEF => {
                            // Set methods have one parameter.
                            check_state!(
                                first
                                    .unwrap()
                                    .get_second_child(ast)
                                    .unwrap()
                                    .has_one_child(ast),
                                "%s",
                                node.to_string(ast)
                            );
                            self.add(&("set ").into());
                        }
                        // nothing to do.
                        Token::MEMBER_FUNCTION_DEF | Token::MEMBER_VARIABLE_DEF => {}
                        _ => {}
                    }

                    // The name is on the GET or SET node.
                    let mut name = node.get_string(ast);
                    if (node.is_member_variable_def(ast)) {
                        self.add(&(node.get_string(ast)));
                        self.maybe_add_optional(ast, node);
                        self.maybe_add_type_decl(ast, node);
                    } else {
                        check_state!(child_count == 1, "%s", node.to_string(ast));
                        check_state!(
                            first.unwrap().is_function(ast),
                            "%s",
                            first.unwrap().to_string(ast)
                        );

                        // The function referenced by the definition should always be unnamed.
                        check_state!(
                            first
                                .unwrap()
                                .get_first_child(ast)
                                .unwrap()
                                .get_string(ast)
                                .is_empty(),
                            "%s",
                            first.unwrap().to_string(ast)
                        );

                        let mut fn_ = first.unwrap();
                        let mut parameters = fn_.get_second_child(ast);
                        let mut body = fn_.get_last_child(ast);

                        // Add the property name.
                        if (!node.is_quoted_string_key(ast)
                            && (JSIdentifier::is_js_identifier(&name)
                                || node.is_private_identifier(ast))
                            // do not encode literally any non-literal characters that were
                            // Unicode escaped.
                            && NodeUtil::is_latin(&name))
                        {
                            self.add(&(name));
                            self.maybe_add_generic_types(ast, fn_.get_first_child(ast).unwrap());
                        } else {
                            // Determine if the string is a simple number.
                            let d = CodeGenerator::get_simple_number(&name);
                            if (!d.is_nan()) {
                                self.code_generator_mut().cc.add_number(d, ast, Some(node));
                            } else {
                                self.add_js_string(ast, node);
                            }
                        }
                        self.maybe_add_optional(ast, fn_);
                        self.add_node(
                            ast,
                            parameters.expect("NullPointerException: function parameters"),
                        );
                        self.maybe_add_type_decl(ast, fn_);
                        self.add_node(ast, body.expect("NullPointerException: function body"));
                    }
                }
                Token::MEMBER_FIELD_DEF | Token::COMPUTED_FIELD_DEF => {
                    check_state!(node.get_parent(ast).unwrap().is_class_members(ast));
                    if (node.get_boolean_prop(ast, NodeId::STATIC_MEMBER)) {
                        self.add(&("static ").into());
                    }
                    let mut init = None;
                    match type_ {
                        Token::MEMBER_FIELD_DEF => {
                            let property_name = node.get_string(ast);
                            self.add(&(property_name));
                            init = first;
                        }
                        Token::COMPUTED_FIELD_DEF => {
                            self.add(&("[").into());
                            // Must use addExpr() with a priority of 1, because comma expressions aren't allowed.
                            // https://www.ecma-international.org/ecma-262/9.0/index.html#prod-ComputedPropertyName
                            self.add_expr(ast, first.unwrap(), 1, Context::OTHER);
                            self.add(&("]").into());
                            init = node.get_second_child(ast);
                        }
                        _ => {}
                    }

                    if (init.is_some()) {
                        self.code_generator_mut().cc.add_op("=", true);
                        self.add_expr(ast, init.unwrap(), 1, Context::OTHER);
                    }
                    self.add(&(";").into());
                }
                Token::SCRIPT | Token::MODULE_BODY | Token::BLOCK | Token::ROOT => {
                    if node.get_class(ast) != "com.google.javascript.rhino.Node" {
                        panic!("Unexpected Node subclass.");
                    }
                    if (node.has_parent(ast)) {
                        let mut static_block = node.is_block(ast)
                            && node.get_parent(ast).unwrap().is_class_members(ast);
                        if (static_block) {
                            self.add(&("static").into());
                        }
                    }

                    // A BLOCK marked as synthetic is not a real JS block with {} around it in the JS code.
                    // It just represents a span of statements that need to be kept together.
                    let mut preserve_block = node.is_block(ast) && !node.is_synthetic_block(ast);
                    if (preserve_block) {
                        self.code_generator_mut().cc.begin_block();
                    }

                    let mut prefer_line_breaks = type_ == Token::SCRIPT
                        || (type_ == Token::BLOCK
                            && !preserve_block
                            && node.get_parent(ast).unwrap().is_script(ast));
                    for c in node.children(ast) {
                        self.add_node_with_context(ast, c, Context::STATEMENT);

                        if (c.is_function(ast) || c.is_class(ast)) {
                            self.code_generator_mut().cc.maybe_line_break();
                        }

                        // Prefer to break lines in between top-level statements
                        // because top-level statements are more homogeneous.
                        if (prefer_line_breaks) {
                            self.code_generator_mut().cc.note_preferred_line_break();
                        }
                    }
                    if (preserve_block) {
                        let break_after_block = self.code_generator_mut().cc.break_after_block_for(
                            ast,
                            node,
                            context == Context::STATEMENT,
                        );
                        self.code_generator_mut()
                            .cc
                            .end_block_with_end_line(break_after_block);
                    }
                }
                Token::FOR => {
                    check_state!(child_count == 4, "%s", node.to_string(ast));
                    self.add(&("for").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("(").into());
                    if (NodeUtil::is_name_declaration(ast, first)) {
                        self.add_node_with_context(
                            ast,
                            first.unwrap(),
                            Context::IN_FOR_INIT_CLAUSE,
                        );
                    } else {
                        self.add_expr(ast, first.unwrap(), 0, Context::IN_FOR_INIT_CLAUSE);
                    }
                    self.add(&(";").into());
                    if (!first.unwrap().get_next(ast).unwrap().is_empty(ast)) {
                        self.code_generator_mut().cc.maybe_insert_space();
                    }
                    self.add_node(ast, first.unwrap().get_next(ast).unwrap());
                    self.add(&(";").into());
                    if (!first
                        .unwrap()
                        .get_next(ast)
                        .unwrap()
                        .get_next(ast)
                        .unwrap()
                        .is_empty(ast))
                    {
                        self.code_generator_mut().cc.maybe_insert_space();
                    }
                    self.add_node(
                        ast,
                        first.unwrap().get_next(ast).unwrap().get_next(ast).unwrap(),
                    );
                    self.add(&(")").into());
                    self.add_non_empty_statement(
                        ast,
                        last.unwrap(),
                        CodeGenerator::get_context_for_non_empty_expression(context),
                        false,
                    );
                }
                Token::FOR_IN => {
                    check_state!(child_count == 3, "%s", node.to_string(ast));
                    self.add(&("for").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("(").into());
                    self.add_node(ast, first.unwrap());
                    self.add(&("in").into());
                    self.add_node(ast, first.unwrap().get_next(ast).unwrap());
                    self.add(&(")").into());
                    self.add_non_empty_statement(
                        ast,
                        last.unwrap(),
                        CodeGenerator::get_context_for_non_empty_expression(context),
                        false,
                    );
                }
                Token::FOR_OF => {
                    check_state!(child_count == 3, "%s", node.to_string(ast));
                    self.add(&("for").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("(").into());
                    self.add_node(ast, first.unwrap());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("of").into());
                    self.code_generator_mut().cc.maybe_insert_space();

                    // the iterable must be an AssignmentExpression
                    self.add_expr(
                        ast,
                        first.unwrap().get_next(ast).unwrap(),
                        NodeUtil::precedence(Token::ASSIGN),
                        Context::OTHER,
                    );
                    self.add(&(")").into());
                    self.add_non_empty_statement(
                        ast,
                        last.unwrap(),
                        CodeGenerator::get_context_for_non_empty_expression(context),
                        false,
                    );
                }
                Token::FOR_AWAIT_OF => {
                    check_state!(child_count == 3, "%s", node.to_string(ast));
                    self.add(&("for await").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("(").into());
                    self.add_node(ast, first.unwrap());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("of").into());
                    self.code_generator_mut().cc.maybe_insert_space();

                    // the iterable must be an AssignmentExpression
                    self.add_expr(
                        ast,
                        first.unwrap().get_next(ast).unwrap(),
                        NodeUtil::precedence(Token::ASSIGN),
                        Context::OTHER,
                    );
                    self.add(&(")").into());
                    self.add_non_empty_statement(
                        ast,
                        last.unwrap(),
                        CodeGenerator::get_context_for_non_empty_expression(context),
                        false,
                    );
                }
                Token::DO => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    self.add(&("do").into());
                    self.add_non_empty_statement(ast, first.unwrap(), Context::OTHER, false);
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("while").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("(").into());
                    self.add_node(ast, last.unwrap());
                    self.add(&(")").into());
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        .end_statement(has_trailing_comment_on_same_line);
                }
                Token::WHILE => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    self.add(&("while").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("(").into());
                    self.add_node(ast, first.unwrap());
                    self.add(&(")").into());
                    self.add_non_empty_statement(
                        ast,
                        last.unwrap(),
                        CodeGenerator::get_context_for_non_empty_expression(context),
                        false,
                    );
                }
                Token::EMPTY => check_state!(child_count == 0, "%s", node.to_string(ast)),
                Token::OPTCHAIN_GETPROP => {
                    self.add_expr(ast, first.unwrap(), NodeUtil::precedence(type_), context);
                    self.add(
                        &(if node.is_optional_chain_start(ast) {
                            "?."
                        } else {
                            "."
                        })
                        .into(),
                    );
                    self.add_getprop_identifier(ast, node);
                }
                Token::GETPROP => {
                    // This attempts to convert rewritten aliased code back to the original code,
                    // such as when using goog.scope(). See ScopedAliases.java for the original code.
                    // NOTE: OPTCHAIN_GETPROP case doesn't need this logic, because it only applies to
                    // qualified names.
                    if (self.code_generator().use_original_name
                        && node.get_original_name(ast).is_some())
                    {
                        // The ScopedAliases pass will convert variable assignments and function declarations
                        // to assignments to GETPROP nodes, like $jscomp.scope.SOME_VAR = 3;. This attempts to
                        // rewrite it back to the original code.
                        if (QualifiedName::of(CodeGenerator::JSCOMP_SCOPE)
                            .matches(ast, node.get_first_child(ast).unwrap())
                            && node.get_parent(ast).unwrap().is_assign(ast))
                        {
                            self.add(&("var ").into());
                        }
                        self.add_getprop_identifier(ast, node);
                        break 'token_switch;
                    }

                    // We need parentheses to distinguish
                    // `a?.b.c` from `(a?.b).c`
                    let mut break_out_of_optional_chain =
                        NodeUtil::is_opt_chain_node(ast, first.unwrap());

                    // `2.toString()` is invalid - it must be `(2).toString()`
                    let mut needs_parens =
                        first.unwrap().is_number(ast) || break_out_of_optional_chain;
                    if (needs_parens) {
                        self.add(&("(").into());
                    }
                    self.add_expr(ast, first.unwrap(), NodeUtil::precedence(type_), context);
                    if (needs_parens) {
                        self.add(&(")").into());
                    }
                    if (self.code_generator().quote_keyword_properties
                        && TokenStream::is_keyword(&node.get_string(ast)))
                    {
                        // NOTE: We don't have to worry about quoting keyword properties in the
                        // OPTCHAIN_GETPROP case above, because we only need to quote keywords for
                        // ES3-compatible output.
                        // Must be a single call to `add` otherwise the generator will add a trailing space.
                        self.add(
                            &(JsString::from("[\"")
                                .concat(&node.get_string(ast))
                                .concat(&"\"]".into())),
                        );
                    } else {
                        self.add(&(".").into());
                        self.add_getprop_identifier(ast, node);
                    }
                }
                Token::OPTCHAIN_GETELEM => {
                    check_state!(
                        child_count == 2,
                        "Bad GETELEM node: Expected 2 children but got %s. For node: %s",
                        child_count,
                        node.to_string(ast)
                    );
                    self.add_expr(ast, first.unwrap(), NodeUtil::precedence(type_), context);
                    if (node.is_optional_chain_start(ast)) {
                        self.add(&("?.").into());
                    }
                    self.add(&("[").into());
                    self.add_node(ast, first.unwrap().get_next(ast).unwrap());
                    self.add(&("]").into());
                }
                Token::GETELEM => {
                    check_state!(
                        child_count == 2,
                        "Bad GETELEM node: Expected 2 children but got %s. For node: %s",
                        child_count,
                        node.to_string(ast)
                    );
                    let mut needs_parens = NodeUtil::is_opt_chain_node(ast, first.unwrap());
                    if (needs_parens) {
                        self.add(&("(").into());
                    }
                    self.add_expr(ast, first.unwrap(), NodeUtil::precedence(type_), context);
                    if (needs_parens) {
                        self.add(&(")").into());
                    }
                    self.add(&("[").into());
                    self.add_node(ast, first.unwrap().get_next(ast).unwrap());
                    self.add(&("]").into());
                }
                Token::WITH => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    self.add(&("with(").into());
                    self.add_node(ast, first.unwrap());
                    self.add(&(")").into());
                    self.add_non_empty_statement(
                        ast,
                        last.unwrap(),
                        CodeGenerator::get_context_for_non_empty_expression(context),
                        false,
                    );
                }
                Token::INC | Token::DEC => {
                    check_state!(child_count == 1, "%s", node.to_string(ast));
                    let mut o = if type_ == Token::INC { "++" } else { "--" };
                    let mut post_prop = node.get_boolean_prop(ast, NodeId::INCRDECR_PROP);
                    if (post_prop) {
                        self.add_expr(ast, first.unwrap(), NodeUtil::precedence(type_), context);
                        self.code_generator_mut().cc.add_op(o, false);
                    } else {
                        self.code_generator_mut().cc.add_op(o, false);
                        self.add_node(ast, first.unwrap());
                    }
                }
                Token::OPTCHAIN_CALL => {
                    // We have two special cases here:
                    // 1) If the left hand side of the call is a direct reference to eval,
                    // then it must have a DIRECT_EVAL annotation. If it does not, then
                    // that means it was originally an indirect call to eval, and that
                    // indirectness must be preserved.
                    // 2) If the left hand side of the call is a property reference,
                    // then the call must not a FREE_CALL annotation. If it does, then
                    // that means it was originally an call without an explicit this and
                    // that must be preserved.
                    if (CodeGenerator::is_indirect_eval(ast, first.unwrap())
                        || (node.get_boolean_prop(ast, NodeId::FREE_CALL)
                            && NodeUtil::is_normal_or_opt_chain_get(ast, first.unwrap())))
                    {
                        self.add(&("(0,").into());
                        self.add_expr(
                            ast,
                            first.unwrap(),
                            NodeUtil::precedence(Token::COMMA),
                            Context::OTHER,
                        );
                        self.add(&(")").into());
                    } else {
                        self.add_expr(ast, first.unwrap(), NodeUtil::precedence(type_), context);
                    }
                    let mut args = first.unwrap().get_next(ast);
                    if (node.is_optional_chain_start(ast)) {
                        self.add(&("?.").into());
                    }
                    self.add(&("(").into());
                    self.add_list(ast, args);
                    self.add(&(")").into());
                }
                Token::CALL => {
                    self.add_invocation_target(ast, node, context);

                    self.add(&("(").into());
                    self.add_list(ast, first.unwrap().get_next(ast));
                    self.add(&(")").into());
                }
                Token::IF => {
                    check_state!(
                        child_count == 2 || child_count == 3,
                        "%s",
                        node.to_string(ast)
                    );
                    let mut has_else = child_count == 3;
                    let mut ambiguous_else_clause =
                        context == Context::BEFORE_DANGLING_ELSE && !has_else;
                    if (ambiguous_else_clause) {
                        self.code_generator_mut().cc.begin_block();
                    }

                    self.add(&("if").into());
                    self.code_generator_mut().cc.maybe_insert_space();
                    self.add(&("(").into());
                    self.add_node(ast, first.unwrap());
                    self.add(&(")").into());

                    if (has_else) {
                        self.add_non_empty_statement(
                            ast,
                            first.unwrap().get_next(ast).unwrap(),
                            Context::BEFORE_DANGLING_ELSE,
                            false,
                        );
                        self.code_generator_mut().cc.maybe_insert_space();
                        self.add(&("else").into());
                        self.add_non_empty_statement(
                            ast,
                            last.unwrap(),
                            CodeGenerator::get_context_for_non_empty_expression(context),
                            false,
                        );
                    } else {
                        self.add_non_empty_statement(
                            ast,
                            first.unwrap().get_next(ast).unwrap(),
                            Context::OTHER,
                            false,
                        );
                    }

                    if (ambiguous_else_clause) {
                        self.code_generator_mut().cc.end_block();
                    }
                }
                Token::NULL => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.code_generator_mut().cc.add_constant(&("null").into());
                }
                Token::THIS => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.add(&("this").into());
                }
                Token::SUPER => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.add(&("super").into());
                }
                Token::NEW_TARGET => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.add(&("new.target").into());
                }
                Token::YIELD => {
                    self.add(&("yield").into());
                    if (node.is_yield_all(ast)) {
                        first.unwrap();
                        self.add(&("*").into());
                    }
                    if (first.is_some()) {
                        self.code_generator_mut().cc.maybe_insert_space();
                        self.add_expr(
                            ast,
                            first.unwrap(),
                            NodeUtil::precedence(type_),
                            Context::OTHER,
                        );
                    }
                }
                Token::AWAIT => {
                    self.add(&("await ").into());
                    self.add_expr(
                        ast,
                        first.unwrap(),
                        NodeUtil::precedence(type_),
                        Context::OTHER,
                    );
                }
                Token::FALSE => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.code_generator_mut().cc.add_constant(&("false").into());
                }
                Token::TRUE => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.code_generator_mut().cc.add_constant(&("true").into());
                }
                Token::CONTINUE => {
                    check_state!(child_count <= 1, "%s", node.to_string(ast));
                    self.add(&("continue").into());
                    if (child_count == 1) {
                        if (!first.unwrap().is_label_name(ast)) {
                            panic!("Unexpected token type. Should be LABEL_NAME.");
                        }
                        self.add(&(" ").into());
                        self.add_node(ast, first.unwrap());
                    }
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        .end_statement(has_trailing_comment_on_same_line);
                }
                Token::DEBUGGER => {
                    check_state!(child_count == 0, "%s", node.to_string(ast));
                    self.add(&("debugger").into());
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        .end_statement(has_trailing_comment_on_same_line);
                }
                Token::BREAK => {
                    check_state!(child_count <= 1, "%s", node.to_string(ast));
                    self.add(&("break").into());
                    if (child_count == 1) {
                        if (!first.unwrap().is_label_name(ast)) {
                            panic!("Unexpected token type. Should be LABEL_NAME.");
                        }
                        self.add(&(" ").into());
                        self.add_node(ast, first.unwrap());
                    }
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        .end_statement(has_trailing_comment_on_same_line);
                }
                Token::EXPR_RESULT => {
                    check_state!(child_count == 1, "%s", node.to_string(ast));
                    self.add_node_with_context(ast, first.unwrap(), Context::START_OF_EXPR);
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        .end_statement(has_trailing_comment_on_same_line);
                }
                Token::NEW => {
                    self.add(&("new ").into());
                    let mut precedence = NodeUtil::precedence(type_);

                    // `new void 0` is a syntax error add parenthese in this case.  This is only particularly
                    // interesting for code in dead branches.
                    let mut precedence_of_first =
                        NodeUtil::precedence(first.unwrap().get_token(ast));
                    if (precedence_of_first == precedence) {
                        precedence = precedence + 1;
                    }

                    // If the first child contains a CALL, then claim higher precedence
                    // to force parentheses. Otherwise, when parsed, NEW will bind to the
                    // first viable parentheses (don't traverse into functions).
                    // Also, NEW requires parentheses around an optional chain callee.
                    // If the first child is an arrow function, then parentheses is needed
                    if (NodeUtil::has(
                        ast,
                        first.unwrap(),
                        &|ast, n| n.is_call(ast),
                        &NodeUtil::MATCH_NOT_FUNCTION,
                    ) || NodeUtil::is_opt_chain_node(ast, first.unwrap()))
                    {
                        precedence = NodeUtil::precedence(first.unwrap().get_token(ast)) + 1;
                    }
                    self.add_expr(ast, first.unwrap(), precedence, Context::OTHER);

                    // '()' is optional when no arguments are present
                    let mut next = first.unwrap().get_next(ast);
                    if (next.is_some()) {
                        self.add(&("(").into());
                        self.add_list(ast, next);
                        self.add(&(")").into());
                    } else {
                        if (self
                            .code_generator_mut()
                            .cc
                            .should_preserve_extras(ast, node))
                        {
                            self.add(&("(").into());
                            self.add(&(")").into());
                        }
                    }
                }
                Token::STRING_KEY => self.add_string_key(ast, node),
                Token::STRINGLIT => {
                    check_state!(
                        child_count == 0,
                        "String node %s may not have children",
                        "%s",
                        node.to_string(ast)
                    );
                    self.add_js_string(ast, node);
                }
                Token::DELPROP => {
                    check_state!(child_count == 1, "%s", node.to_string(ast));
                    self.add(&("delete ").into());
                    self.add_node(ast, first.unwrap());
                }
                Token::OBJECTLIT => {
                    let mut needs_parens =
                        context == Context::START_OF_EXPR || context.at_arrow_function_body();
                    if (needs_parens) {
                        self.add(&("(").into());
                    }
                    self.add(&("{").into());
                    for c in node.children(ast) {
                        if (Some(c) != first) {
                            self.code_generator_mut().cc.list_separator();
                        }

                        check_state!(
                            NodeUtil::is_obj_lit_property(ast, c) || c.is_spread(ast),
                            "%s",
                            c.to_string(ast)
                        );
                        self.add_node(ast, c);
                    }
                    if (first.is_some() && node.has_trailing_comma(ast)) {
                        self.code_generator_mut().cc.optional_list_separator();
                    }
                    self.add(&("}").into());
                    if (needs_parens) {
                        self.add(&(")").into());
                    }
                }
                Token::COMPUTED_PROP => {
                    self.maybe_add_accessibility_modifier(ast, node);
                    if (node.get_boolean_prop(ast, NodeId::STATIC_MEMBER)) {
                        self.add(&("static ").into());
                    }

                    if (node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_GETTER)) {
                        self.add(&("get ").into());
                    } else if (node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_SETTER)) {
                        self.add(&("set ").into());
                    } else if (node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_METHOD)) {
                        if (last.unwrap().is_async_function(ast)) {
                            self.add(&("async").into());
                        }
                        if (last.unwrap().get_boolean_prop(ast, NodeId::GENERATOR_FN)) {
                            self.add(&("*").into());
                        }
                    }
                    self.add(&("[").into());

                    // Must use addExpr() with a priority of 1, because comma expressions aren't allowed.
                    // https://www.ecma-international.org/ecma-262/9.0/index.html#prod-ComputedPropertyName
                    self.add_expr(ast, first.unwrap(), 1, Context::OTHER);
                    self.add(&("]").into());

                    // TODO(martinprobst): There's currently no syntax for properties in object literals that
                    // have type declarations on them (a la `{foo: number: 12}`). This comes up for, e.g.,
                    // function parameters with default values. Support when figured out.
                    self.maybe_add_type_decl(ast, node);
                    if (node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_METHOD)
                        || node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_GETTER)
                        || node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_SETTER))
                    {
                        let mut function = first.unwrap().get_next(ast).unwrap();
                        let mut params = function.get_second_child(ast);
                        let mut body = function.get_last_child(ast);

                        self.add_node(
                            ast,
                            params.expect("NullPointerException: function parameters"),
                        );
                        self.add_node(ast, body.expect("NullPointerException: function body"));
                    } else {
                        // This is a field or object literal property.
                        let mut is_in_class = node.get_parent(ast).unwrap().is_class_members(ast);
                        let mut initializer = first.unwrap().get_next(ast);
                        if (initializer.is_some()) {
                            // Object literal value.
                            check_state!(
                                !is_in_class,
                                "initializers should only exist in object literals, not classes"
                            );
                            self.code_generator_mut().cc.add(&(":").into());

                            // Must use addExpr() with a priority of 1, because a comma expression here would cause
                            // a syntax error within the object literal.
                            self.add_expr(ast, initializer.unwrap(), 1, Context::OTHER);
                        } else {
                            // Computed properties must either have an initializer or be computed member-variable
                            // properties that exist for their type declaration.
                            check_state!(
                                node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_VARIABLE),
                                "%s",
                                node.to_string(ast)
                            );
                        }
                    }
                }
                Token::OBJECT_PATTERN => {
                    self.add_object_pattern(ast, node);
                    self.maybe_add_type_decl(ast, node);
                }
                Token::SWITCH => {
                    self.add(&("switch(").into());
                    self.add_node(ast, first.unwrap());
                    self.add(&(")").into());
                    self.add_node_with_context(ast, last.unwrap(), context);
                }
                Token::SWITCH_BODY => {
                    self.code_generator_mut().cc.begin_block();
                    self.add_all_siblings(ast, first);
                    self.code_generator_mut()
                        .cc
                        .end_block_with_end_line(context == Context::STATEMENT);
                }
                Token::CASE => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    self.add(&("case ").into());
                    self.add_node(ast, first.unwrap());
                    self.add_case_body(ast, last.unwrap());
                }
                Token::DEFAULT_CASE => {
                    check_state!(child_count == 1, "%s", node.to_string(ast));
                    self.add(&("default").into());
                    self.add_case_body(ast, first.unwrap());
                }
                Token::LABEL => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    if (!first.unwrap().is_label_name(ast)) {
                        panic!("Unexpected token type. Should be LABEL_NAME.");
                    }
                    self.add_node(ast, first.unwrap());
                    self.add(&(":").into());
                    if (!last.unwrap().is_block(ast)) {
                        self.code_generator_mut().cc.maybe_insert_space();
                    }
                    self.add_non_empty_statement(
                        ast,
                        last.unwrap(),
                        CodeGenerator::get_context_for_non_empty_expression(context),
                        true,
                    );
                }
                Token::CAST => {
                    if (self.code_generator().preserve_type_annotations) {
                        self.add(&("(").into());
                        self.add_node(ast, first.unwrap());
                        // drop context because of added parentheses
                        self.add(&(")").into());
                    } else {
                        self.add_node_with_context(ast, first.unwrap(), context);
                    }
                }
                // preserve context
                Token::TAGGED_TEMPLATELIT => {
                    self.add_invocation_target(ast, node, context);
                    self.add_node(ast, first.unwrap().get_next(ast).unwrap());
                }
                Token::TEMPLATELIT => {
                    self.code_generator_mut().cc.begin_template_lit();
                    for c in node.children(ast) {
                        if (c.is_template_lit_string(ast)) {
                            self.add(
                                &(self.escape_unrecognized_characters(&c.get_raw_string(ast))),
                            );
                        } else {
                            self.code_generator_mut().cc.begin_template_lit_sub();
                            self.add_node_with_context(
                                ast,
                                c.get_first_child(ast).unwrap(),
                                Context::START_OF_EXPR,
                            );
                            self.code_generator_mut().cc.end_template_lit_sub();
                        }
                    }
                    self.code_generator_mut().cc.end_template_lit();
                }

                // Type Declaration ASTs.
                Token::STRING_TYPE => self.add(&("string").into()),
                Token::BOOLEAN_TYPE => self.add(&("boolean").into()),
                Token::NUMBER_TYPE => self.add(&("number").into()),
                Token::ANY_TYPE => self.add(&("any").into()),
                Token::VOID_TYPE => self.add(&("void").into()),
                // Children are a chain of getprop nodes.
                Token::NAMED_TYPE => self.add_node(ast, first.unwrap()),
                Token::ARRAY_TYPE => {
                    self.add_expr(
                        ast,
                        first.unwrap(),
                        NodeUtil::precedence(Token::ARRAY_TYPE),
                        context,
                    );
                    self.add(&("[]").into());
                }
                Token::FUNCTION_TYPE => {
                    let mut return_type = first.unwrap();
                    self.add(&("(").into());
                    self.add_list(ast, first.unwrap().get_next(ast));
                    self.add(&(")").into());
                    self.code_generator_mut().cc.add_op("=>", true);
                    self.add_node(ast, return_type);
                }
                Token::UNION_TYPE => self.add_list_with_separator(ast, first, "|"),
                Token::RECORD_TYPE => {
                    self.add(&("{").into());
                    self.add_list_full(ast, first, false, Context::OTHER, ",");
                    self.add(&("}").into());
                }
                Token::PARAMETERIZED_TYPE => {
                    // First child is the type that's parameterized, later children are the arguments.
                    self.add_node(ast, first.unwrap());
                    self.add(&("<").into());
                    self.add_list(ast, first.unwrap().get_next(ast));
                    self.add(&(">").into());
                }
                // CLASS -> NAME,EXPR|EMPTY,BLOCK
                Token::GENERIC_TYPE_LIST => {
                    self.add(&("<").into());
                    self.add_list_full(ast, first, false, Context::STATEMENT, ",");
                    self.add(&(">").into());
                }
                Token::GENERIC_TYPE => {
                    self.add_identifier(node.get_string(ast));
                    if (node.has_children(ast)) {
                        self.add(&("extends").into());
                        self.code_generator_mut().cc.maybe_insert_space();
                        self.add_node(ast, node.get_first_child(ast).unwrap());
                    }
                }
                Token::INTERFACE => {
                    check_state!(child_count == 3, "%s", node.to_string(ast));
                    let mut name = first.unwrap();
                    let mut super_types = first.unwrap().get_next(ast).unwrap();
                    let mut members = last.unwrap();

                    self.add(&("interface").into());
                    self.add_node(ast, name);
                    self.maybe_add_generic_types(ast, name);
                    if (!super_types.is_empty(ast)) {
                        self.add(&("extends").into());
                        let mut super_type = super_types.get_first_child(ast).unwrap();
                        self.add_node(ast, super_type);
                        while let Some(next) = super_type.get_next(ast) {
                            super_type = next;
                            self.add(&(",").into());
                            self.code_generator_mut().cc.maybe_insert_space();
                            self.add_node(ast, super_type);
                        }
                    }
                    self.add_node(ast, members);
                }
                Token::ENUM => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    let mut name = first.unwrap();
                    let mut members = last.unwrap();
                    self.add(&("enum").into());
                    self.add_node(ast, name);
                    self.add_node(ast, members);
                }
                Token::NAMESPACE => {
                    check_state!(child_count == 2, "%s", node.to_string(ast));
                    let mut name = first.unwrap();
                    let mut elements = last.unwrap();
                    self.add(&("namespace").into());
                    self.add_node(ast, name);
                    self.add_node(ast, elements);
                }
                Token::TYPE_ALIAS => {
                    self.add(&("type").into());
                    self.add(&(node.get_string(ast)));
                    self.code_generator_mut().cc.add_op("=", true);
                    self.add_node(ast, last.unwrap());
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        // needSemiColon=
                        .end_statement_with_semicolon(true, has_trailing_comment_on_same_line);
                }
                Token::DECLARE => {
                    self.add(&("declare").into());
                    self.add_node(ast, first.unwrap());
                    self.process_end(ast, node, context);
                }
                Token::INDEX_SIGNATURE => {
                    self.add(&("[").into());
                    self.add_node(ast, first.unwrap());
                    self.add(&("]").into());
                    self.maybe_add_type_decl(ast, node);
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        // needSemiColon=
                        .end_statement_with_semicolon(true, has_trailing_comment_on_same_line);
                }
                Token::CALL_SIGNATURE => {
                    if (node.get_boolean_prop(ast, NodeId::CONSTRUCT_SIGNATURE)) {
                        self.add(&("new ").into());
                    }
                    self.maybe_add_generic_types(ast, node);
                    self.add_node(ast, first.unwrap());
                    self.maybe_add_type_decl(ast, node);
                    let has_trailing_comment_on_same_line =
                        self.has_trailing_comment_on_same_line(ast, node);
                    self.code_generator_mut()
                        .cc
                        // needSemiColon=
                        .end_statement_with_semicolon(true, has_trailing_comment_on_same_line);
                }
                _ => panic!("Unknown token {}\n{}", type_, node.to_string_tree(ast)),
            }
        }

        self.print_trailing_comment(ast, node);
        self.code_generator_mut().cc.end_source_mapping(ast, node);
    }

    // port: CodeGenerator#addIdentifier
    fn add_identifier(&mut self, identifier: JsString) {
        self.code_generator_mut()
            .cc
            .add_identifier(&CodeGenerator::identifier_escape(&identifier));
    }

    // port: CodeGenerator#addGetpropIdentifier
    fn add_getprop_identifier(&mut self, ast: &Ast, getprop: NodeId) {
        self.code_generator_mut()
            .cc
            .start_source_mapping(ast, getprop);
        self.add_identifier(getprop.get_string(ast));
        self.code_generator_mut()
            .cc
            .end_source_mapping(ast, getprop);
    }

    // port: CodeGenerator#precedence
    fn precedence(&self, ast: &Ast, n: NodeId) -> i32 {
        if n.is_cast(ast) {
            return self.precedence(ast, n.get_first_child(ast).unwrap());
        }
        NodeUtil::precedence(n.get_token(ast))
    }
    /// We have two special cases here:
    ///
    /// <ul>
    /// <li>If the left hand side of the call is a direct reference to eval, then it must have a
    /// DIRECT_EVAL annotation. If it does not, then that means it was originally an indirect
    /// call to eval, and that indirectness must be preserved.
    /// <li>If the left hand side of the call is a property reference, then the call must not a
    /// FREE_CALL annotation. If it does, then that means it was originally an call without an
    /// explicit this and that must be preserved.
    // port: CodeGenerator#addInvocationTarget
    fn add_invocation_target(&mut self, ast: &Ast, node: NodeId, context: Context) {
        let first = node.get_first_child(ast).unwrap();
        let needs_parens = NodeUtil::is_opt_chain_node(ast, first);
        if CodeGenerator::is_indirect_eval(ast, first)
            || (node.get_boolean_prop(ast, NodeId::FREE_CALL)
                && NodeUtil::is_normal_or_opt_chain_get(ast, first))
        {
            self.add(&("(0,").into());
            self.add_expr(
                ast,
                first,
                NodeUtil::precedence(Token::COMMA),
                Context::OTHER,
            );
            self.add(&(")").into());
        } else {
            if needs_parens {
                self.add(&("(").into());
            }
            self.add_expr(
                ast,
                first,
                NodeUtil::precedence(node.get_token(ast)),
                context,
            );
            if needs_parens {
                self.add(&(")").into());
            }
        }
    }

    // port: CodeGenerator#arrowFunctionNeedsParens
    fn arrow_function_needs_parens(&self, ast: &Ast, n: NodeId) -> bool {
        let mut parent = n.get_parent(ast);
        let mut expression_or_enclosing_cast = n;
        while parent.is_some_and(|p| p.is_cast(ast)) {
            if self.code_generator().preserve_type_annotations {
                // If printing type annotations, any expression in a CAST automatically is wrapped in
                // parentheses when printing the CAST. Returning true here would add a second, unnecessary
                // pair of parentheses.
                return false;
            }
            // If not printing type annotations, then pretend the CAST node is not there and check the
            // parent of the CAST.
            expression_or_enclosing_cast = parent.unwrap();
            parent = parent.unwrap().get_parent(ast);
        }
        // Once you cut through the layers of non-terminals used to define operator precedence,
        // you can see the following are true.
        // (Read => as "may expand to" and "!=>" as "may not expand to")
        // 1. You can substitute an ArrowFunction into rules where an Expression or
        // AssignmentExpression is required, because
        // Expression => AssignmentExpression => ArrowFunction
        // 2. However, most operators act on LeftHandSideExpression, CallExpression, or
        // MemberExpression. None of these expand to ArrowFunction.
        // 3. CallExpression cannot expand to an ArrowFunction at all, because all of its expansions
        // produce multiple symbols and none can be logically equivalent to ArrowFunction.
        // 4. LeftHandSideExpression and MemberExpression may be replaced with an ArrowFunction in
        // parentheses, because:
        // LeftHandSideExpression => MemberExpression => PrimaryExpression
        // PrimaryExpression => '(' Expression ')' => '(' ArrowFunction ')'
        if parent.is_none() {
            return false;
        }
        let parent = parent.unwrap();
        if NodeUtil::is_binary_operator(ast, parent)
            || NodeUtil::is_unary_operator(ast, parent)
            || NodeUtil::is_update_operator(ast, parent)
            || parent.is_tagged_template_lit(ast)
            || parent.is_get_prop(ast)
            || parent.is_opt_chain_get_prop(ast)
            || parent.is_await(ast)
            || parent.is_yield(ast)
        {
            // LeftHandSideExpression OP LeftHandSideExpression
            // OP LeftHandSideExpression | LeftHandSideExpression OP
            // MemberExpression TemplateLiteral
            // MemberExpression '.' IdentifierName
            true
        } else if parent.is_get_elem(ast)
            || parent.is_call(ast)
            || parent.is_hook(ast)
            || parent.is_opt_chain_get_elem(ast)
            || parent.is_opt_chain_call(ast)
            || parent.is_new(ast)
        {
            // MemberExpression '[' Expression ']'
            // MemberFunction '(' AssignmentExpressionList ')'
            // LeftHandSideExpression ? AssignmentExpression : AssignmentExpression
            expression_or_enclosing_cast.is_first_child_of(ast, Some(parent))
        } else {
            // All other cases are either illegal (e.g. because you cannot assign a value to an
            // ArrowFunction) or do not require parens.
            false
        }
    }

    // port: CodeGenerator#addArrowFunction
    fn add_arrow_function(
        &mut self,
        ast: &Ast,
        n: NodeId,
        first: NodeId,
        last: NodeId,
        context: Context,
    ) {
        check_state!(
            first.get_string_ref(ast).is_empty(),
            "%s",
            first.to_string(ast)
        );
        let func_needs_parens = self.arrow_function_needs_parens(ast, n);
        if func_needs_parens {
            self.add(&("(").into());
        }
        self.maybe_add_generic_types(ast, first);
        if n.is_async_function(ast) {
            self.add(&("async").into());
        }
        self.add_node(ast, first.get_next(ast).unwrap());
        // param list
        self.maybe_add_type_decl(ast, n);
        self.code_generator_mut().cc.add_op("=>", true);
        if last.is_block(ast) {
            self.add_node(ast, last);
        } else {
            // This is a hack. Arrow functions have no token type, but
            // blockless arrow function bodies have lower precedence than anything other than commas.
            self.add_expr(
                ast,
                last,
                NodeUtil::precedence(Token::COMMA) + 1,
                CodeGenerator::get_context_for_arrow_function_body(context),
            );
        }
        self.code_generator_mut()
            .cc
            .end_function(context == Context::STATEMENT);
        if func_needs_parens {
            self.add(&(")").into());
        }
    }

    // port: CodeGenerator#addFunction
    fn add_function(
        &mut self,
        ast: &Ast,
        n: NodeId,
        first: NodeId,
        last: NodeId,
        context: Context,
    ) {
        let func_needs_parens = context == Context::START_OF_EXPR;
        if func_needs_parens {
            self.add(&("(").into());
        }
        self.add(
            &(if n.is_async_function(ast) {
                "async function"
            } else {
                "function"
            })
            .into(),
        );
        if n.is_generator_function(ast) {
            self.add(&("*").into());
            if !first.get_string_ref(ast).is_empty() {
                self.code_generator_mut().cc.maybe_insert_space();
            }
        }
        self.add_node(ast, first);
        self.maybe_add_generic_types(ast, first);
        self.add_node(ast, first.get_next(ast).unwrap());
        // param list
        self.maybe_add_type_decl(ast, n);
        self.add_node(ast, last);
        self.code_generator_mut()
            .cc
            .end_function(context == Context::STATEMENT);
        if func_needs_parens {
            self.add(&(")").into());
        }
    }

    // port: CodeGenerator#maybeAddAccessibilityModifier
    fn maybe_add_accessibility_modifier(&mut self, ast: &Ast, n: NodeId) {
        if let Some(access) = n.get_prop(ast, NodeId::ACCESS_MODIFIER) {
            let access = match access {
                ObjectProp::Opaque(value) => value
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Visibility>()
                    .copied()
                    .expect("ClassCastException"),
                _ => panic!("ClassCastException"),
            };
            self.add(&(format!("{} ", format!("{access:?}").to_ascii_lowercase())).into());
        }
    }

    // port: CodeGenerator#maybeAddTypeDecl
    fn maybe_add_type_decl(&mut self, ast: &Ast, n: NodeId) {
        if let Some(ty) = n.get_declared_type_expression(ast) {
            self.add(&(":").into());
            self.code_generator_mut().cc.maybe_insert_space();
            self.add_node(ast, ty);
        }
    }

    // port: CodeGenerator#maybeAddGenericTypes
    fn maybe_add_generic_types(&mut self, ast: &Ast, n: NodeId) {
        if let Some(generics) = n.get_prop(ast, NodeId::GENERIC_TYPE_LIST) {
            let generics = match generics {
                ObjectProp::Node(n) => n,
                _ => panic!("ClassCastException"),
            };
            self.add_node(ast, generics);
        }
    }

    // port: CodeGenerator#maybeAddOptional
    fn maybe_add_optional(&mut self, ast: &Ast, n: NodeId) {
        if n.get_boolean_prop(ast, NodeId::OPT_ES6_TYPED) {
            self.add(&("?").into());
        }
    }
    /// We could use addList recursively here, but sometimes we produce very deeply nested operators
    /// and run out of stack space, so we just unroll the recursion when possible.
    ///
    /// <p>We assume nodes are left-recursive.
    // port: CodeGenerator#unrollBinaryOperator
    fn unroll_binary_operator(
        &mut self,
        ast: &Ast,
        n: NodeId,
        op: Token,
        op_str: &str,
        context: Context,
        rhs_context: Context,
        left_precedence: i32,
        right_precedence: i32,
    ) {
        let mut first_non_operator = n.get_first_child(ast).unwrap();
        while first_non_operator.get_token(ast) == op {
            first_non_operator = first_non_operator.get_first_child(ast).unwrap();
        }
        self.add_expr(ast, first_non_operator, left_precedence, context);
        let mut current = first_non_operator;
        loop {
            current = current.get_parent(ast).unwrap();
            self.code_generator_mut().cc.add_op(op_str, true);
            self.add_expr(
                ast,
                current.get_second_child(ast).unwrap(),
                right_precedence,
                rhs_context,
            );
            if current == n {
                break;
            }
        }
    }
    /// Adds a block or expression, substituting a VOID with an empty statement. This is used for "for
    /// (...);" and "if (...);" type statements.
    ///
    /// @param n The node to print.
    /// @param context The context to determine how the node should be printed.
    // port: CodeGenerator#addNonEmptyStatement
    fn add_non_empty_statement(
        &mut self,
        ast: &Ast,
        n: NodeId,
        context: Context,
        allow_non_block_child: bool,
    ) {
        let mut node_to_process = n;
        if !allow_non_block_child && !n.is_block(ast) {
            panic!("Missing BLOCK child.");
        }
        // Strip unneeded blocks, that is blocks with <2 children unless
        // the CodePrinter specifically wants to keep them.
        if n.is_block(ast) {
            let count = CodeGenerator::get_non_empty_child_count(ast, n, 2);
            if count == 0 {
                if self.code_generator_mut().cc.should_preserve_extras(ast, n) {
                    self.code_generator_mut().cc.begin_block();
                    self.print_trailing_comment(ast, n);
                    let break_after_block = self.code_generator_mut().cc.break_after_block_for(
                        ast,
                        n,
                        context == Context::STATEMENT,
                    );
                    self.code_generator_mut()
                        .cc
                        .end_block_with_end_line(break_after_block);
                } else {
                    self.print_trailing_comment(ast, n);
                    self.code_generator_mut()
                        .cc
                        // needSemiColon=
                        // hasTrailingCommentOnSameLine=
                        .end_statement_with_semicolon(true, false);
                }
                return;
            }
            if count == 1 {
                // Preserve the block only if needed or requested.
                // 'let', 'const', etc are not allowed by themselves in "if" and other
                // structures. Also, hack around a IE6/7 browser bug that needs a block around DOs.
                let first_and_only_child =
                    CodeGenerator::get_first_non_empty_child(ast, n).unwrap();
                let always_wrap_in_block =
                    self.code_generator_mut().cc.should_preserve_extras(ast, n);
                if always_wrap_in_block
                    || CodeGenerator::is_block_decl_or_do(ast, first_and_only_child)
                {
                    self.code_generator_mut().cc.begin_block();
                    self.add_node_with_context(ast, first_and_only_child, Context::STATEMENT);
                    self.print_trailing_comment(ast, n);
                    self.code_generator_mut().cc.maybe_line_break();
                    let break_after_block = self.code_generator_mut().cc.break_after_block_for(
                        ast,
                        n,
                        context == Context::STATEMENT,
                    );
                    self.code_generator_mut()
                        .cc
                        .end_block_with_end_line(break_after_block);
                    return;
                } else {
                    // Continue with the only child.
                    node_to_process = first_and_only_child;
                }
            }
        }
        if node_to_process.is_empty(ast) {
            self.print_trailing_comment(ast, n);
            self.code_generator_mut()
                .cc
                // needSemiColon=
                // hasTrailingCommentOnSameLine=
                .end_statement_with_semicolon(true, false);
        } else {
            self.add_node_with_context(ast, node_to_process, context);
            self.print_trailing_comment(ast, n);
        }
    }

    // port: CodeGenerator#addExpr
    fn add_expr(&mut self, ast: &Ast, n: NodeId, min_precedence: i32, context: Context) {
        if self.op_requires_parentheses(ast, n, min_precedence, context) {
            self.add(&("(").into());
            self.add_node_with_context(ast, n, Context::OTHER);
            self.add(&(")").into());
        } else {
            self.add_node_with_context(ast, n, context);
        }
    }

    // port: CodeGenerator#opRequiresParentheses
    fn op_requires_parentheses(
        &self,
        ast: &Ast,
        n: NodeId,
        min_precedence: i32,
        context: Context,
    ) -> bool {
        if context.in_for_in_init_clause() && n.is_in(ast) {
            // make sure this operator 'in' isn't confused with the for-loop 'in'
            true
        } else if NodeUtil::is_unary_operator(ast, n)
            && self.is_first_operand_of_exponentiation_expression(ast, n)
        {
            // Unary operators are higher precedence than '**', but
            // ExponentiationExpression cannot expand to
            // UnaryExpression ** ExponentiationExpression
            true
        } else if CodeGenerator::is_logical_andor_logical_or_child_of_nullish_coalesce(ast, n)
            || CodeGenerator::is_nullish_coalesce_child_of_logical_andor_logical_or(ast, n)
        {
            // precedence is not enough here since using && or || with ?? without parentheses
            // is a syntax error as ?? expands directly to |
            true
        } else if n.is_assign(ast) && n.get_parent(ast).unwrap().is_class(ast) {
            // Class declarations with assignments should be wrapped in parentheses.
            true
        } else {
            self.precedence(ast, n) < min_precedence
        }
    }

    // port: CodeGenerator#isFirstOperandOfExponentiationExpression
    fn is_first_operand_of_exponentiation_expression(&self, ast: &Ast, n: NodeId) -> bool {
        n.get_parent(ast)
            .is_some_and(|p| p.is_exponent(ast) && p.get_first_child(ast) == Some(n))
    }

    // port: CodeGenerator#addList(Node)
    fn add_list(&mut self, ast: &Ast, first_in_list: Option<NodeId>) {
        self.add_list_full(ast, first_in_list, true, Context::OTHER, ",");
    }

    // port: CodeGenerator#addList(Node,String)
    fn add_list_with_separator(
        &mut self,
        ast: &Ast,
        first_in_list: Option<NodeId>,
        separator: &str,
    ) {
        self.add_list_full(ast, first_in_list, true, Context::OTHER, separator);
    }

    // port: CodeGenerator#addList(Node,boolean,Context,String)
    fn add_list_full(
        &mut self,
        ast: &Ast,
        first_in_list: Option<NodeId>,
        is_array_or_function_argument: bool,
        lhs_context: Context,
        separator: &str,
    ) {
        if first_in_list.is_none() {
            return;
        }
        let mut n = first_in_list;
        while let Some(cur) = n {
            let is_first = Some(cur) == first_in_list;
            let min_precedence = if is_array_or_function_argument { 1 } else { 0 };
            if is_first {
                self.add_expr(ast, cur, min_precedence, lhs_context);
            } else {
                self.code_generator_mut().cc.add_op(separator, true);
                self.add_expr(
                    ast,
                    cur,
                    min_precedence,
                    CodeGenerator::get_context_for_no_in_operator(lhs_context),
                );
            }
            n = cur.get_next(ast);
        }
        if is_array_or_function_argument
            && first_in_list
                .unwrap()
                .get_parent(ast)
                .unwrap()
                .has_trailing_comma(ast)
        {
            self.code_generator_mut().cc.optional_list_separator();
        }
    }

    // port: CodeGenerator#addStringKey
    fn add_string_key(&mut self, ast: &Ast, n: NodeId) {
        let key = n.get_string(ast);
        // Object literal property names don't have to be quoted if they are not JavaScript keywords.
        let must_be_quoted = n.is_quoted_string_key(ast)
            || (self.code_generator().quote_keyword_properties && TokenStream::is_keyword(&key))
            || !JSIdentifier::is_js_identifier(&key)
            // do not encode literally any non-literal characters that were Unicode escaped.
            || !NodeUtil::is_latin(&key);
        if !must_be_quoted {
            // Check if the property is eligible to be printed as shorthand.
            if n.is_shorthand_property(ast) {
                let child = n.get_first_child(ast).unwrap();
                if child.matches_qualified_name(ast, &key)
                    || (child.is_default_value(ast)
                        && child
                            .get_first_child(ast)
                            .unwrap()
                            .matches_qualified_name(ast, &key))
                {
                    self.add_node(ast, child);
                    return;
                }
            }
            self.add(&(key));
        } else {
            // Determine if the string is a simple number.
            let d = CodeGenerator::get_simple_number(&key);
            if !d.is_nan() {
                self.code_generator_mut().cc.add_number(d, ast, Some(n));
            } else {
                self.add_js_string(ast, n);
            }
        }
        if n.has_children(ast) {
            // NOTE: the only time a STRING_KEY node does *not* have children is when it's
            // inside a TypeScript enum.  We should change these to their own ENUM_KEY token
            // so that the bifurcating logic can be removed from STRING_KEY.
            self.add(&(":").into());
            self.add_expr(ast, n.get_first_child(ast).unwrap(), 1, Context::OTHER);
        }
    }

    // port: CodeGenerator#addObjectPattern
    fn add_object_pattern(&mut self, ast: &Ast, n: NodeId) {
        self.add(&("{").into());
        for child in n.children(ast) {
            if Some(child) != n.get_first_child(ast) {
                self.code_generator_mut().cc.list_separator();
            }
            self.add_node(ast, child);
        }
        self.add(&("}").into());
    }
    /// Adds a comma-separated list as is specified by an ARRAYLIT node.
    ///
    /// @param firstInList The first in the node list (chained through the next property).
    // port: CodeGenerator#addArrayList
    fn add_array_list(&mut self, ast: &Ast, first_in_list: Option<NodeId>) {
        if first_in_list.is_none() {
            return;
        }
        let mut last_was_empty = false;
        let mut n = first_in_list;
        while let Some(cur) = n {
            if Some(cur) != first_in_list {
                self.code_generator_mut().cc.list_separator();
            }
            self.add_expr(ast, cur, 1, Context::OTHER);
            last_was_empty = cur.is_empty(ast);
            n = cur.get_next(ast);
        }
        if last_was_empty {
            self.code_generator_mut().cc.list_separator();
        } else if first_in_list
            .unwrap()
            .get_parent(ast)
            .unwrap()
            .has_trailing_comma(ast)
        {
            self.code_generator_mut().cc.optional_list_separator();
        }
    }

    // port: CodeGenerator#addCaseBody
    fn add_case_body(&mut self, ast: &Ast, case_body: NodeId) {
        check_state!(case_body.is_block(ast), "%s", case_body.to_string(ast));
        self.code_generator_mut().cc.begin_case_body();
        self.add_all_siblings(ast, case_body.get_first_child(ast));
        self.code_generator_mut().cc.end_case_body();
    }

    // port: CodeGenerator#addAllSiblings
    fn add_all_siblings(&mut self, ast: &Ast, n: Option<NodeId>) {
        let mut c = n;
        while let Some(cur) = c {
            self.add_node(ast, cur);
            c = cur.get_next(ast);
        }
    }

    // port: CodeGenerator#addNonJsDoc_nonTrailing
    fn add_non_js_doc_non_trailing(
        &mut self,
        ast: &Ast,
        node: NodeId,
        non_jsdoc_comment: &NonJSDocComment,
    ) {
        let content = non_jsdoc_comment.get_comment_string();
        let comment_end_position = non_jsdoc_comment.get_end_position();
        let node_line_number = node.get_lineno(ast) - 1;
        // source lines are 1-indexed
        if non_jsdoc_comment.is_ending_as_line_comment() {
            // Non trailing line comments can not be on the same line as the node.
            check_state!(
                self.code_generator().gents_mode || comment_end_position.line < node_line_number,
                "Non trailing line comments can not be on the same line as the node."
            );
            self.add(&(content.concat(&"\n".into())));
        } else if node_line_number == comment_end_position.line {
            // e.g. ``` /* comment */ let x; ```
            self.add(&(content.concat(&" ".into())));
        } else {
            // e.g.
            // ```
            // /* comment */
            // let x;
            // ```
            self.add(&(content.concat(&"\n".into())));
        }
    }

    // port: CodeGenerator#addNonJsDoctrailing
    fn add_non_js_doc_trailing(&mut self, non_jsdoc_comment: &NonJSDocComment, same_line: bool) {
        let content = non_jsdoc_comment.get_comment_string();
        if non_jsdoc_comment.is_ending_as_line_comment() {
            // Trailing line comments *must* end with a `\n`. E.g.. `let x; //comment\n`
            self.add(&(JsString::from(" ").concat(&content)));
            if same_line {
                self.code_generator_mut().cc.start_new_line();
            }
        } else if non_jsdoc_comment.is_inline() {
            // e.g. `foo(x /*comment*/);` is inline
            self.add(&(JsString::from(" ").concat(&content)));
        } else {
            // e.g. `let x; /*comment*/` is non-inline
            self.add(&(JsString::from(" ").concat(&content)));
            if same_line {
                self.code_generator_mut().cc.start_new_line();
            }
        }
    }
    /// Outputs a JS string, using the optimal (single/double) quote character
    // port: CodeGenerator#addJsString
    fn add_js_string(&mut self, ast: &Ast, n: NodeId) {
        self.add(&(self.js_string(&n.get_string(ast))));
    }

    // port: CodeGenerator#jsString
    fn js_string(&self, s: &JsString) -> JsString {
        let mut singleq = 0;
        let mut doubleq = 0;
        // could count the quotes and pick the optimal quote character
        for i in 0..s.length() {
            match s.char_at(i) {
                DOUBLE_QUOTE => doubleq += 1,
                SINGLE_QUOTE => singleq += 1,
                _ => {}
            }
        }
        let (quote, doublequote, singlequote) = if if self.code_generator().prefer_single_quotes {
            singleq <= doubleq
        } else {
            singleq < doubleq
        } {
            (b'\'' as u16, "\"", "\\'")
        } else {
            // skip non-quote characters
            // more double quotes so enclose in single quotes.
            (b'"' as u16, "\\\"", "'")
        };
        // more single quotes so escape the doubles
        let mut out = vec![quote];
        out.extend_from_slice(
            self.str_escape(s, doublequote, singlequote, "`", "\\\\", "$", false)
                .as_units(),
        );
        out.push(quote);
        JsString::from_units(out)
    }
    /// Escapes regular expression
    // port: CodeGenerator#regexpEscape
    fn regexp_escape(&self, s: &JsString) -> JsString {
        let mut out = vec![b'/' as u16];
        out.extend_from_slice(
            self.str_escape(s, "\"", "'", "`", "\\", "$", true)
                .as_units(),
        );
        out.push(b'/' as u16);
        JsString::from_units(out)
    }
    /// Helper to escape JavaScript string as well as regular expression
    // port: CodeGenerator#strEscape
    fn str_escape(
        &self,
        s: &JsString,
        doublequote_escape: &str,
        singlequote_escape: &str,
        backtick_escape: &str,
        backslash_escape: &str,
        dollar_escape: &str,
        is_regexp: bool,
    ) -> JsString {
        let mut sb = Vec::with_capacity(s.length() + 2);
        for i in 0..s.length() {
            let c = s.char_at(i);
            match c {
                NUL => sb.extend("\\x00".encode_utf16()),
                VERTICAL_TAB => sb.extend(if !is_regexp { "\\v" } else { "\\x0B" }.encode_utf16()),
                // From the SingleEscapeCharacter grammar production.
                BACKSPACE => sb.extend("\\b".encode_utf16()),
                FORM_FEED => sb.extend("\\f".encode_utf16()),
                NEWLINE => sb.extend("\\n".encode_utf16()),
                CARRIAGE_RETURN => sb.extend("\\r".encode_utf16()),
                TAB => sb.extend("\\t".encode_utf16()),
                BACKSLASH => sb.extend(backslash_escape.encode_utf16()),
                DOUBLE_QUOTE => sb.extend(doublequote_escape.encode_utf16()),
                SINGLE_QUOTE => sb.extend(singlequote_escape.encode_utf16()),
                DOLLAR => sb.extend(dollar_escape.encode_utf16()),
                BACKTICK => sb.extend(backtick_escape.encode_utf16()),
                EQUALS => self.append_escaped_eq(&mut sb, is_regexp),
                AMPERSAND => self.append_escaped_amp(&mut sb, is_regexp),
                GREATER_THAN => self.append_escaped_gt(&mut sb, s, i, is_regexp),
                LESS_THAN => self.append_escaped_lt(&mut sb, s, i, is_regexp),
                _ => {
                    // In 2019 these characters (line and paragraph separators) are valid in strings but
                    // not regular expressions.
                    // https://github.com/tc39/proposal-json-superset
                    if is_regexp
                        || !self
                            .code_generator()
                            .output_feature_set
                            .contains(Feature::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP)
                    {
                        if c == 0x2028 {
                            sb.extend("\\u2028".encode_utf16());
                            continue;
                        }
                        if c == 0x2029 {
                            sb.extend("\\u2029".encode_utf16());
                            continue;
                        }
                    }
                    if self
                        .code_generator()
                        .output_charset_encoder
                        .as_ref()
                        .is_some_and(|encoder| encoder.can_encode(c))
                        || (c > 0x1f && c < 0x7f)
                    {
                        // If we're given an outputCharsetEncoder, then check if the character can be
                        // represented in this character set. If no charsetEncoder provided - pass straight
                        // Latin characters through, and escape the rest. Doing the explicit character check is
                        // measurably faster than using the CharsetEncoder.
                        sb.push(c);
                    } else {
                        // Other characters can be misinterpreted by some JS parsers,
                        // or perhaps mangled by proxies along the way,
                        // so we play it safe and Unicode escape them.
                        append_hex(&mut sb, c);
                    }
                }
            }
        }
        JsString::from_units(sb)
    }

    // port: CodeGenerator#appendEscapedEq
    fn append_escaped_eq(&self, sb: &mut Vec<u16>, is_regexp: bool) {
        // '=' is a syntactically significant regexp character.
        if self.code_generator().trusted_strings || is_regexp {
            sb.push(b'=' as u16);
        } else {
            sb.extend("\\x3d".encode_utf16());
        }
    }

    // port: CodeGenerator#appendEscapedAmp
    fn append_escaped_amp(&self, sb: &mut Vec<u16>, is_regexp: bool) {
        if self.code_generator().trusted_strings || is_regexp {
            sb.push(b'&' as u16);
        } else {
            sb.extend("\\x26".encode_utf16());
        }
    }

    // port: CodeGenerator#appendEscapedGt
    fn append_escaped_gt(&self, sb: &mut Vec<u16>, s: &JsString, index: usize, is_regexp: bool) {
        if !self.code_generator().trusted_strings && !is_regexp {
            sb.extend(CodeGenerator::GT_ESCAPED.encode_utf16());
            return;
        }
        // Break --> into --\> or ]]> into ]]\>
        // This is just to prevent developers from shooting themselves in the
        // foot, and does not provide the level of security that you get
        // with trustedString == false.
        if index >= 2
            && ((s.char_at(index - 1) == b'-' as u16 && s.char_at(index - 2) == b'-' as u16)
                || (s.char_at(index - 1) == b']' as u16 && s.char_at(index - 2) == b']' as u16))
        {
            sb.extend(CodeGenerator::GT_ESCAPED.encode_utf16());
        } else {
            sb.push(b'>' as u16);
        }
    }

    // port: CodeGenerator#appendEscapedLt
    fn append_escaped_lt(&self, sb: &mut Vec<u16>, s: &JsString, index: usize, is_regexp: bool) {
        if !self.code_generator().trusted_strings && !is_regexp {
            sb.extend(CodeGenerator::LT_ESCAPED.encode_utf16());
            return;
        }
        // Break </script into <\/script and </style into <\/style
        // As above, this is just to prevent developers from doing this accidentally.
        let end_script = "/script";
        let end_style = "/style";
        // Break <!-- into <\!--
        let start_comment = "!--";
        if region_matches(s, true, index + 1, end_script)
            || region_matches(s, true, index + 1, end_style)
            || region_matches(s, false, index + 1, start_comment)
        {
            sb.extend(CodeGenerator::LT_ESCAPED.encode_utf16());
        } else {
            sb.push(b'<' as u16);
        }
    }
    /// Helper to escape the characters that might be misinterpreted
    ///
    /// @param s the string to modify
    /// @return the string with unrecognizable characters escaped.
    // port: CodeGenerator#escapeUnrecognizedCharacters
    fn escape_unrecognized_characters(&self, s: &JsString) -> JsString {
        // TODO(yitingwang) Move this method to a suitable place
        let mut sb = Vec::new();
        for i in 0..s.length() {
            let c = s.char_at(i);
            match c {
                // From the SingleEscapeCharacter grammar production.
                BACKSPACE | FORM_FEED | NEWLINE | CARRIAGE_RETURN | TAB | BACKSLASH
                | DOUBLE_QUOTE | SINGLE_QUOTE | DOLLAR | BACKTICK | 0x2028 | 0x2029 => sb.push(c),
                // isRegexp=
                EQUALS => self.append_escaped_eq(&mut sb, false),
                // isRegexp=
                AMPERSAND => self.append_escaped_amp(&mut sb, false),
                // isRegexp=
                GREATER_THAN => self.append_escaped_gt(&mut sb, s, i, false),
                // isRegexp=
                LESS_THAN => self.append_escaped_lt(&mut sb, s, i, false),
                _ => {
                    if self
                        .code_generator()
                        .output_charset_encoder
                        .as_ref()
                        .is_some_and(|encoder| encoder.can_encode(c))
                        || (c > 0x1f && c < 0x7f)
                    {
                        // If we're given an outputCharsetEncoder, then check if the character can be
                        // represented in this character set. If no charsetEncoder provided - pass straight
                        // Latin characters through, and escape the rest. Doing the explicit character check is
                        // measurably faster than using the CharsetEncoder.
                        sb.push(c);
                    } else {
                        // Other characters can be misinterpreted by some JS parsers,
                        // or perhaps mangled by proxies along the way,
                        // so we play it safe and Unicode escape them.
                        append_hex(&mut sb, c);
                    }
                }
            }
        }
        JsString::from_units(sb)
    }

    // port: CodeGenerator#processEnd
    fn process_end(&mut self, ast: &Ast, n: NodeId, context: Context) {
        match n.get_token(ast) {
            Token::CLASS | Token::INTERFACE | Token::ENUM | Token::NAMESPACE => self
                .code_generator_mut()
                .cc
                .end_class(context == Context::STATEMENT),
            Token::FUNCTION => {
                if n.get_last_child(ast).unwrap().is_empty(ast) {
                    self.code_generator_mut()
                        .cc
                        // needSemiColon=
                        // hasTrailingCommentOnSameLine=
                        .end_statement_with_semicolon(true, false);
                } else {
                    self.code_generator_mut()
                        .cc
                        .end_function(context == Context::STATEMENT);
                }
            }
            Token::DECLARE => {
                if n.get_parent(ast).unwrap().get_token(ast) != Token::NAMESPACE_ELEMENTS {
                    self.process_end(ast, n.get_first_child(ast).unwrap(), context);
                }
            }
            Token::EXPORT => {
                if n.get_parent(ast).unwrap().get_token(ast) != Token::NAMESPACE_ELEMENTS
                    && !n.get_first_child(ast).unwrap().is_declare(ast)
                {
                    self.process_end(ast, n.get_first_child(ast).unwrap(), context);
                }
            }
            Token::COMPUTED_PROP => {
                if n.has_one_child(ast) {
                    self.code_generator_mut()
                        .cc
                        // needSemiColon=
                        // hasTrailingCommentOnSameLine=
                        .end_statement_with_semicolon(true, false);
                }
            }
            Token::MEMBER_FUNCTION_DEF | Token::GETTER_DEF | Token::SETTER_DEF => {
                if n.get_first_child(ast)
                    .unwrap()
                    .get_last_child(ast)
                    .unwrap()
                    .is_empty(ast)
                {
                    self.code_generator_mut()
                        .cc
                        // needSemiColon=
                        // hasTrailingCommentOnSameLine=
                        .end_statement_with_semicolon(true, false);
                }
            }
            Token::MEMBER_VARIABLE_DEF => self
                .code_generator_mut()
                .cc
                // needSemiColon=
                // hasTrailingCommentOnSameLine=
                .end_statement_with_semicolon(true, false),
            _ => {
                if context == Context::STATEMENT {
                    // hasTrailingCommentOnSameLine=
                    self.code_generator_mut().cc.end_statement(false);
                }
            }
        }
    }
}
impl<'a> CodeGeneration<'a> for CodeGenerator<'a> {
    fn code_generator(&self) -> &CodeGenerator<'a> {
        self
    }
    fn code_generator_mut(&mut self) -> &mut CodeGenerator<'a> {
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// port: CodeGenerator.Context#Context()
// port: CodeGenerator.Context#Context(boolean,boolean)
// Java constructor flags are encoded by the enum variants and read by the two getters.
/// Information on the current context. Used for disambiguating special cases. For example, a "{"
/// could indicate the start of an object literal or a block, depending on the current context.
pub enum Context {
    STATEMENT,
    BEFORE_DANGLING_ELSE,
    // a hack to resolve the else-clause ambiguity
    START_OF_EXPR,
    // Are we inside the init clause of a for loop?  If so, the containing
    // expression can't contain an in operator.  Pass this context flag down
    // until we reach expressions which no longer have the limitation.
    // inForInitClause=
    // at start of arrow fn
    IN_FOR_INIT_CLAUSE,
    // Handle object literals at the start of a non-block arrow function body.
    // This is only important when the first token after the "=>" is "{".
    // inForInitClause=
    // at start of arrow fn
    START_OF_ARROW_FN_BODY,
    // inForInitClause=
    // atArrowFunctionBody
    START_OF_ARROW_FN_IN_FOR_INIT,
    OTHER, // nothing special to watch out for.
}
// The following two cases are independent, unlike the other enum states, so we have separate
// booleans for them.
impl Context {
    // port: CodeGenerator.Context#inForInInitClause
    pub fn in_for_in_init_clause(self) -> bool {
        matches!(
            self,
            Self::IN_FOR_INIT_CLAUSE | Self::START_OF_ARROW_FN_IN_FOR_INIT
        )
    }
    // port: CodeGenerator.Context#atArrowFunctionBody
    pub fn at_arrow_function_body(self) -> bool {
        matches!(
            self,
            Self::START_OF_ARROW_FN_BODY | Self::START_OF_ARROW_FN_IN_FOR_INIT
        )
    }
}
fn append_hex(sb: &mut Vec<u16>, c: u16) {
    let mut text = String::new();
    Util::append_hex_java_script_representation(&mut text, c);
    sb.extend(text.encode_utf16());
}
fn region_matches(s: &JsString, ignore_case: bool, offset: usize, pattern: &str) -> bool {
    let pattern: Vec<u16> = pattern.encode_utf16().collect();
    if offset + pattern.len() > s.length() {
        return false;
    }
    for (i, &expected) in pattern.iter().enumerate() {
        let c = s.char_at(offset + i);
        if c == expected {
            continue;
        }
        if !ignore_case {
            return false;
        }
        // String.regionMatches(true, ...) compares UTF-16 chars with the JDK's
        // simple case mappings. The only patterns here are "/script" and "/style".
        // JDK 21's complete non-ASCII equivalences for those ASCII letters are
        // U+017F for s, and U+0130/U+0131 for i; full Unicode expansions do not match.
        if c <= 0x7f && (c as u8).eq_ignore_ascii_case(&(expected as u8)) {
            continue;
        }
        if (c == 0x017f && expected == b's' as u16)
            || ((c == 0x0130 || c == 0x0131) && expected == b'i' as u16)
        {
            continue;
        }
        return false;
    }
    true
}
