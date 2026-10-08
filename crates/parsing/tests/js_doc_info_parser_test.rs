/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeUtil.java,
//   test/com/google/javascript/jscomp/parsing/JsDocInfoParserTest.java.

//! Faithful ports of JsDocInfoParserTest.
#![allow(clippy::too_many_arguments, clippy::collapsible_if, unused_mut)]
use closure_jscomp::source_file::SourceFile;
use closure_jstype::{
    prelude::*,
    testing::{
        base_js_type_test_case::BaseJSTypeTestCase, map_based_scope::MapBasedScope,
        type_subject::TypeSubject,
    },
};
use closure_parsing::{
    config::{Config, JsDocParsing, LanguageMode, RunMode, StrictMode},
    js_doc_info_parser::{BAD_TYPE_WIKI_LINK, JsDocInfoParser, JsDocSourceKind},
    js_doc_token_stream::JsDocTokenStream,
    parser_runner::ParserRunner,
};
use closure_rhino::{
    ir::IR,
    js_string::JsString,
    jsdoc_info::{JSDocInfo, Marker, PerFileClosureUnawareMode, Visibility},
    node::{Ast, NodeId},
    simple_source_file::SimpleSourceFile,
    static_source_file::{SourceKind, StaticSourceFile},
    testing::{node_subject::assert_node, test_error_reporter::TestErrorReporter},
    token::Token,
};
use indexmap::IndexSet;
use std::sync::Arc;
const MISSING_TYPE_DECL_WARNING_TEXT: &str = "Missing type declaration.";
struct Fixture {
    types: BaseJSTypeTestCase,
    extra_annotations: IndexSet<JsString>,
    extra_suppressions: IndexSet<JsString>,
    extra_primitives: IndexSet<JsString>,
    js_doc_source_kind: JsDocSourceKind,
    allow_closure_unaware_code: bool,
    prev_license: Option<JsString>,
}
// port: JsDocInfoParserTest#setUp
fn set_up() -> Fixture {
    let config = ParserRunner::create_config(LanguageMode::ECMASCRIPT3, None, StrictMode::SLOPPY);
    let mut suppressions = config.suppression_names().clone();
    suppressions.extend(["x".into(), "y".into(), "z".into()]);
    Fixture {
        types: BaseJSTypeTestCase::new(),
        extra_annotations: config.annotation_names(),
        extra_suppressions: suppressions,
        extra_primitives: ["id".into(), "idA".into(), "idB".into()]
            .into_iter()
            .collect(),
        js_doc_source_kind: JsDocSourceKind::NORMAL,
        allow_closure_unaware_code: false,
        prev_license: None,
    }
}
impl std::ops::Deref for Fixture {
    type Target = BaseJSTypeTestCase;
    fn deref(&self) -> &Self::Target {
        &self.types
    }
}
impl std::ops::DerefMut for Fixture {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.types
    }
}
impl Fixture {
    // port: JsDocInfoParserTest#testParseType(String, String)
    fn test_parse_type(&mut self, type_: JsString, type_expected: JsString) -> TypeId {
        let info = self.parse(
            JsString::from("@type {")
                .concat(&type_)
                .concat(&"}*/".into()),
            &[],
        );
        assert!(info.is_some());
        let info = info.unwrap();
        assert!(info.has_type());
        let actual = self.types.resolve(&info.get_type().unwrap(), &[]);
        assert_eq!(
            actual.to_string(&mut self.types.reg, &self.types.ast),
            type_expected.to_string_lossy()
        );
        actual
    }
    // port: JsDocInfoParserTest#assertTemplatizedTypeEquals
    fn assert_templatized_type_equals(
        &mut self,
        key: TypeId,
        expected: TypeId,
        te: &closure_rhino::js_type_expression::JSTypeExpression,
    ) {
        let resolved = self.types.resolve(te, &[]);
        let actual = resolved
            .get_template_type_map(&self.types.reg)
            .get_resolved_template_type(&mut self.types.reg, &self.types.ast, key);
        TypeSubject::assert_type(actual).is_equal_to(
            &mut self.types.reg,
            &self.types.ast,
            expected,
        );
    }
    // port: JsDocInfoParserTest#createNamedType
    fn create_named_type(&mut self, name: JsString) -> TypeId {
        self.types
            .error_reporter
            .lock()
            .unwrap()
            .expect_all_warnings(&[&format!("Bad type annotation. Unknown type {name}")]);
        self.types.reg.create_named_type(
            &self.types.ast,
            Some(Arc::new(MapBasedScope::empty_scope())),
            name,
            "",
            -1,
            -1,
        )
    }
    // port: JsDocInfoParserTest#parseFull
    fn parse_full(&mut self, code: JsString, warnings: &[JsString]) -> NodeId {
        let mut reporter = TestErrorReporter::new();
        reporter.expect_all_warnings_js_strings(warnings);
        let config = Config::builder()
            .set_extra_annotation_names(self.extra_annotations.clone())
            .set_js_doc_parsing_mode(JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE)
            .set_run_mode(RunMode::KEEP_GOING)
            .set_suppression_names(self.extra_suppressions.clone())
            .set_language_mode(LanguageMode::ECMASCRIPT3)
            .set_parse_inline_source_maps(true)
            .set_strict_mode(StrictMode::SLOPPY)
            .build();
        let result = ParserRunner::parse(
            &mut self.types.ast,
            Arc::new(SimpleSourceFile::new("source", SourceKind::STRONG)),
            code,
            &config,
            &mut reporter,
        );
        reporter.verify_has_encountered_all_warnings_and_errors();
        result.ast.unwrap()
    }
    // port: JsDocInfoParserTest#parse(String, String...)
    fn parse(&mut self, comment: JsString, warnings: &[JsString]) -> Option<Arc<JSDocInfo>> {
        self.parse_with_mode(comment, JsDocParsing::TYPES_ONLY, warnings)
    }
    // port: JsDocInfoParserTest#parse(String, boolean, String...)
    fn parse_with_documentation(
        &mut self,
        comment: JsString,
        documentation: bool,
        warnings: &[JsString],
    ) -> Option<Arc<JSDocInfo>> {
        self.parse_with_mode(
            comment,
            if documentation {
                JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE
            } else {
                JsDocParsing::TYPES_ONLY
            },
            warnings,
        )
    }
    // port: JsDocInfoParserTest#parse(String, JsDocParsing, String...)
    fn parse_with_mode(
        &mut self,
        comment: JsString,
        mode: JsDocParsing,
        warnings: &[JsString],
    ) -> Option<Arc<JSDocInfo>> {
        self.parse_full_options(comment, mode, false, warnings)
    }
    // port: JsDocInfoParserTest#parseFileOverview
    fn parse_file_overview(
        &mut self,
        comment: JsString,
        warnings: &[JsString],
    ) -> Option<Arc<JSDocInfo>> {
        self.parse_full_options(
            comment,
            JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
            true,
            warnings,
        )
    }
    // port: JsDocInfoParserTest#preserveWhitespaceParse
    fn preserve_whitespace_parse(
        &mut self,
        comment: JsString,
        warnings: &[JsString],
    ) -> Option<Arc<JSDocInfo>> {
        self.parse_with_mode(
            comment,
            JsDocParsing::INCLUDE_DESCRIPTIONS_WITH_WHITESPACE,
            warnings,
        )
    }
    // port: JsDocInfoParserTest#parse(String, JsDocParsing, boolean, String...)
    fn parse_full_options(
        &mut self,
        comment: JsString,
        mode: JsDocParsing,
        overview: bool,
        warnings: &[JsString],
    ) -> Option<Arc<JSDocInfo>> {
        let mut reporter = TestErrorReporter::new();
        reporter.expect_all_warnings_js_strings(warnings);
        if self.allow_closure_unaware_code {
            reporter.expect_all_warnings(&[
                "@closureUnaware annotation is not allowed in this compilation",
            ]);
        }
        let config = Config::builder()
            .set_extra_annotation_names(self.extra_annotations.clone())
            .set_js_doc_parsing_mode(mode)
            .set_suppression_names(self.extra_suppressions.clone())
            .set_closure_primitive_names(self.extra_primitives.clone())
            .set_language_mode(LanguageMode::ECMASCRIPT3)
            .set_parse_inline_source_maps(true)
            .set_strict_mode(StrictMode::SLOPPY)
            .build();
        let template = IR::script(&mut self.types.ast);
        template.set_static_source_file(
            &mut self.types.ast,
            Some(Arc::new(SimpleSourceFile::new(
                "testcode",
                SourceKind::STRONG,
            ))),
        );
        let mut parser = JsDocInfoParser::new(
            &mut self.types.ast,
            stream(comment.clone()),
            Some(comment),
            0,
            Some(template),
            &config,
            self.js_doc_source_kind,
            &mut reporter,
        );
        parser.parse(&mut self.types.ast);
        self.prev_license = parser.get_license_text();
        let result = if overview {
            parser.get_file_overview_jsdoc_info()
        } else {
            parser.retrieve_and_reset_parsed_jsdoc_info()
        };
        drop(parser);
        reporter.verify_has_encountered_all_warnings_and_errors();
        if let Some(info) = &result {
            for n in info.get_type_nodes() {
                assert!(get_source_name(&self.types.ast, n).is_some());
            }
        }
        result
    }
    // port: JsDocInfoParserTest#parseType
    fn parse_type(&mut self, comment: JsString) -> Option<NodeId> {
        JsDocInfoParser::parse_type_string(&mut self.types.ast, comment)
    }
}
// port: JsDocInfoParserTest#stream
fn stream(source: JsString) -> JsDocTokenStream {
    JsDocTokenStream::new_with_lineno(source, 0)
}
// port: NodeUtil#getSourceName
fn get_source_name(ast: &Ast, n: NodeId) -> Option<String> {
    let mut current = Some(n);
    while let Some(node) = current {
        let name = node.get_source_file_name(ast);
        if name.is_some() {
            return name;
        }
        current = node.get_parent(ast);
    }
    None
}
// port: JsDocInfoParserTest#checkTokenPosition
fn check_token_position(ast: &Ast, n: NodeId, token: Token, lineno: i32, charno: i32) {
    assert_eq!(n.get_token(ast), token);
    assert_node(n).has_lineno(ast, lineno);
    assert_eq!(n.get_charno(ast), charno);
}
// port: JsDocInfoParserTest#assertDocumentationInMarker
fn assert_documentation_in_marker(
    marker: Marker,
    description: JsString,
    start_charno: i32,
    end_lineno: i32,
    end_charno: i32,
) -> Marker {
    let field = marker.get_description().unwrap();
    assert_eq!(field.get_item(), Some(&description));
    assert_eq!(
        field.get_start_line(),
        marker.get_annotation().unwrap().get_start_line()
    );
    assert_eq!(field.get_position_on_start_line(), start_charno);
    assert_eq!(field.get_end_line(), end_lineno);
    assert_eq!(field.get_position_on_end_line(), end_charno);
    marker
}
// port: JsDocInfoParserTest#assertTypeInMarker
fn assert_type_in_marker(
    ast: &Ast,
    marker: Marker,
    name: JsString,
    start_lineno: i32,
    start_charno: i32,
    end_lineno: i32,
    end_charno: i32,
    brackets: bool,
) -> Marker {
    let field = marker.get_type().unwrap();
    let n = *field.get_item().unwrap();
    assert!(n.is_string_lit(ast));
    assert_eq!(n.get_string(ast), name);
    assert_eq!(field.has_brackets(), brackets);
    assert_eq!(field.get_position_on_start_line(), start_charno);
    assert_eq!(field.get_position_on_end_line(), end_charno);
    assert_eq!(field.get_start_line(), start_lineno);
    assert_eq!(field.get_end_line(), end_lineno);
    marker
}
// port: JsDocInfoParserTest#assertNameInMarker
fn assert_name_in_marker(
    ast: &Ast,
    marker: Marker,
    name: JsString,
    start_line: i32,
    start_charno: i32,
) -> Marker {
    let field = marker.get_name_node().unwrap();
    assert_eq!(field.get_item().unwrap().get_string(ast), name);
    assert_eq!(field.get_position_on_start_line(), start_charno);
    assert_eq!(
        field.get_position_on_end_line(),
        start_charno + name.length() as i32
    );
    assert_eq!(field.get_start_line(), start_line);
    assert_eq!(field.get_end_line(), start_line);
    marker
}
// port: JsDocInfoParserTest#assertAnnotationMarker(JSDocInfo, String, int, int)
fn assert_annotation_marker(
    info: Option<Arc<JSDocInfo>>,
    annotation: JsString,
    lineno: i32,
    charno: i32,
) -> Marker {
    assert_annotation_marker_at(info, annotation, lineno, charno, 0)
}
// port: JsDocInfoParserTest#assertAnnotationMarker(JSDocInfo, String, int, int, int)
fn assert_annotation_marker_at(
    info: Option<Arc<JSDocInfo>>,
    annotation: JsString,
    lineno: i32,
    charno: i32,
    index: usize,
) -> Marker {
    let markers = info.unwrap().get_markers();
    assert!(!markers.is_empty());
    let mut counter = 0;
    for marker in markers {
        if let Some(field) = marker.get_annotation() {
            if field.get_item() == Some(&annotation) {
                if counter == index {
                    assert_eq!(field.get_start_line(), lineno);
                    assert_eq!(field.get_position_on_start_line(), charno);
                    assert_eq!(field.get_end_line(), lineno);
                    assert_eq!(
                        field.get_position_on_end_line(),
                        charno + annotation.length() as i32
                    );
                    return marker;
                }
                counter += 1;
            }
        }
    }
    panic!("No marker found");
}
// Truth containsExactly without inOrder compares multisets.
fn assert_contents<T: Eq + std::fmt::Debug>(
    actual: impl IntoIterator<Item = T>,
    expected: Vec<T>,
    ordered: bool,
) {
    let actual: Vec<_> = actual.into_iter().collect();
    if ordered {
        assert_eq!(actual, expected);
    } else {
        let mut remaining = expected;
        for item in actual {
            let index = remaining
                .iter()
                .position(|e| e == &item)
                .unwrap_or_else(|| panic!("Unexpected item {item:?} in {remaining:?}"));
            remaining.remove(index);
        }
        assert!(remaining.is_empty(), "Missing items: {remaining:?}");
    }
}
// port: JsDocInfoParserTest#testParseInvalidTypeViaStatic
#[test]
fn test_parse_invalid_type_via_static() {
    let mut f = set_up();
    let mut type_node = f.parse_type(JsString::from("sometype<anothertype"));
    assert!(type_node.is_none());
}

// port: JsDocInfoParserTest#testParseInvalidTypeViaStatic2
#[test]
fn test_parse_invalid_type_via_static2() {
    let mut f = set_up();
    let mut type_node = f.parse_type(JsString::from(""));
    assert!(type_node.is_none());
}

// port: JsDocInfoParserTest#testParseNamedType1
#[test]
fn test_parse_named_type1() {
    let mut f = set_up();
    assert!(
        f.parse(
            JsString::from("@type {null}"),
            &[JsString::from("Unexpected end of file")]
        )
        .is_none()
    );
}

// port: JsDocInfoParserTest#testParseNamedTypeError1
#[test]
fn test_parse_named_type_error1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {!goog\n * .Bar} */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseNamedTypeError2
#[test]
fn test_parse_named_type_error2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {!goog.\n * Bar\n * .Baz} */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseUnionType10
#[test]
fn test_parse_union_type10() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {string|}*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseUnionType12
#[test]
fn test_parse_union_type12() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {()}*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseUnionTypeError2
#[test]
fn test_parse_union_type_error2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {string||number} */"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseTemplatizedUnknown1
#[test]
fn test_parse_templatized_unknown1() {
    let mut f = set_up();
    f.parse(JsString::from("@ type {?<?>} */"), &[(JsString::from("illegal use of unknown JSDoc tag \"\"; ignoring it. Place another character before the @")).concat(&(JsString::from(" to stop JSCompiler from parsing it as an annotation.")))]);
}

// port: JsDocInfoParserTest#testParseTemplatizedUnknown2
#[test]
fn test_parse_templatized_unknown2() {
    let mut f = set_up();
    f.parse(JsString::from("@ type {{b: ?<?>}} */"), &[(JsString::from("illegal use of unknown JSDoc tag \"\"; ignoring it. Place another character before the @")).concat(&(JsString::from(" to stop JSCompiler from parsing it as an annotation.")))]);
}

// port: JsDocInfoParserTest#testBug1419535
#[test]
fn test_bug1419535() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function(Object, string, *)?} */"),
        &[],
    );
    f.parse(
        JsString::from("@type {function(Object, string, *)|null} */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testIssue477
#[test]
fn test_issue477() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function} */"),
        &[(JsString::from("Bad type annotation. missing opening ("))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testMalformedThisAnnotation
#[test]
fn test_malformed_this_annotation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@this */"),
        &[
            (JsString::from("Bad type annotation. Type annotations should have curly braces."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseImportTypeError
#[test]
fn test_parse_import_type_error() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {import('http').Stream} */"),
        &[
            (JsString::from("Bad type annotation. Import in typedef is not supported."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError1
#[test]
fn test_parse_functional_type_error1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function number):string}*/"),
        &[(JsString::from("Bad type annotation. missing opening ("))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError2
#[test]
fn test_parse_functional_type_error2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function( number}*/"),
        &[(JsString::from("Bad type annotation. missing closing )"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError3
#[test]
fn test_parse_functional_type_error3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function(...[number], string)}*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError4
#[test]
fn test_parse_functional_type_error4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function(string, ...number, boolean):string}*/"),
        &[
            (JsString::from("Bad type annotation. variable length argument must be last."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError5
#[test]
fn test_parse_functional_type_error5() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function (thi:Array)}*/"),
        &[(JsString::from("Bad type annotation. missing closing )"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError7
#[test]
fn test_parse_functional_type_error7() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function(...[number)}*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError8
#[test]
fn test_parse_functional_type_error8() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function(...number])}*/"),
        &[(JsString::from("Bad type annotation. missing closing )"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError9
#[test]
fn test_parse_functional_type_error9() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function (new:Array, this:Object)} */"),
        &[(JsString::from("Bad type annotation. missing closing )"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError10
#[test]
fn test_parse_functional_type_error10() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function (this:Array, new:Object)} */"),
        &[(JsString::from("Bad type annotation. missing closing )"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError11
#[test]
fn test_parse_functional_type_error11() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function (Array, new:Object)} */"),
        &[(JsString::from("Bad type annotation. missing closing )"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError13
#[test]
fn test_parse_functional_type_error13() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function (...[number]): boolean} */"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError14
#[test]
fn test_parse_functional_type_error14() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function (number, ...[string]): boolean} */"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseFunctionalType8
#[test]
fn test_parse_functional_type8() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {function(this:Array,...[boolean])} */"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseArrayTypeError1
#[test]
fn test_parse_array_type_error1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {[number}*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseArrayTypeError2
#[test]
fn test_parse_array_type_error2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {number]}*/"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseArrayTypeError3
#[test]
fn test_parse_array_type_error3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {[(number,boolean,Object?])]}*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseArrayTypeError5
#[test]
fn test_parse_array_type_error5() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {[Object]}*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseTypeofType1
#[test]
fn test_parse_typeof_type1() {
    let mut f = set_up();
    let mut node = ((((f.parse(JsString::from("@type {typeof Foo}*/"), &[])).unwrap()).get_type())
        .unwrap())
    .get_root();
    assert_eq!((node).get_token(&f.ast), Token::TYPEOF);
    assert_eq!((node).get_child_count(&f.ast), 1);
    let mut foo_node = (node).get_first_child(&f.ast);
    assert_eq!(((foo_node).unwrap()).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!(
        ((foo_node).unwrap()).get_string(&f.ast),
        JsString::from("Foo")
    );
    assert_eq!(((foo_node).unwrap()).get_charno(&f.ast), 14);
}

// port: JsDocInfoParserTest#testParseTypeofType2
#[test]
fn test_parse_typeof_type2() {
    let mut f = set_up();
    let mut node =
        ((((f.parse(JsString::from("@type {(typeof Foo)}*/"), &[])).unwrap()).get_type()).unwrap())
            .get_root();
    assert_eq!((node).get_token(&f.ast), Token::TYPEOF);
    assert_eq!((node).get_child_count(&f.ast), 1);
    let mut foo_node = (node).get_first_child(&f.ast);
    assert_eq!(((foo_node).unwrap()).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!(
        ((foo_node).unwrap()).get_string(&f.ast),
        JsString::from("Foo")
    );
    assert_eq!(((foo_node).unwrap()).get_charno(&f.ast), 15);
}

// port: JsDocInfoParserTest#testParseTypeofType3
#[test]
fn test_parse_typeof_type3() {
    let mut f = set_up();
    let mut node = ((((f.parse(JsString::from("@type {typeof Foo|Bar<typeof Baz>}*/"), &[]))
        .unwrap())
    .get_type())
    .unwrap())
    .get_root();
    assert_eq!((node).get_token(&f.ast), Token::PIPE);
    assert_eq!(
        (((node).get_first_child(&f.ast)).unwrap()).get_token(&f.ast),
        Token::TYPEOF
    );
    let mut foo_node = (node).get_first_first_child(&f.ast);
    assert_eq!(((foo_node).unwrap()).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!(
        ((foo_node).unwrap()).get_string(&f.ast),
        JsString::from("Foo")
    );
    assert_eq!(((foo_node).unwrap()).get_charno(&f.ast), 14);
    let mut bar_node = (node).get_last_child(&f.ast);
    assert_eq!(((bar_node).unwrap()).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!(
        ((bar_node).unwrap()).get_string(&f.ast),
        JsString::from("Bar")
    );
    assert_eq!(((bar_node).unwrap()).get_charno(&f.ast), 18);
    assert_eq!(
        ((((bar_node).unwrap()).get_first_child(&f.ast)).unwrap()).get_token(&f.ast),
        Token::BLOCK
    );
    assert_eq!(
        ((((bar_node).unwrap()).get_first_first_child(&f.ast)).unwrap()).get_token(&f.ast),
        Token::TYPEOF
    );
    let mut baz_node =
        ((((bar_node).unwrap()).get_first_first_child(&f.ast)).unwrap()).get_first_child(&f.ast);
    assert_eq!(((baz_node).unwrap()).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!(
        ((baz_node).unwrap()).get_string(&f.ast),
        JsString::from("Baz")
    );
    assert_eq!(((baz_node).unwrap()).get_charno(&f.ast), 29);
}

// port: JsDocInfoParserTest#testParseTypeofTypeNewLine
#[test]
fn test_parse_typeof_type_new_line() {
    let mut f = set_up();
    let mut node = ((((f.parse(JsString::from("@type {typeof \n  Foo}*/"), &[])).unwrap())
        .get_type())
    .unwrap())
    .get_root();
    assert_eq!((node).get_token(&f.ast), Token::TYPEOF);
    assert_eq!((node).get_child_count(&f.ast), 1);
    let mut foo_node = (node).get_first_child(&f.ast);
    assert_eq!(((foo_node).unwrap()).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!(
        ((foo_node).unwrap()).get_string(&f.ast),
        JsString::from("Foo")
    );
    assert_eq!(((foo_node).unwrap()).get_charno(&f.ast), 2);
    assert_node((foo_node).unwrap()).has_lineno(&f.ast, 1);
}

// port: JsDocInfoParserTest#testTypenameSourceInfo_simpleName
#[test]
fn test_typename_source_info_simple_name() {
    let mut f = set_up();
    let mut node = ((((f.parse(JsString::from("@type {Foo} */"), &[])).unwrap()).get_type())
        .unwrap())
    .get_root();
    assert_eq!((node).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!((node).get_lineno(&f.ast), 0);
    assert_eq!((node).get_charno(&f.ast), 7);
    assert_eq!((node).get_length(&f.ast), 3);
}

// port: JsDocInfoParserTest#testTypenameSourceInfo_qualifiedName
#[test]
fn test_typename_source_info_qualified_name() {
    let mut f = set_up();
    let mut node =
        ((((f.parse(JsString::from("@type {bar.baz.Foo} */"), &[])).unwrap()).get_type()).unwrap())
            .get_root();
    assert_eq!((node).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!((node).get_lineno(&f.ast), 0);
    assert_eq!((node).get_charno(&f.ast), 7);
    assert_eq!((node).get_length(&f.ast), 11);
}

// port: JsDocInfoParserTest#testTypenameSourceInfo_qualifiedName_multiline
#[test]
fn test_typename_source_info_qualified_name_multiline() {
    let mut f = set_up();
    let mut node = ((((f.parse(JsString::from("@type {bar.\n       baz.Foo} */\n"), &[]))
        .unwrap())
    .get_type())
    .unwrap())
    .get_root();
    assert_eq!((node).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!((node).get_lineno(&f.ast), 0);
    assert_eq!((node).get_charno(&f.ast), 7);
    assert_eq!((node).get_length(&f.ast), 19);
}

// port: JsDocInfoParserTest#testTypenameSourceInfo_qualifiedName_leadingWhitespace
#[test]
fn test_typename_source_info_qualified_name_leading_whitespace() {
    let mut f = set_up();
    let mut node =
        ((((f.parse(JsString::from("@type {\n    Foo} */\n"), &[])).unwrap()).get_type()).unwrap())
            .get_root();
    assert_eq!((node).get_token(&f.ast), Token::STRINGLIT);
    assert_eq!((node).get_lineno(&f.ast), 1);
    assert_eq!((node).get_charno(&f.ast), 4);
    assert_eq!((node).get_length(&f.ast), 3);
}

// port: JsDocInfoParserTest#testParseThisType3
#[test]
fn test_parse_this_type3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {number}\n@this {goog.foo.Bar}*/"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseThisType5
#[test]
fn test_parse_this_type5() {
    let mut f = set_up();
    f.parse(JsString::from("@this {Date|Error}*/"), &[]);
}

// port: JsDocInfoParserTest#testParseParam2
#[test]
fn test_parse_param2() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param index*/"),
        &[JsString::from(MISSING_TYPE_DECL_WARNING_TEXT)],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    assert!(
        ((info.clone()).unwrap())
            .get_parameter_type(JsString::from("index"))
            .is_none()
    );
}

// port: JsDocInfoParserTest#testParseParam4
#[test]
fn test_parse_param4() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param index useful comments*/"),
        &[JsString::from(MISSING_TYPE_DECL_WARNING_TEXT)],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    assert!(
        ((info.clone()).unwrap())
            .get_parameter_type(JsString::from("index"))
            .is_none()
    );
}

// port: JsDocInfoParserTest#testParseParam9
#[test]
fn test_parse_param9() {
    let mut f = set_up();
    f.parse(
        JsString::from("@param {...number=} index */"),
        &[
            (JsString::from("Bad type annotation. expected closing }"))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseParam10
#[test]
fn test_parse_param10() {
    let mut f = set_up();
    f.parse(
        JsString::from("@param {...number index */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseParam11
#[test]
fn test_parse_param11() {
    let mut f = set_up();
    f.parse(
        JsString::from("@param {number= index */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseParam22
#[test]
fn test_parse_param22() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {string} .index */"),
        &[JsString::from("invalid param name \".index\"")],
    );
    assert!(info.clone().is_none());
}

// port: JsDocInfoParserTest#testParseParam23
#[test]
fn test_parse_param23() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {string} index. */"),
        &[JsString::from("invalid param name \"index.\"")],
    );
    assert!(info.clone().is_none());
}

// port: JsDocInfoParserTest#testParseParam24
#[test]
fn test_parse_param24() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {string} foo.bar */"),
        &[JsString::from("invalid param name \"foo.bar\"")],
    );
    assert!(info.clone().is_none());
}

// port: JsDocInfoParserTest#testParseParam26
#[test]
fn test_parse_param26() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {{a: number, b: number}} {a, b}\n*/"),
        &[
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    assert!(info.clone().is_none());
}

// port: JsDocInfoParserTest#testParseParam27
#[test]
fn test_parse_param27() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {{a: number, b: number}} '{a, b}'\n*/"),
        &[JsString::from("invalid param name \"'\"")],
    );
    assert!(info.clone().is_none());
}

// port: JsDocInfoParserTest#testParseThrows1
#[test]
fn test_parse_throws1() {
    let mut f = set_up();
    let mut info =
        f.parse_with_documentation(JsString::from("@throws {number} Some number */"), true, &[]);
    assert_contents(
        ((info.clone()).unwrap()).get_throws_annotations(),
        vec![JsString::from("{number} Some number")],
        false,
    );
}

// port: JsDocInfoParserTest#testParseThrows2
#[test]
fn test_parse_throws2() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(
        JsString::from("@throws {not a type}\n *     Bla\n*@throws {String} A string */\n"),
        true,
        &[],
    );
    assert_contents(
        ((info.clone()).unwrap()).get_throws_annotations(),
        vec![
            JsString::from("{not a type} Bla"),
            JsString::from("{String} A string"),
        ],
        true,
    );
}

// port: JsDocInfoParserTest#testParseParamError5
#[test]
fn test_parse_param_error5() {
    let mut f = set_up();
    f.parse(
        JsString::from("@param {number} x \n * @param {string} x */"),
        &[
            (JsString::from("Bad type annotation. duplicate variable name \"x\""))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseExtendsNullable1
#[test]
fn test_parse_extends_nullable1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@extends {Base?} */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseExtendsNullable2
#[test]
fn test_parse_extends_nullable2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@extends Base? */"),
        &[
            (JsString::from("Bad type annotation. expected end of line or comment."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseExtendsWithQualifier
#[test]
fn test_parse_extends_with_qualifier() {
    let mut f = set_up();
    f.parse(JsString::from("@extends {!Base}*/"), &[((JsString::from("Bad type annotation. @implements/@extends requires a bare interface/record name without !")).concat(&(JsString::from(" or ?.")))).concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))]);
}

// port: JsDocInfoParserTest#testParseExtends_multipleExtends
#[test]
fn test_parse_extends_multiple_extends() {
    let mut f = set_up();
    f.js_doc_source_kind = JsDocSourceKind::NORMAL;
    f.parse(JsString::from("@extends {Foo}\n@extends {Bar} */"), &[((JsString::from("Bad type annotation. type annotation incompatible with other annotations. See")).concat(&(JsString::from(" https://github.com/google/closure-compiler/wiki/Annotating-JavaScript-for-the-Closure-Compiler")))).concat(&(JsString::from(" for more information.")))]);
}

// port: JsDocInfoParserTest#testParseExtends_multipleExtendsTsickleMode
#[test]
fn test_parse_extends_multiple_extends_tsickle_mode() {
    let mut f = set_up();
    f.js_doc_source_kind = JsDocSourceKind::TSICKLE;
    let mut result = f.parse(JsString::from("@extends {Foo}\n@extends {Bar} */"), &[]);
    assert_eq!(
        (((((result.clone()).unwrap()).get_base_type()).unwrap()).get_root()).get_token(&f.ast),
        Token::BANG
    );
    assert_eq!(
        ((((((result.clone()).unwrap()).get_base_type()).unwrap()).get_root())
            .get_only_child(&f.ast))
        .get_string(&f.ast),
        JsString::from("JsDocInfoParser_TsickleMode_MissingSupertypePlaceholder")
    );
}

// port: JsDocInfoParserTest#testParseEnum6
#[test]
fn test_parse_enum6() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from(" @enum {Foo} */"), &[]);
    let mut enum_type_node =
        ((((jsdoc.clone()).unwrap()).get_enum_parameter_type()).unwrap()).get_root();
    assert_eq!((enum_type_node).get_token(&f.ast), Token::BANG);
}

// port: JsDocInfoParserTest#testParseJsDocAfterEnum
#[test]
fn test_parse_js_doc_after_enum() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@enum {string} @suppress {x} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [JsString::from("x")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testParseDesc1
#[test]
fn test_parse_desc1() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@desc hello world!*/"), &[])).unwrap()).get_description(),
        Some(JsString::from("hello world!"))
    );
}

// port: JsDocInfoParserTest#testParseDesc2
#[test]
fn test_parse_desc2() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@desc hello world!\n*/"), &[])).unwrap()).get_description(),
        Some(JsString::from("hello world!"))
    );
}

// port: JsDocInfoParserTest#testParseDesc3
#[test]
fn test_parse_desc3() {
    let mut f = set_up();
    assert!(
        ((((f.parse(JsString::from("@desc*/"), &[])).unwrap()).get_description()).unwrap())
            .is_empty()
    );
}

// port: JsDocInfoParserTest#testParseDesc4
#[test]
fn test_parse_desc4() {
    let mut f = set_up();
    assert!(
        ((((f.parse(JsString::from("@desc\n*/"), &[])).unwrap()).get_description()).unwrap())
            .is_empty()
    );
}

// port: JsDocInfoParserTest#testParseDesc5
#[test]
fn test_parse_desc5() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@desc hello\nworld!\n*/"), &[])).unwrap()).get_description(),
        Some(JsString::from("hello world!"))
    );
}

// port: JsDocInfoParserTest#testParseDesc6
#[test]
fn test_parse_desc6() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@desc hello\n* world!\n*/"), &[])).unwrap()).get_description(),
        Some(JsString::from("hello world!"))
    );
}

// port: JsDocInfoParserTest#testParseDesc7
#[test]
fn test_parse_desc7() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@desc a\n\nb\nc*/"), &[])).unwrap()).get_description(),
        Some(JsString::from("a b c"))
    );
}

// port: JsDocInfoParserTest#testParseDesc8
#[test]
fn test_parse_desc8() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@desc a\n      *b\n\n  *c\n\nd*/"), &[])).unwrap())
            .get_description(),
        Some(JsString::from("a b c d"))
    );
}

// port: JsDocInfoParserTest#testParseDesc9
#[test]
fn test_parse_desc9() {
    let mut f = set_up();
    let mut comment = JsString::from("@desc\n.\n,\n{\n)\n}\n|\n.<\n>\n<\n?\n~\n+\n-\n;\n:\n*/");
    assert_eq!(
        ((f.parse(comment.clone(), &[])).unwrap()).get_description(),
        Some(JsString::from(". , { ) } | < > < ? ~ + - ; :"))
    );
}

// port: JsDocInfoParserTest#testParseDesc10
#[test]
fn test_parse_desc10() {
    let mut f = set_up();
    let mut comment = JsString::from("@desc\n?\n?\n?\n?*/");
    assert_eq!(
        ((f.parse(comment.clone(), &[])).unwrap()).get_description(),
        Some(JsString::from("? ? ? ?"))
    );
}

// port: JsDocInfoParserTest#testParseDesc11
#[test]
fn test_parse_desc11() {
    let mut f = set_up();
    let mut comment = JsString::from("@desc :[]*/");
    assert_eq!(
        ((f.parse(comment.clone(), &[])).unwrap()).get_description(),
        Some(JsString::from(":[]"))
    );
}

// port: JsDocInfoParserTest#testParseDesc12
#[test]
fn test_parse_desc12() {
    let mut f = set_up();
    let mut comment = JsString::from("@desc\n:\n[\n]\n...*/");
    assert_eq!(
        ((f.parse(comment.clone(), &[])).unwrap()).get_description(),
        Some(JsString::from(": [ ] ..."))
    );
}

// port: JsDocInfoParserTest#testParseMeaning1
#[test]
fn test_parse_meaning1() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@meaning tigers   */"), &[])).unwrap()).get_meaning(),
        Some(JsString::from("tigers"))
    );
}

// port: JsDocInfoParserTest#testParseMeaning2
#[test]
fn test_parse_meaning2() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(
            JsString::from("@meaning tigers\n * and lions\n * and bears */"),
            &[]
        ))
        .unwrap())
        .get_meaning(),
        Some(JsString::from("tigers and lions and bears"))
    );
}

// port: JsDocInfoParserTest#testParseMeaning3
#[test]
fn test_parse_meaning3() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@meaning  tigers\n * and lions\n * @desc  and bears */"),
        &[],
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_meaning(),
        Some(JsString::from("tigers and lions"))
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_description(),
        Some(JsString::from("and bears"))
    );
}

// port: JsDocInfoParserTest#testParseMeaning4
#[test]
fn test_parse_meaning4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@meaning  tigers\n * @meaning and lions  */"),
        &[JsString::from("extra @meaning tag")],
    );
}

// port: JsDocInfoParserTest#testParseAlternateMessageId1
#[test]
fn test_parse_alternate_message_id1() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@alternateMessageId A   */"), &[])).unwrap())
            .get_alternate_message_id(),
        Some(JsString::from("A"))
    );
}

// port: JsDocInfoParserTest#testParseAlternateMessageId2
#[test]
fn test_parse_alternate_message_id2() {
    let mut f = set_up();
    assert_eq!(
        ((f.parse(JsString::from("@alternateMessageId A\n * B\n * C */"), &[])).unwrap())
            .get_alternate_message_id(),
        Some(JsString::from("A"))
    );
}

// port: JsDocInfoParserTest#testParseAlternateMessageId3
#[test]
fn test_parse_alternate_message_id3() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@alternateMessageId  A\n * B\n * @desc  C */"),
        &[],
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_alternate_message_id(),
        Some(JsString::from("A"))
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_description(),
        Some(JsString::from("C"))
    );
}

// port: JsDocInfoParserTest#testParseAlternateMessageId4
#[test]
fn test_parse_alternate_message_id4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@alternateMessageId  A\n * @alternateMessageId B  */"),
        &[JsString::from("extra @alternateMessageId tag")],
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitive
#[test]
fn test_parse_closure_primitive() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@closurePrimitive {id} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_closure_primitive_id(),
        Some(JsString::from("id"))
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveOnNewLine
#[test]
fn test_parse_closure_primitive_on_new_line() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@closurePrimitive    \n\n {id} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_closure_primitive_id(),
        Some(JsString::from("id"))
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveWithAnnotationAfterId
#[test]
fn test_parse_closure_primitive_with_annotation_after_id() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@closurePrimitive {id} @const */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_closure_primitive_id(),
        Some(JsString::from("id"))
    );
    assert!((((info.clone()).unwrap()).is_constant()));
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveWithStringAfterId
#[test]
fn test_parse_closure_primitive_with_string_after_id() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@closurePrimitive {id} some comment\n@const */"),
        &[],
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_closure_primitive_id(),
        Some(JsString::from("id"))
    );
    assert!((((info.clone()).unwrap()).is_constant()));
}

