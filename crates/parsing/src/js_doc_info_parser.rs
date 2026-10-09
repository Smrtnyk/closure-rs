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
//   src/com/google/javascript/jscomp/parsing/JsDocInfoParser.java.

//! Port of com.google.javascript.jscomp.parsing.JsDocInfoParser.
#![allow(
    clippy::needless_return,
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::too_many_arguments
)]
use crate::{
    annotation::Annotation,
    config::{Config, JsDocParsing, LanguageMode, StrictMode},
    js_doc_token::JsDocToken,
    js_doc_token_stream::JsDocTokenStream,
    type_transformation_parser::TypeTransformationParser,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{check_argument, check_state};
use closure_rhino::{
    error_reporter::{ErrorReporter, NullErrorReporter},
    ir::IR,
    java_lang,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::{Builder, JSDocInfo, PerFileClosureUnawareMode, Visibility},
    msg::Msg,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
    token::Token,
    token_stream::TokenStream,
    token_util::TokenUtil,
};
use std::sync::Arc;

const TTL_START_DELIMITER: &str = "=";
const TTL_END_DELIMITER: &str = "=:";
pub const BAD_TYPE_WIKI_LINK: &str = " See https://github.com/google/closure-compiler/wiki/Annotating-JavaScript-for-the-Closure-Compiler for more information.";
const TSICKLE_MISSING_TYPE_PLACEHOLDER: &str =
    "JsDocInfoParser_TsickleMode_MissingSupertypePlaceholder";
const TS_TYPE_PLACEHOLDER: &str = "JsDocInfoParser_tsType_PlaceHolder";
const MODIFIES_ANNOTATION_KEYWORDS: &[&str] = &["this", "arguments"];
const ID_GENERATOR_ANNOTATION_KEYWORDS: &[&str] =
    &["unique", "consistent", "stable", "mapped", "xid"];
const NO_UNREAD_TOKEN: Option<JsDocToken> = None;
const PRIMITIVE_TYPES: &[&str] = &["number", "string", "boolean", "symbol"];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // NEXT_IS_ANNOTATION is also unused in Java.
enum State {
    SEARCHING_ANNOTATION,
    SEARCHING_NEWLINE,
    NEXT_IS_ANNOTATION,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsDocSourceKind {
    NORMAL,
    TSICKLE,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WhitespaceOption {
    PRESERVE,
    TRIM,
    SINGLE_LINE,
}
struct ExtractionInfo {
    string: JsString,
    token: JsDocToken,
}
impl ExtractionInfo {
    // port: JsDocInfoParser.ExtractionInfo#ExtractionInfo
    fn new(string: impl Into<JsString>, token: JsDocToken) -> Self {
        Self {
            string: string.into(),
            token,
        }
    }
}
struct ExtendedTypeInfo {
    r#type: Option<Arc<JSTypeExpression>>,
    lineno: i32,
    charno: i32,
}
impl ExtendedTypeInfo {
    // port: JsDocInfoParser.ExtendedTypeInfo#ExtendedTypeInfo
    fn new(r#type: Option<Arc<JSTypeExpression>>, lineno: i32, charno: i32) -> Self {
        Self {
            r#type,
            lineno,
            charno,
        }
    }
}
pub struct JsDocInfoParser<'a> {
    stream: JsDocTokenStream,
    jsdoc_builder: Builder,
    error_reporter: &'a mut dyn ErrorReporter,
    js_doc_source_kind: JsDocSourceKind,
    template_node: NodeId,
    file_overview_jsdoc_info: Option<Arc<JSDocInfo>>,
    state: State,
    annotations: Arc<IndexMap<JsString, Annotation>>,
    suppression_names: Arc<IndexSet<JsString>>,
    closure_primitive_names: Arc<IndexSet<JsString>>,
    preserve_whitespace: bool,
    only_parse_license_js_doc: bool,
    license_text: Option<JsString>,
    record_ts_type: bool,
    unread_token: Option<JsDocToken>,
}
impl<'a> JsDocInfoParser<'a> {
    // port: JsDocInfoParser#addParserWarning(Msg, String)
    fn add_parser_warning_arg(&mut self, ast: &Ast, msg: Msg, arg: &JsString) {
        self.add_parser_warning_arg_at(
            ast,
            msg,
            arg,
            self.stream.get_lineno(),
            self.stream.get_charno(),
        );
    }
    // port: JsDocInfoParser#addParserWarning(Msg, String, int, int)
    fn add_parser_warning_arg_at(
        &mut self,
        ast: &Ast,
        msg: Msg,
        arg: &JsString,
        lineno: i32,
        charno: i32,
    ) {
        self.error_reporter.warning_js_string(
            &msg.format_with_js_strings(std::slice::from_ref(arg)),
            &self.get_source_name(ast),
            lineno,
            charno,
        );
    }
    // port: JsDocInfoParser#addParserWarning(Msg)
    fn add_parser_warning(&mut self, ast: &Ast, msg: Msg) {
        self.add_parser_warning_at(ast, msg, self.stream.get_lineno(), self.stream.get_charno());
    }
    // port: JsDocInfoParser#addParserWarning(Msg, int, int)
    fn add_parser_warning_at(&mut self, ast: &Ast, msg: Msg, lineno: i32, charno: i32) {
        self.error_reporter
            .warning(msg.format(), &self.get_source_name(ast), lineno, charno);
    }
    // port: JsDocInfoParser#addTypeWarning(Msg, String)
    fn add_type_warning_arg(&mut self, ast: &Ast, msg: Msg, arg: &JsString) {
        self.add_type_warning_arg_at(
            ast,
            msg,
            arg,
            self.stream.get_lineno(),
            self.stream.get_charno(),
        );
    }
    // port: JsDocInfoParser#addTypeWarning(Msg, String, int, int)
    fn add_type_warning_arg_at(
        &mut self,
        ast: &Ast,
        msg: Msg,
        arg: &JsString,
        lineno: i32,
        charno: i32,
    ) {
        self.error_reporter.warning_js_string(
            &JsString::from("Bad type annotation. ")
                .concat(&msg.format_with_js_strings(std::slice::from_ref(arg)))
                .concat(&BAD_TYPE_WIKI_LINK.into()),
            &self.get_source_name(ast),
            lineno,
            charno,
        );
    }
    // port: JsDocInfoParser#addTypeWarning(Msg)
    fn add_type_warning(&mut self, ast: &Ast, msg: Msg) {
        self.add_type_warning_at(ast, msg, self.stream.get_lineno(), self.stream.get_charno());
    }
    // port: JsDocInfoParser#addTypeWarning(Msg, int, int)
    fn add_type_warning_at(&mut self, ast: &Ast, msg: Msg, lineno: i32, charno: i32) {
        self.error_reporter.warning(
            &format!("Bad type annotation. {}{BAD_TYPE_WIKI_LINK}", msg.format()),
            &self.get_source_name(ast),
            lineno,
            charno,
        );
    }
    // port: JsDocInfoParser#addMissingTypeWarning
    fn add_missing_type_warning(&mut self, ast: &Ast, lineno: i32, charno: i32) {
        self.error_reporter.warning(
            "Missing type declaration.",
            &self.get_source_name(ast),
            lineno,
            charno,
        );
    }
    // port: JsDocInfoParser#setLicenseTextMonotonic
    fn set_license_text_monotonic(&mut self, x: JsString) {
        check_state!(self.license_text.is_none());
        self.license_text = Some(x);
    }
    // port: JsDocInfoParser#getLicenseText
    pub fn get_license_text(&self) -> Option<JsString> {
        self.license_text.clone()
    }
    // port: JsDocInfoParser#setFileOverviewJSDocInfo
    pub fn set_file_overview_jsdoc_info(&mut self, info: Option<Arc<JSDocInfo>>) {
        self.file_overview_jsdoc_info = info;
    }
    // port: JsDocInfoParser#getSourceFile
    pub fn get_source_file(&self, ast: &Ast) -> Option<Arc<dyn StaticSourceFile>> {
        self.template_node.get_static_source_file(ast)
    }
    // port: JsDocInfoParser#JsDocInfoParser
    pub fn new(
        ast: &mut Ast,
        stream: JsDocTokenStream,
        comment: Option<JsString>,
        comment_position: i32,
        template_node: Option<NodeId>,
        config: &Config,
        js_doc_source_kind: JsDocSourceKind,
        error_reporter: &'a mut dyn ErrorReporter,
    ) -> Self {
        let mut jsdoc_builder = JSDocInfo::builder();
        if config.js_doc_parsing_mode().should_parse_descriptions() {
            jsdoc_builder.parse_documentation();
        }
        if let Some(comment) = comment {
            jsdoc_builder.record_original_comment_string(comment);
            jsdoc_builder.record_original_comment_position(comment_position);
        }
        Self {
            stream,
            jsdoc_builder,
            error_reporter,
            js_doc_source_kind,
            template_node: template_node.unwrap_or_else(|| IR::script(ast)),
            file_overview_jsdoc_info: None,
            state: State::SEARCHING_ANNOTATION,
            annotations: config.annotations.clone(),
            suppression_names: config.suppression_names.clone(),
            closure_primitive_names: config.closure_primitive_names.clone(),
            preserve_whitespace: config.js_doc_parsing_mode().should_preserve_whitespace(),
            only_parse_license_js_doc: config.js_doc_parsing_mode()
                == JsDocParsing::LICENSE_COMMENTS_ONLY,
            license_text: None,
            record_ts_type: config.js_doc_parsing_mode().should_parse_descriptions(),
            unread_token: NO_UNREAD_TOKEN,
        }
    }
    // port: JsDocInfoParser#getSourceName
    fn get_source_name(&self, ast: &Ast) -> String {
        self.get_source_file(ast)
            .map(|f| f.get_name().to_owned())
            .unwrap_or_default()
    }
    // port: JsDocInfoParser#parseInlineTypeDoc
    pub fn parse_inline_type_doc(&mut self, ast: &mut Ast) -> Option<Arc<JSDocInfo>> {
        self.skip_eols();
        let token = self.next();
        let lineno = self.stream.get_lineno();
        let start_charno = self.stream.get_charno();
        let type_ast = self.parse_param_type_expression(ast, token);
        self.record_type_node(
            lineno,
            start_charno,
            type_ast,
            token == JsDocToken::LEFT_CURLY,
        );
        let expr = self.create_jstype_expression(ast, type_ast);
        if expr.is_some() {
            self.jsdoc_builder.record_type(expr);
            self.jsdoc_builder.record_inline_type();
            return self.retrieve_and_reset_parsed_jsdoc_info();
        }
        None
    }
    // port: JsDocInfoParser#recordTypeNode
    fn record_type_node(
        &mut self,
        lineno: i32,
        start_charno: i32,
        type_ast: Option<NodeId>,
        matching_lc: bool,
    ) {
        if let Some(n) = type_ast {
            self.jsdoc_builder.mark_type_node(
                Some(n),
                lineno,
                start_charno,
                self.stream.get_lineno(),
                self.stream.get_charno(),
                matching_lc,
            );
        }
    }
    // port: JsDocInfoParser#parseTypeString
    pub fn parse_type_string(ast: &mut Ast, type_string: impl Into<JsString>) -> Option<NodeId> {
        let mut reporter = NullErrorReporter;
        let mut parser = Self::get_parser(ast, type_string.into(), &mut reporter);
        let token = parser.next();
        parser.parse_top_level_type_expression(ast, token)
    }
    // port: JsDocInfoParser#parseJsdoc
    pub fn parse_jsdoc(ast: &mut Ast, to_parse: impl Into<JsString>) -> Option<Arc<JSDocInfo>> {
        let mut reporter = NullErrorReporter;
        let mut parser = Self::get_parser(ast, to_parse.into(), &mut reporter);
        parser.parse(ast);
        parser.retrieve_and_reset_parsed_jsdoc_info()
    }
    // port: JsDocInfoParser#parseFileOverviewJsdoc
    pub fn parse_file_overview_jsdoc(
        ast: &mut Ast,
        to_parse: impl Into<JsString>,
    ) -> Option<Arc<JSDocInfo>> {
        let mut reporter = NullErrorReporter;
        let mut parser = Self::get_parser(ast, to_parse.into(), &mut reporter);
        parser.parse(ast);
        parser.get_file_overview_jsdoc_info()
    }
    // port: JsDocInfoParser#getParser
    fn get_parser<'b>(
        ast: &mut Ast,
        to_parse: JsString,
        reporter: &'b mut dyn ErrorReporter,
    ) -> JsDocInfoParser<'b> {
        let config = Config::builder()
            .set_language_mode(LanguageMode::ECMASCRIPT3)
            .set_strict_mode(StrictMode::SLOPPY)
            .set_closure_primitive_names(["testPrimitive"])
            .build();
        JsDocInfoParser::new(
            ast,
            JsDocTokenStream::new(to_parse.clone()),
            Some(to_parse),
            0,
            None,
            &config,
            JsDocSourceKind::NORMAL,
            reporter,
        )
    }
    // port: JsDocInfoParser#parse
    pub fn parse(&mut self, ast: &mut Ast) -> bool {
        self.state = State::SEARCHING_ANNOTATION;
        self.skip_eols();
        let mut token = self.next();
        if self.jsdoc_builder.should_parse_documentation() {
            let block_info = self.extract_block_comment(token);
            token = block_info.token;
            if !block_info.string.is_empty() {
                self.jsdoc_builder
                    .record_block_description(block_info.string);
            }
        } else if token != JsDocToken::ANNOTATION && token != JsDocToken::EOC {
            self.jsdoc_builder.record_block_description("");
        }
        self.parse_helper_loop(ast, token, &mut Vec::new())
    }
    // port: JsDocInfoParser#parseImportantComment
    pub fn parse_important_comment(&mut self) {
        self.state = State::SEARCHING_ANNOTATION;
        self.skip_eols();
        let token = self.next();
        let info = self.extract_multiline_comment(token, WhitespaceOption::PRESERVE, false, true);
        let license = JsString::from(" ").concat(&info.string);
        self.set_license_text_monotonic(license.clone());
        self.jsdoc_builder.record_block_description(
            if self.jsdoc_builder.should_parse_documentation() {
                license
            } else {
                "".into()
            },
        );
    }
    // port: JsDocInfoParser#parseHelperLoop
    fn parse_helper_loop(
        &mut self,
        ast: &mut Ast,
        mut token: JsDocToken,
        extended_types: &mut Vec<ExtendedTypeInfo>,
    ) -> bool {
        loop {
            match token {
                JsDocToken::ANNOTATION => {
                    if self.state == State::SEARCHING_ANNOTATION {
                        self.state = State::SEARCHING_NEWLINE;
                        token = self.parse_annotation(ast, token, extended_types);
                    } else {
                        token = self.next();
                    }
                }
                JsDocToken::EOC => {
                    let mut success = true;
                    self.check_extended_types(ast, extended_types);
                    if self.has_parsed_file_overview_doc_info() {
                        self.file_overview_jsdoc_info = self.retrieve_and_reset_parsed_jsdoc_info();
                        let visibility = self
                            .file_overview_jsdoc_info
                            .as_ref()
                            .unwrap()
                            .get_visibility();
                        if matches!(visibility, Visibility::PRIVATE | Visibility::PROTECTED) {
                            self.add_parser_warning_arg(
                                ast,
                                Msg::BAD_FILEOVERVIEW_VISIBIIITY_ANNOTATION,
                                &format!("{visibility:?}").to_ascii_lowercase().into(),
                            );
                            success = false;
                        }
                    }
                    return success;
                }
                JsDocToken::EOF => {
                    self.jsdoc_builder.build();
                    self.add_parser_warning(ast, Msg::UNEXPECTED_EOF);
                    self.check_extended_types(ast, extended_types);
                    return false;
                }
                JsDocToken::EOL => {
                    if self.state == State::SEARCHING_NEWLINE {
                        self.state = State::SEARCHING_ANNOTATION;
                    }
                    token = self.next();
                }
                _ => {
                    if token == JsDocToken::STAR && self.state == State::SEARCHING_ANNOTATION {
                        token = self.next();
                    } else {
                        self.state = State::SEARCHING_NEWLINE;
                        token = self.eat_tokens_until_eol();
                    }
                }
            }
        }
    }
    // port: JsDocInfoParser#parseAnnotation
    #[allow(unused_assignments)] // Preserve Java token consumption and assignments.
    fn parse_annotation(
        &mut self,
        ast: &mut Ast,
        mut token: JsDocToken,
        extended_types: &mut Vec<ExtendedTypeInfo>,
    ) -> JsDocToken {
        let mut lineno = self.stream.get_lineno();
        let mut charno = self.stream.get_charno();
        let annotation_name = self.stream.get_string();
        let annotation = self.annotations.get(&annotation_name).copied();
        if self.only_parse_license_js_doc
            && !matches!(annotation, Some(Annotation::LICENSE | Annotation::PRESERVE))
        {
            return self.next();
        }
        if annotation.is_none() || annotation_name.is_empty() {
            self.add_parser_warning_arg(ast, Msg::BAD_JSDOC_TAG, &annotation_name);
            return self.next();
        }
        self.jsdoc_builder
            .mark_annotation(annotation_name, lineno, charno);
        let annotation = annotation.unwrap();
        match annotation {
            Annotation::NG_INJECT => {
                if self.jsdoc_builder.is_ng_inject_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_NGINJECT_EXTRA);
                } else {
                    self.jsdoc_builder.record_ng_inject(true);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::ABSTRACT => {
                if !self.jsdoc_builder.record_abstract() {
                    self.add_type_warning(ast, Msg::JSDOC_INCOMPAT_TYPE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::AUTHOR => {
                if self.jsdoc_builder.should_parse_documentation() {
                    let author_info = self.extract_single_line_block();
                    let author = author_info.string;
                    if author.is_empty() {
                        self.add_parser_warning(ast, Msg::JSDOC_AUTHORMISSING);
                    } else {
                        self.jsdoc_builder.record_author(author);
                    }
                    token = author_info.token;
                } else {
                    token = self.eat_until_eol_if_not_annotation();
                }
                return token;
            }
            Annotation::UNRESTRICTED => {
                if !self.jsdoc_builder.record_unrestricted() {
                    self.add_type_warning(ast, Msg::JSDOC_INCOMPAT_TYPE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::STRUCT => {
                if !self.jsdoc_builder.record_struct() {
                    self.add_type_warning(ast, Msg::JSDOC_INCOMPAT_TYPE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::DICT => {
                if !self.jsdoc_builder.record_dict() {
                    self.add_type_warning(ast, Msg::JSDOC_INCOMPAT_TYPE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::COLLAPSIBLE_OR_BREAK_MY_CODE => {
                if !self.jsdoc_builder.record_collapsible_or_break_my_code() {
                    self.add_parser_warning(ast, Msg::JSDOC_COLLAPSIBLEORBREAKMYCODE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::CONSTRUCTOR => {
                if !self.jsdoc_builder.record_constructor() {
                    if self.jsdoc_builder.is_interface_recorded() {
                        self.add_type_warning(ast, Msg::JSDOC_INTERFACE_CONSTRUCTOR);
                    } else {
                        self.add_type_warning(ast, Msg::JSDOC_INCOMPAT_TYPE);
                    }
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::RECORD => {
                if !self.jsdoc_builder.record_implicit_match() {
                    self.add_type_warning(ast, Msg::JSDOC_RECORD);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::DEPRECATED => {
                if !self.jsdoc_builder.record_deprecated() {
                    self.add_parser_warning(ast, Msg::JSDOC_DEPRECATED);
                }
                let info = self.extract_multiline_textual_block(token);
                if !info.string.is_empty() {
                    self.jsdoc_builder.record_deprecation_reason(info.string);
                }
                return info.token;
            }
            Annotation::INTERFACE => {
                if !self.jsdoc_builder.record_interface() {
                    if self.jsdoc_builder.is_constructor_recorded() {
                        self.add_type_warning(ast, Msg::JSDOC_INTERFACE_CONSTRUCTOR);
                    } else {
                        self.add_type_warning(ast, Msg::JSDOC_INCOMPAT_TYPE);
                    }
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::DESC => {
                if self.jsdoc_builder.is_description_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_DESC_EXTRA);
                    return self.eat_until_eol_if_not_annotation();
                }
                let info = self.extract_multiline_textual_block(token);
                self.jsdoc_builder.record_description(info.string);
                return info.token;
            }
            Annotation::TS_TYPE => {
                let info = self.extract_multiline_textual_block(token);
                self.jsdoc_builder.record_ts_type(if self.record_ts_type {
                    info.string
                } else {
                    TS_TYPE_PLACEHOLDER.into()
                });
                return info.token;
            }
            Annotation::FILE_OVERVIEW => {
                let mut overview = JsString::from("");
                if self.jsdoc_builder.should_parse_documentation()
                    && !self.look_ahead_for_annotation()
                {
                    let info = self.extract_multiline_textual_block_with_options(
                        token,
                        self.get_whitespace_option(WhitespaceOption::TRIM),
                        false,
                    );
                    overview = info.string;
                    token = info.token;
                } else {
                    token = self.eat_until_eol_if_not_annotation();
                }
                if !self.jsdoc_builder.record_file_overview(overview) {
                    self.add_parser_warning(ast, Msg::JSDOC_FILEOVERVIEW_EXTRA);
                }
                return token;
            }
            Annotation::LICENSE | Annotation::PRESERVE => {
                let info = self.extract_multiline_textual_block_with_options(
                    token,
                    WhitespaceOption::PRESERVE,
                    true,
                );
                if !info.string.is_empty() {
                    self.set_license_text_monotonic(info.string);
                }
                return info.token;
            }
            Annotation::ENHANCE => {
                let info = self.extract_single_line_block();
                self.jsdoc_builder.record_enhance(info.string);
                return info.token;
            }
            Annotation::MODS => {
                if self.jsdoc_builder.is_mods_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_MODS_EXTRA);
                } else {
                    let next = self.next();
                    token = self.parse_mods_tag(ast, next);
                }
                if token != JsDocToken::EOC && token != JsDocToken::EOF {
                    token = self.eat_until_eol_if_not_annotation();
                }
                return token;
            }
            Annotation::ENUM => {
                token = self.next();
                lineno = self.stream.get_lineno();
                charno = self.stream.get_charno();
                let mut expr = None;
                if token != JsDocToken::EOL && token != JsDocToken::EOC {
                    let mut node = self.parse_and_record_type_node(ast, token);
                    if let Some(n) = node {
                        if n.is_string_lit(ast)
                            && !PRIMITIVE_TYPES.iter().any(|s| n.get_string(ast) == *s)
                        {
                            node = self.wrap_node(ast, Token::BANG, node);
                        }
                    }
                    expr = self.create_jstype_expression(ast, node);
                } else {
                    self.restore_look_ahead(token);
                }
                if expr.is_none() {
                    let n = self.new_string_node(ast, "number".into());
                    expr = self.create_jstype_expression(ast, Some(n));
                }
                if !self.jsdoc_builder.record_enum_parameter_type(expr) {
                    self.add_type_warning_at(ast, Msg::JSDOC_INCOMPAT_TYPE, lineno, charno);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::EXTERNS => {
                if !self.jsdoc_builder.record_externs() {
                    self.add_parser_warning(ast, Msg::JSDOC_EXTERNS);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::NO_COVERAGE => {
                if !self.jsdoc_builder.record_no_coverage() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOCOVERAGE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::TYPE_SUMMARY => {
                if !self.jsdoc_builder.record_type_summary() {
                    self.add_parser_warning(ast, Msg::JSDOC_TYPESUMMARY);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::EXTENDS | Annotation::IMPLEMENTS => {
                self.skip_eols();
                token = self.next();
                lineno = self.stream.get_lineno();
                charno = self.stream.get_charno();
                let mut matching_rc = false;
                if token == JsDocToken::LEFT_CURLY {
                    token = self.next();
                    matching_rc = true;
                }
                if token == JsDocToken::STRING {
                    let node = self.parse_and_record_type_name_node(
                        ast,
                        token,
                        lineno,
                        charno,
                        matching_rc,
                    );
                    lineno = self.stream.get_lineno();
                    charno = self.stream.get_charno();
                    let node = self.wrap_node(ast, Token::BANG, node);
                    let expr = self.create_jstype_expression(ast, node);
                    if annotation == Annotation::EXTENDS {
                        extended_types.push(ExtendedTypeInfo::new(
                            expr,
                            self.stream.get_lineno(),
                            self.stream.get_charno(),
                        ));
                    } else {
                        check_state!(annotation == Annotation::IMPLEMENTS);
                        if !self.jsdoc_builder.record_implemented_interface(ast, expr) {
                            self.add_type_warning_at(
                                ast,
                                Msg::JSDOC_IMPLEMENTS_DUPLICATE,
                                lineno,
                                charno,
                            );
                        }
                    }
                    token = self.next();
                    if matching_rc {
                        if token != JsDocToken::RIGHT_CURLY {
                            self.add_type_warning(ast, Msg::JSDOC_MISSING_RC);
                        } else {
                            token = self.next();
                        }
                    } else if !matches!(token, JsDocToken::EOL | JsDocToken::EOF | JsDocToken::EOC)
                    {
                        self.add_type_warning(ast, Msg::END_ANNOTATION_EXPECTED);
                    }
                } else if matches!(token, JsDocToken::BANG | JsDocToken::QMARK) {
                    self.add_type_warning_at(
                        ast,
                        Msg::JSDOC_IMPLEMENTS_EXTRAQUALIFIER,
                        lineno,
                        charno,
                    );
                } else {
                    self.add_type_warning_at(ast, Msg::NO_TYPE_NAME, lineno, charno);
                }
                return self.eat_until_eol_if_not_annotation_from(token);
            }
            Annotation::LENDS => {
                self.skip_eols();
                let mut matching_rc = false;
                if self.match_token(JsDocToken::LEFT_CURLY) {
                    token = self.next();
                    matching_rc = true;
                }
                if self.match_token(JsDocToken::STRING) {
                    let next = self.next();
                    let n = self.parse_name_expression(ast, next);
                    let expr = self.create_jstype_expression(ast, n);
                    if !self.jsdoc_builder.record_lends(expr) {
                        self.add_type_warning(ast, Msg::JSDOC_LENDS_INCOMPATIBLE);
                    }
                } else {
                    self.add_type_warning(ast, Msg::JSDOC_LENDS_MISSING);
                }
                if matching_rc && !self.match_token(JsDocToken::RIGHT_CURLY) {
                    self.add_type_warning(ast, Msg::JSDOC_MISSING_RC);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::MEANING => {
                let info = self.extract_multiline_textual_block(token);
                if !self.jsdoc_builder.record_meaning(info.string) {
                    self.add_parser_warning(ast, Msg::JSDOC_MEANING_EXTRA);
                }
                return info.token;
            }
            Annotation::ALTERNATE_MESSAGE_ID => {
                let info = self.extract_single_line_block();
                if !self.jsdoc_builder.record_alternate_message_id(info.string) {
                    self.add_parser_warning(ast, Msg::JSDOC_ALTERNATEMESSAGEID_EXTRA);
                }
                return info.token;
            }
            Annotation::CLOSURE_PRIMITIVE => {
                self.skip_eols();
                let next = self.next();
                return self.parse_closure_primitive_tag(ast, next);
            }
            Annotation::NO_COMPILE => {
                if !self.jsdoc_builder.record_no_compile() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOCOMPILE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::NO_DTS => {
                if !self.jsdoc_builder.record_no_dts() {
                    self.add_parser_warning(ast, Msg::JSDOC_NODTS);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::NO_COLLAPSE => {
                if !self.jsdoc_builder.record_no_collapse() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOCOLLAPSE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::ENCOURAGE_INLINING => {
                if !self.jsdoc_builder.record_encourage_inlining() {
                    self.add_parser_warning(ast, Msg::JSDOC_ENCOURAGE_INLINING);
                }
                if self.jsdoc_builder.is_require_inlining() {
                    self.add_parser_warning(ast, Msg::JSDOC_INCOMPAT_INLINING);
                }
                if self.jsdoc_builder.is_no_inline() {
                    self.add_parser_warning(ast, Msg::JSDOC_INCOMPAT_INLINING);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::REQUIRE_INLINING => {
                if !self.jsdoc_builder.record_require_inlining() {
                    self.add_parser_warning(ast, Msg::JSDOC_REQUIRE_INLINING);
                }
                if self.jsdoc_builder.is_encourage_inlining() {
                    self.add_parser_warning(ast, Msg::JSDOC_INCOMPAT_INLINING);
                }
                if self.jsdoc_builder.is_no_inline() {
                    self.add_parser_warning(ast, Msg::JSDOC_INCOMPAT_INLINING);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::NO_INLINE => {
                if !self.jsdoc_builder.record_no_inline() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOINLINE);
                }
                if self.jsdoc_builder.is_encourage_inlining() {
                    self.add_parser_warning(ast, Msg::JSDOC_INCOMPAT_INLINING);
                }
                if self.jsdoc_builder.is_require_inlining() {
                    self.add_parser_warning(ast, Msg::JSDOC_INCOMPAT_INLINING);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::PROVIDE_GOOG => {
                if !self.jsdoc_builder.record_provide_goog() {
                    self.add_parser_warning(ast, Msg::JSDOC_PROVIDE_GOOG);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::PROVIDE_ALREADY_PROVIDED => {
                if !self.jsdoc_builder.record_provide_already_provided() {
                    self.add_parser_warning(ast, Msg::JSDOC_PROVIDE_ALREADY_PROVIDED);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::PURE_OR_BREAK_MY_CODE => {
                if !self.jsdoc_builder.record_pure_or_break_my_code() {
                    self.add_parser_warning(ast, Msg::JSDOC_PUREORBREAKMYCODE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::INHERIT_DOC | Annotation::OVERRIDE => {
                if !self.jsdoc_builder.record_override() {
                    self.add_type_warning(ast, Msg::JSDOC_OVERRIDE);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::POLYMER => {
                if self.jsdoc_builder.is_polymer_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_POLYMER_EXTRA);
                } else {
                    self.jsdoc_builder.record_polymer();
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::POLYMER_BEHAVIOR => {
                if self.jsdoc_builder.is_polymer_behavior_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_POLYMERBEHAVIOR_EXTRA);
                } else {
                    self.jsdoc_builder.record_polymer_behavior();
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::CUSTOM_ELEMENT => {
                if self.jsdoc_builder.is_custom_element_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_CUSTOMELEMENT_EXTRA);
                } else {
                    self.jsdoc_builder.record_custom_element();
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::MIXIN_CLASS => {
                if self.jsdoc_builder.is_mixin_class_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_MIXINCLASS_EXTRA);
                } else {
                    self.jsdoc_builder.record_mixin_class();
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::MIXIN_FUNCTION => {
                if self.jsdoc_builder.is_mixin_function_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_MIXINFUNCTION_EXTRA);
                } else {
                    self.jsdoc_builder.record_mixin_function();
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::SASS_GENERATED_CSS_TS => {
                if self.jsdoc_builder.is_sass_generated_css_ts_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_SASS_GENERATED_CSS_TS);
                } else {
                    self.jsdoc_builder.record_sass_generated_css_ts();
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::CLOSURE_UNAWARE_CODE => return self.parse_closure_unaware_code(ast),
            Annotation::THROWS => {
                lineno = self.stream.get_lineno();
                charno = self.stream.get_charno();
                let mut throws = JsString::from("");
                if !self.look_ahead_for_annotation() {
                    let info = self.extract_multiline_textual_block(token);
                    throws = java_lang::trim(&info.string);
                    token = info.token;
                }
                if self.jsdoc_builder.is_no_side_effects_recorded() {
                    self.add_parser_warning_at(
                        ast,
                        Msg::JSDOC_NOSIDEEFFECTS_WITH_THROWS,
                        lineno,
                        charno,
                    );
                } else if !self.jsdoc_builder.record_throws_annotation(throws) {
                    self.add_parser_warning_at(ast, Msg::JSDOC_THROWS, lineno, charno);
                }
                return token;
            }
            Annotation::PARAM => {
                self.skip_eols();
                token = self.next();
                lineno = self.stream.get_lineno();
                charno = self.stream.get_charno();
                let mut expr = None;
                let mut has_param_type = false;
                if token == JsDocToken::LEFT_CURLY {
                    let n = self.parse_and_record_param_type_node(ast, token);
                    expr = self.create_jstype_expression(ast, n);
                    if expr.is_none() {
                        return self.eat_until_eol_if_not_annotation();
                    }
                    self.skip_eols();
                    token = self.next();
                    lineno = self.stream.get_lineno();
                    charno = self.stream.get_charno();
                    has_param_type = true;
                }
                let mut name = None;
                let is_bracketed_param = token == JsDocToken::LEFT_SQUARE;
                if is_bracketed_param {
                    token = self.next();
                }
                if token != JsDocToken::STRING {
                    self.add_type_warning_at(ast, Msg::MISSING_VARIABLE_NAME, lineno, charno);
                } else {
                    if !has_param_type {
                        self.add_missing_type_warning(
                            ast,
                            self.stream.get_lineno(),
                            self.stream.get_charno(),
                        );
                    }
                    let param_name = self.stream.get_string();
                    name = Some(param_name.clone());
                    if is_bracketed_param {
                        token = self.next();
                        if token == JsDocToken::EQUALS {
                            token = self.next();
                            if token == JsDocToken::STRING {
                                token = self.next();
                            }
                        }
                        if token != JsDocToken::RIGHT_SQUARE {
                            self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_RB);
                        } else if let Some(t) = expr {
                            expr = Some(JSTypeExpression::make_optional_arg(ast, t));
                        }
                    }
                    if !TokenStream::is_js_identifier(&param_name) {
                        self.add_parser_warning_arg_at(
                            ast,
                            Msg::INVALID_VARIABLE_NAME,
                            &param_name,
                            lineno,
                            charno,
                        );
                        name = None;
                    } else if !self
                        .jsdoc_builder
                        .record_parameter(param_name.clone(), expr)
                    {
                        if self.jsdoc_builder.has_parameter(param_name.clone()) {
                            self.add_type_warning_arg_at(
                                ast,
                                Msg::DUP_VARIABLE_NAME,
                                &param_name,
                                lineno,
                                charno,
                            );
                        } else {
                            self.add_type_warning_arg_at(
                                ast,
                                Msg::JSDOC_INCOMPAT_TYPE,
                                &param_name,
                                lineno,
                                charno,
                            );
                        }
                    }
                }
                let Some(name) = name else {
                    return self.eat_until_eol_if_not_annotation_from(token);
                };
                self.jsdoc_builder.mark_name(
                    ast,
                    name.clone(),
                    Some(self.template_node),
                    lineno,
                    charno,
                );
                if self.jsdoc_builder.should_parse_documentation()
                    && token != JsDocToken::ANNOTATION
                {
                    let info = self.extract_multiline_textual_block(token);
                    if !info.string.is_empty() {
                        self.jsdoc_builder
                            .record_parameter_description(name, info.string);
                    }
                    token = info.token;
                } else if token != JsDocToken::EOC && token != JsDocToken::EOF {
                    token = self.eat_until_eol_if_not_annotation();
                }
                return token;
            }
            Annotation::NO_SIDE_EFFECTS => {
                if self.jsdoc_builder.is_modifies_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOSIDEEFFECTS_WITH_MODIFIES);
                } else if self.jsdoc_builder.is_throws_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOSIDEEFFECTS_WITH_THROWS);
                } else if !self.jsdoc_builder.record_no_side_effects() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOSIDEEFFECTS);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::MODIFIES => {
                let next = self.next();
                return self.parse_modifies_tag(ast, next);
            }
            Annotation::IMPLICIT_CAST => {
                if !self.jsdoc_builder.record_implicit_cast() {
                    self.add_type_warning(ast, Msg::JSDOC_IMPLICITCAST);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::SEE => {
                if self.jsdoc_builder.should_parse_documentation() {
                    let reference_info = self.extract_single_line_block();
                    let reference = reference_info.string;
                    if reference.is_empty() {
                        self.add_parser_warning(ast, Msg::JSDOC_SEEMISSING);
                    } else {
                        self.jsdoc_builder.record_reference(reference);
                    }
                    token = reference_info.token;
                } else {
                    token = self.eat_until_eol_if_not_annotation();
                }
                return token;
            }
            Annotation::SUPPRESS => {
                let next = self.next();
                return self.parse_suppress_tag(ast, next);
            }
            Annotation::TEMPLATE => {
                let mut bound_type_expression = None;
                if self.match_token(JsDocToken::LEFT_CURLY) {
                    self.add_parser_warning_at(
                        ast,
                        Msg::JSDOC_TEMPLATE_BOUNDEDGENERICS_USED,
                        lineno,
                        charno,
                    );
                    let next = self.next();
                    let node = self.parse_type_expression_annotation(ast, next);
                    if node.is_some() {
                        bound_type_expression = self.create_jstype_expression(ast, node);
                    }
                }
                let mut template_names = Vec::new();
                if !self.match_token(JsDocToken::COLON) {
                    loop {
                        let next = self.next();
                        let node = self.parse_name_expression(ast, next);
                        let name = node.map(|n| n.get_string(ast));
                        if Self::valid_template_type_name(name.as_ref()) {
                            template_names.push(name.unwrap());
                        }
                        if !self.eat_if_match(JsDocToken::COMMA) {
                            break;
                        }
                    }
                }
                let mut ttl_ast = None;
                if self.match_token(JsDocToken::COLON) {
                    let current = self.current();
                    let info = self.extract_multiline_textual_block_with_options(
                        current,
                        WhitespaceOption::TRIM,
                        false,
                    );
                    token = info.token;
                    ttl_ast = self.parse_ttl_ast(ast, info, lineno, charno);
                } else {
                    let current = self.current();
                    token = self.eat_until_eol_if_not_annotation_from(current);
                }
                match template_names.len() {
                    0 => {
                        self.add_type_warning_at(
                            ast,
                            Msg::JSDOC_TEMPLATE_NAME_MISSING,
                            lineno,
                            charno,
                        );
                        return token;
                    }
                    1 => {}
                    _ => {
                        if bound_type_expression.is_some() || ttl_ast.is_some() {
                            self.add_type_warning_at(
                                ast,
                                Msg::JSDOC_TEMPLATE_MULTIPLEDECLARATION,
                                lineno,
                                charno,
                            );
                        }
                    }
                }
                if bound_type_expression.is_some() && ttl_ast.is_some() {
                    self.add_type_warning_at(
                        ast,
                        Msg::JSDOC_TEMPLATE_BOUNDSWITHTTL,
                        lineno,
                        charno,
                    );
                    return token;
                }
                if let Some(ttl) = ttl_ast {
                    if !self
                        .jsdoc_builder
                        .record_type_transformation(template_names[0].clone(), ttl)
                    {
                        self.add_type_warning_at(
                            ast,
                            Msg::JSDOC_TEMPLATE_NAME_REDECLARATION,
                            lineno,
                            charno,
                        );
                    }
                } else if let Some(bound) = bound_type_expression {
                    if !self.jsdoc_builder.record_template_type_name_with_bound(
                        ast,
                        template_names[0].clone(),
                        Some(bound),
                    ) {
                        self.add_type_warning_at(
                            ast,
                            Msg::JSDOC_TEMPLATE_NAME_REDECLARATION,
                            lineno,
                            charno,
                        );
                    }
                } else {
                    for name in template_names {
                        if !self.jsdoc_builder.record_template_type_name(ast, name) {
                            self.add_type_warning_at(
                                ast,
                                Msg::JSDOC_TEMPLATE_NAME_REDECLARATION,
                                lineno,
                                charno,
                            );
                        }
                    }
                }
                return token;
            }
            Annotation::IDGENERATOR => {
                let next = self.next();
                return self.parse_id_generator_tag(ast, next);
            }
            Annotation::LOG_TYPE_IN_COMPILER => {
                self.jsdoc_builder.record_log_type_in_compiler();
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::NOT_IMPLEMENTED
            | Annotation::JSX
            | Annotation::JSX_FRAGMENT
            | Annotation::SOY_MODULE
            | Annotation::SOY_TEMPLATE
            | Annotation::WIZ_ANALYZER
            | Annotation::MAY_HAVE_EXTRA_EDGE => return self.eat_until_eol_if_not_annotation(),
            Annotation::USED_VIA_DOT_CONSTRUCTOR => {
                if !self.jsdoc_builder.record_used_via_dot_constructor() {
                    self.add_parser_warning(ast, Msg::JSDOC_USEDVIADOTCONSTRUCTOR);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::WIZACTION => {
                if !self.jsdoc_builder.record_wizaction() {
                    self.add_parser_warning(ast, Msg::JSDOC_WIZACTION);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::WIZCALLBACK => {
                if !self.jsdoc_builder.record_wizcallback() {
                    self.add_parser_warning(ast, Msg::JSDOC_WIZCALLBACK);
                }
                return self.eat_until_eol_if_not_annotation();
            }
            Annotation::CONSTANT
            | Annotation::FINAL
            | Annotation::DEFINE
            | Annotation::EXPORT
            | Annotation::RETURN
            | Annotation::PACKAGE
            | Annotation::PRIVATE
            | Annotation::PROTECTED
            | Annotation::PUBLIC
            | Annotation::THIS
            | Annotation::TYPE
            | Annotation::TYPEDEF => {
                lineno = self.stream.get_lineno();
                charno = self.stream.get_charno();
                let has_type = self.look_ahead_for_type();
                let is_alternate_type_annotation = matches!(
                    annotation,
                    Annotation::PACKAGE
                        | Annotation::PRIVATE
                        | Annotation::PROTECTED
                        | Annotation::PUBLIC
                        | Annotation::CONSTANT
                        | Annotation::FINAL
                        | Annotation::EXPORT
                );
                let can_skip_type_annotation =
                    is_alternate_type_annotation || annotation == Annotation::RETURN;
                let mut expr = None;
                if annotation == Annotation::RETURN && !has_type {
                    self.add_missing_type_warning(
                        ast,
                        self.stream.get_lineno(),
                        self.stream.get_charno(),
                    );
                }
                if has_type || !can_skip_type_annotation {
                    self.skip_eols();
                    token = self.next();
                    let mut node = self.parse_and_record_type_node(ast, token);
                    if annotation == Annotation::THIS {
                        node = self.wrap_node(ast, Token::BANG, node);
                    }
                    expr = self.create_jstype_expression(ast, node);
                }
                let has_error = expr.is_none() && !can_skip_type_annotation;
                if !has_error {
                    if ((expr.is_some() && is_alternate_type_annotation)
                        || annotation == Annotation::TYPE)
                        && !self.jsdoc_builder.record_type(expr.clone())
                    {
                        self.add_type_warning_at(ast, Msg::JSDOC_INCOMPAT_TYPE, lineno, charno);
                    }
                    let is_annotation_next = self.look_ahead_for_annotation();
                    match annotation {
                        Annotation::CONSTANT => {
                            if !self.jsdoc_builder.record_constancy() {
                                self.add_parser_warning(ast, Msg::JSDOC_CONST);
                            }
                        }
                        Annotation::FINAL => {
                            if !self.jsdoc_builder.record_finality() {
                                self.add_type_warning(ast, Msg::JSDOC_FINAL);
                            }
                        }
                        Annotation::DEFINE => {
                            if !self.jsdoc_builder.record_define_type(expr) {
                                self.add_parser_warning_at(ast, Msg::JSDOC_DEFINE, lineno, charno);
                            }
                            if !is_annotation_next {
                                return self.record_description(token);
                            }
                        }
                        Annotation::EXPORT => {
                            if !self.jsdoc_builder.record_export() {
                                self.add_parser_warning_at(ast, Msg::JSDOC_EXPORT, lineno, charno);
                            }
                            if !is_annotation_next {
                                return self.record_description(token);
                            }
                        }
                        Annotation::PRIVATE => {
                            if !self.jsdoc_builder.record_visibility(Visibility::PRIVATE) {
                                self.add_parser_warning_at(
                                    ast,
                                    Msg::JSDOC_EXTRA_VISIBILITY,
                                    lineno,
                                    charno,
                                );
                            }
                            if !is_annotation_next {
                                return self.record_description(token);
                            }
                        }
                        Annotation::PACKAGE => {
                            if !self.jsdoc_builder.record_visibility(Visibility::PACKAGE) {
                                self.add_parser_warning_at(
                                    ast,
                                    Msg::JSDOC_EXTRA_VISIBILITY,
                                    lineno,
                                    charno,
                                );
                            }
                            if !is_annotation_next {
                                return self.record_description(token);
                            }
                        }
                        Annotation::PROTECTED => {
                            if !self.jsdoc_builder.record_visibility(Visibility::PROTECTED) {
                                self.add_parser_warning_at(
                                    ast,
                                    Msg::JSDOC_EXTRA_VISIBILITY,
                                    lineno,
                                    charno,
                                );
                            }
                            if !is_annotation_next {
                                return self.record_description(token);
                            }
                        }
                        Annotation::PUBLIC => {
                            if !self.jsdoc_builder.record_visibility(Visibility::PUBLIC) {
                                self.add_parser_warning_at(
                                    ast,
                                    Msg::JSDOC_EXTRA_VISIBILITY,
                                    lineno,
                                    charno,
                                );
                            }
                            if !is_annotation_next {
                                return self.record_description(token);
                            }
                        }
                        Annotation::RETURN => {
                            if expr.is_none() {
                                let node = self.new_node(ast, Token::QMARK);
                                expr = self.create_jstype_expression(ast, Some(node));
                            }
                            if !self.jsdoc_builder.record_return_type(expr) {
                                self.add_type_warning_at(
                                    ast,
                                    Msg::JSDOC_INCOMPAT_TYPE,
                                    lineno,
                                    charno,
                                );
                            } else {
                                if self.jsdoc_builder.should_parse_documentation()
                                    && !is_annotation_next
                                {
                                    let info = self.extract_multiline_textual_block(token);
                                    if !info.string.is_empty() {
                                        self.jsdoc_builder.record_return_description(info.string);
                                    }
                                    token = info.token;
                                } else {
                                    token = self.eat_until_eol_if_not_annotation();
                                }
                                return token;
                            }
                        }
                        Annotation::THIS => {
                            if !self.jsdoc_builder.record_this_type(expr) {
                                self.add_type_warning_at(
                                    ast,
                                    Msg::JSDOC_INCOMPAT_TYPE,
                                    lineno,
                                    charno,
                                );
                            }
                        }
                        Annotation::TYPEDEF => {
                            if !self.jsdoc_builder.record_typedef(expr) {
                                self.add_type_warning_at(
                                    ast,
                                    Msg::JSDOC_INCOMPAT_TYPE,
                                    lineno,
                                    charno,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                return self.eat_until_eol_if_not_annotation();
            }
        }
    }

    // port: JsDocInfoParser#parseTtlAst
    fn parse_ttl_ast(
        &mut self,
        ast: &mut Ast,
        info: ExtractionInfo,
        lineno: i32,
        charno: i32,
    ) -> Option<NodeId> {
        let mut expression = info.string;
        if !expression.starts_with(TTL_START_DELIMITER) {
            return None;
        }
        expression = expression.substring_from(TTL_START_DELIMITER.len());
        let end_index = expression.index_of(TTL_END_DELIMITER);
        if end_index >= 0 {
            expression = expression.substring(0, end_index as usize);
        } else {
            self.add_type_warning_at(
                ast,
                Msg::JSDOC_TEMPLATE_TYPETRANSFORMATION_MISSINGDELIMIIER,
                lineno,
                charno,
            );
        }
        expression = java_lang::trim(&expression);
        if expression.is_empty() {
            self.add_type_warning_at(
                ast,
                Msg::JSDOC_TEMPLATE_TYPETRANSFORMATION_EXPRESSIONMISSING,
                lineno,
                charno,
            );
            return None;
        }
        let source_file = self.get_source_file(ast);
        let mut parser = TypeTransformationParser::new(
            expression,
            source_file,
            self.error_reporter,
            lineno,
            charno,
        );
        if !parser.parse_type_transformation(ast) {
            return None;
        }
        parser.get_type_transformation_ast()
    }
    // port: JsDocInfoParser#validTemplateTypeName
    fn valid_template_type_name(name: Option<&JsString>) -> bool {
        name.is_some_and(|n| {
            !n.is_empty()
                && n.as_units().iter().all(|&c| {
                    java_lang::is_letter_or_digit(c) || c == b'_' as u16 || c == b'$' as u16
                })
        })
    }
    // port: JsDocInfoParser#recordDescription
    fn record_description(&mut self, token: JsDocToken) -> JsDocToken {
        if self.jsdoc_builder.should_parse_documentation() {
            self.extract_multiline_textual_block(token).token
        } else {
            self.eat_tokens_until_eol_from(token)
        }
    }
    // port: JsDocInfoParser#checkExtendedTypes
    fn check_extended_types(&mut self, ast: &mut Ast, extended_types: &[ExtendedTypeInfo]) {
        if extended_types.len() > 1 {
            let is_interface = self.jsdoc_builder.is_interface_recorded();
            let is_from_tsickle = self.js_doc_source_kind == JsDocSourceKind::TSICKLE;
            if !is_interface && is_from_tsickle {
                let node = self.new_string_node(ast, TSICKLE_MISSING_TYPE_PLACEHOLDER.into());
                let node = self.wrap_node(ast, Token::BANG, Some(node)).unwrap();
                node.srcref_tree(ast, extended_types[0].r#type.as_ref().unwrap().get_root());
                let expr = self.create_jstype_expression(ast, Some(node));
                let result = self.jsdoc_builder.record_base_type(expr);
                check_state!(result, "Unexpected failure to record base type");
                return;
            }
        }
        for info in extended_types {
            if self.jsdoc_builder.is_interface_recorded() {
                if !self
                    .jsdoc_builder
                    .record_extended_interface(ast, info.r#type.clone())
                {
                    self.add_parser_warning_at(
                        ast,
                        Msg::JSDOC_EXTENDS_DUPLICATE,
                        info.lineno,
                        info.charno,
                    );
                }
            } else if !self.jsdoc_builder.record_base_type(info.r#type.clone()) {
                self.add_type_warning_at(ast, Msg::JSDOC_INCOMPAT_TYPE, info.lineno, info.charno);
            }
        }
    }
    // port: JsDocInfoParser#parseSuppressTag
    #[allow(unused_assignments)] // Preserve Java token consumption and assignments.
    fn parse_suppress_tag(&mut self, ast: &Ast, mut token: JsDocToken) -> JsDocToken {
        if token != JsDocToken::LEFT_CURLY {
            self.add_parser_warning(ast, Msg::JSDOC_SUPPRESS);
            return token;
        }
        let mut suppressions = IndexSet::<_>::default();
        loop {
            if self.match_token(JsDocToken::STRING) {
                let name = self.stream.get_string();
                if !self.suppression_names.contains(&name) {
                    self.add_parser_warning_arg(ast, Msg::JSDOC_SUPPRESS_UNKNOWN, &name);
                }
                suppressions.insert(self.stream.get_string());
                token = self.next();
            } else {
                self.add_parser_warning(ast, Msg::JSDOC_SUPPRESS);
                return token;
            }
            if self.match_two(JsDocToken::PIPE, JsDocToken::COMMA) {
                token = self.next();
            } else {
                break;
            }
        }
        token = self.next();
        if token != JsDocToken::RIGHT_CURLY {
            self.add_parser_warning(ast, Msg::JSDOC_SUPPRESS);
            return token;
        }
        if self.jsdoc_builder.should_parse_documentation() && !self.look_ahead_for_annotation() {
            let info = self.extract_multiline_textual_block(token);
            self.jsdoc_builder
                .record_suppressions_with_description(&suppressions, info.string);
            token = info.token;
        } else {
            let next = self.next();
            token = self.eat_until_eol_if_not_annotation_from(next);
            self.jsdoc_builder.record_suppressions(&suppressions);
        }
        token
    }
    // port: JsDocInfoParser#parseModsTag
    fn parse_mods_tag(&mut self, ast: &Ast, mut token: JsDocToken) -> JsDocToken {
        if token != JsDocToken::LEFT_CURLY || !self.match_token(JsDocToken::STRING) {
            self.add_parser_warning(ast, Msg::JSDOC_MODS);
            return token;
        }
        let namespace = self.stream.get_string();
        token = self.next();
        if !self.match_token(JsDocToken::RIGHT_CURLY) {
            self.add_parser_warning(ast, Msg::JSDOC_MODS);
        } else {
            token = self.next();
            self.jsdoc_builder.record_mods(namespace);
        }
        token
    }
    // port: JsDocInfoParser#parseClosurePrimitiveTag
    #[allow(unused_assignments)] // Preserve Java token consumption and assignments.
    fn parse_closure_primitive_tag(&mut self, ast: &Ast, mut token: JsDocToken) -> JsDocToken {
        if token != JsDocToken::LEFT_CURLY {
            self.add_parser_warning(ast, Msg::JSDOC_MISSING_LC);
            return token;
        } else if self.match_token(JsDocToken::STRING) {
            let name = self.stream.get_string();
            if !self.closure_primitive_names.contains(&name) {
                self.add_parser_warning_arg(ast, Msg::JSDOC_CLOSUREPRIMITIVE_INVALID, &name);
            } else if !self.jsdoc_builder.record_closure_primitive_id(name) {
                self.add_parser_warning(ast, Msg::JSDOC_CLOSUREPRIMITIVE_EXTRA);
            }
            token = self.next();
        } else {
            self.add_parser_warning(ast, Msg::JSDOC_CLOSUREPRIMITIVE_MISSING);
            return token;
        }
        if !self.match_token(JsDocToken::RIGHT_CURLY) {
            self.add_parser_warning(ast, Msg::JSDOC_MISSING_RC);
        } else {
            token = self.next();
        }
        self.eat_until_eol_if_not_annotation()
    }
    // port: JsDocInfoParser#parseModifiesTag
    fn parse_modifies_tag(&mut self, ast: &Ast, mut token: JsDocToken) -> JsDocToken {
        if token == JsDocToken::LEFT_CURLY {
            let mut modifies = IndexSet::<_>::default();
            loop {
                if self.match_token(JsDocToken::STRING) {
                    let name = self.stream.get_string();
                    if !MODIFIES_ANNOTATION_KEYWORDS.iter().any(|s| name == *s)
                        && !self.jsdoc_builder.has_parameter(name.clone())
                    {
                        self.add_parser_warning_arg(ast, Msg::JSDOC_MODIFIES_UNKNOWN, &name);
                    }
                    modifies.insert(self.stream.get_string());
                    token = self.next();
                } else {
                    self.add_parser_warning(ast, Msg::JSDOC_MODIFIES);
                    return token;
                }
                if self.match_token(JsDocToken::PIPE) {
                    token = self.next();
                } else {
                    break;
                }
            }
            if !self.match_token(JsDocToken::RIGHT_CURLY) {
                self.add_parser_warning(ast, Msg::JSDOC_MODIFIES);
            } else {
                token = self.next();
                if self.jsdoc_builder.is_no_side_effects_recorded() {
                    self.add_parser_warning(ast, Msg::JSDOC_NOSIDEEFFECTS_WITH_MODIFIES);
                } else if !self.jsdoc_builder.record_modifies(&modifies) {
                    self.add_parser_warning(ast, Msg::JSDOC_MODIFIES_DUPLICATE);
                }
            }
        }
        token
    }
    // port: JsDocInfoParser#parseIdGeneratorTag
    fn parse_id_generator_tag(&mut self, ast: &Ast, mut token: JsDocToken) -> JsDocToken {
        let mut idgen_kind = JsString::from("unique");
        if token == JsDocToken::LEFT_CURLY {
            if self.match_token(JsDocToken::STRING) {
                let name = self.stream.get_string();
                if !ID_GENERATOR_ANNOTATION_KEYWORDS.iter().any(|s| name == *s)
                    && !self.jsdoc_builder.has_parameter(name.clone())
                {
                    self.add_parser_warning_arg(ast, Msg::JSDOC_IDGEN_UNKNOWN, &name);
                }
                idgen_kind = name;
                token = self.next();
            } else {
                self.add_parser_warning(ast, Msg::JSDOC_IDGEN_BAD);
                return token;
            }
            if !self.match_token(JsDocToken::RIGHT_CURLY) {
                self.add_parser_warning(ast, Msg::JSDOC_IDGEN_BAD);
            } else {
                token = self.next();
            }
        }
        let recorded = match &idgen_kind {
            s if s == "unique" => self.jsdoc_builder.record_id_generator(),
            s if s == "consistent" => self.jsdoc_builder.record_consistent_id_generator(),
            s if s == "stable" => self.jsdoc_builder.record_stable_id_generator(),
            s if s == "xid" => self.jsdoc_builder.record_xid_generator(),
            s if s == "mapped" => self.jsdoc_builder.record_mapped_id_generator(),
            _ => true,
        };
        if !recorded {
            self.add_parser_warning(ast, Msg::JSDOC_IDGEN_DUPLICATE);
        }
        token
    }
    // port: JsDocInfoParser#parseAndRecordTypeNode(JsDocToken)
    pub fn parse_and_record_type_node(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
    ) -> Option<NodeId> {
        self.parse_and_record_type_node_at(
            ast,
            token,
            self.stream.get_lineno(),
            self.stream.get_charno(),
            token == JsDocToken::LEFT_CURLY,
            false,
        )
    }
    // port: JsDocInfoParser#parseAndRecordTypeNode(JsDocToken, int, int, boolean, boolean)
    fn parse_and_record_type_node_at(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
        lineno: i32,
        start_charno: i32,
        matching_lc: bool,
        only_parse_simple_names: bool,
    ) -> Option<NodeId> {
        let node = if only_parse_simple_names {
            self.parse_type_name_annotation(ast, token)
        } else {
            self.parse_type_expression_annotation(ast, token)
        };
        self.record_type_node(lineno, start_charno, node, matching_lc);
        node
    }
    // port: JsDocInfoParser#parseAndRecordTypeNameNode
    fn parse_and_record_type_name_node(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
        lineno: i32,
        start_charno: i32,
        matching_lc: bool,
    ) -> Option<NodeId> {
        self.parse_and_record_type_node_at(ast, token, lineno, start_charno, matching_lc, true)
    }
    // port: JsDocInfoParser#parseAndRecordParamTypeNode
    fn parse_and_record_param_type_node(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
    ) -> Option<NodeId> {
        check_argument!(token == JsDocToken::LEFT_CURLY);
        let lineno = self.stream.get_lineno();
        let start_charno = self.stream.get_charno();
        let node = self.parse_param_type_expression_annotation(ast, token);
        self.record_type_node(lineno, start_charno, node, true);
        node
    }
    // port: JsDocInfoParser#toString
    fn to_string(&self, token: JsDocToken) -> JsString {
        match token {
            JsDocToken::ANNOTATION => JsString::from("@").concat(&self.stream.get_string()),
            JsDocToken::STRING => self.stream.get_string(),
            JsDocToken::BANG => "!".into(),
            JsDocToken::COMMA => ",".into(),
            JsDocToken::COLON => ":".into(),
            JsDocToken::RIGHT_ANGLE => ">".into(),
            JsDocToken::LEFT_SQUARE => "[".into(),
            JsDocToken::LEFT_CURLY => "{".into(),
            JsDocToken::LEFT_PAREN => "(".into(),
            JsDocToken::LEFT_ANGLE => "<".into(),
            JsDocToken::QMARK => "?".into(),
            JsDocToken::PIPE => "|".into(),
            JsDocToken::RIGHT_SQUARE => "]".into(),
            JsDocToken::RIGHT_CURLY => "}".into(),
            JsDocToken::RIGHT_PAREN => ")".into(),
            JsDocToken::STAR => "*".into(),
            JsDocToken::ITER_REST => "...".into(),
            JsDocToken::EQUALS => "=".into(),
            _ => panic!("{token:?}"),
        }
    }
    // port: JsDocInfoParser#createJSTypeExpression
    pub fn create_jstype_expression(
        &self,
        ast: &Ast,
        node: Option<NodeId>,
    ) -> Option<Arc<JSTypeExpression>> {
        node.map(|n| Arc::new(JSTypeExpression::new(n, self.get_source_name(ast))))
    }
    // port: JsDocInfoParser#extractSingleLineBlock
    fn extract_single_line_block(&mut self) -> ExtractionInfo {
        self.stream.update();
        let lineno = self.stream.get_lineno();
        let charno = self.stream.get_charno().wrapping_add(1);
        let line = java_lang::trim(&self.get_remaining_jsdoc_line());
        if !line.is_empty() {
            self.jsdoc_builder.mark_text(
                line.clone(),
                lineno,
                charno,
                lineno,
                charno.wrapping_add(line.length() as i32),
            );
        }
        ExtractionInfo::new(line, self.next())
    }
    // port: JsDocInfoParser#extractMultilineTextualBlock(JsDocToken)
    fn extract_multiline_textual_block(&mut self, token: JsDocToken) -> ExtractionInfo {
        self.extract_multiline_textual_block_with_options(
            token,
            self.get_whitespace_option(WhitespaceOption::SINGLE_LINE),
            false,
        )
    }
    // port: JsDocInfoParser#extractMultilineTextualBlock(JsDocToken, WhitespaceOption, boolean)
    fn extract_multiline_textual_block_with_options(
        &mut self,
        token: JsDocToken,
        option: WhitespaceOption,
        include_annotations: bool,
    ) -> ExtractionInfo {
        if matches!(token, JsDocToken::EOC | JsDocToken::EOL | JsDocToken::EOF) {
            return ExtractionInfo::new("", token);
        }
        self.extract_multiline_comment(token, option, true, include_annotations)
    }
    // port: JsDocInfoParser#getWhitespaceOption
    fn get_whitespace_option(&self, default_value: WhitespaceOption) -> WhitespaceOption {
        if self.preserve_whitespace {
            WhitespaceOption::PRESERVE
        } else {
            default_value
        }
    }
    // port: JsDocInfoParser#extractBlockComment
    fn extract_block_comment(&mut self, token: JsDocToken) -> ExtractionInfo {
        self.extract_multiline_comment(
            token,
            self.get_whitespace_option(WhitespaceOption::TRIM),
            false,
            false,
        )
    }
    // port: JsDocInfoParser#extractMultilineComment
    fn extract_multiline_comment(
        &mut self,
        mut token: JsDocToken,
        option: WhitespaceOption,
        is_marker: bool,
        include_annotations: bool,
    ) -> ExtractionInfo {
        let mut builder = Vec::<u16>::new();
        let mut start_lineno = -1;
        let mut start_charno = -1;
        if is_marker {
            self.stream.update();
            start_lineno = self.stream.get_lineno();
            start_charno = self.stream.get_charno().wrapping_add(1);
            let mut line = self.get_remaining_jsdoc_line();
            if option != WhitespaceOption::PRESERVE {
                line = java_lang::trim(&line);
            }
            builder.extend_from_slice(line.as_units());
            self.state = State::SEARCHING_ANNOTATION;
            token = self.next();
        }
        let mut ignore_star = false;
        let mut line_start_char = -1;
        loop {
            match token {
                JsDocToken::STAR => {
                    if ignore_star {
                        line_start_char = self.stream.get_charno().wrapping_add(1);
                        ignore_star = false;
                    } else {
                        self.pad_line(&mut builder, line_start_char, option);
                        line_start_char = -1;
                        builder.push(b'*' as u16);
                    }
                    token = self.next();
                    while token == JsDocToken::STAR {
                        if line_start_char != -1 {
                            self.pad_line(&mut builder, line_start_char, option);
                            line_start_char = -1;
                        }
                        builder.push(b'*' as u16);
                        token = self.next();
                    }
                }
                JsDocToken::EOL => {
                    if option != WhitespaceOption::SINGLE_LINE {
                        builder.push(b'\n' as u16);
                    }
                    ignore_star = true;
                    line_start_char = 0;
                    token = self.next();
                }
                _ => {
                    ignore_star = false;
                    self.state = State::SEARCHING_ANNOTATION;
                    if token != JsDocToken::EOC {
                        self.pad_line(&mut builder, line_start_char, option);
                        line_start_char = -1;
                    }
                    if matches!(token, JsDocToken::EOC | JsDocToken::EOF)
                        || (token == JsDocToken::ANNOTATION && !include_annotations)
                    {
                        let mut text = JsString::from_units(builder);
                        if option != WhitespaceOption::PRESERVE {
                            text = java_lang::trim(&text);
                        }
                        if is_marker && !text.is_empty() {
                            self.jsdoc_builder.mark_text(
                                text.clone(),
                                start_lineno,
                                start_charno,
                                self.stream.get_lineno(),
                                self.stream.get_charno(),
                            );
                        }
                        return ExtractionInfo::new(text, token);
                    }
                    builder.extend_from_slice(self.to_string(token).as_units());
                    let mut line = self.get_remaining_jsdoc_line();
                    if option != WhitespaceOption::PRESERVE {
                        line = Self::trim_end(line);
                    }
                    builder.extend_from_slice(line.as_units());
                    token = self.next();
                }
            }
        }
    }
    // port: JsDocInfoParser#padLine
    fn pad_line(&self, builder: &mut Vec<u16>, line_start_char: i32, option: WhitespaceOption) {
        if line_start_char != -1 && option == WhitespaceOption::PRESERVE {
            let num_spaces = self.stream.get_charno().wrapping_sub(line_start_char);
            for _ in 0..num_spaces {
                builder.push(b' ' as u16);
            }
        } else if !builder.is_empty()
            && (builder.last() != Some(&(b'\n' as u16)) || option == WhitespaceOption::PRESERVE)
        {
            builder.push(b' ' as u16);
        }
    }
    // port: JsDocInfoParser#trimEnd
    fn trim_end(s: JsString) -> JsString {
        let mut trim_count = 0;
        while trim_count < s.length() {
            let ch = s.char_at(s.length() - trim_count - 1);
            if TokenUtil::is_whitespace(ch as i32) {
                trim_count += 1;
            } else {
                break;
            }
        }
        if trim_count == 0 {
            s
        } else {
            s.substring(0, s.length() - trim_count)
        }
    }
    // port: JsDocInfoParser#parseTypeExpressionAnnotation
    fn parse_type_expression_annotation(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
    ) -> Option<NodeId> {
        if token == JsDocToken::LEFT_CURLY {
            self.skip_eols();
            let next = self.next();
            let node = self.parse_top_level_type_expression(ast, next);
            if let Some(n) = node {
                self.skip_eols();
                if !self.match_token(JsDocToken::RIGHT_CURLY) {
                    self.report_type_syntax_warning(
                        ast,
                        if n.is_string_lit(ast) && n.get_string_ref(ast) == "import" {
                            Msg::JSDOC_IMPORT
                        } else {
                            Msg::JSDOC_MISSING_RC
                        },
                    );
                } else {
                    self.next();
                }
            }
            node
        } else {
            self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_BRACES);
            self.parse_type_expression(ast, token)
        }
    }
    // port: JsDocInfoParser#parseParamTypeExpression
    fn parse_param_type_expression(
        &mut self,
        ast: &mut Ast,
        mut token: JsDocToken,
    ) -> Option<NodeId> {
        let mut rest_arg = false;
        if token == JsDocToken::ITER_REST {
            token = self.next();
            if token == JsDocToken::RIGHT_CURLY {
                self.restore_look_ahead(token);
                let empty = self.new_node(ast, Token::EMPTY);
                return self.wrap_node(ast, Token::ITER_REST, Some(empty));
            }
            rest_arg = true;
        }
        let mut node = self.parse_top_level_type_expression(ast, token);
        if node.is_some() {
            self.skip_eols();
            if rest_arg {
                node = self.wrap_node(ast, Token::ITER_REST, node);
            } else if self.match_token(JsDocToken::EQUALS) {
                self.next();
                self.skip_eols();
                node = self.wrap_node(ast, Token::EQUALS, node);
            }
        }
        node
    }
    // port: JsDocInfoParser#parseParamTypeExpressionAnnotation
    fn parse_param_type_expression_annotation(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
    ) -> Option<NodeId> {
        check_argument!(token == JsDocToken::LEFT_CURLY);
        self.skip_eols();
        let next = self.next();
        let node = self.parse_param_type_expression(ast, next);
        if node.is_some() {
            if !self.match_token(JsDocToken::RIGHT_CURLY) {
                self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_RC);
            } else {
                self.next();
            }
        }
        node
    }
    // port: JsDocInfoParser#parseTypeNameAnnotation
    fn parse_type_name_annotation(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        if token == JsDocToken::LEFT_CURLY {
            self.skip_eols();
            let next = self.next();
            let node = self.parse_type_name(ast, next);
            if node.is_some() {
                self.skip_eols();
                if !self.match_token(JsDocToken::RIGHT_CURLY) {
                    self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_RC);
                } else {
                    self.next();
                }
            }
            node
        } else {
            self.parse_type_name(ast, token)
        }
    }
    // port: JsDocInfoParser#parseTopLevelTypeExpression
    fn parse_top_level_type_expression(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
    ) -> Option<NodeId> {
        let expr = self.parse_type_expression(ast, token);
        if expr.is_some() && self.match_token(JsDocToken::PIPE) {
            self.next();
            self.skip_eols();
            let next = self.next();
            return self.parse_union_type_with_alternate(ast, next, expr);
        }
        expr
    }
    // port: JsDocInfoParser#parseTypeExpressionList
    fn parse_type_expression_list(
        &mut self,
        ast: &mut Ast,
        type_name: JsString,
        token: JsDocToken,
    ) -> Option<NodeId> {
        let mut expr = self.parse_top_level_type_expression(ast, token)?;
        let list = self.new_node(ast, Token::BLOCK);
        let mut num_type_exprs = 1;
        list.add_child_to_back(ast, expr);
        while self.match_token(JsDocToken::COMMA) {
            self.next();
            self.skip_eols();
            let next = self.next();
            expr = self.parse_top_level_type_expression(ast, next)?;
            num_type_exprs += 1;
            list.add_child_to_back(ast, expr);
        }
        if type_name == "Object" && num_type_exprs == 1 {
            let unknown = self.new_node(ast, Token::QMARK);
            list.add_child_to_front(ast, unknown);
        }
        Some(list)
    }
    // port: JsDocInfoParser#parseTypeExpression
    fn parse_type_expression(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        let mut lineno = self.stream.get_lineno();
        let mut charno = self.stream.get_charno();
        match token {
            JsDocToken::QMARK => {
                let next = self.next();
                if matches!(
                    next,
                    JsDocToken::COMMA
                        | JsDocToken::EQUALS
                        | JsDocToken::RIGHT_SQUARE
                        | JsDocToken::RIGHT_CURLY
                        | JsDocToken::RIGHT_PAREN
                        | JsDocToken::PIPE
                        | JsDocToken::RIGHT_ANGLE
                        | JsDocToken::EOC
                        | JsDocToken::EOL
                        | JsDocToken::EOF
                ) {
                    self.restore_look_ahead(next);
                    return Some(self.new_node(ast, Token::QMARK));
                }
                let basic = self.parse_basic_type_expression(ast, next);
                self.wrap_node_at(ast, Token::QMARK, basic, lineno, charno)
            }
            JsDocToken::BANG => {
                let next = self.next();
                let basic = self.parse_basic_type_expression(ast, next);
                self.wrap_node_at(ast, Token::BANG, basic, lineno, charno)
            }
            _ => {
                let basic = self.parse_basic_type_expression(ast, token);
                lineno = self.stream.get_lineno();
                charno = self.stream.get_charno();
                if basic.is_some() {
                    if self.match_token(JsDocToken::QMARK) {
                        self.next();
                        return self.wrap_node_at(ast, Token::QMARK, basic, lineno, charno);
                    } else if self.match_token(JsDocToken::BANG) {
                        self.next();
                        return self.wrap_node_at(ast, Token::BANG, basic, lineno, charno);
                    }
                }
                basic
            }
        }
    }
    // port: JsDocInfoParser#parseContextTypeExpression
    fn parse_context_type_expression(
        &mut self,
        ast: &mut Ast,
        token: JsDocToken,
    ) -> Option<NodeId> {
        if token == JsDocToken::QMARK {
            Some(self.new_node(ast, Token::QMARK))
        } else {
            self.parse_basic_type_expression(ast, token)
        }
    }
    // port: JsDocInfoParser#parseBasicTypeExpression
    fn parse_basic_type_expression(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        if token == JsDocToken::STAR {
            return Some(self.new_node(ast, Token::STAR));
        } else if token == JsDocToken::LEFT_CURLY {
            self.skip_eols();
            let next = self.next();
            return self.parse_record_type(ast, next);
        } else if token == JsDocToken::LEFT_PAREN {
            self.skip_eols();
            let next = self.next();
            return self.parse_union_type(ast, next);
        } else if token == JsDocToken::STRING {
            let string = self.stream.get_string();
            return match &string {
                s if s == "function" => {
                    self.skip_eols();
                    let next = self.next();
                    self.parse_function_type(ast, next)
                }
                s if s == "null" || s == "undefined" => Some(self.new_string_node(ast, string)),
                s if s == "typeof" => {
                    self.skip_eols();
                    let next = self.next();
                    self.parse_typeof_type(ast, next)
                }
                _ => self.parse_type_name(ast, token),
            };
        }
        self.restore_look_ahead(token);
        self.report_generic_type_syntax_warning(ast)
    }
    // port: JsDocInfoParser#parseNameExpression
    fn parse_name_expression(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        if token != JsDocToken::STRING {
            self.add_parser_warning_at(
                ast,
                Msg::JSDOC_NAME_SYNTAX,
                self.stream.get_lineno(),
                self.stream.get_charno(),
            );
            return None;
        }
        let start_lineno = self.stream.get_lineno();
        let start_charno = self.stream.get_charno();
        let start_offset = self
            .stream
            .get_cursor()
            .wrapping_sub(self.stream.get_string().length() as i32);
        let mut type_name = self.stream.get_string();
        let mut end_offset = self.stream.get_cursor();
        while self.match_token(JsDocToken::EOL) && type_name.ends_with(".") {
            self.skip_eols();
            if self.match_token(JsDocToken::STRING) {
                self.next();
                end_offset = self.stream.get_cursor();
                type_name = type_name.concat(&self.stream.get_string());
            }
        }
        let n = self.new_string_node(ast, type_name);
        n.set_lineno_charno(ast, start_lineno, start_charno);
        n.set_length(ast, end_offset.wrapping_sub(start_offset));
        Some(n)
    }
    // port: JsDocInfoParser#parseTypeName
    fn parse_type_name(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        let type_name = self.parse_name_expression(ast, token);
        if self.match_token(JsDocToken::LEFT_ANGLE) {
            self.next();
            self.skip_eols();
            let next = self.next();
            let member =
                self.parse_type_expression_list(ast, type_name.unwrap().get_string(ast), next);
            if let Some(member) = member {
                type_name.unwrap().add_child_to_front(ast, member);
                self.skip_eols();
                if !self.match_token(JsDocToken::RIGHT_ANGLE) {
                    return self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_GT);
                }
                self.next();
            }
        }
        type_name
    }
    // port: JsDocInfoParser#parseTypeofType
    fn parse_typeof_type(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        if token == JsDocToken::LEFT_CURLY {
            return self.report_type_syntax_warning(ast, Msg::JSDOC_UNNECESSARY_BRACES);
        }
        let typeof_type = self.new_node(ast, Token::TYPEOF);
        let name = self.parse_name_expression(ast, token)?;
        self.skip_eols();
        typeof_type.add_child_to_front(ast, name);
        Some(typeof_type)
    }
    // port: JsDocInfoParser#parseFunctionType
    fn parse_function_type(&mut self, ast: &mut Ast, mut token: JsDocToken) -> Option<NodeId> {
        if token != JsDocToken::LEFT_PAREN {
            self.restore_look_ahead(token);
            return self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_LP);
        }
        let function_type = self.new_node(ast, Token::FUNCTION);
        let mut parameters = None;
        self.skip_eols();
        if !self.match_token(JsDocToken::RIGHT_PAREN) {
            token = self.next();
            let mut has_params = true;
            if token == JsDocToken::STRING {
                let string = self.stream.get_string();
                let is_this = string == "this";
                let is_new = string == "new";
                if is_this || is_new {
                    if self.match_token(JsDocToken::COLON) {
                        self.next();
                        self.skip_eols();
                        let next = self.next();
                        let context_type = self.parse_context_type_expression(ast, next);
                        let context_type = self.wrap_node(
                            ast,
                            if is_this { Token::THIS } else { Token::NEW },
                            context_type,
                        )?;
                        function_type.add_child_to_front(ast, context_type);
                    } else {
                        return self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_COLON);
                    }
                    if self.match_token(JsDocToken::COMMA) {
                        self.next();
                        self.skip_eols();
                        token = self.next();
                    } else {
                        has_params = false;
                    }
                }
            }
            if has_params {
                parameters = Some(self.parse_parameters_type(ast, token)?);
            }
        }
        if let Some(p) = parameters {
            function_type.add_child_to_back(ast, p);
        }
        self.skip_eols();
        if !self.match_token(JsDocToken::RIGHT_PAREN) {
            return self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_RP);
        }
        self.skip_eols();
        self.next();
        let result = self.parse_result_type(ast)?;
        function_type.add_child_to_back(ast, result);
        Some(function_type)
    }
    // port: JsDocInfoParser#parseParametersType
    fn parse_parameters_type(&mut self, ast: &mut Ast, mut token: JsDocToken) -> Option<NodeId> {
        let params = self.new_node(ast, Token::PARAM_LIST);
        let mut is_var_args = false;
        let mut param_type = None;
        if token != JsDocToken::RIGHT_PAREN {
            loop {
                if param_type.is_some() {
                    self.next();
                    self.skip_eols();
                    token = self.next();
                }
                if token == JsDocToken::ITER_REST {
                    self.skip_eols();
                    if self.match_token(JsDocToken::RIGHT_PAREN) {
                        param_type = Some(self.new_node(ast, Token::ITER_REST));
                    } else {
                        self.skip_eols();
                        let next = self.next();
                        let expr = self.parse_type_expression(ast, next);
                        param_type = self.wrap_node(ast, Token::ITER_REST, expr);
                        self.skip_eols();
                    }
                    is_var_args = true;
                } else {
                    param_type = self.parse_type_expression(ast, token);
                    if self.match_token(JsDocToken::EQUALS) {
                        self.skip_eols();
                        self.next();
                        param_type = self.wrap_node(ast, Token::EQUALS, param_type);
                    }
                }
                let p = param_type?;
                params.add_child_to_back(ast, p);
                if is_var_args || !self.match_token(JsDocToken::COMMA) {
                    break;
                }
            }
        }
        if is_var_args && self.match_token(JsDocToken::COMMA) {
            return self.report_type_syntax_warning(ast, Msg::JSDOC_FUNCTION_VARARGS);
        }
        Some(params)
    }
    // port: JsDocInfoParser#parseResultType
    fn parse_result_type(&mut self, ast: &mut Ast) -> Option<NodeId> {
        self.skip_eols();
        if !self.match_token(JsDocToken::COLON) {
            return Some(self.new_node(ast, Token::EMPTY));
        }
        self.next();
        self.skip_eols();
        if self.match_token(JsDocToken::STRING) && self.stream.get_string() == "void" {
            self.next();
            Some(self.new_node(ast, Token::VOID))
        } else {
            let next = self.next();
            self.parse_type_expression(ast, next)
        }
    }
    // port: JsDocInfoParser#parseUnionType
    fn parse_union_type(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        self.parse_union_type_with_alternate(ast, token, None)
    }
    // port: JsDocInfoParser#parseUnionTypeWithAlternate
    fn parse_union_type_with_alternate(
        &mut self,
        ast: &mut Ast,
        mut token: JsDocToken,
        alternate: Option<NodeId>,
    ) -> Option<NodeId> {
        let union = self.new_node(ast, Token::PIPE);
        if let Some(n) = alternate {
            union.add_child_to_back(ast, n);
        }
        let mut expr = None;
        loop {
            if expr.is_some() {
                self.skip_eols();
                token = self.next();
                check_state!(token == JsDocToken::PIPE);
                self.skip_eols();
                token = self.next();
            }
            expr = self.parse_type_expression(ast, token);
            union.add_child_to_back(ast, expr?);
            if !self.match_token(JsDocToken::PIPE) {
                break;
            }
        }
        if alternate.is_none() {
            self.skip_eols();
            if !self.match_token(JsDocToken::RIGHT_PAREN) {
                return self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_RP);
            }
            self.next();
        }
        if union.has_one_child(ast) {
            let first = union.get_first_child(ast).unwrap();
            first.detach(ast);
            return Some(first);
        }
        Some(union)
    }
    // port: JsDocInfoParser#parseRecordType
    fn parse_record_type(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        let record = self.new_node(ast, Token::LC);
        let Some(fields) = self.parse_field_type_list(ast, token) else {
            return self.report_generic_type_syntax_warning(ast);
        };
        self.skip_eols();
        if !self.match_token(JsDocToken::RIGHT_CURLY) {
            return self.report_type_syntax_warning(ast, Msg::JSDOC_MISSING_RC);
        }
        self.next();
        record.add_child_to_back(ast, fields);
        Some(record)
    }
    // port: JsDocInfoParser#parseFieldTypeList
    fn parse_field_type_list(&mut self, ast: &mut Ast, mut token: JsDocToken) -> Option<NodeId> {
        let list = self.new_node(ast, Token::LB);
        let mut names = IndexSet::<_>::default();
        loop {
            let field = self.parse_field_type(ast, token)?;
            let name = if field.is_string_key(ast) {
                field.get_string(ast)
            } else {
                field.get_first_child(ast).unwrap().get_string(ast)
            };
            if names.insert(name.clone()) {
                list.add_child_to_back(ast, field);
            } else {
                self.add_type_warning_arg(ast, Msg::JSDOC_TYPE_RECORD_DUPLICATE, &name);
            }
            self.skip_eols();
            if !self.match_token(JsDocToken::COMMA) {
                break;
            }
            self.next();
            self.skip_eols();
            if self.match_token(JsDocToken::RIGHT_CURLY) {
                break;
            }
            token = self.next();
        }
        Some(list)
    }
    // port: JsDocInfoParser#parseFieldType
    fn parse_field_type(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        let field_name = self.parse_field_name(ast, token)?;
        self.skip_eols();
        if !self.match_token(JsDocToken::COLON) {
            return Some(field_name);
        }
        self.next();
        self.skip_eols();
        let next = self.next();
        let expr = self.parse_type_expression(ast, next)?;
        let field = self.new_node(ast, Token::COLON);
        field.add_child_to_back(ast, field_name);
        field.add_child_to_back(ast, expr);
        Some(field)
    }
    // port: JsDocInfoParser#parseFieldName
    fn parse_field_name(&mut self, ast: &mut Ast, token: JsDocToken) -> Option<NodeId> {
        if token != JsDocToken::STRING {
            return None;
        }
        let s = self.stream.get_string();
        let n = ast.new_string_with_token(Token::STRING_KEY, s.clone());
        n.set_lineno_charno(ast, self.stream.get_lineno(), self.stream.get_charno());
        n.clone_props_from(ast, self.template_node);
        n.set_length(ast, s.length() as i32);
        Some(n)
    }
    // port: JsDocInfoParser#wrapNode(Token, Node)
    fn wrap_node(&self, ast: &mut Ast, token: Token, n: Option<NodeId>) -> Option<NodeId> {
        n.and_then(|n| self.wrap_node_at(ast, token, Some(n), n.get_lineno(ast), n.get_charno(ast)))
    }
    // port: JsDocInfoParser#wrapNode(Token, Node, int, int)
    fn wrap_node_at(
        &self,
        ast: &mut Ast,
        token: Token,
        n: Option<NodeId>,
        lineno: i32,
        charno: i32,
    ) -> Option<NodeId> {
        n.map(|n| {
            let wrapped = ast.new_node_with_child(token, n);
            wrapped.set_lineno_charno(ast, lineno, charno);
            wrapped.clone_props_from(ast, self.template_node);
            wrapped
        })
    }
    // port: JsDocInfoParser#newNode
    fn new_node(&self, ast: &mut Ast, token: Token) -> NodeId {
        let n = ast.new_node(token);
        n.set_lineno_charno(ast, self.stream.get_lineno(), self.stream.get_charno());
        n.clone_props_from(ast, self.template_node);
        n
    }
    // port: JsDocInfoParser#newStringNode
    fn new_string_node(&self, ast: &mut Ast, s: JsString) -> NodeId {
        let n = ast.new_string(s.clone());
        n.set_lineno_charno(ast, self.stream.get_lineno(), self.stream.get_charno());
        n.clone_props_from(ast, self.template_node);
        n.set_length(ast, s.length() as i32);
        n
    }
    // port: JsDocInfoParser#reportTypeSyntaxWarning
    fn report_type_syntax_warning(&mut self, ast: &Ast, warning: Msg) -> Option<NodeId> {
        self.add_type_warning_at(
            ast,
            warning,
            self.stream.get_lineno(),
            self.stream.get_charno(),
        );
        None
    }
    // port: JsDocInfoParser#reportGenericTypeSyntaxWarning
    fn report_generic_type_syntax_warning(&mut self, ast: &Ast) -> Option<NodeId> {
        self.report_type_syntax_warning(ast, Msg::JSDOC_TYPE_SYNTAX)
    }
    // port: JsDocInfoParser#eatUntilEOLIfNotAnnotation()
    fn eat_until_eol_if_not_annotation(&mut self) -> JsDocToken {
        let next = self.next();
        self.eat_until_eol_if_not_annotation_from(next)
    }
    // port: JsDocInfoParser#eatUntilEOLIfNotAnnotation(JsDocToken)
    fn eat_until_eol_if_not_annotation_from(&mut self, token: JsDocToken) -> JsDocToken {
        if token == JsDocToken::ANNOTATION {
            self.state = State::SEARCHING_ANNOTATION;
            token
        } else {
            self.eat_tokens_until_eol_from(token)
        }
    }
    // port: JsDocInfoParser#eatTokensUntilEOL()
    fn eat_tokens_until_eol(&mut self) -> JsDocToken {
        let next = self.next();
        self.eat_tokens_until_eol_from(next)
    }
    // port: JsDocInfoParser#eatTokensUntilEOL(JsDocToken)
    fn eat_tokens_until_eol_from(&mut self, mut token: JsDocToken) -> JsDocToken {
        loop {
            if matches!(token, JsDocToken::EOL | JsDocToken::EOC | JsDocToken::EOF) {
                self.state = State::SEARCHING_ANNOTATION;
                return token;
            }
            token = self.next();
        }
    }
    // port: JsDocInfoParser#restoreLookAhead
    fn restore_look_ahead(&mut self, token: JsDocToken) {
        self.unread_token = Some(token);
    }
    // port: JsDocInfoParser#match(JsDocToken)
    fn match_token(&mut self, token: JsDocToken) -> bool {
        self.unread_token = Some(self.next());
        self.unread_token == Some(token)
    }
    // port: JsDocInfoParser#match(JsDocToken, JsDocToken)
    fn match_two(&mut self, token1: JsDocToken, token2: JsDocToken) -> bool {
        self.unread_token = Some(self.next());
        self.unread_token == Some(token1) || self.unread_token == Some(token2)
    }
    // port: JsDocInfoParser#eatIfMatch
    fn eat_if_match(&mut self, token: JsDocToken) -> bool {
        if self.match_token(token) {
            self.next();
            true
        } else {
            false
        }
    }
    // port: JsDocInfoParser#next
    fn next(&mut self) -> JsDocToken {
        if self.unread_token == NO_UNREAD_TOKEN {
            self.stream.get_js_doc_token()
        } else {
            self.current()
        }
    }
    // port: JsDocInfoParser#current
    fn current(&mut self) -> JsDocToken {
        self.unread_token.take().unwrap()
    }
    // port: JsDocInfoParser#skipEOLs
    fn skip_eols(&mut self) {
        while self.match_token(JsDocToken::EOL) {
            self.next();
            if self.match_token(JsDocToken::STAR) {
                self.next();
            }
        }
    }
    // port: JsDocInfoParser#getRemainingJSDocLine
    fn get_remaining_jsdoc_line(&mut self) -> JsString {
        let result = self.stream.get_remaining_js_doc_line();
        self.unread_token = NO_UNREAD_TOKEN;
        result
    }
    // port: JsDocInfoParser#hasParsedFileOverviewDocInfo
    fn has_parsed_file_overview_doc_info(&self) -> bool {
        self.jsdoc_builder.is_populated_with_file_overview()
    }
    // port: JsDocInfoParser#retrieveAndResetParsedJSDocInfo
    pub fn retrieve_and_reset_parsed_jsdoc_info(&mut self) -> Option<Arc<JSDocInfo>> {
        self.jsdoc_builder.build()
    }
    // port: JsDocInfoParser#getFileOverviewJSDocInfo
    pub fn get_file_overview_jsdoc_info(&self) -> Option<Arc<JSDocInfo>> {
        self.file_overview_jsdoc_info.clone()
    }
    // port: JsDocInfoParser#lookAheadForType
    fn look_ahead_for_type(&mut self) -> bool {
        self.look_ahead_for(b'{' as u16)
    }
    // port: JsDocInfoParser#lookAheadForAnnotation
    fn look_ahead_for_annotation(&mut self) -> bool {
        self.look_ahead_for(b'@' as u16)
    }
    // port: JsDocInfoParser#lookAheadFor
    fn look_ahead_for(&mut self, expect: u16) -> bool {
        let matched;
        let mut c;
        loop {
            c = self.stream.get_char();
            if c == b' ' as i32 {
                continue;
            }
            matched = c == expect as i32;
            break;
        }
        self.stream.unget_char(c);
        matched
    }
    // port: JsDocInfoParser#parseClosureUnawareCode
    fn parse_closure_unaware_code(&mut self, ast: &Ast) -> JsDocToken {
        self.add_parser_warning(ast, Msg::JSDOC_CLOSURE_UNAWARE_CODE_INVALID);
        if !self.match_token(JsDocToken::LEFT_CURLY) {
            if !self.jsdoc_builder.record_closure_unaware_code() {
                self.add_parser_warning(ast, Msg::JSDOC_CLOSURE_UNAWARE_CODE_EXTRA);
            }
            let current = self.current();
            return self.eat_until_eol_if_not_annotation_from(current);
        }
        let token = self.next();
        if self.match_token(JsDocToken::STRING) {
            let value = self.stream.get_string();
            let mode = match &value {
                s if s == "WHITESPACE" => PerFileClosureUnawareMode::WHITESPACE,
                s if s == "SIMPLE" => PerFileClosureUnawareMode::SIMPLE,
                _ => {
                    self.add_parser_warning_arg(
                        ast,
                        Msg::JSDOC_CLOSURE_UNAWARE_CONFIG_INVALID_VALUE,
                        &value,
                    );
                    PerFileClosureUnawareMode::SIMPLE
                }
            };
            if !self
                .jsdoc_builder
                .record_closure_unaware_code_with_mode(mode)
            {
                self.add_parser_warning(ast, Msg::JSDOC_CLOSURE_UNAWARE_CODE_EXTRA);
            }
        }
        self.eat_until_eol_if_not_annotation_from(token)
    }
}
