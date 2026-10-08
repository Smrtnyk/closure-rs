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

//! FORMAT.md: one unit-corpus record (one line of `corpus/unit/records/<Class>.jsonl.gz`).

use crate::json::{JsString, JsonValue};
use crate::post_call::PostCall;
use crate::reader::{
    ModelResult, Obj, ObjOut, arr, as_js_string, as_opt_js_string, as_opt_string, as_string, err,
    int, js, list_of, opt_js, opt_st, st, strs,
};
use crate::value::{
    DiagnosticGroupRef, DiagnosticTypeRef, FieldDump, SourceFile, Value, field_dump_from_json,
    field_dump_to_json,
};

/// FORMAT.md "Top-level fields" `kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecordKind {
    CompilerTestCase,
    Integration,
    TypeCheck,
}

impl RecordKind {
    pub fn name(self) -> &'static str {
        match self {
            RecordKind::CompilerTestCase => "compiler_test_case",
            RecordKind::Integration => "integration",
            RecordKind::TypeCheck => "type_check",
        }
    }

    pub fn from_name(s: &str) -> Option<RecordKind> {
        Some(match s {
            "compiler_test_case" => RecordKind::CompilerTestCase,
            "integration" => RecordKind::Integration,
            "type_check" => RecordKind::TypeCheck,
            _ => return None,
        })
    }
}

/// FORMAT.md "Top-level fields" `api`: the hooked entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Api {
    /// CompilerTestCase `testInternal`.
    TestInternal,
    /// CompilerTestCase `testExternChanges`.
    TestExternChanges,
    /// IntegrationTestCase `test`.
    Test,
    /// IntegrationTestCase `test_warning`.
    TestWarning,
    /// IntegrationTestCase `test_warnings`.
    TestWarnings,
    /// IntegrationTestCase `testParseError`.
    TestParseError,
    /// IntegrationTestCase `testNoWarnings`.
    TestNoWarnings,
    /// IntegrationTestCase `compile`.
    Compile,
    /// TypeCheckTestCase `TypeTestBuilder.run`.
    TypeTestBuilderRun,
    /// TypeCheckTestCase `parseAndTypeCheckWithScope`.
    ParseAndTypeCheckWithScope,
}

impl Api {
    pub const ALL: [Api; 10] = [
        Api::TestInternal,
        Api::TestExternChanges,
        Api::Test,
        Api::TestWarning,
        Api::TestWarnings,
        Api::TestParseError,
        Api::TestNoWarnings,
        Api::Compile,
        Api::TypeTestBuilderRun,
        Api::ParseAndTypeCheckWithScope,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Api::TestInternal => "testInternal",
            Api::TestExternChanges => "testExternChanges",
            Api::Test => "test",
            Api::TestWarning => "test_warning",
            Api::TestWarnings => "test_warnings",
            Api::TestParseError => "testParseError",
            Api::TestNoWarnings => "testNoWarnings",
            Api::Compile => "compile",
            Api::TypeTestBuilderRun => "TypeTestBuilder.run",
            Api::ParseAndTypeCheckWithScope => "parseAndTypeCheckWithScope",
        }
    }

    pub fn from_name(s: &str) -> Option<Api> {
        Api::ALL.into_iter().find(|a| a.name() == s)
    }

    /// The record kind this api belongs to.
    pub fn kind(self) -> RecordKind {
        match self {
            Api::TestInternal | Api::TestExternChanges => RecordKind::CompilerTestCase,
            Api::TypeTestBuilderRun | Api::ParseAndTypeCheckWithScope => RecordKind::TypeCheck,
            _ => RecordKind::Integration,
        }
    }
}

/// FORMAT.md "inputs". Which keys are present depends on the kind and api.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Inputs {
    pub externs: Option<Vec<SourceFile>>,
    pub sources: Option<Vec<SourceFile>>,
    pub chunks: Option<Vec<Chunk>>,
    /// Integration: the `String[]` passed to `test`.
    pub originals: Option<Vec<JsString>>,
    pub input_file_name_prefix: Option<String>,
    pub input_file_name_suffix: Option<String>,
    /// Type-check builder.
    pub extern_strings: Option<Vec<JsString>>,
    pub include_default_externs: Option<bool>,
    pub default_externs: Option<JsString>,
}

