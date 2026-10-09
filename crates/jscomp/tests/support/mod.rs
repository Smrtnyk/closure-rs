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
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch).

use closure_jscomp::{
    compiler_options::*, compose_warnings_guard::ComposeWarningsGuard,
    dependency_options::DependencyOptions,
    diagnostic_group_warnings_guard::DiagnosticGroupWarningsGuard,
    module_identifier::ModuleIdentifier, show_by_path_warnings_guard::ShowByPathWarningsGuard,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    java_lang::{double_to_string, pattern::Pattern},
    js_string::JsString,
};
use closure_testing::json::{JsString as RecordString, JsonValue as Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
pub trait RecordValue {
    // port: UnitRecorder#value
    fn record(&self) -> Value;
}
// port: UnitRecorder#value
pub fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}
// port: UnitRecorder#value
pub fn java_object(class: &str, fields: Value) -> Value {
    object([("object", Value::str(class)), ("fields", fields)])
}
// port: UnitRecorder#value
pub fn enum_value(class: &str, name: &str) -> Value {
    object([("enum", Value::str(class)), ("name", Value::str(name))])
}
// port: UnitRecorder#value
pub fn guava_optional<T: RecordValue>(value: &Option<T>) -> Value {
    match value {
        Some(v) => java_object(
            "com.google.common.base.Present",
            object([("reference", v.record())]),
        ),
        None => java_object("com.google.common.base.Absent", object([])),
    }
}
impl<T: RecordValue + ?Sized> RecordValue for &T {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        (*self).record()
    }
}
impl<T: RecordValue + ?Sized> RecordValue for Arc<T> {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        self.as_ref().record()
    }
}
impl<T: RecordValue> RecordValue for Option<T> {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        self.as_ref().map_or(Value::Null, RecordValue::record)
    }
}
impl RecordValue for bool {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        Value::Bool(*self)
    }
}
impl RecordValue for i32 {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        Value::int(*self as i64)
    }
}
impl RecordValue for f64 {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        object([("double", Value::str(&double_to_string(*self)))])
    }
}
impl RecordValue for str {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        Value::str(self)
    }
}
impl RecordValue for String {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        self.as_str().record()
    }
}
impl RecordValue for JsString {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        Value::String(RecordString(self.as_units().to_vec()))
    }
}
impl RecordValue for Path {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        Value::str(&self.to_string_lossy())
    }
}
impl RecordValue for PathBuf {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        self.as_path().record()
    }
}
impl<T: RecordValue> RecordValue for Vec<T> {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        object([
            (
                "list",
                Value::Array(self.iter().map(RecordValue::record).collect()),
            ),
            (
                "impl",
                Value::str(if self.len() == 1 {
                    "com.google.common.collect.SingletonImmutableList"
                } else {
                    "com.google.common.collect.RegularImmutableList"
                }),
            ),
        ])
    }
}
impl<T: RecordValue> RecordValue for IndexSet<T> {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        object([
            (
                "set",
                Value::Array(self.iter().map(RecordValue::record).collect()),
            ),
            (
                "impl",
                Value::str(if self.len() == 1 {
                    "com.google.common.collect.SingletonImmutableSet"
                } else {
                    "com.google.common.collect.RegularImmutableSet"
                }),
            ),
        ])
    }
}
impl<K: RecordValue, V: RecordValue> RecordValue for IndexMap<K, V> {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        object([
            (
                "map",
                Value::Array(
                    self.iter()
                        .map(|(k, v)| Value::Array(vec![k.record(), v.record()]))
                        .collect(),
                ),
            ),
            (
                "impl",
                Value::str("com.google.common.collect.RegularImmutableMap"),
            ),
        ])
    }
}
impl RecordValue for DefineValue {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        match self {
            Self::Boolean(v) => v.record(),
            Self::Integer(v) => v.record(),
            Self::Double(v) => v.record(),
            Self::String(v) => v.record(),
        }
    }
}
impl RecordValue for Pattern {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        object([
            ("regex", self.pattern().record()),
            ("flags", self.flags().record()),
        ])
    }
}
impl RecordValue for FeatureSet {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        let features: IndexSet<_> = self.get_features().into_iter().collect();
        java_object(
            "com.google.javascript.jscomp.parsing.parser.FeatureSet",
            object([(
                "features",
                if features.len() > 1 {
                    implementation(
                        features.record(),
                        "com.google.common.collect.ImmutableEnumSet",
                    )
                } else {
                    features.record()
                },
            )]),
        )
    }
}
impl RecordValue for DependencyOptions {
    // port: UnitRecorder#fields
    fn record(&self) -> Value {
        java_object(
            "com.google.javascript.jscomp.DependencyOptions",
            dependency_fields(self),
        )
    }
}
// port: UnitRecorder#fields
pub fn dependency_fields(d: &DependencyOptions) -> Value {
    object([
        ("mode", d.mode().record()),
        ("entryPoints", d.entry_points().to_vec().record()),
    ])
}
impl RecordValue for ModuleIdentifier {
    // port: UnitRecorder#fields
    fn record(&self) -> Value {
        java_object(
            "com.google.javascript.jscomp.ModuleIdentifier",
            object([
                ("name", self.name().record()),
                ("closureNamespace", self.closure_namespace().record()),
                ("moduleName", self.module_name().record()),
            ]),
        )
    }
}
impl RecordValue for ComposeWarningsGuard {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        let guards = self
            .get_guards()
            .into_iter()
            .map(|guard| {
                if guard.as_any().is::<DiagnosticGroupWarningsGuard>() {
                    // The merged diagnostics API exposes these private values through Display.
                    let text = guard.to_string();
                    let (group, level) = text
                        .strip_prefix("DiagnosticGroup<")
                        .unwrap()
                        .split_once(">(")
                        .unwrap();
                    let level = level.strip_suffix(')').unwrap();
                    java_object(
                        "com.google.javascript.jscomp.DiagnosticGroupWarningsGuard",
                        object([
                            (
                                "group",
                                object([(
                                    "diagnosticGroup",
                                    object([
                                        ("group", group.record()),
                                        ("types", Value::Array(vec![])),
                                    ]),
                                )]),
                            ),
                            (
                                "level",
                                enum_value("com.google.javascript.jscomp.CheckLevel", level),
                            ),
                        ]),
                    )
                } else if let Some(show) = guard.as_any().downcast_ref::<ShowByPathWarningsGuard>()
                {
                    // Debug is the other public projection of this merged module's private fields.
                    let text = format!("{show:?}");
                    let paths = text
                        .split_once("paths: ")
                        .unwrap()
                        .1
                        .split_once(", include: ")
                        .unwrap()
                        .0;
                    let paths: Vec<String> = serde_json::from_str(paths).unwrap();
                    let include = text
                        .split_once("include: ")
                        .unwrap()
                        .1
                        .split_once(',')
                        .unwrap()
                        .0
                        == "true";
                    let level = text
                        .split_once("level: ")
                        .unwrap()
                        .1
                        .split_once(' ')
                        .unwrap()
                        .0;
                    let priority = guard.get_priority();
                    java_object(
                        "com.google.javascript.jscomp.ShowByPathWarningsGuard",
                        object([(
                            "warningsGuard",
                            java_object(
                                "com.google.javascript.jscomp.ByPathWarningsGuard",
                                object([
                                    ("paths", paths.record()),
                                    ("include", include.record()),
                                    ("priority", priority.record()),
                                    (
                                        "level",
                                        enum_value(
                                            "com.google.javascript.jscomp.CheckLevel",
                                            level,
                                        ),
                                    ),
                                ]),
                            ),
                        )]),
                    )
                } else {
                    panic!("Unsupported guard in option configuration: {guard}");
                }
            })
            .collect();
        object([(
            "composeWarningsGuard",
            object([("guards", Value::Array(guards))]),
        )])
    }
}
// port: UnitRecorder#value (stand-in class tag only)
pub fn standin_class(class: &str) -> Value {
    java_object(class, object([]))
}
// port: UnitRecorder#value (collection implementation tag)
pub fn implementation(mut value: Value, class: &str) -> Value {
    if let Value::Object(fields) = &mut value {
        fields.insert("impl".into(), class.record());
    }
    value
}
// port: UnitRecorder#value (deferred state comparisons)
pub fn normalize_standins(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            // DefaultNameGenerator has no public state getters.
            if fields
                .get("object")
                .and_then(Value::as_js_string)
                .is_some_and(|s| s.eq_str("com.google.javascript.jscomp.DefaultNameGenerator"))
            {
                fields.insert("fields".into(), object([]));
            }
            if let Some(Value::Object(group)) = fields.get_mut("diagnosticGroup") {
                group.insert("types".into(), Value::Array(vec![]));
            }
            for v in fields.values_mut() {
                normalize_standins(v);
            }
        }
        Value::Array(items) => {
            for v in items {
                normalize_standins(v);
            }
        }
        _ => {}
    }
}
impl RecordValue for LanguageMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$LanguageMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for Environment {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$Environment",
            &self.to_string(),
        )
    }
}
impl RecordValue for IncrementalCheckMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$IncrementalCheckMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_parsing::config::JsDocParsing {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.parsing.Config$JsDocParsing",
            &format!("{self:?}"),
        )
    }
}
impl RecordValue for DevMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$DevMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_rhino::jscomp_base::Tri {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.base.Tri",
            &format!("{self:?}"),
        )
    }
}
impl RecordValue for ExtractPrototypeMemberDeclarationsMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$ExtractPrototypeMemberDeclarationsMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for AliasStringsMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$AliasStringsMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::variable_renaming_policy::VariableRenamingPolicy {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.VariableRenamingPolicy",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::property_renaming_policy::PropertyRenamingPolicy {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.PropertyRenamingPolicy",
            &self.to_string(),
        )
    }
}
impl RecordValue for OptimizeLocalAccess {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$OptimizeLocalAccess",
            &self.to_string(),
        )
    }
}
impl RecordValue for PropertyCollapseLevel {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$PropertyCollapseLevel",
            &self.to_string(),
        )
    }
}
impl RecordValue for J2clPassMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$J2clPassMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for TweakProcessing {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$TweakProcessing",
            &self.to_string(),
        )
    }
}
impl RecordValue for OutputJs {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$OutputJs",
            &self.to_string(),
        )
    }
}
impl RecordValue for Es6SubclassTranspilation {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$Es6SubclassTranspilation",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::js::runtime_js_lib_manager::RuntimeLibraryMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.js.RuntimeJsLibManager$RuntimeLibraryMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for TracerMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$TracerMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::error_format::ErrorFormat {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.ErrorFormat",
            &format!("{self:?}"),
        )
    }
}
impl RecordValue for closure_jscomp::source_map::DetailLevel {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.SourceMap$DetailLevel",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::source_map::Format {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.SourceMap$Format",
            &self.to_string(),
        )
    }
}
impl RecordValue for InstrumentOption {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$InstrumentOption",
            &self.to_string(),
        )
    }
}
impl RecordValue for ConformanceReportingMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$ConformanceReportingMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::deps::module_loader::ResolutionMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.deps.ModuleLoader$ResolutionMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::deps::module_loader::PathEscaper {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.deps.ModuleLoader$PathEscaper",
            &self.to_string(),
        )
    }
}
impl RecordValue for ChunkOutputType {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$ChunkOutputType",
            &self.to_string(),
        )
    }
}
impl RecordValue for Reach {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$Reach",
            &self.to_string(),
        )
    }
}
impl RecordValue for Es6ModuleTranspilation {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$Es6ModuleTranspilation",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_parsing::parser::feature_set::Feature {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.parsing.parser.FeatureSet$Feature",
            &format!("{self:?}"),
        )
    }
}
impl RecordValue for closure_jscomp::dependency_options::DependencyMode {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.DependencyOptions$DependencyMode",
            &self.to_string(),
        )
    }
}
impl RecordValue for closure_jscomp::custom_pass_execution_time::CustomPassExecutionTime {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CustomPassExecutionTime",
            &self.to_string(),
        )
    }
}
impl RecordValue for dyn closure_jscomp::name_generator::NameGenerator + Send + Sync {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.DefaultNameGenerator")
    }
}
impl RecordValue for dyn closure_jscomp::coding_convention::CodingConvention + Send + Sync {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.CodingConvention")
    }
}
impl RecordValue for dyn closure_jscomp::message_bundle::MessageBundle + Send + Sync {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.MessageBundle")
    }
}
impl RecordValue for dyn closure_jscomp::css_renaming_map::CssRenamingMap + Send + Sync {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.CssRenamingMap")
    }
}
impl RecordValue for dyn closure_jscomp::renaming_map::RenamingMap + Send + Sync {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.RenamingMap")
    }
}
impl RecordValue for dyn closure_jscomp::xid::HashFunction + Send + Sync {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.Xid$HashFunction")
    }
}
impl RecordValue for dyn closure_jscomp::sorting_error_manager::ErrorReportGenerator + Send {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.SortingErrorManager$ErrorReportGenerator")
    }
}
impl RecordValue for dyn closure_jscomp::error_handler::ErrorHandler + Send {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.ErrorHandler")
    }
}
impl RecordValue for dyn closure_jscomp::source_map::LocationMapping + Send + Sync {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.SourceMap$LocationMapping")
    }
}
impl RecordValue for closure_jscomp::variable_map::VariableMap {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.VariableMap")
    }
}
impl RecordValue for closure_jscomp::conformance_config::ConformanceConfig {
    // port: UnitRecorder#value (MessageOrBuilder)
    fn record(&self) -> Value {
        closure_testing::replay::proto_values::encode_message(self).to_json()
    }
}
impl RecordValue for CompilerPassRef {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.CompilerPass")
    }
}
impl RecordValue for closure_jscomp::source_map_input::SourceMapInput {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("com.google.javascript.jscomp.SourceMapInput")
    }
}
impl RecordValue for closure_rhino::java_lang::charset::Charset {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        standin_class("java.nio.charset.Charset")
    }
}
impl RecordValue for OutputStreamWrapper {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        object([("unrepresentable", Value::str("java.util.function.Function"))])
    }
}
impl RecordValue for InputStreamWrapper {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        object([("unrepresentable", Value::str("java.util.function.Function"))])
    }
}
// port: UnitRecorder#fields
pub fn options_fields(options: &CompilerOptions) -> Value {
    object([
        (
            "emitUseStrict",
            guava_optional(options.__field_emit_use_strict()),
        ),
        ("languageIn", options.get_language_in().record()),
        (
            "outputFeatureSet",
            guava_optional(options.__field_output_feature_set()),
        ),
        (
            "experimentalOutputFeatureSet",
            guava_optional(&options.get_experimental_output_feature_set()),
        ),
        (
            "languageOutIsDefaultStrict",
            guava_optional(options.__field_language_out_is_default_strict()),
        ),
        ("environment", options.get_environment().record()),
        (
            "browserFeaturesetYear",
            options.get_browser_featureset_year_object().record(),
        ),
        (
            "instrumentForCoverageOnly",
            options.get_instrument_for_coverage_only().record(),
        ),
        (
            "typedAstOutputFile",
            options.get_typed_ast_output_file().record(),
        ),
        (
            "mergedPrecompiledLibraries",
            options.get_merged_precompiled_libraries().record(),
        ),
        ("inferConsts", options.should_infer_consts().record()),
        ("assumeStrictThis", options.assume_strict_this().record()),
        (
            "preserveDetailedSourceInfo",
            options.preserves_detailed_source_info().record(),
        ),
        (
            "preserveNonJSDocComments",
            options.get_preserve_non_jsdoc_comments().record(),
        ),
        (
            "continueAfterErrors",
            options.can_continue_after_errors().record(),
        ),
        (
            "incrementalCheckMode",
            options.__field_incremental_check_mode().record(),
        ),
        (
            "parseJsDocDocumentation",
            options.is_parse_js_doc_documentation().record(),
        ),
        ("printExterns", options.__field_print_externs().record()),
        ("inferTypes", options.get_infer_types().record()),
        (
            "skipNonTranspilationPasses",
            options.get_skip_non_transpilation_passes().record(),
        ),
        ("devMode", options.get_dev_mode().record()),
        ("checkDeterminism", options.get_check_determinism().record()),
        (
            "dependencyOptions",
            options.get_dependency_options().record(),
        ),
        ("messageBundle", options.get_message_bundle().record()),
        (
            "strictMessageReplacement",
            options.get_strict_message_replacement().record(),
        ),
        ("checkSymbols", options.get_check_symbols().record()),
        (
            "checkSuspiciousCode",
            options.get_check_suspicious_code().record(),
        ),
        ("checkTypes", options.get_check_types().record()),
        (
            "extraAnnotationNames",
            options.get_extra_annotation_names().record(),
        ),
        (
            "numParallelThreads",
            options.get_num_parallel_threads().record(),
        ),
        ("foldConstants", options.should_fold_constants().record()),
        (
            "deadAssignmentElimination",
            options.should_run_dead_assignment_elimination().record(),
        ),
        (
            "deadPropertyAssignmentElimination",
            options
                .__field_dead_property_assignment_elimination()
                .record(),
        ),
        (
            "inlineConstantVars",
            options.should_inline_constant_vars().record(),
        ),
        (
            "maxFunctionSizeAfterInlining",
            options.get_max_function_size_after_inlining().record(),
        ),
        (
            "assumeClosuresOnlyCaptureReferences",
            options.assume_closures_only_capture_references().record(),
        ),
        (
            "inlineProperties",
            options.should_inline_properties().record(),
        ),
        (
            "crossChunkCodeMotion",
            options.should_run_cross_chunk_code_motion().record(),
        ),
        (
            "crossChunkCodeMotionNoStubMethods",
            options
                .get_cross_chunk_code_motion_no_stub_methods()
                .record(),
        ),
        (
            "parentChunkCanSeeSymbolsDeclaredInChildren",
            options
                .get_parent_chunk_can_see_symbols_declared_in_children()
                .record(),
        ),
        (
            "coalesceVariableNames",
            options.should_coalesce_variable_names().record(),
        ),
        (
            "optimizeLetAndConst",
            options.should_optimize_let_and_const().record(),
        ),
        (
            "assumeGlobalScopeIsIsolated",
            options.__field_assume_global_scope_is_isolated().record(),
        ),
        (
            "crossChunkMethodMotion",
            options.get_cross_chunk_method_motion().record(),
        ),
        ("inlineGetters", options.should_inline_getters().record()),
        (
            "inlineVariables",
            options.should_inline_variables().record(),
        ),
        (
            "inlineLocalVariables",
            options.should_inline_local_variables().record(),
        ),
        (
            "flowSensitiveInlineVariables",
            options.get_flow_sensitive_inline_variables().record(),
        ),
        (
            "smartNameRemoval",
            options.get_smart_name_removal().record(),
        ),
        (
            "extractPrototypeMemberDeclarations",
            options
                .get_extract_prototype_member_declarations_mode()
                .record(),
        ),
        (
            "removeUnusedPrototypeProperties",
            options.should_remove_unused_prototype_properties().record(),
        ),
        (
            "removeUnusedClassProperties",
            options.is_remove_unused_class_properties().record(),
        ),
        (
            "removeUnusedVars",
            options.should_remove_unused_variables().record(),
        ),
        (
            "removeUnusedLocalVars",
            options.should_remove_unused_local_variables().record(),
        ),
        (
            "collapseVariableDeclarations",
            options.should_collapse_variable_declarations().record(),
        ),
        (
            "collapseAnonymousFunctions",
            options.should_collapse_anonymous_functions().record(),
        ),
        (
            "aliasStringsMode",
            options.get_alias_strings_mode().record(),
        ),
        (
            "outputJsStringUsage",
            options.should_output_js_string_usage().record(),
        ),
        (
            "convertToDottedProperties",
            options.should_convert_to_dotted_properties().record(),
        ),
        (
            "rewriteFunctionExpressions",
            options.should_rewrite_function_expressions().record(),
        ),
        ("optimizeCalls", options.should_optimize_calls().record()),
        (
            "optimizeESClassConstructors",
            options.get_optimize_es_class_constructors().record(),
        ),
        (
            "useTypesForLocalOptimization",
            options.should_use_types_for_local_optimization().record(),
        ),
        (
            "useSizeHeuristicToStopOptimizationLoop",
            options
                .should_use_size_heuristic_to_stop_optimization_loop()
                .record(),
        ),
        (
            "optimizationLoopMaxIterations",
            options.get_max_optimization_loop_iterations().record(),
        ),
        ("variableRenaming", options.get_variable_renaming().record()),
        ("propertyRenaming", options.get_property_renaming().record()),
        (
            "propertyRenamingOnlyCompilationMode",
            options
                .is_property_renaming_only_compilation_mode()
                .record(),
        ),
        ("labelRenaming", options.should_rename_labels().record()),
        (
            "reserveRawExports",
            options.should_reserve_raw_exports().record(),
        ),
        (
            "preferStableNames",
            options.should_prefer_stable_names().record(),
        ),
        (
            "generatePseudoNames",
            options.should_generate_pseudo_names().record(),
        ),
        ("renamePrefix", options.get_rename_prefix().record()),
        (
            "renamePrefixNamespace",
            options.get_rename_prefix_namespace().record(),
        ),
        (
            "renamePrefixNamespaceAssumeCrossChunkNames",
            options
                .assume_cross_chunk_names_for_rename_prefix_namespace()
                .record(),
        ),
        (
            "optimizeLocalAccessForGlobalSymbolNamespace",
            options
                .get_optimize_local_access_for_global_symbol_namespace()
                .record(),
        ),
        (
            "collapsePropertiesLevel",
            options.get_collapse_properties_level().record(),
        ),
        (
            "collapseObjectLiterals",
            options.get_collapse_object_literals().record(),
        ),
        (
            "devirtualizeMethods",
            options.should_devirtualize_methods().record(),
        ),
        (
            "computeFunctionSideEffects",
            options.should_compute_function_side_effects().record(),
        ),
        (
            "disambiguateProperties",
            options.should_disambiguate_properties().record(),
        ),
        (
            "ambiguateProperties",
            options.should_ambiguate_properties().record(),
        ),
        ("inputSourceMaps", options.get_input_source_maps().record()),
        (
            "inputVariableMap",
            options.get_input_variable_map().record(),
        ),
        (
            "inputPropertyMap",
            options.get_input_property_map().record(),
        ),
        (
            "exportTestFunctions",
            options.should_export_test_functions().record(),
        ),
        ("nameGenerator", options.get_name_generator().record()),
        (
            "replaceMessagesWithChromeI18n",
            options.__field_replace_messages_with_chrome_i18n().record(),
        ),
        ("tcProjectId", options.get_tc_project_id().record()),
        ("codingConvention", options.get_coding_convention().record()),
        (
            "syntheticBlockStartMarker",
            options.get_synthetic_block_start_marker().record(),
        ),
        (
            "syntheticBlockEndMarker",
            options.get_synthetic_block_end_marker().record(),
        ),
        ("locale", options.get_locale().record()),
        (
            "doLateLocalization",
            options.do_late_localization().record(),
        ),
        ("markAsCompiled", options.should_mark_as_compiled().record()),
        ("closurePass", options.get_closure_pass().record()),
        (
            "preserveClosurePrimitives",
            options.should_preserve_goog_library_primitives().record(),
        ),
        ("angularPass", options.get_angular_pass().record()),
        ("polymerPass", options.is_polymer_pass_enabled().record()),
        ("chromePass", options.is_chrome_pass_enabled().record()),
        ("j2clPassMode", options.get_j2cl_pass().record()),
        (
            "j2clMinifierEnabled",
            options.is_j2cl_minifier_enabled().record(),
        ),
        (
            "j2clMinifierPruningManifest",
            options.get_j2cl_minifier_pruning_manifest().record(),
        ),
        (
            "removeAbstractMethods",
            options.should_remove_abstract_methods().record(),
        ),
        (
            "removeClosureAsserts",
            options.should_remove_closure_asserts().record(),
        ),
        (
            "removeJ2clAsserts",
            options.should_remove_j2cl_asserts().record(),
        ),
        ("gatherCssNames", options.should_gather_css_names().record()),
        ("stripTypes", options.get_strip_types().record()),
        (
            "stripNameSuffixes",
            options.get_strip_name_suffixes().record(),
        ),
        (
            "stripNamePrefixes",
            options.get_strip_name_prefixes().record(),
        ),
        ("customPasses", options.__field_custom_passes().record()),
        (
            "defineReplacements",
            implementation(
                options.__field_define_replacements().record(),
                "java.util.LinkedHashMap",
            ),
        ),
        ("tweakProcessing", options.get_tweak_processing().record()),
        (
            "rewriteGlobalDeclarationsForTryCatchWrapping",
            options
                .should_rewrite_global_declarations_for_try_catch_wrapping()
                .record(),
        ),
        ("checksOnly", options.is_checks_only().record()),
        ("outputJs", options.get_output_js().record()),
        (
            "generateExports",
            options.should_generate_exports().record(),
        ),
        (
            "exportLocalPropertyDefinitions",
            options.should_export_local_property_definitions().record(),
        ),
        ("cssRenamingMap", options.get_css_renaming_map().record()),
        (
            "cssRenamingSkiplist",
            options.get_css_renaming_skiplist().record(),
        ),
        (
            "replaceIdGenerators",
            options.should_replace_id_generators().record(),
        ),
        ("idGenerators", options.get_id_generators().record()),
        ("xidHashFunction", options.get_xid_hash_function().record()),
        (
            "chunkIdHashFunction",
            options.get_chunk_id_hash_function().record(),
        ),
        (
            "idGeneratorsMapSerialized",
            options.get_id_generators_map_serialized().record(),
        ),
        (
            "replaceStringsFunctionDescriptions",
            options.get_replace_strings_function_descriptions().record(),
        ),
        (
            "replaceStringsPlaceholderToken",
            options.get_replace_strings_placeholder_token().record(),
        ),
        (
            "propertiesThatMustDisambiguate",
            options.get_properties_that_must_disambiguate().record(),
        ),
        (
            "validateRequireInliningAnnotation",
            options.get_should_validate_required_inlinings().record(),
        ),
        (
            "processCommonJSModules",
            options.get_process_common_js_modules().record(),
        ),
        ("moduleRoots", options.get_module_roots().record()),
        ("rewritePolyfills", options.get_rewrite_polyfills().record()),
        ("isolatePolyfills", options.get_isolate_polyfills().record()),
        (
            "injectPolyfillsNewerThan",
            options.get_inject_polyfills_newer_than().record(),
        ),
        (
            "es6SubclassTranspilation",
            options.get_es6_subclass_transpilation().record(),
        ),
        (
            "instrumentAsyncContext",
            options.get_instrument_async_context().record(),
        ),
        (
            "forceLibraryInjection",
            options.get_force_library_injection_list().record(),
        ),
        (
            "runtimeLibraryMode",
            options.get_runtime_library_mode().record(),
        ),
        (
            "assumeForwardDeclaredForMissingTypes",
            options.assume_forward_declared_for_missing_types().record(),
        ),
        (
            "unusedImportsToRemove",
            options.get_unused_imports_to_remove().record(),
        ),
        (
            "preserveTypeAnnotations",
            options.should_preserve_type_annotations().record(),
        ),
        ("gentsMode", options.get_gents_mode().record()),
        ("prettyPrint", options.is_pretty_print().record()),
        ("lineBreak", options.should_add_line_break().record()),
        (
            "printInputDelimiter",
            options.should_print_input_delimiter().record(),
        ),
        ("inputDelimiter", options.get_input_delimiter().record()),
        (
            "debugLogDirectory",
            options.get_debug_log_directory().record(),
        ),
        ("debugLogFilter", options.get_debug_log_filter().record()),
        (
            "serializeExtraDebugInfo",
            options.__field_serialize_extra_debug_info().record(),
        ),
        (
            "stateCompressionWrapper",
            options.get_state_compression_wrapper().record(),
        ),
        (
            "stateDecompressionWrapper",
            options.get_state_decompression_wrapper().record(),
        ),
        (
            "quoteKeywordProperties",
            options.__field_quote_keyword_properties().record(),
        ),
        (
            "preferSingleQuotes",
            options.should_prefer_single_quotes().record(),
        ),
        ("trustedStrings", options.assume_trusted_strings().record()),
        (
            "printSourceAfterEachPass",
            options.should_print_source_after_each_pass().record(),
        ),
        (
            "filesToPrintAfterEachPassRegexList",
            options
                .get_files_to_print_after_each_pass_regex_list()
                .record(),
        ),
        (
            "chunksToPrintAfterEachPassRegexList",
            options
                .get_chunks_to_print_after_each_pass_regex_list()
                .record(),
        ),
        (
            "qnameUsesToPrintAfterEachPassList",
            options
                .get_qname_uses_to_print_after_each_pass_list()
                .record(),
        ),
        ("tracer", options.get_tracer_mode().record()),
        ("tracerOutput", options.get_tracer_output().record()),
        (
            "colorizeErrorOutput",
            options.should_colorize_error_output().record(),
        ),
        ("errorFormat", options.get_error_format().record()),
        ("warningsGuard", options.get_warnings_guard().record()),
        (
            "summaryDetailLevel",
            options.get_summary_detail_level().record(),
        ),
        (
            "lineLengthThreshold",
            options.get_line_length_threshold().record(),
        ),
        (
            "useOriginalNamesInOutput",
            options.get_use_original_names_in_output().record(),
        ),
        (
            "externExportsPath",
            options.get_extern_exports_path().record(),
        ),
        (
            "extraReportGenerators",
            implementation(
                options.get_extra_report_generators().record(),
                "java.util.ArrayList",
            ),
        ),
        (
            "sourceMapOutputPath",
            options.get_source_map_output_path().record(),
        ),
        (
            "shouldAlwaysGatherSourceMapInfo",
            options
                .__field_should_always_gather_source_map_info()
                .record(),
        ),
        (
            "sourceMapDetailLevel",
            options.get_source_map_detail_level().record(),
        ),
        ("sourceMapFormat", options.get_source_map_format().record()),
        (
            "parseInlineSourceMaps",
            options.get_parse_inline_source_maps().record(),
        ),
        (
            "applyInputSourceMaps",
            options.get_apply_input_source_maps().record(),
        ),
        (
            "resolveSourceMapAnnotations",
            options.get_resolve_source_map_annotations().record(),
        ),
        (
            "sourceMapLocationMappings",
            options.get_source_map_location_mappings().record(),
        ),
        (
            "sourceMapIncludeSourcesContent",
            options.get_source_map_include_sources_content().record(),
        ),
        ("outputCharset", options.get_output_charset().record()),
        (
            "protectHiddenSideEffects",
            options.__field_protect_hidden_side_effects().record(),
        ),
        (
            "assumeGettersArePure",
            options.get_assume_getters_are_pure().record(),
        ),
        (
            "assumePropertiesAreStaticallyAnalyzable",
            options
                .get_assume_properties_are_statically_analyzable()
                .record(),
        ),
        (
            "assumeStaticInheritanceIsNotUsed",
            options.get_assume_static_inheritance_is_not_used().record(),
        ),
        ("errorHandler", options.get_error_handler().record()),
        (
            "instrumentForCoverageOption",
            options.get_instrument_for_coverage_option().record(),
        ),
        (
            "productionInstrumentationArrayName",
            options.get_production_instrumentation_array_name().record(),
        ),
        (
            "conformanceConfigs",
            options.get_conformance_configs().record(),
        ),
        (
            "conformanceReportingMode",
            options.get_conformance_reporting_mode().record(),
        ),
        (
            "conformanceRemoveRegexFromPath",
            guava_optional(options.get_conformance_remove_regex_from_path()),
        ),
        (
            "wrapGoogModulesForWhitespaceOnly",
            options
                .should_wrap_goog_modules_for_whitespace_only()
                .record(),
        ),
        ("printConfig", options.should_print_config().record()),
        (
            "isStrictModeInput",
            guava_optional(options.__field_is_strict_mode_input()),
        ),
        (
            "rewriteModulesBeforeTypechecking",
            options
                .__field_rewrite_modules_before_typechecking()
                .record(),
        ),
        (
            "enableModuleRewriting",
            options.should_rewrite_modules().record(),
        ),
        (
            "moduleResolutionMode",
            options.get_module_resolution_mode().record(),
        ),
        (
            "browserResolverPrefixReplacements",
            options.get_browser_resolver_prefix_replacements().record(),
        ),
        ("pathEscaper", options.get_path_escaper().record()),
        (
            "packageJsonEntryNames",
            options.get_package_json_entry_names().record(),
        ),
        (
            "allowDynamicImport",
            options.should_allow_dynamic_import().record(),
        ),
        (
            "dynamicImportAlias",
            options.get_dynamic_import_alias().record(),
        ),
        ("chunkOutputType", options.get_chunk_output_type().record()),
        (
            "unknownDefinesToIgnore",
            options.get_unknown_defines_to_ignore().record(),
        ),
        (
            "inlineFunctionsLevel",
            options.get_inline_functions_level().record(),
        ),
        (
            "enableZonesDefineName",
            options.get_enable_zones_define_name().record(),
        ),
        (
            "zoneInputPattern",
            options.get_zone_input_pattern().record(),
        ),
        (
            "es6ModuleTranspilation",
            options.get_es6_module_transpilation().record(),
        ),
    ])
}
impl RecordValue for BrowserFeaturesetYear {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$BrowserFeaturesetYear",
            &self.to_string(),
        )
    }
}
impl RecordValue for ExperimentalOutputFeatureSet {
    // port: UnitRecorder#value
    fn record(&self) -> Value {
        enum_value(
            "com.google.javascript.jscomp.CompilerOptions$ExperimentalOutputFeatureSet",
            &self.to_string(),
        )
    }
}

impl<T: RecordValue + ?Sized> RecordValue for std::sync::Mutex<T> {
    fn record(&self) -> Value {
        self.lock().unwrap().record()
    }
}
