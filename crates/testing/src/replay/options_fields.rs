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
/*
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
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
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayOptions.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CompilerOptions.java,
//   src/com/google/javascript/jscomp/deps/ModuleLoader.java,
//   src/com/google/javascript/jscomp/parsing/Config.java.

//! Reflective replay assignment and capture of the real CompilerOptions fields.
use crate::{
    derived::OptionsDefaults,
    jscomp_api::CompilerOptions,
    json::JsonValue,
    replay::{
        options_values::OptionValue,
        replay_dsl::DslValue,
        replay_values::{decode, encode_with_template},
    },
    throwable::Throwable,
    value::Value,
};
use closure_rhino::fx_hash::IndexMap;
use std::sync::LazyLock;
// port: ReplayOptions#buildSkipping (captured Java default schema)
pub fn defaults() -> &'static OptionsDefaults {
    static DEFAULTS: LazyLock<OptionsDefaults> = LazyLock::new(|| {
        let raw = crate::json::parse_json(include_str!(
            "../../../../corpus/unit/options_defaults.json"
        ))
        .expect("options defaults JSON");
        OptionsDefaults::from_json(&raw, "$").expect("options defaults schema")
    });
    &DEFAULTS
}
// port: ReplayValues#setField (CompilerOptions reflection)
pub fn set_field(
    options: &mut CompilerOptions,
    name: &str,
    value: &Value,
    class_map: &IndexMap<String, String>,
) -> Result<(), Throwable> {
    let declared = defaults()
        .field_types
        .get(name)
        .ok_or_else(|| Throwable::HarnessError(format!("no option field {name}")))?;
    let decoded = decode(value, declared, class_map)?;
    match name {
        "emitUseStrict" => {
            *options.replay_fields_mut().emit_use_strict = OptionValue::decode_value(&decoded)?
        }
        "languageIn" => {
            *options.replay_fields_mut().language_in = OptionValue::decode_value(&decoded)?
        }
        "outputFeatureSet" => {
            *options.replay_fields_mut().output_feature_set = OptionValue::decode_value(&decoded)?
        }
        "experimentalOutputFeatureSet" => {
            *options.replay_fields_mut().experimental_output_feature_set =
                OptionValue::decode_value(&decoded)?
        }
        "languageOutIsDefaultStrict" => {
            *options.replay_fields_mut().language_out_is_default_strict =
                OptionValue::decode_value(&decoded)?
        }
        "environment" => {
            *options.replay_fields_mut().environment = OptionValue::decode_value(&decoded)?
        }
        "browserFeaturesetYear" => {
            *options.replay_fields_mut().browser_featureset_year =
                OptionValue::decode_value(&decoded)?
        }
        "instrumentForCoverageOnly" => {
            *options.replay_fields_mut().instrument_for_coverage_only =
                OptionValue::decode_value(&decoded)?
        }
        "typedAstOutputFile" => {
            *options.replay_fields_mut().typed_ast_output_file =
                OptionValue::decode_value(&decoded)?
        }
        "mergedPrecompiledLibraries" => {
            *options.replay_fields_mut().merged_precompiled_libraries =
                OptionValue::decode_value(&decoded)?
        }
        "inferConsts" => {
            *options.replay_fields_mut().infer_consts = OptionValue::decode_value(&decoded)?
        }
        "assumeStrictThis" => {
            *options.replay_fields_mut().assume_strict_this = OptionValue::decode_value(&decoded)?
        }
        "preserveDetailedSourceInfo" => {
            *options.replay_fields_mut().preserve_detailed_source_info =
                OptionValue::decode_value(&decoded)?
        }
        "preserveNonJSDocComments" => {
            *options.replay_fields_mut().preserve_non_jsdoc_comments =
                OptionValue::decode_value(&decoded)?
        }
        "continueAfterErrors" => {
            *options.replay_fields_mut().continue_after_errors =
                OptionValue::decode_value(&decoded)?
        }
        "incrementalCheckMode" => {
            *options.replay_fields_mut().incremental_check_mode =
                OptionValue::decode_value(&decoded)?
        }
        "parseJsDocDocumentation" => {
            *options.replay_fields_mut().parse_js_doc_documentation =
                OptionValue::decode_value(&decoded)?
        }
        "printExterns" => {
            *options.replay_fields_mut().print_externs = OptionValue::decode_value(&decoded)?
        }
        "inferTypes" => {
            *options.replay_fields_mut().infer_types = OptionValue::decode_value(&decoded)?
        }
        "skipNonTranspilationPasses" => {
            *options.replay_fields_mut().skip_non_transpilation_passes =
                OptionValue::decode_value(&decoded)?
        }
        "devMode" => *options.replay_fields_mut().dev_mode = OptionValue::decode_value(&decoded)?,
        "checkDeterminism" => {
            *options.replay_fields_mut().check_determinism = OptionValue::decode_value(&decoded)?
        }
        "dependencyOptions" => {
            *options.replay_fields_mut().dependency_options = OptionValue::decode_value(&decoded)?
        }
        "messageBundle" => {
            *options.replay_fields_mut().message_bundle = OptionValue::decode_value(&decoded)?
        }
        "strictMessageReplacement" => {
            *options.replay_fields_mut().strict_message_replacement =
                OptionValue::decode_value(&decoded)?
        }
        "checkSymbols" => {
            *options.replay_fields_mut().check_symbols = OptionValue::decode_value(&decoded)?
        }
        "checkSuspiciousCode" => {
            *options.replay_fields_mut().check_suspicious_code =
                OptionValue::decode_value(&decoded)?
        }
        "checkTypes" => {
            *options.replay_fields_mut().check_types = OptionValue::decode_value(&decoded)?
        }
        "extraAnnotationNames" => {
            *options.replay_fields_mut().extra_annotation_names =
                OptionValue::decode_value(&decoded)?
        }
        "numParallelThreads" => {
            *options.replay_fields_mut().num_parallel_threads = OptionValue::decode_value(&decoded)?
        }
        "foldConstants" => {
            *options.replay_fields_mut().fold_constants = OptionValue::decode_value(&decoded)?
        }
        "deadAssignmentElimination" => {
            *options.replay_fields_mut().dead_assignment_elimination =
                OptionValue::decode_value(&decoded)?
        }
        "deadPropertyAssignmentElimination" => {
            *options
                .replay_fields_mut()
                .dead_property_assignment_elimination = OptionValue::decode_value(&decoded)?
        }
        "inlineConstantVars" => {
            *options.replay_fields_mut().inline_constant_vars = OptionValue::decode_value(&decoded)?
        }
        "maxFunctionSizeAfterInlining" => {
            *options.replay_fields_mut().max_function_size_after_inlining =
                OptionValue::decode_value(&decoded)?
        }
        "assumeClosuresOnlyCaptureReferences" => {
            *options
                .replay_fields_mut()
                .assume_closures_only_capture_references = OptionValue::decode_value(&decoded)?
        }
        "inlineProperties" => {
            *options.replay_fields_mut().inline_properties = OptionValue::decode_value(&decoded)?
        }
        "crossChunkCodeMotion" => {
            *options.replay_fields_mut().cross_chunk_code_motion =
                OptionValue::decode_value(&decoded)?
        }
        "crossChunkCodeMotionNoStubMethods" => {
            *options
                .replay_fields_mut()
                .cross_chunk_code_motion_no_stub_methods = OptionValue::decode_value(&decoded)?
        }
        "parentChunkCanSeeSymbolsDeclaredInChildren" => {
            *options
                .replay_fields_mut()
                .parent_chunk_can_see_symbols_declared_in_children =
                OptionValue::decode_value(&decoded)?
        }
        "coalesceVariableNames" => {
            *options.replay_fields_mut().coalesce_variable_names =
                OptionValue::decode_value(&decoded)?
        }
        "optimizeLetAndConst" => {
            *options.replay_fields_mut().optimize_let_and_const =
                OptionValue::decode_value(&decoded)?
        }
        "assumeGlobalScopeIsIsolated" => {
            *options.replay_fields_mut().assume_global_scope_is_isolated =
                OptionValue::decode_value(&decoded)?
        }
        "crossChunkMethodMotion" => {
            *options.replay_fields_mut().cross_chunk_method_motion =
                OptionValue::decode_value(&decoded)?
        }
        "inlineGetters" => {
            *options.replay_fields_mut().inline_getters = OptionValue::decode_value(&decoded)?
        }
        "inlineVariables" => {
            *options.replay_fields_mut().inline_variables = OptionValue::decode_value(&decoded)?
        }
        "inlineLocalVariables" => {
            *options.replay_fields_mut().inline_local_variables =
                OptionValue::decode_value(&decoded)?
        }
        "flowSensitiveInlineVariables" => {
            *options.replay_fields_mut().flow_sensitive_inline_variables =
                OptionValue::decode_value(&decoded)?
        }
        "smartNameRemoval" => {
            *options.replay_fields_mut().smart_name_removal = OptionValue::decode_value(&decoded)?
        }
        "extractPrototypeMemberDeclarations" => {
            *options
                .replay_fields_mut()
                .extract_prototype_member_declarations = OptionValue::decode_value(&decoded)?
        }
        "removeUnusedPrototypeProperties" => {
            *options
                .replay_fields_mut()
                .remove_unused_prototype_properties = OptionValue::decode_value(&decoded)?
        }
        "removeUnusedClassProperties" => {
            *options.replay_fields_mut().remove_unused_class_properties =
                OptionValue::decode_value(&decoded)?
        }
        "removeUnusedVars" => {
            *options.replay_fields_mut().remove_unused_vars = OptionValue::decode_value(&decoded)?
        }
        "removeUnusedLocalVars" => {
            *options.replay_fields_mut().remove_unused_local_vars =
                OptionValue::decode_value(&decoded)?
        }
        "collapseVariableDeclarations" => {
            *options.replay_fields_mut().collapse_variable_declarations =
                OptionValue::decode_value(&decoded)?
        }
        "collapseAnonymousFunctions" => {
            *options.replay_fields_mut().collapse_anonymous_functions =
                OptionValue::decode_value(&decoded)?
        }
        "aliasStringsMode" => {
            *options.replay_fields_mut().alias_strings_mode = OptionValue::decode_value(&decoded)?
        }
        "outputJsStringUsage" => {
            *options.replay_fields_mut().output_js_string_usage =
                OptionValue::decode_value(&decoded)?
        }
        "convertToDottedProperties" => {
            *options.replay_fields_mut().convert_to_dotted_properties =
                OptionValue::decode_value(&decoded)?
        }
        "rewriteFunctionExpressions" => {
            *options.replay_fields_mut().rewrite_function_expressions =
                OptionValue::decode_value(&decoded)?
        }
        "optimizeCalls" => {
            *options.replay_fields_mut().optimize_calls = OptionValue::decode_value(&decoded)?
        }
        "optimizeESClassConstructors" => {
            *options.replay_fields_mut().optimize_es_class_constructors =
                OptionValue::decode_value(&decoded)?
        }
        "useTypesForLocalOptimization" => {
            *options.replay_fields_mut().use_types_for_local_optimization =
                OptionValue::decode_value(&decoded)?
        }
        "useSizeHeuristicToStopOptimizationLoop" => {
            *options
                .replay_fields_mut()
                .use_size_heuristic_to_stop_optimization_loop = OptionValue::decode_value(&decoded)?
        }
        "optimizationLoopMaxIterations" => {
            *options.replay_fields_mut().optimization_loop_max_iterations =
                OptionValue::decode_value(&decoded)?
        }
        "variableRenaming" => {
            *options.replay_fields_mut().variable_renaming = OptionValue::decode_value(&decoded)?
        }
        "propertyRenaming" => {
            *options.replay_fields_mut().property_renaming = OptionValue::decode_value(&decoded)?
        }
        "propertyRenamingOnlyCompilationMode" => {
            *options
                .replay_fields_mut()
                .property_renaming_only_compilation_mode = OptionValue::decode_value(&decoded)?
        }
        "labelRenaming" => {
            *options.replay_fields_mut().label_renaming = OptionValue::decode_value(&decoded)?
        }
        "reserveRawExports" => {
            *options.replay_fields_mut().reserve_raw_exports = OptionValue::decode_value(&decoded)?
        }
        "preferStableNames" => {
            *options.replay_fields_mut().prefer_stable_names = OptionValue::decode_value(&decoded)?
        }
        "generatePseudoNames" => {
            *options.replay_fields_mut().generate_pseudo_names =
                OptionValue::decode_value(&decoded)?
        }
        "renamePrefix" => {
            *options.replay_fields_mut().rename_prefix = OptionValue::decode_value(&decoded)?
        }
        "renamePrefixNamespace" => {
            *options.replay_fields_mut().rename_prefix_namespace =
                OptionValue::decode_value(&decoded)?
        }
        "renamePrefixNamespaceAssumeCrossChunkNames" => {
            *options
                .replay_fields_mut()
                .rename_prefix_namespace_assume_cross_chunk_names =
                OptionValue::decode_value(&decoded)?
        }
        "optimizeLocalAccessForGlobalSymbolNamespace" => {
            *options
                .replay_fields_mut()
                .optimize_local_access_for_global_symbol_namespace =
                OptionValue::decode_value(&decoded)?
        }
        "collapsePropertiesLevel" => {
            *options.replay_fields_mut().collapse_properties_level =
                OptionValue::decode_value(&decoded)?
        }
        "collapseObjectLiterals" => {
            *options.replay_fields_mut().collapse_object_literals =
                OptionValue::decode_value(&decoded)?
        }
        "devirtualizeMethods" => {
            *options.replay_fields_mut().devirtualize_methods = OptionValue::decode_value(&decoded)?
        }
        "computeFunctionSideEffects" => {
            *options.replay_fields_mut().compute_function_side_effects =
                OptionValue::decode_value(&decoded)?
        }
        "disambiguateProperties" => {
            *options.replay_fields_mut().disambiguate_properties =
                OptionValue::decode_value(&decoded)?
        }
        "ambiguateProperties" => {
            *options.replay_fields_mut().ambiguate_properties = OptionValue::decode_value(&decoded)?
        }
        "inputSourceMaps" => {
            *options.replay_fields_mut().input_source_maps = OptionValue::decode_value(&decoded)?
        }
        "inputVariableMap" => {
            *options.replay_fields_mut().input_variable_map = OptionValue::decode_value(&decoded)?
        }
        "inputPropertyMap" => {
            *options.replay_fields_mut().input_property_map = OptionValue::decode_value(&decoded)?
        }
        "exportTestFunctions" => {
            *options.replay_fields_mut().export_test_functions =
                OptionValue::decode_value(&decoded)?
        }
        "nameGenerator" => {
            *options.replay_fields_mut().name_generator = OptionValue::decode_value(&decoded)?
        }
        "replaceMessagesWithChromeI18n" => {
            *options
                .replay_fields_mut()
                .replace_messages_with_chrome_i18n = OptionValue::decode_value(&decoded)?
        }
        "tcProjectId" => {
            *options.replay_fields_mut().tc_project_id = OptionValue::decode_value(&decoded)?
        }
        "codingConvention" => {
            *options.replay_fields_mut().coding_convention = OptionValue::decode_value(&decoded)?
        }
        "syntheticBlockStartMarker" => {
            *options.replay_fields_mut().synthetic_block_start_marker =
                OptionValue::decode_value(&decoded)?
        }
        "syntheticBlockEndMarker" => {
            *options.replay_fields_mut().synthetic_block_end_marker =
                OptionValue::decode_value(&decoded)?
        }
        "locale" => *options.replay_fields_mut().locale = OptionValue::decode_value(&decoded)?,
        "doLateLocalization" => {
            *options.replay_fields_mut().do_late_localization = OptionValue::decode_value(&decoded)?
        }
        "markAsCompiled" => {
            *options.replay_fields_mut().mark_as_compiled = OptionValue::decode_value(&decoded)?
        }
        "closurePass" => {
            *options.replay_fields_mut().closure_pass = OptionValue::decode_value(&decoded)?
        }
        "preserveClosurePrimitives" => {
            *options.replay_fields_mut().preserve_closure_primitives =
                OptionValue::decode_value(&decoded)?
        }
        "angularPass" => {
            *options.replay_fields_mut().angular_pass = OptionValue::decode_value(&decoded)?
        }
        "polymerPass" => {
            *options.replay_fields_mut().polymer_pass = OptionValue::decode_value(&decoded)?
        }
        "chromePass" => {
            *options.replay_fields_mut().chrome_pass = OptionValue::decode_value(&decoded)?
        }
        "j2clPassMode" => {
            *options.replay_fields_mut().j2cl_pass_mode = OptionValue::decode_value(&decoded)?
        }
        "j2clMinifierEnabled" => {
            *options.replay_fields_mut().j2cl_minifier_enabled =
                OptionValue::decode_value(&decoded)?
        }
        "j2clMinifierPruningManifest" => {
            *options.replay_fields_mut().j2cl_minifier_pruning_manifest =
                OptionValue::decode_value(&decoded)?
        }
        "removeAbstractMethods" => {
            *options.replay_fields_mut().remove_abstract_methods =
                OptionValue::decode_value(&decoded)?
        }
        "removeClosureAsserts" => {
            *options.replay_fields_mut().remove_closure_asserts =
                OptionValue::decode_value(&decoded)?
        }
        "removeJ2clAsserts" => {
            *options.replay_fields_mut().remove_j2cl_asserts = OptionValue::decode_value(&decoded)?
        }
        "gatherCssNames" => {
            *options.replay_fields_mut().gather_css_names = OptionValue::decode_value(&decoded)?
        }
        "stripTypes" => {
            *options.replay_fields_mut().strip_types = OptionValue::decode_value(&decoded)?
        }
        "stripNameSuffixes" => {
            *options.replay_fields_mut().strip_name_suffixes = OptionValue::decode_value(&decoded)?
        }
        "stripNamePrefixes" => {
            *options.replay_fields_mut().strip_name_prefixes = OptionValue::decode_value(&decoded)?
        }
        "customPasses" => {
            *options.replay_fields_mut().custom_passes = OptionValue::decode_value(&decoded)?
        }
        "defineReplacements" => {
            *options.replay_fields_mut().define_replacements = OptionValue::decode_value(&decoded)?
        }
        "tweakProcessing" => {
            *options.replay_fields_mut().tweak_processing = OptionValue::decode_value(&decoded)?
        }
        "rewriteGlobalDeclarationsForTryCatchWrapping" => {
            *options
                .replay_fields_mut()
                .rewrite_global_declarations_for_try_catch_wrapping =
                OptionValue::decode_value(&decoded)?
        }
        "checksOnly" => {
            *options.replay_fields_mut().checks_only = OptionValue::decode_value(&decoded)?
        }
        "outputJs" => *options.replay_fields_mut().output_js = OptionValue::decode_value(&decoded)?,
        "generateExports" => {
            *options.replay_fields_mut().generate_exports = OptionValue::decode_value(&decoded)?
        }
        "exportLocalPropertyDefinitions" => {
            *options
                .replay_fields_mut()
                .export_local_property_definitions = OptionValue::decode_value(&decoded)?
        }
        "cssRenamingMap" => {
            *options.replay_fields_mut().css_renaming_map = OptionValue::decode_value(&decoded)?
        }
        "cssRenamingSkiplist" => {
            *options.replay_fields_mut().css_renaming_skiplist =
                OptionValue::decode_value(&decoded)?
        }
        "replaceIdGenerators" => {
            *options.replay_fields_mut().replace_id_generators =
                OptionValue::decode_value(&decoded)?
        }
        "idGenerators" => {
            *options.replay_fields_mut().id_generators = OptionValue::decode_value(&decoded)?
        }
        "xidHashFunction" => {
            *options.replay_fields_mut().xid_hash_function = OptionValue::decode_value(&decoded)?
        }
        "chunkIdHashFunction" => {
            *options.replay_fields_mut().chunk_id_hash_function =
                OptionValue::decode_value(&decoded)?
        }
        "idGeneratorsMapSerialized" => {
            *options.replay_fields_mut().id_generators_map_serialized =
                OptionValue::decode_value(&decoded)?
        }
        "replaceStringsFunctionDescriptions" => {
            *options
                .replay_fields_mut()
                .replace_strings_function_descriptions = OptionValue::decode_value(&decoded)?
        }
        "replaceStringsPlaceholderToken" => {
            *options
                .replay_fields_mut()
                .replace_strings_placeholder_token = OptionValue::decode_value(&decoded)?
        }
        "propertiesThatMustDisambiguate" => {
            *options
                .replay_fields_mut()
                .properties_that_must_disambiguate = OptionValue::decode_value(&decoded)?
        }
        "validateRequireInliningAnnotation" => {
            *options
                .replay_fields_mut()
                .validate_require_inlining_annotation = OptionValue::decode_value(&decoded)?
        }
        "processCommonJSModules" => {
            *options.replay_fields_mut().process_common_js_modules =
                OptionValue::decode_value(&decoded)?
        }
        "moduleRoots" => {
            *options.replay_fields_mut().module_roots = OptionValue::decode_value(&decoded)?
        }
        "rewritePolyfills" => {
            *options.replay_fields_mut().rewrite_polyfills = OptionValue::decode_value(&decoded)?
        }
        "isolatePolyfills" => {
            *options.replay_fields_mut().isolate_polyfills = OptionValue::decode_value(&decoded)?
        }
        "injectPolyfillsNewerThan" => {
            *options.replay_fields_mut().inject_polyfills_newer_than =
                OptionValue::decode_value(&decoded)?
        }
        "es6SubclassTranspilation" => {
            *options.replay_fields_mut().es6_subclass_transpilation =
                OptionValue::decode_value(&decoded)?
        }
        "instrumentAsyncContext" => {
            *options.replay_fields_mut().instrument_async_context =
                OptionValue::decode_value(&decoded)?
        }
        "forceLibraryInjection" => {
            *options.replay_fields_mut().force_library_injection =
                OptionValue::decode_value(&decoded)?
        }
        "runtimeLibraryMode" => {
            *options.replay_fields_mut().runtime_library_mode = OptionValue::decode_value(&decoded)?
        }
        "assumeForwardDeclaredForMissingTypes" => {
            *options
                .replay_fields_mut()
                .assume_forward_declared_for_missing_types = OptionValue::decode_value(&decoded)?
        }
        "unusedImportsToRemove" => {
            *options.replay_fields_mut().unused_imports_to_remove =
                OptionValue::decode_value(&decoded)?
        }
        "preserveTypeAnnotations" => {
            *options.replay_fields_mut().preserve_type_annotations =
                OptionValue::decode_value(&decoded)?
        }
        "gentsMode" => {
            *options.replay_fields_mut().gents_mode = OptionValue::decode_value(&decoded)?
        }
        "prettyPrint" => {
            *options.replay_fields_mut().pretty_print = OptionValue::decode_value(&decoded)?
        }
        "lineBreak" => {
            *options.replay_fields_mut().line_break = OptionValue::decode_value(&decoded)?
        }
        "printInputDelimiter" => {
            *options.replay_fields_mut().print_input_delimiter =
                OptionValue::decode_value(&decoded)?
        }
        "inputDelimiter" => {
            *options.replay_fields_mut().input_delimiter = OptionValue::decode_value(&decoded)?
        }
        "debugLogDirectory" => {
            *options.replay_fields_mut().debug_log_directory = OptionValue::decode_value(&decoded)?
        }
        "debugLogFilter" => {
            *options.replay_fields_mut().debug_log_filter = OptionValue::decode_value(&decoded)?
        }
        "serializeExtraDebugInfo" => {
            *options.replay_fields_mut().serialize_extra_debug_info =
                OptionValue::decode_value(&decoded)?
        }
        "stateCompressionWrapper" => {
            *options.replay_fields_mut().state_compression_wrapper =
                OptionValue::decode_value(&decoded)?
        }
        "stateDecompressionWrapper" => {
            *options.replay_fields_mut().state_decompression_wrapper =
                OptionValue::decode_value(&decoded)?
        }
        "quoteKeywordProperties" => {
            *options.replay_fields_mut().quote_keyword_properties =
                OptionValue::decode_value(&decoded)?
        }
        "preferSingleQuotes" => {
            *options.replay_fields_mut().prefer_single_quotes = OptionValue::decode_value(&decoded)?
        }
        "trustedStrings" => {
            *options.replay_fields_mut().trusted_strings = OptionValue::decode_value(&decoded)?
        }
        "printSourceAfterEachPass" => {
            *options.replay_fields_mut().print_source_after_each_pass =
                OptionValue::decode_value(&decoded)?
        }
        "filesToPrintAfterEachPassRegexList" => {
            *options
                .replay_fields_mut()
                .files_to_print_after_each_pass_regex_list = OptionValue::decode_value(&decoded)?
        }
        "chunksToPrintAfterEachPassRegexList" => {
            *options
                .replay_fields_mut()
                .chunks_to_print_after_each_pass_regex_list = OptionValue::decode_value(&decoded)?
        }
        "qnameUsesToPrintAfterEachPassList" => {
            *options
                .replay_fields_mut()
                .qname_uses_to_print_after_each_pass_list = OptionValue::decode_value(&decoded)?
        }
        "tracer" => *options.replay_fields_mut().tracer = OptionValue::decode_value(&decoded)?,
        "tracerOutput" => {
            *options.replay_fields_mut().tracer_output = OptionValue::decode_value(&decoded)?
        }
        "colorizeErrorOutput" => {
            *options.replay_fields_mut().colorize_error_output =
                OptionValue::decode_value(&decoded)?
        }
        "errorFormat" => {
            *options.replay_fields_mut().error_format = OptionValue::decode_value(&decoded)?
        }
        "warningsGuard" => {
            *options.replay_fields_mut().warnings_guard = OptionValue::decode_value(&decoded)?
        }
        "summaryDetailLevel" => {
            *options.replay_fields_mut().summary_detail_level = OptionValue::decode_value(&decoded)?
        }
        "lineLengthThreshold" => {
            *options.replay_fields_mut().line_length_threshold =
                OptionValue::decode_value(&decoded)?
        }
        "useOriginalNamesInOutput" => {
            *options.replay_fields_mut().use_original_names_in_output =
                OptionValue::decode_value(&decoded)?
        }
        "externExportsPath" => {
            *options.replay_fields_mut().extern_exports_path = OptionValue::decode_value(&decoded)?
        }
        "extraReportGenerators" => {
            *options.replay_fields_mut().extra_report_generators =
                OptionValue::decode_value(&decoded)?
        }
        "sourceMapOutputPath" => {
            *options.replay_fields_mut().source_map_output_path =
                OptionValue::decode_value(&decoded)?
        }
        "shouldAlwaysGatherSourceMapInfo" => {
            *options
                .replay_fields_mut()
                .should_always_gather_source_map_info = OptionValue::decode_value(&decoded)?
        }
        "sourceMapDetailLevel" => {
            *options.replay_fields_mut().source_map_detail_level =
                OptionValue::decode_value(&decoded)?
        }
        "sourceMapFormat" => {
            *options.replay_fields_mut().source_map_format = OptionValue::decode_value(&decoded)?
        }
        "parseInlineSourceMaps" => {
            *options.replay_fields_mut().parse_inline_source_maps =
                OptionValue::decode_value(&decoded)?
        }
        "applyInputSourceMaps" => {
            *options.replay_fields_mut().apply_input_source_maps =
                OptionValue::decode_value(&decoded)?
        }
        "resolveSourceMapAnnotations" => {
            *options.replay_fields_mut().resolve_source_map_annotations =
                OptionValue::decode_value(&decoded)?
        }
        "sourceMapLocationMappings" => {
            *options.replay_fields_mut().source_map_location_mappings =
                OptionValue::decode_value(&decoded)?
        }
        "sourceMapIncludeSourcesContent" => {
            *options
                .replay_fields_mut()
                .source_map_include_sources_content = OptionValue::decode_value(&decoded)?
        }
        "outputCharset" => {
            *options.replay_fields_mut().output_charset = OptionValue::decode_value(&decoded)?
        }
        "protectHiddenSideEffects" => {
            *options.replay_fields_mut().protect_hidden_side_effects =
                OptionValue::decode_value(&decoded)?
        }
        "assumeGettersArePure" => {
            *options.replay_fields_mut().assume_getters_are_pure =
                OptionValue::decode_value(&decoded)?
        }
        "assumePropertiesAreStaticallyAnalyzable" => {
            *options
                .replay_fields_mut()
                .assume_properties_are_statically_analyzable = OptionValue::decode_value(&decoded)?
        }
        "assumeStaticInheritanceIsNotUsed" => {
            *options
                .replay_fields_mut()
                .assume_static_inheritance_is_not_used = OptionValue::decode_value(&decoded)?
        }
        "errorHandler" => {
            *options.replay_fields_mut().error_handler = OptionValue::decode_value(&decoded)?
        }
        "instrumentForCoverageOption" => {
            *options.replay_fields_mut().instrument_for_coverage_option =
                OptionValue::decode_value(&decoded)?
        }
        "productionInstrumentationArrayName" => {
            *options
                .replay_fields_mut()
                .production_instrumentation_array_name = OptionValue::decode_value(&decoded)?
        }
        "conformanceConfigs" => {
            *options.replay_fields_mut().conformance_configs = OptionValue::decode_value(&decoded)?
        }
        "conformanceReportingMode" => {
            *options.replay_fields_mut().conformance_reporting_mode =
                OptionValue::decode_value(&decoded)?
        }
        "conformanceRemoveRegexFromPath" => {
            *options
                .replay_fields_mut()
                .conformance_remove_regex_from_path = OptionValue::decode_value(&decoded)?
        }
        "wrapGoogModulesForWhitespaceOnly" => {
            *options
                .replay_fields_mut()
                .wrap_goog_modules_for_whitespace_only = OptionValue::decode_value(&decoded)?
        }
        "printConfig" => {
            *options.replay_fields_mut().print_config = OptionValue::decode_value(&decoded)?
        }
        "isStrictModeInput" => {
            *options.replay_fields_mut().is_strict_mode_input = OptionValue::decode_value(&decoded)?
        }
        "rewriteModulesBeforeTypechecking" => {
            *options
                .replay_fields_mut()
                .rewrite_modules_before_typechecking = OptionValue::decode_value(&decoded)?
        }
        "enableModuleRewriting" => {
            *options.replay_fields_mut().enable_module_rewriting =
                OptionValue::decode_value(&decoded)?
        }
        "moduleResolutionMode" => {
            *options.replay_fields_mut().module_resolution_mode =
                OptionValue::decode_value(&decoded)?
        }
        "browserResolverPrefixReplacements" => {
            *options
                .replay_fields_mut()
                .browser_resolver_prefix_replacements = OptionValue::decode_value(&decoded)?
        }
        "pathEscaper" => {
            *options.replay_fields_mut().path_escaper = OptionValue::decode_value(&decoded)?
        }
        "packageJsonEntryNames" => {
            *options.replay_fields_mut().package_json_entry_names =
                OptionValue::decode_value(&decoded)?
        }
        "allowDynamicImport" => {
            *options.replay_fields_mut().allow_dynamic_import = OptionValue::decode_value(&decoded)?
        }
        "dynamicImportAlias" => {
            *options.replay_fields_mut().dynamic_import_alias = OptionValue::decode_value(&decoded)?
        }
        "chunkOutputType" => {
            *options.replay_fields_mut().chunk_output_type = OptionValue::decode_value(&decoded)?
        }
        "unknownDefinesToIgnore" => {
            *options.replay_fields_mut().unknown_defines_to_ignore =
                OptionValue::decode_value(&decoded)?
        }
        "inlineFunctionsLevel" => {
            *options.replay_fields_mut().inline_functions_level =
                OptionValue::decode_value(&decoded)?
        }
        "enableZonesDefineName" => {
            *options.replay_fields_mut().enable_zones_define_name =
                OptionValue::decode_value(&decoded)?
        }
        "zoneInputPattern" => {
            *options.replay_fields_mut().zone_input_pattern = OptionValue::decode_value(&decoded)?
        }
        "es6ModuleTranspilation" => {
            *options.replay_fields_mut().es6_module_transpilation =
                OptionValue::decode_value(&decoded)?
        }
        _ => return Err(Throwable::HarnessError(format!("no option field {name}"))),
    }
    Ok(())
}
// port: UnitRecorder#fields (CompilerOptions reflection)
pub fn get_field(options: &CompilerOptions, name: &str) -> Result<JsonValue, Throwable> {
    let decoded = match name {
        "emitUseStrict" => options.replay_fields().emit_use_strict.encode_value()?,
        "languageIn" => options.replay_fields().language_in.encode_value()?,
        "outputFeatureSet" => options.replay_fields().output_feature_set.encode_value()?,
        "experimentalOutputFeatureSet" => options
            .replay_fields()
            .experimental_output_feature_set
            .encode_value()?,
        "languageOutIsDefaultStrict" => options
            .replay_fields()
            .language_out_is_default_strict
            .encode_value()?,
        "environment" => options.replay_fields().environment.encode_value()?,
        "browserFeaturesetYear" => options
            .replay_fields()
            .browser_featureset_year
            .encode_value()?,
        "instrumentForCoverageOnly" => options
            .replay_fields()
            .instrument_for_coverage_only
            .encode_value()?,
        "typedAstOutputFile" => options
            .replay_fields()
            .typed_ast_output_file
            .encode_value()?,
        "mergedPrecompiledLibraries" => options
            .replay_fields()
            .merged_precompiled_libraries
            .encode_value()?,
        "inferConsts" => options.replay_fields().infer_consts.encode_value()?,
        "assumeStrictThis" => options.replay_fields().assume_strict_this.encode_value()?,
        "preserveDetailedSourceInfo" => options
            .replay_fields()
            .preserve_detailed_source_info
            .encode_value()?,
        "preserveNonJSDocComments" => options
            .replay_fields()
            .preserve_non_jsdoc_comments
            .encode_value()?,
        "continueAfterErrors" => options
            .replay_fields()
            .continue_after_errors
            .encode_value()?,
        "incrementalCheckMode" => options
            .replay_fields()
            .incremental_check_mode
            .encode_value()?,
        "parseJsDocDocumentation" => options
            .replay_fields()
            .parse_js_doc_documentation
            .encode_value()?,
        "printExterns" => options.replay_fields().print_externs.encode_value()?,
        "inferTypes" => options.replay_fields().infer_types.encode_value()?,
        "skipNonTranspilationPasses" => options
            .replay_fields()
            .skip_non_transpilation_passes
            .encode_value()?,
        "devMode" => options.replay_fields().dev_mode.encode_value()?,
        "checkDeterminism" => options.replay_fields().check_determinism.encode_value()?,
        "dependencyOptions" => options.replay_fields().dependency_options.encode_value()?,
        "messageBundle" => options.replay_fields().message_bundle.encode_value()?,
        "strictMessageReplacement" => options
            .replay_fields()
            .strict_message_replacement
            .encode_value()?,
        "checkSymbols" => options.replay_fields().check_symbols.encode_value()?,
        "checkSuspiciousCode" => options
            .replay_fields()
            .check_suspicious_code
            .encode_value()?,
        "checkTypes" => options.replay_fields().check_types.encode_value()?,
        "extraAnnotationNames" => options
            .replay_fields()
            .extra_annotation_names
            .encode_value()?,
        "numParallelThreads" => options
            .replay_fields()
            .num_parallel_threads
            .encode_value()?,
        "foldConstants" => options.replay_fields().fold_constants.encode_value()?,
        "deadAssignmentElimination" => options
            .replay_fields()
            .dead_assignment_elimination
            .encode_value()?,
        "deadPropertyAssignmentElimination" => options
            .replay_fields()
            .dead_property_assignment_elimination
            .encode_value()?,
        "inlineConstantVars" => options
            .replay_fields()
            .inline_constant_vars
            .encode_value()?,
        "maxFunctionSizeAfterInlining" => options
            .replay_fields()
            .max_function_size_after_inlining
            .encode_value()?,
        "assumeClosuresOnlyCaptureReferences" => options
            .replay_fields()
            .assume_closures_only_capture_references
            .encode_value()?,
        "inlineProperties" => options.replay_fields().inline_properties.encode_value()?,
        "crossChunkCodeMotion" => options
            .replay_fields()
            .cross_chunk_code_motion
            .encode_value()?,
        "crossChunkCodeMotionNoStubMethods" => options
            .replay_fields()
            .cross_chunk_code_motion_no_stub_methods
            .encode_value()?,
        "parentChunkCanSeeSymbolsDeclaredInChildren" => options
            .replay_fields()
            .parent_chunk_can_see_symbols_declared_in_children
            .encode_value()?,
        "coalesceVariableNames" => options
            .replay_fields()
            .coalesce_variable_names
            .encode_value()?,
        "optimizeLetAndConst" => options
            .replay_fields()
            .optimize_let_and_const
            .encode_value()?,
        "assumeGlobalScopeIsIsolated" => options
            .replay_fields()
            .assume_global_scope_is_isolated
            .encode_value()?,
        "crossChunkMethodMotion" => options
            .replay_fields()
            .cross_chunk_method_motion
            .encode_value()?,
        "inlineGetters" => options.replay_fields().inline_getters.encode_value()?,
        "inlineVariables" => options.replay_fields().inline_variables.encode_value()?,
        "inlineLocalVariables" => options
            .replay_fields()
            .inline_local_variables
            .encode_value()?,
        "flowSensitiveInlineVariables" => options
            .replay_fields()
            .flow_sensitive_inline_variables
            .encode_value()?,
        "smartNameRemoval" => options.replay_fields().smart_name_removal.encode_value()?,
        "extractPrototypeMemberDeclarations" => options
            .replay_fields()
            .extract_prototype_member_declarations
            .encode_value()?,
        "removeUnusedPrototypeProperties" => options
            .replay_fields()
            .remove_unused_prototype_properties
            .encode_value()?,
        "removeUnusedClassProperties" => options
            .replay_fields()
            .remove_unused_class_properties
            .encode_value()?,
        "removeUnusedVars" => options.replay_fields().remove_unused_vars.encode_value()?,
        "removeUnusedLocalVars" => options
            .replay_fields()
            .remove_unused_local_vars
            .encode_value()?,
        "collapseVariableDeclarations" => options
            .replay_fields()
            .collapse_variable_declarations
            .encode_value()?,
        "collapseAnonymousFunctions" => options
            .replay_fields()
            .collapse_anonymous_functions
            .encode_value()?,
        "aliasStringsMode" => options.replay_fields().alias_strings_mode.encode_value()?,
        "outputJsStringUsage" => options
            .replay_fields()
            .output_js_string_usage
            .encode_value()?,
        "convertToDottedProperties" => options
            .replay_fields()
            .convert_to_dotted_properties
            .encode_value()?,
        "rewriteFunctionExpressions" => options
            .replay_fields()
            .rewrite_function_expressions
            .encode_value()?,
        "optimizeCalls" => options.replay_fields().optimize_calls.encode_value()?,
        "optimizeESClassConstructors" => options
            .replay_fields()
            .optimize_es_class_constructors
            .encode_value()?,
        "useTypesForLocalOptimization" => options
            .replay_fields()
            .use_types_for_local_optimization
            .encode_value()?,
        "useSizeHeuristicToStopOptimizationLoop" => options
            .replay_fields()
            .use_size_heuristic_to_stop_optimization_loop
            .encode_value()?,
        "optimizationLoopMaxIterations" => options
            .replay_fields()
            .optimization_loop_max_iterations
            .encode_value()?,
        "variableRenaming" => options.replay_fields().variable_renaming.encode_value()?,
        "propertyRenaming" => options.replay_fields().property_renaming.encode_value()?,
        "propertyRenamingOnlyCompilationMode" => options
            .replay_fields()
            .property_renaming_only_compilation_mode
            .encode_value()?,
        "labelRenaming" => options.replay_fields().label_renaming.encode_value()?,
        "reserveRawExports" => options.replay_fields().reserve_raw_exports.encode_value()?,
        "preferStableNames" => options.replay_fields().prefer_stable_names.encode_value()?,
        "generatePseudoNames" => options
            .replay_fields()
            .generate_pseudo_names
            .encode_value()?,
        "renamePrefix" => options.replay_fields().rename_prefix.encode_value()?,
        "renamePrefixNamespace" => options
            .replay_fields()
            .rename_prefix_namespace
            .encode_value()?,
        "renamePrefixNamespaceAssumeCrossChunkNames" => options
            .replay_fields()
            .rename_prefix_namespace_assume_cross_chunk_names
            .encode_value()?,
        "optimizeLocalAccessForGlobalSymbolNamespace" => options
            .replay_fields()
            .optimize_local_access_for_global_symbol_namespace
            .encode_value()?,
        "collapsePropertiesLevel" => options
            .replay_fields()
            .collapse_properties_level
            .encode_value()?,
        "collapseObjectLiterals" => options
            .replay_fields()
            .collapse_object_literals
            .encode_value()?,
        "devirtualizeMethods" => options
            .replay_fields()
            .devirtualize_methods
            .encode_value()?,
        "computeFunctionSideEffects" => options
            .replay_fields()
            .compute_function_side_effects
            .encode_value()?,
        "disambiguateProperties" => options
            .replay_fields()
            .disambiguate_properties
            .encode_value()?,
        "ambiguateProperties" => options
            .replay_fields()
            .ambiguate_properties
            .encode_value()?,
        "inputSourceMaps" => options.replay_fields().input_source_maps.encode_value()?,
        "inputVariableMap" => options.replay_fields().input_variable_map.encode_value()?,
        "inputPropertyMap" => options.replay_fields().input_property_map.encode_value()?,
        "exportTestFunctions" => options
            .replay_fields()
            .export_test_functions
            .encode_value()?,
        "nameGenerator" => options.replay_fields().name_generator.encode_value()?,
        "replaceMessagesWithChromeI18n" => options
            .replay_fields()
            .replace_messages_with_chrome_i18n
            .encode_value()?,
        "tcProjectId" => options.replay_fields().tc_project_id.encode_value()?,
        "codingConvention" => options.replay_fields().coding_convention.encode_value()?,
        "syntheticBlockStartMarker" => options
            .replay_fields()
            .synthetic_block_start_marker
            .encode_value()?,
        "syntheticBlockEndMarker" => options
            .replay_fields()
            .synthetic_block_end_marker
            .encode_value()?,
        "locale" => options.replay_fields().locale.encode_value()?,
        "doLateLocalization" => options
            .replay_fields()
            .do_late_localization
            .encode_value()?,
        "markAsCompiled" => options.replay_fields().mark_as_compiled.encode_value()?,
        "closurePass" => options.replay_fields().closure_pass.encode_value()?,
        "preserveClosurePrimitives" => options
            .replay_fields()
            .preserve_closure_primitives
            .encode_value()?,
        "angularPass" => options.replay_fields().angular_pass.encode_value()?,
        "polymerPass" => options.replay_fields().polymer_pass.encode_value()?,
        "chromePass" => options.replay_fields().chrome_pass.encode_value()?,
        "j2clPassMode" => options.replay_fields().j2cl_pass_mode.encode_value()?,
        "j2clMinifierEnabled" => options
            .replay_fields()
            .j2cl_minifier_enabled
            .encode_value()?,
        "j2clMinifierPruningManifest" => options
            .replay_fields()
            .j2cl_minifier_pruning_manifest
            .encode_value()?,
        "removeAbstractMethods" => options
            .replay_fields()
            .remove_abstract_methods
            .encode_value()?,
        "removeClosureAsserts" => options
            .replay_fields()
            .remove_closure_asserts
            .encode_value()?,
        "removeJ2clAsserts" => options.replay_fields().remove_j2cl_asserts.encode_value()?,
        "gatherCssNames" => options.replay_fields().gather_css_names.encode_value()?,
        "stripTypes" => options.replay_fields().strip_types.encode_value()?,
        "stripNameSuffixes" => options.replay_fields().strip_name_suffixes.encode_value()?,
        "stripNamePrefixes" => options.replay_fields().strip_name_prefixes.encode_value()?,
        "customPasses" => options.replay_fields().custom_passes.encode_value()?,
        "defineReplacements" => options.replay_fields().define_replacements.encode_value()?,
        "tweakProcessing" => options.replay_fields().tweak_processing.encode_value()?,
        "rewriteGlobalDeclarationsForTryCatchWrapping" => options
            .replay_fields()
            .rewrite_global_declarations_for_try_catch_wrapping
            .encode_value()?,
        "checksOnly" => options.replay_fields().checks_only.encode_value()?,
        "outputJs" => options.replay_fields().output_js.encode_value()?,
        "generateExports" => options.replay_fields().generate_exports.encode_value()?,
        "exportLocalPropertyDefinitions" => options
            .replay_fields()
            .export_local_property_definitions
            .encode_value()?,
        "cssRenamingMap" => options.replay_fields().css_renaming_map.encode_value()?,
        "cssRenamingSkiplist" => options
            .replay_fields()
            .css_renaming_skiplist
            .encode_value()?,
        "replaceIdGenerators" => options
            .replay_fields()
            .replace_id_generators
            .encode_value()?,
        "idGenerators" => options.replay_fields().id_generators.encode_value()?,
        "xidHashFunction" => options.replay_fields().xid_hash_function.encode_value()?,
        "chunkIdHashFunction" => options
            .replay_fields()
            .chunk_id_hash_function
            .encode_value()?,
        "idGeneratorsMapSerialized" => options
            .replay_fields()
            .id_generators_map_serialized
            .encode_value()?,
        "replaceStringsFunctionDescriptions" => options
            .replay_fields()
            .replace_strings_function_descriptions
            .encode_value()?,
        "replaceStringsPlaceholderToken" => options
            .replay_fields()
            .replace_strings_placeholder_token
            .encode_value()?,
        "propertiesThatMustDisambiguate" => options
            .replay_fields()
            .properties_that_must_disambiguate
            .encode_value()?,
        "validateRequireInliningAnnotation" => options
            .replay_fields()
            .validate_require_inlining_annotation
            .encode_value()?,
        "processCommonJSModules" => options
            .replay_fields()
            .process_common_js_modules
            .encode_value()?,
        "moduleRoots" => options.replay_fields().module_roots.encode_value()?,
        "rewritePolyfills" => options.replay_fields().rewrite_polyfills.encode_value()?,
        "isolatePolyfills" => options.replay_fields().isolate_polyfills.encode_value()?,
        "injectPolyfillsNewerThan" => options
            .replay_fields()
            .inject_polyfills_newer_than
            .encode_value()?,
        "es6SubclassTranspilation" => options
            .replay_fields()
            .es6_subclass_transpilation
            .encode_value()?,
        "instrumentAsyncContext" => options
            .replay_fields()
            .instrument_async_context
            .encode_value()?,
        "forceLibraryInjection" => options
            .replay_fields()
            .force_library_injection
            .encode_value()?,
        "runtimeLibraryMode" => options
            .replay_fields()
            .runtime_library_mode
            .encode_value()?,
        "assumeForwardDeclaredForMissingTypes" => options
            .replay_fields()
            .assume_forward_declared_for_missing_types
            .encode_value()?,
        "unusedImportsToRemove" => options
            .replay_fields()
            .unused_imports_to_remove
            .encode_value()?,
        "preserveTypeAnnotations" => options
            .replay_fields()
            .preserve_type_annotations
            .encode_value()?,
        "gentsMode" => options.replay_fields().gents_mode.encode_value()?,
        "prettyPrint" => options.replay_fields().pretty_print.encode_value()?,
        "lineBreak" => options.replay_fields().line_break.encode_value()?,
        "printInputDelimiter" => options
            .replay_fields()
            .print_input_delimiter
            .encode_value()?,
        "inputDelimiter" => options.replay_fields().input_delimiter.encode_value()?,
        "debugLogDirectory" => options.replay_fields().debug_log_directory.encode_value()?,
        "debugLogFilter" => options.replay_fields().debug_log_filter.encode_value()?,
        "serializeExtraDebugInfo" => options
            .replay_fields()
            .serialize_extra_debug_info
            .encode_value()?,
        "stateCompressionWrapper" => options
            .replay_fields()
            .state_compression_wrapper
            .encode_value()?,
        "stateDecompressionWrapper" => options
            .replay_fields()
            .state_decompression_wrapper
            .encode_value()?,
        "quoteKeywordProperties" => options
            .replay_fields()
            .quote_keyword_properties
            .encode_value()?,
        "preferSingleQuotes" => options
            .replay_fields()
            .prefer_single_quotes
            .encode_value()?,
        "trustedStrings" => options.replay_fields().trusted_strings.encode_value()?,
        "printSourceAfterEachPass" => options
            .replay_fields()
            .print_source_after_each_pass
            .encode_value()?,
        "filesToPrintAfterEachPassRegexList" => options
            .replay_fields()
            .files_to_print_after_each_pass_regex_list
            .encode_value()?,
        "chunksToPrintAfterEachPassRegexList" => options
            .replay_fields()
            .chunks_to_print_after_each_pass_regex_list
            .encode_value()?,
        "qnameUsesToPrintAfterEachPassList" => options
            .replay_fields()
            .qname_uses_to_print_after_each_pass_list
            .encode_value()?,
        "tracer" => options.replay_fields().tracer.encode_value()?,
        "tracerOutput" => options.replay_fields().tracer_output.encode_value()?,
        "colorizeErrorOutput" => options
            .replay_fields()
            .colorize_error_output
            .encode_value()?,
        "errorFormat" => options.replay_fields().error_format.encode_value()?,
        "warningsGuard" => options.replay_fields().warnings_guard.encode_value()?,
        "summaryDetailLevel" => options
            .replay_fields()
            .summary_detail_level
            .encode_value()?,
        "lineLengthThreshold" => options
            .replay_fields()
            .line_length_threshold
            .encode_value()?,
        "useOriginalNamesInOutput" => options
            .replay_fields()
            .use_original_names_in_output
            .encode_value()?,
        "externExportsPath" => options.replay_fields().extern_exports_path.encode_value()?,
        "extraReportGenerators" => options
            .replay_fields()
            .extra_report_generators
            .encode_value()?,
        "sourceMapOutputPath" => options
            .replay_fields()
            .source_map_output_path
            .encode_value()?,
        "shouldAlwaysGatherSourceMapInfo" => options
            .replay_fields()
            .should_always_gather_source_map_info
            .encode_value()?,
        "sourceMapDetailLevel" => options
            .replay_fields()
            .source_map_detail_level
            .encode_value()?,
        "sourceMapFormat" => options.replay_fields().source_map_format.encode_value()?,
        "parseInlineSourceMaps" => options
            .replay_fields()
            .parse_inline_source_maps
            .encode_value()?,
        "applyInputSourceMaps" => options
            .replay_fields()
            .apply_input_source_maps
            .encode_value()?,
        "resolveSourceMapAnnotations" => options
            .replay_fields()
            .resolve_source_map_annotations
            .encode_value()?,
        "sourceMapLocationMappings" => options
            .replay_fields()
            .source_map_location_mappings
            .encode_value()?,
        "sourceMapIncludeSourcesContent" => options
            .replay_fields()
            .source_map_include_sources_content
            .encode_value()?,
        "outputCharset" => options.replay_fields().output_charset.encode_value()?,
        "protectHiddenSideEffects" => options
            .replay_fields()
            .protect_hidden_side_effects
            .encode_value()?,
        "assumeGettersArePure" => options
            .replay_fields()
            .assume_getters_are_pure
            .encode_value()?,
        "assumePropertiesAreStaticallyAnalyzable" => options
            .replay_fields()
            .assume_properties_are_statically_analyzable
            .encode_value()?,
        "assumeStaticInheritanceIsNotUsed" => options
            .replay_fields()
            .assume_static_inheritance_is_not_used
            .encode_value()?,
        "errorHandler" => options.replay_fields().error_handler.encode_value()?,
        "instrumentForCoverageOption" => options
            .replay_fields()
            .instrument_for_coverage_option
            .encode_value()?,
        "productionInstrumentationArrayName" => options
            .replay_fields()
            .production_instrumentation_array_name
            .encode_value()?,
        "conformanceConfigs" => options.replay_fields().conformance_configs.encode_value()?,
        "conformanceReportingMode" => options
            .replay_fields()
            .conformance_reporting_mode
            .encode_value()?,
        "conformanceRemoveRegexFromPath" => options
            .replay_fields()
            .conformance_remove_regex_from_path
            .encode_value()?,
        "wrapGoogModulesForWhitespaceOnly" => options
            .replay_fields()
            .wrap_goog_modules_for_whitespace_only
            .encode_value()?,
        "printConfig" => options.replay_fields().print_config.encode_value()?,
        "isStrictModeInput" => options
            .replay_fields()
            .is_strict_mode_input
            .encode_value()?,
        "rewriteModulesBeforeTypechecking" => options
            .replay_fields()
            .rewrite_modules_before_typechecking
            .encode_value()?,
        "enableModuleRewriting" => options
            .replay_fields()
            .enable_module_rewriting
            .encode_value()?,
        "moduleResolutionMode" => options
            .replay_fields()
            .module_resolution_mode
            .encode_value()?,
        "browserResolverPrefixReplacements" => options
            .replay_fields()
            .browser_resolver_prefix_replacements
            .encode_value()?,
        "pathEscaper" => options.replay_fields().path_escaper.encode_value()?,
        "packageJsonEntryNames" => options
            .replay_fields()
            .package_json_entry_names
            .encode_value()?,
        "allowDynamicImport" => options
            .replay_fields()
            .allow_dynamic_import
            .encode_value()?,
        "dynamicImportAlias" => options
            .replay_fields()
            .dynamic_import_alias
            .encode_value()?,
        "chunkOutputType" => options.replay_fields().chunk_output_type.encode_value()?,
        "unknownDefinesToIgnore" => options
            .replay_fields()
            .unknown_defines_to_ignore
            .encode_value()?,
        "inlineFunctionsLevel" => options
            .replay_fields()
            .inline_functions_level
            .encode_value()?,
        "enableZonesDefineName" => options
            .replay_fields()
            .enable_zones_define_name
            .encode_value()?,
        "zoneInputPattern" => options.replay_fields().zone_input_pattern.encode_value()?,
        "es6ModuleTranspilation" => options
            .replay_fields()
            .es6_module_transpilation
            .encode_value()?,
        _ => return Err(Throwable::HarnessError(format!("no option field {name}"))),
    };
    let decoded = if defaults()
        .field_types
        .get(name)
        .is_some_and(|ty| ty.starts_with("com.google.common.base.Optional<"))
    {
        DslValue::Optional(if matches!(decoded, DslValue::Null) {
            None
        } else {
            Some(Box::new(decoded))
        })
    } else {
        decoded
    };
    encode_with_template(
        &decoded,
        defaults().fields.get(name).map(Value::to_json).as_ref(),
    )
}
// port: ReplayValues#enumValue
pub fn enum_name(value: &DslValue) -> Result<&str, Throwable> {
    match value.untyped() {
        DslValue::Enum { name, .. } => Ok(name),
        _ => Err(Throwable::HarnessError("expected an enum value".into())),
    }
}
// port: CompilerOptions.LanguageMode#valueOf
pub fn language_mode(value: &DslValue) -> Result<crate::jscomp_api::LanguageMode, Throwable> {
    OptionValue::decode_value(value)
}
// port: ModuleLoader.ResolutionMode#valueOf
pub fn resolution_mode(
    value: &DslValue,
) -> Result<closure_jscomp::deps::module_loader::ResolutionMode, Throwable> {
    OptionValue::decode_value(value)
}
// port: Config.JsDocParsing#valueOf
pub fn jsdoc_parsing(value: &DslValue) -> Result<closure_parsing::config::JsDocParsing, Throwable> {
    OptionValue::decode_value(value)
}
