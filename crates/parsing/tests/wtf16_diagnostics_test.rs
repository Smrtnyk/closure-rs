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

use closure_parsing::{
    config::{JsDocParsing, LanguageMode, RunMode, StrictMode},
    parser_runner::ParserRunner,
};
use closure_rhino::{
    js_string::JsString, node::Ast, simple_source_file::SimpleSourceFile,
    static_source_file::SourceKind, testing::test_error_reporter::TestErrorReporter,
};
use std::sync::Arc;

// port: JsDocInfoParser#addParserWarning(Msg, String, int, int)
#[test]
fn invalid_parameter_warning_preserves_lone_surrogates() {
    for unit in [0xd800, 0xdc00] {
        let name = JsString::from_units(vec![unit]);
        let code = JsString::from("/** @param {string} ")
            .concat(&name)
            .concat(&" */ function f() {}".into());
        let warning = JsString::from("invalid param name \"")
            .concat(&name)
            .concat(&"\"".into());
        let mut reporter = TestErrorReporter::new();
        reporter.expect_all_warnings_js_strings(&[warning]);
        let config = ParserRunner::create_config_full(
            LanguageMode::ECMASCRIPT3,
            JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
            RunMode::KEEP_GOING,
            None,
            true,
            StrictMode::SLOPPY,
        );
        let result = ParserRunner::parse(
            &mut Ast::new(),
            Arc::new(SimpleSourceFile::new("input", SourceKind::STRONG)),
            code,
            &config,
            &mut reporter,
        );
        assert!(result.ast.is_some());
        reporter.verify_has_encountered_all_warnings_and_errors();
    }
}