/// FORMAT.md "inputs" chunk `{name, deps: [names], inputs: [SourceFile]}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub name: String,
    pub deps: Vec<String>,
    pub inputs: Vec<SourceFile>,
}

impl Chunk {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<Chunk> {
        let mut o = Obj::new(v, path)?;
        let name = o.req_string("name")?;
        let p = o.sub("deps");
        let deps = list_of(o.req("deps")?, &p, as_string)?;
        let p = o.sub("inputs");
        let inputs = SourceFile::list_from_json(o.req("inputs")?, &p)?;
        o.finish()?;
        Ok(Chunk { name, deps, inputs })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("name", st(&self.name))
            .put("deps", strs(&self.deps))
            .put("inputs", SourceFile::list_to_json(&self.inputs))
            .build()
    }
}

impl Inputs {
    fn from_json(v: &JsonValue, path: &str, kind: RecordKind) -> ModelResult<Inputs> {
        let mut o = Obj::new(v, path)?;
        let files = |o: &mut Obj<'_>, k: &str| -> ModelResult<Option<Vec<SourceFile>>> {
            let p = o.sub(k);
            o.opt(k)
                .map(|x| SourceFile::list_from_json(x, &p))
                .transpose()
        };
        let externs = files(&mut o, "externs")?;
        let sources = files(&mut o, "sources")?;
        let p = o.sub("chunks");
        let chunks = o
            .opt("chunks")
            .map(|x| list_of(x, &p, Chunk::from_json))
            .transpose()?;
        let p = o.sub("originals");
        let originals = o
            .opt("originals")
            .map(|x| list_of(x, &p, as_js_string))
            .transpose()?;
        let input_file_name_prefix = o.opt_string("inputFileNamePrefix")?;
        let input_file_name_suffix = o.opt_string("inputFileNameSuffix")?;
        let p = o.sub("externStrings");
        let extern_strings = o
            .opt("externStrings")
            .map(|x| list_of(x, &p, as_js_string))
            .transpose()?;
        let include_default_externs = o.opt_bool("includeDefaultExterns")?;
        let p = o.sub("defaultExterns");
        let default_externs = o
            .opt("defaultExterns")
            .map(|x| as_js_string(x, &p))
            .transpose()?;
        o.finish()?;
        let r = Inputs {
            externs,
            sources,
            chunks,
            originals,
            input_file_name_prefix,
            input_file_name_suffix,
            extern_strings,
            include_default_externs,
            default_externs,
        };
        // FORMAT.md "inputs": the key combinations each kind records.
        let ok = match kind {
            RecordKind::CompilerTestCase => {
                r.externs.is_some()
                    && (r.sources.is_some() != r.chunks.is_some())
                    && r.originals.is_none()
                    && r.extern_strings.is_none()
            }
            RecordKind::Integration => {
                r.externs.is_some()
                    && r.sources.is_none()
                    && r.extern_strings.is_none()
                    && if r.chunks.is_some() {
                        r.originals.is_none() && r.input_file_name_prefix.is_none()
                    } else {
                        r.originals.is_some()
                            && r.input_file_name_prefix.is_some()
                            && r.input_file_name_suffix.is_some()
                    }
            }
            RecordKind::TypeCheck => {
                r.sources.is_some()
                    && r.chunks.is_none()
                    && r.originals.is_none()
                    && (r.extern_strings.is_some()
                        == (r.include_default_externs.is_some() && r.default_externs.is_some()))
            }
        };
        if !ok {
            return err(path, format!("input keys do not fit kind {}", kind.name()));
        }
        Ok(r)
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put_opt(
                "externs",
                self.externs.as_deref().map(SourceFile::list_to_json),
            )
            .put_opt(
                "sources",
                self.sources.as_deref().map(SourceFile::list_to_json),
            )
            .put_opt(
                "chunks",
                self.chunks.as_deref().map(|c| arr(c, Chunk::to_json)),
            )
            .put_opt("originals", self.originals.as_deref().map(|c| arr(c, js)))
            .put_opt(
                "inputFileNamePrefix",
                self.input_file_name_prefix.as_deref().map(st),
            )
            .put_opt(
                "inputFileNameSuffix",
                self.input_file_name_suffix.as_deref().map(st),
            )
            .put_opt(
                "externStrings",
                self.extern_strings.as_deref().map(|c| arr(c, js)),
            )
            .put_opt(
                "includeDefaultExterns",
                self.include_default_externs.map(JsonValue::Bool),
            )
            .put_opt("defaultExterns", self.default_externs.as_ref().map(js))
            .build()
    }
}

