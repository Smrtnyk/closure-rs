/*
 * Copyright 2013 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/IRFactory.java.

//! IRFactory transforms the external AST to the internal AST.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::too_many_arguments,
    clippy::needless_late_init,
    clippy::while_let_loop,
    clippy::if_same_then_else,
    clippy::needless_return
)]
use crate::{
    config::{Config, JsDocParsing, LanguageMode, RunMode},
    js_doc_info_parser::{JsDocInfoParser, JsDocSourceKind},
    js_doc_token_stream::JsDocTokenStream,
    parser::{
        feature_set::{Feature, FeatureSet},
        parser::ParseError,
        source_file::SourceFile,
        token::Token as ParserToken,
        token_type::TokenType,
        trees::*,
        util::SourcePosition,
    },
    parsing_util::ParsingUtil,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    error_reporter::ErrorReporter,
    ir::IR,
    js_string::JsString,
    jsdoc_info::{JSDocInfo, PerFileClosureUnawareMode},
    node::{Ast, NodeId},
    non_jsdoc_comment::NonJSDocComment,
    qualified_name::QualifiedName,
    static_source_file::StaticSourceFile,
    token::Token,
    token_stream::TokenStream,
};
use indexmap::IndexSet;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};
type P<T> = Result<T, ParseError>;

pub const GETTER_ERROR_MESSAGE: &str = "getters are not supported in older versions of JavaScript. If you are targeting newer versions of JavaScript, set the appropriate language_in option.";
pub const SETTER_ERROR_MESSAGE: &str = "setters are not supported in older versions of JavaScript. If you are targeting newer versions of JavaScript, set the appropriate language_in option.";
pub const INVALID_ES3_PROP_NAME: &str = "Keywords and reserved words are not allowed as unquoted property names in older versions of JavaScript. If you are targeting newer versions of JavaScript, set the appropriate language_in option.";
pub const INVALID_ES5_STRICT_OCTAL: &str =
    "Octal integer literals are not supported in strict mode.";
pub const INVALID_OCTAL_DIGIT: &str = "Invalid octal digit in octal literal.";
pub const STRING_CONTINUATION_WARNING: &str = "String continuations are not recommended. See https://google.github.io/styleguide/jsguide.html#features-strings-no-line-continuations";
pub const OCTAL_STRING_LITERAL_WARNING: &str =
    "Octal literals in strings are not supported in this language mode.";
pub const DUPLICATE_PARAMETER: &str = "Duplicate parameter name \"%s\"";
pub const DUPLICATE_LABEL: &str = "Duplicate label \"%s\"";
pub const UNLABELED_BREAK: &str = "unlabelled break must be inside loop or switch";
pub const UNEXPECTED_CONTINUE: &str = "continue must be inside loop";
pub const UNEXPECTED_LABELLED_CONTINUE: &str =
    "continue can only use labels of iteration statements";
pub const UNEXPECTED_RETURN: &str = "return must be inside function";
pub const UNEXPECTED_YIELD: &str = "yield must be inside generator function";
pub const UNEXPECTED_AWAIT: &str = "await must be inside asynchronous function";
pub const UNEXPECTED_FOR_AWAIT_OF: &str = "'for-await-of' used in a non-async function context";
pub const UNEXPECTED_NEW_DOT_TARGET: &str = "new.target must be inside a function";
pub const UNDEFINED_LABEL: &str = "undefined label \"%s\"";
const ES5_RESERVED_KEYWORDS: &[&str] = &[
    "class", "const", "enum", "export", "extends", "import", "super",
];
const ES5_STRICT_RESERVED_KEYWORDS: &[&str] = &[
    "class",
    "const",
    "enum",
    "export",
    "extends",
    "import",
    "super",
    "implements",
    "interface",
    "let",
    "package",
    "private",
    "protected",
    "public",
    "static",
    "yield",
];

pub struct IRFactory<'a> {
    #[allow(dead_code)] // Java keeps this field; Rust shares it through the template node.
    source_file: Arc<dyn StaticSourceFile>,
    source_name: String,
    file_with_content: Arc<SourceFile>,
    config: Config,
    error_reporter: &'a mut dyn ErrorReporter,
    transform_dispatcher: TransformDispatcher,
    reserved_keywords: Option<&'static [&'static str]>,
    parsed_comments: IndexSet<i32>, // Comment object identity, uniquely identified by source offset.
    license_builder: IndexSet<JsString>,
    first_fileoverview: Option<Arc<JSDocInfo>>,
    template_node: NodeId,
    default_template_node: NodeId,
    closure_unaware_template_node: NodeId,
    jsdoc_tracker: CommentTracker,
    non_jsdoc_tracker: CommentTracker,
    js_doc_source_kind: JsDocSourceKind,
    current_file_is_externs: bool,
    features: FeatureSet,
    result_node: Option<NodeId>,
    closure_unaware_code_ranges: Vec<(LineAndColumn, LineAndColumn)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LineAndColumn {
    line_no: i32,
    column: i32,
}
impl LineAndColumn {
    // port: IRFactory.LineAndColumn#fromSourcePosition
    fn from_source_position(pos: &SourcePosition) -> Self {
        Self {
            line_no: pos.line,
            column: pos.column,
        }
    }
    // port: IRFactory.LineAndColumn#compareTo
    fn compare_to(self, other: Self) -> std::cmp::Ordering {
        self.cmp(&other)
    }
}

impl<'a> IRFactory<'a> {
    // port: IRFactory#IRFactory
    fn new(
        ast: &mut Ast,
        source_file: Arc<dyn StaticSourceFile>,
        config: Config,
        error_reporter: &'a mut dyn ErrorReporter,
        comments: &[Comment],
        file_with_content: Arc<SourceFile>,
        js_doc_source_kind: JsDocSourceKind,
    ) -> Self {
        let jsdoc_tracker = CommentTracker::new(comments.to_vec(), true);
        let non_jsdoc_tracker = CommentTracker::new(comments.to_vec(), false);
        let default_template_node = Self::create_template_node(ast, source_file.clone());
        let closure_unaware_template_node =
            Self::create_closure_unaware_template_node(ast, default_template_node);
        let source_name = source_file.get_name().to_owned();
        let reserved_keywords = if config.strict_mode().is_strict() {
            Some(ES5_STRICT_RESERVED_KEYWORDS)
        } else if config.language_mode() == LanguageMode::ECMASCRIPT3 {
            None
        } else {
            Some(ES5_RESERVED_KEYWORDS)
        };
        Self {
            source_file,
            source_name,
            file_with_content,
            config,
            error_reporter,
            transform_dispatcher: TransformDispatcher::default(),
            reserved_keywords,
            parsed_comments: IndexSet::new(),
            license_builder: IndexSet::new(),
            first_fileoverview: None,
            template_node: default_template_node,
            default_template_node,
            closure_unaware_template_node,
            jsdoc_tracker,
            non_jsdoc_tracker,
            js_doc_source_kind,
            current_file_is_externs: false,
            features: FeatureSet::BARE_MINIMUM,
            result_node: None,
            closure_unaware_code_ranges: Vec::new(),
        }
    }
    // port: IRFactory#createTemplateNode
    fn create_template_node(ast: &mut Ast, source_file: Arc<dyn StaticSourceFile>) -> NodeId {
        let template_node = ast.new_node(Token::SCRIPT);
        template_node.set_static_source_file(ast, Some(source_file));
        template_node
    }
    // port: IRFactory#createClosureUnawareTemplateNode
    fn create_closure_unaware_template_node(
        ast: &mut Ast,
        default_template_node: NodeId,
    ) -> NodeId {
        let closure_unaware_template_node = default_template_node.clone_node(ast);
        closure_unaware_template_node.set_is_in_closure_unaware_subtree(ast, true);
        closure_unaware_template_node
    }
    // port: IRFactory#transformTree
    pub fn transform_tree(
        ast: &mut Ast,
        tree: &ParseTree,
        source_file: Arc<dyn StaticSourceFile>,
        config: Config,
        error_reporter: &'a mut dyn ErrorReporter,
        file: Arc<SourceFile>,
    ) -> P<Self> {
        let js_doc_source_kind = if source_file.is_type_script_source() {
            JsDocSourceKind::TSICKLE
        } else {
            JsDocSourceKind::NORMAL
        };
        let mut ir_factory = Self::new(
            ast,
            source_file,
            config,
            error_reporter,
            &tree.as_program().source_comments,
            file,
            js_doc_source_kind,
        );
        let n = ir_factory.process(ast, tree)?;
        ir_factory.set_source_info(ast, n, tree);
        for comment in &tree.as_program().source_comments {
            if matches!(
                comment.type_,
                comment::Type::JSDOC | comment::Type::IMPORTANT
            ) && !ir_factory
                .parsed_comments
                .contains(&comment.get_absolute_position())
            {
                let use_licenses_only_config = ir_factory.within_closure_unaware_code_range(
                    comment.location.start.line,
                    comment.location.start.column,
                );
                ir_factory.handle_possible_file_overview_jsdoc_comment(
                    ast,
                    comment,
                    use_licenses_only_config,
                );
            }
        }
        if n.is_script(ast) {
            let non_jsdoc_comment =
                ir_factory.parse_non_jsdoc_comment_at(&tree.location.end, false);
            if let Some(comment) = non_jsdoc_comment {
                n.set_trailing_non_jsdoc_comment(ast, Some(comment));
            }
        }
        ir_factory.set_file_overview_jsdoc(ast, n);
        ir_factory.validate_all(ast, n);
        ir_factory.result_node = Some(n);
        Ok(ir_factory)
    }
    // port: IRFactory#getResultNode
    pub fn get_result_node(&self) -> NodeId {
        self.result_node.unwrap()
    }
    // port: IRFactory#getFeatures
    pub fn get_features(&self) -> FeatureSet {
        self.features
    }
    // port: IRFactory#validateAll
    fn validate_all(&mut self, ast: &Ast, n: NodeId) {
        let mut work = VecDeque::new();
        let mut n = Some(n);
        while let Some(cur) = n {
            self.validate(ast, cur);
            let next_sibling = cur.get_next(ast);
            let first_child = cur.get_first_child(ast);
            if let Some(first_child) = first_child {
                if let Some(next_sibling) = next_sibling {
                    work.push_front(next_sibling);
                }
                n = Some(first_child);
            } else if next_sibling.is_some() {
                n = next_sibling;
            } else {
                n = work.pop_front();
            }
        }
        check_state!(work.is_empty());
    }
    // port: IRFactory#validate
    fn validate(&mut self, ast: &Ast, n: NodeId) {
        self.validate_parameters(ast, n);
        self.validate_break_continue(ast, n);
        self.validate_return(ast, n);
        self.validate_yield(ast, n);
        self.validate_await(ast, n);
        self.validate_new_dot_target(ast, n);
        self.validate_label(ast, n);
        self.validate_block_scoped_functions(ast, n);
        self.validate_shadow_host(ast, n);
    }
    // port: IRFactory#validateShadowHost
    fn validate_shadow_host(&mut self, ast: &Ast, n: NodeId) {
        if n.get_closure_unaware_shadow(ast).is_none() {
            return;
        }
        if !self
            .first_fileoverview
            .as_ref()
            .is_some_and(|f| f.is_closure_unaware_code())
        {
            self.error(
                ast,
                n,
                "invalid @closureUnaware node: must be in a file annotated with @closureUnaware",
            );
            return;
        }
        let parent = n.get_parent(ast).unwrap();
        if parent.is_call(ast) {
            return;
        }
        let grandparent = parent.get_parent(ast);
        if parent.is_get_prop(ast) && grandparent.is_some_and(|p| p.is_call(ast)) {
            let called_fn = parent.get_string(ast);
            if called_fn == "call" || called_fn == "apply" {
                return;
            }
        }
        self.error(
            ast,
            n,
            "invalid @closureUnaware node: must be directly called",
        );
    }
    // port: IRFactory#validateAwait
    fn validate_await(&mut self, ast: &Ast, n: NodeId) {
        let error_msg = if n.is_await(ast) {
            UNEXPECTED_AWAIT
        } else if n.is_for_await_of(ast) {
            UNEXPECTED_FOR_AWAIT_OF
        } else {
            return;
        };
        let mut parent = n.get_parent(ast);
        while let Some(p) = parent {
            if (p.is_block(ast) || p.is_member_field_def(ast))
                && p.get_parent(ast).is_some_and(|x| x.is_class_members(ast))
            {
                self.error(ast, n, error_msg);
                return;
            }
            if p.is_async_function(ast) {
                return;
            }
            if p.is_function(ast) {
                self.error(ast, n, error_msg);
                return;
            }
            if p.is_script(ast) && p.get_boolean_prop(ast, NodeId::ES6_MODULE) {
                self.maybe_warn_for_feature_node(ast, n, Feature::TOP_LEVEL_AWAIT);
                return;
            }
            parent = p.get_parent(ast);
        }
        self.error(ast, n, error_msg);
    }
    // port: IRFactory#validateYield
    fn validate_yield(&mut self, ast: &Ast, n: NodeId) {
        if !n.is_yield(ast) {
            return;
        }
        let mut parent = n.get_parent(ast);
        while let Some(p) = parent {
            if p.is_class_members(ast) {
                self.error(ast, n, UNEXPECTED_YIELD);
                return;
            }
            if p.is_generator_function(ast) {
                return;
            }
            parent = p.get_parent(ast);
        }
    }
    // port: IRFactory#validateReturn
    fn validate_return(&mut self, ast: &Ast, n: NodeId) {
        if !n.is_return(ast) {
            return;
        }
        let mut parent = n.get_parent(ast);
        while let Some(p) = parent {
            if p.is_class_members(ast) {
                self.error(ast, n, UNEXPECTED_RETURN);
                return;
            }
            if p.is_function(ast) {
                return;
            }
            parent = p.get_parent(ast);
        }
        self.error(ast, n, UNEXPECTED_RETURN);
    }
    // port: IRFactory#validateNewDotTarget
    fn validate_new_dot_target(&mut self, ast: &Ast, n: NodeId) {
        if n.get_token(ast) != Token::NEW_TARGET {
            return;
        }
        let mut parent = n.get_parent(ast);
        while let Some(p) = parent {
            if p.is_function(ast) {
                return;
            }
            parent = p.get_parent(ast);
        }
        self.error(ast, n, UNEXPECTED_NEW_DOT_TARGET);
    }
    // port: IRFactory#validateBreakContinue
    fn validate_break_continue(&mut self, ast: &Ast, n: NodeId) {
        if !n.is_break(ast) && !n.is_continue(ast) {
            return;
        }
        if let Some(label_name) = n.get_first_child(ast) {
            let mut parent = n.get_parent(ast).unwrap();
            while !parent.is_label(ast) || !Self::labels_match(ast, parent, label_name) {
                if parent.is_function(ast) || parent.is_script(ast) || parent.is_class_members(ast)
                {
                    self.error_js_string(
                        ast,
                        n,
                        &closure_rhino::java_lang::format_message(
                            &UNDEFINED_LABEL.into(),
                            &[label_name.get_string(ast)],
                        ),
                    );
                    break;
                }
                parent = parent.get_parent(ast).unwrap();
            }
            if parent.is_label(ast)
                && Self::labels_match(ast, parent, label_name)
                && n.is_continue(ast)
                && !Self::is_continue_target(ast, parent.get_last_child(ast).unwrap())
            {
                self.error(ast, n, UNEXPECTED_LABELLED_CONTINUE);
            }
        } else if n.is_continue(ast) {
            let mut parent = n.get_parent(ast).unwrap();
            while !Self::is_continue_target(ast, parent) {
                if parent.is_function(ast) || parent.is_script(ast) || parent.is_class_members(ast)
                {
                    self.error(ast, n, UNEXPECTED_CONTINUE);
                    break;
                }
                parent = parent.get_parent(ast).unwrap();
            }
        } else {
            let mut parent = n.get_parent(ast).unwrap();
            while !Self::is_break_target(ast, parent) {
                if parent.is_function(ast) || parent.is_script(ast) || parent.is_class_members(ast)
                {
                    self.error(ast, n, UNLABELED_BREAK);
                    break;
                }
                parent = parent.get_parent(ast).unwrap();
            }
        }
    }
    // port: IRFactory#isBreakTarget
    fn is_break_target(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::WHILE
                | Token::DO
                | Token::SWITCH
        )
    }
    // port: IRFactory#isContinueTarget
    fn is_continue_target(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::WHILE
                | Token::DO
        )
    }
    // port: IRFactory#labelsMatch
    fn labels_match(ast: &Ast, label: NodeId, label_name: NodeId) -> bool {
        label.get_first_child(ast).unwrap().get_string(ast) == label_name.get_string(ast)
    }
    // port: IRFactory#validateLabel
    fn validate_label(&mut self, ast: &Ast, n: NodeId) {
        if !n.is_label(ast) {
            return;
        }
        let label_name = n.get_first_child(ast).unwrap();
        let mut parent = n.get_parent(ast);
        while let Some(p) = parent {
            if p.is_function(ast) {
                break;
            }
            if p.is_label(ast) && Self::labels_match(ast, p, label_name) {
                self.error_js_string(
                    ast,
                    n,
                    &closure_rhino::java_lang::format_message(
                        &DUPLICATE_LABEL.into(),
                        &[label_name.get_string(ast)],
                    ),
                );
                break;
            }
            parent = p.get_parent(ast);
        }
    }
    // port: IRFactory#validateParameters
    fn validate_parameters(&mut self, ast: &Ast, n: NodeId) {
        if !n.is_param_list(ast) {
            return;
        }
        let mut seen_names = IndexSet::new();
        for c in n.children(ast) {
            ParsingUtil::get_param_or_pattern_names(ast, c, &mut |param| {
                let param_name = param.get_string(ast);
                if !seen_names.insert(param_name.clone()) {
                    self.error_reporter.warning_js_string(
                        &closure_rhino::java_lang::format_message(
                            &DUPLICATE_PARAMETER.into(),
                            &[param_name],
                        ),
                        &self.source_name,
                        param.get_lineno(ast),
                        param.get_charno(ast),
                    );
                }
            });
        }
    }
    // port: IRFactory#validateBlockScopedFunctions
    fn validate_block_scoped_functions(&mut self, ast: &Ast, n: NodeId) {
        if n.is_function(ast)
            && n.get_parent(ast).is_some_and(|p| p.is_block(ast))
            && !n.get_grandparent(ast).is_some_and(|p| p.is_function(ast))
        {
            self.maybe_warn_for_feature_node(ast, n, Feature::BLOCK_SCOPED_FUNCTION_DECLARATION);
        }
    }
    // port: IRFactory#setFileOverviewJsDoc
    fn set_file_overview_jsdoc(&mut self, ast: &mut Ast, ir_node: NodeId) {
        let mut fileoverview = self
            .first_fileoverview
            .as_ref()
            .map_or_else(JSDocInfo::builder, |f| f.to_builder());
        if !self.license_builder.is_empty() {
            let mut text = Vec::new();
            for s in &self.license_builder {
                text.extend_from_slice(s.as_units());
            }
            fileoverview.record_license(JsString::from_units(text));
        }
        ir_node.set_jsdoc_info(ast, fileoverview.build_with_always(false));
    }
    // port: IRFactory#transformBlock
    fn transform_block(&mut self, ast: &mut Ast, node: &ParseTree) -> P<NodeId> {
        let ir_node = self.transform(ast, node)?;
        if ir_node.is_block(ast) {
            return Ok(ir_node);
        }
        let new_block = if ir_node.is_empty(ast) {
            self.new_node(ast, Token::BLOCK)
        } else {
            self.new_node1(ast, Token::BLOCK, ir_node)
        };
        self.set_source_info_node(ast, new_block, ir_node);
        new_block.set_is_added_block(ast, true);
        Ok(new_block)
    }
    // port: IRFactory#handlePossibleFileOverviewJsDoc(JsDocInfoParser)
    fn handle_possible_file_overview_jsdoc(&mut self, parsed: &ParsedJsDoc) -> bool {
        if let Some(license) = &parsed.license {
            self.license_builder.insert(license.clone());
        }
        let new_fileoverview = &parsed.fileoverview;
        if match (new_fileoverview, &self.first_fileoverview) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        } {
            return false;
        }
        let new_fileoverview = new_fileoverview.as_ref().unwrap();
        if self.first_fileoverview.is_none() {
            self.first_fileoverview = Some(new_fileoverview.clone());
            self.current_file_is_externs = new_fileoverview.is_externs();
        } else {
            let mut merged = self.first_fileoverview.as_ref().unwrap().to_builder();
            if !new_fileoverview.get_suppressions().is_empty() {
                merged.record_suppressions(&new_fileoverview.get_suppressions());
            }
            if new_fileoverview.is_externs() {
                self.current_file_is_externs = true;
                merged.record_externs();
            }
            if new_fileoverview.is_no_coverage() {
                merged.record_no_coverage();
            }
            if new_fileoverview.is_type_summary() {
                merged.record_type_summary();
            }
            self.first_fileoverview = merged.build();
        }
        true
    }
    // port: IRFactory#handlePossibleFileOverviewJsDoc(Comment, boolean)
    fn handle_possible_file_overview_jsdoc_comment(
        &mut self,
        ast: &mut Ast,
        comment: &Comment,
        use_licenses_only_config: bool,
    ) {
        let parsed =
            self.create_js_doc_info_parser_with_config(ast, comment, use_licenses_only_config);
        self.parsed_comments.insert(comment.get_absolute_position());
        self.handle_possible_file_overview_jsdoc(&parsed);
    }
    // port: IRFactory#getJSDocCommentAt
    fn get_jsdoc_comment_at(&mut self, pos: &SourcePosition) -> Option<Comment> {
        let mut closest_previous_comment = None;
        while self.jsdoc_tracker.has_pending_comment_before(pos) {
            closest_previous_comment = self.jsdoc_tracker.current().cloned();
            self.jsdoc_tracker.advance();
        }
        closest_previous_comment
    }
    // port: IRFactory#parseJSDocInfoFrom
    fn parse_jsdoc_info_from(
        &mut self,
        ast: &mut Ast,
        comment: Option<&Comment>,
    ) -> Option<Arc<JSDocInfo>> {
        let comment = comment?;
        if self.within_closure_unaware_code_range(
            comment.location.start.line,
            comment.location.start.column,
        ) {
            self.jsdoc_tracker.backtrack();
            return None;
        }
        let parsed = self.create_js_doc_info_parser(ast, comment);
        self.parsed_comments.insert(comment.get_absolute_position());
        if self.handle_possible_file_overview_jsdoc(&parsed) {
            return None;
        }
        parsed.info
    }
    // port: IRFactory#parseJSDocInfoOnTree
    fn parse_jsdoc_info_on_tree(
        &mut self,
        ast: &mut Ast,
        tree: &ParseTree,
    ) -> Option<Arc<JSDocInfo>> {
        use ParseTreeType::*;
        match tree.type_ {
            EXPRESSION_STATEMENT
            | LABELLED_STATEMENT
            | EXPORT_DECLARATION
            | TEMPLATE_SUBSTITUTION => return None,
            CALL_EXPRESSION
            | CONDITIONAL_EXPRESSION
            | COMMA_EXPRESSION
            | BINARY_OPERATOR
            | MEMBER_EXPRESSION
            | MEMBER_LOOKUP_EXPRESSION
            | UPDATE_EXPRESSION => {
                if Self::find_nearest_node(tree).type_ == PAREN_EXPRESSION {
                    return None;
                }
            }
            _ => {}
        }
        let comment = self.get_jsdoc_comment_at(&tree.get_start());
        self.parse_jsdoc_info_from(ast, comment.as_ref())
    }
    // port: IRFactory#parseJSDocInfoOnToken
    fn parse_jsdoc_info_on_token(
        &mut self,
        ast: &mut Ast,
        token: &ParserToken,
    ) -> Option<Arc<JSDocInfo>> {
        let comment = self.get_jsdoc_comment_at(&token.get_start());
        self.parse_jsdoc_info_from(ast, comment.as_ref())
    }
    // port: IRFactory#parseInlineJSDocAt
    fn parse_inline_jsdoc_at(
        &mut self,
        ast: &mut Ast,
        pos: &SourcePosition,
    ) -> Option<Arc<JSDocInfo>> {
        if self.within_closure_unaware_code_range(pos.line, pos.column) {
            return None;
        }
        let comment = self.get_jsdoc_comment_at(pos);
        if let Some(c) = comment
            .as_ref()
            .filter(|c| c.value.index_of_char(b'@' as u16) < 0)
        {
            self.parse_inline_type_doc(ast, c)
        } else {
            self.parse_jsdoc_info_from(ast, comment.as_ref())
        }
    }
    // port: IRFactory#hasAnyPendingCommentBefore
    fn has_any_pending_comment_before(
        &self,
        pos: &SourcePosition,
        within_closure_unaware_code_range: bool,
    ) -> bool {
        self.non_jsdoc_tracker.has_pending_comment_before(pos)
            || (within_closure_unaware_code_range
                && self.jsdoc_tracker.has_pending_comment_before(pos))
    }
    // port: IRFactory#parseNonJSDocCommentAt
    fn parse_non_jsdoc_comment_at(
        &mut self,
        pos: &SourcePosition,
        is_inline: bool,
    ) -> Option<Arc<NonJSDocComment>> {
        let within_closure_unaware_code_range =
            self.within_closure_unaware_code_range(pos.line, pos.column);
        let mut result = Vec::new();
        let mut first_comment: Option<Comment> = None;
        let mut last_comment: Option<Comment> = None;
        while self.has_any_pending_comment_before(pos, within_closure_unaware_code_range) {
            let mut current_comment = self.non_jsdoc_tracker.current().cloned();
            let mut comment_from_jsdoc_tracker = false;
            if within_closure_unaware_code_range {
                let current_jsdoc_comment = self.jsdoc_tracker.current().cloned();
                if current_comment.is_none()
                    || current_jsdoc_comment.as_ref().is_some_and(|j| {
                        LineAndColumn::from_source_position(
                            &current_comment.as_ref().unwrap().location.start,
                        )
                        .compare_to(LineAndColumn::from_source_position(&j.location.start))
                        .is_gt()
                    })
                {
                    if current_jsdoc_comment
                        .as_ref()
                        .unwrap()
                        .value
                        .index_of(&JsString::from("@license"))
                        >= 0
                    {
                        self.jsdoc_tracker.advance();
                        continue;
                    }
                    comment_from_jsdoc_tracker = true;
                    current_comment = current_jsdoc_comment;
                }
            }
            let current_comment = current_comment.unwrap();
            if first_comment.is_none() {
                first_comment = Some(current_comment.clone());
            }
            if let Some(last_comment) = &last_comment {
                for _ in 0..current_comment.location.start.line - last_comment.location.end.line {
                    result.push(b'\n' as u16);
                }
            }
            result.extend_from_slice(current_comment.value.as_units());
            if comment_from_jsdoc_tracker {
                self.parsed_comments
                    .insert(current_comment.get_absolute_position());
                self.jsdoc_tracker.advance();
            } else {
                self.non_jsdoc_tracker.advance();
            }
            last_comment = Some(current_comment);
        }
        let first_comment = first_comment?;
        if self.config.js_doc_parsing_mode() != JsDocParsing::INCLUDE_ALL_COMMENTS {
            return None;
        }
        let last_comment = last_comment.unwrap();
        let mut non_jsdoc_comment = NonJSDocComment::new(
            first_comment.location.start,
            last_comment.location.end,
            Some(JsString::from_units(result)),
        );
        non_jsdoc_comment.set_ends_as_line_comment(last_comment.type_ == comment::Type::LINE);
        non_jsdoc_comment.set_is_inline(is_inline);
        Some(Arc::new(non_jsdoc_comment))
    }
    // port: IRFactory#parseTrailingNonJSDocCommentAt
    fn parse_trailing_non_jsdoc_comment_at(
        &mut self,
        token_end: &SourcePosition,
        possible_next_token_start: &SourcePosition,
    ) -> Option<Arc<NonJSDocComment>> {
        if self.config.js_doc_parsing_mode() != JsDocParsing::INCLUDE_ALL_COMMENTS {
            return None;
        }
        if !self
            .non_jsdoc_tracker
            .has_pending_comment_before(possible_next_token_start)
        {
            return None;
        }
        let comment = self.non_jsdoc_tracker.current().unwrap().clone();
        if token_end.line != comment.location.start.line
            || comment.location.start.offset <= token_end.offset
        {
            return None;
        }
        let pre_comment_text = self.file_with_content.contents.substring(
            (token_end.offset + 1) as usize,
            comment.location.start.offset as usize,
        );
        if !closure_rhino::java_lang::trim(&pre_comment_text).is_empty() {
            return None;
        }
        self.non_jsdoc_tracker.advance();
        let mut non_jsdoc_comment = NonJSDocComment::new(
            comment.location.start,
            comment.location.end,
            Some(comment.value),
        );
        non_jsdoc_comment.set_ends_as_line_comment(comment.type_ == comment::Type::LINE);
        Some(Arc::new(non_jsdoc_comment))
    }
    // port: IRFactory#findNearestNode
    fn find_nearest_node(mut tree: &ParseTree) -> &ParseTree {
        loop {
            tree = match &tree.data {
                ParseTreeData::ExpressionStatementTree(t) => &t.expression,
                ParseTreeData::CallExpressionTree(t) => &t.operand,
                ParseTreeData::BinaryOperatorTree(t) => &t.left,
                ParseTreeData::ConditionalExpressionTree(t) => &t.condition,
                ParseTreeData::MemberExpressionTree(t) => &t.operand,
                ParseTreeData::MemberLookupExpressionTree(t) => &t.operand,
                ParseTreeData::UpdateExpressionTree(t) => &t.operand,
                ParseTreeData::CommaExpressionTree(t) => &t.expressions[0],
                _ => return tree,
            };
        }
    }
    // port: IRFactory#transform
    fn transform(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let info = self.parse_jsdoc_info_on_tree(ast, tree);
        let comment = self.parse_non_jsdoc_comment_at(&tree.get_start(), false);
        let is_root_of_closure_unaware_sub_tree =
            info.as_ref().is_some_and(|i| i.is_closure_unaware_code());
        if is_root_of_closure_unaware_sub_tree {
            self.template_node = self.closure_unaware_template_node;
            let start = LineAndColumn::from_source_position(&tree.location.start);
            let end = LineAndColumn::from_source_position(&tree.location.end);
            // TreeRangeSet.add(closed(start, end)): merging overlaps does not affect contains().
            self.closure_unaware_code_ranges.push((start, end));
        }
        let mut node = self.process(ast, tree)?;
        if let Some(info) = info {
            node = self.maybe_inject_cast_node(ast, tree, &info, node);
            node.set_jsdoc_info(ast, Some(info));
        }
        if let Some(comment) = comment {
            node.set_non_jsdoc_comment(ast, Some(comment));
        }
        if is_root_of_closure_unaware_sub_tree {
            self.set_source_info(ast, node, tree);
            self.template_node = self.default_template_node;
            node = self.hide_closure_unaware_code_root(ast, node);
        }
        self.set_source_info(ast, node, tree);
        Ok(node)
    }
    // port: IRFactory#hideClosureUnawareCodeRoot
    fn hide_closure_unaware_code_root(&mut self, ast: &mut Ast, function: NodeId) -> NodeId {
        if !function.is_function(ast) {
            self.error(ast, function, "@closureUnaware root must be function");
            return function;
        }
        let info = function.get_jsdoc_info(ast).unwrap();
        if info.get_per_file_closure_unaware_mode() != Some(PerFileClosureUnawareMode::UNSPECIFIED)
        {
            self.error(
                ast,
                function,
                "@closureUnaware mode can only be specified at the fileoverview level",
            );
        }
        function.set_jsdoc_info(ast, None);
        let shadow_name_node = IR::name(ast, JSCOMP_CLOSURE_UNAWARE_CODE_SHADOW_HOST_NAME);
        let sink_name =
            IR::name(ast, JSCOMP_CLOSURE_UNAWARE_SINK_VALUE).srcref(ast, self.template_node);
        let sink_call = IR::call(ast, sink_name, &[function]).srcref(ast, self.template_node);
        sink_call.put_boolean_prop(ast, NodeId::FREE_CALL, true);
        let expr = IR::expr_result(ast, sink_call).srcref(ast, self.template_node);
        let script = IR::script_with_children(ast, &[expr]);
        let shadow_js_root = IR::root(ast, &[script]);
        script.set_static_source_file_from(ast, self.template_node);
        script.set_is_in_closure_unaware_subtree(ast, true);
        shadow_name_node.clone_props_from(ast, self.template_node);
        shadow_name_node.set_closure_unaware_shadow(ast, Some(shadow_js_root));
        shadow_name_node
    }
    // port: IRFactory#withinClosureUnawareCodeRange
    fn within_closure_unaware_code_range(&self, line: i32, line_column_no: i32) -> bool {
        let point = LineAndColumn {
            line_no: line,
            column: line_column_no,
        };
        self.closure_unaware_code_ranges
            .iter()
            .any(|&(start, end)| start <= point && point <= end)
    }
    // port: IRFactory#maybeInjectCastNode
    fn maybe_inject_cast_node(
        &self,
        ast: &mut Ast,
        node: &ParseTree,
        info: &JSDocInfo,
        ir_node: NodeId,
    ) -> NodeId {
        if node.type_ == ParseTreeType::PAREN_EXPRESSION && info.has_type() {
            self.new_node1(ast, Token::CAST, ir_node)
        } else {
            ir_node
        }
    }
    // port: IRFactory#transformNodeWithInlineComments
    fn transform_node_with_inline_comments(
        &mut self,
        ast: &mut Ast,
        tree: &ParseTree,
    ) -> P<NodeId> {
        let info = self.parse_inline_jsdoc_at(ast, &tree.get_start());
        let comment = self.parse_non_jsdoc_comment_at(&tree.get_start(), true);
        let node = self.process(ast, tree)?;
        if let Some(info) = info {
            node.set_jsdoc_info(ast, Some(info));
        }
        if let Some(comment) = comment {
            node.set_non_jsdoc_comment(ast, Some(comment));
        }
        self.set_source_info(ast, node, tree);
        Ok(node)
    }
    // port: IRFactory#lineno(ParseTree)
    fn lineno(node: &ParseTree) -> i32 {
        Self::lineno_position(&node.location.start)
    }
    // port: IRFactory#lineno(Token)
    fn lineno_token(token: &ParserToken) -> i32 {
        Self::lineno_position(&token.location.start)
    }
    // port: IRFactory#lineno(SourcePosition)
    fn lineno_position(location: &SourcePosition) -> i32 {
        location.line + 1
    }
    // port: IRFactory#charno(ParseTree)
    fn charno(node: &ParseTree) -> i32 {
        Self::charno_position(&node.location.start)
    }
    // port: IRFactory#charno(Token)
    fn charno_token(token: &ParserToken) -> i32 {
        Self::charno_position(&token.location.start)
    }
    // port: IRFactory#charno(SourcePosition)
    fn charno_position(location: &SourcePosition) -> i32 {
        location.column
    }
    // port: IRFactory#languageFeatureWarningMessage
    pub fn language_feature_warning_message(feature: Feature) -> String {
        let for_feature = LanguageMode::minimum_required_for(feature);
        if for_feature == LanguageMode::UNSUPPORTED {
            format!("This language feature is not currently supported by the compiler: {feature}")
        } else {
            format!(
                "This language feature is only supported for {for_feature:?} mode or better: {feature}"
            )
        }
    }
    // port: IRFactory#maybeWarnForFeature(ParseTree, Feature)
    fn maybe_warn_for_feature(&mut self, node: &ParseTree, feature: Feature) {
        self.features = self.features.with(feature);
        if self
            .within_closure_unaware_code_range(node.location.start.line, node.location.start.column)
        {
            return;
        }
        if self.current_file_is_externs {
            return;
        }
        if !self.is_supported_for_input_language_mode(feature) {
            self.error_reporter.warning(
                &Self::language_feature_warning_message(feature),
                &self.source_name,
                Self::lineno(node),
                Self::charno(node),
            );
        }
    }
    // port: IRFactory#maybeWarnForFeature(Token, Feature)
    fn maybe_warn_for_feature_token(&mut self, token: &ParserToken, feature: Feature) {
        self.features = self.features.with(feature);
        if self.within_closure_unaware_code_range(
            token.location.start.line,
            token.location.start.column,
        ) {
            return;
        }
        if self.current_file_is_externs {
            return;
        }
        if !self.is_supported_for_input_language_mode(feature) {
            self.error_reporter.warning(
                &Self::language_feature_warning_message(feature),
                &self.source_name,
                Self::lineno_token(token),
                Self::charno_token(token),
            );
        }
    }
    // port: IRFactory#maybeWarnForFeature(Node, Feature)
    fn maybe_warn_for_feature_node(&mut self, ast: &Ast, node: NodeId, feature: Feature) {
        self.features = self.features.with(feature);
        if self.within_closure_unaware_code_range(node.get_lineno(ast), node.get_charno(ast)) {
            return;
        }
        if self.current_file_is_externs {
            return;
        }
        if !self.is_supported_for_input_language_mode(feature) {
            self.error_reporter.warning(
                &Self::language_feature_warning_message(feature),
                &self.source_name,
                node.get_lineno(ast),
                node.get_charno(ast),
            );
        }
    }
    // port: IRFactory#setSourceInfo(Node, Node)
    fn set_source_info_node(&self, ast: &mut Ast, node: NodeId, reference: NodeId) {
        node.set_lineno_charno(ast, reference.get_lineno(ast), reference.get_charno(ast));
        self.set_length_from(ast, node, reference);
    }
    // port: IRFactory#setSourceInfo(Node, ParseTree)
    fn set_source_info(&self, ast: &mut Ast, ir_node: NodeId, node: &ParseTree) {
        if ir_node.get_lineno(ast) == -1 {
            self.set_source_info_position(ast, ir_node, &node.location.start, &node.location.end);
        }
    }
    // port: IRFactory#setSourceInfo(Node, Token)
    fn set_source_info_token(&self, ast: &mut Ast, ir_node: NodeId, token: &ParserToken) {
        self.set_source_info_position(ast, ir_node, &token.location.start, &token.location.end);
    }
    // port: IRFactory#setSourceInfo(Node, SourcePosition, SourcePosition)
    fn set_source_info_position(
        &self,
        ast: &mut Ast,
        node: NodeId,
        start: &SourcePosition,
        end: &SourcePosition,
    ) {
        if node.get_lineno(ast) == -1 {
            node.set_lineno_charno(
                ast,
                Self::lineno_position(start),
                Self::charno_position(start),
            );
            self.set_length(ast, node, start, end);
        }
    }
    // port: IRFactory#createJsDocInfoParser(Comment)
    fn create_js_doc_info_parser(&mut self, ast: &mut Ast, node: &Comment) -> ParsedJsDoc {
        self.create_js_doc_info_parser_with_config(ast, node, false)
    }
    // port: IRFactory#createJsDocInfoParser(Comment, boolean)
    fn create_js_doc_info_parser_with_config(
        &mut self,
        ast: &mut Ast,
        node: &Comment,
        use_licenses_only_config: bool,
    ) -> ParsedJsDoc {
        let comment = node.value.clone();
        let lineno = Self::lineno_position(&node.location.start);
        let charno = Self::charno_position(&node.location.start);
        let mut config = self.config.clone();
        if use_licenses_only_config {
            config = config
                .to_builder()
                .set_js_doc_parsing_mode(JsDocParsing::LICENSE_COMMENTS_ONLY)
                .build();
        }
        let mut parser = JsDocInfoParser::new(
            ast,
            JsDocTokenStream::new_with_lineno_and_charno(
                comment.substring_from(3),
                lineno,
                charno + 3,
            ),
            Some(comment),
            node.location.start.offset,
            Some(self.template_node),
            &config,
            self.js_doc_source_kind,
            self.error_reporter,
        );
        parser.set_file_overview_jsdoc_info(self.first_fileoverview.clone());
        if node.type_ == comment::Type::IMPORTANT && !node.value.is_empty() {
            parser.parse_important_comment();
        } else {
            parser.parse(ast);
        }
        // Release the reporter borrow before updating IRFactory's fileoverview state.
        ParsedJsDoc {
            license: parser.get_license_text(),
            fileoverview: parser.get_file_overview_jsdoc_info(),
            info: parser.retrieve_and_reset_parsed_jsdoc_info(),
        }
    }
    // port: IRFactory#parseInlineTypeDoc
    fn parse_inline_type_doc(&mut self, ast: &mut Ast, node: &Comment) -> Option<Arc<JSDocInfo>> {
        let comment = node.value.clone();
        let lineno = Self::lineno_position(&node.location.start);
        let charno = Self::charno_position(&node.location.start);
        let mut parser = JsDocInfoParser::new(
            ast,
            JsDocTokenStream::new_with_lineno_and_charno(
                comment.substring_from(3),
                lineno,
                charno + 3,
            ),
            Some(comment),
            node.location.start.offset,
            Some(self.template_node),
            &self.config,
            self.js_doc_source_kind,
            self.error_reporter,
        );
        parser.parse_inline_type_doc(ast)
    }
    // port: IRFactory#setLength
    fn set_length(
        &self,
        ast: &mut Ast,
        node: NodeId,
        start: &SourcePosition,
        end: &SourcePosition,
    ) {
        node.set_length(ast, end.offset - start.offset);
    }
    // port: IRFactory#setLengthFrom
    fn set_length_from(&self, ast: &mut Ast, node: NodeId, reference: NodeId) {
        node.set_length(ast, reference.get_length(ast));
    }
    // TransformDispatcher's methods take the factory receiver so its mutable state is borrowed
    // only for the individual arena calls; the nested dispatcher's scope trackers remain fields.
    // port: IRFactory.TransformDispatcher#newStringNodeFromIdentifier
    fn new_string_node_from_identifier(
        &mut self,
        ast: &mut Ast,
        type_: Token,
        mut identifier_type: IdentifierType,
        identifier_token: &ParserToken,
    ) -> NodeId {
        if !self.transform_dispatcher.class_scope.in_scope() {
            identifier_type = IdentifierType::STANDARD;
        }
        let value;
        if identifier_token.is_private_identifier() {
            if identifier_type == IdentifierType::CAN_BE_PRIVATE {
                self.maybe_warn_for_feature_token(identifier_token, Feature::PRIVATE_ELEMENTS);
            } else {
                self.error_reporter.error(
                    "Private identifiers may not be used in this context",
                    &self.source_name,
                    Self::lineno_token(identifier_token),
                    Self::charno_token(identifier_token),
                );
            }
            value = identifier_token.get_maybe_private_value().clone();
        } else {
            value = identifier_token.get_value().clone();
        }
        let node = self.new_string_node(ast, type_, value);
        if identifier_type == IdentifierType::CAN_BE_PRIVATE
            && identifier_token.is_private_identifier()
        {
            node.set_private_identifier(ast);
        }
        node
    }
    // port: IRFactory.TransformDispatcher#processObjectLitKey
    fn process_object_lit_key(
        &mut self,
        ast: &mut Ast,
        input: Option<&ParserToken>,
        output: Token,
        identifier_type: IdentifierType,
    ) -> P<NodeId> {
        let Some(input) = input else {
            return Ok(self.create_missing_expression_node(ast));
        };
        if input.type_ == TokenType::IDENTIFIER {
            return self.process_name(ast, input, output, identifier_type);
        }
        let jsdoc_info = self.parse_jsdoc_info_on_token(ast, input);
        let comment = self.parse_non_jsdoc_comment_at(&input.get_start(), true);
        let value = match input.type_ {
            TokenType::NUMBER => JsString::from_units(
                closure_rhino::dtoa::d_to_a::number_to_string(self.normalize_number(input)?)
                    .expect("DToA.numberToString"),
            ),
            TokenType::BIGINT => self.normalize_big_int(input)?.to_string().into(),
            _ => self.normalize_string(input, false)?,
        };
        let node = self.new_string_node(ast, output, value);
        if let Some(info) = jsdoc_info {
            node.set_jsdoc_info(ast, Some(info));
        }
        if let Some(comment) = comment {
            node.set_non_jsdoc_comment(ast, Some(comment));
        }
        self.set_source_info_token(ast, node, input);
        node.put_boolean_prop(ast, NodeId::QUOTED_PROP, true);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processComprehension
    fn process_comprehension(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.unsupported_language_feature(ast, tree, "array/generator comprehensions")
    }
    // port: IRFactory.TransformDispatcher#processComprehensionFor
    fn process_comprehension_for(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.unsupported_language_feature(ast, tree, "array/generator comprehensions")
    }
    // port: IRFactory.TransformDispatcher#processComprehensionIf
    fn process_comprehension_if(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.unsupported_language_feature(ast, tree, "array/generator comprehensions")
    }
    // port: IRFactory.TransformDispatcher#processArrayLiteral
    fn process_array_literal(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_array_literal_expression();
        let node = self.new_node(ast, Token::ARRAYLIT);
        node.set_trailing_comma(ast, tree.has_trailing_comma);
        for child in &tree.elements {
            let c = self.transform(ast, child)?;
            node.add_child_to_back(ast, c);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processArrayPattern
    fn process_array_pattern(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::ARRAY_DESTRUCTURING);
        let node = self.new_node(ast, Token::ARRAY_PATTERN);
        for child in &tree.as_array_pattern().elements {
            let element_node = match child.type_ {
                ParseTreeType::DEFAULT_PARAMETER => self.process_default_parameter(ast, child)?,
                ParseTreeType::ITER_REST => {
                    self.maybe_warn_for_feature(child, Feature::ARRAY_PATTERN_REST);
                    self.transform_node_with_inline_comments(ast, child)?
                }
                _ => self.transform_node_with_inline_comments(ast, child)?,
            };
            node.add_child_to_back(ast, element_node);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processObjectPattern
    fn process_object_pattern(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::OBJECT_DESTRUCTURING);
        let node = self.new_node(ast, Token::OBJECT_PATTERN);
        for child in &tree.as_object_pattern().fields {
            let child_node = self.process_object_pattern_element(ast, child)?;
            node.add_child_to_back(ast, child_node);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processObjectPatternElement
    fn process_object_pattern_element(&mut self, ast: &mut Ast, child: &ParseTree) -> P<NodeId> {
        match child.type_ {
            ParseTreeType::DEFAULT_PARAMETER => {
                self.process_object_pattern_shorthand_with_default(ast, child)
            }
            ParseTreeType::PROPERTY_NAME_ASSIGNMENT => {
                self.process_object_pattern_property_name_assignment(ast, child)
            }
            ParseTreeType::COMPUTED_PROPERTY_DEFINITION => {
                self.process_object_pattern_computed_property_definition(ast, child)
            }
            ParseTreeType::OBJECT_REST => {
                self.maybe_warn_for_feature(child, Feature::OBJECT_PATTERN_REST);
                let target = self.transform_node_with_inline_comments(
                    ast,
                    &child.as_object_rest().assignment_target,
                )?;
                let rest = self.new_node1(ast, Token::OBJECT_REST, target);
                self.set_source_info(ast, rest, child);
                Ok(rest)
            }
            _ => Err(ParseError {
                class: "java.lang.IllegalStateException",
                message: Some(
                    format!("Unexpected object pattern element: {:?}", child.type_).into(),
                ),
            }),
        }
    }
    // port: IRFactory.TransformDispatcher#processObjectPatternShorthandWithDefault
    fn process_object_pattern_shorthand_with_default(
        &mut self,
        ast: &mut Ast,
        default_parameter: &ParseTree,
    ) -> P<NodeId> {
        let default_value_node = self.process_default_parameter(ast, default_parameter)?;
        let name_node = default_value_node.get_first_child(ast).unwrap();
        let string_key_node =
            self.new_string_node(ast, Token::STRING_KEY, name_node.get_string(ast));
        self.maybe_add_non_js_doc_comment(ast, string_key_node, &default_parameter.get_start());
        self.set_source_info_node(ast, string_key_node, name_node);
        string_key_node.set_shorthand_property(ast, true);
        string_key_node.add_child_to_back(ast, default_value_node);
        Ok(string_key_node)
    }
    // port: IRFactory.TransformDispatcher#processObjectPatternPropertyNameAssignment
    fn process_object_pattern_property_name_assignment(
        &mut self,
        ast: &mut Ast,
        property_name_assignment: &ParseTree,
    ) -> P<NodeId> {
        let property_name_assignment = property_name_assignment.as_property_name_assignment();
        let key = self.process_object_lit_key(
            ast,
            Some(&property_name_assignment.name),
            Token::STRING_KEY,
            IdentifierType::STANDARD,
        )?;
        let value_node;
        if let Some(target_tree) = &property_name_assignment.value {
            value_node = self.process_destructuring_element_target(ast, target_tree)?;
        } else {
            value_node = self.process_name_with_inline_comments(
                ast,
                &property_name_assignment.name,
                IdentifierType::STANDARD,
            );
            key.set_shorthand_property(ast, true);
        }
        key.add_child_to_front(ast, value_node);
        Ok(key)
    }
    // port: IRFactory.TransformDispatcher#processDestructuringElementTarget
    fn process_destructuring_element_target(
        &mut self,
        ast: &mut Ast,
        target_tree: &ParseTree,
    ) -> P<NodeId> {
        if target_tree.type_ == ParseTreeType::DEFAULT_PARAMETER {
            self.process_default_parameter(ast, target_tree)
        } else if target_tree.type_ == ParseTreeType::IDENTIFIER_EXPRESSION {
            self.process_name_tree_with_inline_comments(ast, target_tree, IdentifierType::STANDARD)
        } else {
            self.transform_node_with_inline_comments(ast, target_tree)
        }
    }
    // port: IRFactory.TransformDispatcher#processObjectPatternComputedPropertyDefinition
    fn process_object_pattern_computed_property_definition(
        &mut self,
        ast: &mut Ast,
        tree: &ParseTree,
    ) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::COMPUTED_PROPERTIES);
        let value = tree.as_computed_property_definition();
        let expression_node = self.transform(ast, &value.property)?;
        let value_node = self.process_destructuring_element_target(ast, &value.value)?;
        let computed_property_node =
            self.new_node2(ast, Token::COMPUTED_PROP, expression_node, value_node);
        self.set_source_info(ast, computed_property_node, tree);
        Ok(computed_property_node)
    }
    // port: IRFactory.TransformDispatcher#processAstRoot
    fn process_ast_root(&mut self, ast: &mut Ast, root_node: &ParseTree) -> P<NodeId> {
        let script_node = self.new_node(ast, Token::SCRIPT);
        for child in &root_node.as_program().source_elements {
            let n = self.transform(ast, child)?;
            script_node.add_child_to_back(ast, n);
        }
        self.parse_directives(ast, script_node);
        let is_goog_module = Self::is_goog_module_file(ast, script_node);
        if is_goog_module || self.features.has(Feature::MODULES) {
            let module_node = self.new_node(ast, Token::MODULE_BODY);
            self.set_source_info(ast, module_node, root_node);
            let children = script_node.remove_children(ast);
            module_node.add_children_to_back(ast, children);
            script_node.add_child_to_back(ast, module_node);
            script_node.put_boolean_prop(
                ast,
                if is_goog_module {
                    NodeId::GOOG_MODULE
                } else {
                    NodeId::ES6_MODULE
                },
                true,
            );
        }
        Ok(script_node)
    }
    // port: IRFactory.TransformDispatcher#isGoogModuleFile
    fn is_goog_module_file(ast: &Ast, script_node: NodeId) -> bool {
        check_argument!(script_node.is_script(ast));
        let Some(expr_result) = script_node.get_first_child(ast) else {
            return false;
        };
        if !expr_result.is_expr_result(ast) {
            return false;
        }
        let call = expr_result.get_first_child(ast).unwrap();
        if !call.is_call(ast) {
            return false;
        }
        QualifiedName::of("goog.module").matches(ast, call.get_first_child(ast).unwrap())
            && call.has_two_children(ast)
            && call.get_second_child(ast).unwrap().is_string_lit(ast)
    }
    // port: IRFactory.TransformDispatcher#parseDirectives
    fn parse_directives(&self, ast: &mut Ast, node: NodeId) {
        let mut use_strict = false;
        loop {
            let Some(statement) = node.get_first_child(ast) else {
                break;
            };
            if !statement.is_expr_result(ast) {
                break;
            }
            let directive = statement.get_first_child(ast).unwrap();
            if !directive.is_string_lit(ast) || directive.get_string_ref(ast) != "use strict" {
                break;
            }
            use_strict = true;
            statement.detach(ast);
        }
        if use_strict {
            node.set_use_strict(ast, true);
        }
    }
    // port: IRFactory.TransformDispatcher#processBlock
    fn process_block(&mut self, ast: &mut Ast, block_node: &ParseTree) -> P<NodeId> {
        let node = self.new_node(ast, Token::BLOCK);
        for child in &block_node.as_block().statements {
            let child_node = self.transform(ast, child)?;
            node.add_child_to_back(ast, child_node);
            self.attach_possible_trailing_comment(ast, child_node, &child.get_end());
        }
        let last_comment = self.parse_non_jsdoc_comment_at(&block_node.get_end(), false);
        self.add_extra_trailing_comment(ast, node, last_comment);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processBreakStatement
    fn process_break_statement(&mut self, ast: &mut Ast, statement_node: &ParseTree) -> P<NodeId> {
        let node = self.new_node(ast, Token::BREAK);
        if let Some(name) = &statement_node.as_break_statement().name {
            let label = self.transform_label_name(ast, name);
            node.add_child_to_back(ast, label);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#transformLabelName
    fn transform_label_name(&mut self, ast: &mut Ast, token: &ParserToken) -> NodeId {
        let label = self.new_string_node_from_identifier(
            ast,
            Token::LABEL_NAME,
            IdentifierType::STANDARD,
            token,
        );
        self.maybe_add_non_js_doc_comment(ast, label, &token.get_start());
        self.set_source_info_token(ast, label, token);
        label
    }
    // port: IRFactory.TransformDispatcher#processConditionalExpression
    fn process_conditional_expression(
        &mut self,
        ast: &mut Ast,
        expr_node: &ParseTree,
    ) -> P<NodeId> {
        let expr_node = expr_node.as_conditional_expression();
        let condition = self.transform(ast, &expr_node.condition)?;
        let left = self.transform(ast, &expr_node.left)?;
        let right = self.transform(ast, &expr_node.right)?;
        Ok(self.new_node3(ast, Token::HOOK, condition, left, right))
    }
    // port: IRFactory.TransformDispatcher#processContinueStatement
    fn process_continue_statement(
        &mut self,
        ast: &mut Ast,
        statement_node: &ParseTree,
    ) -> P<NodeId> {
        let node = self.new_node(ast, Token::CONTINUE);
        if let Some(name) = &statement_node.as_continue_statement().name {
            let label = self.transform_label_name(ast, name);
            node.add_child_to_back(ast, label);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processDoLoop
    fn process_do_loop(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_do_while_statement();
        let body = self.transform_block(ast, &tree.body)?;
        let condition = self.transform(ast, &tree.condition)?;
        Ok(self.new_node2(ast, Token::DO, body, condition))
    }
    // port: IRFactory.TransformDispatcher#processElementGet
    fn process_element_get(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_member_lookup_expression();
        let operand = self.transform(ast, &tree.operand)?;
        let member_expression = self.transform(ast, &tree.member_expression)?;
        Ok(self.new_node2(ast, Token::GETELEM, operand, member_expression))
    }
    // port: IRFactory.TransformDispatcher#processOptChainElementGet
    fn process_opt_chain_element_get(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::OPTIONAL_CHAINING);
        let tree = tree.as_optional_member_lookup_expression();
        let operand = self.transform(ast, &tree.operand)?;
        let member_expression = self.transform(ast, &tree.member_expression)?;
        let get_elem = self.new_node2(ast, Token::OPTCHAIN_GETELEM, operand, member_expression);
        get_elem.set_is_optional_chain_start(ast, tree.is_start_of_optional_chain);
        Ok(get_elem)
    }
    // port: IRFactory.TransformDispatcher#processEmptyStatement
    fn process_empty_statement(&mut self, ast: &mut Ast, _tree: &ParseTree) -> P<NodeId> {
        Ok(self.new_node(ast, Token::EMPTY))
    }
    // port: IRFactory.TransformDispatcher#processExpressionStatement
    fn process_expression_statement(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let node = self.new_node(ast, Token::EXPR_RESULT);
        let expression = self.transform(ast, &tree.as_expression_statement().expression)?;
        node.add_child_to_back(ast, expression);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processForInLoop
    fn process_for_in_loop(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_for_in_statement();
        let initializer = self.transform(ast, &tree.initializer)?;
        let collection = self.transform(ast, &tree.collection)?;
        let body = self.transform_block(ast, &tree.body)?;
        Ok(self.new_node3(ast, Token::FOR_IN, initializer, collection, body))
    }
    // port: IRFactory.TransformDispatcher#processForOf
    fn process_for_of(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::FOR_OF);
        let tree = tree.as_for_of_statement();
        let initializer = self.transform(ast, &tree.initializer)?;
        let collection = self.transform(ast, &tree.collection)?;
        let body = self.transform_block(ast, &tree.body)?;
        Ok(self.new_node3(ast, Token::FOR_OF, initializer, collection, body))
    }
    // port: IRFactory.TransformDispatcher#processForAwaitOf
    fn process_for_await_of(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::FOR_AWAIT_OF);
        let tree = tree.as_for_await_of_statement();
        let initializer = self.transform(ast, &tree.initializer)?;
        let collection = self.transform(ast, &tree.collection)?;
        let body = self.transform_block(ast, &tree.body)?;
        Ok(self.new_node3(ast, Token::FOR_AWAIT_OF, initializer, collection, body))
    }
    // port: IRFactory.TransformDispatcher#processForLoop
    fn process_for_loop(&mut self, ast: &mut Ast, loop_node: &ParseTree) -> P<NodeId> {
        let tree = loop_node.as_for_statement();
        let initializer = self.transform_or_empty(ast, tree.initializer.as_deref(), loop_node)?;
        let condition = self.transform_or_empty(ast, tree.condition.as_deref(), loop_node)?;
        let increment = self.transform_or_empty(ast, tree.increment.as_deref(), loop_node)?;
        let node = self.new_node3(ast, Token::FOR, initializer, condition, increment);
        let body = self.transform_block(ast, &tree.body)?;
        node.add_child_to_back(ast, body);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#transformOrEmpty(ParseTree, ParseTree)
    fn transform_or_empty(
        &mut self,
        ast: &mut Ast,
        tree: Option<&ParseTree>,
        parent: &ParseTree,
    ) -> P<NodeId> {
        match tree {
            None => {
                let n = self.new_node(ast, Token::EMPTY);
                self.set_source_info(ast, n, parent);
                Ok(n)
            }
            Some(tree) => self.transform(ast, tree),
        }
    }
    // port: IRFactory.TransformDispatcher#transformOrEmpty(IdentifierToken, ParseTree)
    fn transform_or_empty_token(
        &mut self,
        ast: &mut Ast,
        token: Option<&ParserToken>,
        parent: &ParseTree,
    ) -> P<NodeId> {
        match token {
            None => {
                let n = self.new_node(ast, Token::EMPTY);
                self.set_source_info(ast, n, parent);
                Ok(n)
            }
            Some(token) => self.process_name(ast, token, Token::NAME, IdentifierType::STANDARD),
        }
    }
    // port: IRFactory.TransformDispatcher#processFunctionCall
    fn process_function_call(&mut self, ast: &mut Ast, call_node: &ParseTree) -> P<NodeId> {
        let call_node = call_node.as_call_expression();
        let operand = self.transform(ast, &call_node.operand)?;
        let node = self.new_node1(ast, Token::CALL, operand);
        let arguments_tree = call_node.arguments.as_argument_list();
        node.set_trailing_comma(ast, arguments_tree.has_trailing_comma);
        let zones = Self::get_end_of_arg_comment_zones(
            &arguments_tree.arguments,
            &arguments_tree.comma_positions,
            &call_node.arguments.location.end,
        );
        let arg_count = 0; // Java intentionally never increments this counter for calls.
        for child in &arguments_tree.arguments {
            let child_node = self.transform(ast, child)?;
            node.add_child_to_back(ast, child_node);
            self.attach_possible_trailing_comments_for_arg(ast, child_node, &zones[arg_count]);
        }
        Self::annotate_calls(ast, node);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#annotateCalls
    fn annotate_calls(ast: &mut Ast, n: NodeId) {
        check_state!(
            n.is_call(ast) || n.is_opt_chain_call(ast) || n.is_tagged_template_lit(ast),
            "%s",
            n.to_string(ast)
        );
        let mut callee = n.get_first_child(ast).unwrap();
        while callee.is_cast(ast) {
            callee = callee.get_first_child(ast).unwrap();
        }
        if !Self::is_normal_or_opt_chain_get(ast, callee) {
            n.put_boolean_prop(ast, NodeId::FREE_CALL, true);
            if callee.is_name(ast) && callee.get_string_ref(ast) == "eval" {
                callee.put_boolean_prop(ast, NodeId::DIRECT_EVAL, true);
            } else if callee.is_comma(ast) && callee.get_first_child(ast).unwrap().is_number(ast) {
                let real_callee = callee.get_second_child(ast).unwrap();
                real_callee.detach(ast);
                callee.replace_with(ast, real_callee);
            }
        }
    }
    // port: IRFactory.TransformDispatcher#isNormalOrOptChainGet
    fn is_normal_or_opt_chain_get(ast: &Ast, n: NodeId) -> bool {
        n.is_get_prop(ast)
            || n.is_get_elem(ast)
            || n.is_opt_chain_get_prop(ast)
            || n.is_opt_chain_get_elem(ast)
    }
    // port: IRFactory.TransformDispatcher#getEndOfArgCommentZones
    fn get_end_of_arg_comment_zones(
        args: &[Tree],
        comma_positions: &[SourcePosition],
        arg_list_end_position: &SourcePosition,
    ) -> Vec<SourcePosition> {
        let mut zones = Vec::new();
        for (comma_count, arg) in args.iter().enumerate() {
            if args.len() > comma_count + 1 {
                let next_param = &args[comma_count + 1];
                if next_param.location.start.line > arg.location.end.line {
                    zones.push(SourcePosition::new(
                        None,
                        i32::MAX,
                        arg.location.end.line,
                        i32::MAX,
                    ));
                } else {
                    zones.push(comma_positions[comma_count].clone());
                }
            } else {
                zones.push(arg_list_end_position.clone());
            }
        }
        zones
    }
    // port: IRFactory.TransformDispatcher#attachPossibleTrailingCommentsForArg
    fn attach_possible_trailing_comments_for_arg(
        &mut self,
        ast: &mut Ast,
        param_node: NodeId,
        end_zone: &SourcePosition,
    ) {
        if let Some(trailing_comment) = self.parse_non_jsdoc_comment_at(end_zone, true) {
            param_node.set_trailing_non_jsdoc_comment(ast, Some(trailing_comment));
        }
    }
    // port: IRFactory.TransformDispatcher#attachPossibleTrailingComment
    fn attach_possible_trailing_comment(
        &mut self,
        ast: &mut Ast,
        node: NodeId,
        token_end: &SourcePosition,
    ) {
        let next_line = SourcePosition::new(None, i32::MAX, token_end.line + 1, 0);
        if let Some(trailing_comment) =
            self.parse_trailing_non_jsdoc_comment_at(token_end, &next_line)
        {
            node.set_trailing_non_jsdoc_comment(ast, Some(trailing_comment));
        }
    }
    // port: IRFactory.TransformDispatcher#addExtraTrailingComment
    fn add_extra_trailing_comment(
        &self,
        ast: &mut Ast,
        node: NodeId,
        last_comment: Option<Arc<NonJSDocComment>>,
    ) {
        let Some(last_comment) = last_comment else {
            return;
        };
        let Some(last_child) = node.get_last_child(ast) else {
            node.set_trailing_non_jsdoc_comment(ast, Some(last_comment));
            return;
        };
        let Some(current_comment) = last_child.get_trailing_non_jsdoc_comment(ast) else {
            let end = last_comment.get_end_position();
            let new_start = SourcePosition::new(None, end.offset - 1, end.line - 1, 0);
            let newline_comment = NonJSDocComment::new(
                new_start,
                end.clone(),
                Some(JsString::from("\n").concat(&last_comment.get_comment_string())),
            );
            last_child.set_trailing_non_jsdoc_comment(ast, Some(Arc::new(newline_comment)));
            return;
        };
        let blank_lines =
            last_comment.get_start_position().line - current_comment.get_end_position().line;
        let num_white_space = if blank_lines == 0 {
            last_comment.get_start_position().column - current_comment.get_end_position().column - 1
        } else {
            0
        };
        let mut comment = current_comment.get_comment_string().as_units().to_vec();
        for _ in 0..num_white_space {
            comment.push(b' ' as u16);
        }
        for _ in 0..blank_lines {
            comment.push(b'\n' as u16);
        }
        comment.extend_from_slice(last_comment.get_comment_string().as_units());
        let mut all_comments = NonJSDocComment::new(
            current_comment.get_start_position().clone(),
            last_comment.get_end_position().clone(),
            Some(JsString::from_units(comment)),
        );
        all_comments.set_ends_as_line_comment(last_comment.is_ending_as_line_comment());
        all_comments.set_is_inline(true);
        last_child.set_trailing_non_jsdoc_comment(ast, Some(Arc::new(all_comments)));
    }
    // port: IRFactory.TransformDispatcher#processOptChainFunctionCall
    fn process_opt_chain_function_call(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::OPTIONAL_CHAINING);
        let tree = tree.as_opt_chain_call_expression();
        let operand = self.transform(ast, &tree.operand)?;
        let node = self.new_node1(ast, Token::OPTCHAIN_CALL, operand);
        node.set_trailing_comma(ast, tree.has_trailing_comma);
        for child in &tree.arguments.as_argument_list().arguments {
            let c = self.transform(ast, child)?;
            node.add_child_to_back(ast, c);
        }
        node.set_is_optional_chain_start(ast, tree.is_start_of_optional_chain);
        Self::annotate_calls(ast, node);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processFunction
    fn process_function(&mut self, ast: &mut Ast, function_tree: &ParseTree) -> P<NodeId> {
        use function_declaration_tree::Kind;
        let tree = function_tree.as_function_declaration();
        let is_declaration = tree.kind == Kind::DECLARATION;
        let is_member = tree.kind == Kind::MEMBER;
        let is_arrow = tree.kind == Kind::ARROW;
        let is_async = tree.is_async;
        let is_generator = tree.is_generator;
        let is_signature = tree.function_body.type_ == ParseTreeType::EMPTY_STATEMENT;
        let identifier_type = if tree.is_class_member {
            IdentifierType::CAN_BE_PRIVATE
        } else {
            IdentifierType::STANDARD
        };
        if is_generator {
            self.maybe_warn_for_feature(function_tree, Feature::GENERATORS);
        }
        if is_member {
            self.maybe_warn_for_feature(function_tree, Feature::MEMBER_DECLARATIONS);
        }
        if is_arrow {
            self.maybe_warn_for_feature(function_tree, Feature::ARROW_FUNCTIONS);
        }
        if is_async {
            self.maybe_warn_for_feature(function_tree, Feature::ASYNC_FUNCTIONS);
        }
        if is_generator && is_async {
            self.maybe_warn_for_feature(function_tree, Feature::ASYNC_GENERATORS);
        }
        let new_name = if let Some(name) = &tree.name {
            self.process_name_with_inline_comments(ast, name, identifier_type)
        } else {
            let new_name = if is_declaration || is_member {
                self.error_reporter.error(
                    "unnamed function statement",
                    &self.source_name,
                    Self::lineno(function_tree),
                    Self::charno(function_tree),
                );
                self.create_missing_name_node(ast)
            } else {
                self.new_string_node(ast, Token::NAME, "")
            };
            self.set_source_info(ast, new_name, function_tree);
            new_name
        };
        let node = self.new_node(ast, Token::FUNCTION);
        if is_member {
            new_name.set_string(ast, "");
        }
        node.add_child_to_back(ast, new_name);
        let params = self.transform(ast, &tree.formal_parameter_list)?;
        node.add_child_to_back(ast, params);
        let mut body_node = self.transform(ast, &tree.function_body)?;
        if !is_arrow && !is_signature && !body_node.is_block(ast) {
            check_state!(self.config.run_mode() == RunMode::KEEP_GOING);
            body_node = IR::block(ast);
        }
        self.parse_directives(ast, body_node);
        node.add_child_to_back(ast, body_node);
        node.set_is_generator_function(ast, is_generator);
        node.set_is_arrow_function(ast, is_arrow);
        node.set_is_async_function(ast, is_async);
        node.put_boolean_prop(ast, NodeId::OPT_ES6_TYPED, tree.is_optional);
        self.attach_possible_trailing_comment(ast, node, &function_tree.get_end());
        if is_member {
            self.set_source_info(ast, node, function_tree);
            let name = required(tree.name.as_ref())?;
            let member = self.new_string_node_from_identifier(
                ast,
                Token::MEMBER_FUNCTION_DEF,
                identifier_type,
                name,
            );
            member.add_child_to_back(ast, node);
            member.set_static_member(ast, tree.is_static);
            self.set_source_info_token(ast, member, name);
            Ok(member)
        } else {
            Ok(node)
        }
    }
    // port: IRFactory.TransformDispatcher#processField
    fn process_field(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let field = tree.as_field_declaration();
        self.maybe_warn_for_feature(
            tree,
            if field.name.is_private_identifier() {
                Feature::PRIVATE_ELEMENTS
            } else {
                Feature::PUBLIC_CLASS_FIELDS
            },
        );
        let node = self.new_string_node_from_identifier(
            ast,
            Token::MEMBER_FIELD_DEF,
            IdentifierType::CAN_BE_PRIVATE,
            &field.name,
        );
        self.maybe_add_non_js_doc_comment(ast, node, &tree.get_start());
        if let Some(initializer) = &field.initializer {
            let initializer = self.transform(ast, initializer)?;
            node.add_child_to_back(ast, initializer);
            self.set_length(ast, node, &tree.location.start, &tree.location.end);
        }
        node.put_boolean_prop(ast, NodeId::STATIC_MEMBER, field.is_static);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processComputedPropertyField
    fn process_computed_property_field(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::PUBLIC_CLASS_FIELDS);
        let tree = tree.as_computed_property_field();
        let key = self.transform(ast, &tree.property)?;
        let node = if let Some(initializer) = &tree.initializer {
            let initializer = self.transform(ast, initializer)?;
            self.new_node2(ast, Token::COMPUTED_FIELD_DEF, key, initializer)
        } else {
            self.new_node1(ast, Token::COMPUTED_FIELD_DEF, key)
        };
        node.put_boolean_prop(ast, NodeId::STATIC_MEMBER, tree.is_static);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processFormalParameterList
    fn process_formal_parameter_list(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let formal = tree.as_formal_parameter_list();
        let params = self.new_node(ast, Token::PARAM_LIST);
        params.set_trailing_comma(ast, formal.has_trailing_comma);
        if !self.check_parameters(tree) {
            return Ok(params);
        }
        let zones = Self::get_end_of_arg_comment_zones(
            &formal.parameters,
            &formal.comma_positions,
            &tree.location.end,
        );
        for (arg_count, param) in formal.parameters.iter().enumerate() {
            let param_node = match param.type_ {
                ParseTreeType::DEFAULT_PARAMETER => self.process_default_parameter(ast, param)?,
                ParseTreeType::ITER_REST => {
                    self.maybe_warn_for_feature(param, Feature::REST_PARAMETERS);
                    self.transform_node_with_inline_comments(ast, param)?
                }
                _ => {
                    let param_node = self.transform_node_with_inline_comments(ast, param)?;
                    self.attach_possible_trailing_comments_for_arg(
                        ast,
                        param_node,
                        &zones[arg_count],
                    );
                    param_node
                }
            };
            check_state!(
                param_node.is_name(ast)
                    || param_node.is_rest(ast)
                    || param_node.is_array_pattern(ast)
                    || param_node.is_object_pattern(ast)
                    || param_node.is_default_value(ast)
            );
            params.add_child_to_back(ast, param_node);
        }
        Ok(params)
    }
    // port: IRFactory.TransformDispatcher#processDefaultParameter
    fn process_default_parameter(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::DEFAULT_PARAMETERS);
        let default = tree.as_default_parameter();
        let target_tree = &default.lhs;
        let target_node = if target_tree.type_ == ParseTreeType::IDENTIFIER_EXPRESSION {
            self.process_name_with_inline_comments(
                ast,
                required(
                    target_tree
                        .as_identifier_expression()
                        .identifier_token
                        .as_ref(),
                )?,
                IdentifierType::STANDARD,
            )
        } else {
            self.transform_node_with_inline_comments(ast, target_tree)?
        };
        let default_value_expression = self.transform(ast, &default.default_value)?;
        let default_value_node = self.new_node2(
            ast,
            Token::DEFAULT_VALUE,
            target_node,
            default_value_expression,
        );
        self.report_error_if_yield_or_await_in_default_value(ast, default_value_node);
        self.set_source_info(ast, default_value_node, tree);
        Ok(default_value_node)
    }
    // port: IRFactory.TransformDispatcher#processIterRest
    fn process_iter_rest(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let target =
            self.transform_node_with_inline_comments(ast, &tree.as_iter_rest().assignment_target)?;
        Ok(self.new_node1(ast, Token::ITER_REST, target))
    }
    // port: IRFactory.TransformDispatcher#processIterSpread
    fn process_iter_spread(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::SPREAD_EXPRESSIONS);
        let expression = self.transform(ast, &tree.as_iter_spread().expression)?;
        Ok(self.new_node1(ast, Token::ITER_SPREAD, expression))
    }
    // port: IRFactory.TransformDispatcher#processObjectSpread
    fn process_object_spread(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::OBJECT_LITERALS_WITH_SPREAD);
        let expression = self.transform(ast, &tree.as_object_spread().expression)?;
        Ok(self.new_node1(ast, Token::OBJECT_SPREAD, expression))
    }
    // port: IRFactory.TransformDispatcher#processIfStatement
    fn process_if_statement(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_if_statement();
        let node = self.new_node(ast, Token::IF);
        let condition = self.transform(ast, &tree.condition)?;
        node.add_child_to_back(ast, condition);
        let if_clause = self.transform_block(ast, &tree.if_clause)?;
        node.add_child_to_back(ast, if_clause);
        if let Some(else_clause) = &tree.else_clause {
            let else_clause = self.transform_block(ast, else_clause)?;
            node.add_child_to_back(ast, else_clause);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#markBinaryExpressionFeatures
    fn mark_binary_expression_features(&mut self, expr_node: &ParseTree) {
        match expr_node.as_binary_operator().operator.type_ {
            TokenType::STAR_STAR | TokenType::STAR_STAR_EQUAL => {
                self.maybe_warn_for_feature(expr_node, Feature::EXPONENT_OP)
            }
            TokenType::QUESTION_QUESTION => {
                self.maybe_warn_for_feature(expr_node, Feature::NULL_COALESCE_OP)
            }
            TokenType::QUESTION_QUESTION_EQUAL => {
                self.maybe_warn_for_feature(expr_node, Feature::NULL_COALESCE_OP);
                self.maybe_warn_for_feature(expr_node, Feature::LOGICAL_ASSIGNMENT);
            }
            TokenType::OR_EQUAL | TokenType::AND_EQUAL => {
                self.maybe_warn_for_feature(expr_node, Feature::LOGICAL_ASSIGNMENT)
            }
            _ => {}
        }
    }
    // port: IRFactory.TransformDispatcher#processBinaryExpression
    fn process_binary_expression(&mut self, ast: &mut Ast, expr_node: &ParseTree) -> P<NodeId> {
        let expr = expr_node.as_binary_operator();
        if self
            .jsdoc_tracker
            .has_pending_comment_before(&expr.right.location.start)
            || self
                .non_jsdoc_tracker
                .has_pending_comment_before(&expr.right.location.start)
        {
            self.mark_binary_expression_features(expr_node);
            let left = self.transform_lhs_of_binary_tree(ast, expr_node)?;
            let right = self.transform(ast, &expr.right)?;
            Ok(self.new_node2(
                ast,
                Self::transform_binary_token_type(expr.operator.type_),
                left,
                right,
            ))
        } else {
            self.process_binary_expression_helper(ast, expr_node)
        }
    }
    // port: IRFactory.TransformDispatcher#processBinaryExpressionHelper
    fn process_binary_expression_helper(
        &mut self,
        ast: &mut Ast,
        expr_tree: &ParseTree,
    ) -> P<NodeId> {
        let mut root = None;
        let mut current: Option<NodeId> = None;
        let mut expr_tree = Some(expr_tree);
        while let Some(tree) = expr_tree {
            self.mark_binary_expression_features(tree);
            let previous = current;
            let expr = tree.as_binary_operator();
            let right = self.transform(ast, &expr.right)?;
            let n = self.new_node1(
                ast,
                Self::transform_binary_token_type(expr.operator.type_),
                right,
            );
            current = Some(n);
            self.set_source_info(ast, n, tree);
            if let Some(previous) = previous {
                previous.add_child_to_front(ast, n);
            }
            if expr.left.type_ == ParseTreeType::BINARY_OPERATOR {
                expr_tree = Some(&expr.left);
            } else {
                let left_node = self.transform_lhs_of_binary_tree(ast, tree)?;
                n.add_child_to_front(ast, left_node);
                expr_tree = None;
            }
            if root.is_none() {
                root = current;
            }
        }
        Ok(root.unwrap())
    }
    // port: IRFactory.TransformDispatcher#transformLhsOfBinaryTree
    fn transform_lhs_of_binary_tree(&mut self, ast: &mut Ast, expr_tree: &ParseTree) -> P<NodeId> {
        let expr = expr_tree.as_binary_operator();
        let lhs_of_in_is_private_id = expr.operator.type_ == TokenType::IN
            && expr.left.type_ == ParseTreeType::IDENTIFIER_EXPRESSION
            && required(
                expr.left
                    .as_identifier_expression()
                    .identifier_token
                    .as_ref(),
            )?
            .is_private_identifier();
        let _decrement = self
            .transform_dispatcher
            .private_id_lhs_of_in_scope
            .maybe_increment(lhs_of_in_is_private_id);
        self.transform(ast, &expr.left)
    }
    // port: IRFactory.TransformDispatcher#processDebuggerStatement
    fn process_debugger_statement(&mut self, ast: &mut Ast, _tree: &ParseTree) -> P<NodeId> {
        Ok(self.new_node(ast, Token::DEBUGGER))
    }
    // port: IRFactory.TransformDispatcher#processThisExpression
    fn process_this_expression(&mut self, ast: &mut Ast, _tree: &ParseTree) -> P<NodeId> {
        Ok(self.new_node(ast, Token::THIS))
    }
    // port: IRFactory.TransformDispatcher#processLabeledStatement
    fn process_labeled_statement(&mut self, ast: &mut Ast, label_tree: &ParseTree) -> P<NodeId> {
        let tree = label_tree.as_labelled_statement();
        let statement = self.transform(ast, &tree.statement)?;
        if statement.is_function(ast)
            || statement.is_class(ast)
            || statement.is_let(ast)
            || statement.is_const(ast)
        {
            self.error_reporter.error(
                "Lexical declarations are only allowed at top level or inside a block.",
                &self.source_name,
                Self::lineno(label_tree),
                Self::charno(label_tree),
            );
            return Ok(statement);
        }
        let name = self.transform_label_name(ast, required(tree.name.as_ref())?);
        Ok(self.new_node2(ast, Token::LABEL, name, statement))
    }
    // port: IRFactory.TransformDispatcher#processName(IdentifierExpressionTree, IdentifierType)
    fn process_name_tree(
        &mut self,
        ast: &mut Ast,
        name_node: &ParseTree,
        identifier_type: IdentifierType,
    ) -> P<NodeId> {
        self.process_name(
            ast,
            required(
                name_node
                    .as_identifier_expression()
                    .identifier_token
                    .as_ref(),
            )?,
            Token::NAME,
            identifier_type,
        )
    }
    // port: IRFactory.TransformDispatcher#processName(IdentifierToken, Token, IdentifierType)
    fn process_name(
        &mut self,
        ast: &mut Ast,
        token: &ParserToken,
        output: Token,
        identifier_type: IdentifierType,
    ) -> P<NodeId> {
        let comment = self.parse_non_jsdoc_comment_at(&token.get_start(), true);
        let node = self.new_string_node_from_identifier(ast, output, identifier_type, token);
        if output == Token::NAME {
            self.maybe_warn_reserved_keyword(token);
            if let Some(info) = self.parse_jsdoc_info_on_token(ast, token) {
                node.set_jsdoc_info(ast, Some(info));
            }
        }
        if let Some(comment) = comment {
            node.set_non_jsdoc_comment(ast, Some(comment));
        }
        self.set_source_info_token(ast, node, token);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processString
    fn process_string(&mut self, ast: &mut Ast, token: &ParserToken) -> P<NodeId> {
        check_argument!(token.type_ == TokenType::STRING);
        let comment = self.parse_non_jsdoc_comment_at(&token.get_start(), true);
        let value = self.normalize_string(token, false)?;
        let node = self.new_string_node(ast, Token::STRINGLIT, value);
        if let Some(comment) = comment {
            node.set_non_jsdoc_comment(ast, Some(comment));
        }
        self.set_source_info_token(ast, node, token);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processTemplateLiteralToken
    fn process_template_literal_token(&mut self, ast: &mut Ast, token: &ParserToken) -> P<NodeId> {
        check_argument!(matches!(
            token.type_,
            TokenType::NO_SUBSTITUTION_TEMPLATE
                | TokenType::TEMPLATE_HEAD
                | TokenType::TEMPLATE_MIDDLE
                | TokenType::TEMPLATE_TAIL
        ));
        let template = token.as_template_literal();
        let cooked = if template.has_error() {
            None
        } else {
            Some(self.normalize_string(token, true)?)
        };
        let node = self.new_template_lit_string_node(ast, cooked, template.literal.value.clone());
        self.set_source_info_token(ast, node, token);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processNameWithInlineComments(IdentifierExpressionTree, IdentifierType)
    fn process_name_tree_with_inline_comments(
        &mut self,
        ast: &mut Ast,
        expression: &ParseTree,
        identifier_type: IdentifierType,
    ) -> P<NodeId> {
        Ok(self.process_name_with_inline_comments(
            ast,
            required(
                expression
                    .as_identifier_expression()
                    .identifier_token
                    .as_ref(),
            )?,
            identifier_type,
        ))
    }
    // port: IRFactory.TransformDispatcher#processNameWithInlineComments(IdentifierToken, IdentifierType)
    fn process_name_with_inline_comments(
        &mut self,
        ast: &mut Ast,
        token: &ParserToken,
        identifier_type: IdentifierType,
    ) -> NodeId {
        let info = self.parse_inline_jsdoc_at(ast, &token.get_start());
        let comment = self.parse_non_jsdoc_comment_at(&token.get_start(), false);
        self.maybe_warn_reserved_keyword(token);
        let node = self.new_string_node_from_identifier(ast, Token::NAME, identifier_type, token);
        if let Some(info) = info {
            node.set_jsdoc_info(ast, Some(info));
        }
        if let Some(comment) = comment {
            node.set_non_jsdoc_comment(ast, Some(comment));
        }
        self.set_source_info_token(ast, node, token);
        node
    }
    // port: IRFactory.TransformDispatcher#maybeWarnKeywordProperty
    fn maybe_warn_keyword_property(&mut self, ast: &Ast, node: NodeId) {
        if self.current_file_is_externs {
            return;
        }
        if !TokenStream::is_keyword(&node.get_string(ast)) {
            return;
        }
        self.features = self.features.with(Feature::KEYWORDS_AS_PROPERTIES);
        if self.config.language_mode() == LanguageMode::ECMASCRIPT3 {
            self.error_reporter.warning(
                INVALID_ES3_PROP_NAME,
                &self.source_name,
                node.get_lineno(ast),
                node.get_charno(ast),
            );
        }
    }
    // port: IRFactory.TransformDispatcher#maybeWarnReservedKeyword
    fn maybe_warn_reserved_keyword(&mut self, token: &ParserToken) {
        let identifier = token.get_maybe_private_value();
        let mut is_identifier = false;
        if TokenStream::is_keyword(identifier) {
            self.features = self.features.with(Feature::ES3_KEYWORDS_AS_IDENTIFIERS);
            is_identifier = self.config.language_mode() == LanguageMode::ECMASCRIPT3;
        }
        if self
            .reserved_keywords
            .is_some_and(|keywords| keywords.iter().any(|s| identifier == s))
        {
            self.features = self.features.with(Feature::KEYWORDS_AS_PROPERTIES);
            is_identifier = self.config.language_mode() == LanguageMode::ECMASCRIPT3;
        }
        if is_identifier {
            self.error_reporter.error(
                "identifier is a reserved word",
                &self.source_name,
                Self::lineno_token(token),
                Self::charno_token(token),
            );
        }
    }
    // port: IRFactory.TransformDispatcher#processNewExpression
    fn process_new_expression(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_new_expression();
        let operand = self.transform(ast, &tree.operand)?;
        let node = self.new_node1(ast, Token::NEW, operand);
        node.set_trailing_comma(ast, tree.has_trailing_comma);
        if let Some(args) = &tree.arguments {
            for arg in &args.as_argument_list().arguments {
                let arg = self.transform(ast, arg)?;
                node.add_child_to_back(ast, arg);
            }
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processNumberLiteral
    fn process_number_literal(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let value = self.normalize_number(&tree.as_literal_expression().literal_token)?;
        let number = self.new_number_node(ast, value);
        self.set_source_info(ast, number, tree);
        Ok(number)
    }
    // port: IRFactory.TransformDispatcher#processBigIntLiteral
    fn process_big_int_literal(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::BIGINT);
        let value = self.normalize_big_int(&tree.as_literal_expression().literal_token)?;
        let bigint = self.new_big_int_node(ast, value);
        self.set_source_info(ast, bigint, tree);
        Ok(bigint)
    }
    // port: IRFactory.TransformDispatcher#processObjectLiteral
    fn process_object_literal(&mut self, ast: &mut Ast, obj_tree: &ParseTree) -> P<NodeId> {
        let tree = obj_tree.as_object_literal_expression();
        let node = self.new_node(ast, Token::OBJECTLIT);
        node.set_trailing_comma(ast, tree.has_trailing_comma);
        let mut maybe_warn = false;
        for el in &tree.property_name_and_values {
            if el.type_ == ParseTreeType::DEFAULT_PARAMETER {
                self.error_reporter.error(
                    "Default value cannot appear at top level of an object literal.",
                    &self.source_name,
                    Self::lineno(el),
                    0,
                );
                continue;
            } else if el.type_ == ParseTreeType::GET_ACCESSOR && self.maybe_report_getter(el) {
                continue;
            } else if el.type_ == ParseTreeType::SET_ACCESSOR && self.maybe_report_setter(el) {
                continue;
            }
            let key = self.transform(ast, el)?;
            if !key.is_computed_prop(ast)
                && !key.is_quoted_string_key(ast)
                && !key.is_spread(ast)
                && !self.current_file_is_externs
            {
                self.maybe_warn_keyword_property(ast, key);
            }
            if key.is_shorthand_property(ast) {
                maybe_warn = true;
            }
            node.add_child_to_back(ast, key);
        }
        if maybe_warn {
            self.maybe_warn_for_feature(obj_tree, Feature::SHORTHAND_OBJECT_PROPERTIES);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processComputedPropertyDefinition
    fn process_computed_property_definition(
        &mut self,
        ast: &mut Ast,
        tree: &ParseTree,
    ) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::COMPUTED_PROPERTIES);
        let tree = tree.as_computed_property_definition();
        let property = self.transform(ast, &tree.property)?;
        let value = self.transform(ast, &tree.value)?;
        Ok(self.new_node2(ast, Token::COMPUTED_PROP, property, value))
    }
    // port: IRFactory.TransformDispatcher#processComputedPropertyMethod
    fn process_computed_property_method(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::COMPUTED_PROPERTIES);
        let tree = tree.as_computed_property_method();
        let property = self.transform(ast, &tree.property)?;
        let method = self.transform(ast, &tree.method)?;
        let n = self.new_node2(ast, Token::COMPUTED_PROP, property, method);
        n.put_boolean_prop(ast, NodeId::COMPUTED_PROP_METHOD, true);
        if tree.method.as_function_declaration().is_static {
            n.set_static_member(ast, true);
        }
        Ok(n)
    }
    // port: IRFactory.TransformDispatcher#processComputedPropertyGetter
    fn process_computed_property_getter(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::COMPUTED_PROPERTIES);
        let tree = tree.as_computed_property_getter();
        let key = self.transform(ast, &tree.property)?;
        let body = self.transform(ast, &tree.body)?;
        let name = IR::name(ast, "");
        let params = IR::param_list(ast, &[]);
        let function = IR::function(ast, name, params, body);
        function.srcref_tree_if_missing(ast, body);
        let n = self.new_node2(ast, Token::COMPUTED_PROP, key, function);
        n.put_boolean_prop(ast, NodeId::COMPUTED_PROP_GETTER, true);
        n.put_boolean_prop(ast, NodeId::STATIC_MEMBER, tree.is_static);
        Ok(n)
    }
    // port: IRFactory.TransformDispatcher#processComputedPropertySetter
    fn process_computed_property_setter(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::COMPUTED_PROPERTIES);
        let tree = tree.as_computed_property_setter();
        let key = self.transform(ast, &tree.property)?;
        let params = self.process_formal_parameter_list(ast, &tree.parameter)?;
        self.set_source_info(ast, params, &tree.parameter);
        let body = self.transform(ast, &tree.body)?;
        let name = IR::name(ast, "");
        let function = IR::function(ast, name, params, body);
        function.srcref_tree_if_missing(ast, body);
        let n = self.new_node2(ast, Token::COMPUTED_PROP, key, function);
        n.put_boolean_prop(ast, NodeId::COMPUTED_PROP_SETTER, true);
        n.put_boolean_prop(ast, NodeId::STATIC_MEMBER, tree.is_static);
        Ok(n)
    }
    // port: IRFactory.TransformDispatcher#processGetAccessor
    fn process_get_accessor(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_get_accessor();
        let identifier_type = if tree.is_class_member {
            IdentifierType::CAN_BE_PRIVATE
        } else {
            IdentifierType::STANDARD
        };
        let key = self.process_object_lit_key(
            ast,
            Some(&tree.property_name),
            Token::GETTER_DEF,
            identifier_type,
        )?;
        let body = self.transform(ast, &tree.body)?;
        let dummy_name = self.new_string_node(ast, Token::NAME, "");
        self.set_source_info(ast, dummy_name, &tree.body);
        let param_list = self.new_node(ast, Token::PARAM_LIST);
        self.set_source_info(ast, param_list, &tree.body);
        let value = self.new_node3(ast, Token::FUNCTION, dummy_name, param_list, body);
        self.set_source_info(ast, value, &tree.body);
        key.add_child_to_front(ast, value);
        key.set_static_member(ast, tree.is_static);
        Ok(key)
    }
    // port: IRFactory.TransformDispatcher#processSetAccessor
    fn process_set_accessor(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_set_accessor();
        let identifier_type = if tree.is_class_member {
            IdentifierType::CAN_BE_PRIVATE
        } else {
            IdentifierType::STANDARD
        };
        let key = self.process_object_lit_key(
            ast,
            Some(&tree.property_name),
            Token::SETTER_DEF,
            identifier_type,
        )?;
        let param_list = self.process_formal_parameter_list(ast, &tree.parameter)?;
        self.set_source_info(ast, param_list, &tree.parameter);
        let body = self.transform(ast, &tree.body)?;
        let dummy_name = self.new_string_node(ast, Token::NAME, "");
        self.set_source_info_token(ast, dummy_name, &tree.property_name);
        let value = self.new_node3(ast, Token::FUNCTION, dummy_name, param_list, body);
        self.set_source_info(ast, value, &tree.body);
        key.add_child_to_front(ast, value);
        key.set_static_member(ast, tree.is_static);
        Ok(key)
    }
    // port: IRFactory.TransformDispatcher#processPropertyNameAssignment
    fn process_property_name_assignment(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_property_name_assignment();
        let key = self.process_object_lit_key(
            ast,
            Some(&tree.name),
            Token::STRING_KEY,
            IdentifierType::STANDARD,
        )?;
        if let Some(value) = &tree.value {
            let value = self.transform(ast, value)?;
            key.add_child_to_front(ast, value);
        } else {
            let value = self.new_string_node(ast, Token::NAME, key.get_string(ast));
            self.maybe_add_non_js_doc_comment(ast, value, &tree.name.get_start());
            value.srcref(ast, key);
            key.set_shorthand_property(ast, true);
            key.add_child_to_front(ast, value);
        }
        Ok(key)
    }
    // port: IRFactory.TransformDispatcher#checkParenthesizedExpression
    fn check_parenthesized_expression(&mut self, expr_node: &ParseTree) {
        let expression = &expr_node.as_paren_expression().expression;
        if expression.type_ == ParseTreeType::COMMA_EXPRESSION {
            let last_child = expression.as_comma_expression().expressions.last().unwrap();
            if last_child.type_ == ParseTreeType::ITER_REST {
                self.error_reporter.error(
                    "A rest parameter must be in a parameter list.",
                    &self.source_name,
                    Self::lineno(last_child),
                    Self::charno(last_child),
                );
            }
        }
    }
    // port: IRFactory.TransformDispatcher#processParenthesizedExpression
    fn process_parenthesized_expression(
        &mut self,
        ast: &mut Ast,
        expr_node: &ParseTree,
    ) -> P<NodeId> {
        self.check_parenthesized_expression(expr_node);
        let expr = self.transform(ast, &expr_node.as_paren_expression().expression)?;
        expr.set_is_parenthesized(ast, true);
        Ok(expr)
    }
    // port: IRFactory.TransformDispatcher#processPropertyGet
    fn process_property_get(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let get_node = tree.as_member_expression();
        let left_child = self.transform(ast, &get_node.operand)?;
        let Some(prop_name) = &get_node.member_name else {
            return Ok(left_child);
        };
        let identifier_type = if left_child.is_super(ast) {
            IdentifierType::STANDARD
        } else {
            IdentifierType::CAN_BE_PRIVATE
        };
        let get_prop =
            self.new_string_node_from_identifier(ast, Token::GETPROP, identifier_type, prop_name);
        self.maybe_add_non_js_doc_comment(ast, get_prop, &prop_name.get_start());
        get_prop.add_child_to_back(ast, left_child);
        self.set_source_info_token(ast, get_prop, prop_name);
        self.maybe_warn_keyword_property(ast, get_prop);
        Ok(get_prop)
    }
    // port: IRFactory.TransformDispatcher#processOptChainPropertyGet
    fn process_opt_chain_property_get(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::OPTIONAL_CHAINING);
        let get_node = tree.as_optional_member_expression();
        let left_child = self.transform(ast, &get_node.operand)?;
        let Some(prop_name) = &get_node.member_name else {
            return Ok(left_child);
        };
        let identifier_type = if left_child.is_super(ast) {
            IdentifierType::STANDARD
        } else {
            IdentifierType::CAN_BE_PRIVATE
        };
        let get_prop = self.new_string_node_from_identifier(
            ast,
            Token::OPTCHAIN_GETPROP,
            identifier_type,
            prop_name,
        );
        self.maybe_add_non_js_doc_comment(ast, get_prop, &prop_name.get_start());
        get_prop.add_child_to_back(ast, left_child);
        get_prop.set_is_optional_chain_start(ast, get_node.is_start_of_optional_chain);
        self.set_source_info_token(ast, get_prop, prop_name);
        self.maybe_warn_keyword_property(ast, get_prop);
        Ok(get_prop)
    }
    // port: IRFactory.TransformDispatcher#processRegExpLiteral
    fn process_reg_exp_literal(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let token = &tree.as_literal_expression().literal_token;
        let literal_string_node = self.new_string_literal_node(ast, Self::normalize_regex(token));
        self.set_source_info_token(ast, literal_string_node, token);
        let node = self.new_node1(ast, Token::REGEXP, literal_string_node);
        let raw_regex = &token.as_literal().value;
        let last_slash = raw_regex.last_index_of_char(b'/' as u16);
        let flags = raw_regex.substring_from((last_slash + 1) as usize);
        self.validate_reg_exp_flags(tree, &flags);
        if !flags.is_empty() {
            let flags_node = self.new_string_literal_node(ast, flags);
            self.set_source_info_token(ast, flags_node, token);
            node.add_child_to_back(ast, flags_node);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#validateRegExpFlags
    fn validate_reg_exp_flags(&mut self, tree: &ParseTree, flags: &JsString) {
        for &flag in flags.as_units() {
            match flag {
                c if c == u16::from(b'g') || c == u16::from(b'i') || c == u16::from(b'm') => {}
                c if c == u16::from(b'u') || c == u16::from(b'y') => self.maybe_warn_for_feature(
                    tree,
                    if flag == u16::from(b'u') {
                        Feature::REGEXP_FLAG_U
                    } else {
                        Feature::REGEXP_FLAG_Y
                    },
                ),
                c if c == u16::from(b's') => {
                    self.maybe_warn_for_feature(tree, Feature::REGEXP_FLAG_S)
                }
                c if c == u16::from(b'd') => {
                    self.maybe_warn_for_feature(tree, Feature::REGEXP_FLAG_D)
                }
                _ => self.error_reporter.error_js_string(
                    &JsString::from("Invalid RegExp flag '")
                        .concat(&JsString::from_units(vec![flag]))
                        .concat(&"'".into()),
                    &self.source_name,
                    Self::lineno(tree),
                    Self::charno(tree),
                ),
            }
        }
    }
    // port: IRFactory.TransformDispatcher#processReturnStatement
    fn process_return_statement(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let node = self.new_node(ast, Token::RETURN);
        if let Some(expression) = &tree.as_return_statement().expression {
            let n = self.transform(ast, expression)?;
            node.add_child_to_back(ast, n);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processStringLiteral
    fn process_string_literal(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.process_string(ast, &tree.as_literal_expression().literal_token)
    }
    // port: IRFactory.TransformDispatcher#processTemplateLiteral
    fn process_template_literal(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::TEMPLATE_LITERALS);
        let template_lit_node = self.new_node(ast, Token::TEMPLATELIT);
        self.set_source_info(ast, template_lit_node, tree);
        let tree = tree.as_template_literal_expression();
        let node = if let Some(operand) = &tree.operand {
            let operand = self.transform(ast, operand)?;
            self.new_node2(ast, Token::TAGGED_TEMPLATELIT, operand, template_lit_node)
        } else {
            template_lit_node
        };
        for child in &tree.elements {
            let child = self.transform(ast, child)?;
            template_lit_node.add_child_to_back(ast, child);
        }
        if node.is_tagged_template_lit(ast) {
            Self::annotate_calls(ast, node);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processTemplateLiteralPortion
    fn process_template_literal_portion(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.process_template_literal_token(ast, &tree.as_template_literal_portion().value)
    }
    // port: IRFactory.TransformDispatcher#processTemplateSubstitution
    fn process_template_substitution(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let expression = self.transform(ast, &tree.as_template_substitution().expression)?;
        Ok(self.new_node1(ast, Token::TEMPLATELIT_SUB, expression))
    }
    // port: IRFactory.TransformDispatcher#processSwitchCase
    fn process_switch_case(&mut self, ast: &mut Ast, case_node: &ParseTree) -> P<NodeId> {
        let tree = case_node.as_case_clause();
        let expr = self.transform(ast, &tree.expression)?;
        let node = self.new_node1(ast, Token::CASE, expr);
        let block = self.new_node(ast, Token::BLOCK);
        block.set_is_added_block(ast, true);
        self.set_source_info(ast, block, case_node);
        for child in &tree.statements {
            let n = self.transform(ast, child)?;
            block.add_child_to_back(ast, n);
        }
        node.add_child_to_back(ast, block);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processSwitchDefault
    fn process_switch_default(&mut self, ast: &mut Ast, case_node: &ParseTree) -> P<NodeId> {
        let node = self.new_node(ast, Token::DEFAULT_CASE);
        let block = self.new_node(ast, Token::BLOCK);
        block.set_is_added_block(ast, true);
        self.set_source_info(ast, block, case_node);
        for child in &case_node.as_default_clause().statements {
            let n = self.transform(ast, child)?;
            block.add_child_to_back(ast, n);
        }
        node.add_child_to_back(ast, block);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processSwitchStatement
    fn process_switch_statement(&mut self, ast: &mut Ast, statement_node: &ParseTree) -> P<NodeId> {
        let tree = statement_node.as_switch_statement();
        let expression = self.transform(ast, &tree.expression)?;
        let node = self.new_node1(ast, Token::SWITCH, expression);
        let switch_body = self.new_node(ast, Token::SWITCH_BODY);
        self.set_source_info(ast, switch_body, statement_node);
        node.add_child_to_back(ast, switch_body);
        for child in &tree.case_clauses {
            let n = self.transform(ast, child)?;
            switch_body.add_child_to_back(ast, n);
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processThrowStatement
    fn process_throw_statement(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let value = self.transform(ast, required(tree.as_throw_statement().value.as_deref())?)?;
        Ok(self.new_node1(ast, Token::THROW, value))
    }
    // port: IRFactory.TransformDispatcher#processTryStatement
    fn process_try_statement(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_try_statement();
        let body = self.transform_block(ast, &tree.body)?;
        let node = self.new_node1(ast, Token::TRY, body);
        let block = self.new_node(ast, Token::BLOCK);
        let mut line_set = false;
        if let Some(cc) = &tree.catch_block {
            self.set_source_info(ast, block, cc);
            line_set = true;
            let cc = self.transform(ast, cc)?;
            block.add_child_to_back(ast, cc);
        }
        node.add_child_to_back(ast, block);
        if let Some(finally_block) = &tree.finally_block {
            let n = self.transform_block(ast, finally_block)?;
            node.add_child_to_back(ast, n);
        }
        if !line_set {
            if let Some(finally_block) = &tree.finally_block {
                self.set_source_info(ast, block, finally_block);
            }
        }
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processCatchClause
    fn process_catch_clause(&mut self, ast: &mut Ast, clause_node: &ParseTree) -> P<NodeId> {
        let tree = clause_node.as_catch();
        let exception = required(tree.exception.as_deref())?;
        if exception.type_ == ParseTreeType::EMPTY_STATEMENT {
            self.maybe_warn_for_feature(clause_node, Feature::OPTIONAL_CATCH_BINDING);
        }
        let exception = self.transform(ast, exception)?;
        let body = self.transform_block(ast, &tree.catch_body)?;
        Ok(self.new_node2(ast, Token::CATCH, exception, body))
    }
    // port: IRFactory.TransformDispatcher#processFinally
    fn process_finally(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.transform_block(ast, &tree.as_finally().block)
    }
    // port: IRFactory.TransformDispatcher#processUnaryExpression
    fn process_unary_expression(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_unary_expression();
        let type_ = Self::transform_unary_token_type(tree.operator.type_);
        let operand = self.transform(ast, &tree.operand)?;
        match type_ {
            Token::DELPROP => {
                if !(operand.is_get_prop(ast)
                    || operand.is_get_elem(ast)
                    || operand.is_name(ast)
                    || operand.is_opt_chain_get_prop(ast)
                    || operand.is_opt_chain_get_elem(ast))
                {
                    self.error_reporter.error(
                        "Invalid delete operand. Only properties can be deleted.",
                        &self.source_name,
                        operand.get_lineno(ast),
                        0,
                    );
                }
            }
            Token::POS => {
                if operand.is_big_int(ast) {
                    self.error_reporter.error(
                        "Cannot convert a BigInt value to a number",
                        &self.source_name,
                        operand.get_lineno(ast),
                        0,
                    );
                }
            }
            _ => {}
        }
        Ok(self.new_node1(ast, type_, operand))
    }
    // port: IRFactory.TransformDispatcher#processUpdateExpression
    fn process_update_expression(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_update_expression();
        let type_ = Self::transform_update_token_type(tree.operator.type_);
        let operand = self.transform(ast, &tree.operand)?;
        Ok(self.create_update_node(
            ast,
            type_,
            tree.operator_position == update_expression_tree::OperatorPosition::POSTFIX,
            operand,
        ))
    }
    // port: IRFactory.TransformDispatcher#createUpdateNode
    fn create_update_node(
        &mut self,
        ast: &mut Ast,
        type_: Token,
        postfix: bool,
        operand: NodeId,
    ) -> NodeId {
        let assign_target = if operand.is_cast(ast) {
            operand.get_first_child(ast).unwrap()
        } else {
            operand
        };
        if !assign_target.is_valid_assignment_target(ast) {
            let position = if postfix { "postfix" } else { "prefix" };
            let operation = if type_ == Token::INC {
                "increment"
            } else {
                "decrement"
            };
            self.error_reporter.error(
                &format!("Invalid {position} {operation} operand."),
                &self.source_name,
                operand.get_lineno(ast),
                operand.get_charno(ast),
            );
        }
        let node = self.new_node1(ast, type_, operand);
        node.put_boolean_prop(ast, NodeId::INCRDECR_PROP, postfix);
        node
    }
    // port: IRFactory.TransformDispatcher#processVariableStatement
    fn process_variable_statement(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.process(ast, &tree.as_variable_statement().declarations)
    }
    // port: IRFactory.TransformDispatcher#processVariableDeclarationList
    fn process_variable_declaration_list(&mut self, ast: &mut Ast, decl: &ParseTree) -> P<NodeId> {
        let tree = decl.as_variable_declaration_list();
        let decl_type = match tree.declaration_type {
            TokenType::CONST => {
                self.maybe_warn_for_feature(decl, Feature::CONST_DECLARATIONS);
                Token::CONST
            }
            TokenType::LET => {
                self.maybe_warn_for_feature(decl, Feature::LET_DECLARATIONS);
                Token::LET
            }
            TokenType::VAR => Token::VAR,
            _ => {
                return Err(ParseError {
                    class: "java.lang.IllegalStateException",
                    message: None,
                });
            }
        };
        let node = self.new_node(ast, decl_type);
        for child in &tree.declarations {
            let c = self.transform_node_with_inline_comments(ast, child)?;
            node.add_child_to_back(ast, c);
        }
        self.attach_possible_trailing_comment(ast, node, &decl.get_end());
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processVariableDeclaration
    fn process_variable_declaration(&mut self, ast: &mut Ast, decl: &ParseTree) -> P<NodeId> {
        let tree = decl.as_variable_declaration();
        let node = self.transform_node_with_inline_comments(ast, &tree.lvalue)?;
        let lhs = if node.is_destructuring_pattern(ast) {
            self.new_node1(ast, Token::DESTRUCTURING_LHS, node)
        } else {
            node
        };
        if let Some(initializer) = &tree.initializer {
            let initializer = self.transform(ast, initializer)?;
            lhs.add_child_to_back(ast, initializer);
            self.set_length(ast, lhs, &decl.location.start, &decl.location.end);
        }
        Ok(lhs)
    }
    // port: IRFactory.TransformDispatcher#processWhileLoop
    fn process_while_loop(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_while_statement();
        let condition = self.transform(ast, &tree.condition)?;
        let body = self.transform_block(ast, &tree.body)?;
        Ok(self.new_node2(ast, Token::WHILE, condition, body))
    }
    // port: IRFactory.TransformDispatcher#processWithStatement
    fn process_with_statement(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_with_statement();
        let expression = self.transform(ast, &tree.expression)?;
        let body = self.transform_block(ast, &tree.body)?;
        Ok(self.new_node2(ast, Token::WITH, expression, body))
    }
    // port: IRFactory.TransformDispatcher#processMissingExpression
    fn process_missing_expression(&mut self, ast: &mut Ast, _tree: &ParseTree) -> P<NodeId> {
        Ok(self.create_missing_expression_node(ast))
    }
    // port: IRFactory.TransformDispatcher#createMissingNameNode
    fn create_missing_name_node(&self, ast: &mut Ast) -> NodeId {
        self.new_string_node(ast, Token::NAME, "__missing_name__")
    }
    // port: IRFactory.TransformDispatcher#createMissingExpressionNode
    fn create_missing_expression_node(&self, ast: &mut Ast) -> NodeId {
        self.new_string_node(ast, Token::NAME, "__missing_expression__")
    }
    // port: IRFactory.TransformDispatcher#processIllegalToken
    fn process_illegal_token(&mut self, ast: &mut Ast, node: &ParseTree) -> P<NodeId> {
        self.error_reporter.error(
            &format!("Unsupported syntax: {:?}", node.type_),
            &self.source_name,
            Self::lineno(node),
            0,
        );
        Ok(self.new_node(ast, Token::EMPTY))
    }
    // port: IRFactory.TransformDispatcher#maybeReportGetter
    fn maybe_report_getter(&mut self, node: &ParseTree) -> bool {
        self.features = self.features.with(Feature::GETTER);
        if self.config.language_mode() == LanguageMode::ECMASCRIPT3 {
            self.error_reporter.error(
                GETTER_ERROR_MESSAGE,
                &self.source_name,
                Self::lineno(node),
                0,
            );
            true
        } else {
            false
        }
    }
    // port: IRFactory.TransformDispatcher#maybeReportSetter
    fn maybe_report_setter(&mut self, node: &ParseTree) -> bool {
        self.features = self.features.with(Feature::SETTER);
        if self.config.language_mode() == LanguageMode::ECMASCRIPT3 {
            self.error_reporter.error(
                SETTER_ERROR_MESSAGE,
                &self.source_name,
                Self::lineno(node),
                0,
            );
            true
        } else {
            false
        }
    }
    // port: IRFactory.TransformDispatcher#processBooleanLiteral
    fn process_boolean_literal(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        Ok(self.new_node(
            ast,
            Self::transform_boolean_token_type(tree.as_literal_expression().literal_token.type_),
        ))
    }
    // port: IRFactory.TransformDispatcher#processNullLiteral
    fn process_null_literal(&mut self, ast: &mut Ast, _tree: &ParseTree) -> P<NodeId> {
        Ok(self.new_node(ast, Token::NULL))
    }
    // port: IRFactory.TransformDispatcher#processNull
    fn process_null(&mut self, ast: &mut Ast, _tree: &ParseTree) -> P<NodeId> {
        Ok(self.new_node(ast, Token::EMPTY))
    }
    // port: IRFactory.TransformDispatcher#processCommaExpression
    fn process_comma_expression(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_comma_expression();
        let mut root = self.new_node(ast, Token::COMMA);
        let start = &tree.expressions[0].location.start;
        let mut end = &tree.expressions[1].location.end;
        self.set_source_info_position(ast, root, start, end);
        for expr in &tree.expressions {
            let count = root.get_child_count(ast);
            if count < 2 {
                let n = self.transform(ast, expr)?;
                root.add_child_to_back(ast, n);
            } else {
                end = &expr.location.end;
                let n = self.transform(ast, expr)?;
                root = self.new_node2(ast, Token::COMMA, root, n);
                self.set_source_info_position(ast, root, start, end);
            }
        }
        Ok(root)
    }
    // port: IRFactory.TransformDispatcher#processClassDeclaration
    fn process_class_declaration(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::CLASSES);
        let class = tree.as_class_declaration();
        let name = self.transform_or_empty_token(ast, class.name.as_ref(), tree)?;
        let super_class = self.transform_or_empty(ast, class.super_class.as_deref(), tree)?;
        let class_members = self.new_node(ast, Token::CLASS_MEMBERS);
        self.set_source_info(ast, class_members, tree);
        {
            let _decrement = self.transform_dispatcher.class_scope.increment();
            let mut has_constructor = false;
            for child in &class.elements {
                match child.type_ {
                    ParseTreeType::COMPUTED_PROPERTY_GETTER
                    | ParseTreeType::COMPUTED_PROPERTY_SETTER
                    | ParseTreeType::GET_ACCESSOR
                    | ParseTreeType::SET_ACCESSOR => {
                        self.features = self.features.with(Feature::CLASS_GETTER_SETTER)
                    }
                    ParseTreeType::BLOCK => {
                        self.features = self.features.with(Feature::CLASS_STATIC_BLOCK)
                    }
                    _ => {}
                }
                let child_is_ctor = self.validate_class_constructor_member(child)?;
                if child_is_ctor {
                    if has_constructor {
                        self.error_reporter.error(
                            "Class may have only one constructor.",
                            &self.source_name,
                            Self::lineno(child),
                            Self::charno(child),
                        );
                    }
                    has_constructor = true;
                }
                let n = self.transform(ast, child)?;
                class_members.add_child_to_back(ast, n);
            }
        }
        if !self.transform_dispatcher.class_scope.in_scope() {
            let names = self.get_private_props_and_validate_uniqueness(ast, class_members);
            self.validate_private_property_usage(ast, &names, class_members);
        }
        let class_node = self.new_node3(ast, Token::CLASS, name, super_class, class_members);
        self.attach_possible_trailing_comment(ast, class_node, &tree.get_end());
        Ok(class_node)
    }
    // port: IRFactory.TransformDispatcher#validatePrivatePropertyUsage
    fn validate_private_property_usage(
        &mut self,
        ast: &Ast,
        private_prop_names: &BTreeSet<JsString>,
        class_members: NodeId,
    ) {
        check_state!(class_members.is_class_members(ast));
        let Some(first) = class_members.get_first_child(ast) else {
            return;
        };
        let mut visit_stack = VecDeque::new();
        visit_stack.push_back(first);
        while let Some(node) = visit_stack.pop_back() {
            if node.is_private_identifier(ast)
                && (node.is_get_prop(ast)
                    || node.is_opt_chain_get_prop(ast)
                    || (node.is_name(ast) && node.get_parent(ast).is_some_and(|p| p.is_in(ast))))
                && !private_prop_names.contains(&node.get_string(ast))
            {
                let prop_type = if node.get_parent(ast).unwrap().is_call(ast) {
                    "methods"
                } else {
                    "fields"
                };
                self.error_reporter.error(
                    &format!("Private {prop_type} must be declared in an enclosing class"),
                    &self.source_name,
                    node.get_lineno(ast),
                    node.get_charno(ast),
                );
            }
            if node.is_del_prop(ast) {
                let first_child = check_not_null!(node.get_first_child(ast));
                if first_child.is_private_identifier(ast)
                    && (first_child.is_get_prop(ast) || first_child.is_opt_chain_get_prop(ast))
                {
                    self.error_reporter.error(
                        "Private fields cannot be deleted",
                        &self.source_name,
                        node.get_lineno(ast),
                        node.get_charno(ast),
                    );
                }
            }
            if node.is_class_members(ast) {
                let mut inner_names = self.get_private_props_and_validate_uniqueness(ast, node);
                inner_names.extend(private_prop_names.iter().cloned());
                self.validate_private_property_usage(ast, &inner_names, node);
            } else {
                if let Some(next) = node.get_next(ast) {
                    visit_stack.push_back(next);
                }
                if let Some(first) = node.get_first_child(ast) {
                    visit_stack.push_back(first);
                }
            }
        }
    }
    // port: IRFactory.TransformDispatcher#getPrivatePropsAndValidateUniqueness
    fn get_private_props_and_validate_uniqueness(
        &mut self,
        ast: &Ast,
        class_members: NodeId,
    ) -> BTreeSet<JsString> {
        let mut private_getter_names = BTreeSet::new();
        let mut private_setter_names = BTreeSet::new();
        let mut private_static_getter_names = BTreeSet::new();
        let mut private_static_setter_names = BTreeSet::new();
        let mut private_field_and_method_names = BTreeSet::new();
        let mut private_prop_names = BTreeSet::new();
        for cur_node in class_members.children(ast) {
            if !cur_node.is_private_identifier(ast) {
                continue;
            }
            let prop_name = cur_node.get_string(ast);
            check_state!(
                cur_node.is_getter_def(ast)
                    || cur_node.is_setter_def(ast)
                    || cur_node.is_member_field_def(ast)
                    || cur_node.is_member_function_def(ast),
                "Private property '%s' has an unsupported token type: %s",
                prop_name,
                cur_node.get_token(ast)
            );
            let is_static = cur_node.is_static_member(ast);
            let mut already_declared = private_prop_names.contains(&prop_name);
            match cur_node.get_token(ast) {
                Token::GETTER_DEF => {
                    if already_declared {
                        let other_than_setter = private_field_and_method_names.contains(&prop_name)
                            || private_getter_names.contains(&prop_name)
                            || private_static_getter_names.contains(&prop_name)
                            || if is_static {
                                private_setter_names.contains(&prop_name)
                            } else {
                                private_static_setter_names.contains(&prop_name)
                            };
                        check_state!(
                            other_than_setter
                                || if is_static {
                                    private_static_setter_names.contains(&prop_name)
                                } else {
                                    private_setter_names.contains(&prop_name)
                                }
                        );
                        already_declared = other_than_setter;
                    }
                    if is_static {
                        private_static_getter_names.insert(prop_name.clone());
                    } else {
                        private_getter_names.insert(prop_name.clone());
                    }
                }
                Token::SETTER_DEF => {
                    if already_declared {
                        let other_than_getter = private_field_and_method_names.contains(&prop_name)
                            || private_setter_names.contains(&prop_name)
                            || private_static_setter_names.contains(&prop_name)
                            || if is_static {
                                private_getter_names.contains(&prop_name)
                            } else {
                                private_static_getter_names.contains(&prop_name)
                            };
                        check_state!(
                            other_than_getter
                                || if is_static {
                                    private_static_getter_names.contains(&prop_name)
                                } else {
                                    private_getter_names.contains(&prop_name)
                                }
                        );
                        already_declared = other_than_getter;
                    }
                    if is_static {
                        private_static_setter_names.insert(prop_name.clone());
                    } else {
                        private_setter_names.insert(prop_name.clone());
                    }
                }
                Token::MEMBER_FIELD_DEF | Token::MEMBER_FUNCTION_DEF => {
                    private_field_and_method_names.insert(prop_name.clone());
                }
                _ => {}
            }
            if already_declared {
                self.error_reporter.error_js_string(
                    &JsString::from("Identifier '")
                        .concat(&prop_name)
                        .concat(&"' has already been declared".into()),
                    &self.source_name,
                    cur_node.get_lineno(ast),
                    cur_node.get_charno(ast),
                );
            }
            private_prop_names.insert(prop_name);
        }
        private_prop_names
    }
    // port: IRFactory.TransformDispatcher#validateClassConstructorMember
    fn validate_class_constructor_member(&mut self, member: &ParseTree) -> P<bool> {
        let (member_name, is_static, has_illegal_modifier) = match &member.data {
            ParseTreeData::GetAccessorTree(t) => (&t.property_name, t.is_static, true),
            ParseTreeData::SetAccessorTree(t) => (&t.property_name, t.is_static, true),
            ParseTreeData::FunctionDeclarationTree(t) => (
                required(t.name.as_ref())?,
                t.is_static,
                t.is_generator || t.is_async,
            ),
            _ => return Ok(false),
        };
        if is_static {
            return Ok(false);
        }
        if member_name.type_ != TokenType::IDENTIFIER
            || !member_name.value_equals(&JsString::from("constructor"))
        {
            return Ok(false);
        }
        if has_illegal_modifier {
            self.error_reporter.error(
                "Class constructor may not be getter, setter, async, or generator.",
                &self.source_name,
                Self::lineno(member),
                Self::charno(member),
            );
            return Ok(false);
        }
        Ok(true)
    }
    // port: IRFactory.TransformDispatcher#processSuper
    fn process_super(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::SUPER);
        Ok(self.new_node(ast, Token::SUPER))
    }
    // port: IRFactory.TransformDispatcher#processNewTarget
    fn process_new_target(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::NEW_TARGET);
        Ok(self.new_node(ast, Token::NEW_TARGET))
    }
    // port: IRFactory.TransformDispatcher#processYield
    fn process_yield(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_yield_statement();
        let n = self.new_node(ast, Token::YIELD);
        if let Some(expression) = &tree.expression {
            let n2 = self.transform(ast, expression)?;
            n.add_child_to_back(ast, n2);
        }
        n.set_yield_all(ast, tree.is_yield_all);
        Ok(n)
    }
    // port: IRFactory.TransformDispatcher#processAwait
    fn process_await(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::ASYNC_FUNCTIONS);
        let n = self.new_node(ast, Token::AWAIT);
        let expression = self.transform(ast, &tree.as_await_expression().expression)?;
        n.add_child_to_back(ast, expression);
        Ok(n)
    }
    // port: IRFactory.TransformDispatcher#processExportDecl
    fn process_export_decl(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::MODULES);
        let export = tree.as_export_declaration();
        let decls = if export.is_export_all {
            check_state!(export.declaration.is_none() && export.export_specifier_list.is_none());
            self.new_node(ast, Token::EMPTY)
        } else if let Some(declaration) = &export.declaration {
            check_state!(export.export_specifier_list.is_none());
            self.transform(ast, declaration)?
        } else {
            self.transform_list(
                ast,
                Token::EXPORT_SPECS,
                required(export.export_specifier_list.as_deref())?,
            )?
        };
        self.set_source_info(ast, decls, tree);
        let node = self.new_node1(ast, Token::EXPORT, decls);
        if let Some(from) = &export.from {
            let from = self.process_string(ast, from)?;
            node.add_child_to_back(ast, from);
        }
        node.put_boolean_prop(ast, NodeId::EXPORT_ALL_FROM, export.is_export_all);
        node.put_boolean_prop(ast, NodeId::EXPORT_DEFAULT, export.is_default);
        Ok(node)
    }
    // port: IRFactory.TransformDispatcher#processExportSpec
    fn process_export_spec(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_export_specifier();
        let imported_name = self.process_name(
            ast,
            &tree.imported_name,
            Token::NAME,
            IdentifierType::STANDARD,
        )?;
        let export_spec = self.new_node1(ast, Token::EXPORT_SPEC, imported_name);
        if let Some(destination) = &tree.destination_name {
            let destination =
                self.process_name(ast, destination, Token::NAME, IdentifierType::STANDARD)?;
            export_spec.add_child_to_back(ast, destination);
        } else {
            export_spec.set_shorthand_property(ast, true);
            let cloned = imported_name.clone_tree(ast);
            export_spec.add_child_to_back(ast, cloned);
        }
        Ok(export_spec)
    }
    // port: IRFactory.TransformDispatcher#processImportDecl
    fn process_import_decl(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::MODULES);
        let import = tree.as_import_declaration();
        let first_child =
            self.transform_or_empty_token(ast, import.default_binding_identifier.as_ref(), tree)?;
        let second_child = if let Some(identifier) = &import.name_space_import_identifier {
            let n = self.new_string_node_from_identifier(
                ast,
                Token::IMPORT_STAR,
                IdentifierType::STANDARD,
                identifier,
            );
            self.set_source_info_token(ast, n, identifier);
            n
        } else {
            let n = self.transform_list_or_empty(
                ast,
                Token::IMPORT_SPECS,
                import.import_specifier_list.as_deref(),
            )?;
            self.set_source_info(ast, n, tree);
            n
        };
        let third_child = self.process_string(ast, required(import.module_specifier.as_ref())?)?;
        Ok(self.new_node3(ast, Token::IMPORT, first_child, second_child, third_child))
    }
    // port: IRFactory.TransformDispatcher#processImportSpec
    fn process_import_spec(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        let tree = tree.as_import_specifier();
        let imported_name = self.process_name(
            ast,
            &tree.imported_name,
            Token::NAME,
            IdentifierType::STANDARD,
        )?;
        let import_spec = self.new_node1(ast, Token::IMPORT_SPEC, imported_name);
        if let Some(destination) = &tree.destination_name {
            let destination =
                self.process_name(ast, destination, Token::NAME, IdentifierType::STANDARD)?;
            import_spec.add_child_to_back(ast, destination);
        } else {
            import_spec.set_shorthand_property(ast, true);
            let cloned = imported_name.clone_tree(ast);
            import_spec.add_child_to_back(ast, cloned);
        }
        Ok(import_spec)
    }
    // port: IRFactory.TransformDispatcher#processDynamicImport
    fn process_dynamic_import(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::DYNAMIC_IMPORT);
        let argument = self.transform(ast, &tree.as_dynamic_import_expression().argument)?;
        Ok(self.new_node1(ast, Token::DYNAMIC_IMPORT, argument))
    }
    // port: IRFactory.TransformDispatcher#processImportMeta
    fn process_import_meta(&mut self, ast: &mut Ast, tree: &ParseTree) -> P<NodeId> {
        self.maybe_warn_for_feature(tree, Feature::MODULES);
        self.maybe_warn_for_feature(tree, Feature::IMPORT_META);
        Ok(self.new_node(ast, Token::IMPORT_META))
    }
    // port: IRFactory.TransformDispatcher#checkParameters
    fn check_parameters(&mut self, tree: &ParseTree) -> bool {
        let params = tree.as_formal_parameter_list();
        let mut good = true;
        for (i, param) in params.parameters.iter().enumerate() {
            if param.type_ == ParseTreeType::ITER_REST {
                if i != params.parameters.len() - 1 {
                    self.error_reporter.error(
                        "A rest parameter must be last in a parameter list.",
                        &self.source_name,
                        Self::lineno(param),
                        Self::charno(param),
                    );
                    good = false;
                } else if params.has_trailing_comma {
                    self.error_reporter.error(
                        "A trailing comma must not follow a rest parameter.",
                        &self.source_name,
                        Self::lineno(param),
                        Self::charno(param),
                    );
                    good = false;
                }
            }
        }
        good
    }
    // port: IRFactory.TransformDispatcher#transformList
    fn transform_list(&mut self, ast: &mut Ast, type_: Token, list: &[Tree]) -> P<NodeId> {
        let n = self.new_node(ast, type_);
        for tree in list {
            let c = self.transform(ast, tree)?;
            n.add_child_to_back(ast, c);
        }
        Ok(n)
    }
    // port: IRFactory.TransformDispatcher#transformListOrEmpty
    fn transform_list_or_empty(
        &mut self,
        ast: &mut Ast,
        type_: Token,
        list: Option<&[Tree]>,
    ) -> P<NodeId> {
        if list.is_none_or(|l| l.is_empty()) {
            Ok(self.new_node(ast, Token::EMPTY))
        } else {
            self.transform_list(ast, type_, list.unwrap())
        }
    }
    // port: IRFactory.TransformDispatcher#unsupportedLanguageFeature
    fn unsupported_language_feature(
        &mut self,
        ast: &mut Ast,
        node: &ParseTree,
        feature: &str,
    ) -> P<NodeId> {
        self.error_reporter.error(
            &format!("unsupported language feature: {feature}"),
            &self.source_name,
            Self::lineno(node),
            Self::charno(node),
        );
        Ok(self.create_missing_expression_node(ast))
    }
    // port: IRFactory.TransformDispatcher#processLiteralExpression
    fn process_literal_expression(&mut self, ast: &mut Ast, expr: &ParseTree) -> P<NodeId> {
        match expr.as_literal_expression().literal_token.type_ {
            TokenType::NUMBER => self.process_number_literal(ast, expr), TokenType::STRING => self.process_string_literal(ast, expr), TokenType::BIGINT => self.process_big_int_literal(ast, expr),
            TokenType::FALSE | TokenType::TRUE => self.process_boolean_literal(ast, expr), TokenType::NULL => self.process_null_literal(ast, expr), TokenType::REGULAR_EXPRESSION => self.process_reg_exp_literal(ast, expr),
            type_ => Err(ParseError { class: "java.lang.IllegalStateException", message: Some(format!("Unexpected literal type: class com.google.javascript.jscomp.parsing.parser.Token type: {type_}").into()) }),
        }
    }
    // port: IRFactory.TransformDispatcher#process
    fn process(&mut self, ast: &mut Ast, node: &ParseTree) -> P<NodeId> {
        match node.type_ {
            ParseTreeType::BINARY_OPERATOR => self.process_binary_expression(ast, node),
            ParseTreeType::ARRAY_LITERAL_EXPRESSION => self.process_array_literal(ast, node),
            ParseTreeType::TEMPLATE_LITERAL_EXPRESSION => self.process_template_literal(ast, node),
            ParseTreeType::TEMPLATE_LITERAL_PORTION => {
                self.process_template_literal_portion(ast, node)
            }
            ParseTreeType::TEMPLATE_SUBSTITUTION => self.process_template_substitution(ast, node),
            ParseTreeType::UNARY_EXPRESSION => self.process_unary_expression(ast, node),
            ParseTreeType::BLOCK => self.process_block(ast, node),
            ParseTreeType::BREAK_STATEMENT => self.process_break_statement(ast, node),
            ParseTreeType::CALL_EXPRESSION => self.process_function_call(ast, node),
            ParseTreeType::OPT_CHAIN_CALL_EXPRESSION => {
                self.process_opt_chain_function_call(ast, node)
            }
            ParseTreeType::CASE_CLAUSE => self.process_switch_case(ast, node),
            ParseTreeType::DEFAULT_CLAUSE => self.process_switch_default(ast, node),
            ParseTreeType::CATCH => self.process_catch_clause(ast, node),
            ParseTreeType::CONTINUE_STATEMENT => self.process_continue_statement(ast, node),
            ParseTreeType::DO_WHILE_STATEMENT => self.process_do_loop(ast, node),
            ParseTreeType::EMPTY_STATEMENT => self.process_empty_statement(ast, node),
            ParseTreeType::EXPRESSION_STATEMENT => self.process_expression_statement(ast, node),
            ParseTreeType::DEBUGGER_STATEMENT => self.process_debugger_statement(ast, node),
            ParseTreeType::THIS_EXPRESSION => self.process_this_expression(ast, node),
            ParseTreeType::FOR_STATEMENT => self.process_for_loop(ast, node),
            ParseTreeType::FOR_IN_STATEMENT => self.process_for_in_loop(ast, node),
            ParseTreeType::FUNCTION_DECLARATION => self.process_function(ast, node),
            ParseTreeType::MEMBER_LOOKUP_EXPRESSION => self.process_element_get(ast, node),
            ParseTreeType::OPT_CHAIN_MEMBER_LOOKUP_EXPRESSION => {
                self.process_opt_chain_element_get(ast, node)
            }
            ParseTreeType::MEMBER_EXPRESSION => self.process_property_get(ast, node),
            ParseTreeType::OPT_CHAIN_MEMBER_EXPRESSION => {
                self.process_opt_chain_property_get(ast, node)
            }
            ParseTreeType::CONDITIONAL_EXPRESSION => self.process_conditional_expression(ast, node),
            ParseTreeType::IF_STATEMENT => self.process_if_statement(ast, node),
            ParseTreeType::LABELLED_STATEMENT => self.process_labeled_statement(ast, node),
            ParseTreeType::PAREN_EXPRESSION => self.process_parenthesized_expression(ast, node),
            ParseTreeType::IDENTIFIER_EXPRESSION => self.process_name_tree(
                ast,
                node,
                if self
                    .transform_dispatcher
                    .private_id_lhs_of_in_scope
                    .in_scope()
                {
                    IdentifierType::CAN_BE_PRIVATE
                } else {
                    IdentifierType::STANDARD
                },
            ),
            ParseTreeType::NEW_EXPRESSION => self.process_new_expression(ast, node),
            ParseTreeType::OBJECT_LITERAL_EXPRESSION => self.process_object_literal(ast, node),
            ParseTreeType::COMPUTED_PROPERTY_DEFINITION => {
                self.process_computed_property_definition(ast, node)
            }
            ParseTreeType::COMPUTED_PROPERTY_GETTER => {
                self.process_computed_property_getter(ast, node)
            }
            ParseTreeType::COMPUTED_PROPERTY_METHOD => {
                self.process_computed_property_method(ast, node)
            }
            ParseTreeType::COMPUTED_PROPERTY_SETTER => {
                self.process_computed_property_setter(ast, node)
            }
            ParseTreeType::RETURN_STATEMENT => self.process_return_statement(ast, node),
            ParseTreeType::UPDATE_EXPRESSION => self.process_update_expression(ast, node),
            ParseTreeType::PROGRAM => self.process_ast_root(ast, node),
            ParseTreeType::LITERAL_EXPRESSION => self.process_literal_expression(ast, node),
            ParseTreeType::SWITCH_STATEMENT => self.process_switch_statement(ast, node),
            ParseTreeType::THROW_STATEMENT => self.process_throw_statement(ast, node),
            ParseTreeType::TRY_STATEMENT => self.process_try_statement(ast, node),
            ParseTreeType::VARIABLE_STATEMENT => self.process_variable_statement(ast, node),
            ParseTreeType::VARIABLE_DECLARATION_LIST => {
                self.process_variable_declaration_list(ast, node)
            }
            ParseTreeType::VARIABLE_DECLARATION => self.process_variable_declaration(ast, node),
            ParseTreeType::WHILE_STATEMENT => self.process_while_loop(ast, node),
            ParseTreeType::WITH_STATEMENT => self.process_with_statement(ast, node),
            ParseTreeType::COMMA_EXPRESSION => self.process_comma_expression(ast, node),
            ParseTreeType::NULL => self.process_null(ast, node),
            ParseTreeType::FINALLY => self.process_finally(ast, node),
            ParseTreeType::MISSING_PRIMARY_EXPRESSION => self.process_missing_expression(ast, node),
            ParseTreeType::PROPERTY_NAME_ASSIGNMENT => {
                self.process_property_name_assignment(ast, node)
            }
            ParseTreeType::GET_ACCESSOR => self.process_get_accessor(ast, node),
            ParseTreeType::SET_ACCESSOR => self.process_set_accessor(ast, node),
            ParseTreeType::FORMAL_PARAMETER_LIST => self.process_formal_parameter_list(ast, node),
            ParseTreeType::CLASS_DECLARATION => self.process_class_declaration(ast, node),
            ParseTreeType::SUPER_EXPRESSION => self.process_super(ast, node),
            ParseTreeType::NEW_TARGET_EXPRESSION => self.process_new_target(ast, node),
            ParseTreeType::YIELD_EXPRESSION => self.process_yield(ast, node),
            ParseTreeType::AWAIT_EXPRESSION => self.process_await(ast, node),
            ParseTreeType::FOR_OF_STATEMENT => self.process_for_of(ast, node),
            ParseTreeType::FOR_AWAIT_OF_STATEMENT => self.process_for_await_of(ast, node),
            ParseTreeType::EXPORT_DECLARATION => self.process_export_decl(ast, node),
            ParseTreeType::EXPORT_SPECIFIER => self.process_export_spec(ast, node),
            ParseTreeType::IMPORT_DECLARATION => self.process_import_decl(ast, node),
            ParseTreeType::IMPORT_SPECIFIER => self.process_import_spec(ast, node),
            ParseTreeType::DYNAMIC_IMPORT_EXPRESSION => self.process_dynamic_import(ast, node),
            ParseTreeType::IMPORT_META_EXPRESSION => self.process_import_meta(ast, node),
            ParseTreeType::ARRAY_PATTERN => self.process_array_pattern(ast, node),
            ParseTreeType::OBJECT_PATTERN => self.process_object_pattern(ast, node),
            ParseTreeType::COMPREHENSION => self.process_comprehension(ast, node),
            ParseTreeType::COMPREHENSION_FOR => self.process_comprehension_for(ast, node),
            ParseTreeType::COMPREHENSION_IF => self.process_comprehension_if(ast, node),
            ParseTreeType::DEFAULT_PARAMETER => self.process_default_parameter(ast, node),
            ParseTreeType::ITER_REST => self.process_iter_rest(ast, node),
            ParseTreeType::ITER_SPREAD => self.process_iter_spread(ast, node),
            ParseTreeType::OBJECT_REST => self.process_object_pattern_element(ast, node),
            ParseTreeType::OBJECT_SPREAD => self.process_object_spread(ast, node),
            ParseTreeType::FIELD_DECLARATION => self.process_field(ast, node),
            ParseTreeType::COMPUTED_PROPERTY_FIELD => {
                self.process_computed_property_field(ast, node)
            }
            _ => self.process_illegal_token(ast, node),
        }
    }
    // port: IRFactory#reportErrorIfYieldOrAwaitInDefaultValue
    fn report_error_if_yield_or_await_in_default_value(
        &mut self,
        ast: &Ast,
        default_value_node: NodeId,
    ) {
        if let Some(n) = Self::find_node_type_in_expression(ast, default_value_node, Token::YIELD) {
            self.error_reporter.error(
                "`yield` is illegal in parameter default value.",
                &n.get_source_file_name(ast).unwrap_or_default(),
                n.get_lineno(ast),
                n.get_charno(ast),
            );
        }
        if let Some(n) = Self::find_node_type_in_expression(ast, default_value_node, Token::AWAIT) {
            self.error_reporter.error(
                "`await` is illegal in parameter default value.",
                &n.get_source_file_name(ast).unwrap_or_default(),
                n.get_lineno(ast),
                n.get_charno(ast),
            );
        }
    }
    // port: IRFactory#findNodeTypeInExpression
    fn find_node_type_in_expression(
        ast: &Ast,
        expression_node: NodeId,
        token: Token,
    ) -> Option<NodeId> {
        let mut worklist = VecDeque::new();
        worklist.push_back(expression_node);
        while let Some(node) = worklist.pop_front() {
            if node.get_token(ast) == token {
                return Some(node);
            } else if !node.is_function(ast) && !node.is_class(ast) {
                for child in node.children(ast) {
                    worklist.push_back(child);
                }
            }
        }
        None
    }
    // port: IRFactory#normalizeRegex
    fn normalize_regex(token: &ParserToken) -> JsString {
        let value = &token.as_literal().value;
        let last_slash = value.last_index_of_char(b'/' as u16) as usize;
        let mut cur = value.index_of_char(b'\\' as u16);
        if cur == -1 {
            return value.substring(1, last_slash);
        }
        let mut result = Vec::new();
        let mut start = 1;
        while cur != -1 {
            result.extend_from_slice(&value.as_units()[start..cur as usize]);
            cur += 1;
            let c = value.char_at(cur as usize);
            if "^$\\/.*+?()[]{}|-bBc dDf nkpPrsStuv wWx0123456789"
                .replace(' ', "")
                .encode_utf16()
                .any(|u| u == c)
            {
                result.push(b'\\' as u16);
            }
            result.push(c);
            start = cur as usize + 1;
            cur = value.index_of_from(&JsString::from("\\"), start as i32);
        }
        result.extend_from_slice(&value.as_units()[start..last_slash]);
        JsString::from_units(result)
    }
    // port: IRFactory#normalizeString
    fn normalize_string(&mut self, token: &ParserToken, template_literal: bool) -> P<JsString> {
        let mut value = token.as_literal().value.clone();
        if value.index_of_char(b'\r' as u16) >= 0 {
            let mut replaced = Vec::new();
            let mut i = 0;
            while i < value.length() {
                let c = value.char_at(i);
                if c == b'\r' as u16 {
                    replaced.push(b'\n' as u16);
                    if i + 1 < value.length() && value.char_at(i + 1) == b'\n' as u16 {
                        i += 1;
                    }
                } else {
                    replaced.push(c);
                }
                i += 1;
            }
            value = JsString::from_units(replaced);
        }
        let mut start = if template_literal { 0 } else { 1 };
        let mut cur = value.index_of_char(b'\\' as u16);
        if cur == -1 {
            return Ok(if template_literal {
                value
            } else {
                value.substring(1, value.length() - 1)
            });
        }
        let mut result = Vec::new();
        while cur != -1 {
            result.extend_from_slice(&value.as_units()[start..cur as usize]);
            cur += 1;
            let c = value.char_at(cur as usize);
            match c {
                c if c == u16::from(b'b') => result.push(u16::from(b'\x08')),
                c if c == u16::from(b'f') => result.push(u16::from(b'\x0c')),
                c if c == u16::from(b'n') => result.push(u16::from(b'\n')),
                c if c == u16::from(b'r') => result.push(u16::from(b'\r')),
                c if c == u16::from(b't') => result.push(u16::from(b'\t')),
                c if c == u16::from(b'v') => result.push(u16::from(b'\x0b')),
                c if c == u16::from(b'\n') => {
                    self.maybe_warn_for_feature_token(token, Feature::STRING_CONTINUATION);
                    self.error_reporter.warning(
                        STRING_CONTINUATION_WARNING,
                        &self.source_name,
                        Self::lineno_token(token),
                        Self::charno_token(token),
                    );
                }
                c if (u16::from(b'0')..=u16::from(b'7')).contains(&c) => {
                    let num_digits = if cur as usize + 1 < value.length()
                        && Self::is_octal_digit(value.char_at(cur as usize + 1))
                    {
                        if cur as usize + 2 < value.length()
                            && Self::is_octal_digit(value.char_at(cur as usize + 2))
                        {
                            3
                        } else {
                            2
                        }
                    } else {
                        1
                    };
                    if (self.in_strict_context() || template_literal)
                        && !(c == u16::from(b'0') && num_digits == 1)
                    {
                        self.error_reporter.warning(
                            OCTAL_STRING_LITERAL_WARNING,
                            &self.source_name,
                            Self::lineno_token(token),
                            Self::charno_token(token),
                        );
                    }
                    let digits = &value.as_units()[cur as usize..cur as usize + num_digits];
                    result.push(
                        closure_rhino::java_lang::parse_int(digits, 8)
                            .map_err(number_format_error)? as u16,
                    );
                    cur += num_digits as i32 - 1;
                }
                c if c == u16::from(b'x') => {
                    result.push(
                        (Self::hex_digit(value.char_at(cur as usize + 1)) * 0x10
                            + Self::hex_digit(value.char_at(cur as usize + 2)))
                            as u16,
                    );
                    cur += 2;
                }
                c if c == u16::from(b'u') => {
                    let mut escape_end;
                    let hex_digits;
                    if value.char_at(cur as usize + 1) != u16::from(b'{') {
                        escape_end = cur as usize + 5;
                        hex_digits = value.substring(cur as usize + 1, escape_end);
                    } else {
                        escape_end = cur as usize + 2;
                        while closure_rhino::java_lang::digit(value.char_at(escape_end), 0x10) >= 0
                        {
                            escape_end += 1;
                        }
                        hex_digits = value.substring(cur as usize + 2, escape_end);
                        escape_end += 1;
                    }
                    let code_point_value =
                        closure_rhino::java_lang::parse_int(hex_digits.as_units(), 0x10)
                            .map_err(number_format_error)?;
                    if code_point_value > 0x10ffff {
                        self.error_reporter.error(
                            "Undefined Unicode code-point",
                            &self.source_name,
                            Self::lineno_token(token),
                            Self::charno_token(token),
                        );
                        result.extend("\\u{".encode_utf16());
                        result.extend_from_slice(hex_digits.as_units());
                        result.push(u16::from(b'}'));
                    } else if code_point_value < 0x10000 {
                        result.push(code_point_value as u16);
                    } else {
                        let v = code_point_value - 0x10000;
                        result.push((0xd800 + (v >> 10)) as u16);
                        result.push((0xdc00 + (v & 0x3ff)) as u16);
                    }
                    cur = escape_end as i32 - 1;
                }
                _ => result.push(c),
            }
            start = cur as usize + 1;
            cur = value.index_of_from(&JsString::from("\\"), start as i32);
        }
        result.extend_from_slice(
            &value.as_units()[start..if template_literal {
                value.length()
            } else {
                value.length() - 1
            }],
        );
        Ok(JsString::from_units(result))
    }
    // port: IRFactory#isSupportedForInputLanguageMode
    fn is_supported_for_input_language_mode(&self, feature: Feature) -> bool {
        self.config.language_mode().feature_set().has(feature)
    }
    // port: IRFactory#inStrictContext
    fn in_strict_context(&self) -> bool {
        self.config.strict_mode().is_strict()
    }
    // port: IRFactory#normalizeNumber
    fn normalize_number(&mut self, token: &ParserToken) -> P<f64> {
        let mut value = token.as_literal().value.clone();
        if value.index_of_char(u16::from(b'_')) >= 0 {
            value = self.remove_numeric_separators(value, token);
        }
        let length = value.length();
        check_state!(length > 0);
        check_state!(value.char_at(0) != u16::from(b'-') && value.char_at(0) != u16::from(b'+'));
        if value.char_at(0) == u16::from(b'.') {
            return closure_rhino::java_lang::parse_double(&JsString::from("0").concat(&value))
                .map_err(number_format_error);
        } else if value.char_at(0) == u16::from(b'0') && length > 1 {
            match value.char_at(1) {
                c if c == u16::from(b'.') || c == u16::from(b'e') || c == u16::from(b'E') => {
                    closure_rhino::java_lang::parse_double(&value).map_err(number_format_error)
                }
                c if c == u16::from(b'b') || c == u16::from(b'B') => {
                    self.maybe_warn_for_feature_token(token, Feature::BINARY_LITERALS);
                    let mut v = 0.0;
                    for c in 2..length {
                        v = v * 2.0 + Self::binary_digit(value.char_at(c)) as f64;
                    }
                    Ok(v)
                }
                c if c == u16::from(b'o') || c == u16::from(b'O') => {
                    self.maybe_warn_for_feature_token(token, Feature::OCTAL_LITERALS);
                    let mut v = 0.0;
                    for c in 2..length {
                        v = v * 8.0 + Self::octal_digit(value.char_at(c)) as f64;
                    }
                    Ok(v)
                }
                c if c == u16::from(b'x') || c == u16::from(b'X') => {
                    let mut v = 0.0;
                    for c in 2..length {
                        v = v * 16.0 + Self::hex_digit(value.char_at(c)) as f64;
                    }
                    Ok(v)
                }
                c if (u16::from(b'0')..=u16::from(b'7')).contains(&c) => {
                    let mut v = 0.0;
                    for c in 1..length {
                        let digit = value.char_at(c);
                        if Self::is_octal_digit(digit) {
                            v = v * 8.0 + Self::octal_digit(digit) as f64;
                        } else {
                            self.error_reporter.error(
                                INVALID_OCTAL_DIGIT,
                                &self.source_name,
                                Self::lineno_token(token),
                                Self::charno_token(token),
                            );
                            return Ok(0.0);
                        }
                    }
                    if self.in_strict_context() {
                        self.error_reporter.error(
                            INVALID_ES5_STRICT_OCTAL,
                            &self.source_name,
                            Self::lineno_token(token),
                            Self::charno_token(token),
                        );
                    } else {
                        self.error_reporter.warning(
                            INVALID_ES5_STRICT_OCTAL,
                            &self.source_name,
                            Self::lineno_token(token),
                            Self::charno_token(token),
                        );
                    }
                    Ok(v)
                }
                c if c == u16::from(b'8') || c == u16::from(b'9') => {
                    self.error_reporter.error(
                        INVALID_OCTAL_DIGIT,
                        &self.source_name,
                        Self::lineno_token(token),
                        Self::charno_token(token),
                    );
                    Ok(0.0)
                }
                c => Err(ParseError {
                    class: "java.lang.IllegalStateException",
                    message: Some(
                        JsString::from("Unexpected character in number literal: ")
                            .concat(&JsString::from_units(vec![c])),
                    ),
                }),
            }
        } else {
            closure_rhino::java_lang::parse_double(&value).map_err(number_format_error)
        }
    }
    // port: IRFactory#normalizeBigInt
    fn normalize_big_int(&mut self, token: &ParserToken) -> P<num_bigint::BigInt> {
        let raw = &token.as_literal().value;
        let mut value = raw.substring(0, raw.index_of_char(u16::from(b'n')) as usize);
        if value.index_of_char(u16::from(b'_')) >= 0 {
            value = self.remove_numeric_separators(value, token);
        }
        let length = value.length();
        check_state!(length > 0);
        check_state!(value.char_at(0) != u16::from(b'-') && value.char_at(0) != u16::from(b'+'));
        if value.char_at(0) == u16::from(b'0') && length > 1 {
            match value.char_at(1) {
                c if c == u16::from(b'b') || c == u16::from(b'B') => {
                    self.maybe_warn_for_feature_token(token, Feature::BINARY_LITERALS);
                    closure_rhino::java_lang::parse_big_integer(&value.substring_from(2), 2)
                        .map_err(big_integer_error)
                }
                c if c == u16::from(b'o') || c == u16::from(b'O') => {
                    self.maybe_warn_for_feature_token(token, Feature::OCTAL_LITERALS);
                    closure_rhino::java_lang::parse_big_integer(&value.substring_from(2), 8)
                        .map_err(big_integer_error)
                }
                c if c == u16::from(b'x') || c == u16::from(b'X') => {
                    closure_rhino::java_lang::parse_big_integer(&value.substring_from(2), 16)
                        .map_err(big_integer_error)
                }
                c if (u16::from(b'0')..=u16::from(b'9')).contains(&c) => Err(ParseError {
                    class: "java.lang.IllegalStateException",
                    message: Some("Nonzero BigInts can't have a leading zero".into()),
                }),
                c => Err(ParseError {
                    class: "java.lang.IllegalStateException",
                    message: Some(
                        JsString::from("Unexpected character in bigint literal: ")
                            .concat(&JsString::from_units(vec![c])),
                    ),
                }),
            }
        } else {
            closure_rhino::java_lang::parse_big_integer(&value, 10).map_err(big_integer_error)
        }
    }
    // port: IRFactory#removeNumericSeparators
    fn remove_numeric_separators(&mut self, value: JsString, token: &ParserToken) -> JsString {
        self.maybe_warn_for_feature_token(token, Feature::NUMERIC_SEPARATOR);
        JsString::from_units(
            value
                .as_units()
                .iter()
                .copied()
                .filter(|&u| u != u16::from(b'_'))
                .collect::<Vec<_>>(),
        )
    }
    // port: IRFactory#binarydigit
    fn binary_digit(c: u16) -> i32 {
        if (u16::from(b'0')..=u16::from(b'1')).contains(&c) {
            i32::from(c - u16::from(b'0'))
        } else {
            panic!("unexpected: {}", JsString::from_units(vec![c]));
        }
    }
    // port: IRFactory#isOctalDigit
    fn is_octal_digit(c: u16) -> bool {
        (u16::from(b'0')..=u16::from(b'7')).contains(&c)
    }
    // port: IRFactory#octaldigit
    fn octal_digit(c: u16) -> i32 {
        if Self::is_octal_digit(c) {
            i32::from(c - u16::from(b'0'))
        } else {
            panic!("unexpected: {}", JsString::from_units(vec![c]));
        }
    }
    // port: IRFactory#hexdigit
    fn hex_digit(c: u16) -> i32 {
        match u8::try_from(c) {
            Ok(b'0') => 0,
            Ok(b'1') => 1,
            Ok(b'2') => 2,
            Ok(b'3') => 3,
            Ok(b'4') => 4,
            Ok(b'5') => 5,
            Ok(b'6') => 6,
            Ok(b'7') => 7,
            Ok(b'8') => 8,
            Ok(b'9') => 9,
            Ok(b'a' | b'A') => 10,
            Ok(b'b' | b'B') => 11,
            Ok(b'c' | b'C') => 12,
            Ok(b'd' | b'D') => 13,
            Ok(b'e' | b'E') => 14,
            Ok(b'f' | b'F') => 15,
            _ => panic!("unexpected: {}", JsString::from_units(vec![c])),
        }
    }
    // port: IRFactory#transformBooleanTokenType
    fn transform_boolean_token_type(token: TokenType) -> Token {
        match token {
            TokenType::TRUE => Token::TRUE,
            TokenType::FALSE => Token::FALSE,
            _ => panic!("{token}"),
        }
    }
    // port: IRFactory#transformUpdateTokenType
    fn transform_update_token_type(token: TokenType) -> Token {
        match token {
            TokenType::PLUS_PLUS => Token::INC,
            TokenType::MINUS_MINUS => Token::DEC,
            _ => panic!("{token}"),
        }
    }
    // port: IRFactory#transformUnaryTokenType
    fn transform_unary_token_type(token: TokenType) -> Token {
        match token {
            TokenType::BANG => Token::NOT,
            TokenType::TILDE => Token::BITNOT,
            TokenType::PLUS => Token::POS,
            TokenType::MINUS => Token::NEG,
            TokenType::DELETE => Token::DELPROP,
            TokenType::TYPEOF => Token::TYPEOF,
            TokenType::VOID => Token::VOID,
            _ => panic!("{token}"),
        }
    }
    // port: IRFactory#transformBinaryTokenType
    fn transform_binary_token_type(token: TokenType) -> Token {
        match token {
            TokenType::BAR => Token::BITOR,
            TokenType::CARET => Token::BITXOR,
            TokenType::AMPERSAND => Token::BITAND,
            TokenType::EQUAL_EQUAL => Token::EQ,
            TokenType::NOT_EQUAL => Token::NE,
            TokenType::OPEN_ANGLE => Token::LT,
            TokenType::LESS_EQUAL => Token::LE,
            TokenType::CLOSE_ANGLE => Token::GT,
            TokenType::GREATER_EQUAL => Token::GE,
            TokenType::LEFT_SHIFT => Token::LSH,
            TokenType::RIGHT_SHIFT => Token::RSH,
            TokenType::UNSIGNED_RIGHT_SHIFT => Token::URSH,
            TokenType::PLUS => Token::ADD,
            TokenType::MINUS => Token::SUB,
            TokenType::STAR => Token::MUL,
            TokenType::SLASH => Token::DIV,
            TokenType::PERCENT => Token::MOD,
            TokenType::STAR_STAR => Token::EXPONENT,
            TokenType::EQUAL_EQUAL_EQUAL => Token::SHEQ,
            TokenType::NOT_EQUAL_EQUAL => Token::SHNE,
            TokenType::IN => Token::IN,
            TokenType::INSTANCEOF => Token::INSTANCEOF,
            TokenType::COMMA => Token::COMMA,
            TokenType::EQUAL => Token::ASSIGN,
            TokenType::BAR_EQUAL => Token::ASSIGN_BITOR,
            TokenType::CARET_EQUAL => Token::ASSIGN_BITXOR,
            TokenType::AMPERSAND_EQUAL => Token::ASSIGN_BITAND,
            TokenType::LEFT_SHIFT_EQUAL => Token::ASSIGN_LSH,
            TokenType::RIGHT_SHIFT_EQUAL => Token::ASSIGN_RSH,
            TokenType::UNSIGNED_RIGHT_SHIFT_EQUAL => Token::ASSIGN_URSH,
            TokenType::PLUS_EQUAL => Token::ASSIGN_ADD,
            TokenType::MINUS_EQUAL => Token::ASSIGN_SUB,
            TokenType::STAR_EQUAL => Token::ASSIGN_MUL,
            TokenType::STAR_STAR_EQUAL => Token::ASSIGN_EXPONENT,
            TokenType::SLASH_EQUAL => Token::ASSIGN_DIV,
            TokenType::PERCENT_EQUAL => Token::ASSIGN_MOD,
            TokenType::OR => Token::OR,
            TokenType::AND => Token::AND,
            TokenType::QUESTION_QUESTION => Token::COALESCE,
            TokenType::OR_EQUAL => Token::ASSIGN_OR,
            TokenType::AND_EQUAL => Token::ASSIGN_AND,
            TokenType::QUESTION_QUESTION_EQUAL => Token::ASSIGN_COALESCE,
            _ => panic!("{token}"),
        }
    }
    // port: IRFactory#newNode(Token)
    fn new_node(&self, ast: &mut Ast, type_: Token) -> NodeId {
        ast.new_node(type_)
            .clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#newNode(Token, Node)
    fn new_node1(&self, ast: &mut Ast, type_: Token, child1: NodeId) -> NodeId {
        ast.new_node_with_child(type_, child1)
            .clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#newNode(Token, Node, Node)
    fn new_node2(&self, ast: &mut Ast, type_: Token, child1: NodeId, child2: NodeId) -> NodeId {
        ast.new_node_with_children2(type_, child1, child2)
            .clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#newNode(Token, Node, Node, Node)
    fn new_node3(
        &self,
        ast: &mut Ast,
        type_: Token,
        child1: NodeId,
        child2: NodeId,
        child3: NodeId,
    ) -> NodeId {
        ast.new_node_with_children3(type_, child1, child2, child3)
            .clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#newStringNode(String)
    fn new_string_literal_node(&self, ast: &mut Ast, value: impl Into<JsString>) -> NodeId {
        IR::string(ast, value).clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#newStringNode(Token, String)
    fn new_string_node(&self, ast: &mut Ast, type_: Token, value: impl Into<JsString>) -> NodeId {
        ast.new_string_with_token(type_, value)
            .clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#maybeAddNonJsDocComment
    fn maybe_add_non_js_doc_comment(
        &mut self,
        ast: &mut Ast,
        node: NodeId,
        start: &SourcePosition,
    ) -> NodeId {
        if let Some(comment) = self.parse_non_jsdoc_comment_at(start, false) {
            node.set_non_jsdoc_comment(ast, Some(comment));
        }
        node
    }
    // port: IRFactory#newTemplateLitStringNode
    fn new_template_lit_string_node(
        &self,
        ast: &mut Ast,
        cooked: Option<JsString>,
        raw: JsString,
    ) -> NodeId {
        ast.new_template_lit_string(cooked, raw)
            .clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#newNumberNode
    fn new_number_node(&self, ast: &mut Ast, value: f64) -> NodeId {
        IR::number(ast, value).clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#newBigIntNode
    fn new_big_int_node(&self, ast: &mut Ast, value: num_bigint::BigInt) -> NodeId {
        ast.new_big_int(value)
            .clone_props_from(ast, self.template_node)
    }
    // port: IRFactory#error
    fn error_js_string(&mut self, ast: &Ast, rhino_node: NodeId, message: &JsString) {
        self.error_reporter.error_js_string(
            message,
            &rhino_node.get_source_file_name(ast).unwrap_or_default(),
            rhino_node.get_lineno(ast),
            rhino_node.get_charno(ast),
        );
    }
    // port: IRFactory#error
    fn error(&mut self, ast: &Ast, rhino_node: NodeId, message: &str) {
        self.error_reporter.error(
            message,
            &rhino_node.get_source_file_name(ast).unwrap_or_default(),
            rhino_node.get_lineno(ast),
            rhino_node.get_charno(ast),
        );
    }
}
pub const JSCOMP_CLOSURE_UNAWARE_CODE_SHADOW_HOST_NAME: &str = "$jscomp_wrap_closure_unaware_code";
const JSCOMP_CLOSURE_UNAWARE_SINK_VALUE: &str = "$jscomp_sink_closure_unaware_impl";
struct ParsedJsDoc {
    license: Option<JsString>,
    fileoverview: Option<Arc<JSDocInfo>>,
    info: Option<Arc<JSDocInfo>>,
}
struct CommentTracker {
    source: Vec<Comment>,
    filter_jsdoc: bool,
    index: i32,
    previous_index: i32,
}
impl CommentTracker {
    // port: IRFactory.CommentTracker#CommentTracker
    fn new(source: Vec<Comment>, filter_jsdoc: bool) -> Self {
        let mut tracker = Self {
            source,
            filter_jsdoc,
            index: -1,
            previous_index: -1,
        };
        tracker.advance();
        tracker
    }
    // port: IRFactory.CommentTracker#current
    fn current(&self) -> Option<&Comment> {
        self.source.get(self.index as usize)
    }
    // port: IRFactory.CommentTracker#advance
    fn advance(&mut self) {
        self.previous_index = self.index;
        loop {
            self.index += 1;
            if self
                .current()
                .is_none_or(|c| (c.type_ == comment::Type::JSDOC) == self.filter_jsdoc)
            {
                break;
            }
        }
    }
    // port: IRFactory.CommentTracker#backtrack
    fn backtrack(&mut self) {
        self.index = self.previous_index;
    }
    // port: IRFactory.CommentTracker#hasPendingCommentBefore
    fn has_pending_comment_before(&self, pos: &SourcePosition) -> bool {
        self.current()
            .is_some_and(|c| c.location.end.line <= pos.line && c.location.end.offset <= pos.offset)
    }
}
#[derive(Default)]
struct TransformDispatcher {
    class_scope: ScopeTracker,
    private_id_lhs_of_in_scope: ScopeTracker,
}
#[derive(Default)]
struct ScopeTracker {
    usage_count: std::rc::Rc<std::cell::Cell<i32>>,
}
impl ScopeTracker {
    // port: IRFactory.TransformDispatcher.ScopeTracker#inScope
    fn in_scope(&self) -> bool {
        self.usage_count.get() > 0
    }
    // port: IRFactory.TransformDispatcher.ScopeTracker#increment
    fn increment(&self) -> AutoDecrement {
        self.maybe_increment(true)
    }
    // port: IRFactory.TransformDispatcher.ScopeTracker#maybeIncrement
    fn maybe_increment(&self, condition: bool) -> AutoDecrement {
        if condition {
            check_state!(self.usage_count.get() >= 0);
            self.usage_count.set(self.usage_count.get() + 1);
        }
        AutoDecrement {
            usage_count: self.usage_count.clone(),
            condition,
            closed: false,
        }
    }
}
struct AutoDecrement {
    usage_count: std::rc::Rc<std::cell::Cell<i32>>,
    condition: bool,
    closed: bool,
}
impl AutoDecrement {
    // port: IRFactory.TransformDispatcher.AutoDecrement#close
    fn close(&mut self) {
        check_state!(!self.closed);
        if self.condition {
            self.usage_count.set(self.usage_count.get() - 1);
            check_state!(self.usage_count.get() >= 0);
        }
        self.closed = true;
    }
}
impl Drop for AutoDecrement {
    // port: IRFactory.TransformDispatcher.AutoDecrement#close
    fn drop(&mut self) {
        if !self.closed {
            self.close();
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum IdentifierType {
    STANDARD,
    CAN_BE_PRIVATE,
}
// Java null dereferences that occur on malformed recovery trees are control-flow exceptions.
fn required<T>(value: Option<T>) -> P<T> {
    value.ok_or(ParseError {
        class: "java.lang.NullPointerException",
        message: None,
    })
}
fn number_format_error(error: closure_rhino::java_lang::NumberFormatException) -> ParseError {
    ParseError {
        class: "java.lang.NumberFormatException",
        message: Some(JsString::from_units(error.message)),
    }
}
fn big_integer_error(
    error: closure_rhino::java_lang::big_integer::BigIntegerParseError,
) -> ParseError {
    ParseError {
        class: error.class(),
        message: Some(error.message()),
    }
}