// port: JsDocInfoParserTest#testParseTsType_typesOnlyMode
#[test]
fn test_parse_ts_type_types_only_mode() {
    let mut f = set_up();
    assert_contents(
        ((f.parse_with_mode(
            JsString::from("@tsType ():this */"),
            JsDocParsing::TYPES_ONLY,
            &[],
        ))
        .unwrap())
        .get_ts_types(),
        vec![JsString::from("JsDocInfoParser_tsType_PlaceHolder")],
        false,
    );
}

// port: JsDocInfoParserTest#testParseTsType_includeDescriptions
#[test]
fn test_parse_ts_type_include_descriptions() {
    let mut f = set_up();
    assert_contents(
        ((f.parse_with_mode(
            JsString::from("@tsType ():this */"),
            JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
            &[],
        ))
        .unwrap())
        .get_ts_types(),
        vec![JsString::from("():this")],
        false,
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveIdMissingIdentifier
#[test]
fn test_parse_closure_primitive_id_missing_identifier() {
    let mut f = set_up();
    f.parse(
        JsString::from("@closurePrimitive */"),
        &[JsString::from("missing opening {")],
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveIdInvalidId
#[test]
fn test_parse_closure_primitive_id_invalid_id() {
    let mut f = set_up();
    f.parse(
        JsString::from("@closurePrimitive {unrecognizedId } */"),
        &[JsString::from("invalid id in @closurePrimitive tag.")],
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveIdMissingLeftCurly
#[test]
fn test_parse_closure_primitive_id_missing_left_curly() {
    let mut f = set_up();
    f.parse(
        JsString::from("@closurePrimitive id} */"),
        &[JsString::from("missing opening {")],
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveIdMissingRightCurly
#[test]
fn test_parse_closure_primitive_id_missing_right_curly() {
    let mut f = set_up();
    f.parse(
        JsString::from("@closurePrimitive {id */"),
        &[JsString::from("expected closing }")],
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveIdMissingStringInCurly
#[test]
fn test_parse_closure_primitive_id_missing_string_in_curly() {
    let mut f = set_up();
    f.parse(
        JsString::from("@closurePrimitive {} */"),
        &[JsString::from("missing id in @closurePrimitive tag.")],
    );
}

// port: JsDocInfoParserTest#testParseClosurePrimitiveDuplicateTags
#[test]
fn test_parse_closure_primitive_duplicate_tags() {
    let mut f = set_up();
    f.parse(
        JsString::from("@closurePrimitive {idA}\n@closurePrimitive {idB} */"),
        &[JsString::from("conflicting @closurePrimitive tag")],
    );
}

// port: JsDocInfoParserTest#testParseLends1
#[test]
fn test_parse_lends1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@lends {name} */"), &[]);
    assert_eq!(
        (((((info.clone()).unwrap()).get_lends_name()).unwrap()).get_root()).get_string(&f.ast),
        JsString::from("name")
    );
}

// port: JsDocInfoParserTest#testParseLends2
#[test]
fn test_parse_lends2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@lends   foo.bar  */"), &[]);
    assert_eq!(
        (((((info.clone()).unwrap()).get_lends_name()).unwrap()).get_root()).get_string(&f.ast),
        JsString::from("foo.bar")
    );
}

// port: JsDocInfoParserTest#testParseLends3
#[test]
fn test_parse_lends3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@lends {name */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseLends4
#[test]
fn test_parse_lends4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@lends {} */"),
        &[
            (JsString::from("Bad type annotation. missing object name in @lends tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseLends5
#[test]
fn test_parse_lends5() {
    let mut f = set_up();
    f.parse(
        JsString::from("@lends } */"),
        &[
            (JsString::from("Bad type annotation. missing object name in @lends tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseLends6
#[test]
fn test_parse_lends6() {
    let mut f = set_up();
    f.parse(
        JsString::from("@lends {string} \n * @lends {string} */"),
        &[
            (JsString::from(
                "Bad type annotation. @lends tag incompatible with other annotations.",
            ))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseLends7
#[test]
fn test_parse_lends7() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {string} \n * @lends {string} */"),
        &[
            (JsString::from(
                "Bad type annotation. @lends tag incompatible with other annotations.",
            ))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseAbstract
#[test]
fn test_parse_abstract() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@abstract */"), &[]);
    assert!((((info.clone()).unwrap()).is_abstract()));
}

// port: JsDocInfoParserTest#testParseAbstract_alreadyAbstract
#[test]
fn test_parse_abstract_already_abstract() {
    let mut f = set_up();
    f.parse(
        JsString::from("@abstract \n * @abstract */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_typedefBeforeAbstract
#[test]
fn test_parse_abstract_typedef_before_abstract() {
    let mut f = set_up();
    f.parse(
        JsString::from("@typedef {Object<T,T>} \n * @abstract */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_typeBeforeAbstract
#[test]
fn test_parse_abstract_type_before_abstract() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @type {Object<T,T>} \n * @abstract */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_interfaceBeforeAbstract
#[test]
fn test_parse_abstract_interface_before_abstract() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @interface \n * @abstract */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_abstractBeforeTypedef
#[test]
fn test_parse_abstract_abstract_before_typedef() {
    let mut f = set_up();
    f.parse(
        JsString::from("@abstract \n * @typedef {Object<T,T>} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_abstractBeforeType
#[test]
fn test_parse_abstract_abstract_before_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @abstract \n * @type {Object<T,T>} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_abstractBeforeInterface
#[test]
fn test_parse_abstract_abstract_before_interface() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @abstract \n * @interface */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_abstractAndNotPrivate
#[test]
fn test_parse_abstract_abstract_and_not_private() {
    let mut f = set_up();
    let mut info1 = f.parse(JsString::from("* @public \n * @abstract */"), &[]);
    assert!((((info1.clone()).unwrap()).is_abstract()));
    let mut info2 = f.parse(JsString::from("* @protected \n * @abstract */"), &[]);
    assert!((((info2.clone()).unwrap()).is_abstract()));
    let mut info3 = f.parse(JsString::from("* @package \n * @abstract */"), &[]);
    assert!((((info3.clone()).unwrap()).is_abstract()));
}

// port: JsDocInfoParserTest#testParseAbstract_abstractAndPrivate
#[test]
fn test_parse_abstract_abstract_and_private() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @private \n * @abstract */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseAbstract_abstractAndFinal
#[test]
fn test_parse_abstract_abstract_and_final() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @final \n * @abstract */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testStackedAnnotation3
#[test]
fn test_stacked_annotation3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@const @see {string}*/"), &[]);
    assert!((((info.clone()).unwrap()).is_constant()));
    assert!(!(((info.clone()).unwrap()).has_type()));
}

// port: JsDocInfoParserTest#testStackedAnnotation4
#[test]
fn test_stacked_annotation4() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@constructor @extends {Foo} @implements {Bar}*/"),
        &[],
    );
    assert!((((info.clone()).unwrap()).is_constructor()));
    assert!((((info.clone()).unwrap()).has_base_type()));
    assert_eq!(
        ((info.clone()).unwrap()).get_implemented_interface_count(),
        1
    );
}

// port: JsDocInfoParserTest#testStackedAnnotation5
#[test]
fn test_stacked_annotation5() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {number} x @constructor */"), &[]);
    assert!((((info.clone()).unwrap()).has_parameter_type(JsString::from("x"))));
    assert!((((info.clone()).unwrap()).is_constructor()));
}

// port: JsDocInfoParserTest#testStackedAnnotation6
#[test]
fn test_stacked_annotation6() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(
        JsString::from("@return {number} @constructor */"),
        true,
        &[],
    );
    assert!((((info.clone()).unwrap()).has_return_type()));
    assert!((((info.clone()).unwrap()).is_constructor()));
    info = f.parse_with_documentation(
        JsString::from("@return {number} @constructor */"),
        false,
        &[],
    );
    assert!((((info.clone()).unwrap()).has_return_type()));
    assert!((((info.clone()).unwrap()).is_constructor()));
}

// port: JsDocInfoParserTest#testStackedAnnotation7
#[test]
fn test_stacked_annotation7() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@return @constructor */"),
        &[JsString::from(MISSING_TYPE_DECL_WARNING_TEXT)],
    );
    assert!((((info.clone()).unwrap()).has_return_type()));
    assert!((((info.clone()).unwrap()).is_constructor()));
}

// port: JsDocInfoParserTest#testStackedAnnotation8
#[test]
fn test_stacked_annotation8() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(
        JsString::from("@throws {number} @constructor */"),
        true,
        &[],
    );
    assert!(!(((info.clone()).unwrap()).get_throws_annotations()).is_empty());
    assert!(!(((info.clone()).unwrap()).is_constructor()));
    info = f.parse_with_documentation(
        JsString::from("@return {number} @constructor */"),
        false,
        &[],
    );
    assert!((((info.clone()).unwrap()).has_return_type()));
    assert!((((info.clone()).unwrap()).is_constructor()));
}

// port: JsDocInfoParserTest#testStackedAnnotation11
#[test]
fn test_stacked_annotation11() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(JsString::from("@private @const */"), true, &[]);
    assert!(!(((info.clone()).unwrap()).has_type()));
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PRIVATE
    );
    assert!((((info.clone()).unwrap()).is_constant()));
}

// port: JsDocInfoParserTest#testStackedAnnotation12
#[test]
fn test_stacked_annotation12() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(JsString::from("@const @private */"), true, &[]);
    assert!(!(((info.clone()).unwrap()).has_type()));
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PRIVATE
    );
    assert!((((info.clone()).unwrap()).is_constant()));
}

// port: JsDocInfoParserTest#testStackedAnnotation13
#[test]
fn test_stacked_annotation13() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(JsString::from("@final @constructor */"), true, &[]);
    assert!((((info.clone()).unwrap()).is_constructor()));
    assert!((((info.clone()).unwrap()).is_final()));
}

// port: JsDocInfoParserTest#testStackedAnnotation14
#[test]
fn test_stacked_annotation14() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(JsString::from("@constructor @final */"), true, &[]);
    assert!((((info.clone()).unwrap()).is_constructor()));
    assert!((((info.clone()).unwrap()).is_final()));
}

// port: JsDocInfoParserTest#testParsePreserve
#[test]
fn test_parse_preserve() {
    let mut f = set_up();
    let mut comment = JsString::from("@preserve Foo\nBar\n\nBaz*/");
    f.parse(comment.clone(), &[]);
    assert_eq!(
        f.prev_license.clone(),
        Some(JsString::from(" Foo\nBar\n\nBaz"))
    );
}

// port: JsDocInfoParserTest#testParseLicense
#[test]
fn test_parse_license() {
    let mut f = set_up();
    let mut comment = JsString::from("@license Foo\nBar\n\nBaz*/");
    f.parse(comment.clone(), &[]);
    assert_eq!(
        f.prev_license.clone(),
        Some(JsString::from(" Foo\nBar\n\nBaz"))
    );
}

// port: JsDocInfoParserTest#testParseLicenseAscii
#[test]
fn test_parse_license_ascii() {
    let mut f = set_up();
    let mut comment = JsString::from("@license Foo\n *   Bar\n\n  Baz*/");
    f.parse(comment.clone(), &[]);
    assert_eq!(
        f.prev_license.clone(),
        Some(JsString::from(" Foo\n   Bar\n\n  Baz"))
    );
}

// port: JsDocInfoParserTest#testParseLicenseWithAnnotation
#[test]
fn test_parse_license_with_annotation() {
    let mut f = set_up();
    let mut comment = JsString::from("@license Foo \n * @author Charlie Brown */");
    f.parse(comment.clone(), &[]);
    assert_eq!(
        f.prev_license.clone(),
        Some(JsString::from(" Foo \n @author Charlie Brown "))
    );
}

// port: JsDocInfoParserTest#testParseLicenseMultiple
#[test]
fn test_parse_license_multiple() {
    let mut f = set_up();
    let mut comment = JsString::from("@license Foo @license Bar */");
    f.parse(comment.clone(), &[]);
    assert_eq!(
        f.prev_license.clone(),
        Some(JsString::from(" Foo @license Bar "))
    );
}

// port: JsDocInfoParserTest#testParseLicenseOnlyContentParsesLicenseWithInvalidJsDocTag
#[test]
fn test_parse_license_only_content_parses_license_with_invalid_js_doc_tag() {
    let mut f = set_up();
    let mut comment =
        JsString::from("@date 11/12/2023\n@nosideeffects\n@deprecated\n@license Foo\n*/");
    let mut r = f.parse_with_mode(comment.clone(), JsDocParsing::LICENSE_COMMENTS_ONLY, &[]);
    assert!(r.clone().is_some());
    assert_eq!(f.prev_license.clone(), Some(JsString::from(" Foo\n")));
    assert!(!(((r.clone()).unwrap()).is_no_side_effects()));
    assert!(!(((r.clone()).unwrap()).is_deprecated()));
    assert_contents(
        ((((r.clone()).unwrap()).get_markers()).into_iter())
            .map(|m| {
                (((m).get_annotation().cloned()).unwrap())
                    .get_item()
                    .cloned()
            })
            .collect::<Vec<_>>(),
        ((vec![JsString::from("license")])
            .into_iter()
            .collect::<Vec<_>>())
        .into_iter()
        .map(Some)
        .collect(),
        false,
    );
}

// port: JsDocInfoParserTest#testParseLicenseOnlyContentParsesPreservedCommentWithInvalidJsDocTag
#[test]
fn test_parse_license_only_content_parses_preserved_comment_with_invalid_js_doc_tag() {
    let mut f = set_up();
    let mut comment =
        JsString::from("@date 11/12/2023\n@nosideeffects\n@deprecated\n@preserve Foo\n*/");
    let mut r = f.parse_with_mode(comment.clone(), JsDocParsing::LICENSE_COMMENTS_ONLY, &[]);
    assert!(r.clone().is_some());
    assert_eq!(f.prev_license.clone(), Some(JsString::from(" Foo\n")));
    assert!(!(((r.clone()).unwrap()).is_no_side_effects()));
    assert!(!(((r.clone()).unwrap()).is_deprecated()));
    assert_contents(
        ((((r.clone()).unwrap()).get_markers()).into_iter())
            .map(|m| {
                (((m).get_annotation().cloned()).unwrap())
                    .get_item()
                    .cloned()
            })
            .collect::<Vec<_>>(),
        ((vec![JsString::from("preserve")])
            .into_iter()
            .collect::<Vec<_>>())
        .into_iter()
        .map(Some)
        .collect(),
        false,
    );
}

// port: JsDocInfoParserTest#testCantParseLicenseWithInvalidJsDocTag
#[test]
fn test_cant_parse_license_with_invalid_js_doc_tag() {
    let mut f = set_up();
    let mut comment = JsString::from("@date 11/12/2023\n@license Foo\n*/");
    f.parse(comment.clone(), &[(JsString::from("illegal use of unknown JSDoc tag \"date\"; ignoring it. Place another character before the")).concat(&(JsString::from(" @ to stop JSCompiler from parsing it as an annotation.")))]);
    assert_eq!(f.prev_license.clone(), Some(JsString::from(" Foo\n")));
}

// port: JsDocInfoParserTest#testParseDefineDescription
#[test]
fn test_parse_define_description() {
    let mut f = set_up();
    let mut doc = f.parse_with_documentation(
        JsString::from("@define {string} description of element \n next line*/"),
        true,
        &[],
    );
    let mut define_marker = ((((doc.clone()).unwrap()).get_markers()).into_iter()).next();
    assert_eq!(
        ((((define_marker.clone()).unwrap()).get_annotation().cloned()).unwrap())
            .get_item()
            .cloned(),
        Some(JsString::from("define"))
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("description of element")))
            >= 0
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("next line")))
            >= 0
    );
}

// port: JsDocInfoParserTest#testParsePrivateDescription
#[test]
fn test_parse_private_description() {
    let mut f = set_up();
    let mut doc = f.parse_with_documentation(
        JsString::from("@private {string} description \n next line*/"),
        true,
        &[],
    );
    let mut define_marker = ((((doc.clone()).unwrap()).get_markers()).into_iter()).next();
    assert_eq!(
        ((((define_marker.clone()).unwrap()).get_annotation().cloned()).unwrap())
            .get_item()
            .cloned(),
        Some(JsString::from("private"))
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("description ")))
            >= 0
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("next line")))
            >= 0
    );
}

// port: JsDocInfoParserTest#testParsePackagePrivateDescription
#[test]
fn test_parse_package_private_description() {
    let mut f = set_up();
    let mut doc = f.parse_with_documentation(
        JsString::from("@package {string} description \n next line */"),
        true,
        &[],
    );
    let mut define_marker = ((((doc.clone()).unwrap()).get_markers()).into_iter()).next();
    assert_eq!(
        ((((define_marker.clone()).unwrap()).get_annotation().cloned()).unwrap())
            .get_item()
            .cloned(),
        Some(JsString::from("package"))
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("description ")))
            >= 0
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("next line")))
            >= 0
    );
}

// port: JsDocInfoParserTest#testParseProtectedDescription
#[test]
fn test_parse_protected_description() {
    let mut f = set_up();
    let mut doc = f.parse_with_documentation(
        JsString::from("@protected {string} description \n next line*/"),
        true,
        &[],
    );
    let mut define_marker = ((((doc.clone()).unwrap()).get_markers()).into_iter()).next();
    assert_eq!(
        ((((define_marker.clone()).unwrap()).get_annotation().cloned()).unwrap())
            .get_item()
            .cloned(),
        Some(JsString::from("protected"))
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("description ")))
            >= 0
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("next line")))
            >= 0
    );
}