/// CompilerTestCase `expected.output`: a list of SourceFile, `"SAME"`, or null (no output check).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpectedOutput {
    Files(Vec<SourceFile>),
    Same,
    NoCheck,
}

/// FORMAT.md "expected" diagnostic `messageMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageMode {
    /// `withMessage`: the trimmed description equals `message`.
    ExactTrimmed,
    /// `withMessageContaining`.
    Contains,
    /// Any other predicate; `message` is its name and replay reports a harness error.
    Unknown,
}

impl MessageMode {
    pub fn name(self) -> &'static str {
        match self {
            MessageMode::ExactTrimmed => "exact_trimmed",
            MessageMode::Contains => "contains",
            MessageMode::Unknown => "unknown",
        }
    }
}

/// CompilerTestCase expected diagnostic `{key, level, messageMode?, message?, line?, charno?, length?}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedDiagnostic {
    pub key: String,
    pub level: String,
    pub message_mode: Option<MessageMode>,
    pub message: Option<JsString>,
    pub line: Option<i32>,
    pub charno: Option<i32>,
    pub length: Option<i32>,
}

impl ExpectedDiagnostic {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<ExpectedDiagnostic> {
        let mut o = Obj::new(v, path)?;
        let key = o.req_string("key")?;
        let level = o.req_string("level")?;
        let message_mode = match o.opt_string("messageMode")?.as_deref() {
            None => None,
            Some("exact_trimmed") => Some(MessageMode::ExactTrimmed),
            Some("contains") => Some(MessageMode::Contains),
            Some("unknown") => Some(MessageMode::Unknown),
            Some(m) => return err(&o.sub("messageMode"), format!("unknown messageMode {m:?}")),
        };
        let p = o.sub("message");
        let message = o.opt("message").map(|x| as_js_string(x, &p)).transpose()?;
        let line = o.opt_i32("line")?;
        let charno = o.opt_i32("charno")?;
        let length = o.opt_i32("length")?;
        o.finish()?;
        Ok(ExpectedDiagnostic {
            key,
            level,
            message_mode,
            message,
            line,
            charno,
            length,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("key", st(&self.key))
            .put("level", st(&self.level))
            .put_opt("messageMode", self.message_mode.map(|m| st(m.name())))
            .put_opt("message", self.message.as_ref().map(js))
            .put_opt("line", self.line.map(int))
            .put_opt("charno", self.charno.map(int))
            .put_opt("length", self.length.map(int))
            .build()
    }
}

/// FORMAT.md "expected", by kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expected {
    CompilerTestCase {
        output: ExpectedOutput,
        diagnostics: Vec<ExpectedDiagnostic>,
        /// Number of postconditions the original call ran.
        postconditions: i32,
        /// One value per postcondition (absent for `testExternChanges`).
        postcondition_values: Option<Vec<Value>>,
        /// FORMAT.md "Postcondition data": indices of captured postcondition lambdas.
        postconditions_captured: Option<Vec<i32>>,
    },
    Integration {
        output: Option<Vec<JsString>>,
        diagnostic_groups: Option<Vec<DiagnosticGroupRef>>,
    },
    TypeCheck {
        diagnostic_types: Vec<DiagnosticTypeRef>,
        diagnostic_descriptions: Vec<JsString>,
    },
}

impl Expected {
    fn from_json(v: &JsonValue, path: &str, kind: RecordKind) -> ModelResult<Expected> {
        let mut o = Obj::new(v, path)?;
        let r = match kind {
            RecordKind::CompilerTestCase => {
                let p = o.sub("output");
                let output = match o.req("output")? {
                    JsonValue::Null => ExpectedOutput::NoCheck,
                    JsonValue::String(s) if s.eq_str("SAME") => ExpectedOutput::Same,
                    x => ExpectedOutput::Files(SourceFile::list_from_json(x, &p)?),
                };
                let p = o.sub("diagnostics");
                let diagnostics =
                    list_of(o.req("diagnostics")?, &p, ExpectedDiagnostic::from_json)?;
                let postconditions = o.req_i32("postconditions")?;
                let p = o.sub("postconditionValues");
                let postcondition_values = o
                    .opt("postconditionValues")
                    .map(|x| list_of(x, &p, Value::from_json))
                    .transpose()?;
                let p = o.sub("postconditionsCaptured");
                let postconditions_captured = o
                    .opt("postconditionsCaptured")
                    .map(|x| list_of(x, &p, crate::reader::as_i32))
                    .transpose()?;
                Expected::CompilerTestCase {
                    output,
                    diagnostics,
                    postconditions,
                    postcondition_values,
                    postconditions_captured,
                }
            }
            RecordKind::Integration => {
                let p = o.sub("output");
                let output = match o.req("output")? {
                    JsonValue::Null => None,
                    x => Some(list_of(x, &p, as_js_string)?),
                };
                let p = o.sub("diagnosticGroups");
                let diagnostic_groups = match o.req("diagnosticGroups")? {
                    JsonValue::Null => None,
                    x => Some(list_of(x, &p, DiagnosticGroupRef::from_json)?),
                };
                Expected::Integration {
                    output,
                    diagnostic_groups,
                }
            }
            RecordKind::TypeCheck => {
                let p = o.sub("diagnosticTypes");
                let diagnostic_types =
                    list_of(o.req("diagnosticTypes")?, &p, DiagnosticTypeRef::from_json)?;
                let p = o.sub("diagnosticDescriptions");
                let diagnostic_descriptions =
                    list_of(o.req("diagnosticDescriptions")?, &p, as_js_string)?;
                Expected::TypeCheck {
                    diagnostic_types,
                    diagnostic_descriptions,
                }
            }
        };
        o.finish()?;
        Ok(r)
    }

