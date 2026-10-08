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

//! The auxiliary corpus files: `corpus/unit/options_defaults.json` (FORMAT.md "Options defaults
//! and warnings-guard order") and `corpus/unit/derived/expected_pipeline.jsonl.gz` (FORMAT.md
//! "Comparison", HARNESS.md step 8e).

use crate::json::JsonValue;
use crate::reader::{ModelResult, Obj, ObjOut, arr, as_string, int, list_of, map_of, st};
use crate::record::Unrepresentable;
use crate::value::{FieldDump, field_dump_from_json, field_dump_to_json};
use indexmap::IndexMap;

/// `options_defaults.json`: every instance field of `new CompilerOptions()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsDefaults {
    pub v: i32,
    /// `com.google.javascript.jscomp.CompilerOptions`
    pub class: String,
    pub depth: i32,
    /// Field name -> default value (FORMAT.md value encoding).
    pub fields: FieldDump,
    /// Field name -> declared Java type (generic signature).
    pub field_types: IndexMap<String, String>,
    pub unrepresentable: Vec<Unrepresentable>,
}

impl OptionsDefaults {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<OptionsDefaults> {
        let mut o = Obj::new(v, path)?;
        let version = o.req_i32("v")?;
        let class = o.req_string("class")?;
        let depth = o.req_i32("depth")?;
        let fp = o.sub("fields");
        let fields = field_dump_from_json(o.req("fields")?, &fp)?;
        let tp = o.sub("fieldTypes");
        let field_types = map_of(o.req("fieldTypes")?, &tp, as_string)?;
        let up = o.sub("unrepresentable");
        let unrepresentable = list_of(o.req("unrepresentable")?, &up, Unrepresentable::from_json)?;
        o.finish()?;
        Ok(OptionsDefaults {
            v: version,
            class,
            depth,
            fields,
            field_types,
            unrepresentable,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("v", int(self.v))
            .put("class", st(&self.class))
            .put("depth", int(self.depth))
            .put("fields", field_dump_to_json(&self.fields))
            .put(
                "fieldTypes",
                JsonValue::Object(
                    self.field_types
                        .iter()
                        .map(|(k, t)| (k.clone(), st(t)))
                        .collect(),
                ),
            )
            .put(
                "unrepresentable",
                arr(&self.unrepresentable, Unrepresentable::to_json),
            )
            .build()
    }
}

/// One line of `derived/expected_pipeline.jsonl.gz`: the effective expected-side pipeline of one
/// `compiler_test_case` record, keyed by record file base name and 0-based line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedPipeline {
    pub file: String,
    pub index: i32,
    pub class: String,
    pub method: String,
    pub call: i32,
    pub remove_casts_expected: bool,
    pub closure_pass_for_expected: bool,
    pub closure_rewrite_module_for_expected: bool,
    pub closure_provides_for_expected: bool,
    pub transpile_expected: bool,
    pub normalize_expected: bool,
    pub transpile_normalizes: bool,
}

impl ExpectedPipeline {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<ExpectedPipeline> {
        let mut o = Obj::new(v, path)?;
        let e = ExpectedPipeline {
            file: o.req_string("file")?,
            index: o.req_i32("index")?,
            class: o.req_string("class")?,
            method: o.req_string("method")?,
            call: o.req_i32("call")?,
            remove_casts_expected: o.req_bool("removeCastsExpected")?,
            closure_pass_for_expected: o.req_bool("closurePassForExpected")?,
            closure_rewrite_module_for_expected: o.req_bool("closureRewriteModuleForExpected")?,
            closure_provides_for_expected: o.req_bool("closureProvidesForExpected")?,
            transpile_expected: o.req_bool("transpileExpected")?,
            normalize_expected: o.req_bool("normalizeExpected")?,
            transpile_normalizes: o.req_bool("transpileNormalizes")?,
        };
        o.finish()?;
        Ok(e)
    }

    pub fn to_json(&self) -> JsonValue {
        let b = JsonValue::Bool;
        ObjOut::new()
            .put("file", st(&self.file))
            .put("index", int(self.index))
            .put("class", st(&self.class))
            .put("method", st(&self.method))
            .put("call", int(self.call))
            .put("removeCastsExpected", b(self.remove_casts_expected))
            .put("closurePassForExpected", b(self.closure_pass_for_expected))
            .put(
                "closureRewriteModuleForExpected",
                b(self.closure_rewrite_module_for_expected),
            )
            .put(
                "closureProvidesForExpected",
                b(self.closure_provides_for_expected),
            )
            .put("transpileExpected", b(self.transpile_expected))
            .put("normalizeExpected", b(self.normalize_expected))
            .put("transpileNormalizes", b(self.transpile_normalizes))
            .build()
    }
}