// port: JsDocInfoParserTest#testParseDefineErrors1
#[test]
fn test_parse_define_errors1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@enum {string}\n @define {string} */"),
        &[JsString::from("conflicting @define tag")],
    );
}

// port: JsDocInfoParserTest#testParseDefineErrors2
#[test]
fn test_parse_define_errors2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@define {string}\n@enum {string} */\n"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseDefineErrors3
#[test]
fn test_parse_define_errors3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@const\n @define {string} */"),
        &[JsString::from("conflicting @define tag")],
    );
}

// port: JsDocInfoParserTest#testParseDefineErrors4
#[test]
fn test_parse_define_errors4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {string} \n @define {string} */"),
        &[JsString::from("conflicting @define tag")],
    );
}

// port: JsDocInfoParserTest#testParseDefineErrors5
#[test]
fn test_parse_define_errors5() {
    let mut f = set_up();
    f.parse(
        JsString::from("@return {string}\n @define {string} */"),
        &[JsString::from("conflicting @define tag")],
    );
}

// port: JsDocInfoParserTest#testParseDefineErrors7
#[test]
fn test_parse_define_errors7() {
    let mut f = set_up();
    assert!(
        (((f.parse(JsString::from("@define {string}\n @const */"), &[])).unwrap()).is_constant())
    );
}

// port: JsDocInfoParserTest#testParseDefineErrors8
#[test]
fn test_parse_define_errors8() {
    let mut f = set_up();
    f.parse(
        JsString::from("@define {string}\n @type {string} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFinal1
#[test]
fn test_parse_final1() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@final*/"), &[])).unwrap()).is_final()));
}

// port: JsDocInfoParserTest#testParseFinal2
#[test]
fn test_parse_final2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@final\n@final*/"),
        &[(JsString::from("Bad type annotation. extra @final tag."))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseFinal3
#[test]
fn test_parse_final3() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@final*/"), &[])).unwrap()).is_constant()));
}

// port: JsDocInfoParserTest#testParseOverride1
#[test]
fn test_parse_override1() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@override*/"), &[])).unwrap()).is_override()));
}

// port: JsDocInfoParserTest#testParseOverride2
#[test]
fn test_parse_override2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@override\n@override*/"),
        &[
            (JsString::from("Bad type annotation. extra @override/@inheritDoc tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseInheritDoc1
#[test]
fn test_parse_inherit_doc1() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@inheritDoc*/"), &[])).unwrap()).is_override()));
}

// port: JsDocInfoParserTest#testParseInheritDoc2
#[test]
fn test_parse_inherit_doc2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@override\n@inheritDoc*/"),
        &[
            (JsString::from("Bad type annotation. extra @override/@inheritDoc tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseInheritDoc3
#[test]
fn test_parse_inherit_doc3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@inheritDoc\n@inheritDoc*/"),
        &[
            (JsString::from("Bad type annotation. extra @override/@inheritDoc tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseDeprecated1
#[test]
fn test_parse_deprecated1() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@deprecated*/"), &[])).unwrap()).is_deprecated()));
}

// port: JsDocInfoParserTest#testParseDeprecated2
#[test]
fn test_parse_deprecated2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@deprecated\n@deprecated*/"),
        &[JsString::from("extra @deprecated tag")],
    );
}

// port: JsDocInfoParserTest#testParseExport1
#[test]
fn test_parse_export1() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@export*/"), &[])).unwrap()).is_export()));
}

// port: JsDocInfoParserTest#testParseExport2
#[test]
fn test_parse_export2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@export\n@export*/"),
        &[JsString::from("extra @export tag")],
    );
}

// port: JsDocInfoParserTest#testParseExterns1
#[test]
fn test_parse_externs1() {
    let mut f = set_up();
    assert!((((f.parse_file_overview(JsString::from("@externs*/"), &[])).unwrap()).is_externs()));
}

// port: JsDocInfoParserTest#testParseExterns2
#[test]
fn test_parse_externs2() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@externs\n@externs*/"),
        &[JsString::from("extra @externs tag")],
    );
}

// port: JsDocInfoParserTest#testParseExterns3
#[test]
fn test_parse_externs3() {
    let mut f = set_up();
    assert!(f.parse(JsString::from("@externs*/"), &[]).is_none());
}

// port: JsDocInfoParserTest#testParseTypeSummary1
#[test]
fn test_parse_type_summary1() {
    let mut f = set_up();
    assert!(
        (((f.parse_file_overview(JsString::from("@typeSummary*/"), &[])).unwrap())
            .is_type_summary())
    );
}

// port: JsDocInfoParserTest#testParseTypeSummary2
#[test]
fn test_parse_type_summary2() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@typeSummary\n@typeSummary*/"),
        &[JsString::from("extra @typeSummary tag")],
    );
}

// port: JsDocInfoParserTest#testParseTypeSummary3
#[test]
fn test_parse_type_summary3() {
    let mut f = set_up();
    assert!(f.parse(JsString::from("@typeSummary*/"), &[]).is_none());
}

// port: JsDocInfoParserTest#testParseNoCompile1
#[test]
fn test_parse_no_compile1() {
    let mut f = set_up();
    assert!(
        (((f.parse_file_overview(JsString::from("@nocompile*/"), &[])).unwrap()).is_no_compile())
    );
}

// port: JsDocInfoParserTest#testParseNoCompile2
#[test]
fn test_parse_no_compile2() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@nocompile\n@nocompile*/"),
        &[JsString::from("extra @nocompile tag")],
    );
}

// port: JsDocInfoParserTest#testParseClosureUnawareCode1
#[test]
fn test_parse_closure_unaware_code1() {
    let mut f = set_up();
    assert!(
        (((f.parse_file_overview(
            JsString::from("@fileoverview\n@closureUnaware*/"),
            &[JsString::from(
                "@closureUnaware annotation is not allowed in this compilation"
            )]
        ))
        .unwrap())
        .is_closure_unaware_code())
    );
}

// port: JsDocInfoParserTest#testParseClosureUnawareCode2
#[test]
fn test_parse_closure_unaware_code2() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@closureUnaware\n@closureUnaware*/"),
        &[
            JsString::from("@closureUnaware annotation is not allowed in this compilation"),
            JsString::from("@closureUnaware annotation is not allowed in this compilation"),
            JsString::from("extra @closureUnaware tag"),
        ],
    );
}

// port: JsDocInfoParserTest#testParseNoClosureUnawareCode
#[test]
fn test_parse_no_closure_unaware_code() {
    let mut f = set_up();
    assert!(
        !(((f.parse_file_overview(JsString::from("@fileoverview*/"), &[])).unwrap())
            .is_closure_unaware_code())
    );
}

// port: JsDocInfoParserTest#testParseClosureUnawareCode
#[test]
fn test_parse_closure_unaware_code() {
    let mut f = set_up();
    assert!(
        (((f.parse(
            JsString::from("@closureUnaware*/"),
            &[JsString::from(
                "@closureUnaware annotation is not allowed in this compilation"
            )]
        ))
        .unwrap())
        .is_closure_unaware_code())
    );
}

// port: JsDocInfoParserTest#testParseClosureUnawareCode_allowed
#[test]
fn test_parse_closure_unaware_code_allowed() {
    let mut f = set_up();
    f.allow_closure_unaware_code = true;
    assert!(
        (((f.parse(JsString::from("@closureUnaware*/"), &[])).unwrap()).is_closure_unaware_code())
    );
}

// port: JsDocInfoParserTest#testParseClosureUnawareCode_fileoverview_allowed
#[test]
fn test_parse_closure_unaware_code_fileoverview_allowed() {
    let mut f = set_up();
    f.allow_closure_unaware_code = true;
    assert!(
        (((f.parse_file_overview(JsString::from("@fileoverview\n@closureUnaware*/"), &[]))
            .unwrap())
        .is_closure_unaware_code())
    );
}

// port: JsDocInfoParserTest#testParseClosureUnawareCode_doesNotCreateFileoverviewCommentStandalone
#[test]
fn test_parse_closure_unaware_code_does_not_create_fileoverview_comment_standalone() {
    let mut f = set_up();
    f.allow_closure_unaware_code = true;
    assert!(
        f.parse_file_overview(JsString::from("@closureUnaware*/"), &[])
            .is_none()
    );
}

// port: JsDocInfoParserTest#testParseNoCoverage_fileOverview
#[test]
fn test_parse_no_coverage_file_overview() {
    let mut f = set_up();
    assert!(
        (((f.parse_file_overview(JsString::from("@nocoverage*/"), &[])).unwrap()).is_no_coverage())
    );
}

// port: JsDocInfoParserTest#testParseNoCoverage_extraTag
#[test]
fn test_parse_no_coverage_extra_tag() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@nocoverage\n@nocoverage*/"),
        &[JsString::from("extra @nocoverage tag")],
    );
}

// port: JsDocInfoParserTest#testParseNoCoverage_nonFileOverview
#[test]
fn test_parse_no_coverage_non_file_overview() {
    let mut f = set_up();
    assert!(f.parse(JsString::from("@nocoverage*/"), &[]).is_none());
}

// port: JsDocInfoParserTest#testParseNoDts1
#[test]
fn test_parse_no_dts1() {
    let mut f = set_up();
    let mut doc = f.parse_with_documentation(JsString::from("@nodts*/"), true, &[]);
    assert!((((doc.clone()).unwrap()).is_no_dts()));
}

// port: JsDocInfoParserTest#testParseNoDts2
#[test]
fn test_parse_no_dts2() {
    let mut f = set_up();
    f.parse_with_mode(
        JsString::from("@nodts\n@nodts*/"),
        JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
        &[JsString::from("extra @nodts tag")],
    );
}

// port: JsDocInfoParserTest#testBugAnnotation
#[test]
fn test_bug_annotation() {
    let mut f = set_up();
    f.parse(JsString::from("@bug */"), &[]);
}

// port: JsDocInfoParserTest#testDescriptionAnnotation
#[test]
fn test_description_annotation() {
    let mut f = set_up();
    f.parse(JsString::from("@description */"), &[]);
}

// port: JsDocInfoParserTest#testRegression3
#[test]
fn test_regression3() {
    let mut f = set_up();
    let mut comment = JsString::from(
        " * @param mediaTag this specified whether the @media tag is ....\n *\n\n@public\n *\n\n **********\n * @const\n */\n",
    );
    let mut info = f.parse(
        comment.clone(),
        &[JsString::from(MISSING_TYPE_DECL_WARNING_TEXT)],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    assert!(
        ((info.clone()).unwrap())
            .get_parameter_type(JsString::from("mediaTag"))
            .is_none()
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PUBLIC
    );
    assert!((((info.clone()).unwrap()).is_constant()));
}

// port: JsDocInfoParserTest#testRegression4
#[test]
fn test_regression4() {
    let mut f = set_up();
    let mut comment = JsString::from("* @const\n* @private\n* @constructor\n*/\n");
    let mut info = f.parse(comment.clone(), &[]);
    assert!((((info.clone()).unwrap()).is_constant()));
    assert!(!(((info.clone()).unwrap()).is_define()));
    assert!((((info.clone()).unwrap()).is_constructor()));
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PRIVATE
    );
}

// port: JsDocInfoParserTest#testRegression11
#[test]
fn test_regression11() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("* @constructor\n*/\n"), &[]);
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert!(!(((jsdoc.clone()).unwrap()).is_constant()));
    assert!(((jsdoc.clone()).unwrap()).get_description().is_none());
    assert!(
        ((jsdoc.clone()).unwrap())
            .get_enum_parameter_type()
            .is_none()
    );
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 0);
    assert!(((jsdoc.clone()).unwrap()).get_return_type().is_none());
    assert!(((jsdoc.clone()).unwrap()).get_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::INHERITED
    );
}

// port: JsDocInfoParserTest#testRegression14
#[test]
fn test_regression14() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("* @const\n* @private\n*/\n"), &[]);
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert!((((jsdoc.clone()).unwrap()).is_constant()));
    assert!(((jsdoc.clone()).unwrap()).get_description().is_none());
    assert!(
        ((jsdoc.clone()).unwrap())
            .get_enum_parameter_type()
            .is_none()
    );
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 0);
    assert!(((jsdoc.clone()).unwrap()).get_return_type().is_none());
    assert!(((jsdoc.clone()).unwrap()).get_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::PRIVATE
    );
}

// port: JsDocInfoParserTest#testRegression15
#[test]
fn test_regression15() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("* @desc Hello,\n* World!\n*/\n"), &[]);
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert!((((jsdoc.clone()).unwrap()).is_constant()));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description(),
        Some(JsString::from("Hello, World!"))
    );
    assert!(
        ((jsdoc.clone()).unwrap())
            .get_enum_parameter_type()
            .is_none()
    );
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 0);
    assert!(((jsdoc.clone()).unwrap()).get_return_type().is_none());
    assert!(((jsdoc.clone()).unwrap()).get_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::INHERITED
    );
    assert!(!(((jsdoc.clone()).unwrap()).is_export()));
}

// port: JsDocInfoParserTest#testRegression17
#[test]
fn test_regression17() {
    let mut f = set_up();
    assert!(
        ((f.parse(JsString::from("@private*/"), &[])).unwrap())
            .get_description()
            .is_none()
    );
}

// port: JsDocInfoParserTest#testBug907488
#[test]
fn test_bug907488() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {number,null} */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testBug907494
#[test]
fn test_bug907494() {
    let mut f = set_up();
    f.parse(
        JsString::from("@return {Object,undefined} */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testBug909468
#[test]
fn test_bug909468() {
    let mut f = set_up();
    f.parse(
        JsString::from("@extends {(x)}*/"),
        &[
            (JsString::from("Bad type annotation. expecting a type name."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseInterface
#[test]
fn test_parse_interface() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@interface*/"), &[])).unwrap()).is_interface()));
}

// port: JsDocInfoParserTest#testParseImplicitCast1
#[test]
fn test_parse_implicit_cast1() {
    let mut f = set_up();
    assert!(
        (((f.parse(JsString::from("@type {string} \n * @implicitCast*/"), &[])).unwrap())
            .is_implicit_cast())
    );
}

// port: JsDocInfoParserTest#testParseImplicitCast2
#[test]
fn test_parse_implicit_cast2() {
    let mut f = set_up();
    assert!(!(((f.parse(JsString::from("@type {string}*/"), &[])).unwrap()).is_implicit_cast()));
}

// port: JsDocInfoParserTest#testParseDuplicateImplicitCast
#[test]
fn test_parse_duplicate_implicit_cast() {
    let mut f = set_up();
    f.parse(
        JsString::from("@type {string} \n * @implicitCast \n * @implicitCast*/"),
        &[
            (JsString::from("Bad type annotation. extra @implicitCast tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseInterfaceDoubled
#[test]
fn test_parse_interface_doubled() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @interface\n* @interface\n*/\n"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testInvalidTemplatedTypedef1
#[test]
fn test_invalid_templated_typedef1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T \n * @typedef {Object<T,T>} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testInvalidTemplatedTypedef2
#[test]
fn test_invalid_templated_typedef2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@typedef {Array<T>} \n * @template T */"),
        &[(JsString::from(
            "Bad type annotation. Type name(s) for @template annotation declared twice.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseImplementsSameTwice
#[test]
fn test_parse_implements_same_twice() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @implements {Smth}\n* @implements {Smth}\n*/\n"),
        &[
            (JsString::from("Bad type annotation. duplicate @implements tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseImplementsNoName
#[test]
fn test_parse_implements_no_name() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @implements {} */"),
        &[
            (JsString::from("Bad type annotation. expecting a type name."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseImplementsMissingRC
#[test]
fn test_parse_implements_missing_rc() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @implements {Smth */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseImplementsNullable1
#[test]
fn test_parse_implements_nullable1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@implements {Base?} */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseImplementsNullable2
#[test]
fn test_parse_implements_nullable2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@implements Base? */"),
        &[
            (JsString::from("Bad type annotation. expected end of line or comment."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseImplementsWithQualifier
#[test]
fn test_parse_implements_with_qualifier() {
    let mut f = set_up();
    f.parse(JsString::from("@implements {!Base}*/"), &[((JsString::from("Bad type annotation. @implements/@extends requires a bare interface/record name without !")).concat(&(JsString::from(" or ?.")))).concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))]);
}

// port: JsDocInfoParserTest#testBadClassMultiExtends
#[test]
fn test_bad_class_multi_extends() {
    let mut f = set_up();
    f.parse(
        JsString::from("* @extends {Extended1} \n* @constructor \n* @extends {Extended2} */\n"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testBadTypeDefInterfaceAndConstructor1
#[test]
fn test_bad_type_def_interface_and_constructor1() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@interface\n@constructor*/"),
        &[
            (JsString::from("Bad type annotation. cannot be both an interface and a constructor."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    assert!((((jsdoc.clone()).unwrap()).is_interface()));
}

// port: JsDocInfoParserTest#testBadTypeDefInterfaceAndConstructor2
#[test]
fn test_bad_type_def_interface_and_constructor2() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@constructor\n@interface*/"),
        &[
            (JsString::from("Bad type annotation. cannot be both an interface and a constructor."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    assert!((((jsdoc.clone()).unwrap()).is_constructor()));
}

// port: JsDocInfoParserTest#testBadTypeDefInterfaceAndStructuralTyping1
#[test]
fn test_bad_type_def_interface_and_structural_typing1() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@constructor\n@record*/"),
        &[
            (JsString::from("Bad type annotation. conflicting @record tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    assert!(!(((jsdoc.clone()).unwrap()).is_interface()));
}

// port: JsDocInfoParserTest#testBadTypeDefInterfaceAndStructuralTyping2
#[test]
fn test_bad_type_def_interface_and_structural_typing2() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@type{number}\n@record*/"),
        &[
            (JsString::from("Bad type annotation. conflicting @record tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    assert!(!(((jsdoc.clone()).unwrap()).is_interface()));
}

// port: JsDocInfoParserTest#testBadTypeDefInterfaceAndStructuralTyping3
#[test]
fn test_bad_type_def_interface_and_structural_typing3() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@type{{x:number}}\n@record*/"),
        &[
            (JsString::from("Bad type annotation. conflicting @record tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    assert!(!(((jsdoc.clone()).unwrap()).is_interface()));
}

// port: JsDocInfoParserTest#testBadTypeDefInterfaceAndStructuralTyping4
#[test]
fn test_bad_type_def_interface_and_structural_typing4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@interface\n@record*/"),
        &[
            (JsString::from("Bad type annotation. conflicting @record tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testBadTypeDefInterfaceAndStructuralTyping5
#[test]
fn test_bad_type_def_interface_and_structural_typing5() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("@record*/"), &[]);
    assert!((((jsdoc.clone()).unwrap()).is_interface()));
}

// port: JsDocInfoParserTest#testDocumentationParameter
#[test]
fn test_documentation_parameter() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@param {Number} number42 This is a description.*/"),
        true,
        &[],
    );
    assert!((((jsdoc.clone()).unwrap()).has_description_for_parameter(JsString::from("number42"))));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description_for_parameter(JsString::from("number42")),
        Some(JsString::from("This is a description."))
    );
}

// port: JsDocInfoParserTest#testMultilineDocumentationParameter
#[test]
fn test_multiline_documentation_parameter() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        (JsString::from("@param {Number} number42 This is a description"))
            .concat(&(JsString::from("\n* on multiple \n* lines.*/"))),
        true,
        &[],
    );
    assert!((((jsdoc.clone()).unwrap()).has_description_for_parameter(JsString::from("number42"))));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description_for_parameter(JsString::from("number42")),
        Some(JsString::from("This is a description on multiple lines."))
    );
}

// port: JsDocInfoParserTest#testDocumentationMultipleParameter
#[test]
fn test_documentation_multiple_parameter() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@param {Number} number42 This is a description.\n* @param {Integer} number87 This is another description.*/\n"), true, &[]);
    assert!((((jsdoc.clone()).unwrap()).has_description_for_parameter(JsString::from("number42"))));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description_for_parameter(JsString::from("number42")),
        Some(JsString::from("This is a description."))
    );
    assert!((((jsdoc.clone()).unwrap()).has_description_for_parameter(JsString::from("number87"))));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description_for_parameter(JsString::from("number87")),
        Some(JsString::from("This is another description."))
    );
}

// port: JsDocInfoParserTest#testDocumentationMultipleParameter2
#[test]
fn test_documentation_multiple_parameter2() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@param {number} delta = 0 results in a redraw\n  != 0 ..... */\n"),
        true,
        &[],
    );
    assert!((((jsdoc.clone()).unwrap()).has_description_for_parameter(JsString::from("delta"))));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description_for_parameter(JsString::from("delta")),
        Some(JsString::from("= 0 results in a redraw != 0 ....."))
    );
}

// port: JsDocInfoParserTest#testAuthors
#[test]
fn test_authors() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@param {Number} number42 This is a description.\n* @param {Integer} number87 This is another description.\n* @author a@google.com (A Person)\n* @author b@google.com (B Person)\n* @author c@google.com (C Person)*/\n"), true, &[]);
    let mut authors = ((jsdoc.clone()).unwrap()).get_authors();
    assert!(authors.clone().is_some());
    assert_eq!(((authors.clone()).unwrap()).len(), 3usize);
    assert!(((authors.clone()).unwrap()).contains(&(JsString::from("a@google.com (A Person)"))));
    assert!(((authors.clone()).unwrap()).contains(&(JsString::from("b@google.com (B Person)"))));
    assert!(((authors.clone()).unwrap()).contains(&(JsString::from("c@google.com (C Person)"))));
}

// port: JsDocInfoParserTest#testSuppress1
#[test]
fn test_suppress1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@suppress {x} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [JsString::from("x")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testSuppressFollowedByAnnotationOnSameLine_parseTypesOnly
#[test]
fn test_suppress_followed_by_annotation_on_same_line_parse_types_only() {
    let mut f = set_up();
    let mut same_line = f.parse_with_mode(
        JsString::from("@suppress {visibility} @return {boolean} */"),
        JsDocParsing::TYPES_ONLY,
        &[],
    );
    assert!(((same_line.clone()).unwrap()).get_return_type().is_some());
    assert!(
        (((same_line.clone()).unwrap()).get_suppressions())
            .contains(&(JsString::from("visibility")))
    );
}

// port: JsDocInfoParserTest#testSuppressFollowedByAnnotationOnSameLine_parseWithDescription
#[test]
fn test_suppress_followed_by_annotation_on_same_line_parse_with_description() {
    let mut f = set_up();
    let mut same_line = f.parse_with_mode(
        JsString::from("@suppress {visibility} @return {boolean} */"),
        JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
        &[],
    );
    assert!(((same_line.clone()).unwrap()).get_return_type().is_some());
    assert!(
        (((same_line.clone()).unwrap()).get_suppressions())
            .contains(&(JsString::from("visibility")))
    );
}

// port: JsDocInfoParserTest#testSuppressWithDescriptionFollowedByAnnotation_parseTypeOnly
#[test]
fn test_suppress_with_description_followed_by_annotation_parse_type_only() {
    let mut f = set_up();
    let mut same_line = f.parse_with_mode(
        JsString::from("@suppress {visibility} this is a description @return {boolean} */"),
        JsDocParsing::TYPES_ONLY,
        &[],
    );
    assert!(((same_line.clone()).unwrap()).get_return_type().is_none());
    assert!(
        (((same_line.clone()).unwrap()).get_suppressions())
            .contains(&(JsString::from("visibility")))
    );
}

// port: JsDocInfoParserTest#testSuppressWithDescriptionFollowedByAnnotation_includeDescriptions
#[test]
fn test_suppress_with_description_followed_by_annotation_include_descriptions() {
    let mut f = set_up();
    let mut same_line = f.parse_with_mode(
        JsString::from("@suppress {visibility} this is a description @return {boolean} */"),
        JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
        &[],
    );
    assert!(((same_line.clone()).unwrap()).get_return_type().is_none());
    assert!(
        (((same_line.clone()).unwrap()).get_suppressions())
            .contains(&(JsString::from("visibility")))
    );
}

// port: JsDocInfoParserTest#testSuppressWithDescription
#[test]
fn test_suppress_with_description() {
    let mut f = set_up();
    let mut js_doc_comment =
        JsString::from("@suppress {x,y} Some description.\n * @suppress {z}\n*/\n");
    let mut info = f.parse_with_documentation(js_doc_comment.clone(), true, &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [
            JsString::from("x"),
            JsString::from("y"),
            JsString::from("z")
        ]
        .into_iter()
        .collect::<IndexSet<JsString>>()
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("x"), JsString::from("y")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("Some description.")))
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("z")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("")))
    );
}

// port: JsDocInfoParserTest#testSuppressWithDescription_multipleLines
#[test]
fn test_suppress_with_description_multiple_lines() {
    let mut f = set_up();
    let mut js_doc_comment = JsString::from(
        "@suppress {x,y} Some description.\n * Spans across lines.\n * @suppress {z}\n*/\n",
    );
    let mut info = f.parse_with_documentation(js_doc_comment.clone(), true, &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [
            JsString::from("x"),
            JsString::from("y"),
            JsString::from("z")
        ]
        .into_iter()
        .collect::<IndexSet<JsString>>()
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("x"), JsString::from("y")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("Some description. Spans across lines.")))
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("z")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("")))
    );
}

// port: JsDocInfoParserTest#testSuppressWithDescription_multipleLines_withOtherAnnotations
#[test]
fn test_suppress_with_description_multiple_lines_with_other_annotations() {
    let mut f = set_up();
    let mut js_doc_comment = JsString::from(
        "@author XYZ\n * @suppress {x,y} Some description.\n * Spans across lines.\n * @override\n * @suppress {z}\n*/\n",
    );
    let mut info = f.parse_with_documentation(js_doc_comment.clone(), true, &[]);
    assert!(
        ((((info.clone()).unwrap()).get_authors()).unwrap()).contains(&(JsString::from("XYZ")))
    );
    assert!((((info.clone()).unwrap()).is_override()));
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [
            JsString::from("x"),
            JsString::from("y"),
            JsString::from("z")
        ]
        .into_iter()
        .collect::<IndexSet<JsString>>()
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("x"), JsString::from("y")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("Some description. Spans across lines.")))
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("z")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("")))
    );
}

// port: JsDocInfoParserTest#testSuppressWithDescription_repeatedWarnings1
#[test]
fn test_suppress_with_description_repeated_warnings1() {
    let mut f = set_up();
    let mut js_doc_comment = JsString::from(
        "@suppress {x} Some description.\n * @suppress {x} Another description.\n*/\n",
    );
    let mut info = f.parse_with_documentation(js_doc_comment.clone(), true, &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [JsString::from("x")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
    assert_contents(
        ((info.clone()).unwrap()).get_suppressions_and_their_description(),
        vec![(
            [JsString::from("x")]
                .into_iter()
                .collect::<IndexSet<JsString>>(),
            JsString::from("Some description."),
        )],
        false,
    );
}

// port: JsDocInfoParserTest#testSuppressWithDescription_repeatedWarnings2
#[test]
fn test_suppress_with_description_repeated_warnings2() {
    let mut f = set_up();
    let mut js_doc_comment = JsString::from(
        "@suppress {x} Some description.\n * @suppress {x, y} Another description.\n*/\n",
    );
    let mut info = f.parse_with_documentation(js_doc_comment.clone(), true, &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [JsString::from("x"), JsString::from("y")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
    assert_eq!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description()).len(),
        2usize
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("x")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("Some description.")))
    );
    assert!(
        (((info.clone()).unwrap()).get_suppressions_and_their_description())
            .iter()
            .any(|(k, v)| k
                == &([JsString::from("x"), JsString::from("y")]
                    .into_iter()
                    .collect::<IndexSet<JsString>>())
                && v == &(JsString::from("Another description.")))
    );
}

// port: JsDocInfoParserTest#testSuppress2
#[test]
fn test_suppress2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@suppress {x|y|x|z} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [
            JsString::from("x"),
            JsString::from("y"),
            JsString::from("z")
        ]
        .into_iter()
        .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testSuppress3
#[test]
fn test_suppress3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@suppress {x,y} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [JsString::from("x"), JsString::from("y")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testMultipleSuppressTags
#[test]
fn test_multiple_suppress_tags() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@suppress {x} \n * @suppress {y} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_suppressions(),
        [JsString::from("x"), JsString::from("y")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testBadSuppress1
#[test]
fn test_bad_suppress1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@suppress {} */"),
        &[JsString::from("malformed @suppress tag")],
    );
}

// port: JsDocInfoParserTest#testBadSuppress2
#[test]
fn test_bad_suppress2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@suppress {x|} */"),
        &[JsString::from("malformed @suppress tag")],
    );
}

// port: JsDocInfoParserTest#testBadSuppress3
#[test]
fn test_bad_suppress3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@suppress {|x} */"),
        &[JsString::from("malformed @suppress tag")],
    );
}

// port: JsDocInfoParserTest#testBadSuppress4
#[test]
fn test_bad_suppress4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@suppress {x|y */"),
        &[JsString::from("malformed @suppress tag")],
    );
}

// port: JsDocInfoParserTest#testBadSuppress7
#[test]
fn test_bad_suppress7() {
    let mut f = set_up();
    f.parse(
        JsString::from("@suppress {impossible} */"),
        &[JsString::from("unknown @suppress parameter: impossible")],
    );
}

// port: JsDocInfoParserTest#testBadSuppress8
#[test]
fn test_bad_suppress8() {
    let mut f = set_up();
    f.parse(
        JsString::from("@suppress */"),
        &[JsString::from("malformed @suppress tag")],
    );
}

// port: JsDocInfoParserTest#testMods1
#[test]
fn test_mods1() {
    let mut f = set_up();
    let mut info = f.parse_file_overview(JsString::from("@mods {ns.Foo} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_mods(),
        Some(JsString::from("ns.Foo"))
    );
}

// port: JsDocInfoParserTest#testMods2
#[test]
fn test_mods2() {
    let mut f = set_up();
    let mut info = f.parse_file_overview(JsString::from("@mods {google3.path.to.file} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_mods(),
        Some(JsString::from("google3.path.to.file"))
    );
}

// port: JsDocInfoParserTest#testJsDocAfterMods
#[test]
fn test_js_doc_after_mods() {
    let mut f = set_up();
    let mut info = f.parse_file_overview(
        JsString::from("@mods {ns.Foo} @modName {new_feature_flag} */"),
        &[],
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_mods(),
        Some(JsString::from("ns.Foo"))
    );
}

// port: JsDocInfoParserTest#testMultipleModsTags
#[test]
fn test_multiple_mods_tags() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@mods {ns.Foo} \n * @mods {ns.Bar} */"),
        &[JsString::from("extra @mods tag")],
    );
}

// port: JsDocInfoParserTest#testBadMods1
#[test]
fn test_bad_mods1() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@mods {} */"),
        &[JsString::from("malformed @mods tag")],
    );
}

// port: JsDocInfoParserTest#testBadMods2
#[test]
fn test_bad_mods2() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@mods { */"),
        &[JsString::from("malformed @mods tag")],
    );
}

// port: JsDocInfoParserTest#testBadMods3
#[test]
fn test_bad_mods3() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@mods } */"),
        &[JsString::from("malformed @mods tag")],
    );
}

// port: JsDocInfoParserTest#testBadMods4
#[test]
fn test_bad_mods4() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@mods ns.Foo */"),
        &[JsString::from("malformed @mods tag")],
    );
}