    fn to_json(&self) -> JsonValue {
        match self {
            Expected::CompilerTestCase {
                output,
                diagnostics,
                postconditions,
                postcondition_values,
                postconditions_captured,
            } => ObjOut::new()
                .put(
                    "output",
                    match output {
                        ExpectedOutput::Files(f) => SourceFile::list_to_json(f),
                        ExpectedOutput::Same => st("SAME"),
                        ExpectedOutput::NoCheck => JsonValue::Null,
                    },
                )
                .put("diagnostics", arr(diagnostics, ExpectedDiagnostic::to_json))
                .put("postconditions", int(*postconditions))
                .put_opt(
                    "postconditionValues",
                    postcondition_values
                        .as_deref()
                        .map(|v| arr(v, Value::to_json)),
                )
                .put_opt(
                    "postconditionsCaptured",
                    postconditions_captured
                        .as_deref()
                        .map(|v| arr(v, |i| int(*i))),
                )
                .build(),
            Expected::Integration {
                output,
                diagnostic_groups,
            } => ObjOut::new()
                .put(
                    "output",
                    output.as_deref().map_or(JsonValue::Null, |o| arr(o, js)),
                )
                .put(
                    "diagnosticGroups",
                    diagnostic_groups
                        .as_deref()
                        .map_or(JsonValue::Null, |g| arr(g, DiagnosticGroupRef::to_json)),
                )
                .build(),
            Expected::TypeCheck {
                diagnostic_types,
                diagnostic_descriptions,
            } => ObjOut::new()
                .put(
                    "diagnosticTypes",
                    arr(diagnostic_types, DiagnosticTypeRef::to_json),
                )
                .put("diagnosticDescriptions", arr(diagnostic_descriptions, js))
                .build(),
        }
    }
}

/// FORMAT.md "comparison" `mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComparisonMode {
    /// CompilerTestCase `compareAsTree` (`isEquivalentTo`).
    Ast,
    /// CompilerTestCase string comparison.
    String,
    /// IntegrationTestCase `assertNode(...).isEqualTo(...)`.
    AstAssertNode,
    DiagnosticTypesInOrder,
    DiagnosticDescriptionsInOrder,
    /// Direct `parseAndTypeCheckWithScope`: the test's own follow-up assertions are not captured.
    ObservedOnly,
}

impl ComparisonMode {
    const ALL: [ComparisonMode; 6] = [
        ComparisonMode::Ast,
        ComparisonMode::String,
        ComparisonMode::AstAssertNode,
        ComparisonMode::DiagnosticTypesInOrder,
        ComparisonMode::DiagnosticDescriptionsInOrder,
        ComparisonMode::ObservedOnly,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ComparisonMode::Ast => "ast",
            ComparisonMode::String => "string",
            ComparisonMode::AstAssertNode => "ast_assertNode",
            ComparisonMode::DiagnosticTypesInOrder => "diagnostic_types_in_order",
            ComparisonMode::DiagnosticDescriptionsInOrder => "diagnostic_descriptions_in_order",
            ComparisonMode::ObservedOnly => "observed_only",
        }
    }

    pub fn from_name(s: &str) -> Option<ComparisonMode> {
        ComparisonMode::ALL.into_iter().find(|m| m.name() == s)
    }
}

