/*
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
//   src/com/google/javascript/jscomp/parsing/ParserRunner.java.

//! Port of `com.google.javascript.jscomp.parsing.ParserRunner`.

use std::sync::OnceLock;

use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;

use crate::config::{Config, JsDocParsing, LanguageMode, RunMode, StrictMode};
use crate::parser_configuration::ParserConfiguration;

/// The static caches `annotationNames`, `suppressionNames`, `reservedVars` and
/// `closurePrimitiveNames`, filled once by `initResourceConfig`.
struct ResourceConfig {
    annotation_names: IndexSet<JsString>,
    suppression_names: IndexSet<JsString>,
    reserved_vars: IndexSet<JsString>,
    closure_primitive_names: IndexSet<JsString>,
}

static RESOURCE_CONFIG: OnceLock<ResourceConfig> = OnceLock::new();

/// parser runner
pub struct ParserRunner;

impl ParserRunner {
    // port: ParserRunner#createConfig(LanguageMode, Set, StrictMode)
    pub fn create_config(
        language_mode: LanguageMode,
        extra_annotation_names: Option<&IndexSet<JsString>>,
        strict_mode: StrictMode,
    ) -> Config {
        Self::create_config_full(
            language_mode,
            JsDocParsing::TYPES_ONLY,
            RunMode::STOP_AFTER_ERROR,
            extra_annotation_names,
            true,
            strict_mode,
        )
    }

    // port: ParserRunner#createConfig(LanguageMode, JsDocParsing, RunMode, Set, boolean, StrictMode)
    pub fn create_config_full(
        language_mode: LanguageMode,
        jsdoc_parsing_mode: JsDocParsing,
        run_mode: RunMode,
        extra_annotation_names: Option<&IndexSet<JsString>>,
        parse_inline_source_maps: bool,
        strict_mode: StrictMode,
    ) -> Config {
        let resource_config = Self::init_resource_config();
        let effective_annotation_names: IndexSet<JsString> = match extra_annotation_names {
            None => resource_config.annotation_names.clone(),
            Some(extra_annotation_names) => {
                let mut effective_annotation_names = resource_config.annotation_names.clone();
                effective_annotation_names.extend(extra_annotation_names.iter().cloned());
                effective_annotation_names
            }
        };
        Config::builder()
            .set_extra_annotation_names(effective_annotation_names)
            .set_js_doc_parsing_mode(jsdoc_parsing_mode)
            .set_run_mode(run_mode)
            .set_suppression_names(resource_config.suppression_names.iter().cloned())
            .set_closure_primitive_names(resource_config.closure_primitive_names.iter().cloned())
            .set_language_mode(language_mode)
            .set_parse_inline_source_maps(parse_inline_source_maps)
            .set_strict_mode(strict_mode)
            .build()
    }

    // port: ParserRunner#parse
    pub fn parse(
        ast: &mut closure_rhino::node::Ast,
        source_file: std::sync::Arc<dyn closure_rhino::static_source_file::StaticSourceFile>,
        source_string: JsString,
        config: &Config,
        error_reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
    ) -> ParseResult {
        let source_name = source_file.get_name().to_owned();
        Self::try_parse(ast, source_file, source_string, config, error_reporter)
            .unwrap_or_else(|t| panic!("Exception parsing \"{source_name}\": {}", t.class))
    }

    // port: ParserRunner#parse (Throwable is represented as Result for callers handling failures)
    #[allow(clippy::collapsible_if)] // Java combines tree != null with errorSeen; Rc requires a separate borrow.
    pub fn try_parse(
        ast: &mut closure_rhino::node::Ast,
        source_file: std::sync::Arc<dyn closure_rhino::static_source_file::StaticSourceFile>,
        source_string: JsString,
        config: &Config,
        error_reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
    ) -> Result<ParseResult, crate::parser::parser::ParseError> {
        use crate::{
            ir_factory::IRFactory,
            parser::{parser::Parser, source_file::SourceFile, util::Reporter},
        };
        use closure_rhino::node::{NodeId, ObjectProp};
        use std::{cell::RefCell, rc::Rc, sync::Arc};
        let source_name = source_file.get_name().to_owned();
        let file = Arc::new(SourceFile::new(source_name, source_string));
        let keep_going = config.run_mode() == RunMode::KEEP_GOING;
        let es6_error_reporter = Rc::new(RefCell::new(Es6ErrorReporter::new(keep_going)));
        let reporter = Reporter::new(Es6ErrorReporterSink(es6_error_reporter.clone()));
        let mut p = Parser::new(Self::new_parser_config(config), reporter, file.clone());
        let parsed = p.parse_program();
        // Parser's shared Reporter requires a 'static sink. Replay its calls before IRFactory,
        // preserving the Java parser -> transformer diagnostic order without storing a reference.
        for report in &es6_error_reporter.borrow().reports {
            let location = &report.location;
            let source_name = location.source.map(|s| s.name());
            let source_name = source_name.as_deref().unwrap_or("");
            if report.is_error {
                error_reporter.error_js_string(
                    &report.message,
                    source_name,
                    location.line + 1,
                    location.column,
                );
            } else {
                error_reporter.warning_js_string(
                    &report.message,
                    source_name,
                    location.line + 1,
                    location.column,
                );
            }
        }
        let tree = parsed?;
        let mut root = None;
        let mut comments = Vec::new();
        let mut top_level_statement_ranges = Vec::new();
        let mut features = p.get_features();
        if let Some(tree) = tree {
            if !es6_error_reporter.borrow().error_seen || keep_going {
                let factory = IRFactory::transform_tree(
                    ast,
                    &tree,
                    source_file,
                    config.clone(),
                    error_reporter,
                    file,
                )?;
                let node = factory.get_result_node();
                features = features.union(factory.get_features());
                node.put_prop(
                    ast,
                    NodeId::FEATURE_SET,
                    Some(ObjectProp::Opaque(Arc::new(features))),
                );
                root = Some(node);
                if config.js_doc_parsing_mode().should_parse_descriptions() {
                    comments = p.get_comments();
                    for element in &tree.as_program().source_elements {
                        top_level_statement_ranges.push(element.location.clone());
                    }
                }
            }
        }
        Ok(ParseResult::new_with_statement_ranges(
            root,
            comments,
            top_level_statement_ranges,
            features,
            p.get_source_map_url(),
        ))
    }

    // port: ParserRunner#newParserConfig
    fn new_parser_config(config: &Config) -> crate::parser::parser::Config {
        use crate::parser::parser::{Config as ParserConfig, Mode};
        let parser_config_language_mode = match config.language_mode() {
            LanguageMode::ECMASCRIPT3 => Mode::ES3,
            LanguageMode::ECMASCRIPT5 => Mode::ES5,
            LanguageMode::ECMASCRIPT_2015 | LanguageMode::ECMASCRIPT_2016 => Mode::ES6_OR_ES7,
            _ => Mode::ES8_OR_GREATER,
        };
        ParserConfig::new(
            parser_config_language_mode,
            config.strict_mode().is_strict(),
        )
    }

    // port: ParserRunner#getReservedVars
    pub fn get_reserved_vars() -> &'static IndexSet<JsString> {
        &Self::init_resource_config().reserved_vars
    }

    // port: ParserRunner#getSuppressionNames
    pub fn get_suppression_names() -> &'static IndexSet<JsString> {
        &Self::init_resource_config().suppression_names
    }

    // port: ParserRunner#initResourceConfig
    fn init_resource_config() -> &'static ResourceConfig {
        RESOURCE_CONFIG.get_or_init(|| ResourceConfig {
            annotation_names: Self::extract_list(&ParserConfiguration::get_string(
                "jsdoc.annotations",
            )),
            suppression_names: Self::extract_list(&ParserConfiguration::get_string(
                "jsdoc.suppressions",
            )),
            closure_primitive_names: Self::extract_list(&ParserConfiguration::get_string(
                "jsdoc.primitives",
            )),
            reserved_vars: Self::extract_list(&ParserConfiguration::get_string(
                "compiler.reserved.vars",
            )),
        })
    }

    // port: ParserRunner#extractList
    fn extract_list(config_prop: &str) -> IndexSet<JsString> {
        // Splitter.on(',').trimResults() trims CharMatcher.whitespace(), then ImmutableSet.copyOf.
        config_prop
            .split(',')
            .map(|s| JsString::from(trim_guava_whitespace(s)))
            .collect()
    }
}

/// `CharMatcher.whitespace().trimFrom(s)`: Guava's whitespace table (Unicode White_Space).
fn trim_guava_whitespace(s: &str) -> &str {
    s.trim_matches(is_guava_whitespace)
}

/// Guava `CharMatcher.whitespace()`: `\t \n \u000B \f \r`, space, `\u0085`, ` `, ` `,
/// ` `-` `, ` `, ` `, ` `, ` `, `　`.
fn is_guava_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{000B}'
            | '\u{000C}'
            | '\r'
            | ' '
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// Holds results of parsing.
pub struct ParseResult {
    pub ast: Option<closure_rhino::node::NodeId>,
    pub comments: Vec<crate::parser::trees::comment::Comment>,
    pub top_level_statement_ranges: Vec<crate::parser::util::source_range::SourceRange>,
    pub features: crate::parser::feature_set::FeatureSet,
    pub source_map_url: Option<JsString>,
}
impl ParseResult {
    // port: ParserRunner.ParseResult#ParseResult(Node, List, FeatureSet, String)
    pub fn new(
        ast: Option<closure_rhino::node::NodeId>,
        comments: Vec<crate::parser::trees::comment::Comment>,
        features: crate::parser::feature_set::FeatureSet,
        source_map_url: Option<JsString>,
    ) -> Self {
        Self::new_with_statement_ranges(ast, comments, Vec::new(), features, source_map_url)
    }
    // port: ParserRunner.ParseResult#ParseResult(Node, List, ImmutableList, FeatureSet, String)
    pub fn new_with_statement_ranges(
        ast: Option<closure_rhino::node::NodeId>,
        comments: Vec<crate::parser::trees::comment::Comment>,
        top_level_statement_ranges: Vec<crate::parser::util::source_range::SourceRange>,
        features: crate::parser::feature_set::FeatureSet,
        source_map_url: Option<JsString>,
    ) -> Self {
        Self {
            ast,
            comments,
            top_level_statement_ranges,
            features,
            source_map_url,
        }
    }
}
struct Es6Report {
    is_error: bool,
    location: crate::parser::util::SourcePosition,
    message: JsString,
}
struct Es6ErrorReporter {
    error_seen: bool,
    report_all_errors: bool,
    reports: Vec<Es6Report>,
}
impl Es6ErrorReporter {
    // port: ParserRunner.Es6ErrorReporter#Es6ErrorReporter
    fn new(report_all_errors: bool) -> Self {
        Self {
            error_seen: false,
            report_all_errors,
            reports: Vec::new(),
        }
    }
    // port: ParserRunner.Es6ErrorReporter#reportError
    fn report_error(&mut self, location: crate::parser::util::SourcePosition, message: JsString) {
        if self.report_all_errors || !self.error_seen {
            self.error_seen = true;
            self.reports.push(Es6Report {
                is_error: true,
                location,
                message,
            });
        }
    }
    // port: ParserRunner.Es6ErrorReporter#reportWarning
    fn report_warning(&mut self, location: crate::parser::util::SourcePosition, message: JsString) {
        self.reports.push(Es6Report {
            is_error: false,
            location,
            message,
        });
    }
}
struct Es6ErrorReporterSink(std::rc::Rc<std::cell::RefCell<Es6ErrorReporter>>);
impl crate::parser::util::error_reporter::ErrorReporter for Es6ErrorReporterSink {
    // port: ParserRunner.Es6ErrorReporter#reportError
    fn report_error(&mut self, location: crate::parser::util::SourcePosition, message: JsString) {
        self.0.borrow_mut().report_error(location, message);
    }
    // port: ParserRunner.Es6ErrorReporter#reportWarning
    fn report_warning(&mut self, location: crate::parser::util::SourcePosition, message: JsString) {
        self.0.borrow_mut().report_warning(location, message);
    }
}