// port: JsDocInfoParserTest#testBadMods5
#[test]
fn test_bad_mods5() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@mods {ns.Foo */"),
        &[JsString::from("malformed @mods tag")],
    );
}

// port: JsDocInfoParserTest#testBadMods6
#[test]
fn test_bad_mods6() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@mods ns.Foo} */"),
        &[JsString::from("malformed @mods tag")],
    );
}

// port: JsDocInfoParserTest#testModifies1
#[test]
fn test_modifies1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@modifies {this} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_modifies(),
        [JsString::from("this")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testModifies2
#[test]
fn test_modifies2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@modifies {arguments} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_modifies(),
        [JsString::from("arguments")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testModifies3
#[test]
fn test_modifies3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@modifies {this|arguments} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_modifies(),
        [JsString::from("this"), JsString::from("arguments")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testModifies4
#[test]
fn test_modifies4() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {*} x\n * @modifies {x} */"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_modifies(),
        [JsString::from("x")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testModifies5
#[test]
fn test_modifies5() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {*} x\n * @param {*} y\n * @modifies {x} */\n"),
        &[],
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_modifies(),
        [JsString::from("x")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testModifies6
#[test]
fn test_modifies6() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {*} x\n * @param {*} y\n * @modifies {x|y} */\n"),
        &[],
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_modifies(),
        [JsString::from("x"), JsString::from("y")]
            .into_iter()
            .collect::<IndexSet<JsString>>()
    );
}

// port: JsDocInfoParserTest#testBadModifies1
#[test]
fn test_bad_modifies1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {} */"),
        &[JsString::from("malformed @modifies tag")],
    );
}

// port: JsDocInfoParserTest#testBadModifies2
#[test]
fn test_bad_modifies2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {this|} */"),
        &[JsString::from("malformed @modifies tag")],
    );
}

// port: JsDocInfoParserTest#testBadModifies3
#[test]
fn test_bad_modifies3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {|this} */"),
        &[JsString::from("malformed @modifies tag")],
    );
}

// port: JsDocInfoParserTest#testBadModifies4
#[test]
fn test_bad_modifies4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {this|arguments */"),
        &[JsString::from("malformed @modifies tag")],
    );
}

// port: JsDocInfoParserTest#testBadModifies5
#[test]
fn test_bad_modifies5() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {this,arguments} */"),
        &[JsString::from("malformed @modifies tag")],
    );
}

// port: JsDocInfoParserTest#testBadModifies6
#[test]
fn test_bad_modifies6() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {this} \n * @modifies {this} */"),
        &[JsString::from("conflicting @modifies tag")],
    );
}

// port: JsDocInfoParserTest#testBadModifies7
#[test]
fn test_bad_modifies7() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {impossible} */"),
        &[JsString::from("unknown @modifies parameter: impossible")],
    );
}

// port: JsDocInfoParserTest#testBadModifies8
#[test]
fn test_bad_modifies8() {
    let mut f = set_up();
    f.parse(
        JsString::from("@modifies {this}\n@nosideeffects */\n"),
        &[(JsString::from(
            "@nosideeffects functions cannot have @modifies (modifying arguments/this is a side",
        ))
        .concat(&(JsString::from(" effect)")))],
    );
}

// port: JsDocInfoParserTest#testBadModifies9
#[test]
fn test_bad_modifies9() {
    let mut f = set_up();
    f.parse(
        JsString::from("@nosideeffects @modifies {this} */"),
        &[(JsString::from(
            "@nosideeffects functions cannot have @modifies (modifying arguments/this is a side",
        ))
        .concat(&(JsString::from(" effect)")))],
    );
}

// port: JsDocInfoParserTest#testConflictingThrowsAndNoSideEffects1
#[test]
fn test_conflicting_throws_and_no_side_effects1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@throws {Error}\n@nosideeffects */\n"),
        &[JsString::from(
            "@nosideeffects functions cannot have @throws (throwing is a side effect)",
        )],
    );
}

// port: JsDocInfoParserTest#testConflictingThrowsAndNoSideEffects2
#[test]
fn test_conflicting_throws_and_no_side_effects2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@nosideeffects\n@throws {Error} */\n"),
        &[JsString::from(
            "@nosideeffects functions cannot have @throws (throwing is a side effect)",
        )],
    );
}

// port: JsDocInfoParserTest#testDuplicateNoSideEffects
#[test]
fn test_duplicate_no_side_effects() {
    let mut f = set_up();
    f.parse(
        JsString::from("@nosideeffects\n@nosideeffects */\n"),
        &[JsString::from("conflicting @nosideeffects tag")],
    );
}

// port: JsDocInfoParserTest#testThrowsAndModifiesCompatible
#[test]
fn test_throws_and_modifies_compatible() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(
        JsString::from("@modifies {this}\n@throws {Error} Some error */\n"),
        true,
        &[],
    );
    assert!((((info.clone()).unwrap()).modifies_this()));
    assert_contents(
        ((info.clone()).unwrap()).get_throws_annotations(),
        vec![JsString::from("{Error} Some error")],
        false,
    );
    info = f.parse_with_documentation(
        JsString::from("@throws {Error} Some error\n@modifies {this} */\n"),
        true,
        &[],
    );
    assert!((((info.clone()).unwrap()).modifies_this()));
    assert_contents(
        ((info.clone()).unwrap()).get_throws_annotations(),
        vec![JsString::from("{Error} Some error")],
        false,
    );
}

// port: JsDocInfoParserTest#testFileOverviewSingleLine
#[test]
fn test_file_overview_single_line() {
    let mut f = set_up();
    let mut jsdoc = f.parse_file_overview(JsString::from("@fileoverview Hi mom! */"), &[]);
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_file_overview(),
        Some(JsString::from("Hi mom!"))
    );
}

// port: JsDocInfoParserTest#testFileOverviewMultiLine
#[test]
fn test_file_overview_multi_line() {
    let mut f = set_up();
    let mut jsdoc =
        f.parse_file_overview(JsString::from("@fileoverview Pie is \n * good! */"), &[]);
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_file_overview(),
        Some(JsString::from("Pie is\ngood!"))
    );
}

// port: JsDocInfoParserTest#testFileOverviewDuplicate
#[test]
fn test_file_overview_duplicate() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@fileoverview Pie \n * @fileoverview Cake */"),
        &[JsString::from("extra @fileoverview tag")],
    );
}

// port: JsDocInfoParserTest#testPublicVisibilityAllowedInFileOverview
#[test]
fn test_public_visibility_allowed_in_file_overview() {
    let mut f = set_up();
    f.parse_file_overview(JsString::from("@fileoverview \n * @public */"), &[]);
}

// port: JsDocInfoParserTest#testPackageVisibilityAllowedInFileOverview
#[test]
fn test_package_visibility_allowed_in_file_overview() {
    let mut f = set_up();
    f.parse_file_overview(JsString::from("@fileoverview \n * @package */"), &[]);
}

// port: JsDocInfoParserTest#testImplicitVisibilityAllowedInFileOverview
#[test]
fn test_implicit_visibility_allowed_in_file_overview() {
    let mut f = set_up();
    f.parse_file_overview(JsString::from("@fileoverview */"), &[]);
}

// port: JsDocInfoParserTest#testProtectedVisibilityNotAllowedInFileOverview
#[test]
fn test_protected_visibility_not_allowed_in_file_overview() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@fileoverview \n * @protected */"),
        &[JsString::from(
            "protected visibility not allowed in @fileoverview block",
        )],
    );
    f.parse_file_overview(
        JsString::from("@fileoverview @protected */"),
        &[JsString::from(
            "protected visibility not allowed in @fileoverview block",
        )],
    );
}

// port: JsDocInfoParserTest#testPrivateVisibilityNotAllowedInFileOverview
#[test]
fn test_private_visibility_not_allowed_in_file_overview() {
    let mut f = set_up();
    f.parse_file_overview(
        JsString::from("@fileoverview \n @private */"),
        &[JsString::from(
            "private visibility not allowed in @fileoverview block",
        )],
    );
    f.parse_file_overview(
        JsString::from("@fileoverview @private */"),
        &[JsString::from(
            "private visibility not allowed in @fileoverview block",
        )],
    );
}

// port: JsDocInfoParserTest#testReferences
#[test]
fn test_references() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@see A cool place!\n* @see The world.\n* @see SomeClass#SomeMember\n* @see A boring test case*/\n"), true, &[]);
    let mut references = ((jsdoc.clone()).unwrap()).get_references();
    assert!(references.clone().is_some());
    assert_eq!(((references.clone()).unwrap()).len(), 4usize);
    assert!(((references.clone()).unwrap()).contains(&(JsString::from("A cool place!"))));
    assert!(((references.clone()).unwrap()).contains(&(JsString::from("The world."))));
    assert!(((references.clone()).unwrap()).contains(&(JsString::from("SomeClass#SomeMember"))));
    assert!(((references.clone()).unwrap()).contains(&(JsString::from("A boring test case"))));
}

// port: JsDocInfoParserTest#testSingleTags
#[test]
fn test_single_tags() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@deprecated In favor of the new one!\n* @return {SomeType} The most important object :-)*/\n"), true, &[]);
    assert!((((jsdoc.clone()).unwrap()).is_deprecated()));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_deprecation_reason(),
        Some(JsString::from("In favor of the new one!"))
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_return_description(),
        Some(JsString::from("The most important object :-)"))
    );
}

// port: JsDocInfoParserTest#testSingleTags2
#[test]
fn test_single_tags2() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@param {SomeType} a The most important object :-)*/"),
        true,
        &[],
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description_for_parameter(JsString::from("a")),
        Some(JsString::from("The most important object :-)"))
    );
}

// port: JsDocInfoParserTest#testSingleTagsReordered
#[test]
fn test_single_tags_reordered() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@deprecated In favor of the new one!\n * @return {SomeType} The most important object :-)*/\n"), true, &[]);
    assert!((((jsdoc.clone()).unwrap()).is_deprecated()));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_deprecation_reason(),
        Some(JsString::from("In favor of the new one!"))
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_return_description(),
        Some(JsString::from("The most important object :-)"))
    );
}

// port: JsDocInfoParserTest#testAuthorMissing
#[test]
fn test_author_missing() {
    let mut f = set_up();
    f.parse_with_documentation(
        JsString::from("* @author */"),
        true,
        &[JsString::from("@author tag missing author")],
    );
}

// port: JsDocInfoParserTest#testSeeMissing
#[test]
fn test_see_missing() {
    let mut f = set_up();
    f.parse_with_documentation(
        JsString::from("* @see */"),
        true,
        &[JsString::from("@see tag missing description")],
    );
}

// port: JsDocInfoParserTest#testParseBlockComment
#[test]
fn test_parse_block_comment() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from(
            "this is a nice comment\n * that is multiline\n * @author abc@google.com */\n",
        ),
        true,
        &[],
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_block_description(),
        Some(JsString::from("this is a nice comment\nthat is multiline"))
    );
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("author"), 2, 3),
        JsString::from("abc@google.com"),
        10,
        2,
        24,
    );
}

// port: JsDocInfoParserTest#testParseBlockComment2
#[test]
fn test_parse_block_comment2() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from(
            "this is a nice comment\n * that is *** multiline\n * @author abc@google.com */\n",
        ),
        true,
        &[],
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_block_description(),
        Some(JsString::from(
            "this is a nice comment\nthat is *** multiline"
        ))
    );
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("author"), 2, 3),
        JsString::from("abc@google.com"),
        10,
        2,
        24,
    );
}

// port: JsDocInfoParserTest#testParseBlockComment3
#[test]
fn test_parse_block_comment3() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from(" \n* hello world \n* @author abc@google.com */\n"),
        true,
        &[],
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_block_description(),
        Some(JsString::from("hello world"))
    );
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("author"), 2, 2),
        JsString::from("abc@google.com"),
        9,
        2,
        23,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkers1
#[test]
fn test_parse_with_markers1() {
    let mut f = set_up();
    let mut jsdoc =
        f.parse_with_documentation(JsString::from("@author abc@google.com */"), true, &[]);
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("author"), 0, 0),
        JsString::from("abc@google.com"),
        7,
        0,
        21,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkers2
#[test]
fn test_parse_with_markers2() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@param {Foo} somename abc@google.com */"),
        true,
        &[],
    );
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("param"), 0, 0),
        JsString::from("abc@google.com"),
        21,
        0,
        37,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkers3
#[test]
fn test_parse_with_markers3() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@return {Foo} some long\n * multiline\n * description */\n"),
        true,
        &[],
    );
    let mut return_doc = assert_annotation_marker(jsdoc.clone(), JsString::from("return"), 0, 0);
    assert_documentation_in_marker(
        return_doc.clone(),
        JsString::from("some long multiline description"),
        14,
        2,
        15,
    );
    assert_eq!(
        (((return_doc.clone()).get_type().cloned()).unwrap()).get_position_on_start_line(),
        8
    );
    assert_eq!(
        (((return_doc.clone()).get_type().cloned()).unwrap()).get_position_on_end_line(),
        12
    );
}

// port: JsDocInfoParserTest#testParseWithMarkers4
#[test]
fn test_parse_with_markers4() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@author foobar\n * @param {Foo} somename abc@google.com */\n"),
        true,
        &[],
    );
    assert_annotation_marker(jsdoc.clone(), JsString::from("author"), 0, 0);
    assert_annotation_marker(jsdoc.clone(), JsString::from("param"), 1, 3);
}

// port: JsDocInfoParserTest#testParseWithMarkers5
#[test]
fn test_parse_with_markers5() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@return some long\n * multiline\n * description */\n"),
        true,
        &[JsString::from(MISSING_TYPE_DECL_WARNING_TEXT)],
    );
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("return"), 0, 0),
        JsString::from("some long multiline description"),
        8,
        2,
        15,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkers6
#[test]
fn test_parse_with_markers6() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@param x some long\n * multiline\n * description */\n"),
        true,
        &[JsString::from(MISSING_TYPE_DECL_WARNING_TEXT)],
    );
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("param"), 0, 0),
        JsString::from("some long multiline description"),
        8,
        2,
        15,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkerNames1
#[test]
fn test_parse_with_marker_names1() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@param {SomeType} name somedescription */"),
        true,
        &[],
    );
    assert_name_in_marker(
        &f.ast,
        assert_annotation_marker(jsdoc.clone(), JsString::from("param"), 0, 0),
        JsString::from("name"),
        0,
        18,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkerNames2
#[test]
fn test_parse_with_marker_names2() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from(
            "@param {SomeType} name somedescription\n* @param {AnotherType} anothername des */\n",
        ),
        true,
        &[],
    );
    assert_type_in_marker(
        &f.ast,
        assert_name_in_marker(
            &f.ast,
            assert_annotation_marker_at(jsdoc.clone(), JsString::from("param"), 0, 0, 0),
            JsString::from("name"),
            0,
            18,
        ),
        JsString::from("SomeType"),
        0,
        7,
        0,
        16,
        true,
    );
    assert_type_in_marker(
        &f.ast,
        assert_name_in_marker(
            &f.ast,
            assert_annotation_marker_at(jsdoc.clone(), JsString::from("param"), 1, 2, 1),
            JsString::from("anothername"),
            1,
            23,
        ),
        JsString::from("AnotherType"),
        1,
        9,
        1,
        21,
        true,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkerNames3
#[test]
fn test_parse_with_marker_names3() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@param {Some.Long.Type.\n *  Name} name somedescription */"),
        true,
        &[],
    );
    assert_type_in_marker(
        &f.ast,
        assert_name_in_marker(
            &f.ast,
            assert_annotation_marker_at(jsdoc.clone(), JsString::from("param"), 0, 0, 0),
            JsString::from("name"),
            1,
            10,
        ),
        JsString::from("Some.Long.Type.Name"),
        0,
        7,
        1,
        8,
        true,
    );
}

// port: JsDocInfoParserTest#testParseWithoutMarkerName
#[test]
fn test_parse_without_marker_name() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@author helloworld*/"), true, &[]);
    assert!(
        (assert_annotation_marker(jsdoc.clone(), JsString::from("author"), 0, 0))
            .get_name_node()
            .cloned()
            .is_none()
    );
}

// port: JsDocInfoParserTest#testParseWithMarkerType
#[test]
fn test_parse_with_marker_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@extends {FooBar}*/"), true, &[]);
    assert_type_in_marker(
        &f.ast,
        assert_annotation_marker(jsdoc.clone(), JsString::from("extends"), 0, 0),
        JsString::from("FooBar"),
        0,
        9,
        0,
        16,
        true,
    );
}