/// FORMAT.md "comparison". **Do not use** `normalize_expected` / `closure_pass_for_expected` to
/// build the expected side (FORMAT.md "comparison" warning); use `ExpectedPipeline`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub mode: ComparisonMode,
    pub compare_js_doc: Option<bool>,
    pub normalize_expected: Option<bool>,
    pub closure_pass_for_expected: Option<bool>,
}

impl Comparison {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<Comparison> {
        let mut o = Obj::new(v, path)?;
        let m = o.req_string("mode")?;
        let Some(mode) = ComparisonMode::from_name(&m) else {
            return err(&o.sub("mode"), format!("unknown comparison mode {m:?}"));
        };
        let compare_js_doc = o.opt_bool("compareJsDoc")?;
        let normalize_expected = o.opt_bool("normalizeExpected")?;
        let closure_pass_for_expected = o.opt_bool("closurePassForExpected")?;
        o.finish()?;
        Ok(Comparison {
            mode,
            compare_js_doc,
            normalize_expected,
            closure_pass_for_expected,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("mode", st(self.mode.name()))
            .put_opt("compareJsDoc", self.compare_js_doc.map(JsonValue::Bool))
            .put_opt(
                "normalizeExpected",
                self.normalize_expected.map(JsonValue::Bool),
            )
            .put_opt(
                "closurePassForExpected",
                self.closure_pass_for_expected.map(JsonValue::Bool),
            )
            .build()
    }
}

/// FORMAT.md "harness" `overrides` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Override {
    /// `name(ParamTypes)`
    pub method: String,
    pub declared_in: String,
}

/// FORMAT.md "harness" `effective`: effective values of overridable hooks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effective {
    pub get_num_repetitions: i32,
    /// Class name of the coding convention.
    pub get_coding_convention: String,
    pub get_name: String,
    /// Class of the compiler actually used.
    pub create_compiler: String,
}

/// FORMAT.md "harness".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Harness {
    pub base: String,
    /// Every instance field declared in the base class (CompilerTestCase, IntegrationTestCase).
    pub fields: Option<FieldDump>,
    pub overrides: Option<Vec<Override>>,
    pub effective: Option<Effective>,
    /// CompilerTestCase: base-class fields whose dump changed while `getOptions()` ran.
    pub fields_after_get_options: Option<FieldDump>,
    /// Type-check builder.
    pub diagnostics_are_errors: Option<bool>,
    pub report_unknown_types: Option<bool>,
    pub suppress: Option<Vec<DiagnosticGroupRef>>,
}

