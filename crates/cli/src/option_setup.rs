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
// Ported from closure-rs' own Java oracle tooling: crates/cli/tools/CliOptionSetup.java.

use closure_jscomp::coding_convention::CodingConvention;
use closure_jscomp::compiler_options::*;
use closure_jscomp::dependency_options::DependencyOptions;
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{java_lang::charset::Charset, js_string::JsString};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
// port: CliOptionSetup#normalize
pub trait ValueForSetup {
    fn setup_json(&self) -> Value;
}
impl<T: ValueForSetup + ?Sized> ValueForSetup for &T {
    fn setup_json(&self) -> Value {
        (*self).setup_json()
    }
}
impl<T: ValueForSetup + ?Sized> ValueForSetup for Arc<T> {
    fn setup_json(&self) -> Value {
        self.as_ref().setup_json()
    }
}
impl<T: ValueForSetup> ValueForSetup for Option<T> {
    fn setup_json(&self) -> Value {
        self.as_ref().map_or(Value::Null, ValueForSetup::setup_json)
    }
}
impl<T: ValueForSetup> ValueForSetup for Vec<T> {
    fn setup_json(&self) -> Value {
        Value::Array(self.iter().map(ValueForSetup::setup_json).collect())
    }
}
impl<T: ValueForSetup> ValueForSetup for IndexSet<T> {
    fn setup_json(&self) -> Value {
        Value::Array(self.iter().map(ValueForSetup::setup_json).collect())
    }
}
impl<T: ValueForSetup> ValueForSetup for IndexMap<String, T> {
    fn setup_json(&self) -> Value {
        Value::Object(
            self.iter()
                .map(|(k, v)| (k.clone(), v.setup_json()))
                .collect(),
        )
    }
}
macro_rules! primitive { ($($type:ty),*)=>{$(impl ValueForSetup for $type{ fn setup_json(&self)->Value{json!(self)}})*}; }
primitive!(bool, i32, str, String, f64);
impl ValueForSetup for JsString {
    fn setup_json(&self) -> Value {
        json!(self.to_string_lossy())
    }
}
impl ValueForSetup for Path {
    fn setup_json(&self) -> Value {
        json!(self.to_string_lossy())
    }
}
impl ValueForSetup for PathBuf {
    fn setup_json(&self) -> Value {
        self.as_path().setup_json()
    }
}
impl ValueForSetup for FeatureSet {
    fn setup_json(&self) -> Value {
        json!(self.to_string())
    }
}
impl ValueForSetup for BrowserFeaturesetYear {
    fn setup_json(&self) -> Value {
        json!(format!("YEAR_{}", self.get_year()))
    }
}
impl ValueForSetup for Charset {
    fn setup_json(&self) -> Value {
        json!(self.name())
    }
}
impl ValueForSetup for DependencyOptions {
    fn setup_json(&self) -> Value {
        json!({"mode":self.mode().to_string(),"entryPoints":self.entry_points().iter().map(ToString::to_string).collect::<Vec<_>>()})
    }
}
impl ValueForSetup for DefineValue {
    fn setup_json(&self) -> Value {
        match self {
            Self::Boolean(v) => v.setup_json(),
            Self::Integer(v) => v.setup_json(),
            Self::Double(v) => v.setup_json(),
            Self::String(v) => v.setup_json(),
        }
    }
}
// port: CliOptionSetup#normalize
pub fn coding_convention_class_name(value: &dyn CodingConvention) -> &'static str {
    let debug = format!("{value:?}");
    if debug.starts_with("ClosureCodingConvention") {
        "ClosureCodingConvention"
    } else if debug.starts_with("ChromeCodingConvention") {
        "ChromeCodingConvention"
    } else {
        "DefaultCodingConvention"
    }
}
impl ValueForSetup for dyn CodingConvention + Send + Sync {
    fn setup_json(&self) -> Value {
        json!(coding_convention_class_name(self))
    }
}
macro_rules! enums { ($($type:ty),*)=>{$(impl ValueForSetup for $type{fn setup_json(&self)->Value{json!(format!("{self:?}"))}})*}; }
enums!(
    LanguageMode,
    Environment,
    IncrementalCheckMode,
    DevMode,
    ExtractPrototypeMemberDeclarationsMode,
    AliasStringsMode,
    OptimizeLocalAccess,
    PropertyCollapseLevel,
    J2clPassMode,
    TweakProcessing,
    OutputJs,
    Es6SubclassTranspilation,
    TracerMode,
    InstrumentOption,
    ConformanceReportingMode,
    ChunkOutputType,
    Reach,
    Es6ModuleTranspilation,
    closure_parsing::config::JsDocParsing,
    closure_rhino::jscomp_base::Tri,
    closure_jscomp::variable_renaming_policy::VariableRenamingPolicy,
    closure_jscomp::property_renaming_policy::PropertyRenamingPolicy,
    closure_jscomp::js::runtime_js_lib_manager::RuntimeLibraryMode,
    closure_jscomp::error_format::ErrorFormat,
    closure_jscomp::source_map::DetailLevel,
    closure_jscomp::source_map::Format,
    closure_jscomp::deps::module_loader::ResolutionMode,
    closure_jscomp::deps::module_loader::PathEscaper
);
fn guava_optional<T: ValueForSetup>(value: &Option<T>) -> Value {
    value.setup_json()
}
fn implementation(value: Value, _class: &str) -> Value {
    value
}
// port: CliOptionSetup#fields
pub fn options_fields(
    options: &CompilerOptions,
    config: Option<&crate::abstract_command_line_runner::CommandLineConfig>,
) -> Value {
    Value::Object(
        [
            (
                "emitUseStrict".into(),
                guava_optional(options.__field_emit_use_strict()),
            ),
            ("languageIn".into(), options.get_language_in().setup_json()),
            (
                "outputFeatureSet".into(),
                guava_optional(options.__field_output_feature_set()),
            ),
            (
                "languageOutIsDefaultStrict".into(),
                guava_optional(options.__field_language_out_is_default_strict()),
            ),
            ("environment".into(), options.get_environment().setup_json()),
            (
                "browserFeaturesetYear".into(),
                options.get_browser_featureset_year_object().setup_json(),
            ),
            (
                "instrumentForCoverageOnly".into(),
                options.get_instrument_for_coverage_only().setup_json(),
            ),
            (
                "typedAstOutputFile".into(),
                options.get_typed_ast_output_file().setup_json(),
            ),
            (
                "mergedPrecompiledLibraries".into(),
                options.get_merged_precompiled_libraries().setup_json(),
            ),
            (
                "inferConsts".into(),
                options.should_infer_consts().setup_json(),
            ),
            (
                "assumeStrictThis".into(),
                options.assume_strict_this().setup_json(),
            ),
            (
                "preserveDetailedSourceInfo".into(),
                options.preserves_detailed_source_info().setup_json(),
            ),
            (
                "preserveNonJSDocComments".into(),
                options.get_preserve_non_jsdoc_comments().setup_json(),
            ),
            (
                "continueAfterErrors".into(),
                options.can_continue_after_errors().setup_json(),
            ),
            (
                "incrementalCheckMode".into(),
                options.__field_incremental_check_mode().setup_json(),
            ),
            (
                "parseJsDocDocumentation".into(),
                options.is_parse_js_doc_documentation().setup_json(),
            ),
            (
                "printExterns".into(),
                options.__field_print_externs().setup_json(),
            ),
            ("inferTypes".into(), options.get_infer_types().setup_json()),
            (
                "skipNonTranspilationPasses".into(),
                options.get_skip_non_transpilation_passes().setup_json(),
            ),
            ("devMode".into(), options.get_dev_mode().setup_json()),
            (
                "checkDeterminism".into(),
                options.get_check_determinism().setup_json(),
            ),
            (
                "dependencyOptions".into(),
                options.get_dependency_options().setup_json(),
            ),
            (
                "strictMessageReplacement".into(),
                options.get_strict_message_replacement().setup_json(),
            ),
            (
                "checkSymbols".into(),
                options.get_check_symbols().setup_json(),
            ),
            (
                "checkSuspiciousCode".into(),
                options.get_check_suspicious_code().setup_json(),
            ),
            ("checkTypes".into(), options.get_check_types().setup_json()),
            (
                "extraAnnotationNames".into(),
                options.get_extra_annotation_names().setup_json(),
            ),
            (
                "numParallelThreads".into(),
                options.get_num_parallel_threads().setup_json(),
            ),
            (
                "foldConstants".into(),
                options.should_fold_constants().setup_json(),
            ),
            (
                "deadAssignmentElimination".into(),
                options
                    .should_run_dead_assignment_elimination()
                    .setup_json(),
            ),
            (
                "deadPropertyAssignmentElimination".into(),
                options
                    .__field_dead_property_assignment_elimination()
                    .setup_json(),
            ),
            (
                "inlineConstantVars".into(),
                options.should_inline_constant_vars().setup_json(),
            ),
            (
                "maxFunctionSizeAfterInlining".into(),
                options.get_max_function_size_after_inlining().setup_json(),
            ),
            (
                "assumeClosuresOnlyCaptureReferences".into(),
                options
                    .assume_closures_only_capture_references()
                    .setup_json(),
            ),
            (
                "inlineProperties".into(),
                options.should_inline_properties().setup_json(),
            ),
            (
                "crossChunkCodeMotion".into(),
                options.should_run_cross_chunk_code_motion().setup_json(),
            ),
            (
                "crossChunkCodeMotionNoStubMethods".into(),
                options
                    .get_cross_chunk_code_motion_no_stub_methods()
                    .setup_json(),
            ),
            (
                "parentChunkCanSeeSymbolsDeclaredInChildren".into(),
                options
                    .get_parent_chunk_can_see_symbols_declared_in_children()
                    .setup_json(),
            ),
            (
                "coalesceVariableNames".into(),
                options.should_coalesce_variable_names().setup_json(),
            ),
            (
                "optimizeLetAndConst".into(),
                options.should_optimize_let_and_const().setup_json(),
            ),
            (
                "assumeGlobalScopeIsIsolated".into(),
                options
                    .__field_assume_global_scope_is_isolated()
                    .setup_json(),
            ),
            (
                "crossChunkMethodMotion".into(),
                options.get_cross_chunk_method_motion().setup_json(),
            ),
            (
                "inlineGetters".into(),
                options.should_inline_getters().setup_json(),
            ),
            (
                "inlineVariables".into(),
                options.should_inline_variables().setup_json(),
            ),
            (
                "inlineLocalVariables".into(),
                options.should_inline_local_variables().setup_json(),
            ),
            (
                "flowSensitiveInlineVariables".into(),
                options.get_flow_sensitive_inline_variables().setup_json(),
            ),
            (
                "smartNameRemoval".into(),
                options.get_smart_name_removal().setup_json(),
            ),
            (
                "extractPrototypeMemberDeclarations".into(),
                options
                    .get_extract_prototype_member_declarations_mode()
                    .setup_json(),
            ),
            (
                "removeUnusedPrototypeProperties".into(),
                options
                    .should_remove_unused_prototype_properties()
                    .setup_json(),
            ),
            (
                "removeUnusedClassProperties".into(),
                options.is_remove_unused_class_properties().setup_json(),
            ),
            (
                "removeUnusedVars".into(),
                options.should_remove_unused_variables().setup_json(),
            ),
            (
                "removeUnusedLocalVars".into(),
                options.should_remove_unused_local_variables().setup_json(),
            ),
            (
                "collapseVariableDeclarations".into(),
                options.should_collapse_variable_declarations().setup_json(),
            ),
            (
                "collapseAnonymousFunctions".into(),
                options.should_collapse_anonymous_functions().setup_json(),
            ),
            (
                "aliasStringsMode".into(),
                options.get_alias_strings_mode().setup_json(),
            ),
            (
                "outputJsStringUsage".into(),
                options.should_output_js_string_usage().setup_json(),
            ),
            (
                "convertToDottedProperties".into(),
                options.should_convert_to_dotted_properties().setup_json(),
            ),
            (
                "rewriteFunctionExpressions".into(),
                options.should_rewrite_function_expressions().setup_json(),
            ),
            (
                "optimizeCalls".into(),
                options.should_optimize_calls().setup_json(),
            ),
            (
                "optimizeESClassConstructors".into(),
                options.get_optimize_es_class_constructors().setup_json(),
            ),
            (
                "useTypesForLocalOptimization".into(),
                options
                    .should_use_types_for_local_optimization()
                    .setup_json(),
            ),
            (
                "useSizeHeuristicToStopOptimizationLoop".into(),
                options
                    .should_use_size_heuristic_to_stop_optimization_loop()
                    .setup_json(),
            ),
            (
                "optimizationLoopMaxIterations".into(),
                options.get_max_optimization_loop_iterations().setup_json(),
            ),
            (
                "variableRenaming".into(),
                options.get_variable_renaming().setup_json(),
            ),
            (
                "propertyRenaming".into(),
                options.get_property_renaming().setup_json(),
            ),
            (
                "propertyRenamingOnlyCompilationMode".into(),
                options
                    .is_property_renaming_only_compilation_mode()
                    .setup_json(),
            ),
            (
                "labelRenaming".into(),
                options.should_rename_labels().setup_json(),
            ),
            (
                "reserveRawExports".into(),
                options.should_reserve_raw_exports().setup_json(),
            ),
            (
                "preferStableNames".into(),
                options.should_prefer_stable_names().setup_json(),
            ),
            (
                "generatePseudoNames".into(),
                options.should_generate_pseudo_names().setup_json(),
            ),
            (
                "renamePrefix".into(),
                options.get_rename_prefix().setup_json(),
            ),
            (
                "renamePrefixNamespace".into(),
                options.get_rename_prefix_namespace().setup_json(),
            ),
            (
                "renamePrefixNamespaceAssumeCrossChunkNames".into(),
                options
                    .assume_cross_chunk_names_for_rename_prefix_namespace()
                    .setup_json(),
            ),
            (
                "optimizeLocalAccessForGlobalSymbolNamespace".into(),
                options
                    .get_optimize_local_access_for_global_symbol_namespace()
                    .setup_json(),
            ),
            (
                "collapsePropertiesLevel".into(),
                options.get_collapse_properties_level().setup_json(),
            ),
            (
                "collapseObjectLiterals".into(),
                options.get_collapse_object_literals().setup_json(),
            ),
            (
                "devirtualizeMethods".into(),
                options.should_devirtualize_methods().setup_json(),
            ),
            (
                "computeFunctionSideEffects".into(),
                options.should_compute_function_side_effects().setup_json(),
            ),
            (
                "disambiguateProperties".into(),
                options.should_disambiguate_properties().setup_json(),
            ),
            (
                "ambiguateProperties".into(),
                options.should_ambiguate_properties().setup_json(),
            ),
            (
                "exportTestFunctions".into(),
                options.should_export_test_functions().setup_json(),
            ),
            (
                "replaceMessagesWithChromeI18n".into(),
                options
                    .__field_replace_messages_with_chrome_i18n()
                    .setup_json(),
            ),
            (
                "tcProjectId".into(),
                options.get_tc_project_id().setup_json(),
            ),
            (
                "codingConvention".into(),
                options.get_coding_convention().setup_json(),
            ),
            (
                "syntheticBlockStartMarker".into(),
                options.get_synthetic_block_start_marker().setup_json(),
            ),
            (
                "syntheticBlockEndMarker".into(),
                options.get_synthetic_block_end_marker().setup_json(),
            ),
            ("locale".into(), options.get_locale().setup_json()),
            (
                "doLateLocalization".into(),
                options.do_late_localization().setup_json(),
            ),
            (
                "markAsCompiled".into(),
                options.should_mark_as_compiled().setup_json(),
            ),
            (
                "closurePass".into(),
                options.get_closure_pass().setup_json(),
            ),
            (
                "preserveClosurePrimitives".into(),
                options
                    .should_preserve_goog_library_primitives()
                    .setup_json(),
            ),
            (
                "angularPass".into(),
                options.get_angular_pass().setup_json(),
            ),
            (
                "polymerPass".into(),
                options.is_polymer_pass_enabled().setup_json(),
            ),
            (
                "chromePass".into(),
                options.is_chrome_pass_enabled().setup_json(),
            ),
            ("j2clPassMode".into(), options.get_j2cl_pass().setup_json()),
            (
                "j2clMinifierEnabled".into(),
                options.is_j2cl_minifier_enabled().setup_json(),
            ),
            (
                "j2clMinifierPruningManifest".into(),
                options.get_j2cl_minifier_pruning_manifest().setup_json(),
            ),
            (
                "removeAbstractMethods".into(),
                options.should_remove_abstract_methods().setup_json(),
            ),
            (
                "removeClosureAsserts".into(),
                options.should_remove_closure_asserts().setup_json(),
            ),
            (
                "removeJ2clAsserts".into(),
                options.should_remove_j2cl_asserts().setup_json(),
            ),
            (
                "gatherCssNames".into(),
                options.should_gather_css_names().setup_json(),
            ),
            ("stripTypes".into(), options.get_strip_types().setup_json()),
            (
                "stripNameSuffixes".into(),
                options.get_strip_name_suffixes().setup_json(),
            ),
            (
                "stripNamePrefixes".into(),
                options.get_strip_name_prefixes().setup_json(),
            ),
            (
                "defineReplacements".into(),
                implementation(
                    options.__field_define_replacements().setup_json(),
                    "java.util.LinkedHashMap",
                ),
            ),
            (
                "tweakProcessing".into(),
                options.get_tweak_processing().setup_json(),
            ),
            (
                "rewriteGlobalDeclarationsForTryCatchWrapping".into(),
                options
                    .should_rewrite_global_declarations_for_try_catch_wrapping()
                    .setup_json(),
            ),
            ("checksOnly".into(), options.is_checks_only().setup_json()),
            ("outputJs".into(), options.get_output_js().setup_json()),
            (
                "generateExports".into(),
                options.should_generate_exports().setup_json(),
            ),
            (
                "exportLocalPropertyDefinitions".into(),
                options
                    .should_export_local_property_definitions()
                    .setup_json(),
            ),
            (
                "cssRenamingSkiplist".into(),
                options.get_css_renaming_skiplist().setup_json(),
            ),
            (
                "replaceIdGenerators".into(),
                options.should_replace_id_generators().setup_json(),
            ),
            (
                "idGeneratorsMapSerialized".into(),
                options.get_id_generators_map_serialized().setup_json(),
            ),
            (
                "replaceStringsFunctionDescriptions".into(),
                options
                    .get_replace_strings_function_descriptions()
                    .setup_json(),
            ),
            (
                "replaceStringsPlaceholderToken".into(),
                options.get_replace_strings_placeholder_token().setup_json(),
            ),
            (
                "propertiesThatMustDisambiguate".into(),
                options.get_properties_that_must_disambiguate().setup_json(),
            ),
            (
                "validateRequireInliningAnnotation".into(),
                options
                    .get_should_validate_required_inlinings()
                    .setup_json(),
            ),
            (
                "processCommonJSModules".into(),
                options.get_process_common_js_modules().setup_json(),
            ),
            (
                "moduleRoots".into(),
                options.get_module_roots().setup_json(),
            ),
            (
                "rewritePolyfills".into(),
                options.get_rewrite_polyfills().setup_json(),
            ),
            (
                "isolatePolyfills".into(),
                options.get_isolate_polyfills().setup_json(),
            ),
            (
                "injectPolyfillsNewerThan".into(),
                options.get_inject_polyfills_newer_than().setup_json(),
            ),
            (
                "es6SubclassTranspilation".into(),
                options.get_es6_subclass_transpilation().setup_json(),
            ),
            (
                "instrumentAsyncContext".into(),
                options.get_instrument_async_context().setup_json(),
            ),
            (
                "forceLibraryInjection".into(),
                options.get_force_library_injection_list().setup_json(),
            ),
            (
                "runtimeLibraryMode".into(),
                options.get_runtime_library_mode().setup_json(),
            ),
            (
                "assumeForwardDeclaredForMissingTypes".into(),
                options
                    .assume_forward_declared_for_missing_types()
                    .setup_json(),
            ),
            (
                "unusedImportsToRemove".into(),
                options.get_unused_imports_to_remove().setup_json(),
            ),
            (
                "preserveTypeAnnotations".into(),
                options.should_preserve_type_annotations().setup_json(),
            ),
            ("gentsMode".into(), options.get_gents_mode().setup_json()),
            ("prettyPrint".into(), options.is_pretty_print().setup_json()),
            (
                "lineBreak".into(),
                options.should_add_line_break().setup_json(),
            ),
            (
                "printInputDelimiter".into(),
                options.should_print_input_delimiter().setup_json(),
            ),
            (
                "inputDelimiter".into(),
                options.get_input_delimiter().setup_json(),
            ),
            (
                "debugLogDirectory".into(),
                options.get_debug_log_directory().setup_json(),
            ),
            (
                "debugLogFilter".into(),
                options.get_debug_log_filter().setup_json(),
            ),
            (
                "serializeExtraDebugInfo".into(),
                options.__field_serialize_extra_debug_info().setup_json(),
            ),
            (
                "quoteKeywordProperties".into(),
                options.__field_quote_keyword_properties().setup_json(),
            ),
            (
                "preferSingleQuotes".into(),
                options.should_prefer_single_quotes().setup_json(),
            ),
            (
                "trustedStrings".into(),
                options.assume_trusted_strings().setup_json(),
            ),
            (
                "printSourceAfterEachPass".into(),
                options.should_print_source_after_each_pass().setup_json(),
            ),
            (
                "filesToPrintAfterEachPassRegexList".into(),
                options
                    .get_files_to_print_after_each_pass_regex_list()
                    .setup_json(),
            ),
            (
                "chunksToPrintAfterEachPassRegexList".into(),
                options
                    .get_chunks_to_print_after_each_pass_regex_list()
                    .setup_json(),
            ),
            (
                "qnameUsesToPrintAfterEachPassList".into(),
                options
                    .get_qname_uses_to_print_after_each_pass_list()
                    .setup_json(),
            ),
            ("tracer".into(), options.get_tracer_mode().setup_json()),
            (
                "tracerOutput".into(),
                options.get_tracer_output().setup_json(),
            ),
            (
                "colorizeErrorOutput".into(),
                options.should_colorize_error_output().setup_json(),
            ),
            (
                "errorFormat".into(),
                options.get_error_format().setup_json(),
            ),
            (
                "summaryDetailLevel".into(),
                options.get_summary_detail_level().setup_json(),
            ),
            (
                "lineLengthThreshold".into(),
                options.get_line_length_threshold().setup_json(),
            ),
            (
                "useOriginalNamesInOutput".into(),
                options.get_use_original_names_in_output().setup_json(),
            ),
            (
                "externExportsPath".into(),
                options.get_extern_exports_path().setup_json(),
            ),
            (
                "sourceMapOutputPath".into(),
                options.get_source_map_output_path().setup_json(),
            ),
            (
                "shouldAlwaysGatherSourceMapInfo".into(),
                options
                    .__field_should_always_gather_source_map_info()
                    .setup_json(),
            ),
            (
                "sourceMapDetailLevel".into(),
                options.get_source_map_detail_level().setup_json(),
            ),
            (
                "sourceMapFormat".into(),
                options.get_source_map_format().setup_json(),
            ),
            (
                "parseInlineSourceMaps".into(),
                options.get_parse_inline_source_maps().setup_json(),
            ),
            (
                "applyInputSourceMaps".into(),
                options.get_apply_input_source_maps().setup_json(),
            ),
            (
                "resolveSourceMapAnnotations".into(),
                options.get_resolve_source_map_annotations().setup_json(),
            ),
            (
                "sourceMapLocationMappings".into(),
                config.map_or(json!([]), |c| {
                    json!(
                        c.source_map_location_mappings
                            .iter()
                            .map(|m| json!({"prefix":m.prefix.to_string_lossy(),"replacement":m.replacement.to_string_lossy()}))
                            .collect::<Vec<_>>()
                    )
                }),
            ),
            (
                "sourceMapIncludeSourcesContent".into(),
                options
                    .get_source_map_include_sources_content()
                    .setup_json(),
            ),
            (
                "outputCharset".into(),
                options.get_output_charset().setup_json(),
            ),
            (
                "protectHiddenSideEffects".into(),
                options.__field_protect_hidden_side_effects().setup_json(),
            ),
            (
                "assumeGettersArePure".into(),
                options.get_assume_getters_are_pure().setup_json(),
            ),
            (
                "assumePropertiesAreStaticallyAnalyzable".into(),
                options
                    .get_assume_properties_are_statically_analyzable()
                    .setup_json(),
            ),
            (
                "assumeStaticInheritanceIsNotUsed".into(),
                options
                    .get_assume_static_inheritance_is_not_used()
                    .setup_json(),
            ),
            (
                "instrumentForCoverageOption".into(),
                options.get_instrument_for_coverage_option().setup_json(),
            ),
            (
                "productionInstrumentationArrayName".into(),
                options
                    .get_production_instrumentation_array_name()
                    .setup_json(),
            ),
            (
                "conformanceReportingMode".into(),
                options.get_conformance_reporting_mode().setup_json(),
            ),
            (
                "wrapGoogModulesForWhitespaceOnly".into(),
                options
                    .should_wrap_goog_modules_for_whitespace_only()
                    .setup_json(),
            ),
            (
                "printConfig".into(),
                options.should_print_config().setup_json(),
            ),
            (
                "isStrictModeInput".into(),
                guava_optional(options.__field_is_strict_mode_input()),
            ),
            (
                "rewriteModulesBeforeTypechecking".into(),
                options
                    .__field_rewrite_modules_before_typechecking()
                    .setup_json(),
            ),
            (
                "enableModuleRewriting".into(),
                options.should_rewrite_modules().setup_json(),
            ),
            (
                "moduleResolutionMode".into(),
                options.get_module_resolution_mode().setup_json(),
            ),
            (
                "browserResolverPrefixReplacements".into(),
                options
                    .get_browser_resolver_prefix_replacements()
                    .setup_json(),
            ),
            (
                "pathEscaper".into(),
                options.get_path_escaper().setup_json(),
            ),
            (
                "packageJsonEntryNames".into(),
                options.get_package_json_entry_names().setup_json(),
            ),
            (
                "allowDynamicImport".into(),
                options.should_allow_dynamic_import().setup_json(),
            ),
            (
                "dynamicImportAlias".into(),
                options.get_dynamic_import_alias().setup_json(),
            ),
            (
                "chunkOutputType".into(),
                options.get_chunk_output_type().setup_json(),
            ),
            (
                "unknownDefinesToIgnore".into(),
                options.get_unknown_defines_to_ignore().setup_json(),
            ),
            (
                "inlineFunctionsLevel".into(),
                options.get_inline_functions_level().setup_json(),
            ),
            (
                "enableZonesDefineName".into(),
                options.get_enable_zones_define_name().setup_json(),
            ),
            (
                "es6ModuleTranspilation".into(),
                options.get_es6_module_transpilation().setup_json(),
            ),
        ]
        .into_iter()
        .collect(),
    )
}
// port: CliOptionSetup#capture
pub fn diff_against_defaults(
    options: &CompilerOptions,
    config: &crate::abstract_command_line_runner::CommandLineConfig,
) -> Value {
    let defaults = options_fields(&CompilerOptions::new(), None);
    let mut values = options_fields(options, Some(config));
    values
        .as_object_mut()
        .unwrap()
        .retain(|k, v| defaults.get(k) != Some(v));
    values
}
