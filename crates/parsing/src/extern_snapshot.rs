/*
 * Copyright 2026 The closure-rs Authors.
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

//! Parser snapshots embedded for the bundled browser externs. The host build and the target
//! compiler use this same schema and parser config. A file with diagnostics, source maps or
//! unsupported properties uses the ordinary parser.

use crate::{
    config::{Config, LanguageMode, StrictMode},
    parser::feature_set::FeatureSet,
    parser_runner::{ParseResult, ParserRunner},
};
use closure_rhino::{
    error_reporter::ErrorReporter,
    js_string::JsString,
    node::{Ast, snapshot::AstSnapshot},
    static_source_file::StaticSourceFile,
};
use std::sync::{Arc, LazyLock};

pub static CONFIG: LazyLock<Config> =
    LazyLock::new(|| ParserRunner::create_config(LanguageMode::ES_NEXT, None, StrictMode::STRICT));

#[derive(serde::Serialize, serde::Deserialize)]
struct Script {
    arena: AstSnapshot<FeatureSet>,
    features: FeatureSet,
}

pub fn generate(file: Arc<dyn StaticSourceFile>, code: JsString) -> Option<Vec<u8>> {
    let mut ast = Ast::new_for_preparse();
    let mut diagnostics = Diagnostics::default();
    let result =
        ParserRunner::try_parse(&mut ast, file.clone(), code, &CONFIG, &mut diagnostics).ok()?;
    if diagnostics.0
        || !result.comments.is_empty()
        || !result.top_level_statement_ranges.is_empty()
        || result.source_map_url.is_some()
    {
        return None;
    }
    let arena = AstSnapshot::capture(&ast, result.ast?, &file)?;
    Some(
        postcard::to_allocvec(&Script {
            arena,
            features: result.features,
        })
        .expect("serialize bundled extern"),
    )
}

pub fn load(bytes: &'static [u8], ast: &mut Ast, file: Arc<dyn StaticSourceFile>) -> ParseResult {
    let script: Script = postcard::from_bytes(bytes).expect("build-generated extern snapshot");
    let root = script.arena.append(ast, file);
    ParseResult::new(Some(root), Vec::new(), script.features, None)
}

#[derive(Default)]
struct Diagnostics(bool);
impl ErrorReporter for Diagnostics {
    fn error(&mut self, _: &str, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
    fn warning(&mut self, _: &str, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
    fn error_js_string(&mut self, _: &JsString, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
    fn warning_js_string(&mut self, _: &JsString, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
}