impl Harness {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<Harness> {
        let mut o = Obj::new(v, path)?;
        let base = o.req_string("base")?;
        let p = o.sub("fields");
        let fields = o
            .opt("fields")
            .map(|x| field_dump_from_json(x, &p))
            .transpose()?;
        let p = o.sub("overrides");
        let overrides = o
            .opt("overrides")
            .map(|x| {
                list_of(x, &p, |e, ep| {
                    let mut eo = Obj::new(e, ep)?;
                    let method = eo.req_string("method")?;
                    let declared_in = eo.req_string("declaredIn")?;
                    eo.finish()?;
                    Ok(Override {
                        method,
                        declared_in,
                    })
                })
            })
            .transpose()?;
        let p = o.sub("effective");
        let effective = o
            .opt("effective")
            .map(|x| {
                let mut eo = Obj::new(x, &p)?;
                let e = Effective {
                    get_num_repetitions: eo.req_i32("getNumRepetitions")?,
                    get_coding_convention: eo.req_string("getCodingConvention")?,
                    get_name: eo.req_string("getName")?,
                    create_compiler: eo.req_string("createCompiler")?,
                };
                eo.finish()?;
                Ok(e)
            })
            .transpose()?;
        let p = o.sub("fieldsAfterGetOptions");
        let fields_after_get_options = o
            .opt("fieldsAfterGetOptions")
            .map(|x| field_dump_from_json(x, &p))
            .transpose()?;
        let diagnostics_are_errors = o.opt_bool("diagnosticsAreErrors")?;
        let report_unknown_types = o.opt_bool("reportUnknownTypes")?;
        let p = o.sub("suppress");
        let suppress = o
            .opt("suppress")
            .map(|x| list_of(x, &p, DiagnosticGroupRef::from_json))
            .transpose()?;
        o.finish()?;
        Ok(Harness {
            base,
            fields,
            overrides,
            effective,
            fields_after_get_options,
            diagnostics_are_errors,
            report_unknown_types,
            suppress,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("base", st(&self.base))
            .put_opt("fields", self.fields.as_ref().map(field_dump_to_json))
            .put_opt(
                "overrides",
                self.overrides.as_deref().map(|o| {
                    arr(o, |e| {
                        ObjOut::new()
                            .put("method", st(&e.method))
                            .put("declaredIn", st(&e.declared_in))
                            .build()
                    })
                }),
            )
            .put_opt(
                "effective",
                self.effective.as_ref().map(|e| {
                    ObjOut::new()
                        .put("getNumRepetitions", int(e.get_num_repetitions))
                        .put("getCodingConvention", st(&e.get_coding_convention))
                        .put("getName", st(&e.get_name))
                        .put("createCompiler", st(&e.create_compiler))
                        .build()
                }),
            )
            .put_opt(
                "fieldsAfterGetOptions",
                self.fields_after_get_options
                    .as_ref()
                    .map(field_dump_to_json),
            )
            .put_opt(
                "diagnosticsAreErrors",
                self.diagnostics_are_errors.map(JsonValue::Bool),
            )
            .put_opt(
                "reportUnknownTypes",
                self.report_unknown_types.map(JsonValue::Bool),
            )
            .put_opt(
                "suppress",
                self.suppress
                    .as_deref()
                    .map(|s| arr(s, DiagnosticGroupRef::to_json)),
            )
            .build()
    }
}

/// FORMAT.md "observed": a JSError dump `{key, defaultLevel, description, source, line, charno, length}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JSErrorDump {
    pub key: String,
    /// The DiagnosticType's default level, not the effective one.
    pub default_level: String,
    pub description: JsString,
    pub source: Option<String>,
    pub line: i32,
    pub charno: i32,
    pub length: i32,
}

impl JSErrorDump {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<JSErrorDump> {
        let mut o = Obj::new(v, path)?;
        let key = o.req_string("key")?;
        let default_level = o.req_string("defaultLevel")?;
        let description = o.req_js_string("description")?;
        let p = o.sub("source");
        let source = as_opt_string(o.req("source")?, &p)?;
        let line = o.req_i32("line")?;
        let charno = o.req_i32("charno")?;
        let length = o.req_i32("length")?;
        o.finish()?;
        Ok(JSErrorDump {
            key,
            default_level,
            description,
            source,
            line,
            charno,
            length,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("key", st(&self.key))
            .put("defaultLevel", st(&self.default_level))
            .put("description", js(&self.description))
            .put("source", opt_st(&self.source))
            .put("line", int(self.line))
            .put("charno", int(self.charno))
            .put("length", int(self.length))
            .build()
    }
}

/// FORMAT.md "observed".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observed {
    pub errors: Vec<JSErrorDump>,
    pub warnings: Vec<JSErrorDump>,
    /// Integration: `compiler.toSource()`; `Some(None)` when it was null.
    #[allow(clippy::option_option)]
    pub output: Option<Option<JsString>>,
    /// Integration: informational, never compared.
    pub compiler_class: Option<String>,
}

impl Observed {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<Observed> {
        let mut o = Obj::new(v, path)?;
        let p = o.sub("errors");
        let errors = list_of(o.req("errors")?, &p, JSErrorDump::from_json)?;
        let p = o.sub("warnings");
        let warnings = list_of(o.req("warnings")?, &p, JSErrorDump::from_json)?;
        let p = o.sub("output");
        let output = o
            .opt("output")
            .map(|x| as_opt_js_string(x, &p))
            .transpose()?;
        let compiler_class = o.opt_string("compilerClass")?;
        o.finish()?;
        Ok(Observed {
            errors,
            warnings,
            output,
            compiler_class,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("errors", arr(&self.errors, JSErrorDump::to_json))
            .put("warnings", arr(&self.warnings, JSErrorDump::to_json))
            .put_opt("output", self.output.as_ref().map(opt_js))
            .put_opt("compilerClass", self.compiler_class.as_deref().map(st))
            .build()
    }
}

/// FORMAT.md "Outcome contract".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Normal,
    Exception {
        exception_class: String,
        /// `getMessage()`, cut to 4,000 characters (null allowed).
        message: Option<JsString>,
        /// True iff the throwable is a `java.lang.AssertionError`.
        assertion: bool,
    },
}

impl Outcome {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<Outcome> {
        let mut o = Obj::new(v, path)?;
        let status = o.req_string("status")?;
        let r = match status.as_str() {
            "normal" => Outcome::Normal,
            "exception" => {
                let exception_class = o.req_string("exceptionClass")?;
                let p = o.sub("message");
                let message = as_opt_js_string(o.req("message")?, &p)?;
                let assertion = o.req_bool("assertion")?;
                Outcome::Exception {
                    exception_class,
                    message,
                    assertion,
                }
            }
            s => return err(&o.sub("status"), format!("unknown outcome status {s:?}")),
        };
        o.finish()?;
        Ok(r)
    }