// port: JsDocInfoParserTest#testParseWithMarkerType2
#[test]
fn test_parse_with_marker_type2() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(JsString::from("@extends FooBar*/"), true, &[]);
    assert_type_in_marker(
        &f.ast,
        assert_annotation_marker(jsdoc.clone(), JsString::from("extends"), 0, 0),
        JsString::from("FooBar"),
        0,
        9,
        0,
        15,
        false,
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict1
#[test]
fn test_type_tag_conflict1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@constructor \n * @constructor */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict2
#[test]
fn test_type_tag_conflict2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@interface \n * @interface */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict3
#[test]
fn test_type_tag_conflict3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@constructor \n * @interface */"),
        &[
            (JsString::from("Bad type annotation. cannot be both an interface and a constructor."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict4
#[test]
fn test_type_tag_conflict4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@interface \n * @constructor */"),
        &[
            (JsString::from("Bad type annotation. cannot be both an interface and a constructor."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict5
#[test]
fn test_type_tag_conflict5() {
    let mut f = set_up();
    f.parse(
        JsString::from("@interface \n * @type {string} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict6
#[test]
fn test_type_tag_conflict6() {
    let mut f = set_up();
    f.parse(
        JsString::from("@typedef {string} \n * @type {string} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict7
#[test]
fn test_type_tag_conflict7() {
    let mut f = set_up();
    f.parse(
        JsString::from("@typedef {string} \n * @constructor */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict8
#[test]
fn test_type_tag_conflict8() {
    let mut f = set_up();
    f.parse(
        JsString::from("@typedef {string} \n * @return {boolean} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict9
#[test]
fn test_type_tag_conflict9() {
    let mut f = set_up();
    f.parse(
        JsString::from("@enum {string} \n * @return {boolean} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict10
#[test]
fn test_type_tag_conflict10() {
    let mut f = set_up();
    f.parse(
        JsString::from("@this {Object} \n * @enum {boolean} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict11
#[test]
fn test_type_tag_conflict11() {
    let mut f = set_up();
    f.parse(
        JsString::from("@param {Object} x \n * @type {boolean} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict12
#[test]
fn test_type_tag_conflict12() {
    let mut f = set_up();
    f.parse(
        JsString::from("@typedef {boolean} \n * @param {Object} x */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict13
#[test]
fn test_type_tag_conflict13() {
    let mut f = set_up();
    f.parse(
        JsString::from("@typedef {boolean} \n * @extends {Object} */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict14
#[test]
fn test_type_tag_conflict14() {
    let mut f = set_up();
    f.parse(
        JsString::from("@return {?} x \n * @return {?} y */"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict15
#[test]
fn test_type_tag_conflict15() {
    let mut f = set_up();
    f.parse(
        JsString::from("/**\n * @struct\n * @struct\n */\nfunction StrStr() {}\n"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict16
#[test]
fn test_type_tag_conflict16() {
    let mut f = set_up();
    f.parse(
        JsString::from("/**\n * @struct\n * @interface\n */\nfunction StrIntf() {}\n"),
        &[],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict17
#[test]
fn test_type_tag_conflict17() {
    let mut f = set_up();
    f.parse(
        JsString::from("/**\n * @interface\n * @struct\n */\nfunction StrIntf() {}\n"),
        &[],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict18
#[test]
fn test_type_tag_conflict18() {
    let mut f = set_up();
    f.parse(
        JsString::from("/**\n * @dict\n * @dict\n */\nfunction DictDict() {}\n"),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict19
#[test]
fn test_type_tag_conflict19() {
    let mut f = set_up();
    f.parse(
        JsString::from("/**\n * @dict\n * @interface\n */\nfunction DictDict() {}\n"),
        &[],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict20
#[test]
fn test_type_tag_conflict20() {
    let mut f = set_up();
    f.parse(
        JsString::from("/**\n * @interface\n * @dict\n */\nfunction DictDict() {}\n"),
        &[],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict21
#[test]
fn test_type_tag_conflict21() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "/**\n * @private {string}\n * @type {number}\n */\nfunction DictDict() {}\n",
        ),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict22
#[test]
fn test_type_tag_conflict22() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "/**\n * @protected {string}\n * @param {string} x\n */\nfunction DictDict(x) {}\n",
        ),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict23
#[test]
fn test_type_tag_conflict23() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "/**\n * @public {string}\n * @return {string} x\n */\nfunction DictDict() {}\n",
        ),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict24
#[test]
fn test_type_tag_conflict24() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "/**\n * @const {string}\n * @return {string} x\n */\nfunction DictDict() {}\n",
        ),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTypeTagConflict25
#[test]
fn test_type_tag_conflict25() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "/**\n * @package {string}\n * @return {string} x\n */\nfunction DictDict() {}\n",
        ),
        &[(JsString::from(
            "Bad type annotation. type annotation incompatible with other annotations.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testMixedVisibility
#[test]
fn test_mixed_visibility() {
    let mut f = set_up();
    f.parse(
        JsString::from("@public @private */"),
        &[JsString::from("extra visibility tag")],
    );
    f.parse(
        JsString::from("@public @protected */"),
        &[JsString::from("extra visibility tag")],
    );
    f.parse(JsString::from("@export @protected */"), &[]);
    f.parse(JsString::from("@export {string}\n * @private */"), &[]);
    f.parse(JsString::from("@export {string}\n * @public */"), &[]);
}

// port: JsDocInfoParserTest#testStableIdGenerator
#[test]
fn test_stable_id_generator() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("/**\n * @idGenerator {stable}\n */\nfunction getId() {}\n"),
        &[],
    );
    assert!((((info.clone()).unwrap()).is_stable_id_generator()));
}

// port: JsDocInfoParserTest#testStableIdGeneratorConflict
#[test]
fn test_stable_id_generator_conflict() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "/**\n * @idGenerator {stable}\n * @idGenerator {stable}\n */\nfunction getId() {}\n",
        ),
        &[JsString::from("extra @idGenerator tag")],
    );
}

// port: JsDocInfoParserTest#testXidGenerator
#[test]
fn test_xid_generator() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("/**\n * @idGenerator {xid}\n */\nfunction getId() {}\n"),
        &[],
    );
    assert!((((info.clone()).unwrap()).is_xid_generator()));
}

// port: JsDocInfoParserTest#testXidGeneratorConflict
#[test]
fn test_xid_generator_conflict() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "/**\n * @idGenerator {xid}\n * @idGenerator {xid}\n */\nfunction getId() {}\n",
        ),
        &[JsString::from("extra @idGenerator tag")],
    );
}

// port: JsDocInfoParserTest#testIdGenerator
#[test]
fn test_id_generator() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("/**\n * @idGenerator\n */\nfunction getId() {}\n"),
        &[],
    );
    assert!((((info.clone()).unwrap()).is_id_generator()));
}

// port: JsDocInfoParserTest#testIdGeneratorConflict
#[test]
fn test_id_generator_conflict() {
    let mut f = set_up();
    f.parse(
        JsString::from("/**\n * @idGenerator\n * @idGenerator\n */\nfunction getId() {}\n"),
        &[JsString::from("extra @idGenerator tag")],
    );
}

// port: JsDocInfoParserTest#testIdGenerator1
#[test]
fn test_id_generator1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@idGenerator {unique} */"), &[]);
    assert!((((info.clone()).unwrap()).is_id_generator()));
}

// port: JsDocInfoParserTest#testIdGenerator2
#[test]
fn test_id_generator2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@idGenerator {consistent} */"), &[]);
    assert!((((info.clone()).unwrap()).is_consistent_id_generator()));
}

// port: JsDocInfoParserTest#testIdGenerator3
#[test]
fn test_id_generator3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@idGenerator {stable} */"), &[]);
    assert!((((info.clone()).unwrap()).is_stable_id_generator()));
}

// port: JsDocInfoParserTest#testIdGenerator4
#[test]
fn test_id_generator4() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@idGenerator {xid} */"), &[]);
    assert!((((info.clone()).unwrap()).is_xid_generator()));
}

// port: JsDocInfoParserTest#testIdGenerator5
#[test]
fn test_id_generator5() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@idGenerator {mapped} */"), &[]);
    assert!((((info.clone()).unwrap()).is_mapped_id_generator()));
}

// port: JsDocInfoParserTest#testBadIdGenerator1
#[test]
fn test_bad_id_generator1() {
    let mut f = set_up();
    f.parse(
        JsString::from("@idGenerator {} */"),
        &[JsString::from("malformed @idGenerator tag")],
    );
}

// port: JsDocInfoParserTest#testBadIdGenerator2
#[test]
fn test_bad_id_generator2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@idGenerator {impossible} */"),
        &[JsString::from("unknown @idGenerator parameter: impossible")],
    );
}

// port: JsDocInfoParserTest#testBadIdGenerator3
#[test]
fn test_bad_id_generator3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@idGenerator {unique */"),
        &[JsString::from("malformed @idGenerator tag")],
    );
}

// port: JsDocInfoParserTest#testParserWithTemplateTypeNameMissing
#[test]
fn test_parser_with_template_type_name_missing() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template */"),
        &[
            JsString::from("name not recognized due to syntax error."),
            (JsString::from("Bad type annotation. @template tag missing type name."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            JsString::from("Unexpected end of file"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTwoTemplates
#[test]
fn test_parser_with_two_templates() {
    let mut f = set_up();
    f.parse(JsString::from("@template T,V */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithInvalidTemplateType
#[test]
fn test_parser_with_invalid_template_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template {T} */"),
        &[
            JsString::from("Bounded generic semantics are currently still in development"),
            JsString::from("name not recognized due to syntax error."),
            (JsString::from("Bad type annotation. @template tag missing type name."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            JsString::from("Unexpected end of file"),
        ],
    );
}

// port: JsDocInfoParserTest#testBoundedGenericWithoutName
#[test]
fn test_bounded_generic_without_name() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template {number} */"),
        &[
            JsString::from("Bounded generic semantics are currently still in development"),
            JsString::from("name not recognized due to syntax error."),
            (JsString::from("Bad type annotation. @template tag missing type name."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            JsString::from("Unexpected end of file"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithValidTemplateType
#[test]
fn test_parser_with_valid_template_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template {string} T */"),
        &[JsString::from(
            "Bounded generic semantics are currently still in development",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithValidAndInvalidTemplateType
#[test]
fn test_parser_with_valid_and_invalid_template_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template S, {T} */"),
        &[JsString::from("name not recognized due to syntax error.")],
    );
}

// port: JsDocInfoParserTest#testParserWithTemplateDuplicated
#[test]
fn test_parser_with_template_duplicated() {
    let mut f = set_up();
    f.parse(JsString::from("@template T\n@template V */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTemplateDuplicated2
#[test]
fn test_parser_with_template_duplicated2() {
    let mut f = set_up();
    f.parse(JsString::from("@template T,R\n@template V,U */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTemplateDuplicated3
#[test]
fn test_parser_with_template_duplicated3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T\n@param {string} x\n@template V */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTemplateTypeNameDeclaredTwice
#[test]
fn test_parser_with_template_type_name_declared_twice() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T\n@template T */"),
        &[(JsString::from(
            "Bad type annotation. Type name(s) for @template annotation declared twice.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParserWithDoubleTemplateDeclaration2
#[test]
fn test_parser_with_double_template_declaration2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T,T */"),
        &[(JsString::from(
            "Bad type annotation. Type name(s) for @template annotation declared twice.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParserWithTemplateDuplicatedTypeNameMissing
#[test]
fn test_parser_with_template_duplicated_type_name_missing() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T,R\n@template */"),
        &[
            JsString::from("name not recognized due to syntax error."),
            (JsString::from("Bad type annotation. @template tag missing type name."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            JsString::from("Unexpected end of file"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithLowercaseTemplateTypeName
#[test]
fn test_parser_with_lowercase_template_type_name() {
    let mut f = set_up();
    f.parse(JsString::from("@template t */"), &[]);
    f.parse(JsString::from("@template s,t */"), &[]);
    f.parse(JsString::from("@template key,value */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTemplateTypeNameWithDollar
#[test]
fn test_parser_with_template_type_name_with_dollar() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@template A,B$,C */"), &[]);
    assert_contents(
        ((info.clone()).unwrap()).get_template_type_names(),
        vec![
            JsString::from("A"),
            JsString::from("B$"),
            JsString::from("C"),
        ],
        false,
    );
}

// port: JsDocInfoParserTest#testParserWithMissingTypeTransformationExpression
#[test]
fn test_parser_with_missing_type_transformation_expression() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := */"),
        &[
            (JsString::from(
                "Bad type annotation. Expected end delimiter for a type transformation.",
            ))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            (JsString::from("Bad type annotation. Missing type transformation expression."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithMissingTypeTransformationExpression2
#[test]
fn test_parser_with_missing_type_transformation_expression2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := =:*/"),
        &[
            (JsString::from("Bad type annotation. Missing type transformation expression."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testBug16129690
#[test]
fn test_bug16129690() {
    let mut f = set_up();
    f.parse(
        JsString::from("@param {T} x\n@template T\nrandom documentation text*/\n"),
        &[],
    );
}

// port: JsDocInfoParserTest#testAllowlistedNewAnnotations
#[test]
fn test_allow_listed_new_annotations() {
    let mut f = set_up();
    f.parse(JsString::from("@foobar */"), &[(JsString::from("illegal use of unknown JSDoc tag \"foobar\"; ignoring it. Place another character before")).concat(&(JsString::from(" the @ to stop JSCompiler from parsing it as an annotation.")))]);
    f.extra_annotations.insert(JsString::from("foobar"));
    f.parse(JsString::from("@foobar */"), &[]);
}

// port: JsDocInfoParserTest#testEscapingIllegalAnnotations
#[test]
fn test_escaping_illegal_annotations() {
    let mut f = set_up();
    f.parse(JsString::from("_@foobar */"), &[]);
    f.parse(JsString::from("\\@foobar */"), &[]);
}

// port: JsDocInfoParserTest#testNonIdentifierAnnotation
#[test]
fn test_non_identifier_annotation() {
    let mut f = set_up();
    f.extra_annotations.insert(JsString::from("123"));
    f.parse(JsString::from("@123 */"), &[(JsString::from("illegal use of unknown JSDoc tag \"\"; ignoring it. Place another character before the @")).concat(&(JsString::from(" to stop JSCompiler from parsing it as an annotation.")))]);
}

// port: JsDocInfoParserTest#testUnsupportedJsDocSyntax2
#[test]
fn test_unsupported_js_doc_syntax2() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(JsString::from("@param {?} userInfo The user info.\n * @param {?} userInfo.name The name of the user */\n"), true, &[JsString::from("invalid param name \"userInfo.name\"")]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    assert_eq!(
        ((info.clone()).unwrap()).get_description_for_parameter(JsString::from("userInfo")),
        Some(JsString::from("The user info."))
    );
}

// port: JsDocInfoParserTest#testAllowlistedAnnotations
#[test]
fn test_allow_listed_annotations() {
    let mut f = set_up();
    f.parse(JsString::from("* @addon \n* @augments \n* @base \n* @borrows \n* @bug \n* @channel \n* @class \n* @config \n* @constructs \n* @default \n* @description \n* @enhance \n* @enhanceable \n* @event \n* @example \n* @exception \n* @exec \n* @externs \n* @field \n* @function \n* @delcall \n* @deltemplate \n* @hassoydelcall \n* @hassoydeltemplate \n* @id \n* @ignore \n* @jsx \n* @jsxFrag \n* @inner \n* @lends {string} \n* @link \n* @mayhaveextraedge \n* @member \n* @memberOf \n* @modName \n* @name \n* @namespace \n* @ngInject \n* @nocompile \n* @pintomodule \n* @property \n* @requirecss \n* @requires \n* @since \n* @soyTemplate \n* @soyModule \n* @static \n* @supported\n* @wizaction \n* @wizcallback \n*/\n"), &[]);
}

// port: JsDocInfoParserTest#testGetOriginalCommentString
#[test]
fn test_get_original_comment_string() {
    let mut f = set_up();
    let mut comment = JsString::from("* @desc This is a comment */");
    let mut info = f.parse(comment.clone(), &[]);
    assert!(
        ((info.clone()).unwrap())
            .get_original_comment_string()
            .is_none()
    );
    info = f.parse_with_documentation(comment.clone(), true, &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_original_comment_string(),
        Some(comment.clone())
    );
}

// port: JsDocInfoParserTest#testGetMultilineDesc
#[test]
fn test_get_multiline_desc() {
    let mut f = set_up();
    let mut comment = JsString::from(" * @desc description here\n         continued here\n */\n");
    let mut info = f.parse(comment.clone(), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_description(),
        Some(JsString::from("description here continued here"))
    );
}

// port: JsDocInfoParserTest#testParseNgInject1
#[test]
fn test_parse_ng_inject1() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@ngInject*/"), &[])).unwrap()).is_ng_inject()));
}

// port: JsDocInfoParserTest#testParseNgInject2
#[test]
fn test_parse_ng_inject2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@ngInject \n@ngInject*/"),
        &[JsString::from("extra @ngInject tag")],
    );
}

// port: JsDocInfoParserTest#testParsePolymerBehavior
#[test]
fn test_parse_polymer_behavior() {
    let mut f = set_up();
    assert!(
        (((f.parse(JsString::from("@polymerBehavior*/"), &[])).unwrap()).is_polymer_behavior())
    );
}

// port: JsDocInfoParserTest#testParsePolymerBehaviorExtra
#[test]
fn test_parse_polymer_behavior_extra() {
    let mut f = set_up();
    f.parse(
        JsString::from("@polymerBehavior \n@polymerBehavior*/"),
        &[JsString::from("extra @polymerBehavior tag")],
    );
}

// port: JsDocInfoParserTest#testParsePolymer
#[test]
fn test_parse_polymer() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@polymer*/"), &[])).unwrap()).is_polymer()));
}

// port: JsDocInfoParserTest#testParsePolymerExtra
#[test]
fn test_parse_polymer_extra() {
    let mut f = set_up();
    f.parse(
        JsString::from("@polymer \n@polymer*/"),
        &[JsString::from("extra @polymer tag")],
    );
}

// port: JsDocInfoParserTest#testParseCustomElement
#[test]
fn test_parse_custom_element() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@customElement*/"), &[])).unwrap()).is_custom_element()));
}

// port: JsDocInfoParserTest#testParseCustomElementExtra
#[test]
fn test_parse_custom_element_extra() {
    let mut f = set_up();
    f.parse(
        JsString::from("@customElement \n@customElement*/"),
        &[JsString::from("extra @customElement tag")],
    );
}

// port: JsDocInfoParserTest#testParseMixinClass
#[test]
fn test_parse_mixin_class() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@mixinClass*/"), &[])).unwrap()).is_mixin_class()));
}

// port: JsDocInfoParserTest#testParseMixinClassExtra
#[test]
fn test_parse_mixin_class_extra() {
    let mut f = set_up();
    f.parse(
        JsString::from("@mixinClass \n@mixinClass*/"),
        &[JsString::from("extra @mixinClass tag")],
    );
}

// port: JsDocInfoParserTest#testParseMixinFunction
#[test]
fn test_parse_mixin_function() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@mixinFunction*/"), &[])).unwrap()).is_mixin_function()));
}

// port: JsDocInfoParserTest#testParseMixinFunctionExtra
#[test]
fn test_parse_mixin_function_extra() {
    let mut f = set_up();
    f.parse(
        JsString::from("@mixinFunction \n@mixinFunction*/"),
        &[JsString::from("extra @mixinFunction tag")],
    );
}

// port: JsDocInfoParserTest#testParseSassGeneratedCssTs
#[test]
fn test_parse_sass_generated_css_ts() {
    let mut f = set_up();
    assert!(
        (((f.parse(JsString::from("@sassGeneratedCssTs*/"), &[])).unwrap())
            .is_sass_generated_css_ts())
    );
}

// port: JsDocInfoParserTest#testParseSassGeneratedCssTsExtra
#[test]
fn test_parse_sass_generated_css_ts_extra() {
    let mut f = set_up();
    f.parse(
        JsString::from("@sassGeneratedCssTs \n@sassGeneratedCssTs*/"),
        &[JsString::from("extra @sassGeneratedCssTs tag")],
    );
}

// port: JsDocInfoParserTest#testParseWizaction1
#[test]
fn test_parse_wizaction1() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@wizaction*/"), &[])).unwrap()).is_wizaction()));
}

// port: JsDocInfoParserTest#testParseWizaction2
#[test]
fn test_parse_wizaction2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@wizaction \n@wizaction*/"),
        &[JsString::from("extra @wizaction tag")],
    );
}

// port: JsDocInfoParserTest#testParseProvideAlreadyProvided
#[test]
fn test_parse_provide_already_provided() {
    let mut f = set_up();
    assert!(
        (((f.parse(JsString::from("@provideAlreadyProvided*/"), &[])).unwrap())
            .is_provide_already_provided())
    );
}

// port: JsDocInfoParserTest#testParseProvideAlreadyProvidedExtra
#[test]
fn test_parse_provide_already_provided_extra() {
    let mut f = set_up();
    f.parse(
        JsString::from("@provideAlreadyProvided \n@provideAlreadyProvided*/"),
        &[JsString::from("extra @provideAlreadyProvided tag")],
    );
}

// port: JsDocInfoParserTest#testParseWizcallback
#[test]
fn test_parse_wizcallback() {
    let mut f = set_up();
    assert!((((f.parse(JsString::from("@wizcallback*/"), &[])).unwrap()).is_wizcallback()));
}

// port: JsDocInfoParserTest#testParseWizcallbackExpectDuplicateError
#[test]
fn test_parse_wizcallback_expect_duplicate_error() {
    let mut f = set_up();
    f.parse(
        JsString::from("@wizcallback \n@wizcallback*/"),
        &[JsString::from("extra @wizcallback tag")],
    );
}

// port: JsDocInfoParserTest#testTextExtents
#[test]
fn test_text_extents() {
    let mut f = set_up();
    f.parse_with_documentation(
        JsString::from("@return {@code foo} bar \n *    baz. */"),
        true,
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseDuplicateNoCollapse
#[test]
fn test_parse_duplicate_no_collapse() {
    let mut f = set_up();
    f.parse(
        JsString::from("@nocollapse \n * @nocollapse \n * @type {Object}*/"),
        &[JsString::from("extra @nocollapse tag")],
    );
}

// port: JsDocInfoParserTest#testParsePureOrBreakMyCode
#[test]
fn test_parse_pure_or_break_my_code() {
    let mut f = set_up();
    let mut js_doc_info = f.parse(JsString::from("@pureOrBreakMyCode */"), &[]);
    assert!((((js_doc_info.clone()).unwrap()).is_pure_or_break_my_code()));
}

// port: JsDocInfoParserTest#testParseCollapsibleOrBreakMyCode
#[test]
fn test_parse_collapsible_or_break_my_code() {
    let mut f = set_up();
    let mut js_doc_info = f.parse(JsString::from("@collapsibleOrBreakMyCode */"), &[]);
    assert!((((js_doc_info.clone()).unwrap()).is_collapsible_or_break_my_code()));
}

// port: JsDocInfoParserTest#testParseDuplicatePureOrBreakMyCode
#[test]
fn test_parse_duplicate_pure_or_break_my_code() {
    let mut f = set_up();
    f.parse(
        JsString::from("@pureOrBreakMyCode \n * @pureOrBreakMyCode \n * @type {Object}*/"),
        &[JsString::from("extra @pureOrBreakMyCode tag")],
    );
}

// port: JsDocInfoParserTest#testPreserveWhitespace1
#[test]
fn test_preserve_whitespace1() {
    let mut f = set_up();
    let mut jsdoc = f.preserve_whitespace_parse(JsString::from("this is a nice comment\n * that spans multiple lines\n *     with custom\n *     formatting\n * @author abc@google.com */\n"), &[]);
    assert_eq!(((jsdoc.clone()).unwrap()).get_block_description(), Some((JsString::from("this is a nice comment\n that spans multiple lines\n     with custom\n     formatting\n")).concat(&(JsString::from(" ")))));
    assert_documentation_in_marker(
        assert_annotation_marker(jsdoc.clone(), JsString::from("author"), 4, 3),
        JsString::from("abc@google.com"),
        10,
        4,
        24,
    );
}

// port: JsDocInfoParserTest#testPreserveWhitespace2
#[test]
fn test_preserve_whitespace2() {
    let mut f = set_up();
    let mut jsdoc = f.preserve_whitespace_parse(JsString::from("@param {string} x this is a nice comment\n * that spans multiple lines\n *     with custom\n *     formatting\n * @param {string} y */\n"), &[]);
    assert_eq!(((jsdoc.clone()).unwrap()).get_description_for_parameter(JsString::from("x")), Some((JsString::from(" this is a nice comment\n that spans multiple lines\n     with custom\n     formatting\n")).concat(&(JsString::from(" ")))));
}

// port: JsDocInfoParserTest#testPreserveWhitespace3
#[test]
fn test_preserve_whitespace3() {
    let mut f = set_up();
    let mut jsdoc = f.preserve_whitespace_parse(JsString::from("@return {string} this is a nice comment\n * that spans multiple lines\n *     with custom\n *     formatting\n * @param {string} y */\n"), &[]);
    assert_eq!(((jsdoc.clone()).unwrap()).get_return_description(), Some((JsString::from("this is a nice comment\n that spans multiple lines\n     with custom\n     formatting\n")).concat(&(JsString::from(" ")))));
}

// port: JsDocInfoParserTest#testPreserveWhitespace4
#[test]
fn test_preserve_whitespace4() {
    let mut f = set_up();
    let mut jsdoc = f.preserve_whitespace_parse(JsString::from("@throws {string} this is a nice comment\n * that spans multiple lines\n *     with custom\n *     formatting\n * @param {string} y */\n"), &[]);
    assert_eq!(((((((((jsdoc.clone()).unwrap()).get_markers()).into_iter()).next()).unwrap()).get_description().cloned()).unwrap()).get_item().cloned(), Some((JsString::from("{string} this is a nice comment\n that spans multiple lines\n     with custom\n     formatting\n")).concat(&(JsString::from(" ")))));
}

// port: JsDocInfoParserTest#testPreserveWhitespace5
#[test]
fn test_preserve_whitespace5() {
    let mut f = set_up();
    let mut jsdoc = f.preserve_whitespace_parse(JsString::from("@desc this is a nice comment\n * that spans multiple lines\n *     with custom\n *     formatting\n */\n"), &[]);
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description(),
        Some(JsString::from(
            " this is a nice comment\n that spans multiple lines\n     with custom\n     formatting\n"
        ))
    );
}

// port: JsDocInfoParserTest#testParseCommentWithStarsAfterLeadingSpace
#[test]
fn test_parse_comment_with_stars_after_leading_space() {
    let mut f = set_up();
    let mut jsdoc = f.preserve_whitespace_parse(
        JsString::from("@desc this comment has extra\n * * stars on new lines\n */\n"),
        &[],
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description(),
        Some(JsString::from(
            " this comment has extra\n * stars on new lines\n"
        ))
    );
}

// port: JsDocInfoParserTest#testParseCommentWithThickLeadingStarBlockPreserveWhitespace
#[test]
fn test_parse_comment_with_thick_leading_star_block_preserve_whitespace() {
    let mut f = set_up();
    let mut jsdoc = f.preserve_whitespace_parse(
        JsString::from("@desc line 2 has extra stars\n **** that should pad content\n */\n"),
        &[],
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description(),
        Some(JsString::from(
            " line 2 has extra stars\n*** that should pad content\n"
        ))
    );
}

// port: JsDocInfoParserTest#testParseCommentWithThickLeadingStarBlockSingeLine
#[test]
fn test_parse_comment_with_thick_leading_star_block_singe_line() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@desc line 2 has extra stars\n **** that should pad content\n */\n"),
        &[],
    );
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_description(),
        Some(JsString::from(
            "line 2 has extra stars *** that should pad content"
        ))
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions1
#[test]
fn test_parse_token_positions1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type\n {!Object} */"), &[]);
    assert!(info.clone().is_some());
    assert!(((info.clone()).unwrap()).get_type().is_some());
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(&f.ast, root, Token::BANG, 1, 2);
    check_token_position(
        &f.ast,
        ((root).get_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        1,
        3,
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions2
#[test]
fn test_parse_token_positions2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {?Object} */"), &[]);
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(&f.ast, root, Token::QMARK, 0, 7);
    check_token_position(
        &f.ast,
        ((root).get_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        8,
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions3
#[test]
fn test_parse_token_positions3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {Object!} */"), &[]);
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(&f.ast, root, Token::BANG, 0, 13);
    check_token_position(
        &f.ast,
        ((root).get_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        7,
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions4
#[test]
fn test_parse_token_positions4() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {Object?} */"), &[]);
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(&f.ast, root, Token::QMARK, 0, 13);
    check_token_position(
        &f.ast,
        ((root).get_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        7,
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions5
#[test]
fn test_parse_token_positions5() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {(string|!Object)} */"), &[]);
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(
        &f.ast,
        ((root).get_last_child(&f.ast)).unwrap(),
        Token::BANG,
        0,
        15,
    );
    check_token_position(
        &f.ast,
        ((((root).get_last_child(&f.ast)).unwrap()).get_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        16,
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions6
#[test]
fn test_parse_token_positions6() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {Set<!string>} */"), &[]);
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(
        &f.ast,
        ((root).get_first_first_child(&f.ast)).unwrap(),
        Token::BANG,
        0,
        11,
    );
    check_token_position(
        &f.ast,
        ((((root).get_first_child(&f.ast)).unwrap()).get_first_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        12,
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions7
#[test]
fn test_parse_token_positions7() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {?{foo: !Object}} */"), &[]);
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(&f.ast, root, Token::QMARK, 0, 7);
    check_token_position(
        &f.ast,
        ((((root).get_first_first_child(&f.ast)).unwrap()).get_first_first_child(&f.ast)).unwrap(),
        Token::STRING_KEY,
        0,
        9,
    );
    check_token_position(
        &f.ast,
        ((((((root).get_first_child(&f.ast)).unwrap()).get_first_first_child(&f.ast)).unwrap())
            .get_last_child(&f.ast))
        .unwrap(),
        Token::BANG,
        0,
        14,
    );
    check_token_position(
        &f.ast,
        ((((((((root).get_first_child(&f.ast)).unwrap()).get_first_first_child(&f.ast))
            .unwrap())
        .get_last_child(&f.ast))
        .unwrap())
        .get_first_child(&f.ast))
        .unwrap(),
        Token::STRINGLIT,
        0,
        15,
    );
}

// port: JsDocInfoParserTest#testParseTokenPositions8
#[test]
fn test_parse_token_positions8() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {function(!Object): ?string} */"), &[]);
    let mut root = ((((info.clone()).unwrap()).get_type()).unwrap()).get_root();
    check_token_position(
        &f.ast,
        ((root).get_first_first_child(&f.ast)).unwrap(),
        Token::BANG,
        0,
        16,
    );
    check_token_position(
        &f.ast,
        ((((root).get_first_child(&f.ast)).unwrap()).get_first_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        17,
    );
    check_token_position(
        &f.ast,
        ((root).get_last_child(&f.ast)).unwrap(),
        Token::QMARK,
        0,
        26,
    );
    check_token_position(
        &f.ast,
        ((((root).get_last_child(&f.ast)).unwrap()).get_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        27,
    );
}

// port: JsDocInfoParserTest#testBoundedGeneric
#[test]
fn test_bounded_generic() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template {number} T */"),
        &[JsString::from(
            "Bounded generic semantics are currently still in development",
        )],
    );
}

// port: JsDocInfoParserTest#testUnbalancedBracesBoundedGeneric
#[test]
fn test_unbalanced_braces_bounded_generic() {
    let mut f = set_up();
    f.parse(JsString::from("@template {number T */"), &[JsString::from("Bounded generic semantics are currently still in development"), ((JsString::from("Bad type annotation. expected closing } See")).concat(&(JsString::from(" https://github.com/google/closure-compiler/wiki/Annotating-JavaScript-for-the-Closure-Compiler")))).concat(&(JsString::from(" for more information.")))]);
}

// port: JsDocInfoParserTest#testMultipleBoundedGeneric
#[test]
fn test_multiple_bounded_generic() {
    let mut f = set_up();
    f.parse(JsString::from("@template {string} T,U */"), &[JsString::from("Bounded generic semantics are currently still in development"), (JsString::from("Bad type annotation. Multiple template names cannot be declared with bounds or TTL.")).concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))]);
}

// port: JsDocInfoParserTest#testMultipleBoundsBoundedGeneric
#[test]
fn test_multiple_bounds_bounded_generic() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template {string} T, {number} U */"),
        &[
            JsString::from("Bounded generic semantics are currently still in development"),
            JsString::from("name not recognized due to syntax error."),
        ],
    );
}

// port: JsDocInfoParserTest#testBadTypeExpressionBoundedGeneric
#[test]
fn test_bad_type_expression_bounded_generic() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template {string || number} T */"),
        &[
            JsString::from("Bounded generic semantics are currently still in development"),
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            JsString::from("name not recognized due to syntax error."),
            (JsString::from("Bad type annotation. @template tag missing type name."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testReuseTemplateTypeName
#[test]
fn test_reuse_template_type_name() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template {string} T \n* @template {number} T */"),
        &[
            JsString::from("Bounded generic semantics are currently still in development"),
            JsString::from("Bounded generic semantics are currently still in development"),
            (JsString::from(
                "Bad type annotation. Type name(s) for @template annotation declared twice.",
            ))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testMultipleTemplateBoundDeclarations
#[test]
fn test_multiple_template_bound_declarations() {
    let mut f = set_up();
    f.parse(
        JsString::from(" @template {string} T \n* @template {number} U */"),
        &[
            JsString::from("Bounded generic semantics are currently still in development"),
            JsString::from("Bounded generic semantics are currently still in development"),
        ],
    );
}

// port: JsDocInfoParserTest#testTemplateJSTypeExpression
#[test]
fn test_template_jstype_expression() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@template {string|number} T */"),
        &[JsString::from(
            "Bounded generic semantics are currently still in development",
        )],
    );
    assert!(((((info.clone()).unwrap()).get_template_types()).len() as i32 == 1));
    assert!(((((info.clone()).unwrap()).get_template_type_names())[0usize].clone() == "T"));
    let mut root = (((((info.clone()).unwrap()).get_template_types())
        .get(&(JsString::from("T")))
        .cloned()
        .flatten())
    .unwrap())
    .get_root();
    check_token_position(&f.ast, root, Token::PIPE, 0, 18);
    check_token_position(
        &f.ast,
        ((root).get_first_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        11,
    );
    assert!(((((root).get_first_child(&f.ast)).unwrap()).get_string(&f.ast) == "string"));
    check_token_position(
        &f.ast,
        ((root).get_second_child(&f.ast)).unwrap(),
        Token::STRINGLIT,
        0,
        18,
    );
    assert!(((((root).get_second_child(&f.ast)).unwrap()).get_string(&f.ast) == "number"));
}

// port: JsDocInfoParserTest#testClosureUnawareMode_fileoverview_whitespace
#[test]
fn test_closure_unaware_mode_fileoverview_whitespace() {
    let mut f = set_up();
    let mut info = f.parse_file_overview(
        JsString::from("@fileoverview\n@closureUnaware {WHITESPACE} */"),
        &[JsString::from(
            "@closureUnaware annotation is not allowed in this compilation",
        )],
    );
    assert!((((info.clone()).unwrap()).is_closure_unaware_code()));
    assert_eq!(
        ((info.clone()).unwrap()).get_per_file_closure_unaware_mode(),
        Some(PerFileClosureUnawareMode::WHITESPACE)
    );
}

// port: JsDocInfoParserTest#testClosureUnawareMode_fileoverview_simple
#[test]
fn test_closure_unaware_mode_fileoverview_simple() {
    let mut f = set_up();
    let mut info = f.parse_file_overview(
        JsString::from("@fileoverview\n@closureUnaware {SIMPLE} */"),
        &[JsString::from(
            "@closureUnaware annotation is not allowed in this compilation",
        )],
    );
    assert!((((info.clone()).unwrap()).is_closure_unaware_code()));
    assert_eq!(
        ((info.clone()).unwrap()).get_per_file_closure_unaware_mode(),
        Some(PerFileClosureUnawareMode::SIMPLE)
    );
}

// port: JsDocInfoParserTest#testSupportsClosureUnaware_invalidValue_warnsAndDefaultsToSimple
#[test]
fn test_supports_closure_unaware_invalid_value_warns_and_defaults_to_simple() {
    let mut f = set_up();
    let mut info = f.parse_file_overview(
        JsString::from("@fileoverview @closureUnaware {FOOBAR} */"),
        &[
            JsString::from("@closureUnaware annotation is not allowed in this compilation"),
            JsString::from("invalid value for @closureUnaware: FOOBAR"),
        ],
    );
    assert!((((info.clone()).unwrap()).is_closure_unaware_code()));
    assert_eq!(
        ((info.clone()).unwrap()).get_per_file_closure_unaware_mode(),
        Some(PerFileClosureUnawareMode::SIMPLE)
    );
}

// port: JsDocInfoParserTest#testClosureUnawareMode_outsideFileoverview_errorIfSpecified
#[test]
fn test_closure_unaware_mode_outside_fileoverview_error_if_specified() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@closureUnaware {SIMPLE} */"),
        &[JsString::from(
            "@closureUnaware annotation is not allowed in this compilation",
        )],
    );
    assert!((((info.clone()).unwrap()).is_closure_unaware_code()));
}

// port: JsDocInfoParserTest#testParseRecordType1
#[test]
fn test_parse_record_type1() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{x}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType2
#[test]
fn test_parse_record_type2() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{z, y}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType3
#[test]
fn test_parse_record_type3() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{z, y, x, q, hello, thisisatest}} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType4
#[test]
fn test_parse_record_type4() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{a, 'a', 'hello', 2, this, do, while, for}} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType5
#[test]
fn test_parse_record_type5() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{x : hello}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType6
#[test]
fn test_parse_record_type6() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{'x' : hello}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType7
#[test]
fn test_parse_record_type7() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{'x' : !hello}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType8
#[test]
fn test_parse_record_type8() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{'x' : !hello, y : bar}} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType9
#[test]
fn test_parse_record_type9() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{'x' : !hello, y : {z : bar, 3 : meh}}} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType10
#[test]
fn test_parse_record_type10() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{__proto__ : moo}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType11
#[test]
fn test_parse_record_type11() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{a : b} n\n*/"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParseRecordType12
#[test]
fn test_parse_record_type12() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{!hello : hey}} n\n*/"),
        &[
            (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseRecordType13
#[test]
fn test_parse_record_type13() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{x}|number} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType14
#[test]
fn test_parse_record_type14() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{x : y}|number} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType15
#[test]
fn test_parse_record_type15() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{'x' : y}|number} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType16
#[test]
fn test_parse_record_type16() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{x, y}|number} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType17
#[test]
fn test_parse_record_type17() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{x : hello, 'y'}|number} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType18
#[test]
fn test_parse_record_type18() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {number|{x : hello, 'y'}} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType19
#[test]
fn test_parse_record_type19() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {?{x : hello, 'y'}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType20
#[test]
fn test_parse_record_type20() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {!{x : hello, 'y'}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseRecordType21
#[test]
fn test_parse_record_type21() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{x : hello, 'y'}|boolean} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType22
#[test]
fn test_parse_record_type22() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{x : hello, 'y'}|function()} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType23
#[test]
fn test_parse_record_type23() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {{x : function(), 'y'}|function()} n\n*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParseRecordType24
#[test]
fn test_parse_record_type24() {
    let mut f = set_up();
    f.parse_full(JsString::from("/** @param {{a : b, c,}} n\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParseParamError1
#[test]
fn test_parse_param_error1() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param\n*/"),
        &[
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseParamError2
#[test]
fn test_parse_param_error2() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {Number}*/"),
        &[
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseParamError3
#[test]
fn test_parse_param_error3() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {Number}\n*/"),
        &[
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testParseParamError4
#[test]
fn test_parse_param_error4() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {Number}\n* * num */"),
        &[
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testFullRegression1
#[test]
fn test_full_regression1() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param (string,number) foo*/function bar(foo){}"),
        &[
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testFullRegression2
#[test]
fn test_full_regression2() {
    let mut f = set_up();
    f.parse_full(
        JsString::from("/** @param {string,number) foo*/function bar(foo){}"),
        &[
            (JsString::from("Bad type annotation. expected closing }"))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            (JsString::from("Bad type annotation. expecting a variable name in a @param tag."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
}

// port: JsDocInfoParserTest#testFullRegression3
#[test]
fn test_full_regression3() {
    let mut f = set_up();
    f.parse_full(JsString::from("/**..\n*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTemplateTypeNameDeclaredTwice2
#[test]
fn test_parser_with_template_type_name_declared_twice2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := S =: \n @template T := R =:*/"),
        &[(JsString::from(
            "Bad type annotation. Type name(s) for @template annotation declared twice.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParserWithTemplateTypeNameDeclaredTwice3
#[test]
fn test_parser_with_template_type_name_declared_twice3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T \n @template T := R =:*/"),
        &[(JsString::from(
            "Bad type annotation. Type name(s) for @template annotation declared twice.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParserWithTemplateTypeNameDeclaredTwice4
#[test]
fn test_parser_with_template_type_name_declared_twice4() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := R =: \n @template T*/"),
        &[(JsString::from(
            "Bad type annotation. Type name(s) for @template annotation declared twice.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testParserWithTypeTransformationNewline
#[test]
fn test_parser_with_type_transformation_newline() {
    let mut f = set_up();
    f.parse(JsString::from("@template R := \n 'string' =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTypeTransformation
#[test]
fn test_parser_with_type_transformation() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := 'string' =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTypeTransformation2
#[test]
fn test_parser_with_type_transformation2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := 'string' =:\nRandom text*/\n"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTypeTransformationMultipleNames
#[test]
fn test_parser_with_type_transformation_multiple_names() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T, R := 'string' =:*/"),
        &[(JsString::from(
            "Bad type annotation. Multiple template names cannot be declared with bounds or TTL.",
        ))
        .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
}

// port: JsDocInfoParserTest#testTTLLineNoCharNo
#[test]
fn test_ttl_line_no_char_no() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("Some text on line 0\nMore text! This is line 1\n@template T := foo =:*/\n"),
        &[],
    );
    assert_eq!(
        (((info.clone()).unwrap()).get_type_transformations()).len(),
        1usize
    );
    let mut n = (((info.clone()).unwrap()).get_type_transformations())
        .get(&(JsString::from("T")))
        .copied();
    assert_node((n).unwrap()).has_lineno(&f.ast, 2);
    assert_eq!(((n).unwrap()).get_charno(&f.ast), 0);
}

// port: JsDocInfoParserTest#testMultilineTTLLineNoCharNo
#[test]
fn test_multiline_ttl_line_no_char_no() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from(
            "Some text on line 0\nMore text! This is line 1\n@template T :=\n  foo\n=:*/\n",
        ),
        &[],
    );
    assert_eq!(
        (((info.clone()).unwrap()).get_type_transformations()).len(),
        1usize
    );
    let mut n = (((info.clone()).unwrap()).get_type_transformations())
        .get(&(JsString::from("T")))
        .copied();
    assert_node((n).unwrap()).has_lineno(&f.ast, 2);
    assert_eq!(((n).unwrap()).get_charno(&f.ast), 0);
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidOperation
#[test]
fn test_parser_with_ttl_invalid_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := foo() =:*/"),
        &[JsString::from(
            "Bad type annotation. Invalid type transformation expression",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidTypeTransformation
#[test]
fn test_parser_with_ttl_invalid_type_transformation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := var a; =:*/"),
        &[JsString::from(
            "Bad type annotation. Invalid type transformation expression",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidTypename
#[test]
fn test_parser_with_ttl_valid_typename() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := foo =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLValidTypename2
#[test]
fn test_parser_with_ttl_valid_typename2() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := R =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLValidTypename3
#[test]
fn test_parser_with_ttl_valid_typename3() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := _Foo =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLValidTypename4
#[test]
fn test_parser_with_ttl_valid_typename4() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := $foo =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLBasicType
#[test]
fn test_parser_with_ttl_basic_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := 'string' =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLValidUnionType
#[test]
fn test_parser_with_ttl_valid_union_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := union('string', 'number') =:*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidUnionType2
#[test]
fn test_parser_with_ttl_valid_union_type2() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := union(R, S) =:*/"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLValidUnionType3
#[test]
fn test_parser_with_ttl_valid_union_type3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := union(R, 'string', S) =:*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLEmptyUnionType
#[test]
fn test_parser_with_ttl_empty_union_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := union() =:*/"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in union",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLSingletonUnionType
#[test]
fn test_parser_with_ttl_singleton_union_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := union('string') =:*/"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in union",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidUnionType2
#[test]
fn test_parser_with_ttl_invalid_union_type2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := union(function(a){}, T) =:*/"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside union type"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithNestedUnionFirstParam
#[test]
fn test_parser_with_nested_union_first_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := union(union(N, 'null'), S) =:*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithNestedUnionSecondParam
#[test]
fn test_parser_with_nested_union_second_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := union(N, union('null', S)) =:*/"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithNestedBooleanFirstParam
#[test]
fn test_parser_with_nested_boolean_first_param() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := cond( eq(cond(eq(N, N), 'string', 'number'), 'string'),\n'string',\n'number') =: */\n"), &[]);
}

// port: JsDocInfoParserTest#testParserWithNestedBooleanSecondParam
#[test]
fn test_parser_with_nested_boolean_second_param() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := cond( eq('string', cond(eq(N, N), 'string', 'number')),\n'string',\n'number') =:*/\n"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLConditional
#[test]
fn test_parser_with_ttl_conditional() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(T, R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditional2
#[test]
fn test_parser_with_ttl_conditional2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(sub(T, R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalStringEquivalence
#[test]
fn test_parser_with_ttl_conditional_string_equivalence() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(streq(R, S), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalStringEquivalence2
#[test]
fn test_parser_with_ttl_conditional_string_equivalence2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(streq(R, 'foo'), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalStringEquivalence3
#[test]
fn test_parser_with_ttl_conditional_string_equivalence3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(streq('foo', 'bar'), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsConstructor
#[test]
fn test_parser_with_ttl_conditional_is_constructor() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsConstructor2
#[test]
fn test_parser_with_ttl_conditional_is_constructor2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor('foo'), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsTemplatized
#[test]
fn test_parser_with_ttl_conditional_is_templatized() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isTemplatized(R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsTemplatized2
#[test]
fn test_parser_with_ttl_conditional_is_templatized2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isTemplatized('foo'), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsRecord
#[test]
fn test_parser_with_ttl_conditional_is_record() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isRecord(R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalAndOperation
#[test]
fn test_parser_with_ttl_conditional_and_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(R) && isCtor(S), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalOrOperation
#[test]
fn test_parser_with_ttl_conditional_or_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(R) || isCtor(S), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalNotOperation
#[test]
fn test_parser_with_ttl_conditional_not_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(!isCtor(R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalNestedBoolOperation
#[test]
fn test_parser_with_ttl_conditional_nested_bool_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(((isCtor(R) || isCtor(S)) && !isCtor(R)), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsRecord2
#[test]
fn test_parser_with_ttl_conditional_is_record2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isRecord('foo'), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsDefined
#[test]
fn test_parser_with_ttl_conditional_is_defined() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isDefined(R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsUnknown
#[test]
fn test_parser_with_ttl_conditional_is_unknown() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isUnknown(R), R, S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalStringEquivalenceInvalidParam
#[test]
fn test_parser_with_ttl_conditional_string_equivalence_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(streq('foo', foo()), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid string"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalStringEquivalenceInvalidParamEmptyStr
#[test]
fn test_parser_with_ttl_conditional_string_equivalence_invalid_param_empty_str() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(streq('', S), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid string parameter"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsConstructorInvalidParam
#[test]
fn test_parser_with_ttl_conditional_is_constructor_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(foo()), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsTemplatizedInvalidParam
#[test]
fn test_parser_with_ttl_conditional_is_templatized_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isTemplatized(foo()), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsRecordInvalidParam
#[test]
fn test_parser_with_ttl_conditional_is_record_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isRecord(foo()), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsDefinedInvalidParam
#[test]
fn test_parser_with_ttl_conditional_is_defined_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isDefined('foo'), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid name"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalIsUnknownInvalidParam
#[test]
fn test_parser_with_ttl_conditional_is_unknown_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isUnknown(foo()), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalAndInvalidParam
#[test]
fn test_parser_with_ttl_conditional_and_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond('foo' && isCtor(R), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid boolean expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalAndInvalidParam2
#[test]
fn test_parser_with_ttl_conditional_and_invalid_param2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(R) && 'foo', R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid boolean expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalOrInvalidParam
#[test]
fn test_parser_with_ttl_conditional_or_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond('foo' || isCtor(R), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid boolean expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalOrInvalidParam2
#[test]
fn test_parser_with_ttl_conditional_or_invalid_param2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(R) || 'foo', R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid boolean expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLConditionalNotInvalidParam
#[test]
fn test_parser_with_ttl_conditional_not_invalid_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(!'foo', R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid boolean expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLExtraParamBoolean
#[test]
fn test_parser_with_ttl_extra_param_boolean() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(T, R, S), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in eq"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLExtraParamStringEq
#[test]
fn test_parser_with_ttl_extra_param_string_eq() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(streq(T, R, S), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in streq"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLExtraParamIsConstructor
#[test]
fn test_parser_with_ttl_extra_param_is_constructor() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(T, R, S), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in isCtor"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLExtraParamIsTemplatized
#[test]
fn test_parser_with_ttl_extra_param_is_templatized() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isTemplatized(T, R, S), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in isTemplatized"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLExtraParamIsRecord
#[test]
fn test_parser_with_ttl_extra_param_is_record() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isRecord(T, R), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in isRecord"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLExtraParamIsDefined
#[test]
fn test_parser_with_ttl_extra_param_is_defined() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isDefined(T, R), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in isDefined"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLExtraParamIsUnknown
#[test]
fn test_parser_with_ttl_extra_param_is_unknown() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isUnknown(T, R), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in isUnknown"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMissingParamBoolean
#[test]
fn test_parser_with_ttl_missing_param_boolean() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(T), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in eq"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMissingParamStringEquivalence
#[test]
fn test_parser_with_ttl_missing_param_string_equivalence() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(streq(T), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in streq"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMissingParamIsConstructor
#[test]
fn test_parser_with_ttl_missing_param_is_constructor() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isCtor(), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in isCtor"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMissingParamIsTemplatized
#[test]
fn test_parser_with_ttl_missing_param_is_templatized() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isTemplatized(), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in isTemplatized"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMissingParamIsRecord
#[test]
fn test_parser_with_ttl_missing_param_is_record() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isRecord(), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in isRecord"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMissingParamIsDefined
#[test]
fn test_parser_with_ttl_missing_param_is_defined() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isDefined(), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in isDefined"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMissingParamIsUnknown
#[test]
fn test_parser_with_ttl_missing_param_is_unknown() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(isUnknown(), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in isUnknown"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidBooleanConditional
#[test]
fn test_parser_with_ttl_invalid_boolean_conditional() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(aaa, R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid boolean expression"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidBooleanConditional2
#[test]
fn test_parser_with_ttl_invalid_boolean_conditional2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(foo(T, R), S, R) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid boolean predicate"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidBooleanConditional3
#[test]
fn test_parser_with_ttl_invalid_boolean_conditional3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(T, foo()), R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside boolean"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidConditionalMissingParam
#[test]
fn test_parser_with_ttl_invalid_conditional_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(sub(T, R), T) =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in cond",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidConditionalExtraParam
#[test]
fn test_parser_with_ttl_invalid_conditional_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(sub(T, R), T, R, R) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in cond",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidConditional
#[test]
fn test_parser_with_ttl_invalid_conditional() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(T, R), foo(), S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidConditional2
#[test]
fn test_parser_with_ttl_invalid_conditional2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(T, R), S, foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside conditional"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidMapunion
#[test]
fn test_parser_with_ttl_valid_mapunion() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(T, (S) => S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidMapunion2
#[test]
fn test_parser_with_ttl_valid_mapunion2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(union('string', 'number'), (S) => S) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMapunionType
#[test]
fn test_parser_with_ttl_invalid_mapunion_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(foo(), (S) => S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside mapunion"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMapunionFn
#[test]
fn test_parser_with_ttl_invalid_mapunion_fn() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid map function"),
            JsString::from("Bad type annotation. Invalid expression inside mapunion"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMapunionMissingParams
#[test]
fn test_parser_with_ttl_invalid_mapunion_missing_params() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(T) =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in mapunion",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMapunionExtraParams
#[test]
fn test_parser_with_ttl_invalid_mapunion_extra_params() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(T, (S) => S, R) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in mapunion",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMapunionMissingFnParams
#[test]
fn test_parser_with_ttl_invalid_mapunion_missing_fn_params() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(T, () => S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in map function"),
            JsString::from("Bad type annotation. Invalid expression inside mapunion"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMapunionExtraFnParams
#[test]
fn test_parser_with_ttl_invalid_mapunion_extra_fn_params() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(T, (S, R) => S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in map function"),
            JsString::from("Bad type annotation. Invalid expression inside mapunion"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMapunionFunctionBody
#[test]
fn test_parser_with_ttl_invalid_mapunion_function_body() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := mapunion(T, (S) => foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside map function body"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLUseCaseObject
#[test]
fn test_parser_with_ttl_use_case_object() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := mapunion(T, (x) => cond(eq(x, 'string'), 'String',\ncond(eq(x, 'number'), 'Number',\ncond(eq(x, 'boolean'), 'Boolean',\ncond(eq(x, 'null'), 'Object',\ncond(eq(x, 'undefined'), 'Object',\nx)))))) =: */\n"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLNoneType
#[test]
fn test_parser_with_ttl_none_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := none() =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLNoneType2
#[test]
fn test_parser_with_ttl_none_type2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(S, none()), S, T) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidNoneType
#[test]
fn test_parser_with_ttl_invalid_none_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := none(foo) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in none",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidNoneType2
#[test]
fn test_parser_with_ttl_invalid_none_type2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := none(a, b, c) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in none",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLAllType
#[test]
fn test_parser_with_ttl_all_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := all() =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLAllType2
#[test]
fn test_parser_with_ttl_all_type2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(S, all()), S, T) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidAllType
#[test]
fn test_parser_with_ttl_invalid_all_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := all(foo) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in all",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidAllType2
#[test]
fn test_parser_with_ttl_invalid_all_type2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := all(a, b, c) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in all",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLUnknownType
#[test]
fn test_parser_with_ttl_unknown_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := unknown() =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLUnknownType2
#[test]
fn test_parser_with_ttl_unknown_type2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := cond(eq(S, unknown()), S, T) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidUnknownType
#[test]
fn test_parser_with_ttl_invalid_unknown_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := unknown(foo) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in unknown",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidUnknownType2
#[test]
fn test_parser_with_ttl_invalid_unknown_type2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := unknown(a, b, c) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in unknown",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLTemplateTypeOperation
#[test]
fn test_parser_with_ttl_template_type_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := type('Map', 'string', 'number') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLTemplateTypeOperationGeneric
#[test]
fn test_parser_with_ttl_template_type_operation_generic() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := type('Array', T) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLTemplateTypeOperationGeneric2
#[test]
fn test_parser_with_ttl_template_type_operation_generic2() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := type(T, R) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLTemplateTypeOperationNestedGeneric
#[test]
fn test_parser_with_ttl_template_type_operation_nested_generic() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := type(T, type(R, S)) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLTemplateTypeOperationGenericWithUnion
#[test]
fn test_parser_with_ttl_template_type_operation_generic_with_union() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := type(T, union(R, S)) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidTemplateTypeOperationGenericUnion
#[test]
fn test_parser_with_ttl_invalid_template_type_operation_generic_union() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := type(union(R, S), T) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type name or type variable"),
            JsString::from(
                "Bad type annotation. Invalid expression inside template type operation",
            ),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidTypeOperationNestedGeneric
#[test]
fn test_parser_with_ttl_invalid_type_operation_nested_generic() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := type(T, foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from(
                "Bad type annotation. Invalid expression inside template type operation",
            ),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidRawTypeOperation
#[test]
fn test_parser_with_ttl_valid_raw_type_operation() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := rawTypeOf(T) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLValidRawTypeOperation2
#[test]
fn test_parser_with_ttl_valid_raw_type_operation2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := rawTypeOf(type(T, R)) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidRawTypeOperation
#[test]
fn test_parser_with_ttl_invalid_raw_type_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := rawTypeOf(foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside rawTypeOf"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidRawTypeOperationExtraParam
#[test]
fn test_parser_with_ttl_invalid_raw_type_operation_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := rawTypeOf(R, S) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in rawTypeOf",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidRawTypeOperationMissingParam
#[test]
fn test_parser_with_ttl_invalid_raw_type_operation_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := rawTypeOf() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in rawTypeOf",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNestedRawTypeOperation
#[test]
fn test_parser_with_ttl_nested_raw_type_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := rawTypeOf(type(T, rawTypeOf(type(R, S)))) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidTemplateTypeOfOperation
#[test]
fn test_parser_with_ttl_valid_template_type_of_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(T, 1) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidTemplateTypeOfOperation2
#[test]
fn test_parser_with_ttl_valid_template_type_of_operation2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(type(T, R), 1) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidFirstParamTemplateTypeOf
#[test]
fn test_parser_with_ttl_invalid_first_param_template_type_of() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(foo(), 1) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside templateTypeOf"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidSecondParamTemplateTypeOf
#[test]
fn test_parser_with_ttl_invalid_second_param_template_type_of() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(R, foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid index"),
            JsString::from("Bad type annotation. Invalid expression inside templateTypeOf"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidSecondParamTemplateTypeOf2
#[test]
fn test_parser_with_ttl_invalid_second_param_template_type_of2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(R, 1.5) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid index"),
            JsString::from("Bad type annotation. Invalid expression inside templateTypeOf"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidSecondParamTemplateTypeOf3
#[test]
fn test_parser_with_ttl_invalid_second_param_template_type_of3() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(R, -1) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid index"),
            JsString::from("Bad type annotation. Invalid expression inside templateTypeOf"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidTemplateTypeOfExtraParam
#[test]
fn test_parser_with_ttl_invalid_template_type_of_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(R, 1, S) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in templateTypeOf",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidTemplateTypeOfMissingParam
#[test]
fn test_parser_with_ttl_invalid_template_type_of_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in templateTypeOf",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidTemplateTypeOfMissingParam2
#[test]
fn test_parser_with_ttl_invalid_template_type_of_missing_param2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := templateTypeOf(T) =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in templateTypeOf",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNestedTemplateTypeOfOperation
#[test]
fn test_parser_with_ttl_nested_template_type_of_operation() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "@template T := templateTypeOf(templateTypeOf(type(T, type(R, S)), 0),0) =: */",
        ),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidPrintType
#[test]
fn test_parser_with_ttl_valid_print_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := printType('msg', R) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidFirstParamPrintType
#[test]
fn test_parser_with_ttl_invalid_first_param_print_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := printType(foo(), R) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid message"),
            JsString::from("Bad type annotation. Invalid expression inside printType"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidSecondParamPrintType
#[test]
fn test_parser_with_ttl_invalid_second_param_print_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := printType('msg', foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside printType"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidPrintTypeExtraParam
#[test]
fn test_parser_with_ttl_invalid_print_type_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := printType(R, S, U) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in printType",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidPrintTypeOfMissingParam
#[test]
fn test_parser_with_ttl_invalid_print_type_of_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := printType() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in printType",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidPrintTypeOfMissingParam2
#[test]
fn test_parser_with_ttl_invalid_print_type_of_missing_param2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := printType(R) =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in printType",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidPropType
#[test]
fn test_parser_with_ttl_valid_prop_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := propType('p', R) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidFirstParamPropType
#[test]
fn test_parser_with_ttl_invalid_first_param_prop_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := propType(foo(), R) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid property name"),
            JsString::from("Bad type annotation. Invalid expression inside propType"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidSecondParamPropType
#[test]
fn test_parser_with_ttl_invalid_second_param_prop_type() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := propType('msg', foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside propType"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidPropTypeExtraParam
#[test]
fn test_parser_with_ttl_invalid_prop_type_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := propType(R, S, U) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in propType",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidPropTypeOfMissingParam
#[test]
fn test_parser_with_ttl_invalid_prop_type_of_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := propType() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in propType",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidPropTypeOfMissingParam2
#[test]
fn test_parser_with_ttl_invalid_prop_type_of_missing_param2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := propType(R) =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in propType",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLRecordType
#[test]
fn test_parser_with_ttl_record_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := record({prop:T}) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLNestedRecordType
#[test]
fn test_parser_with_ttl_nested_record_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := record({prop: record({p1:'number', p2:'boolean'}), x:'string' })\n=: */\n"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidRecordTypeMissingParam
#[test]
fn test_parser_with_ttl_invalid_record_type_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := record() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in record",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLMergeRecords
#[test]
fn test_parser_with_ttl_merge_records() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := record({x:'number'}, {y:'number'}) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMergeRecords
#[test]
fn test_parser_with_ttl_invalid_merge_records() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := record({x:'number'}, foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside record"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidRecordTypeWithInvalidTypeInProperty
#[test]
fn test_parser_with_ttl_invalid_record_type_with_invalid_type_in_property() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := record({prop:foo()}) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside record"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidRecordTypeMissingTypeInProperty
#[test]
fn test_parser_with_ttl_invalid_record_type_missing_type_in_property() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := record({prop}) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid property, missing type"),
            JsString::from("Bad type annotation. Invalid expression inside record"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidRecordTypeInvalidRecordExpression
#[test]
fn test_parser_with_ttl_invalid_record_type_invalid_record_expression() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := record(foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside record"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLRecordTypeTypeVars
#[test]
fn test_parser_with_ttl_record_type_type_vars() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := record(T, R) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLEmptyRecordType
#[test]
fn test_parser_with_ttl_empty_record_type() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := record({}) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLTypeTransformationInFirstParamMapunion
#[test]
fn test_parser_with_ttl_type_transformation_in_first_param_mapunion() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "@template T := mapunion(templateTypeOf(type(R, union(S, U)), 0), (x) => x) =: */\n",
        ),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidMaprecord
#[test]
fn test_parser_with_ttl_valid_maprecord() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R, (K, V) => V) =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLValidMaprecord2
#[test]
fn test_parser_with_ttl_valid_maprecord2() {
    let mut f = set_up();
    f.parse(
        JsString::from(
            "@template T := maprecord(record({x:'string', y:'number'}), (K, V) => V) =: */",
        ),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordFirstParam
#[test]
fn test_parser_with_ttl_invalid_maprecord_first_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(foo(), (K, V) => V) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside maprecord"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordNotAFunction
#[test]
fn test_parser_with_ttl_invalid_maprecord_not_a_function() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R, S) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid map function"),
            JsString::from("Bad type annotation. Invalid expression inside maprecord"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordMissingParams
#[test]
fn test_parser_with_ttl_invalid_maprecord_missing_params() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R) =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in maprecord",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordExtraParams
#[test]
fn test_parser_with_ttl_invalid_maprecord_extra_params() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R, (K, V) => V, R) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in maprecord",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordMissingParamsInMapFunction
#[test]
fn test_parser_with_ttl_invalid_maprecord_missing_params_in_map_function() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R, () => S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in map function"),
            JsString::from("Bad type annotation. Invalid expression inside maprecord"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordMissingParamsInMapFunction2
#[test]
fn test_parser_with_ttl_invalid_maprecord_missing_params_in_map_function2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R, (K) => S) =: */"),
        &[
            JsString::from("Bad type annotation. Missing parameter in map function"),
            JsString::from("Bad type annotation. Invalid expression inside maprecord"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordExtraParamsInMapFunction
#[test]
fn test_parser_with_ttl_invalid_maprecord_extra_params_in_map_function() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R, (K, V, S) => S) =: */"),
        &[
            JsString::from("Bad type annotation. Found extra parameter in map function"),
            JsString::from("Bad type annotation. Invalid expression inside maprecord"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInvalidMaprecordInvalidFunctionBody
#[test]
fn test_parser_with_ttl_invalid_maprecord_invalid_function_body() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := maprecord(R, (K, V) => foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside map function body"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLTypeOfVar
#[test]
fn test_parser_with_ttl_type_of_var() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := typeOfVar('x') =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLTypeOfVar2
#[test]
fn test_parser_with_ttl_type_of_var2() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := typeOfVar('x.y') =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLTypeOfVarInvalidName
#[test]
fn test_parser_with_ttl_type_of_var_invalid_name() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeOfVar(foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid name"),
            JsString::from("Bad type annotation. Invalid expression inside typeOfVar"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLTypeOfVarMissingParam
#[test]
fn test_parser_with_ttl_type_of_var_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeOfVar() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in typeOfVar",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLTypeOfVarExtraParam
#[test]
fn test_parser_with_ttl_type_of_var_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeOfVar(a, b) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in typeOfVar",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInstanceOf
#[test]
fn test_parser_with_ttl_instance_of() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := instanceOf('x') =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLInstanceOf2
#[test]
fn test_parser_with_ttl_instance_of2() {
    let mut f = set_up();
    f.parse(JsString::from("@template T := instanceOf(R) =: */"), &[]);
}

// port: JsDocInfoParserTest#testParserWithTTLInstanceOfInvalidName
#[test]
fn test_parser_with_ttl_instance_of_invalid_name() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := instanceOf(foo()) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid type transformation expression"),
            JsString::from("Bad type annotation. Invalid expression inside instanceOf"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInstanceOfMissingParam
#[test]
fn test_parser_with_ttl_instance_of_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := instanceOf() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in instanceOf",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLInstanceOfExtraParam
#[test]
fn test_parser_with_ttl_instance_of_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := instanceOf(a, b) =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in instanceOf",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprBasic
#[test]
fn test_parser_with_ttl_native_type_expr_basic() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('string') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprBasic2
#[test]
fn test_parser_with_ttl_native_type_expr_basic2() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('goog.ui.Menu') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprUnion
#[test]
fn test_parser_with_ttl_native_type_expr_union() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('number|boolean') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprRecord
#[test]
fn test_parser_with_ttl_native_type_expr_record() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('{myNum: number, myObject} ') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprNullable
#[test]
fn test_parser_with_ttl_native_type_expr_nullable() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('?number') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprNonNullable
#[test]
fn test_parser_with_ttl_native_type_expr_non_nullable() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('!Object') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprFunction
#[test]
fn test_parser_with_ttl_native_type_expr_function() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('function(string, boolean)') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprFunctionReturn
#[test]
fn test_parser_with_ttl_native_type_expr_function_return() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('function(): number') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprFunctionThis
#[test]
fn test_parser_with_ttl_native_type_expr_function_this() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('function(this:goog.ui.Menu, string)') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprFunctionNew
#[test]
fn test_parser_with_ttl_native_type_expr_function_new() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('function(new:goog.ui.Menu, string)') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprFunctionVarargs
#[test]
fn test_parser_with_ttl_native_type_expr_function_varargs() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('function(string, ...number): number') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprFunctionOptional
#[test]
fn test_parser_with_ttl_native_type_expr_function_optional() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('function(?string=, number=)') =: */"),
        &[],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprMissingParam
#[test]
fn test_parser_with_ttl_native_type_expr_missing_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr() =: */"),
        &[JsString::from(
            "Bad type annotation. Missing parameter in typeExpr",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeTypeExprExtraParam
#[test]
fn test_parser_with_ttl_native_type_expr_extra_param() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr('a', 'b') =: */"),
        &[JsString::from(
            "Bad type annotation. Found extra parameter in typeExpr",
        )],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLNativeInvalidTypeExpr
#[test]
fn test_parser_with_ttl_native_invalid_type_expr() {
    let mut f = set_up();
    f.parse(
        JsString::from("@template T := typeExpr(foo) =: */"),
        &[
            JsString::from("Bad type annotation. Invalid native type expression"),
            JsString::from("Bad type annotation. Invalid expression inside typeExpr"),
        ],
    );
}

// port: JsDocInfoParserTest#testParserWithTTLAsynchUseCase
#[test]
fn test_parser_with_ttl_asynch_use_case() {
    let mut f = set_up();
    f.parse(JsString::from("@template R := cond(eq(T, 'Object'),\nmaprecord(T,\n(K, V) => cond(eq(rawTypeOf(V), 'Promise'),\ntemplateTypeOf(V, 0),\n'undefined')\n),\nT)\n=: */\n"), &[]);
}

// port: JsDocInfoParserTest#testJsDocInfoPosition
#[test]
fn test_js_doc_info_position() {
    let mut f = set_up();
    let mut source_file = SourceFile::from_code(
        &JsString::from("comment-position-test.js").to_string_lossy(),
        JsString::from("   \n  /**\n   * A comment\n   */\n  function f(x) {}\n"),
    );
    let mut script = f.parse_full(source_file.get_code().unwrap(), &[]);
    assert!((script).is_script(&f.ast));
    let mut r#fn = (script).get_first_child(&f.ast);
    assert!(((r#fn).unwrap()).is_function(&f.ast));
    let mut jsdoc = ((r#fn).unwrap()).get_jsdoc_info(&f.ast);
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_original_comment_position(),
        6
    );
    assert_eq!(
        source_file.get_line_of_offset(((jsdoc.clone()).unwrap()).get_original_comment_position()),
        2
    );
    assert_eq!(
        source_file
            .get_column_of_offset(((jsdoc.clone()).unwrap()).get_original_comment_position()),
        2
    );
}

// port: JsDocInfoParserTest#testParseCommentWithStarsOnOpenCommentLine
#[test]
fn test_parse_comment_with_stars_on_open_comment_line() {
    let mut f = set_up();
    let mut script = f.parse_full(JsString::from("/******\n * This is a typedef comment with ASCII art.\n * @desc this is a description\n *****/\nfunction x() {}\n"), &[]);
    assert!((script).is_script(&f.ast));
    let mut r#fn = (script).get_first_child(&f.ast);
    assert!(((r#fn).unwrap()).is_function(&f.ast));
    let mut info = ((r#fn).unwrap()).get_jsdoc_info(&f.ast);
    assert_eq!(
        ((info.clone()).unwrap()).get_block_description(),
        Some(JsString::from(
            "****\nThis is a typedef comment with ASCII art."
        ))
    );
    assert_eq!(
        ((info.clone()).unwrap()).get_description(),
        Some(JsString::from("this is a description ***"))
    );
}

// port: JsDocInfoParserTest#testParseTypeViaStatic1
#[test]
fn test_parse_type_via_static1() {
    let mut f = set_up();
    let mut type_node = f.parse_type(JsString::from("null"));
    {
        let arg0 = f.types.null_type;
        let arg1 = (type_node).unwrap();
        f.types.assert_type_equals_node(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseTypeViaStatic2
#[test]
fn test_parse_type_via_static2() {
    let mut f = set_up();
    let mut type_node = f.parse_type(JsString::from("string"));
    {
        let arg0 = f.types.string_type;
        let arg1 = (type_node).unwrap();
        f.types.assert_type_equals_node(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseTypeViaStatic3
#[test]
fn test_parse_type_via_static3() {
    let mut f = set_up();
    let mut type_node = f.parse_type(JsString::from("!Date"));
    {
        let arg0 = f.types.date_type;
        let arg1 = (type_node).unwrap();
        f.types.assert_type_equals_node(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseTypeViaStatic4
#[test]
fn test_parse_type_via_static4() {
    let mut f = set_up();
    let mut type_node = f.parse_type(JsString::from("boolean|string"));
    {
        let arg0 = {
            let arg0 = f.types.boolean_type;
            let arg1 = f.types.string_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (type_node).unwrap();
        f.types.assert_type_equals_node(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseNamedType2
#[test]
fn test_parse_named_type2() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@type null*/"),
        &[
            (JsString::from("Bad type annotation. Type annotations should have curly braces."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    {
        let arg0 = f.types.null_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNamedType3
#[test]
fn test_parse_named_type3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {string}*/"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNamedType4
#[test]
fn test_parse_named_type4() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type \n {string}*/"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNamedType5
#[test]
fn test_parse_named_type5() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!goog.\nBar}*/"), &[]);
    {
        let arg0 = {
            let arg0 = JsString::from("goog.Bar");
            f.create_named_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNamedType6
#[test]
fn test_parse_named_type6() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!goog.\n * Bar.\n * Baz}*/"), &[]);
    {
        let arg0 = {
            let arg0 = JsString::from("goog.Bar.Baz");
            f.create_named_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testTypedefType1
#[test]
fn test_typedef_type1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@typedef {string} */"), &[]);
    assert!((((info.clone()).unwrap()).has_typedef_type()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_typedef_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testTypedefType2
#[test]
fn test_typedef_type2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@typedef \n {string}*/"), &[]);
    assert!((((info.clone()).unwrap()).has_typedef_type()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_typedef_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testTypedefType3
#[test]
fn test_typedef_type3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@typedef \n {(string|number)}*/"), &[]);
    assert!((((info.clone()).unwrap()).has_typedef_type()));
    {
        let arg0 = {
            let arg0 = f.types.number_type;
            let arg1 = f.types.string_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_typedef_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testTypedefType4
#[test]
fn test_typedef_type4() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@typedef {{\n *  boo: ?,\n *  goo: ?\n * }}\n */\n"),
        &[],
    );
    assert!((((info.clone()).unwrap()).has_typedef_type()));
    let mut record_type = ((({ f.types.create_record_type_builder() }).add_property(
        JsString::from("boo"),
        f.types.unknown_type,
        None,
    ))
    .add_property(JsString::from("goo"), f.types.unknown_type, None))
    .build(&mut f.types.reg, &f.types.ast);
    {
        let arg0 = record_type;
        let arg1 = (((info.clone()).unwrap()).get_typedef_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseStringType1
#[test]
fn test_parse_string_type1() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_type;
        let arg1 =
            (((f.parse(JsString::from("@type {string}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseStringType2
#[test]
fn test_parse_string_type2() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_object_type;
        let arg1 =
            (((f.parse(JsString::from("@type {!String}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseBooleanType1
#[test]
fn test_parse_boolean_type1() {
    let mut f = set_up();
    {
        let arg0 = f.types.boolean_type;
        let arg1 =
            (((f.parse(JsString::from("@type {boolean}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseBooleanType2
#[test]
fn test_parse_boolean_type2() {
    let mut f = set_up();
    {
        let arg0 = f.types.boolean_object_type;
        let arg1 =
            (((f.parse(JsString::from("@type {!Boolean}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNumberType1
#[test]
fn test_parse_number_type1() {
    let mut f = set_up();
    {
        let arg0 = f.types.number_type;
        let arg1 =
            (((f.parse(JsString::from("@type {number}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNumberType2
#[test]
fn test_parse_number_type2() {
    let mut f = set_up();
    {
        let arg0 = f.types.number_object_type;
        let arg1 =
            (((f.parse(JsString::from("@type {!Number}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNullType1
#[test]
fn test_parse_null_type1() {
    let mut f = set_up();
    {
        let arg0 = f.types.null_type;
        let arg1 =
            (((f.parse(JsString::from("@type {null}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNullType2
#[test]
fn test_parse_null_type2() {
    let mut f = set_up();
    {
        let arg0 = f.types.null_type;
        let arg1 =
            (((f.parse(JsString::from("@type {Null}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseAllType1
#[test]
fn test_parse_all_type1() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("*");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseAllType2
#[test]
fn test_parse_all_type2() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("*?");
        let arg1 = JsString::from("*");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseObjectType
#[test]
fn test_parse_object_type() {
    let mut f = set_up();
    {
        let arg0 = f.types.object_type;
        let arg1 =
            (((f.parse(JsString::from("@type {!Object}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseDateType
#[test]
fn test_parse_date_type() {
    let mut f = set_up();
    {
        let arg0 = f.types.date_type;
        let arg1 =
            (((f.parse(JsString::from("@type {!Date}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionType
#[test]
fn test_parse_function_type() {
    let mut f = set_up();
    {
        let arg0 = {
            let arg0 = f.types.function_type;
            f.types.create_nullable_type(arg0)
        };
        let arg1 =
            (((f.parse(JsString::from("@type {Function}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseRegExpType
#[test]
fn test_parse_reg_exp_type() {
    let mut f = set_up();
    {
        let arg0 = f.types.regexp_type;
        let arg1 =
            (((f.parse(JsString::from("@type {!RegExp}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUndefinedType1
#[test]
fn test_parse_undefined_type1() {
    let mut f = set_up();
    {
        let arg0 = f.types.void_type;
        let arg1 =
            (((f.parse(JsString::from("@type {undefined}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUndefinedType2
#[test]
fn test_parse_undefined_type2() {
    let mut f = set_up();
    {
        let arg0 = f.types.void_type;
        let arg1 =
            (((f.parse(JsString::from("@type {Undefined}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUndefinedType3
#[test]
fn test_parse_undefined_type3() {
    let mut f = set_up();
    {
        let arg0 = f.types.void_type;
        let arg1 =
            (((f.parse(JsString::from("@type {void}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedTypeAlternateSyntax
#[test]
fn test_parse_templatized_type_alternate_syntax() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<number>} */"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = f.types.number_type;
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType1
#[test]
fn test_parse_templatized_type1() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@type !Array<number> */"),
        &[
            (JsString::from("Bad type annotation. Type annotations should have curly braces."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = f.types.number_type;
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType2
#[test]
fn test_parse_templatized_type2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<number>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = f.types.number_type;
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType4
#[test]
fn test_parse_templatized_type4() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<(number|null)>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.number_type;
                let arg1 = f.types.null_type;
                f.types.create_union_type(&[arg0, arg1])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType5
#[test]
fn test_parse_templatized_type5() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@type {!Array<Array<(number|null)>>}*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.null_type;
                let arg1 = {
                    let arg0 = f.types.array_type;
                    let arg1 = {
                        let arg0 = f.types.number_type;
                        let arg1 = f.types.null_type;
                        f.types.create_union_type(&[arg0, arg1])
                    };
                    f.types.create_templatized_type(arg0, &[arg1])
                };
                f.types.create_union_type(&[arg0, arg1])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType6
#[test]
fn test_parse_templatized_type6() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@type {!Array<!Array<(number|null)>>}*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.array_type;
                let arg1 = {
                    let arg0 = f.types.number_type;
                    let arg1 = f.types.null_type;
                    f.types.create_union_type(&[arg0, arg1])
                };
                f.types.create_templatized_type(arg0, &[arg1])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType7
#[test]
fn test_parse_templatized_type7() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<function():Date>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = {
                    let arg0 = f.types.date_type;
                    let arg1 = f.types.null_type;
                    f.types.create_union_type(&[arg0, arg1])
                };
                f.types.reg.create_function_type(&f.types.ast, arg0, &[])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType8
#[test]
fn test_parse_templatized_type8() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<function():!Date>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.date_type;
                f.types.reg.create_function_type(&f.types.ast, arg0, &[])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType9
#[test]
fn test_parse_templatized_type9() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<Date|number>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.date_type;
                let arg1 = f.types.number_type;
                let arg2 = f.types.null_type;
                f.types.create_union_type(&[arg0, arg1, arg2])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType10
#[test]
fn test_parse_templatized_type10() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<Date|number|boolean>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.date_type;
                let arg1 = f.types.number_type;
                let arg2 = f.types.boolean_type;
                let arg3 = f.types.null_type;
                f.types.create_union_type(&[arg0, arg1, arg2, arg3])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType11
#[test]
fn test_parse_templatized_type11() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Object<number>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.object_type;
            let arg1 = vec![f.types.unknown_type, f.types.number_type];
            f.types.create_templatized_type(arg0, &arg1)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = f.types.reg.get_object_element_key();
        let arg1 = f.types.number_type;
        let arg2 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.assert_templatized_type_equals(arg0, arg1, &arg2)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType12
#[test]
fn test_parse_templatized_type12() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Object<string,number>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.object_type;
            let arg1 = vec![f.types.string_type, f.types.number_type];
            f.types.create_templatized_type(arg0, &arg1)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = f.types.reg.get_object_element_key();
        let arg1 = f.types.number_type;
        let arg2 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.assert_templatized_type_equals(arg0, arg1, &arg2)
    };
    {
        let arg0 = f.types.reg.get_object_index_key();
        let arg1 = f.types.string_type;
        let arg2 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.assert_templatized_type_equals(arg0, arg1, &arg2)
    };
}

// port: JsDocInfoParserTest#testParseTemplatizedType13
#[test]
fn test_parse_templatized_type13() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<?>} */"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = f.types.unknown_type;
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType2
#[test]
fn test_parse_union_type2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {boolean|null}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.boolean_type;
            let arg1 = f.types.null_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType6
#[test]
fn test_parse_union_type6() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {Array<boolean>|null}*/"), &[]);
    {
        let arg0 = {
            let arg0 = {
                let arg0 = f.types.array_type;
                let arg1 = f.types.boolean_type;
                f.types.create_templatized_type(arg0, &[arg1])
            };
            let arg1 = f.types.null_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType7
#[test]
fn test_parse_union_type7() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {null|Array<boolean>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = {
                let arg0 = f.types.array_type;
                let arg1 = f.types.boolean_type;
                f.types.create_templatized_type(arg0, &[arg1])
            };
            let arg1 = f.types.null_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType13
#[test]
fn test_parse_union_type13() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("(function(this:Date)|function(this:String):number)");
        let arg1 = JsString::from("Function");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType14
#[test]
fn test_parse_union_type14() {
    let mut f = set_up();
    {
        let arg0 = JsString::from(
            "(function(...(function(number):boolean)):number)|function(this:String, string):number",
        );
        let arg1 = JsString::from("Function");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType15
#[test]
fn test_parse_union_type15() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("*|number");
        let arg1 = JsString::from("*");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType16
#[test]
fn test_parse_union_type16() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("number|*");
        let arg1 = JsString::from("*");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType17
#[test]
fn test_parse_union_type17() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("string|number|*");
        let arg1 = JsString::from("*");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType19
#[test]
fn test_parse_union_type19() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {(?)} */"), &[]);
    {
        let arg0 = f.types.unknown_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnionType_functionReturn_threeAlts
#[test]
fn test_parse_union_type_function_return_three_alts() {
    let mut f = set_up();
    let mut r#type = {
        let arg0 = JsString::from("function(): string|number|boolean");
        let arg1 = JsString::from("(boolean|function(): string|number)");
        f.test_parse_type(arg0, arg1)
    };
    TypeSubject::assert_type(r#type).is_union_type(&f.types.reg);
    let mut alts = (((r#type).to_maybe_union_type(&f.types.reg)).unwrap())
        .get_alternates(&mut f.types.reg, &f.types.ast)
        .as_ref()
        .clone();
    assert_eq!((alts).len(), 3usize);
    TypeSubject::assert_type((alts)[0usize])
        .is_function_type_that(&f.types.reg)
        .has_return_type_that(&f.types.reg)
        .is_string(&f.types.reg);
    TypeSubject::assert_type((alts)[1usize]).is_number(&f.types.reg);
    TypeSubject::assert_type((alts)[2usize]).is_boolean(&f.types.reg);
}

// port: JsDocInfoParserTest#testParseUnknownType1
#[test]
fn test_parse_unknown_type1() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("?");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseUnknownType2
#[test]
fn test_parse_unknown_type2() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("(?|number)");
        let arg1 = JsString::from("?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnknownType3
#[test]
fn test_parse_unknown_type3() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("(number|?)");
        let arg1 = JsString::from("?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnknownType_undefined
#[test]
fn test_parse_unknown_type_undefined() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("(undefined|?)");
        let arg1 = JsString::from("(?|undefined)");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseUnknownType_null
#[test]
fn test_parse_unknown_type_null() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("(null|?)");
        let arg1 = JsString::from("?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType1
#[test]
fn test_parse_functional_type1() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(): number");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType2
#[test]
fn test_parse_functional_type2() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(number, string): boolean");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType3
#[test]
fn test_parse_functional_type3() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(this:Array)");
        let arg1 = JsString::from("function(this:Array): ?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType4
#[test]
fn test_parse_functional_type4() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(...number): boolean");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType5
#[test]
fn test_parse_functional_type5() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(number, ...string): boolean");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType6
#[test]
fn test_parse_functional_type6() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(this:Date, number): (boolean|number|string)");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType7
#[test]
fn test_parse_functional_type7() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function()");
        let arg1 = JsString::from("function(): ?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType9
#[test]
fn test_parse_functional_type9() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(this:Array,!Date,...(boolean?))");
        let arg1 = JsString::from("function(this:Array, Date, ...(boolean|null)): ?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType10
#[test]
fn test_parse_functional_type10() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(...(Object?)):boolean?");
        let arg1 = JsString::from("function(...(Object|null)): (boolean|null)");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType12
#[test]
fn test_parse_functional_type12() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(...)");
        let arg1 = JsString::from("function(...?): ?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType13
#[test]
fn test_parse_functional_type13() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(...): void");
        let arg1 = JsString::from("function(...?): undefined");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType14
#[test]
fn test_parse_functional_type14() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(*, string, number): boolean");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType15
#[test]
fn test_parse_functional_type15() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(?, string): boolean");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType16
#[test]
fn test_parse_functional_type16() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(string, ?): ?");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType17
#[test]
fn test_parse_functional_type17() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("(function(?): ?|number)");
        f.test_parse_type(arg0.clone(), arg0)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType18
#[test]
fn test_parse_functional_type18() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function (?): (?|number)");
        let arg1 = JsString::from("function(?): ?");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testParseFunctionalType19
#[test]
fn test_parse_functional_type19() {
    let mut f = set_up();
    {
        let arg0 = JsString::from("function(...?): void");
        let arg1 = JsString::from("function(...?): undefined");
        f.test_parse_type(arg0, arg1)
    };
}

// port: JsDocInfoParserTest#testStructuralConstructor
#[test]
fn test_structural_constructor() {
    let mut f = set_up();
    let mut r#type = {
        let arg0 = JsString::from("function (new:Object)");
        let arg1 = JsString::from("function(new:Object): ?");
        f.test_parse_type(arg0, arg1)
    };
    assert!(((r#type).is_constructor(&f.types.reg)));
    assert!(!((r#type).is_nominal_constructor_or_interface(&f.types.reg)));
}

// port: JsDocInfoParserTest#testStructuralConstructor2
#[test]
fn test_structural_constructor2() {
    let mut f = set_up();
    let mut r#type = {
        let arg0 = JsString::from("function (new:?)");
        let arg1 = JsString::from("function(): ?");
        f.test_parse_type(arg0, arg1)
    };
    assert!(((r#type).is_constructor(&f.types.reg)));
    assert!(!((r#type).is_nominal_constructor_or_interface(&f.types.reg)));
}

// port: JsDocInfoParserTest#testStructuralConstructor3
#[test]
fn test_structural_constructor3() {
    let mut f = set_up();
    {
        let arg0 = (((f.parse(JsString::from("@type {function (new:*)} */"), &[])).unwrap())
            .get_type())
        .unwrap();
        let arg1 = JsString::from("constructed type must be an object type");
        f.types.resolve(&arg0, &[&arg1.to_string_lossy()])
    };
}

// port: JsDocInfoParserTest#testNominalConstructor
#[test]
fn test_nominal_constructor() {
    let mut f = set_up();
    let mut r#type = ({
        let arg0 = JsString::from("Array");
        let arg1 = JsString::from("(Array|null)");
        f.test_parse_type(arg0, arg1)
    })
    .dereference(&mut f.types.reg, &f.types.ast);
    assert!(
        (((((r#type).unwrap()).get_constructor(&f.types.reg)).unwrap())
            .is_nominal_constructor_or_interface(&f.types.reg))
    );
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError6
#[test]
fn test_parse_functional_type_error6() {
    let mut f = set_up();
    {
        let arg0 = (((f.parse(JsString::from("@type {function (this:number)}*/"), &[])).unwrap())
            .get_type())
        .unwrap();
        f.types.resolve(&arg0, &[])
    };
}

// port: JsDocInfoParserTest#testParseFunctionalTypeError12
#[test]
fn test_parse_functional_type_error12() {
    let mut f = set_up();
    {
        let arg0 = (((f.parse(JsString::from("@type {function (new:number)}*/"), &[])).unwrap())
            .get_type())
        .unwrap();
        let arg1 = JsString::from("constructed type must be an object type");
        f.types.resolve(&arg0, &[&arg1.to_string_lossy()])
    };
}

// port: JsDocInfoParserTest#testParseNullableModifiers1
#[test]
fn test_parse_nullable_modifiers1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {string?}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.string_type;
            f.types.create_nullable_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNullableModifiers2
#[test]
fn test_parse_nullable_modifiers2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<string?>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.string_type;
                let arg1 = f.types.null_type;
                f.types.create_union_type(&[arg0, arg1])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNullableModifiers3
#[test]
fn test_parse_nullable_modifiers3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {Array<boolean>?}*/"), &[]);
    {
        let arg0 = {
            let arg0 = {
                let arg0 = f.types.array_type;
                let arg1 = f.types.boolean_type;
                f.types.create_templatized_type(arg0, &[arg1])
            };
            f.types.create_nullable_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNullableModifiers7
#[test]
fn test_parse_nullable_modifiers7() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {string?|boolean}*/"), &[]);
    {
        let arg0 = {
            let arg0 = {
                let arg0 = f.types.string_type;
                f.types.create_nullable_type(arg0)
            };
            let arg1 = f.types.boolean_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNullableModifiers8
#[test]
fn test_parse_nullable_modifiers8() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {string|boolean?}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.string_type;
            let arg1 = {
                let arg0 = f.types.boolean_type;
                f.types.create_nullable_type(arg0)
            };
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNullableModifiers9
#[test]
fn test_parse_nullable_modifiers9() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {foo.Hello.World?}*/"), &[]);
    {
        let arg0 = {
            let arg0 = {
                let arg0 = JsString::from("foo.Hello.World");
                f.create_named_type(arg0)
            };
            f.types.create_nullable_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseOptionalModifier
#[test]
fn test_parse_optional_modifier() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {function(number=)}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.unknown_type;
            let arg1 = {
                let arg0 = f.types.number_type;
                f.types
                    .reg
                    .create_optional_parameters(&f.types.ast, &[arg0])
            };
            f.types
                .reg
                .create_function_type_with_parameters(&f.types.ast, arg0, arg1)
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNewline1
#[test]
fn test_parse_newline1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {string\n* }\n*/"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNewline2
#[test]
fn test_parse_newline2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<\n* number\n* >} */"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = f.types.number_type;
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNewline4
#[test]
fn test_parse_newline4() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {!Array<(number|\n* null)>}*/"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = f.types.number_type;
                let arg1 = f.types.null_type;
                f.types.create_union_type(&[arg0, arg1])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseNewline5
#[test]
fn test_parse_newline5() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@type {!Array<function(\n* )\n* :\n* Date>}*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = f.types.array_type;
            let arg1 = {
                let arg0 = {
                    let arg0 = f.types.date_type;
                    let arg1 = f.types.null_type;
                    f.types.create_union_type(&[arg0, arg1])
                };
                f.types.reg.create_function_type(&f.types.ast, arg0, &[])
            };
            f.types.create_templatized_type(arg0, &[arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseReturnType1
#[test]
fn test_parse_return_type1() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@return {null|string|Array<boolean>}*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = {
                let arg0 = f.types.array_type;
                let arg1 = f.types.boolean_type;
                f.types.create_templatized_type(arg0, &[arg1])
            };
            let arg1 = f.types.null_type;
            let arg2 = f.types.string_type;
            f.types.create_union_type(&[arg0, arg1, arg2])
        };
        let arg1 = (((info.clone()).unwrap()).get_return_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseReturnType2
#[test]
fn test_parse_return_type2() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@returns {null|(string|Array<boolean>)}*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = {
                let arg0 = f.types.array_type;
                let arg1 = f.types.boolean_type;
                f.types.create_templatized_type(arg0, &[arg1])
            };
            let arg1 = f.types.null_type;
            let arg2 = f.types.string_type;
            f.types.create_union_type(&[arg0, arg1, arg2])
        };
        let arg1 = (((info.clone()).unwrap()).get_return_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseReturnType3
#[test]
fn test_parse_return_type3() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@return {((null|Array<boolean>|string)|boolean)}*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = {
                let arg0 = f.types.array_type;
                let arg1 = f.types.boolean_type;
                f.types.create_templatized_type(arg0, &[arg1])
            };
            let arg1 = f.types.null_type;
            let arg2 = f.types.string_type;
            let arg3 = f.types.boolean_type;
            f.types.create_union_type(&[arg0, arg1, arg2, arg3])
        };
        let arg1 = (((info.clone()).unwrap()).get_return_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseThisType1
#[test]
fn test_parse_this_type1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@this {goog.foo.Bar}*/"), &[]);
    {
        let arg0 = {
            let arg0 = JsString::from("goog.foo.Bar");
            f.create_named_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_this_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseThisType2
#[test]
fn test_parse_this_type2() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@this goog.foo.Bar*/"),
        &[
            (JsString::from("Bad type annotation. Type annotations should have curly braces."))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
        ],
    );
    {
        let arg0 = {
            let arg0 = JsString::from("goog.foo.Bar");
            f.create_named_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_this_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseThisType4
#[test]
fn test_parse_this_type4() {
    let mut f = set_up();
    {
        let arg0 = (((f.parse(JsString::from("@this {number}*/"), &[])).unwrap()).get_this_type())
            .unwrap();
        f.types.resolve(&arg0, &[])
    };
}

// port: JsDocInfoParserTest#testParseThisType6
#[test]
fn test_parse_this_type6() {
    let mut f = set_up();
    {
        let arg0 = (((f.parse(JsString::from("@this {Date|number}*/"), &[])).unwrap())
            .get_this_type())
        .unwrap();
        f.types.resolve(&arg0, &[])
    };
}

// port: JsDocInfoParserTest#testParseParam1
#[test]
fn test_parse_param1() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {number} index*/"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam3
#[test]
fn test_parse_param3() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {number} index useful comments*/"),
        &[],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam5
#[test]
fn test_parse_param5() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {number} \n index */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam6
#[test]
fn test_parse_param6() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {number} \n * index */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam7
#[test]
fn test_parse_param7() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {number=} index */"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.number_type;
            f.types.create_optional_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam8
#[test]
fn test_parse_param8() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {...number} index */"), &[]);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam12
#[test]
fn test_parse_param12() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {...number|string} index */"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.string_type;
            let arg1 = f.types.number_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam13
#[test]
fn test_parse_param13() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {...(number|string)} index */"), &[]);
    {
        let arg0 = {
            let arg0 = f.types.string_type;
            let arg1 = f.types.number_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam14
#[test]
fn test_parse_param14() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {string} [index] */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = {
            let arg0 = f.types.string_type;
            f.types.create_optional_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam15
#[test]
fn test_parse_param15() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {string} [index */"),
        &[(JsString::from("Bad type annotation. missing closing ]"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam16
#[test]
fn test_parse_param16() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {string} index] */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam17
#[test]
fn test_parse_param17() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {string=} [index] */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = {
            let arg0 = f.types.string_type;
            f.types.create_optional_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam18
#[test]
fn test_parse_param18() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {...string} [index] */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam19
#[test]
fn test_parse_param19() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {...} [index] */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = {
            let arg0 = f.types.unknown_type;
            f.types.create_optional_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!(
        (((((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap())
            .is_var_args(&f.types.ast))
    );
}

// port: JsDocInfoParserTest#testParseParam20
#[test]
fn test_parse_param20() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {?=} index */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.unknown_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam21
#[test]
fn test_parse_param21() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {...?} index */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.unknown_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!(
        (((((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap())
            .is_var_args(&f.types.ast))
    );
}

// port: JsDocInfoParserTest#testParseParam25
#[test]
fn test_parse_param25() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@param {string} foo.bar\n * @param {string} baz */"),
        &[JsString::from("invalid param name \"foo.bar\"")],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("baz"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseParam28
#[test]
fn test_parse_param28() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@param {...} index */"), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = {
            let arg0 = f.types.unknown_type;
            f.types.create_optional_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!(
        (((((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap())
            .is_var_args(&f.types.ast))
    );
}

// port: JsDocInfoParserTest#testParseExtends1
#[test]
fn test_parse_extends1() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_object_type;
        let arg1 = (((f.parse(JsString::from("@extends String*/"), &[])).unwrap()).get_base_type())
            .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseExtends2
#[test]
fn test_parse_extends2() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@extends com.google.Foo.Bar.Hello.World*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = JsString::from("com.google.Foo.Bar.Hello.World");
            f.create_named_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_base_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseExtendsGenerics
#[test]
fn test_parse_extends_generics() {
    let mut f = set_up();
    let mut info = f.parse(
        JsString::from("@extends com.google.Foo.Bar.Hello.World<Boolean,number>*/"),
        &[],
    );
    {
        let arg0 = {
            let arg0 = JsString::from("com.google.Foo.Bar.Hello.World");
            f.create_named_type(arg0)
        };
        let arg1 = (((info.clone()).unwrap()).get_base_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseImplementsGenerics
#[test]
fn test_parse_implements_generics() {
    let mut f = set_up();
    let mut interfaces = ((f.parse(JsString::from("@implements {SomeInterface<*>} */"), &[]))
        .unwrap())
    .get_implemented_interfaces();
    assert_eq!((interfaces).len(), 1usize);
    {
        let arg0 = {
            let arg0 = JsString::from("SomeInterface");
            f.create_named_type(arg0)
        };
        let arg1 = (interfaces)[0usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseExtends4
#[test]
fn test_parse_extends4() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_object_type;
        let arg1 = (((f.parse(JsString::from("@extends {String}*/"), &[])).unwrap())
            .get_base_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseExtends5
#[test]
fn test_parse_extends5() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_object_type;
        let arg1 = (((f.parse(
            JsString::from("@extends {String*/"),
            &[(JsString::from("Bad type annotation. expected closing }"))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
        ))
        .unwrap())
        .get_base_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseExtends6
#[test]
fn test_parse_extends6() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_object_type;
        let arg1 = (((f.parse(JsString::from("@extends \n * {String}*/"), &[])).unwrap())
            .get_base_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseExtendsInvalidName
#[test]
fn test_parse_extends_invalid_name() {
    let mut f = set_up();
    {
        let arg0 = {
            let arg0 = JsString::from("some_++#%$%_UglyString");
            f.create_named_type(arg0)
        };
        let arg1 = (((f.parse(JsString::from("@extends {some_++#%$%_UglyString} */"), &[]))
            .unwrap())
        .get_base_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseEnum1
#[test]
fn test_parse_enum1() {
    let mut f = set_up();
    {
        let arg0 = f.types.number_type;
        let arg1 = (((f.parse(JsString::from("@enum*/"), &[])).unwrap()).get_enum_parameter_type())
            .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseEnum2
#[test]
fn test_parse_enum2() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_type;
        let arg1 = (((f.parse(JsString::from("@enum {string}*/"), &[])).unwrap())
            .get_enum_parameter_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseEnum3
#[test]
fn test_parse_enum3() {
    let mut f = set_up();
    {
        let arg0 = f.types.number_type;
        let arg1 = (((f.parse(JsString::from("@enum {number}*/"), &[])).unwrap())
            .get_enum_parameter_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseEnum4
#[test]
fn test_parse_enum4() {
    let mut f = set_up();
    {
        let arg0 = f.types.boolean_type;
        let arg1 = (((f.parse(JsString::from("@enum {boolean}*/"), &[])).unwrap())
            .get_enum_parameter_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseEnum5
#[test]
fn test_parse_enum5() {
    let mut f = set_up();
    {
        let arg0 = f.types.symbol_type;
        let arg1 = (((f.parse(JsString::from("@enum {symbol}*/"), &[])).unwrap())
            .get_enum_parameter_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseEnum7
#[test]
fn test_parse_enum7() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_type;
        let arg1 = (((f.parse(
            JsString::from("@enum string*/"),
            &[
                (JsString::from("Bad type annotation. Type annotations should have curly braces."))
                    .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            ],
        ))
        .unwrap())
        .get_enum_parameter_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseBadEnumNoCrash
#[test]
fn test_parse_bad_enum_no_crash() {
    let mut f = set_up();
    {
        let arg0 = f.types.number_type;
        let arg1 = (((f.parse(
            JsString::from("@enum {@enum {string}}*/"),
            &[
                (JsString::from("Bad type annotation. type not recognized due to syntax error."))
                    .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
                (JsString::from(
                    "Bad type annotation. type annotation incompatible with other annotations.",
                ))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK))),
            ],
        ))
        .unwrap())
        .get_enum_parameter_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testStackedAnnotation
#[test]
fn test_stacked_annotation() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@const @type {string}*/"), &[]);
    assert!((((info.clone()).unwrap()).is_constant()));
    assert!((((info.clone()).unwrap()).has_type()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testStackedAnnotation2
#[test]
fn test_stacked_annotation2() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@type {string} @const */"), &[]);
    assert!((((info.clone()).unwrap()).is_constant()));
    assert!((((info.clone()).unwrap()).has_type()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testStackedAnnotation9
#[test]
fn test_stacked_annotation9() {
    let mut f = set_up();
    let mut info =
        f.parse_with_documentation(JsString::from("@const @private {string} */"), true, &[]);
    assert!((((info.clone()).unwrap()).is_constant()));
    assert!((((info.clone()).unwrap()).has_type()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testStackedAnnotation10
#[test]
fn test_stacked_annotation10() {
    let mut f = set_up();
    let mut info =
        f.parse_with_documentation(JsString::from("@private @const {string} */"), true, &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PRIVATE
    );
    assert!((((info.clone()).unwrap()).has_type()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!((((info.clone()).unwrap()).is_constant()));
}

// port: JsDocInfoParserTest#testParseDefine1
#[test]
fn test_parse_define1() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_type;
        let arg1 =
            (((f.parse(JsString::from("@define {string}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseDefine2
#[test]
fn test_parse_define2() {
    let mut f = set_up();
    {
        let arg0 = f.types.string_type;
        let arg1 = (((f.parse(
            JsString::from("@define {string*/"),
            &[(JsString::from("Bad type annotation. expected closing }"))
                .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
        ))
        .unwrap())
        .get_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseDefine3
#[test]
fn test_parse_define3() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@define {boolean}*/"), &[]);
    assert!((((info.clone()).unwrap()).is_constant()));
    assert!((((info.clone()).unwrap()).is_define()));
    {
        let arg0 = f.types.boolean_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseDefine4
#[test]
fn test_parse_define4() {
    let mut f = set_up();
    {
        let arg0 = f.types.number_type;
        let arg1 =
            (((f.parse(JsString::from("@define {number}*/"), &[])).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseDefine5
#[test]
fn test_parse_define5() {
    let mut f = set_up();
    {
        let arg0 = {
            let arg0 = f.types.number_type;
            let arg1 = f.types.boolean_type;
            f.types.create_union_type(&[arg0, arg1])
        };
        let arg1 = (((f.parse(JsString::from("@define {number|boolean}*/"), &[])).unwrap())
            .get_type())
        .unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testRegression1
#[test]
fn test_regression1() {
    let mut f = set_up();
    let mut comment = JsString::from(
        "* @param {number} index the index of blah\n* @return {boolean} whatever\n* @private\n*/\n",
    );
    let mut info = f.parse(comment.clone(), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = f.types.boolean_type;
        let arg1 = (((info.clone()).unwrap()).get_return_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PRIVATE
    );
}

// port: JsDocInfoParserTest#testRegression2
#[test]
fn test_regression2() {
    let mut f = set_up();
    let mut comment = JsString::from(
        " * @return {boolean} whatever\n * but important\n *\n * @param {number} index the index of blah\n * some more comments here\n * @param name the name of the person\n *\n * @protected\n */\n",
    );
    let mut info = f.parse(
        comment.clone(),
        &[JsString::from(MISSING_TYPE_DECL_WARNING_TEXT)],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 2);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!(
        ((info.clone()).unwrap())
            .get_parameter_type(JsString::from("name"))
            .is_none()
    );
    {
        let arg0 = f.types.boolean_type;
        let arg1 = (((info.clone()).unwrap()).get_return_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PROTECTED
    );
}

// port: JsDocInfoParserTest#testRegression5
#[test]
fn test_regression5() {
    let mut f = set_up();
    let mut comment = JsString::from("@const\n@enum {string}\n@public*/");
    let mut info = f.parse(comment.clone(), &[]);
    assert!((((info.clone()).unwrap()).is_constant()));
    assert!(!(((info.clone()).unwrap()).is_define()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_enum_parameter_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(
        ((info.clone()).unwrap()).get_visibility(),
        Visibility::PUBLIC
    );
}

// port: JsDocInfoParserTest#testRegression7
#[test]
fn test_regression7() {
    let mut f = set_up();
    let mut comment = JsString::from(
        "* @desc description here\n* @param {boolean} flag and some more description\n*     nicely formatted\n*/\n",
    );
    let mut info = f.parse(comment.clone(), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.boolean_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("flag"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(
        ((info.clone()).unwrap()).get_description(),
        Some(JsString::from("description here"))
    );
}

// port: JsDocInfoParserTest#testRegression8
#[test]
fn test_regression8() {
    let mut f = set_up();
    let mut comment = JsString::from(
        " * @name random tag here\n * @desc description here\n *\n * @param {boolean} flag and some more description\n *     nicely formatted\n */\n",
    );
    let mut info = f.parse(comment.clone(), &[]);
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.boolean_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("flag"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(
        ((info.clone()).unwrap()).get_description(),
        Some(JsString::from("description here"))
    );
}

// port: JsDocInfoParserTest#testRegression9
#[test]
fn test_regression9() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("* @param {string} p0 blah blah blah\n*/\n"),
        &[],
    );
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert!(!(((jsdoc.clone()).unwrap()).is_constant()));
    assert!(((jsdoc.clone()).unwrap()).get_description().is_none());
    assert!(
        ((jsdoc.clone()).unwrap())
            .get_enum_parameter_type()
            .is_none()
    );
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_parameter_type(JsString::from("p0"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!(((jsdoc.clone()).unwrap()).get_return_type().is_none());
    assert!(((jsdoc.clone()).unwrap()).get_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::INHERITED
    );
}

// port: JsDocInfoParserTest#testRegression10
#[test]
fn test_regression10() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("* @param {!String} p0 blah blah blah\n* @param {boolean} p1 fobar\n* @return {!Date} jksjkash dshad\n*/\n"), &[]);
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert!(!(((jsdoc.clone()).unwrap()).is_constant()));
    assert!(((jsdoc.clone()).unwrap()).get_description().is_none());
    assert!(
        ((jsdoc.clone()).unwrap())
            .get_enum_parameter_type()
            .is_none()
    );
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 2);
    {
        let arg0 = f.types.string_object_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_parameter_type(JsString::from("p0"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = f.types.boolean_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_parameter_type(JsString::from("p1"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = f.types.date_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_return_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!(((jsdoc.clone()).unwrap()).get_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::INHERITED
    );
}

// port: JsDocInfoParserTest#testRegression12
#[test]
fn test_regression12() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("* @extends FooBar\n*/\n"), &[]);
    {
        let arg0 = {
            let arg0 = JsString::from("FooBar");
            f.create_named_type(arg0)
        };
        let arg1 = (((jsdoc.clone()).unwrap()).get_base_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!(!(((jsdoc.clone()).unwrap()).is_constant()));
    assert!(((jsdoc.clone()).unwrap()).get_description().is_none());
    assert!(
        ((jsdoc.clone()).unwrap())
            .get_enum_parameter_type()
            .is_none()
    );
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 0);
    assert!(((jsdoc.clone()).unwrap()).get_return_type().is_none());
    assert!(((jsdoc.clone()).unwrap()).get_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::INHERITED
    );
}

// port: JsDocInfoParserTest#testRegression13
#[test]
fn test_regression13() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("* @type {!RegExp}\n* @protected\n*/\n"), &[]);
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert!(!(((jsdoc.clone()).unwrap()).is_constant()));
    assert!(((jsdoc.clone()).unwrap()).get_description().is_none());
    assert!(
        ((jsdoc.clone()).unwrap())
            .get_enum_parameter_type()
            .is_none()
    );
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 0);
    assert!(((jsdoc.clone()).unwrap()).get_return_type().is_none());
    {
        let arg0 = f.types.regexp_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::PROTECTED
    );
}

// port: JsDocInfoParserTest#testRegression16
#[test]
fn test_regression16() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("Email is plp@foo.bar\n@type {string}\n*/\n"),
        &[],
    );
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert!(!(((jsdoc.clone()).unwrap()).is_constant()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(((jsdoc.clone()).unwrap()).get_parameter_count(), 0);
    assert!(((jsdoc.clone()).unwrap()).get_return_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_visibility(),
        Visibility::INHERITED
    );
}

// port: JsDocInfoParserTest#testParseImplements
#[test]
fn test_parse_implements() {
    let mut f = set_up();
    let mut interfaces = ((f.parse(JsString::from("@implements {SomeInterface}*/"), &[])).unwrap())
        .get_implemented_interfaces();
    assert_eq!((interfaces).len(), 1usize);
    {
        let arg0 = {
            let arg0 = JsString::from("SomeInterface");
            f.create_named_type(arg0)
        };
        let arg1 = (interfaces)[0usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testParseImplementsTwo
#[test]
fn test_parse_implements_two() {
    let mut f = set_up();
    let mut interfaces = ((f.parse(
        JsString::from("* @implements {SomeInterface1}\n* @implements {SomeInterface2}\n*/\n"),
        &[],
    ))
    .unwrap())
    .get_implemented_interfaces();
    assert_eq!((interfaces).len(), 2usize);
    {
        let arg0 = {
            let arg0 = JsString::from("SomeInterface1");
            f.create_named_type(arg0)
        };
        let arg1 = (interfaces)[0usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = {
            let arg0 = JsString::from("SomeInterface2");
            f.create_named_type(arg0)
        };
        let arg1 = (interfaces)[1usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testInterfaceExtends
#[test]
fn test_interface_extends() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("* @interface \n* @extends {Extended} */\n"),
        &[],
    );
    assert!((((jsdoc.clone()).unwrap()).is_interface()));
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_extended_interfaces_count(),
        1
    );
    let mut types = ((jsdoc.clone()).unwrap()).get_extended_interfaces();
    {
        let arg0 = {
            let arg0 = JsString::from("Extended");
            f.create_named_type(arg0)
        };
        let arg1 = (types)[0usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testInterfaceMultiExtends1
#[test]
fn test_interface_multi_extends1() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("* @interface \n* @extends {Extended1} \n* @extends {Extended2} */\n"),
        &[],
    );
    assert!((((jsdoc.clone()).unwrap()).is_interface()));
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_extended_interfaces_count(),
        2
    );
    let mut types = ((jsdoc.clone()).unwrap()).get_extended_interfaces();
    {
        let arg0 = {
            let arg0 = JsString::from("Extended1");
            f.create_named_type(arg0)
        };
        let arg1 = (types)[0usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = {
            let arg0 = JsString::from("Extended2");
            f.create_named_type(arg0)
        };
        let arg1 = (types)[1usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testInterfaceMultiExtends2
#[test]
fn test_interface_multi_extends2() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("* @extends {Extended1} \n* @interface \n* @extends {Extended2} \n* @extends {Extended3} */\n"), &[]);
    assert!((((jsdoc.clone()).unwrap()).is_interface()));
    assert!(((jsdoc.clone()).unwrap()).get_base_type().is_none());
    assert_eq!(
        ((jsdoc.clone()).unwrap()).get_extended_interfaces_count(),
        3
    );
    let mut types = ((jsdoc.clone()).unwrap()).get_extended_interfaces();
    {
        let arg0 = {
            let arg0 = JsString::from("Extended1");
            f.create_named_type(arg0)
        };
        let arg1 = (types)[0usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = {
            let arg0 = JsString::from("Extended2");
            f.create_named_type(arg0)
        };
        let arg1 = (types)[1usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    {
        let arg0 = {
            let arg0 = JsString::from("Extended3");
            f.create_named_type(arg0)
        };
        let arg1 = (types)[2usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testBadExtendsWithNullable
#[test]
fn test_bad_extends_with_nullable() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@constructor\n * @extends {Object?} */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
    assert!((((jsdoc.clone()).unwrap()).is_constructor()));
    {
        let arg0 = f.types.object_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_base_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testBadImplementsWithNullable
#[test]
fn test_bad_implements_with_nullable() {
    let mut f = set_up();
    let mut jsdoc = f.parse(
        JsString::from("@implements {Disposable?}\n * @constructor */"),
        &[(JsString::from("Bad type annotation. expected closing }"))
            .concat(&(JsString::from(BAD_TYPE_WIKI_LINK)))],
    );
    assert!((((jsdoc.clone()).unwrap()).is_constructor()));
    {
        let arg0 = {
            let arg0 = JsString::from("Disposable");
            f.create_named_type(arg0)
        };
        let arg1 = (((jsdoc.clone()).unwrap()).get_implemented_interfaces())[0usize].clone();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testJsDocAfterSuppress
#[test]
fn test_js_doc_after_suppress() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@suppress {x,y} @const {string} */"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testPackageType
#[test]
fn test_package_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("@package {string} */"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testPrivateType
#[test]
fn test_private_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("@private {string} */"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testProtectedType
#[test]
fn test_protected_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("@protected {string} */"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testPublicType
#[test]
fn test_public_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("@public {string} */"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testConstType
#[test]
fn test_const_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("@const {string} */"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testFinalType
#[test]
fn test_final_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse(JsString::from("@final {string} */"), &[]);
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testExportType
#[test]
fn test_export_type() {
    let mut f = set_up();
    let mut jsdoc = f.parse_with_documentation(
        JsString::from("@export {string} descr\n next line */"),
        true,
        &[],
    );
    {
        let arg0 = f.types.string_type;
        let arg1 = (((jsdoc.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert!((((jsdoc.clone()).unwrap()).is_export()));
    let mut define_marker = ((((jsdoc.clone()).unwrap()).get_markers()).into_iter()).next();
    assert_eq!(
        ((((define_marker.clone()).unwrap()).get_annotation().cloned()).unwrap())
            .get_item()
            .cloned(),
        Some(JsString::from("export"))
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("descr")))
            >= 0
    );
    assert!(
        ((((((define_marker.clone()).unwrap())
            .get_description()
            .cloned())
        .unwrap())
        .get_item()
        .cloned())
        .unwrap())
        .index_of(&(JsString::from("next line")))
            >= 0
    );
}

// port: JsDocInfoParserTest#testAllowlistedConflictingAnnotation
#[test]
fn test_allow_listed_conflicting_annotation() {
    let mut f = set_up();
    f.extra_annotations.insert(JsString::from("param"));
    let mut info = f.parse(JsString::from("@param {number} index */"), &[]);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_parameter_type(JsString::from("index"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testJsDocAfterStatic
#[test]
fn test_js_doc_after_static() {
    let mut f = set_up();
    let mut info = f.parse(JsString::from("@static @type {number} */"), &[]);
    {
        let arg0 = f.types.number_type;
        let arg1 = (((info.clone()).unwrap()).get_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}

// port: JsDocInfoParserTest#testUnsupportedJsDocSyntax1
#[test]
fn test_unsupported_js_doc_syntax1() {
    let mut f = set_up();
    let mut info = f.parse_with_documentation(
        JsString::from("@param {string} [accessLevel=\"author\"] The user level */"),
        true,
        &[],
    );
    assert_eq!(((info.clone()).unwrap()).get_parameter_count(), 1);
    {
        let arg0 = {
            let arg0 = f.types.string_type;
            f.types.create_optional_type(arg0)
        };
        let arg1 =
            (((info.clone()).unwrap()).get_parameter_type(JsString::from("accessLevel"))).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
    assert_eq!(
        ((info.clone()).unwrap()).get_description_for_parameter(JsString::from("accessLevel")),
        Some(JsString::from("The user level"))
    );
}

// port: JsDocInfoParserTest#testPreserveWhitespace6
#[test]
fn test_preserve_whitespace6() {
    let mut f = set_up();
    let mut info = f.preserve_whitespace_parse(JsString::from("This is the typedef description\n *     with multiple lines\n * @typedef \n {string}*/\n"), &[]);
    assert_eq!(
        ((info.clone()).unwrap()).get_block_description(),
        Some(
            (JsString::from("This is the typedef description\n     with multiple lines\n"))
                .concat(&(JsString::from(" ")))
        )
    );
    assert!((((info.clone()).unwrap()).has_typedef_type()));
    {
        let arg0 = f.types.string_type;
        let arg1 = (((info.clone()).unwrap()).get_typedef_type()).unwrap();
        f.types.assert_type_equals_expression(arg0, &arg1)
    };
}