    fn to_json(&self) -> JsonValue {
        match self {
            Outcome::Normal => ObjOut::new().put("status", st("normal")).build(),
            Outcome::Exception {
                exception_class,
                message,
                assertion,
            } => ObjOut::new()
                .put("status", st("exception"))
                .put("exceptionClass", st(exception_class))
                .put("message", opt_js(message))
                .put("assertion", JsonValue::Bool(*assertion))
                .build(),
        }
    }
}

/// FORMAT.md "Unrepresentable paths": `{path, reason}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unrepresentable {
    pub path: String,
    pub reason: String,
}

impl Unrepresentable {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<Unrepresentable> {
        let mut uo = Obj::new(v, path)?;
        let path = uo.req_string("path")?;
        let reason = uo.req_string("reason")?;
        uo.finish()?;
        Ok(Unrepresentable { path, reason })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("path", st(&self.path))
            .put("reason", st(&self.reason))
            .build()
    }
}

/// FORMAT.md `processor`: `{class, fields}` of what `getProcessor` returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Processor {
    pub class: String,
    pub fields: FieldDump,
}

/// One record (FORMAT.md "Top-level fields").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub v: i32,
    pub class: String,
    pub method: String,
    pub call: i32,
    pub kind: RecordKind,
    pub api: Api,
    pub instance_class: Option<String>,
    pub inputs: Inputs,
    /// Absent for `parseAndTypeCheckWithScope` records.
    pub expected: Option<Expected>,
    pub comparison: Comparison,
    pub harness: Harness,
    pub test_fields: Option<FieldDump>,
    /// CompilerTestCase: the `testFields` entries that changed during the hooked call.
    pub test_fields_after: Option<FieldDump>,
    pub test_fields_after_error: Option<JsString>,
    /// `CompilerOptions` diff against `new CompilerOptions()` (`options_defaults.json`).
    pub options: FieldDump,
    /// CompilerTestCase: `None` = key absent, `Some(None)` = null (the processor never ran).
    #[allow(clippy::option_option)]
    pub processor: Option<Option<Processor>>,
    pub observed: Observed,
    pub outcome: Outcome,
    pub unrepresentable: Vec<Unrepresentable>,
    pub post_call: Option<PostCall>,
    pub post_call_error: Option<JsString>,
    /// FORMAT.md "passTrace": sorted FQCNs of the in-scope passes whose entry points ran.
    pub pass_trace: Option<Vec<String>>,
}

impl Record {
    pub fn from_json(v: &JsonValue) -> ModelResult<Record> {
        let mut o = Obj::new(v, "$")?;
        let rv = o.req_i32("v")?;
        if rv != 2 {
            return err("$.v", format!("unsupported record format version {rv}"));
        }
        let class = o.req_string("class")?;
        let method = o.req_string("method")?;
        let call = o.req_i32("call")?;
        let k = o.req_string("kind")?;
        let Some(kind) = RecordKind::from_name(&k) else {
            return err("$.kind", format!("unknown kind {k:?}"));
        };
        let a = o.req_string("api")?;
        let Some(api) = Api::from_name(&a) else {
            return err("$.api", format!("unknown api {a:?}"));
        };
        if api.kind() != kind {
            return err("$.api", format!("api {a:?} does not belong to kind {k:?}"));
        }
        let instance_class = o.opt_string("instanceClass")?;
        let inputs = Inputs::from_json(o.req("inputs")?, "$.inputs", kind)?;
        let expected = o
            .opt("expected")
            .map(|x| Expected::from_json(x, "$.expected", kind))
            .transpose()?;
        let comparison = Comparison::from_json(o.req("comparison")?, "$.comparison")?;
        let harness = Harness::from_json(o.req("harness")?, "$.harness")?;
        let test_fields = o
            .opt("testFields")
            .map(|x| field_dump_from_json(x, "$.testFields"))
            .transpose()?;
        let test_fields_after = o
            .opt("testFieldsAfter")
            .map(|x| field_dump_from_json(x, "$.testFieldsAfter"))
            .transpose()?;
        let test_fields_after_error = o
            .opt("testFieldsAfterError")
            .map(|x| as_js_string(x, "$.testFieldsAfterError"))
            .transpose()?;
        let options = field_dump_from_json(o.req("options")?, "$.options")?;
        let processor = o
            .opt("processor")
            .map(|x| match x {
                JsonValue::Null => Ok(None),
                x => {
                    let mut po = Obj::new(x, "$.processor")?;
                    let class = po.req_string("class")?;
                    let fields = field_dump_from_json(po.req("fields")?, "$.processor.fields")?;
                    po.finish()?;
                    Ok(Some(Processor { class, fields }))
                }
            })
            .transpose()?;
        let observed = Observed::from_json(o.req("observed")?, "$.observed")?;
        let outcome = Outcome::from_json(o.req("outcome")?, "$.outcome")?;
        let unrepresentable = list_of(
            o.req("unrepresentable")?,
            "$.unrepresentable",
            Unrepresentable::from_json,
        )?;
        let post_call = o
            .opt("postCall")
            .map(|x| PostCall::from_json(x, "$.postCall"))
            .transpose()?;
        let post_call_error = o
            .opt("postCallError")
            .map(|x| as_js_string(x, "$.postCallError"))
            .transpose()?;
        let pass_trace = o
            .opt("passTrace")
            .map(|x| list_of(x, "$.passTrace", as_string))
            .transpose()?;
        o.finish()?;
        Ok(Record {
            v: rv,
            class,
            method,
            call,
            kind,
            api,
            instance_class,
            inputs,
            expected,
            comparison,
            harness,
            test_fields,
            test_fields_after,
            test_fields_after_error,
            options,
            processor,
            observed,
            outcome,
            unrepresentable,
            post_call,
            post_call_error,
            pass_trace,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("v", int(self.v))
            .put("class", st(&self.class))
            .put("method", st(&self.method))
            .put("call", int(self.call))
            .put("kind", st(self.kind.name()))
            .put("api", st(self.api.name()))
            .put_opt("instanceClass", self.instance_class.as_deref().map(st))
            .put("inputs", self.inputs.to_json())
            .put_opt("expected", self.expected.as_ref().map(Expected::to_json))
            .put("comparison", self.comparison.to_json())
            .put("harness", self.harness.to_json())
            .put_opt(
                "testFields",
                self.test_fields.as_ref().map(field_dump_to_json),
            )
            .put_opt(
                "testFieldsAfter",
                self.test_fields_after.as_ref().map(field_dump_to_json),
            )
            .put_opt(
                "testFieldsAfterError",
                self.test_fields_after_error.as_ref().map(js),
            )
            .put("options", field_dump_to_json(&self.options))
            .put_opt(
                "processor",
                self.processor.as_ref().map(|p| match p {
                    None => JsonValue::Null,
                    Some(p) => ObjOut::new()
                        .put("class", st(&p.class))
                        .put("fields", field_dump_to_json(&p.fields))
                        .build(),
                }),
            )
            .put("observed", self.observed.to_json())
            .put("outcome", self.outcome.to_json())
            .put(
                "unrepresentable",
                arr(&self.unrepresentable, Unrepresentable::to_json),
            )
            .put_opt("postCall", self.post_call.as_ref().map(PostCall::to_json))
            .put_opt("postCallError", self.post_call_error.as_ref().map(js))
            .put_opt("passTrace", self.pass_trace.as_deref().map(strs))
            .build()
    }
}
