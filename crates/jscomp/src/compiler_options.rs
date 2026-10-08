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
//   src/com/google/javascript/jscomp/CompilerOptions.java.

use crate::{
    check_level::CheckLevel,
    coding_convention::CodingConvention,
    compiler_pass::CompilerPass,
    compose_warnings_guard::ComposeWarningsGuard,
    conformance_config::ConformanceConfig,
    css_renaming_map::CssRenamingMap,
    custom_pass_execution_time::CustomPassExecutionTime,
    default_name_generator::DefaultNameGenerator,
    dependency_options::DependencyOptions,
    deps::module_loader::{ModuleLoader, PathEscaper, ResolutionMode},
    diagnostic_group::DiagnosticGroup,
    diagnostic_group_warnings_guard::DiagnosticGroupWarningsGuard,
    empty_message_bundle::EmptyMessageBundle,
    error_format::ErrorFormat,
    error_handler::ErrorHandler,
    js::runtime_js_lib_manager::RuntimeLibraryMode,
    message_bundle::MessageBundle,
    name_generator::NameGenerator,
    node_util::NodeUtil,
    property_renaming_policy::PropertyRenamingPolicy,
    renaming_map::RenamingMap,
    renaming_token::RenamingToken,
    sorting_error_manager::ErrorReportGenerator,
    source_map::{DetailLevel, Format, LocationMapping},
    source_map_input::SourceMapInput,
    variable_map::VariableMap,
    variable_renaming_policy::VariableRenamingPolicy,
    warnings_guard::WarningsGuard,
    xid::HashFunction,
};
use closure_parsing::{
    config::JsDocParsing,
    parser::feature_set::{Feature, FeatureSet},
};
use closure_rhino::{
    ir::IR,
    java_lang::pattern::Pattern,
    js_string::JsString,
    jscomp_base::{Tri, check_argument, check_state},
    node::{Ast, NodeId},
};
use indexmap::{IndexMap, IndexSet};
// Preserve the public Charset path consumed by the integrated printer API.
pub use closure_rhino::java_lang::charset::Charset;
use std::{
    any::TypeId,
    fmt,
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
pub type OutputStreamWrapper =
    Arc<dyn Fn(Box<dyn Write + Send>) -> Box<dyn Write + Send> + Send + Sync>;
pub type InputStreamWrapper =
    Arc<dyn Fn(Box<dyn Read + Send>) -> Box<dyn Read + Send> + Send + Sync>;
#[derive(Clone, Debug, PartialEq)]
pub enum DefineValue {
    Boolean(bool),
    Integer(i32),
    Double(f64),
    String(JsString),
}
#[derive(Clone)]
pub struct CompilerPassRef(pub Arc<std::sync::Mutex<dyn CompilerPass + Send>>);
impl PartialEq for CompilerPassRef {
    // port: Object#equals (CompilerPass identity)
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CompilerPassRef {}
impl Hash for CompilerPassRef {
    // port: Object#hashCode (CompilerPass identity)
    fn hash<H: Hasher>(&self, h: &mut H) {
        (Arc::as_ptr(&self.0) as *const () as usize).hash(h);
    }
}
#[derive(Clone)]
pub struct CompilerOptions {
    emit_use_strict: Option<bool>,
    language_in: LanguageMode,
    output_feature_set: Option<FeatureSet>,
    experimental_output_feature_set: Option<ExperimentalOutputFeatureSet>,
    language_out_is_default_strict: Option<bool>,
    environment: Environment,
    browser_featureset_year: Option<BrowserFeaturesetYear>,
    instrument_for_coverage_only: bool,
    typed_ast_output_file: Option<PathBuf>,
    merged_precompiled_libraries: bool,
    infer_consts: bool,
    assume_strict_this: bool,
    preserve_detailed_source_info: bool,
    preserve_non_jsdoc_comments: bool,
    continue_after_errors: bool,
    incremental_check_mode: IncrementalCheckMode,
    parse_js_doc_documentation: JsDocParsing,
    print_externs: bool,
    infer_types: bool,
    skip_non_transpilation_passes: bool,
    dev_mode: DevMode,
    check_determinism: bool,
    dependency_options: DependencyOptions,
    message_bundle: Option<Arc<dyn MessageBundle + Send + Sync>>,
    strict_message_replacement: bool,
    check_symbols: bool,
    check_suspicious_code: bool,
    check_types: bool,
    extra_annotation_names: Option<IndexSet<String>>,
    num_parallel_threads: i32,
    fold_constants: bool,
    dead_assignment_elimination: bool,
    dead_property_assignment_elimination: Tri,
    inline_constant_vars: bool,
    max_function_size_after_inlining: i32,
    assume_closures_only_capture_references: bool,
    inline_properties: bool,
    cross_chunk_code_motion: bool,
    cross_chunk_code_motion_no_stub_methods: bool,
    parent_chunk_can_see_symbols_declared_in_children: bool,
    coalesce_variable_names: bool,
    optimize_let_and_const: bool,
    assume_global_scope_is_isolated: bool,
    cross_chunk_method_motion: bool,
    inline_getters: bool,
    inline_variables: bool,
    inline_local_variables: bool,
    flow_sensitive_inline_variables: bool,
    smart_name_removal: bool,
    extract_prototype_member_declarations: ExtractPrototypeMemberDeclarationsMode,
    remove_unused_prototype_properties: bool,
    remove_unused_class_properties: bool,
    remove_unused_vars: bool,
    remove_unused_local_vars: bool,
    collapse_variable_declarations: bool,
    collapse_anonymous_functions: bool,
    alias_strings_mode: AliasStringsMode,
    output_js_string_usage: bool,
    convert_to_dotted_properties: bool,
    rewrite_function_expressions: bool,
    optimize_calls: bool,
    optimize_es_class_constructors: bool,
    use_types_for_local_optimization: bool,
    use_size_heuristic_to_stop_optimization_loop: bool,
    optimization_loop_max_iterations: i32,
    variable_renaming: VariableRenamingPolicy,
    property_renaming: PropertyRenamingPolicy,
    property_renaming_only_compilation_mode: bool,
    label_renaming: bool,
    reserve_raw_exports: bool,
    prefer_stable_names: bool,
    generate_pseudo_names: bool,
    rename_prefix: Option<String>,
    rename_prefix_namespace: Option<String>,
    rename_prefix_namespace_assume_cross_chunk_names: bool,
    optimize_local_access_for_global_symbol_namespace: OptimizeLocalAccess,
    collapse_properties_level: PropertyCollapseLevel,
    collapse_object_literals: bool,
    devirtualize_methods: bool,
    compute_function_side_effects: bool,
    disambiguate_properties: bool,
    ambiguate_properties: bool,
    input_source_maps: IndexMap<String, Arc<SourceMapInput>>,
    input_variable_map: Option<Arc<VariableMap>>,
    input_property_map: Option<Arc<VariableMap>>,
    export_test_functions: bool,
    name_generator: Arc<dyn NameGenerator + Send + Sync>,
    replace_messages_with_chrome_i18n: bool,
    tc_project_id: Option<String>,
    coding_convention: Option<Arc<dyn CodingConvention + Send + Sync>>,
    synthetic_block_start_marker: Option<String>,
    synthetic_block_end_marker: Option<String>,
    locale: Option<String>,
    do_late_localization: bool,
    mark_as_compiled: bool,
    closure_pass: bool,
    preserve_closure_primitives: bool,
    angular_pass: bool,
    polymer_pass: bool,
    chrome_pass: bool,
    j2cl_pass_mode: J2clPassMode,
    j2cl_minifier_enabled: bool,
    j2cl_minifier_pruning_manifest: Option<String>,
    remove_abstract_methods: bool,
    remove_closure_asserts: bool,
    remove_j2cl_asserts: bool,
    gather_css_names: bool,
    strip_types: IndexSet<String>,
    strip_name_suffixes: IndexSet<String>,
    strip_name_prefixes: IndexSet<String>,
    custom_passes: Option<IndexMap<CustomPassExecutionTime, IndexSet<CompilerPassRef>>>,
    define_replacements: IndexMap<String, DefineValue>,
    tweak_processing: TweakProcessing,
    rewrite_global_declarations_for_try_catch_wrapping: bool,
    checks_only: bool,
    output_js: OutputJs,
    generate_exports: bool,
    export_local_property_definitions: bool,
    css_renaming_map: Option<Arc<dyn CssRenamingMap + Send + Sync>>,
    css_renaming_skiplist: Option<IndexSet<String>>,
    replace_id_generators: bool,
    id_generators: IndexMap<String, Arc<dyn RenamingMap + Send + Sync>>,
    xid_hash_function: Option<Arc<dyn HashFunction + Send + Sync>>,
    chunk_id_hash_function: Option<Arc<dyn HashFunction + Send + Sync>>,
    id_generators_map_serialized: Option<String>,
    replace_strings_function_descriptions: Vec<String>,
    replace_strings_placeholder_token: String,
    properties_that_must_disambiguate: IndexSet<String>,
    validate_require_inlining_annotation: Tri,
    process_common_js_modules: bool,
    module_roots: Vec<String>,
    rewrite_polyfills: bool,
    isolate_polyfills: bool,
    inject_polyfills_newer_than: Option<LanguageMode>,
    es6_subclass_transpilation: Es6SubclassTranspilation,
    instrument_async_context: bool,
    force_library_injection: Vec<String>,
    runtime_library_mode: RuntimeLibraryMode,
    assume_forward_declared_for_missing_types: bool,
    unused_imports_to_remove: Option<IndexSet<String>>,
    preserve_type_annotations: bool,
    gents_mode: bool,
    pretty_print: bool,
    line_break: bool,
    print_input_delimiter: bool,
    input_delimiter: String,
    debug_log_directory: Option<PathBuf>,
    debug_log_filter: Option<String>,
    serialize_extra_debug_info: bool,
    state_compression_wrapper: Option<OutputStreamWrapper>,
    state_decompression_wrapper: Option<InputStreamWrapper>,
    quote_keyword_properties: bool,
    prefer_single_quotes: bool,
    trusted_strings: bool,
    print_source_after_each_pass: bool,
    files_to_print_after_each_pass_regex_list: Vec<String>,
    chunks_to_print_after_each_pass_regex_list: Vec<String>,
    qname_uses_to_print_after_each_pass_list: Vec<String>,
    tracer: TracerMode,
    tracer_output: Option<PathBuf>,
    colorize_error_output: bool,
    error_format: ErrorFormat,
    warnings_guard: Arc<ComposeWarningsGuard>,
    summary_detail_level: i32,
    line_length_threshold: i32,
    use_original_names_in_output: bool,
    extern_exports_path: Option<String>,
    extra_report_generators: Vec<Arc<std::sync::Mutex<dyn ErrorReportGenerator + Send>>>,
    source_map_output_path: Option<String>,
    should_always_gather_source_map_info: bool,
    source_map_detail_level: DetailLevel,
    source_map_format: Format,
    parse_inline_source_maps: bool,
    apply_input_source_maps: bool,
    resolve_source_map_annotations: bool,
    source_map_location_mappings: Vec<Arc<dyn LocationMapping + Send + Sync>>,
    source_map_include_sources_content: bool,
    output_charset: Option<Charset>,
    protect_hidden_side_effects: bool,
    assume_getters_are_pure: bool,
    assume_properties_are_statically_analyzable: bool,
    assume_static_inheritance_is_not_used: bool,
    error_handler: Option<Arc<std::sync::Mutex<dyn ErrorHandler + Send>>>,
    instrument_for_coverage_option: InstrumentOption,
    production_instrumentation_array_name: String,
    conformance_configs: Vec<ConformanceConfig>,
    conformance_reporting_mode: ConformanceReportingMode,
    conformance_remove_regex_from_path: Option<Pattern>,
    wrap_goog_modules_for_whitespace_only: bool,
    print_config: bool,
    is_strict_mode_input: Option<bool>,
    rewrite_modules_before_typechecking: bool,
    enable_module_rewriting: bool,
    module_resolution_mode: ResolutionMode,
    browser_resolver_prefix_replacements: IndexMap<String, String>,
    path_escaper: PathEscaper,
    package_json_entry_names: Vec<String>,
    allow_dynamic_import: bool,
    dynamic_import_alias: Option<String>,
    chunk_output_type: ChunkOutputType,
    unknown_defines_to_ignore: Vec<String>,
    inline_functions_level: Reach,
    enable_zones_define_name: Option<String>,
    zone_input_pattern: Option<Pattern>,
    es6_module_transpilation: Es6ModuleTranspilation,
}
// port: CompilerOptions.Reach
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reach {
    ALL,
    LOCAL_ONLY,
    NONE,
}
impl Reach {
    pub const VALUES: &'static [Self] = &[Self::ALL, Self::LOCAL_ONLY, Self::NONE];
    // port: CompilerOptions.Reach#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for Reach {
    // port: CompilerOptions.Reach#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.PropertyCollapseLevel
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyCollapseLevel {
    ALL,
    NONE,
    MODULE_EXPORT,
}
impl PropertyCollapseLevel {
    pub const VALUES: &'static [Self] = &[Self::ALL, Self::NONE, Self::MODULE_EXPORT];
    // port: CompilerOptions.PropertyCollapseLevel#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for PropertyCollapseLevel {
    // port: CompilerOptions.PropertyCollapseLevel#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.BrowserFeaturesetYear
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BrowserFeaturesetYear {
    YEAR_2012,
    YEAR_2018,
    YEAR_2019,
    YEAR_2020,
    YEAR_2021,
    YEAR_2022,
    YEAR_2023,
    YEAR_2024,
    YEAR_2025,
    YEAR_2026,
}
impl BrowserFeaturesetYear {
    pub const VALUES: &'static [Self] = &[
        Self::YEAR_2012,
        Self::YEAR_2018,
        Self::YEAR_2019,
        Self::YEAR_2020,
        Self::YEAR_2021,
        Self::YEAR_2022,
        Self::YEAR_2023,
        Self::YEAR_2024,
        Self::YEAR_2025,
        Self::YEAR_2026,
    ];
    // port: CompilerOptions.BrowserFeaturesetYear#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for BrowserFeaturesetYear {
    // port: CompilerOptions.BrowserFeaturesetYear#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.IncrementalCheckMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IncrementalCheckMode {
    OFF,
    GENERATE_IJS,
}
impl IncrementalCheckMode {
    pub const VALUES: &'static [Self] = &[Self::OFF, Self::GENERATE_IJS];
    // port: CompilerOptions.IncrementalCheckMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for IncrementalCheckMode {
    // port: CompilerOptions.IncrementalCheckMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.ExtractPrototypeMemberDeclarationsMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtractPrototypeMemberDeclarationsMode {
    OFF,
    USE_GLOBAL_TEMP,
    USE_CHUNK_TEMP,
    USE_IIFE,
}
impl ExtractPrototypeMemberDeclarationsMode {
    pub const VALUES: &'static [Self] = &[
        Self::OFF,
        Self::USE_GLOBAL_TEMP,
        Self::USE_CHUNK_TEMP,
        Self::USE_IIFE,
    ];
    // port: CompilerOptions.ExtractPrototypeMemberDeclarationsMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for ExtractPrototypeMemberDeclarationsMode {
    // port: CompilerOptions.ExtractPrototypeMemberDeclarationsMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.OutputJs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputJs {
    NONE,
    SENTINEL,
    NORMAL,
}
impl OutputJs {
    pub const VALUES: &'static [Self] = &[Self::NONE, Self::SENTINEL, Self::NORMAL];
    // port: CompilerOptions.OutputJs#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for OutputJs {
    // port: CompilerOptions.OutputJs#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.ConformanceReportingMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConformanceReportingMode {
    IGNORE_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG,
    RESPECT_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG,
}
impl ConformanceReportingMode {
    pub const VALUES: &'static [Self] = &[
        Self::IGNORE_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG,
        Self::RESPECT_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG,
    ];
    // port: CompilerOptions.ConformanceReportingMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for ConformanceReportingMode {
    // port: CompilerOptions.ConformanceReportingMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.ExperimentalOutputFeatureSet
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExperimentalOutputFeatureSet {
    ES5_WITH_SOME_PERFORMANT_ES2015_AND_ASYNC_FUNCTIONS,
    BROWSER_2019_WITHOUT_CLASSES_AND_SPREAD,
}
impl ExperimentalOutputFeatureSet {
    pub const VALUES: &'static [Self] = &[
        Self::ES5_WITH_SOME_PERFORMANT_ES2015_AND_ASYNC_FUNCTIONS,
        Self::BROWSER_2019_WITHOUT_CLASSES_AND_SPREAD,
    ];
    // port: CompilerOptions.ExperimentalOutputFeatureSet#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for ExperimentalOutputFeatureSet {
    // port: CompilerOptions.ExperimentalOutputFeatureSet#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.Es6ModuleTranspilation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Es6ModuleTranspilation {
    NONE,
    RELATIVIZE_IMPORT_PATHS,
    TO_COMMON_JS_LIKE_MODULES,
    COMPILE,
}
impl Es6ModuleTranspilation {
    pub const VALUES: &'static [Self] = &[
        Self::NONE,
        Self::RELATIVIZE_IMPORT_PATHS,
        Self::TO_COMMON_JS_LIKE_MODULES,
        Self::COMPILE,
    ];
    // port: CompilerOptions.Es6ModuleTranspilation#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for Es6ModuleTranspilation {
    // port: CompilerOptions.Es6ModuleTranspilation#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.Es6SubclassTranspilation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Es6SubclassTranspilation {
    CONCISE_UNSAFE,
    SAFE_REFLECT_CONSTRUCT,
}
impl Es6SubclassTranspilation {
    pub const VALUES: &'static [Self] = &[Self::CONCISE_UNSAFE, Self::SAFE_REFLECT_CONSTRUCT];
    // port: CompilerOptions.Es6SubclassTranspilation#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for Es6SubclassTranspilation {
    // port: CompilerOptions.Es6SubclassTranspilation#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.InstrumentOption
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstrumentOption {
    NONE,
    LINE_ONLY,
    BRANCH_ONLY,
    PRODUCTION,
}
impl InstrumentOption {
    pub const VALUES: &'static [Self] = &[
        Self::NONE,
        Self::LINE_ONLY,
        Self::BRANCH_ONLY,
        Self::PRODUCTION,
    ];
    // port: CompilerOptions.InstrumentOption#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for InstrumentOption {
    // port: CompilerOptions.InstrumentOption#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.ChunkOutputType
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChunkOutputType {
    GLOBAL_NAMESPACE,
    ES_MODULES,
}
impl ChunkOutputType {
    pub const VALUES: &'static [Self] = &[Self::GLOBAL_NAMESPACE, Self::ES_MODULES];
    // port: CompilerOptions.ChunkOutputType#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for ChunkOutputType {
    // port: CompilerOptions.ChunkOutputType#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.OptimizeLocalAccess
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptimizeLocalAccess {
    DISABLED,
    DEFINING_CHUNK_ONLY,
    ALL_CHUNKS,
    ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS,
}
impl OptimizeLocalAccess {
    pub const VALUES: &'static [Self] = &[
        Self::DISABLED,
        Self::DEFINING_CHUNK_ONLY,
        Self::ALL_CHUNKS,
        Self::ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS,
    ];
    // port: CompilerOptions.OptimizeLocalAccess#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for OptimizeLocalAccess {
    // port: CompilerOptions.OptimizeLocalAccess#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.LanguageMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LanguageMode {
    ECMASCRIPT3,
    ECMASCRIPT5,
    ECMASCRIPT5_STRICT,
    ECMASCRIPT_2015,
    ECMASCRIPT_2016,
    ECMASCRIPT_2017,
    ECMASCRIPT_2018,
    ECMASCRIPT_2019,
    ECMASCRIPT_2020,
    ECMASCRIPT_2021,
    ECMASCRIPT_2022,
    ECMASCRIPT_NEXT,
    STABLE,
    NO_TRANSPILE,
    UNSTABLE,
    UNSUPPORTED,
}
impl LanguageMode {
    pub const VALUES: &'static [Self] = &[
        Self::ECMASCRIPT3,
        Self::ECMASCRIPT5,
        Self::ECMASCRIPT5_STRICT,
        Self::ECMASCRIPT_2015,
        Self::ECMASCRIPT_2016,
        Self::ECMASCRIPT_2017,
        Self::ECMASCRIPT_2018,
        Self::ECMASCRIPT_2019,
        Self::ECMASCRIPT_2020,
        Self::ECMASCRIPT_2021,
        Self::ECMASCRIPT_2022,
        Self::ECMASCRIPT_NEXT,
        Self::STABLE,
        Self::NO_TRANSPILE,
        Self::UNSTABLE,
        Self::UNSUPPORTED,
    ];
    // port: CompilerOptions.LanguageMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for LanguageMode {
    // port: CompilerOptions.LanguageMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.DevMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DevMode {
    OFF,
    START,
    START_AND_END,
    EVERY_PASS,
}
impl DevMode {
    pub const VALUES: &'static [Self] = &[
        Self::OFF,
        Self::START,
        Self::START_AND_END,
        Self::EVERY_PASS,
    ];
    // port: CompilerOptions.DevMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for DevMode {
    // port: CompilerOptions.DevMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.TracerMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TracerMode {
    ALL,
    RAW_SIZE,
    AST_SIZE_AND_PRUNING,
    AST_SIZE,
    TIMING_ONLY,
    OFF,
}
impl TracerMode {
    pub const VALUES: &'static [Self] = &[
        Self::ALL,
        Self::RAW_SIZE,
        Self::AST_SIZE_AND_PRUNING,
        Self::AST_SIZE,
        Self::TIMING_ONLY,
        Self::OFF,
    ];
    // port: CompilerOptions.TracerMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for TracerMode {
    // port: CompilerOptions.TracerMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.TweakProcessing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TweakProcessing {
    OFF,
    CHECK,
    STRIP,
}
impl TweakProcessing {
    pub const VALUES: &'static [Self] = &[Self::OFF, Self::CHECK, Self::STRIP];
    // port: CompilerOptions.TweakProcessing#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for TweakProcessing {
    // port: CompilerOptions.TweakProcessing#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.IsolationMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IsolationMode {
    NONE,
    IIFE,
}
impl IsolationMode {
    pub const VALUES: &'static [Self] = &[Self::NONE, Self::IIFE];
    // port: CompilerOptions.IsolationMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for IsolationMode {
    // port: CompilerOptions.IsolationMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.SegmentOfCompilationToRun
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SegmentOfCompilationToRun {
    ENTIRE_COMPILATION,
    CHECKS,
    OPTIMIZATIONS_FIRST_HALF,
    OPTIMIZATIONS_SECOND_HALF,
    OPTIMIZATIONS,
    OPTIMIZATIONS_AND_FINALIZATIONS,
    FINALIZATIONS,
}
impl SegmentOfCompilationToRun {
    pub const VALUES: &'static [Self] = &[
        Self::ENTIRE_COMPILATION,
        Self::CHECKS,
        Self::OPTIMIZATIONS_FIRST_HALF,
        Self::OPTIMIZATIONS_SECOND_HALF,
        Self::OPTIMIZATIONS,
        Self::OPTIMIZATIONS_AND_FINALIZATIONS,
        Self::FINALIZATIONS,
    ];
    // port: CompilerOptions.SegmentOfCompilationToRun#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for SegmentOfCompilationToRun {
    // port: CompilerOptions.SegmentOfCompilationToRun#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.AliasStringsMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AliasStringsMode {
    NONE,
    LARGE,
    ALL,
    ALL_AGGRESSIVE,
}
impl AliasStringsMode {
    pub const VALUES: &'static [Self] = &[Self::NONE, Self::LARGE, Self::ALL, Self::ALL_AGGRESSIVE];
    // port: CompilerOptions.AliasStringsMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for AliasStringsMode {
    // port: CompilerOptions.AliasStringsMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.Environment
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Environment {
    BROWSER,
    CUSTOM,
}
impl Environment {
    pub const VALUES: &'static [Self] = &[Self::BROWSER, Self::CUSTOM];
    // port: CompilerOptions.Environment#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for Environment {
    // port: CompilerOptions.Environment#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.JsonStreamMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JsonStreamMode {
    NONE,
    IN,
    OUT,
    BOTH,
}
impl JsonStreamMode {
    pub const VALUES: &'static [Self] = &[Self::NONE, Self::IN, Self::OUT, Self::BOTH];
    // port: CompilerOptions.JsonStreamMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for JsonStreamMode {
    // port: CompilerOptions.JsonStreamMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: CompilerOptions.J2clPassMode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum J2clPassMode {
    OFF,
    AUTO,
}
impl J2clPassMode {
    pub const VALUES: &'static [Self] = &[Self::OFF, Self::AUTO];
    // port: CompilerOptions.J2clPassMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for J2clPassMode {
    // port: CompilerOptions.J2clPassMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl CompilerOptions {
    pub const DEFAULT_LINE_LENGTH_THRESHOLD: i32 = 500;
    pub const UNLIMITED_FUN_SIZE_AFTER_INLINING: i32 = -1;
    // port: CompilerOptions#CompilerOptions
    pub fn new() -> Self {
        let emit_use_strict = None;
        let output_feature_set = None;
        let experimental_output_feature_set = None;
        let language_out_is_default_strict = None;
        let instrument_for_coverage_only = false;
        let typed_ast_output_file = None;
        let merged_precompiled_libraries = false;
        let infer_consts = true;
        let preserve_detailed_source_info = false;
        let preserve_non_jsdoc_comments = false;
        let continue_after_errors = false;
        let incremental_check_mode = IncrementalCheckMode::OFF;
        let parse_js_doc_documentation = JsDocParsing::TYPES_ONLY;
        let dependency_options = DependencyOptions::none();
        let message_bundle = None;
        let num_parallel_threads = 1;
        let use_size_heuristic_to_stop_optimization_loop = true;
        let rename_prefix_namespace_assume_cross_chunk_names = false;
        let optimize_local_access_for_global_symbol_namespace = OptimizeLocalAccess::DISABLED;
        let j2cl_minifier_pruning_manifest = None;
        let remove_j2cl_asserts = true;
        let replace_id_generators = true;
        let process_common_js_modules = false;
        let module_roots = vec![ModuleLoader::DEFAULT_FILENAME_PREFIX.to_string()];
        let rewrite_polyfills = false;
        let isolate_polyfills = false;
        let inject_polyfills_newer_than = None;
        let es6_subclass_transpilation = Es6SubclassTranspilation::CONCISE_UNSAFE;
        let instrument_async_context = false;
        let force_library_injection = Vec::new();
        let runtime_library_mode = RuntimeLibraryMode::INJECT;
        let assume_forward_declared_for_missing_types = false;
        let input_delimiter = "// Input %num%".to_string();
        let state_compression_wrapper = None;
        let state_decompression_wrapper = None;
        let files_to_print_after_each_pass_regex_list = Vec::new();
        let chunks_to_print_after_each_pass_regex_list = Vec::new();
        let qname_uses_to_print_after_each_pass_list = Vec::new();
        let warnings_guard = Arc::new(ComposeWarningsGuard::new(Vec::new()));
        let summary_detail_level = 1;
        let line_length_threshold = Self::DEFAULT_LINE_LENGTH_THRESHOLD;
        let use_original_names_in_output = false;
        let extra_report_generators = Vec::new();
        let should_always_gather_source_map_info = false;
        let source_map_detail_level = DetailLevel::ALL;
        let source_map_format = Format::DEFAULT;
        let parse_inline_source_maps = true;
        let apply_input_source_maps = false;
        let resolve_source_map_annotations = true;
        let source_map_location_mappings = Vec::new();
        let source_map_include_sources_content = false;
        let assume_getters_are_pure = true;
        let assume_properties_are_statically_analyzable = true;
        let assume_static_inheritance_is_not_used = true;
        let conformance_configs = Vec::new();
        let conformance_reporting_mode =
            ConformanceReportingMode::IGNORE_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG;
        let conformance_remove_regex_from_path = Some(Pattern::compile(
            "^((.*/)?google3/)?(/?(blaze|bazel)-out/[^/]+/(bin|genfiles)/)?",
        ));
        let wrap_goog_modules_for_whitespace_only = true;
        let print_config = false;
        let is_strict_mode_input = None;
        let allow_dynamic_import = false;
        let dynamic_import_alias = None;
        let unknown_defines_to_ignore = Vec::new();
        let enable_zones_define_name = None;
        let zone_input_pattern = None;
        let es6_module_transpilation = Es6ModuleTranspilation::COMPILE;
        let language_in = LanguageMode::STABLE_IN;
        let environment = Environment::BROWSER;
        let browser_resolver_prefix_replacements = IndexMap::new();
        let module_resolution_mode = ResolutionMode::BROWSER;
        let package_json_entry_names = vec!["browser".into(), "module".into(), "main".into()];
        let path_escaper = PathEscaper::ESCAPE;
        let rewrite_modules_before_typechecking = false;
        let enable_module_rewriting = true;
        let skip_non_transpilation_passes = false;
        let dev_mode = DevMode::OFF;
        let check_determinism = false;
        let check_symbols = false;
        let check_suspicious_code = false;
        let check_types = false;
        let compute_function_side_effects = false;
        let extra_annotation_names = None;
        let fold_constants = false;
        let coalesce_variable_names = false;
        let optimize_let_and_const = true;
        let assume_global_scope_is_isolated = false;
        let dead_assignment_elimination = false;
        let dead_property_assignment_elimination = Tri::UNKNOWN;
        let inline_constant_vars = false;
        let inline_functions_level = Reach::NONE;
        let max_function_size_after_inlining = Self::UNLIMITED_FUN_SIZE_AFTER_INLINING;
        let assume_strict_this = false;
        let assume_closures_only_capture_references = false;
        let inline_properties = false;
        let cross_chunk_code_motion = false;
        let parent_chunk_can_see_symbols_declared_in_children = false;
        let cross_chunk_method_motion = false;
        let inline_getters = false;
        let inline_variables = false;
        let inline_local_variables = false;
        let smart_name_removal = false;
        let extract_prototype_member_declarations = ExtractPrototypeMemberDeclarationsMode::OFF;
        let remove_unused_prototype_properties = false;
        let remove_unused_class_properties = false;
        let remove_unused_vars = false;
        let remove_unused_local_vars = false;
        let collapse_variable_declarations = false;
        let collapse_anonymous_functions = false;
        let alias_strings_mode = AliasStringsMode::NONE;
        let output_js_string_usage = false;
        let convert_to_dotted_properties = false;
        let rewrite_function_expressions = false;
        let validate_require_inlining_annotation = Tri::UNKNOWN;
        let variable_renaming = VariableRenamingPolicy::OFF;
        let property_renaming = PropertyRenamingPolicy::OFF;
        let property_renaming_only_compilation_mode = false;
        let label_renaming = false;
        let generate_pseudo_names = false;
        let prefer_stable_names = false;
        let rename_prefix = None;
        let collapse_properties_level = PropertyCollapseLevel::NONE;
        let collapse_object_literals = false;
        let devirtualize_methods = false;
        let disambiguate_properties = false;
        let ambiguate_properties = false;
        let export_test_functions = false;
        let name_generator = Arc::new(DefaultNameGenerator::new());
        let synthetic_block_start_marker = None;
        let synthetic_block_end_marker = None;
        let locale = None;
        let do_late_localization = false;
        let mark_as_compiled = false;
        let closure_pass = false;
        let preserve_closure_primitives = false;
        let angular_pass = false;
        let polymer_pass = false;
        let j2cl_pass_mode = J2clPassMode::AUTO;
        let j2cl_minifier_enabled = true;
        let remove_abstract_methods = false;
        let remove_closure_asserts = false;
        let strip_types = IndexSet::new();
        let strip_name_suffixes = IndexSet::new();
        let strip_name_prefixes = IndexSet::new();
        let custom_passes = None;
        let define_replacements = IndexMap::new();
        let tweak_processing = TweakProcessing::OFF;
        let rewrite_global_declarations_for_try_catch_wrapping = false;
        let checks_only = false;
        let output_js = OutputJs::NORMAL;
        let generate_exports = true;
        let export_local_property_definitions = true;
        let css_renaming_map = None;
        let css_renaming_skiplist = None;
        let id_generators = IndexMap::new();
        let replace_strings_function_descriptions = Vec::new();
        let replace_strings_placeholder_token = String::new();
        let properties_that_must_disambiguate = IndexSet::new();
        let input_source_maps = IndexMap::new();
        let instrument_for_coverage_option = InstrumentOption::NONE;
        let production_instrumentation_array_name = String::new();
        let preserve_type_annotations = false;
        let gents_mode = false;
        let print_input_delimiter = false;
        let pretty_print = false;
        let line_break = false;
        let tracer = TracerMode::OFF;
        let colorize_error_output = false;
        let error_format = ErrorFormat::FULL;
        let chunk_output_type = ChunkOutputType::GLOBAL_NAMESPACE;
        let error_handler = None;
        let print_source_after_each_pass = false;
        let strict_message_replacement = false;
        Self {
            emit_use_strict,
            language_in,
            output_feature_set,
            experimental_output_feature_set,
            language_out_is_default_strict,
            environment,
            browser_featureset_year: None,
            instrument_for_coverage_only,
            typed_ast_output_file,
            merged_precompiled_libraries,
            infer_consts,
            assume_strict_this,
            preserve_detailed_source_info,
            preserve_non_jsdoc_comments,
            continue_after_errors,
            incremental_check_mode,
            parse_js_doc_documentation,
            print_externs: false,
            infer_types: false,
            skip_non_transpilation_passes,
            dev_mode,
            check_determinism,
            dependency_options,
            message_bundle,
            strict_message_replacement,
            check_symbols,
            check_suspicious_code,
            check_types,
            extra_annotation_names,
            num_parallel_threads,
            fold_constants,
            dead_assignment_elimination,
            dead_property_assignment_elimination,
            inline_constant_vars,
            max_function_size_after_inlining,
            assume_closures_only_capture_references,
            inline_properties,
            cross_chunk_code_motion,
            cross_chunk_code_motion_no_stub_methods: false,
            parent_chunk_can_see_symbols_declared_in_children,
            coalesce_variable_names,
            optimize_let_and_const,
            assume_global_scope_is_isolated,
            cross_chunk_method_motion,
            inline_getters,
            inline_variables,
            inline_local_variables,
            flow_sensitive_inline_variables: false,
            smart_name_removal,
            extract_prototype_member_declarations,
            remove_unused_prototype_properties,
            remove_unused_class_properties,
            remove_unused_vars,
            remove_unused_local_vars,
            collapse_variable_declarations,
            collapse_anonymous_functions,
            alias_strings_mode,
            output_js_string_usage,
            convert_to_dotted_properties,
            rewrite_function_expressions,
            optimize_calls: false,
            optimize_es_class_constructors: false,
            use_types_for_local_optimization: false,
            use_size_heuristic_to_stop_optimization_loop,
            optimization_loop_max_iterations: 0,
            variable_renaming,
            property_renaming,
            property_renaming_only_compilation_mode,
            label_renaming,
            reserve_raw_exports: false,
            prefer_stable_names,
            generate_pseudo_names,
            rename_prefix,
            rename_prefix_namespace: None,
            rename_prefix_namespace_assume_cross_chunk_names,
            optimize_local_access_for_global_symbol_namespace,
            collapse_properties_level,
            collapse_object_literals,
            devirtualize_methods,
            compute_function_side_effects,
            disambiguate_properties,
            ambiguate_properties,
            input_source_maps,
            input_variable_map: None,
            input_property_map: None,
            export_test_functions,
            name_generator,
            replace_messages_with_chrome_i18n: false,
            tc_project_id: None,
            coding_convention: None,
            synthetic_block_start_marker,
            synthetic_block_end_marker,
            locale,
            do_late_localization,
            mark_as_compiled,
            closure_pass,
            preserve_closure_primitives,
            angular_pass,
            polymer_pass,
            chrome_pass: false,
            j2cl_pass_mode,
            j2cl_minifier_enabled,
            j2cl_minifier_pruning_manifest,
            remove_abstract_methods,
            remove_closure_asserts,
            remove_j2cl_asserts,
            gather_css_names: false,
            strip_types,
            strip_name_suffixes,
            strip_name_prefixes,
            custom_passes,
            define_replacements,
            tweak_processing,
            rewrite_global_declarations_for_try_catch_wrapping,
            checks_only,
            output_js,
            generate_exports,
            export_local_property_definitions,
            css_renaming_map,
            css_renaming_skiplist,
            replace_id_generators,
            id_generators,
            xid_hash_function: None,
            chunk_id_hash_function: None,
            id_generators_map_serialized: None,
            replace_strings_function_descriptions,
            replace_strings_placeholder_token,
            properties_that_must_disambiguate,
            validate_require_inlining_annotation,
            process_common_js_modules,
            module_roots,
            rewrite_polyfills,
            isolate_polyfills,
            inject_polyfills_newer_than,
            es6_subclass_transpilation,
            instrument_async_context,
            force_library_injection,
            runtime_library_mode,
            assume_forward_declared_for_missing_types,
            unused_imports_to_remove: None,
            preserve_type_annotations,
            gents_mode,
            pretty_print,
            line_break,
            print_input_delimiter,
            input_delimiter,
            debug_log_directory: None,
            debug_log_filter: None,
            serialize_extra_debug_info: false,
            state_compression_wrapper,
            state_decompression_wrapper,
            quote_keyword_properties: false,
            prefer_single_quotes: false,
            trusted_strings: false,
            print_source_after_each_pass,
            files_to_print_after_each_pass_regex_list,
            chunks_to_print_after_each_pass_regex_list,
            qname_uses_to_print_after_each_pass_list,
            tracer,
            tracer_output: None,
            colorize_error_output,
            error_format,
            warnings_guard,
            summary_detail_level,
            line_length_threshold,
            use_original_names_in_output,
            extern_exports_path: None,
            extra_report_generators,
            source_map_output_path: None,
            should_always_gather_source_map_info,
            source_map_detail_level,
            source_map_format,
            parse_inline_source_maps,
            apply_input_source_maps,
            resolve_source_map_annotations,
            source_map_location_mappings,
            source_map_include_sources_content,
            output_charset: None,
            protect_hidden_side_effects: false,
            assume_getters_are_pure,
            assume_properties_are_statically_analyzable,
            assume_static_inheritance_is_not_used,
            error_handler,
            instrument_for_coverage_option,
            production_instrumentation_array_name,
            conformance_configs,
            conformance_reporting_mode,
            conformance_remove_regex_from_path,
            wrap_goog_modules_for_whitespace_only,
            print_config,
            is_strict_mode_input,
            rewrite_modules_before_typechecking,
            enable_module_rewriting,
            module_resolution_mode,
            browser_resolver_prefix_replacements,
            path_escaper,
            package_json_entry_names,
            allow_dynamic_import,
            dynamic_import_alias,
            chunk_output_type,
            unknown_defines_to_ignore,
            inline_functions_level,
            enable_zones_define_name,
            zone_input_pattern,
            es6_module_transpilation,
        }
    }
    // port: CompilerOptions#getAngularPropertyReservedFirstChars
    pub fn get_angular_property_reserved_first_chars() -> IndexSet<u16> {
        Self::ANGULAR_PROPERTY_RESERVED_FIRST_CHARS
            .iter()
            .copied()
            .collect()
    }
    // port: CompilerOptions#getPolymerPropertyReservedFirstChars
    pub fn get_polymer_property_reserved_first_chars() -> IndexSet<u16> {
        Self::POLYMER_PROPERTY_RESERVED_FIRST_CHARS
            .encode_utf16()
            .collect()
    }
    // port: CompilerOptions#shouldRunCrossChunkCodeMotion
    pub fn should_run_cross_chunk_code_motion(&self) -> bool {
        self.cross_chunk_code_motion
    }
    // port: CompilerOptions#shouldRunCrossChunkMethodMotion
    pub fn should_run_cross_chunk_method_motion(&self) -> bool {
        self.cross_chunk_method_motion
    }
    // port: CompilerOptions#getSourceMapOutputPath
    pub fn get_source_map_output_path(&self) -> Option<&str> {
        self.source_map_output_path.as_deref()
    }
    // port: CompilerOptions#shouldGatherSourceMapInfo
    pub fn should_gather_source_map_info(&self) -> bool {
        self.should_always_gather_source_map_info
            || self
                .source_map_output_path
                .as_ref()
                .is_some_and(|p| !p.is_empty())
    }
    // port: CompilerOptions#setAlwaysGatherSourceMapInfo
    pub fn set_always_gather_source_map_info(
        &mut self,
        should_always_gather_source_map_info: bool,
    ) {
        self.should_always_gather_source_map_info = should_always_gather_source_map_info;
    }
    // port: CompilerOptions#getBrowserFeaturesetYearObject
    pub fn get_browser_featureset_year_object(&self) -> Option<BrowserFeaturesetYear> {
        self.browser_featureset_year
    }
    // port: CompilerOptions#setBrowserFeaturesetYear(int)
    pub fn set_browser_featureset_year(&mut self, year: i32) {
        self.set_browser_featureset_year_object(BrowserFeaturesetYear::from(year));
    }
    // port: CompilerOptions#setBrowserFeaturesetYear(BrowserFeaturesetYear)
    pub fn set_browser_featureset_year_object(&mut self, year: BrowserFeaturesetYear) {
        self.browser_featureset_year = Some(year);
        year.set_dependent_values_from_year(self);
    }
    // port: CompilerOptions#setInstrumentForCoverageOnly
    pub fn set_instrument_for_coverage_only(&mut self, instrument_for_coverage_only: bool) {
        self.instrument_for_coverage_only = instrument_for_coverage_only;
    }
    // port: CompilerOptions#getInstrumentForCoverageOnly
    pub fn get_instrument_for_coverage_only(&self) -> bool {
        self.instrument_for_coverage_only
    }
    // port: CompilerOptions#setTypedAstOutputFile
    pub fn set_typed_ast_output_file(&mut self, file: Option<PathBuf>) {
        self.typed_ast_output_file = file;
    }
    // port: CompilerOptions#getTypedAstOutputFile
    pub fn get_typed_ast_output_file(&self) -> Option<&Path> {
        self.typed_ast_output_file.as_deref()
    }
    // port: CompilerOptions#setMergedPrecompiledLibraries
    pub fn set_merged_precompiled_libraries(&mut self, merged_precompiled_libraries: bool) {
        self.merged_precompiled_libraries = merged_precompiled_libraries;
    }
    // port: CompilerOptions#getMergedPrecompiledLibraries
    pub fn get_merged_precompiled_libraries(&self) -> bool {
        self.merged_precompiled_libraries
    }
    // port: CompilerOptions#setSkipTranspilationAndCrash
    pub fn set_skip_transpilation_and_crash(&mut self, _value: bool) {}
    // port: CompilerOptions#setInputSourceMaps
    pub fn set_input_source_maps(
        &mut self,
        input_source_maps: IndexMap<String, Arc<SourceMapInput>>,
    ) {
        self.input_source_maps = input_source_maps;
    }
    // port: CompilerOptions#getInputSourceMaps
    pub fn get_input_source_maps(&self) -> &IndexMap<String, Arc<SourceMapInput>> {
        &self.input_source_maps
    }
    // port: CompilerOptions#setInferConst
    pub fn set_infer_const(&mut self, value: bool) {
        self.infer_consts = value;
    }
    // port: CompilerOptions#shouldInferConsts
    pub fn should_infer_consts(&self) -> bool {
        self.infer_consts
    }
    // port: CompilerOptions#setIncrementalChecks
    pub fn set_incremental_checks(&mut self, value: IncrementalCheckMode) {
        self.incremental_check_mode = value;
        match value {
            IncrementalCheckMode::OFF => {}
            IncrementalCheckMode::GENERATE_IJS => {
                self.set_preserve_type_annotations(true);
                self.set_output_js(OutputJs::NORMAL);
            }
        }
    }
    // port: CompilerOptions#shouldGenerateTypedExterns
    pub fn should_generate_typed_externs(&self) -> bool {
        self.incremental_check_mode == IncrementalCheckMode::GENERATE_IJS
    }
    // port: CompilerOptions#setPrintExterns
    pub fn set_print_externs(&mut self, print_externs: bool) {
        self.print_externs = print_externs;
    }
    // port: CompilerOptions#shouldPrintExterns
    pub fn should_print_externs(&self) -> bool {
        self.print_externs || self.incremental_check_mode == IncrementalCheckMode::GENERATE_IJS
    }
    // port: CompilerOptions#setCheckGlobalThisLevel
    pub fn set_check_global_this_level(&mut self, _level: CheckLevel) {}
    // port: CompilerOptions#setNumParallelThreads
    pub fn set_num_parallel_threads(&mut self, parallelism: i32) {
        self.num_parallel_threads = parallelism;
    }
    // port: CompilerOptions#getNumParallelThreads
    pub fn get_num_parallel_threads(&self) -> i32 {
        self.num_parallel_threads
    }
    // port: CompilerOptions#setRenamePrefixNamespaceAssumeCrossChunkNames
    pub fn set_rename_prefix_namespace_assume_cross_chunk_names(&mut self, assume: bool) {
        self.rename_prefix_namespace_assume_cross_chunk_names = assume;
    }
    // port: CompilerOptions#assumeCrossChunkNamesForRenamePrefixNamespace
    pub fn assume_cross_chunk_names_for_rename_prefix_namespace(&self) -> bool {
        self.rename_prefix_namespace_assume_cross_chunk_names
    }
    // port: CompilerOptions#setOptimizeLocalAccessForGlobalSymbolNamespace
    pub fn set_optimize_local_access_for_global_symbol_namespace(
        &mut self,
        optimize: OptimizeLocalAccess,
    ) {
        self.optimize_local_access_for_global_symbol_namespace = optimize;
    }
    // port: CompilerOptions#getOptimizeLocalAccessForGlobalSymbolNamespace
    pub fn get_optimize_local_access_for_global_symbol_namespace(&self) -> OptimizeLocalAccess {
        self.optimize_local_access_for_global_symbol_namespace
    }
    // port: CompilerOptions#shouldCollapseProperties
    pub fn should_collapse_properties(&self) -> bool {
        self.collapse_properties_level != PropertyCollapseLevel::NONE
    }
    // port: CompilerOptions#getPropertyCollapseLevel
    pub fn get_property_collapse_level(&self) -> PropertyCollapseLevel {
        self.collapse_properties_level
    }
    // port: CompilerOptions#setCollapseObjectLiterals
    pub fn set_collapse_object_literals(&mut self, enabled: bool) {
        self.collapse_object_literals = enabled;
    }
    // port: CompilerOptions#getCollapseObjectLiterals
    pub fn get_collapse_object_literals(&self) -> bool {
        self.collapse_object_literals
    }
    // port: CompilerOptions#setNameGenerator
    pub fn set_name_generator(&mut self, name_generator: Arc<dyn NameGenerator + Send + Sync>) {
        self.name_generator = name_generator;
    }
    // port: CompilerOptions#getNameGenerator
    pub fn get_name_generator(&self) -> &Arc<dyn NameGenerator + Send + Sync> {
        &self.name_generator
    }
    // port: CompilerOptions#setReplaceMessagesWithChromeI18n
    pub fn set_replace_messages_with_chrome_i18n(
        &mut self,
        replace_messages_with_chrome_i18n: bool,
        tc_project_id: String,
    ) {
        if replace_messages_with_chrome_i18n
            && self
                .message_bundle
                .as_ref()
                .is_some_and(|b| b.as_ref().type_id() != TypeId::of::<EmptyMessageBundle>())
        {
            panic!(
                "When replacing messages with chrome.i18n.getMessage, a message bundle should not be specified."
            );
        }
        self.replace_messages_with_chrome_i18n = replace_messages_with_chrome_i18n;
        self.tc_project_id = Some(tc_project_id);
    }
    // port: CompilerOptions#getTcProjectId
    pub fn get_tc_project_id(&self) -> Option<&str> {
        self.tc_project_id.as_deref()
    }
    // port: CompilerOptions#shouldRunReplaceMessagesForChrome
    pub fn should_run_replace_messages_for_chrome(&self) -> bool {
        if self.replace_messages_with_chrome_i18n {
            check_state!(
                self.message_bundle
                    .as_ref()
                    .is_none_or(|b| b.as_ref().type_id() == TypeId::of::<EmptyMessageBundle>()),
                "When replacing messages with chrome.i18n.getMessage, a message bundle should not be specified."
            );
            check_state!(
                !self.do_late_localization,
                "Late localization is not supported for chrome.i18n.getMessage"
            );
            true
        } else {
            false
        }
    }
    // port: CompilerOptions#setAssumeForwardDeclaredForMissingTypes
    pub fn set_assume_forward_declared_for_missing_types(
        &mut self,
        assume_forward_declared_for_missing_types: bool,
    ) {
        self.assume_forward_declared_for_missing_types = assume_forward_declared_for_missing_types;
    }
    // port: CompilerOptions#assumeForwardDeclaredForMissingTypes
    pub fn assume_forward_declared_for_missing_types(&self) -> bool {
        self.assume_forward_declared_for_missing_types
    }
    // port: CompilerOptions#setPreferSingleQuotes
    pub fn set_prefer_single_quotes(&mut self, enabled: bool) {
        self.prefer_single_quotes = enabled;
    }
    // port: CompilerOptions#shouldPreferSingleQuotes
    pub fn should_prefer_single_quotes(&self) -> bool {
        self.prefer_single_quotes
    }
    // port: CompilerOptions#setTrustedStrings
    pub fn set_trusted_strings(&mut self, yes: bool) {
        self.trusted_strings = yes;
    }
    // port: CompilerOptions#assumeTrustedStrings
    pub fn assume_trusted_strings(&self) -> bool {
        self.trusted_strings
    }
    // port: CompilerOptions#setPrintSourceAfterEachPass
    pub fn set_print_source_after_each_pass(&mut self, print_source: bool) {
        self.print_source_after_each_pass = print_source;
    }
    // port: CompilerOptions#shouldPrintSourceAfterEachPass
    pub fn should_print_source_after_each_pass(&self) -> bool {
        self.print_source_after_each_pass
    }
    // port: CompilerOptions#setFilesToPrintAfterEachPassRegexList
    pub fn set_files_to_print_after_each_pass_regex_list(
        &mut self,
        file_path_regex_list: Vec<String>,
    ) {
        self.files_to_print_after_each_pass_regex_list = file_path_regex_list;
    }
    // port: CompilerOptions#setChunksToPrintAfterEachPassRegexList
    pub fn set_chunks_to_print_after_each_pass_regex_list(
        &mut self,
        chunk_path_regex_list: Vec<String>,
    ) {
        self.chunks_to_print_after_each_pass_regex_list = chunk_path_regex_list;
    }
    // port: CompilerOptions#setQnameUsesToPrintAfterEachPassList
    pub fn set_qname_uses_to_print_after_each_pass_list(&mut self, qname_regex_list: Vec<String>) {
        self.qname_uses_to_print_after_each_pass_list = qname_regex_list;
    }
    // port: CompilerOptions#getFilesToPrintAfterEachPassRegexList
    pub fn get_files_to_print_after_each_pass_regex_list(&self) -> &Vec<String> {
        &self.files_to_print_after_each_pass_regex_list
    }
    // port: CompilerOptions#getChunksToPrintAfterEachPassRegexList
    pub fn get_chunks_to_print_after_each_pass_regex_list(&self) -> &Vec<String> {
        &self.chunks_to_print_after_each_pass_regex_list
    }
    // port: CompilerOptions#getQnameUsesToPrintAfterEachPassList
    pub fn get_qname_uses_to_print_after_each_pass_list(&self) -> &Vec<String> {
        &self.qname_uses_to_print_after_each_pass_list
    }
    // port: CompilerOptions#getTracerMode
    pub fn get_tracer_mode(&self) -> TracerMode {
        self.tracer
    }
    // port: CompilerOptions#setTracerMode
    pub fn set_tracer_mode(&mut self, mode: TracerMode) {
        self.tracer = mode;
    }
    // port: CompilerOptions#getTracerOutput
    pub fn get_tracer_output(&self) -> Option<&Path> {
        self.tracer_output.as_deref()
    }
    // port: CompilerOptions#setTracerOutput
    pub fn set_tracer_output(&mut self, out: PathBuf) {
        self.tracer_output = Some(out);
    }
    // port: CompilerOptions#getExtraReportGenerators
    pub fn get_extra_report_generators(
        &self,
    ) -> &Vec<Arc<std::sync::Mutex<dyn ErrorReportGenerator + Send>>> {
        &self.extra_report_generators
    }
    // port: CompilerOptions#addReportGenerator
    pub fn add_report_generator(
        &mut self,
        generator: Arc<std::sync::Mutex<dyn ErrorReportGenerator + Send>>,
    ) {
        self.extra_report_generators.push(generator);
    }
    // port: CompilerOptions#setProtectHiddenSideEffects
    pub fn set_protect_hidden_side_effects(&mut self, enable: bool) {
        self.protect_hidden_side_effects = enable;
    }
    // port: CompilerOptions#shouldProtectHiddenSideEffects
    pub fn should_protect_hidden_side_effects(&self) -> bool {
        self.protect_hidden_side_effects
            && (!self.checks_only || self.get_typed_ast_output_file().is_some())
    }
    // port: CompilerOptions#setAssumeGettersArePure
    pub fn set_assume_getters_are_pure(&mut self, x: bool) {
        self.assume_getters_are_pure = x;
    }
    // port: CompilerOptions#getAssumeGettersArePure
    pub fn get_assume_getters_are_pure(&self) -> bool {
        self.assume_getters_are_pure
    }
    // port: CompilerOptions#setAssumePropertiesAreStaticallyAnalyzable
    pub fn set_assume_properties_are_statically_analyzable(&mut self, x: bool) {
        self.assume_properties_are_statically_analyzable = x;
    }
    // port: CompilerOptions#getAssumePropertiesAreStaticallyAnalyzable
    pub fn get_assume_properties_are_statically_analyzable(&self) -> bool {
        self.assume_properties_are_statically_analyzable
    }
    // port: CompilerOptions#setAssumeStaticInheritanceIsNotUsed
    pub fn set_assume_static_inheritance_is_not_used(&mut self, x: bool) {
        self.assume_static_inheritance_is_not_used = x;
    }
    // port: CompilerOptions#getAssumeStaticInheritanceIsNotUsed
    pub fn get_assume_static_inheritance_is_not_used(&self) -> bool {
        self.assume_static_inheritance_is_not_used
    }
    // port: CompilerOptions#getConformanceReportingMode
    pub fn get_conformance_reporting_mode(&self) -> ConformanceReportingMode {
        self.conformance_reporting_mode
    }
    // port: CompilerOptions#setConformanceReportingMode
    pub fn set_conformance_reporting_mode(&mut self, mode: ConformanceReportingMode) {
        self.conformance_reporting_mode = mode;
    }
    // port: CompilerOptions#setConformanceRemoveRegexFromPath
    pub fn set_conformance_remove_regex_from_path(&mut self, pattern: Option<Pattern>) {
        self.conformance_remove_regex_from_path = pattern;
    }
    // port: CompilerOptions#getConformanceRemoveRegexFromPath
    pub fn get_conformance_remove_regex_from_path(&self) -> &Option<Pattern> {
        &self.conformance_remove_regex_from_path
    }
    // port: CompilerOptions#setWrapGoogModulesForWhitespaceOnly
    pub fn set_wrap_goog_modules_for_whitespace_only(&mut self, enable: bool) {
        self.wrap_goog_modules_for_whitespace_only = enable;
    }
    // port: CompilerOptions#shouldWrapGoogModulesForWhitespaceOnly
    pub fn should_wrap_goog_modules_for_whitespace_only(&self) -> bool {
        self.wrap_goog_modules_for_whitespace_only
    }
    // port: CompilerOptions#setBadRewriteModulesBeforeTypecheckingThatWeWantToGetRidOf
    pub fn set_bad_rewrite_modules_before_typechecking_that_we_want_to_get_rid_of(
        &mut self,
        b: bool,
    ) {
        self.rewrite_modules_before_typechecking = b;
    }
    // port: CompilerOptions#shouldRewriteModulesBeforeTypechecking
    pub fn should_rewrite_modules_before_typechecking(&self) -> bool {
        self.enable_module_rewriting
            && (self.rewrite_modules_before_typechecking || self.process_common_js_modules)
    }
    // port: CompilerOptions#setEnableModuleRewriting
    pub fn set_enable_module_rewriting(&mut self, enable: bool) {
        self.enable_module_rewriting = enable;
    }
    // port: CompilerOptions#shouldRewriteModulesAfterTypechecking
    pub fn should_rewrite_modules_after_typechecking(&self) -> bool {
        self.enable_module_rewriting && !self.rewrite_modules_before_typechecking
    }
    // port: CompilerOptions#shouldRewriteModules
    pub fn should_rewrite_modules(&self) -> bool {
        self.enable_module_rewriting
    }
    // port: CompilerOptions#setPrintConfig
    pub fn set_print_config(&mut self, print_config: bool) {
        self.print_config = print_config;
    }
    // port: CompilerOptions#shouldPrintConfig
    pub fn should_print_config(&self) -> bool {
        self.print_config
    }
    // port: CompilerOptions#setAllowDynamicImport
    pub fn set_allow_dynamic_import(&mut self, value: bool) {
        self.allow_dynamic_import = value;
    }
    // port: CompilerOptions#shouldAllowDynamicImport
    pub fn should_allow_dynamic_import(&self) -> bool {
        self.allow_dynamic_import
    }
    // port: CompilerOptions#getDynamicImportAlias
    pub fn get_dynamic_import_alias(&self) -> Option<&str> {
        self.dynamic_import_alias.as_deref()
    }
    // port: CompilerOptions#setDynamicImportAlias
    pub fn set_dynamic_import_alias(&mut self, value: String) {
        self.dynamic_import_alias = Some(value);
    }
    // port: CompilerOptions#shouldAliasDynamicImport
    pub fn should_alias_dynamic_import(&self) -> bool {
        self.dynamic_import_alias.is_some()
    }
    // port: CompilerOptions#isRemoveUnusedClassProperties
    pub fn is_remove_unused_class_properties(&self) -> bool {
        self.remove_unused_class_properties
    }
    // port: CompilerOptions#setRemoveUnusedClassProperties
    pub fn set_remove_unused_class_properties(&mut self, remove_unused_class_properties: bool) {
        self.remove_unused_class_properties = remove_unused_class_properties;
    }
    // port: CompilerOptions#getDefineReplacements
    pub fn get_define_replacements(&self, ast: &mut Ast) -> IndexMap<String, NodeId> {
        let mut map = IndexMap::new();
        for (name, value) in &self.define_replacements {
            let node = match value {
                DefineValue::Boolean(b) => NodeUtil::boolean_node(ast, *b),
                DefineValue::Integer(n) => NodeUtil::number_node(ast, *n as f64, None),
                DefineValue::Double(n) => NodeUtil::number_node(ast, *n, None),
                DefineValue::String(s) => IR::string(ast, s.clone()),
            };
            map.insert(name.clone(), node);
        }
        map
    }
    // port: CompilerOptions#setDefineToBooleanLiteral
    pub fn set_define_to_boolean_literal(&mut self, define_name: &str, value: bool) {
        self.define_replacements
            .insert(define_name.into(), DefineValue::Boolean(value));
    }
    // port: CompilerOptions#setDefineToStringLiteral
    pub fn set_define_to_string_literal(&mut self, define_name: &str, value: impl Into<JsString>) {
        self.define_replacements
            .insert(define_name.into(), DefineValue::String(value.into()));
    }
    // port: CompilerOptions#setDefineToNumberLiteral
    pub fn set_define_to_number_literal(&mut self, define_name: &str, value: i32) {
        self.define_replacements
            .insert(define_name.into(), DefineValue::Integer(value));
    }
    // port: CompilerOptions#setDefineToDoubleLiteral
    pub fn set_define_to_double_literal(&mut self, define_name: &str, value: f64) {
        self.define_replacements
            .insert(define_name.into(), DefineValue::Double(value));
    }
    // port: CompilerOptions#skipAllCompilerPasses
    pub fn skip_all_compiler_passes(&mut self) {
        self.set_skip_non_transpilation_passes(true);
    }
    // port: CompilerOptions#enables
    pub fn enables(&self, group: &DiagnosticGroup) -> bool {
        self.warnings_guard.must_run_checks(group) == Tri::TRUE
    }
    // port: CompilerOptions#disables
    pub fn disables(&self, group: &DiagnosticGroup) -> bool {
        self.warnings_guard.must_run_checks(group) == Tri::FALSE
    }
    // port: CompilerOptions#setUnknownDefinesToIgnore
    pub fn set_unknown_defines_to_ignore(&mut self, unknown_defines_to_ignore: Vec<String>) {
        self.unknown_defines_to_ignore = unknown_defines_to_ignore;
    }
    // port: CompilerOptions#addUnknownDefinesToIgnore
    pub fn add_unknown_defines_to_ignore(&mut self, unknown_defines_to_ignore: Vec<String>) {
        self.unknown_defines_to_ignore
            .extend(unknown_defines_to_ignore);
    }
    // port: CompilerOptions#getUnknownDefinesToIgnore
    pub fn get_unknown_defines_to_ignore(&self) -> &Vec<String> {
        &self.unknown_defines_to_ignore
    }
    // port: CompilerOptions#setWarningLevel
    pub fn set_warning_level(&mut self, type_: Arc<DiagnosticGroup>, level: CheckLevel) {
        self.add_warnings_guard(Arc::new(DiagnosticGroupWarningsGuard::new(type_, level)));
    }
    // port: CompilerOptions#getWarningsGuard
    pub fn get_warnings_guard(&self) -> &Arc<ComposeWarningsGuard> {
        &self.warnings_guard
    }
    // port: CompilerOptions#resetWarningsGuard
    pub fn reset_warnings_guard(&mut self) {
        self.warnings_guard = Arc::new(ComposeWarningsGuard::new(Vec::new()));
    }
    // port: CompilerOptions#addWarningsGuard
    pub fn add_warnings_guard(&mut self, guard: Arc<dyn WarningsGuard + Send + Sync>) {
        self.warnings_guard.add_guard(guard);
    }
    // port: CompilerOptions#setRenamingPolicy
    pub fn set_renaming_policy(
        &mut self,
        new_variable_policy: VariableRenamingPolicy,
        new_property_policy: PropertyRenamingPolicy,
    ) {
        self.variable_renaming = new_variable_policy;
        self.property_renaming = new_property_policy;
    }
    // port: CompilerOptions#setReplaceIdGenerators
    pub fn set_replace_id_generators(&mut self, replace_id_generators: bool) {
        self.replace_id_generators = replace_id_generators;
    }
    // port: CompilerOptions#shouldReplaceIdGenerators
    pub fn should_replace_id_generators(&self) -> bool {
        self.replace_id_generators
    }
    // port: CompilerOptions#setIdGenerators(Set<String>)
    pub fn set_id_generator_names(&mut self, id_generators: IndexSet<String>) {
        let mut builder: IndexMap<String, Arc<dyn RenamingMap + Send + Sync>> = IndexMap::new();
        for name in id_generators {
            builder.insert(name, Arc::new(RenamingToken::INCONSISTENT));
        }
        self.id_generators = builder;
    }
    // port: CompilerOptions#setIdGenerators(Map<String, RenamingMap>)
    pub fn set_id_generators(
        &mut self,
        id_generators: IndexMap<String, Arc<dyn RenamingMap + Send + Sync>>,
    ) {
        self.id_generators = id_generators;
    }
    // port: CompilerOptions#getIdGenerators
    pub fn get_id_generators(&self) -> &IndexMap<String, Arc<dyn RenamingMap + Send + Sync>> {
        &self.id_generators
    }
    // port: CompilerOptions#setIdGeneratorsMap
    pub fn set_id_generators_map(&mut self, previous_mappings: String) {
        self.id_generators_map_serialized = Some(previous_mappings);
    }
    // port: CompilerOptions#getIdGeneratorsMapSerialized
    pub fn get_id_generators_map_serialized(&self) -> Option<&str> {
        self.id_generators_map_serialized.as_deref()
    }
    // port: CompilerOptions#setXidHashFunction
    pub fn set_xid_hash_function(
        &mut self,
        xid_hash_function: Arc<dyn HashFunction + Send + Sync>,
    ) {
        self.xid_hash_function = Some(xid_hash_function);
    }
    // port: CompilerOptions#getXidHashFunction
    pub fn get_xid_hash_function(&self) -> &Option<Arc<dyn HashFunction + Send + Sync>> {
        &self.xid_hash_function
    }
    // port: CompilerOptions#setChunkIdHashFunction
    pub fn set_chunk_id_hash_function(
        &mut self,
        chunk_id_hash_function: Arc<dyn HashFunction + Send + Sync>,
    ) {
        self.chunk_id_hash_function = Some(chunk_id_hash_function);
    }
    // port: CompilerOptions#getChunkIdHashFunction
    pub fn get_chunk_id_hash_function(&self) -> &Option<Arc<dyn HashFunction + Send + Sync>> {
        &self.chunk_id_hash_function
    }
    // port: CompilerOptions#setInlineFunctions
    pub fn set_inline_functions(&mut self, reach: Reach) {
        self.inline_functions_level = reach;
    }
    // port: CompilerOptions#getInlineFunctionsLevel
    pub fn get_inline_functions_level(&self) -> Reach {
        self.inline_functions_level
    }
    // port: CompilerOptions#setMaxFunctionSizeAfterInlining
    pub fn set_max_function_size_after_inlining(&mut self, fun_ast_size: i32) {
        check_argument!(fun_ast_size > 0);
        self.max_function_size_after_inlining = fun_ast_size;
    }
    // port: CompilerOptions#getMaxFunctionSizeAfterInlining
    pub fn get_max_function_size_after_inlining(&self) -> i32 {
        self.max_function_size_after_inlining
    }
    // port: CompilerOptions#setInlineVariables(boolean)
    pub fn set_inline_variables_enabled(&mut self, inline_variables: bool) {
        self.inline_variables = inline_variables;
    }
    // port: CompilerOptions#shouldInlineVariables
    pub fn should_inline_variables(&self) -> bool {
        self.inline_variables
    }
    // port: CompilerOptions#setInlineVariables(Reach)
    pub fn set_inline_variables(&mut self, reach: Reach) {
        match reach {
            Reach::ALL => {
                self.inline_variables = true;
                self.inline_local_variables = true;
            }
            Reach::LOCAL_ONLY => {
                self.inline_variables = false;
                self.inline_local_variables = true;
            }
            Reach::NONE => {
                self.inline_variables = false;
                self.inline_local_variables = false;
            }
        }
    }
    // port: CompilerOptions#setInlineProperties
    pub fn set_inline_properties(&mut self, enable: bool) {
        self.inline_properties = enable;
    }
    // port: CompilerOptions#shouldInlineProperties
    pub fn should_inline_properties(&self) -> bool {
        self.inline_properties
    }
    // port: CompilerOptions#setRemoveUnusedVariables
    pub fn set_remove_unused_variables(&mut self, reach: Reach) {
        match reach {
            Reach::ALL => {
                self.remove_unused_vars = true;
                self.remove_unused_local_vars = true;
            }
            Reach::LOCAL_ONLY => {
                self.remove_unused_vars = false;
                self.remove_unused_local_vars = true;
            }
            Reach::NONE => {
                self.remove_unused_vars = false;
                self.remove_unused_local_vars = false;
            }
        }
    }
    // port: CompilerOptions#shouldRemoveUnusedVariables
    pub fn should_remove_unused_variables(&self) -> bool {
        self.remove_unused_vars
    }
    // port: CompilerOptions#shouldRemoveUnusedLocalVariables
    pub fn should_remove_unused_local_variables(&self) -> bool {
        self.remove_unused_local_vars
    }
    // port: CompilerOptions#setReplaceStringsConfiguration
    pub fn set_replace_strings_configuration(
        &mut self,
        placeholder_token: String,
        function_descriptors: Vec<String>,
    ) {
        self.replace_strings_placeholder_token = placeholder_token;
        self.replace_strings_function_descriptions = function_descriptors;
    }
    // port: CompilerOptions#setRemoveAbstractMethods
    pub fn set_remove_abstract_methods(&mut self, remove: bool) {
        self.remove_abstract_methods = remove;
    }
    // port: CompilerOptions#shouldRemoveAbstractMethods
    pub fn should_remove_abstract_methods(&self) -> bool {
        self.remove_abstract_methods
    }
    // port: CompilerOptions#setRemoveClosureAsserts
    pub fn set_remove_closure_asserts(&mut self, remove: bool) {
        self.remove_closure_asserts = remove;
    }
    // port: CompilerOptions#shouldRemoveClosureAsserts
    pub fn should_remove_closure_asserts(&self) -> bool {
        self.remove_closure_asserts
    }
    // port: CompilerOptions#setRemoveJ2clAsserts
    pub fn set_remove_j2cl_asserts(&mut self, remove: bool) {
        self.remove_j2cl_asserts = remove;
    }
    // port: CompilerOptions#shouldRemoveJ2clAsserts
    pub fn should_remove_j2cl_asserts(&self) -> bool {
        self.remove_j2cl_asserts
    }
    // port: CompilerOptions#setColorizeErrorOutput
    pub fn set_colorize_error_output(&mut self, colorize_error_output: bool) {
        self.colorize_error_output = colorize_error_output;
    }
    // port: CompilerOptions#shouldColorizeErrorOutput
    pub fn should_colorize_error_output(&self) -> bool {
        self.colorize_error_output
    }
    // port: CompilerOptions#setChecksOnly
    pub fn set_checks_only(&mut self, checks_only: bool) {
        self.checks_only = checks_only;
    }
    // port: CompilerOptions#isChecksOnly
    pub fn is_checks_only(&self) -> bool {
        self.checks_only
    }
    // port: CompilerOptions#setOutputJs
    pub fn set_output_js(&mut self, output_js: OutputJs) {
        self.output_js = output_js;
    }
    // port: CompilerOptions#getOutputJs
    pub fn get_output_js(&self) -> OutputJs {
        self.output_js
    }
    // port: CompilerOptions#setGenerateExports
    pub fn set_generate_exports(&mut self, generate_exports: bool) {
        self.generate_exports = generate_exports;
    }
    // port: CompilerOptions#shouldGenerateExports
    pub fn should_generate_exports(&self) -> bool {
        self.generate_exports
    }
    // port: CompilerOptions#setExportLocalPropertyDefinitions
    pub fn set_export_local_property_definitions(&mut self, export: bool) {
        self.export_local_property_definitions = export;
    }
    // port: CompilerOptions#shouldExportLocalPropertyDefinitions
    pub fn should_export_local_property_definitions(&self) -> bool {
        self.export_local_property_definitions
    }
    // port: CompilerOptions#setAngularPass
    pub fn set_angular_pass(&mut self, angular_pass: bool) {
        self.angular_pass = angular_pass;
    }
    // port: CompilerOptions#getAngularPass
    pub fn get_angular_pass(&self) -> bool {
        self.angular_pass
    }
    // port: CompilerOptions#setPolymerVersion
    pub fn set_polymer_version(&mut self, polymer_version: Option<i32>) {
        check_argument!(
            matches!(polymer_version, None | Some(1 | 2)),
            "Invalid Polymer version: (%s)",
            polymer_version.map_or("null".to_string(), |v| v.to_string())
        );
        self.polymer_pass = polymer_version.is_some();
    }
    // port: CompilerOptions#isPolymerPassEnabled
    pub fn is_polymer_pass_enabled(&self) -> bool {
        self.polymer_pass
    }
    // port: CompilerOptions#setChromePass
    pub fn set_chrome_pass(&mut self, chrome_pass: bool) {
        self.chrome_pass = chrome_pass;
    }
    // port: CompilerOptions#isChromePassEnabled
    pub fn is_chrome_pass_enabled(&self) -> bool {
        self.chrome_pass
    }
    // port: CompilerOptions#setJ2clPass
    pub fn set_j2cl_pass(&mut self, j2cl_pass_mode: J2clPassMode) {
        self.j2cl_pass_mode = j2cl_pass_mode;
    }
    // port: CompilerOptions#getJ2clPass
    pub fn get_j2cl_pass(&self) -> J2clPassMode {
        self.j2cl_pass_mode
    }
    // port: CompilerOptions#setJ2clMinifierEnabled
    pub fn set_j2cl_minifier_enabled(&mut self, enabled: bool) {
        self.j2cl_minifier_enabled = enabled;
    }
    // port: CompilerOptions#isJ2clMinifierEnabled
    pub fn is_j2cl_minifier_enabled(&self) -> bool {
        self.j2cl_minifier_enabled
    }
    // port: CompilerOptions#setJ2clMinifierPruningManifest
    pub fn set_j2cl_minifier_pruning_manifest(&mut self, j2cl_minifier_pruning_manifest: String) {
        self.j2cl_minifier_pruning_manifest = Some(j2cl_minifier_pruning_manifest);
    }
    // port: CompilerOptions#getJ2clMinifierPruningManifest
    pub fn get_j2cl_minifier_pruning_manifest(&self) -> Option<&str> {
        self.j2cl_minifier_pruning_manifest.as_deref()
    }
    // port: CompilerOptions#setCodingConvention
    pub fn set_coding_convention(
        &mut self,
        coding_convention: Arc<dyn CodingConvention + Send + Sync>,
    ) {
        self.coding_convention = Some(coding_convention);
    }
    // port: CompilerOptions#getCodingConvention
    pub fn get_coding_convention(&self) -> &Option<Arc<dyn CodingConvention + Send + Sync>> {
        &self.coding_convention
    }
    // port: CompilerOptions#setDependencyOptions
    pub fn set_dependency_options(&mut self, dependency_options: DependencyOptions) {
        self.dependency_options = dependency_options;
    }
    // port: CompilerOptions#getDependencyOptions
    pub fn get_dependency_options(&self) -> &DependencyOptions {
        &self.dependency_options
    }
    // port: CompilerOptions#setSummaryDetailLevel
    pub fn set_summary_detail_level(&mut self, summary_detail_level: i32) {
        self.summary_detail_level = summary_detail_level;
    }
    // port: CompilerOptions#getSummaryDetailLevel
    pub fn get_summary_detail_level(&self) -> i32 {
        self.summary_detail_level
    }
    // port: CompilerOptions#setExtraAnnotationNames
    pub fn set_extra_annotation_names(&mut self, extra_annotation_names: Vec<String>) {
        self.extra_annotation_names = Some(extra_annotation_names.into_iter().collect());
    }
    // port: CompilerOptions#getExtraAnnotationNames
    pub fn get_extra_annotation_names(&self) -> &Option<IndexSet<String>> {
        &self.extra_annotation_names
    }
    // port: CompilerOptions#setOutputCharset
    pub fn set_output_charset(&mut self, charset: Charset) {
        self.output_charset = Some(charset);
    }
    // port: CompilerOptions#getOutputCharset
    pub fn get_output_charset(&self) -> Option<Charset> {
        self.output_charset
    }
    // port: CompilerOptions#setTweakProcessing
    pub fn set_tweak_processing(&mut self, tweak_processing: TweakProcessing) {
        self.tweak_processing = tweak_processing;
    }
    // port: CompilerOptions#getTweakProcessing
    pub fn get_tweak_processing(&self) -> TweakProcessing {
        self.tweak_processing
    }
    // port: CompilerOptions#setLanguage
    pub fn set_language(&mut self, language: LanguageMode) {
        check_state!(language != LanguageMode::NO_TRANSPILE);
        self.set_language_in(language);
        self.set_language_out(language);
    }
    // port: CompilerOptions#setLanguageIn
    pub fn set_language_in(&mut self, language_in: LanguageMode) {
        check_state!(language_in != LanguageMode::NO_TRANSPILE);
        self.language_in = if language_in == LanguageMode::STABLE {
            LanguageMode::STABLE_IN
        } else {
            language_in
        };
    }
    // port: CompilerOptions#getLanguageIn
    pub fn get_language_in(&self) -> LanguageMode {
        self.language_in
    }
    // port: CompilerOptions#setLanguageOut
    pub fn set_language_out(&mut self, mut language_out: LanguageMode) {
        if language_out == LanguageMode::NO_TRANSPILE {
            self.language_out_is_default_strict = None;
            self.output_feature_set = None;
        } else {
            language_out = if language_out == LanguageMode::STABLE {
                LanguageMode::STABLE_OUT
            } else {
                language_out
            };
            self.language_out_is_default_strict = Some(language_out.is_default_strict());
            self.set_output_feature_set(language_out.to_feature_set());
        }
    }
    // port: CompilerOptions#setOutputFeatureSet
    pub fn set_output_feature_set(&mut self, feature_set: FeatureSet) {
        self.output_feature_set = Some(feature_set);
    }
    // port: CompilerOptions#legacySetOutputFeatureSet
    pub fn legacy_set_output_feature_set(&mut self, feature_set: FeatureSet) {
        self.set_output_feature_set(feature_set);
    }
    // port: CompilerOptions#getOutputFeatureSet
    pub fn get_output_feature_set(&self) -> FeatureSet {
        self.output_feature_set
            .unwrap_or_else(|| self.language_in.to_feature_set())
    }
    // port: CompilerOptions#setTranspilePublicClassFields
    pub fn set_transpile_public_class_fields(&mut self, transpile: bool) {
        if transpile {
            self.set_output_feature_set(
                self.get_output_feature_set()
                    .without(Feature::PUBLIC_CLASS_FIELDS),
            );
        } else {
            self.set_output_feature_set(
                self.get_output_feature_set()
                    .with(Feature::PUBLIC_CLASS_FIELDS),
            );
        }
    }
    // port: CompilerOptions#getTranspilePublicClassFields
    pub fn get_transpile_public_class_fields(&self) -> bool {
        !self
            .get_output_feature_set()
            .has(Feature::PUBLIC_CLASS_FIELDS)
    }
    // port: CompilerOptions#setExperimentalOutputFeatureSet
    pub fn set_experimental_output_feature_set(
        &mut self,
        experimental_output_feature_set: Option<ExperimentalOutputFeatureSet>,
    ) {
        self.experimental_output_feature_set = experimental_output_feature_set;
    }
    // port: CompilerOptions#getExperimentalOutputFeatureSet
    pub fn get_experimental_output_feature_set(&self) -> Option<ExperimentalOutputFeatureSet> {
        self.experimental_output_feature_set
    }
    // port: CompilerOptions#needsTranspilationFrom
    pub fn needs_transpilation_from(&self, language_level: FeatureSet) -> bool {
        self.get_language_in()
            .to_feature_set()
            .contains(language_level)
            && !self.get_output_feature_set().contains(language_level)
    }
    // port: CompilerOptions#needsTranspilationOf
    pub fn needs_transpilation_of(&self, feature: Feature) -> bool {
        self.get_language_in().to_feature_set().has(feature)
            && !self.get_output_feature_set().has(feature)
    }
    // port: CompilerOptions#setEnvironment
    pub fn set_environment(&mut self, environment: Environment) {
        self.environment = environment;
    }
    // port: CompilerOptions#getEnvironment
    pub fn get_environment(&self) -> Environment {
        self.environment
    }
    // port: CompilerOptions#setErrorHandler
    pub fn set_error_handler(&mut self, handler: Arc<std::sync::Mutex<dyn ErrorHandler + Send>>) {
        self.error_handler = Some(handler);
    }
    // port: CompilerOptions#getErrorHandler
    pub fn get_error_handler(&self) -> &Option<Arc<std::sync::Mutex<dyn ErrorHandler + Send>>> {
        &self.error_handler
    }
    // port: CompilerOptions#setInferTypes
    pub fn set_infer_types(&mut self, enable: bool) {
        self.infer_types = enable;
    }
    // port: CompilerOptions#getInferTypes
    pub fn get_infer_types(&self) -> bool {
        self.infer_types
    }
    // port: CompilerOptions#setNewTypeInference
    pub fn set_new_type_inference(&mut self, _enable: bool) {}
    // port: CompilerOptions#isTypecheckingEnabled
    pub fn is_typechecking_enabled(&self) -> bool {
        self.check_types
    }
    // port: CompilerOptions#assumeStrictThis
    pub fn assume_strict_this(&self) -> bool {
        self.assume_strict_this
    }
    // port: CompilerOptions#setAssumeStrictThis
    pub fn set_assume_strict_this(&mut self, enable: bool) {
        self.assume_strict_this = enable;
    }
    // port: CompilerOptions#assumeClosuresOnlyCaptureReferences
    pub fn assume_closures_only_capture_references(&self) -> bool {
        self.assume_closures_only_capture_references
    }
    // port: CompilerOptions#setAssumeClosuresOnlyCaptureReferences
    pub fn set_assume_closures_only_capture_references(&mut self, enable: bool) {
        self.assume_closures_only_capture_references = enable;
    }
    // port: CompilerOptions#setPropertiesThatMustDisambiguate
    pub fn set_properties_that_must_disambiguate(&mut self, names: IndexSet<String>) {
        self.properties_that_must_disambiguate = names;
    }
    // port: CompilerOptions#getPropertiesThatMustDisambiguate
    pub fn get_properties_that_must_disambiguate(&self) -> &IndexSet<String> {
        &self.properties_that_must_disambiguate
    }
    // port: CompilerOptions#setValidateRequiredInlinings
    pub fn set_validate_required_inlinings(&mut self, validate_require_inlining_annotation: bool) {
        self.validate_require_inlining_annotation =
            Tri::for_boolean(validate_require_inlining_annotation);
    }
    // port: CompilerOptions#getShouldValidateRequiredInlinings
    pub fn get_should_validate_required_inlinings(&self) -> Tri {
        self.validate_require_inlining_annotation
    }
    // port: CompilerOptions#setPreserveDetailedSourceInfo
    pub fn set_preserve_detailed_source_info(&mut self, preserve_detailed_source_info: bool) {
        self.preserve_detailed_source_info = preserve_detailed_source_info;
    }
    // port: CompilerOptions#preservesDetailedSourceInfo
    pub fn preserves_detailed_source_info(&self) -> bool {
        self.preserve_detailed_source_info
    }
    // port: CompilerOptions#setPreserveNonJSDocComments
    pub fn set_preserve_non_jsdoc_comments(&mut self, preserve_non_jsdoc_comments: bool) {
        self.preserve_non_jsdoc_comments = preserve_non_jsdoc_comments;
    }
    // port: CompilerOptions#getPreserveNonJSDocComments
    pub fn get_preserve_non_jsdoc_comments(&self) -> bool {
        self.preserve_non_jsdoc_comments
    }
    // port: CompilerOptions#setContinueAfterErrors
    pub fn set_continue_after_errors(&mut self, continue_after_errors: bool) {
        self.continue_after_errors = continue_after_errors;
    }
    // port: CompilerOptions#canContinueAfterErrors
    pub fn can_continue_after_errors(&self) -> bool {
        self.continue_after_errors
    }
    // port: CompilerOptions#setParseJsDocDocumentation
    pub fn set_parse_js_doc_documentation(&mut self, parse_js_doc_documentation: JsDocParsing) {
        self.parse_js_doc_documentation = parse_js_doc_documentation;
    }
    // port: CompilerOptions#isParseJsDocDocumentation
    pub fn is_parse_js_doc_documentation(&self) -> JsDocParsing {
        self.parse_js_doc_documentation
    }
    // port: CompilerOptions#setSkipNonTranspilationPasses
    pub fn set_skip_non_transpilation_passes(&mut self, skip_non_transpilation_passes: bool) {
        self.skip_non_transpilation_passes = skip_non_transpilation_passes;
    }
    // port: CompilerOptions#getSkipNonTranspilationPasses
    pub fn get_skip_non_transpilation_passes(&self) -> bool {
        self.skip_non_transpilation_passes
    }
    // port: CompilerOptions#setDevMode
    pub fn set_dev_mode(&mut self, dev_mode: DevMode) {
        self.dev_mode = dev_mode;
    }
    // port: CompilerOptions#getDevMode
    pub fn get_dev_mode(&self) -> DevMode {
        self.dev_mode
    }
    // port: CompilerOptions#setCheckDeterminism
    pub fn set_check_determinism(&mut self, check_determinism: bool) {
        self.check_determinism = check_determinism;
    }
    // port: CompilerOptions#getCheckDeterminism
    pub fn get_check_determinism(&self) -> bool {
        self.check_determinism
    }
    // port: CompilerOptions#setMessageBundle
    pub fn set_message_bundle(&mut self, message_bundle: Arc<dyn MessageBundle + Send + Sync>) {
        self.message_bundle = Some(message_bundle);
    }
    // port: CompilerOptions#getMessageBundle
    pub fn get_message_bundle(&self) -> &Option<Arc<dyn MessageBundle + Send + Sync>> {
        &self.message_bundle
    }
    // port: CompilerOptions#setCheckSymbols
    pub fn set_check_symbols(&mut self, check_symbols: bool) {
        self.check_symbols = check_symbols;
    }
    // port: CompilerOptions#getCheckSymbols
    pub fn get_check_symbols(&self) -> bool {
        self.check_symbols
    }
    // port: CompilerOptions#setCheckSuspiciousCode
    pub fn set_check_suspicious_code(&mut self, check_suspicious_code: bool) {
        self.check_suspicious_code = check_suspicious_code;
    }
    // port: CompilerOptions#getCheckSuspiciousCode
    pub fn get_check_suspicious_code(&self) -> bool {
        self.check_suspicious_code
    }
    // port: CompilerOptions#setCheckTypes
    pub fn set_check_types(&mut self, check_types: bool) {
        self.check_types = check_types;
    }
    // port: CompilerOptions#getCheckTypes
    pub fn get_check_types(&self) -> bool {
        self.check_types
    }
    // port: CompilerOptions#setFoldConstants
    pub fn set_fold_constants(&mut self, fold_constants: bool) {
        self.fold_constants = fold_constants;
    }
    // port: CompilerOptions#shouldFoldConstants
    pub fn should_fold_constants(&self) -> bool {
        self.fold_constants
    }
    // port: CompilerOptions#setDeadAssignmentElimination
    pub fn set_dead_assignment_elimination(&mut self, dead_assignment_elimination: bool) {
        self.dead_assignment_elimination = dead_assignment_elimination;
    }
    // port: CompilerOptions#shouldRunDeadAssignmentElimination
    pub fn should_run_dead_assignment_elimination(&self) -> bool {
        self.dead_assignment_elimination
    }
    // port: CompilerOptions#setDeadPropertyAssignmentElimination
    pub fn set_dead_property_assignment_elimination(
        &mut self,
        dead_property_assignment_elimination: bool,
    ) {
        self.dead_property_assignment_elimination =
            Tri::for_boolean(dead_property_assignment_elimination);
    }
    // port: CompilerOptions#shouldRunDeadPropertyAssignmentElimination
    pub fn should_run_dead_property_assignment_elimination(&self) -> bool {
        self.dead_property_assignment_elimination
            .to_boolean(self.dead_assignment_elimination && !self.polymer_pass)
    }
    // port: CompilerOptions#setInlineConstantVars
    pub fn set_inline_constant_vars(&mut self, inline_constant_vars: bool) {
        self.inline_constant_vars = inline_constant_vars;
    }
    // port: CompilerOptions#shouldInlineConstantVars
    pub fn should_inline_constant_vars(&self) -> bool {
        self.inline_constant_vars
    }
    // port: CompilerOptions#setCrossChunkCodeMotion
    pub fn set_cross_chunk_code_motion(&mut self, cross_chunk_code_motion: bool) {
        self.cross_chunk_code_motion = cross_chunk_code_motion;
    }
    // port: CompilerOptions#setCrossChunkCodeMotionNoStubMethods
    pub fn set_cross_chunk_code_motion_no_stub_methods(
        &mut self,
        cross_chunk_code_motion_no_stub_methods: bool,
    ) {
        self.cross_chunk_code_motion_no_stub_methods = cross_chunk_code_motion_no_stub_methods;
    }
    // port: CompilerOptions#getCrossChunkCodeMotionNoStubMethods
    pub fn get_cross_chunk_code_motion_no_stub_methods(&self) -> bool {
        self.cross_chunk_code_motion_no_stub_methods
    }
    // port: CompilerOptions#setParentChunkCanSeeSymbolsDeclaredInChildren
    pub fn set_parent_chunk_can_see_symbols_declared_in_children(
        &mut self,
        parent_chunk_can_see_symbols_declared_in_children: bool,
    ) {
        self.parent_chunk_can_see_symbols_declared_in_children =
            parent_chunk_can_see_symbols_declared_in_children;
    }
    // port: CompilerOptions#getParentChunkCanSeeSymbolsDeclaredInChildren
    pub fn get_parent_chunk_can_see_symbols_declared_in_children(&self) -> bool {
        self.parent_chunk_can_see_symbols_declared_in_children
    }
    // port: CompilerOptions#setCrossChunkMethodMotion
    pub fn set_cross_chunk_method_motion(&mut self, cross_chunk_method_motion: bool) {
        self.cross_chunk_method_motion = cross_chunk_method_motion;
    }
    // port: CompilerOptions#getCrossChunkMethodMotion
    pub fn get_cross_chunk_method_motion(&self) -> bool {
        self.cross_chunk_method_motion
    }
    // port: CompilerOptions#setCoalesceVariableNames
    pub fn set_coalesce_variable_names(&mut self, coalesce_variable_names: bool) {
        self.coalesce_variable_names = coalesce_variable_names;
    }
    // port: CompilerOptions#shouldCoalesceVariableNames
    pub fn should_coalesce_variable_names(&self) -> bool {
        self.coalesce_variable_names
    }
    // port: CompilerOptions#setOptimizeLetAndConst
    pub fn set_optimize_let_and_const(&mut self, optimize_let_and_const: bool) {
        self.optimize_let_and_const = optimize_let_and_const;
    }
    // port: CompilerOptions#shouldOptimizeLetAndConst
    pub fn should_optimize_let_and_const(&self) -> bool {
        self.optimize_let_and_const
    }
    // port: CompilerOptions#setAssumeGlobalScopeIsIsolated
    pub fn set_assume_global_scope_is_isolated(&mut self, assume_global_scope_is_isolated: bool) {
        self.assume_global_scope_is_isolated = assume_global_scope_is_isolated;
    }
    // port: CompilerOptions#shouldTreatGlobalScopeAsIsolated
    pub fn should_treat_global_scope_as_isolated(&self) -> bool {
        self.assume_global_scope_is_isolated || self.rename_prefix_namespace.is_some()
    }
    // port: CompilerOptions#setInlineLocalVariables
    pub fn set_inline_local_variables(&mut self, inline_local_variables: bool) {
        self.inline_local_variables = inline_local_variables;
    }
    // port: CompilerOptions#shouldInlineLocalVariables
    pub fn should_inline_local_variables(&self) -> bool {
        self.inline_local_variables
    }
    // port: CompilerOptions#setFlowSensitiveInlineVariables
    pub fn set_flow_sensitive_inline_variables(&mut self, enabled: bool) {
        self.flow_sensitive_inline_variables = enabled;
    }
    // port: CompilerOptions#getFlowSensitiveInlineVariables
    pub fn get_flow_sensitive_inline_variables(&self) -> bool {
        self.flow_sensitive_inline_variables
    }
    // port: CompilerOptions#setSmartNameRemoval
    pub fn set_smart_name_removal(&mut self, smart_name_removal: bool) {
        self.smart_name_removal = smart_name_removal;
        if smart_name_removal {
            self.remove_unused_vars = true;
            self.remove_unused_prototype_properties = true;
        }
    }
    // port: CompilerOptions#getSmartNameRemoval
    pub fn get_smart_name_removal(&self) -> bool {
        self.smart_name_removal
    }
    // port: CompilerOptions#setExtractPrototypeMemberDeclarations(boolean)
    pub fn set_extract_prototype_member_declarations(&mut self, enabled: bool) {
        self.extract_prototype_member_declarations = if enabled {
            ExtractPrototypeMemberDeclarationsMode::USE_GLOBAL_TEMP
        } else {
            ExtractPrototypeMemberDeclarationsMode::OFF
        };
    }
    // port: CompilerOptions#setExtractPrototypeMemberDeclarations(ExtractPrototypeMemberDeclarationsMode)
    pub fn set_extract_prototype_member_declarations_mode(
        &mut self,
        mode: ExtractPrototypeMemberDeclarationsMode,
    ) {
        self.extract_prototype_member_declarations = mode;
    }
    // port: CompilerOptions#getExtractPrototypeMemberDeclarationsMode
    pub fn get_extract_prototype_member_declarations_mode(
        &self,
    ) -> ExtractPrototypeMemberDeclarationsMode {
        self.extract_prototype_member_declarations
    }
    // port: CompilerOptions#setRemoveUnusedPrototypeProperties
    pub fn set_remove_unused_prototype_properties(&mut self, enabled: bool) {
        self.remove_unused_prototype_properties = enabled;
        self.inline_getters = enabled;
    }
    // port: CompilerOptions#shouldRemoveUnusedPrototypeProperties
    pub fn should_remove_unused_prototype_properties(&self) -> bool {
        self.remove_unused_prototype_properties
    }
    // port: CompilerOptions#shouldInlineGetters
    pub fn should_inline_getters(&self) -> bool {
        self.inline_getters
    }
    // port: CompilerOptions#setCollapseVariableDeclarations
    pub fn set_collapse_variable_declarations(&mut self, enabled: bool) {
        self.collapse_variable_declarations = enabled;
    }
    // port: CompilerOptions#shouldCollapseVariableDeclarations
    pub fn should_collapse_variable_declarations(&self) -> bool {
        self.collapse_variable_declarations
    }
    // port: CompilerOptions#setCollapseAnonymousFunctions
    pub fn set_collapse_anonymous_functions(&mut self, enabled: bool) {
        self.collapse_anonymous_functions = enabled;
    }
    // port: CompilerOptions#shouldCollapseAnonymousFunctions
    pub fn should_collapse_anonymous_functions(&self) -> bool {
        self.collapse_anonymous_functions
    }
    // port: CompilerOptions#setAliasStringsMode
    pub fn set_alias_strings_mode(&mut self, alias_strings_mode: AliasStringsMode) {
        self.alias_strings_mode = alias_strings_mode;
    }
    // port: CompilerOptions#getAliasStringsMode
    pub fn get_alias_strings_mode(&self) -> AliasStringsMode {
        self.alias_strings_mode
    }
    // port: CompilerOptions#setOutputJsStringUsage
    pub fn set_output_js_string_usage(&mut self, output_js_string_usage: bool) {
        self.output_js_string_usage = output_js_string_usage;
    }
    // port: CompilerOptions#shouldOutputJsStringUsage
    pub fn should_output_js_string_usage(&self) -> bool {
        self.output_js_string_usage
    }
    // port: CompilerOptions#setConvertToDottedProperties
    pub fn set_convert_to_dotted_properties(&mut self, convert_to_dotted_properties: bool) {
        self.convert_to_dotted_properties = convert_to_dotted_properties;
    }
    // port: CompilerOptions#shouldConvertToDottedProperties
    pub fn should_convert_to_dotted_properties(&self) -> bool {
        self.convert_to_dotted_properties
    }
    // port: CompilerOptions#setUseTypesForLocalOptimization
    pub fn set_use_types_for_local_optimization(&mut self, use_types_for_local_optimization: bool) {
        self.use_types_for_local_optimization = use_types_for_local_optimization;
    }
    // port: CompilerOptions#shouldUseTypesForLocalOptimization
    pub fn should_use_types_for_local_optimization(&self) -> bool {
        self.use_types_for_local_optimization
    }
    // port: CompilerOptions#setUseTypesForOptimization
    pub fn set_use_types_for_optimization(&mut self, use_types_for_optimization: bool) {
        if use_types_for_optimization {
            self.disambiguate_properties = true;
            self.ambiguate_properties = true;
            self.inline_properties = true;
            self.use_types_for_local_optimization = true;
        }
    }
    // port: CompilerOptions#requiresTypesForOptimization
    pub fn requires_types_for_optimization(&self) -> bool {
        self.disambiguate_properties
            || self.ambiguate_properties
            || self.inline_properties
            || self.use_types_for_local_optimization
    }
    // port: CompilerOptions#setRewriteFunctionExpressions
    pub fn set_rewrite_function_expressions(&mut self, rewrite_function_expressions: bool) {
        self.rewrite_function_expressions = rewrite_function_expressions;
    }
    // port: CompilerOptions#shouldRewriteFunctionExpressions
    pub fn should_rewrite_function_expressions(&self) -> bool {
        self.rewrite_function_expressions
    }
    // port: CompilerOptions#setOptimizeCalls
    pub fn set_optimize_calls(&mut self, optimize_calls: bool) {
        self.optimize_calls = optimize_calls;
    }
    // port: CompilerOptions#shouldOptimizeCalls
    pub fn should_optimize_calls(&self) -> bool {
        self.optimize_calls
    }
    // port: CompilerOptions#getOptimizeESClassConstructors
    pub fn get_optimize_es_class_constructors(&self) -> bool {
        self.optimize_es_class_constructors
    }
    // port: CompilerOptions#setOptimizeESClassConstructors
    pub fn set_optimize_es_class_constructors(&mut self, optimize_es_class_constructors: bool) {
        self.optimize_es_class_constructors = optimize_es_class_constructors;
    }
    // port: CompilerOptions#setVariableRenaming
    pub fn set_variable_renaming(&mut self, variable_renaming: VariableRenamingPolicy) {
        self.variable_renaming = variable_renaming;
    }
    // port: CompilerOptions#getVariableRenaming
    pub fn get_variable_renaming(&self) -> VariableRenamingPolicy {
        self.variable_renaming
    }
    // port: CompilerOptions#setPropertyRenaming
    pub fn set_property_renaming(&mut self, property_renaming: PropertyRenamingPolicy) {
        self.property_renaming = property_renaming;
    }
    // port: CompilerOptions#getPropertyRenaming
    pub fn get_property_renaming(&self) -> PropertyRenamingPolicy {
        self.property_renaming
    }
    // port: CompilerOptions#setLabelRenaming
    pub fn set_label_renaming(&mut self, label_renaming: bool) {
        self.label_renaming = label_renaming;
    }
    // port: CompilerOptions#shouldRenameLabels
    pub fn should_rename_labels(&self) -> bool {
        self.label_renaming
    }
    // port: CompilerOptions#setReserveRawExports
    pub fn set_reserve_raw_exports(&mut self, reserve_raw_exports: bool) {
        self.reserve_raw_exports = reserve_raw_exports;
    }
    // port: CompilerOptions#shouldReserveRawExports
    pub fn should_reserve_raw_exports(&self) -> bool {
        self.reserve_raw_exports
    }
    // port: CompilerOptions#setPreferStableNames
    pub fn set_prefer_stable_names(&mut self, prefer_stable_names: bool) {
        self.prefer_stable_names = prefer_stable_names;
    }
    // port: CompilerOptions#shouldPreferStableNames
    pub fn should_prefer_stable_names(&self) -> bool {
        self.prefer_stable_names
    }
    // port: CompilerOptions#setGeneratePseudoNames
    pub fn set_generate_pseudo_names(&mut self, generate_pseudo_names: bool) {
        self.generate_pseudo_names = generate_pseudo_names;
    }
    // port: CompilerOptions#shouldGeneratePseudoNames
    pub fn should_generate_pseudo_names(&self) -> bool {
        self.generate_pseudo_names
    }
    // port: CompilerOptions#setPropertyRenamingOnlyCompilationMode
    pub fn set_property_renaming_only_compilation_mode(
        &mut self,
        property_renaming_only_compilation_mode: bool,
    ) {
        self.property_renaming_only_compilation_mode = property_renaming_only_compilation_mode;
    }
    // port: CompilerOptions#isPropertyRenamingOnlyCompilationMode
    pub fn is_property_renaming_only_compilation_mode(&self) -> bool {
        self.property_renaming_only_compilation_mode
    }
    // port: CompilerOptions#setRenamePrefix
    pub fn set_rename_prefix(&mut self, rename_prefix: String) {
        self.rename_prefix = Some(rename_prefix);
    }
    // port: CompilerOptions#getRenamePrefix
    pub fn get_rename_prefix(&self) -> Option<&str> {
        self.rename_prefix.as_deref()
    }
    // port: CompilerOptions#getRenamePrefixNamespace
    pub fn get_rename_prefix_namespace(&self) -> Option<&str> {
        self.rename_prefix_namespace.as_deref()
    }
    // port: CompilerOptions#setRenamePrefixNamespace
    pub fn set_rename_prefix_namespace(&mut self, rename_prefix_namespace: String) {
        self.rename_prefix_namespace = Some(rename_prefix_namespace);
    }
    // port: CompilerOptions#setCollapsePropertiesLevel
    pub fn set_collapse_properties_level(&mut self, level: PropertyCollapseLevel) {
        self.collapse_properties_level = level;
    }
    // port: CompilerOptions#setCollapseProperties
    pub fn set_collapse_properties(&mut self, fully_collapse: bool) {
        self.collapse_properties_level = if fully_collapse {
            PropertyCollapseLevel::ALL
        } else {
            PropertyCollapseLevel::NONE
        };
    }
    // port: CompilerOptions#getCollapsePropertiesLevel
    pub fn get_collapse_properties_level(&self) -> PropertyCollapseLevel {
        self.collapse_properties_level
    }
    // port: CompilerOptions#setDevirtualizeMethods
    pub fn set_devirtualize_methods(&mut self, devirtualize_methods: bool) {
        self.devirtualize_methods = devirtualize_methods;
    }
    // port: CompilerOptions#shouldDevirtualizeMethods
    pub fn should_devirtualize_methods(&self) -> bool {
        self.devirtualize_methods
    }
    // port: CompilerOptions#setComputeFunctionSideEffects
    pub fn set_compute_function_side_effects(&mut self, compute_function_side_effects: bool) {
        self.compute_function_side_effects = compute_function_side_effects;
    }
    // port: CompilerOptions#shouldComputeFunctionSideEffects
    pub fn should_compute_function_side_effects(&self) -> bool {
        self.compute_function_side_effects
    }
    // port: CompilerOptions#setDisambiguateProperties
    pub fn set_disambiguate_properties(&mut self, disambiguate_properties: bool) {
        self.disambiguate_properties = disambiguate_properties;
    }
    // port: CompilerOptions#shouldDisambiguateProperties
    pub fn should_disambiguate_properties(&self) -> bool {
        self.disambiguate_properties
    }
    // port: CompilerOptions#setAmbiguateProperties
    pub fn set_ambiguate_properties(&mut self, ambiguate_properties: bool) {
        self.ambiguate_properties = ambiguate_properties;
    }
    // port: CompilerOptions#shouldAmbiguateProperties
    pub fn should_ambiguate_properties(&self) -> bool {
        self.ambiguate_properties
    }
    // port: CompilerOptions#setInputVariableMap
    pub fn set_input_variable_map(&mut self, input_variable_map: Arc<VariableMap>) {
        self.input_variable_map = Some(input_variable_map);
    }
    // port: CompilerOptions#getInputVariableMap
    pub fn get_input_variable_map(&self) -> &Option<Arc<VariableMap>> {
        &self.input_variable_map
    }
    // port: CompilerOptions#setInputPropertyMap
    pub fn set_input_property_map(&mut self, input_property_map: Arc<VariableMap>) {
        self.input_property_map = Some(input_property_map);
    }
    // port: CompilerOptions#getInputPropertyMap
    pub fn get_input_property_map(&self) -> &Option<Arc<VariableMap>> {
        &self.input_property_map
    }
    // port: CompilerOptions#setExportTestFunctions
    pub fn set_export_test_functions(&mut self, export_test_functions: bool) {
        self.export_test_functions = export_test_functions;
    }
    // port: CompilerOptions#shouldExportTestFunctions
    pub fn should_export_test_functions(&self) -> bool {
        self.export_test_functions
    }
    // port: CompilerOptions#setSyntheticBlockStartMarker
    pub fn set_synthetic_block_start_marker(&mut self, synthetic_block_start_marker: String) {
        self.synthetic_block_start_marker = Some(synthetic_block_start_marker);
    }
    // port: CompilerOptions#setSyntheticBlockEndMarker
    pub fn set_synthetic_block_end_marker(&mut self, synthetic_block_end_marker: String) {
        self.synthetic_block_end_marker = Some(synthetic_block_end_marker);
    }
    // port: CompilerOptions#getSyntheticBlockStartMarker
    pub fn get_synthetic_block_start_marker(&self) -> Option<&str> {
        self.synthetic_block_start_marker.as_deref()
    }
    // port: CompilerOptions#getSyntheticBlockEndMarker
    pub fn get_synthetic_block_end_marker(&self) -> Option<&str> {
        self.synthetic_block_end_marker.as_deref()
    }
    // port: CompilerOptions#setLocale
    pub fn set_locale(&mut self, locale: String) {
        self.locale = Some(locale);
    }
    // port: CompilerOptions#getLocale
    pub fn get_locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }
    // port: CompilerOptions#setDoLateLocalization
    pub fn set_do_late_localization(&mut self, do_late_localization: bool) {
        self.do_late_localization = do_late_localization;
    }
    // port: CompilerOptions#doLateLocalization
    pub fn do_late_localization(&self) -> bool {
        self.do_late_localization
    }
    // port: CompilerOptions#shouldRunReplaceMessagesPass
    pub fn should_run_replace_messages_pass(&self) -> bool {
        !self.should_run_replace_messages_for_chrome() && self.message_bundle.is_some()
    }
    // port: CompilerOptions#setMarkAsCompiled
    pub fn set_mark_as_compiled(&mut self, mark_as_compiled: bool) {
        self.mark_as_compiled = mark_as_compiled;
    }
    // port: CompilerOptions#shouldMarkAsCompiled
    pub fn should_mark_as_compiled(&self) -> bool {
        self.mark_as_compiled
    }
    // port: CompilerOptions#setClosurePass
    pub fn set_closure_pass(&mut self, closure_pass: bool) {
        self.closure_pass = closure_pass;
    }
    // port: CompilerOptions#getClosurePass
    pub fn get_closure_pass(&self) -> bool {
        self.closure_pass
    }
    // port: CompilerOptions#setPreserveClosurePrimitives
    pub fn set_preserve_closure_primitives(&mut self, preserve_closure_primitives: bool) {
        self.preserve_closure_primitives = preserve_closure_primitives;
    }
    // port: CompilerOptions#shouldPreservesGoogProvidesAndRequires
    pub fn should_preserves_goog_provides_and_requires(&self) -> bool {
        self.preserve_closure_primitives
    }
    // port: CompilerOptions#shouldPreserveGoogModule
    pub fn should_preserve_goog_module(&self) -> bool {
        self.preserve_closure_primitives
    }
    // port: CompilerOptions#shouldPreserveGoogLibraryPrimitives
    pub fn should_preserve_goog_library_primitives(&self) -> bool {
        self.preserve_closure_primitives
    }
    // port: CompilerOptions#setPreserveTypeAnnotations
    pub fn set_preserve_type_annotations(&mut self, preserve_type_annotations: bool) {
        self.preserve_type_annotations = preserve_type_annotations;
    }
    // port: CompilerOptions#shouldPreserveTypeAnnotations
    pub fn should_preserve_type_annotations(&self) -> bool {
        self.preserve_type_annotations
    }
    // port: CompilerOptions#setGentsMode
    pub fn set_gents_mode(&mut self, gents_mode: bool) {
        self.gents_mode = gents_mode;
    }
    // port: CompilerOptions#getGentsMode
    pub fn get_gents_mode(&self) -> bool {
        self.gents_mode
    }
    // port: CompilerOptions#setGatherCssNames
    pub fn set_gather_css_names(&mut self, gather_css_names: bool) {
        self.gather_css_names = gather_css_names;
    }
    // port: CompilerOptions#shouldGatherCssNames
    pub fn should_gather_css_names(&self) -> bool {
        self.gather_css_names
    }
    // port: CompilerOptions#setStripTypes
    pub fn set_strip_types(&mut self, strip_types: IndexSet<String>) {
        self.strip_types = strip_types;
    }
    // port: CompilerOptions#getStripTypes
    pub fn get_strip_types(&self) -> &IndexSet<String> {
        &self.strip_types
    }
    // port: CompilerOptions#setStripNameSuffixes
    pub fn set_strip_name_suffixes(&mut self, strip_name_suffixes: IndexSet<String>) {
        self.strip_name_suffixes = strip_name_suffixes;
    }
    // port: CompilerOptions#getStripNameSuffixes
    pub fn get_strip_name_suffixes(&self) -> &IndexSet<String> {
        &self.strip_name_suffixes
    }
    // port: CompilerOptions#setStripNamePrefixes
    pub fn set_strip_name_prefixes(&mut self, strip_name_prefixes: IndexSet<String>) {
        self.strip_name_prefixes = strip_name_prefixes;
    }
    // port: CompilerOptions#getStripNamePrefixes
    pub fn get_strip_name_prefixes(&self) -> &IndexSet<String> {
        &self.strip_name_prefixes
    }
    // port: CompilerOptions#addCustomPass
    pub fn add_custom_pass(
        &mut self,
        time: CustomPassExecutionTime,
        custom_pass: Arc<std::sync::Mutex<dyn CompilerPass + Send>>,
    ) {
        self.custom_passes
            .get_or_insert_with(IndexMap::new)
            .entry(time)
            .or_default()
            .insert(CompilerPassRef(custom_pass));
    }
    // port: CompilerOptions#getCustomPassesAt
    pub fn get_custom_passes_at(
        &self,
        execution_time: CustomPassExecutionTime,
    ) -> Vec<Arc<std::sync::Mutex<dyn CompilerPass + Send>>> {
        self.custom_passes
            .as_ref()
            .and_then(|passes| passes.get(&execution_time))
            .map_or_else(Vec::new, |passes| {
                passes.iter().map(|p| p.0.clone()).collect()
            })
    }
    // port: CompilerOptions#setDefineReplacements
    pub fn set_define_replacements(&mut self, define_replacements: IndexMap<String, DefineValue>) {
        self.define_replacements.clear();
        self.define_replacements.extend(define_replacements);
    }
    // port: CompilerOptions#getEnableZonesDefineName
    pub fn get_enable_zones_define_name(&self) -> Option<&str> {
        self.enable_zones_define_name.as_deref()
    }
    // port: CompilerOptions#setEnableZonesDefineName
    pub fn set_enable_zones_define_name(&mut self, enable_zones_define_name: Option<String>) {
        self.enable_zones_define_name = enable_zones_define_name;
    }
    // port: CompilerOptions#getZoneInputPattern
    pub fn get_zone_input_pattern(&self) -> &Option<Pattern> {
        &self.zone_input_pattern
    }
    // port: CompilerOptions#setZoneInputPattern
    pub fn set_zone_input_pattern(&mut self, zone_input_pattern: Option<Pattern>) {
        self.zone_input_pattern = zone_input_pattern;
    }
    // port: CompilerOptions#setRewriteGlobalDeclarationsForTryCatchWrapping
    pub fn set_rewrite_global_declarations_for_try_catch_wrapping(&mut self, rewrite: bool) {
        self.rewrite_global_declarations_for_try_catch_wrapping = rewrite;
    }
    // port: CompilerOptions#shouldRewriteGlobalDeclarationsForTryCatchWrapping
    pub fn should_rewrite_global_declarations_for_try_catch_wrapping(&self) -> bool {
        self.rewrite_global_declarations_for_try_catch_wrapping
    }
    // port: CompilerOptions#setCssRenamingMap
    pub fn set_css_renaming_map(
        &mut self,
        css_renaming_map: Arc<dyn CssRenamingMap + Send + Sync>,
    ) {
        self.css_renaming_map = Some(css_renaming_map);
    }
    // port: CompilerOptions#getCssRenamingMap
    pub fn get_css_renaming_map(&self) -> &Option<Arc<dyn CssRenamingMap + Send + Sync>> {
        &self.css_renaming_map
    }
    // port: CompilerOptions#setCssRenamingSkiplist
    pub fn set_css_renaming_skiplist(&mut self, skiplist: IndexSet<String>) {
        self.css_renaming_skiplist = Some(skiplist);
    }
    // port: CompilerOptions#getCssRenamingSkiplist
    pub fn get_css_renaming_skiplist(&self) -> &Option<IndexSet<String>> {
        &self.css_renaming_skiplist
    }
    // port: CompilerOptions#setReplaceStringsFunctionDescriptions
    pub fn set_replace_strings_function_descriptions(
        &mut self,
        replace_strings_function_descriptions: Vec<String>,
    ) {
        self.replace_strings_function_descriptions = replace_strings_function_descriptions;
    }
    // port: CompilerOptions#getReplaceStringsFunctionDescriptions
    pub fn get_replace_strings_function_descriptions(&self) -> &Vec<String> {
        &self.replace_strings_function_descriptions
    }
    // port: CompilerOptions#setReplaceStringsPlaceholderToken
    pub fn set_replace_strings_placeholder_token(
        &mut self,
        replace_strings_placeholder_token: String,
    ) {
        self.replace_strings_placeholder_token = replace_strings_placeholder_token;
    }
    // port: CompilerOptions#getReplaceStringsPlaceholderToken
    pub fn get_replace_strings_placeholder_token(&self) -> &str {
        &self.replace_strings_placeholder_token
    }
    // port: CompilerOptions#setPrettyPrint
    pub fn set_pretty_print(&mut self, pretty_print: bool) {
        self.pretty_print = pretty_print;
    }
    // port: CompilerOptions#isPrettyPrint
    pub fn is_pretty_print(&self) -> bool {
        self.pretty_print
    }
    // port: CompilerOptions#setLineBreak
    pub fn set_line_break(&mut self, line_break: bool) {
        self.line_break = line_break;
    }
    // port: CompilerOptions#shouldAddLineBreak
    pub fn should_add_line_break(&self) -> bool {
        self.line_break
    }
    // port: CompilerOptions#setPrintInputDelimiter
    pub fn set_print_input_delimiter(&mut self, print_input_delimiter: bool) {
        self.print_input_delimiter = print_input_delimiter;
    }
    // port: CompilerOptions#shouldPrintInputDelimiter
    pub fn should_print_input_delimiter(&self) -> bool {
        self.print_input_delimiter
    }
    // port: CompilerOptions#setInputDelimiter
    pub fn set_input_delimiter(&mut self, input_delimiter: String) {
        self.input_delimiter = input_delimiter;
    }
    // port: CompilerOptions#getInputDelimiter
    pub fn get_input_delimiter(&self) -> &str {
        &self.input_delimiter
    }
    // port: CompilerOptions#setDebugLogDirectory
    pub fn set_debug_log_directory(&mut self, dir: Option<PathBuf>) {
        self.debug_log_directory = dir;
    }
    // port: CompilerOptions#getDebugLogDirectory
    pub fn get_debug_log_directory(&self) -> Option<&Path> {
        self.debug_log_directory.as_deref()
    }
    // port: CompilerOptions#setDebugLogFilter
    pub fn set_debug_log_filter(&mut self, filter: String) {
        self.debug_log_filter = Some(filter);
    }
    // port: CompilerOptions#getDebugLogFilter
    pub fn get_debug_log_filter(&self) -> Option<&str> {
        self.debug_log_filter.as_deref()
    }
    // port: CompilerOptions#shouldSerializeExtraDebugInfo
    pub fn should_serialize_extra_debug_info(&self) -> bool {
        self.serialize_extra_debug_info || self.get_debug_log_directory().is_some()
    }
    // port: CompilerOptions#setSerializeExtraDebugInfo
    pub fn set_serialize_extra_debug_info(&mut self, serialize_extra_debug_info: bool) {
        self.serialize_extra_debug_info = serialize_extra_debug_info;
    }
    // port: CompilerOptions#setStateCompressionWrapper
    pub fn set_state_compression_wrapper(&mut self, wrapper: Option<OutputStreamWrapper>) {
        self.state_compression_wrapper = wrapper;
    }
    // port: CompilerOptions#getStateCompressionWrapper
    pub fn get_state_compression_wrapper(&self) -> &Option<OutputStreamWrapper> {
        &self.state_compression_wrapper
    }
    // port: CompilerOptions#setStateDecompressionWrapper
    pub fn set_state_decompression_wrapper(&mut self, wrapper: Option<InputStreamWrapper>) {
        self.state_decompression_wrapper = wrapper;
    }
    // port: CompilerOptions#getStateDecompressionWrapper
    pub fn get_state_decompression_wrapper(&self) -> &Option<InputStreamWrapper> {
        &self.state_decompression_wrapper
    }
    // port: CompilerOptions#setQuoteKeywordProperties
    pub fn set_quote_keyword_properties(&mut self, quote_keyword_properties: bool) {
        self.quote_keyword_properties = quote_keyword_properties;
    }
    // port: CompilerOptions#shouldQuoteKeywordProperties
    pub fn should_quote_keyword_properties(&self) -> bool {
        if self.incremental_check_mode == IncrementalCheckMode::GENERATE_IJS {
            return false;
        }
        self.quote_keyword_properties || FeatureSet::ES3.contains(self.get_output_feature_set())
    }
    // port: CompilerOptions#setErrorFormat
    pub fn set_error_format(&mut self, error_format: ErrorFormat) {
        self.error_format = error_format;
    }
    // port: CompilerOptions#getErrorFormat
    pub fn get_error_format(&self) -> ErrorFormat {
        self.error_format
    }
    // port: CompilerOptions#setWarningsGuard
    pub fn set_warnings_guard(&mut self, warnings_guard: Arc<ComposeWarningsGuard>) {
        self.warnings_guard = warnings_guard;
    }
    // port: CompilerOptions#setLineLengthThreshold
    pub fn set_line_length_threshold(&mut self, line_length_threshold: i32) {
        self.line_length_threshold = line_length_threshold;
    }
    // port: CompilerOptions#getLineLengthThreshold
    pub fn get_line_length_threshold(&self) -> i32 {
        self.line_length_threshold
    }
    // port: CompilerOptions#setUseOriginalNamesInOutput
    pub fn set_use_original_names_in_output(&mut self, use_original_names_in_output: bool) {
        self.use_original_names_in_output = use_original_names_in_output;
    }
    // port: CompilerOptions#getUseOriginalNamesInOutput
    pub fn get_use_original_names_in_output(&self) -> bool {
        self.use_original_names_in_output
    }
    // port: CompilerOptions#setExternExportsPath
    pub fn set_extern_exports_path(&mut self, extern_exports_path: Option<String>) {
        self.extern_exports_path = extern_exports_path;
    }
    // port: CompilerOptions#getExternExportsPath
    pub fn get_extern_exports_path(&self) -> Option<&str> {
        self.extern_exports_path.as_deref()
    }
    // port: CompilerOptions#setSourceMapOutputPath
    pub fn set_source_map_output_path(&mut self, source_map_output_path: String) {
        self.source_map_output_path = Some(source_map_output_path);
    }
    // port: CompilerOptions#setApplyInputSourceMaps
    pub fn set_apply_input_source_maps(&mut self, apply_input_source_maps: bool) {
        self.apply_input_source_maps = apply_input_source_maps;
    }
    // port: CompilerOptions#getApplyInputSourceMaps
    pub fn get_apply_input_source_maps(&self) -> bool {
        self.apply_input_source_maps
    }
    // port: CompilerOptions#setResolveSourceMapAnnotations
    pub fn set_resolve_source_map_annotations(&mut self, resolve_source_map_annotations: bool) {
        self.resolve_source_map_annotations = resolve_source_map_annotations;
    }
    // port: CompilerOptions#getResolveSourceMapAnnotations
    pub fn get_resolve_source_map_annotations(&self) -> bool {
        self.resolve_source_map_annotations
    }
    // port: CompilerOptions#setSourceMapIncludeSourcesContent
    pub fn set_source_map_include_sources_content(
        &mut self,
        source_map_include_sources_content: bool,
    ) {
        self.source_map_include_sources_content = source_map_include_sources_content;
    }
    // port: CompilerOptions#getSourceMapIncludeSourcesContent
    pub fn get_source_map_include_sources_content(&self) -> bool {
        self.source_map_include_sources_content
    }
    // port: CompilerOptions#setParseInlineSourceMaps
    pub fn set_parse_inline_source_maps(&mut self, parse_inline_source_maps: bool) {
        self.parse_inline_source_maps = parse_inline_source_maps;
    }
    // port: CompilerOptions#getParseInlineSourceMaps
    pub fn get_parse_inline_source_maps(&self) -> bool {
        self.parse_inline_source_maps
    }
    // port: CompilerOptions#setSourceMapDetailLevel
    pub fn set_source_map_detail_level(&mut self, source_map_detail_level: DetailLevel) {
        self.source_map_detail_level = source_map_detail_level;
    }
    // port: CompilerOptions#getSourceMapDetailLevel
    pub fn get_source_map_detail_level(&self) -> DetailLevel {
        self.source_map_detail_level
    }
    // port: CompilerOptions#setSourceMapFormat
    pub fn set_source_map_format(&mut self, source_map_format: Format) {
        self.source_map_format = source_map_format;
    }
    // port: CompilerOptions#getSourceMapFormat
    pub fn get_source_map_format(&self) -> Format {
        self.source_map_format
    }
    // port: CompilerOptions#setSourceMapLocationMappings
    pub fn set_source_map_location_mappings(
        &mut self,
        source_map_location_mappings: Vec<Arc<dyn LocationMapping + Send + Sync>>,
    ) {
        self.source_map_location_mappings = source_map_location_mappings;
    }
    // port: CompilerOptions#getSourceMapLocationMappings
    pub fn get_source_map_location_mappings(&self) -> &Vec<Arc<dyn LocationMapping + Send + Sync>> {
        &self.source_map_location_mappings
    }
    // port: CompilerOptions#setProcessCommonJSModules
    pub fn set_process_common_js_modules(&mut self, process_common_js_modules: bool) {
        self.process_common_js_modules = process_common_js_modules;
    }
    // port: CompilerOptions#getProcessCommonJSModules
    pub fn get_process_common_js_modules(&self) -> bool {
        self.process_common_js_modules
    }
    // port: CompilerOptions#setEs6ModuleTranspilation
    pub fn set_es6_module_transpilation(&mut self, value: Es6ModuleTranspilation) {
        self.es6_module_transpilation = value;
    }
    // port: CompilerOptions#getEs6ModuleTranspilation
    pub fn get_es6_module_transpilation(&self) -> Es6ModuleTranspilation {
        self.es6_module_transpilation
    }
    // port: CompilerOptions#setCommonJSModulePathPrefix
    pub fn set_common_js_module_path_prefix(&mut self, common_js_module_path_prefix: String) {
        self.set_module_roots(vec![common_js_module_path_prefix]);
    }
    // port: CompilerOptions#setModuleRoots
    pub fn set_module_roots(&mut self, module_roots: Vec<String>) {
        self.module_roots = module_roots;
    }
    // port: CompilerOptions#getModuleRoots
    pub fn get_module_roots(&self) -> &Vec<String> {
        &self.module_roots
    }
    // port: CompilerOptions#setRewritePolyfills
    pub fn set_rewrite_polyfills(&mut self, rewrite_polyfills: bool) {
        self.rewrite_polyfills = rewrite_polyfills;
    }
    // port: CompilerOptions#getRewritePolyfills
    pub fn get_rewrite_polyfills(&self) -> bool {
        self.rewrite_polyfills
    }
    // port: CompilerOptions#setIsolatePolyfills
    pub fn set_isolate_polyfills(&mut self, isolate_polyfills: bool) {
        self.isolate_polyfills = isolate_polyfills;
        if self.isolate_polyfills {
            self.set_define_to_boolean_literal("$jscomp.ISOLATE_POLYFILLS", isolate_polyfills);
        }
    }
    // port: CompilerOptions#getIsolatePolyfills
    pub fn get_isolate_polyfills(&self) -> bool {
        self.isolate_polyfills
    }
    // port: CompilerOptions#setInjectPolyfillsNewerThan
    pub fn set_inject_polyfills_newer_than(&mut self, inject_polyfills_newer_than: LanguageMode) {
        self.inject_polyfills_newer_than = Some(inject_polyfills_newer_than);
    }
    // port: CompilerOptions#getInjectPolyfillsNewerThan
    pub fn get_inject_polyfills_newer_than(&self) -> Option<LanguageMode> {
        self.inject_polyfills_newer_than
    }
    // port: CompilerOptions#setInstrumentAsyncContext
    pub fn set_instrument_async_context(&mut self, instrument_async_context: bool) {
        self.instrument_async_context = instrument_async_context;
        self.set_define_to_boolean_literal(
            "$jscomp.INSTRUMENT_ASYNC_CONTEXT",
            instrument_async_context,
        );
    }
    // port: CompilerOptions#getInstrumentAsyncContext
    pub fn get_instrument_async_context(&self) -> bool {
        self.instrument_async_context
    }
    // port: CompilerOptions#setEs6SubclassTranspilation
    pub fn set_es6_subclass_transpilation(
        &mut self,
        es6_subclass_transpilation: Es6SubclassTranspilation,
    ) {
        self.es6_subclass_transpilation = es6_subclass_transpilation;
    }
    // port: CompilerOptions#getEs6SubclassTranspilation
    pub fn get_es6_subclass_transpilation(&self) -> Es6SubclassTranspilation {
        self.es6_subclass_transpilation
    }
    // port: CompilerOptions#setForceLibraryInjection
    pub fn set_force_library_injection(&mut self, libraries: Vec<String>) {
        self.force_library_injection = libraries;
    }
    // port: CompilerOptions#getForceLibraryInjectionList
    pub fn get_force_library_injection_list(&self) -> &Vec<String> {
        &self.force_library_injection
    }
    // port: CompilerOptions#setPreventLibraryInjection
    pub fn set_prevent_library_injection(&mut self, prevent_library_injection: bool) {
        self.runtime_library_mode = if prevent_library_injection {
            RuntimeLibraryMode::NO_OP
        } else {
            RuntimeLibraryMode::INJECT
        };
    }
    // port: CompilerOptions#setRuntimeLibraryMode
    pub fn set_runtime_library_mode(&mut self, runtime_library_mode: RuntimeLibraryMode) {
        self.runtime_library_mode = runtime_library_mode;
    }
    // port: CompilerOptions#getRuntimeLibraryMode
    pub fn get_runtime_library_mode(&self) -> RuntimeLibraryMode {
        self.runtime_library_mode
    }
    // port: CompilerOptions#setUnusedImportsToRemove
    pub fn set_unused_imports_to_remove(
        &mut self,
        unused_imports_to_remove: Option<IndexSet<String>>,
    ) {
        self.unused_imports_to_remove = unused_imports_to_remove;
    }
    // port: CompilerOptions#getUnusedImportsToRemove
    pub fn get_unused_imports_to_remove(&self) -> &Option<IndexSet<String>> {
        &self.unused_imports_to_remove
    }
    // port: CompilerOptions#setInstrumentForCoverageOption
    pub fn set_instrument_for_coverage_option(
        &mut self,
        instrument_for_coverage_option: InstrumentOption,
    ) {
        self.instrument_for_coverage_option = instrument_for_coverage_option;
    }
    // port: CompilerOptions#getInstrumentForCoverageOption
    pub fn get_instrument_for_coverage_option(&self) -> InstrumentOption {
        self.instrument_for_coverage_option
    }
    // port: CompilerOptions#setProductionInstrumentationArrayName
    pub fn set_production_instrumentation_array_name(
        &mut self,
        production_instrumentation_array_name: String,
    ) {
        self.production_instrumentation_array_name = production_instrumentation_array_name;
    }
    // port: CompilerOptions#getProductionInstrumentationArrayName
    pub fn get_production_instrumentation_array_name(&self) -> &str {
        &self.production_instrumentation_array_name
    }
    // port: CompilerOptions#getConformanceConfigs
    pub fn get_conformance_configs(&self) -> &Vec<ConformanceConfig> {
        &self.conformance_configs
    }
    // port: CompilerOptions#setConformanceConfig
    pub fn set_conformance_config(&mut self, conformance_config: ConformanceConfig) {
        self.set_conformance_configs(vec![conformance_config]);
    }
    // port: CompilerOptions#setConformanceConfigs
    pub fn set_conformance_configs(&mut self, configs: Vec<ConformanceConfig>) {
        self.conformance_configs = configs;
    }
    // port: CompilerOptions#clearConformanceConfigs
    pub fn clear_conformance_configs(&mut self) {
        self.conformance_configs = Vec::new();
    }
    // port: CompilerOptions#shouldEmitUseStrict
    pub fn should_emit_use_strict(&self) -> bool {
        self.emit_use_strict
            .or(self.language_out_is_default_strict)
            .unwrap_or_else(|| self.language_in.is_default_strict())
    }
    // port: CompilerOptions#setEmitUseStrict
    pub fn set_emit_use_strict(&mut self, emit_use_strict: bool) -> &mut Self {
        self.emit_use_strict = Some(emit_use_strict);
        self
    }
    // port: CompilerOptions#getModuleResolutionMode
    pub fn get_module_resolution_mode(&self) -> ResolutionMode {
        self.module_resolution_mode
    }
    // port: CompilerOptions#setModuleResolutionMode
    pub fn set_module_resolution_mode(&mut self, module_resolution_mode: ResolutionMode) {
        self.module_resolution_mode = module_resolution_mode;
    }
    // port: CompilerOptions#getBrowserResolverPrefixReplacements
    pub fn get_browser_resolver_prefix_replacements(&self) -> &IndexMap<String, String> {
        &self.browser_resolver_prefix_replacements
    }
    // port: CompilerOptions#setBrowserResolverPrefixReplacements
    pub fn set_browser_resolver_prefix_replacements(
        &mut self,
        browser_resolver_prefix_replacements: IndexMap<String, String>,
    ) {
        self.browser_resolver_prefix_replacements = browser_resolver_prefix_replacements;
    }
    // port: CompilerOptions#setPathEscaper
    pub fn set_path_escaper(&mut self, path_escaper: PathEscaper) {
        self.path_escaper = path_escaper;
    }
    // port: CompilerOptions#getPathEscaper
    pub fn get_path_escaper(&self) -> PathEscaper {
        self.path_escaper
    }
    // port: CompilerOptions#getPackageJsonEntryNames
    pub fn get_package_json_entry_names(&self) -> &Vec<String> {
        &self.package_json_entry_names
    }
    // port: CompilerOptions#setPackageJsonEntryNames
    pub fn set_package_json_entry_names(&mut self, names: Vec<String>) {
        self.package_json_entry_names = names;
    }
    // port: CompilerOptions#setUseSizeHeuristicToStopOptimizationLoop
    pub fn set_use_size_heuristic_to_stop_optimization_loop(&mut self, may_stop_early: bool) {
        self.use_size_heuristic_to_stop_optimization_loop = may_stop_early;
    }
    // port: CompilerOptions#shouldUseSizeHeuristicToStopOptimizationLoop
    pub fn should_use_size_heuristic_to_stop_optimization_loop(&self) -> bool {
        self.use_size_heuristic_to_stop_optimization_loop
    }
    // port: CompilerOptions#setMaxOptimizationLoopIterations
    pub fn set_max_optimization_loop_iterations(&mut self, max_iterations: i32) {
        self.optimization_loop_max_iterations = max_iterations;
    }
    // port: CompilerOptions#getMaxOptimizationLoopIterations
    pub fn get_max_optimization_loop_iterations(&self) -> i32 {
        self.optimization_loop_max_iterations
    }
    // port: CompilerOptions#getChunkOutputType
    pub fn get_chunk_output_type(&self) -> ChunkOutputType {
        self.chunk_output_type
    }
    // port: CompilerOptions#setChunkOutputType
    pub fn set_chunk_output_type(&mut self, chunk_output_type: ChunkOutputType) {
        self.chunk_output_type = chunk_output_type;
    }
    // port: CompilerOptions#setStrictMessageReplacement
    pub fn set_strict_message_replacement(&mut self, strict_message_replacement: bool) {
        self.strict_message_replacement = strict_message_replacement;
    }
    // port: CompilerOptions#getStrictMessageReplacement
    pub fn get_strict_message_replacement(&self) -> bool {
        self.strict_message_replacement
    }
    // port: CompilerOptions#toString

    // port: CompilerOptions#expectStrictModeInput
    pub fn expect_strict_mode_input(&self) -> bool {
        self.is_strict_mode_input
            .unwrap_or_else(|| self.get_language_in().is_default_strict())
    }
    // port: CompilerOptions#setStrictModeInput
    pub fn set_strict_mode_input(&mut self, is_strict_mode_input: bool) -> &mut Self {
        self.is_strict_mode_input = Some(is_strict_mode_input);
        self
    }
    // port: CompilerOptions#getPropertyReservedNamingFirstChars
    pub fn get_property_reserved_naming_first_chars(&self) -> IndexSet<u16> {
        if self.polymer_pass {
            Self::get_polymer_property_reserved_first_chars()
        } else if self.angular_pass {
            Self::get_angular_property_reserved_first_chars()
        } else {
            IndexSet::new()
        }
    }
    // port: CompilerOptions#getPropertyReservedNamingNonFirstChars
    pub fn get_property_reserved_naming_non_first_chars(&self) -> IndexSet<u16> {
        if self.polymer_pass {
            Self::POLYMER_PROPERTY_RESERVED_NON_FIRST_CHARS
                .iter()
                .copied()
                .collect()
        } else {
            IndexSet::new()
        }
    }
    // port: CompilerOptions#shouldOptimize
    pub fn should_optimize(&self) -> bool {
        !self.skip_non_transpilation_passes
            && !self.checks_only
            && !self.should_generate_typed_externs()
            && !self.instrument_for_coverage_only
    }
}
impl Default for CompilerOptions {
    // port: CompilerOptions#CompilerOptions
    fn default() -> Self {
        Self::new()
    }
}

impl Reach {
    // port: CompilerOptions.Reach#isOn
    pub fn is_on(self) -> bool {
        self != Self::NONE
    }
    // port: CompilerOptions.Reach#includesGlobals
    pub fn includes_globals(self) -> bool {
        self == Self::ALL
    }
}
impl BrowserFeaturesetYear {
    pub const YEAR_MAP: &'static [(i32, Self)] = &[
        (2012, Self::YEAR_2012),
        (2018, Self::YEAR_2018),
        (2019, Self::YEAR_2019),
        (2020, Self::YEAR_2020),
        (2021, Self::YEAR_2021),
        (2022, Self::YEAR_2022),
        (2023, Self::YEAR_2023),
        (2024, Self::YEAR_2024),
        (2025, Self::YEAR_2025),
        (2026, Self::YEAR_2026),
    ];
    // port: CompilerOptions.BrowserFeaturesetYear#from
    pub fn from(year: i32) -> Self {
        check_state!(
            Self::YEAR_MAP.iter().any(|(y, _)| *y == year),
            "Illegal browser_featureset_year=%s. We support values 2012, or 2018..2026 only",
            year
        );
        Self::YEAR_MAP.iter().find(|(y, _)| *y == year).unwrap().1
    }
    // port: CompilerOptions.BrowserFeaturesetYear#setDependentValuesFromYear
    pub fn set_dependent_values_from_year(self, options: &mut CompilerOptions) {
        let year = self.get_year();
        options.set_output_feature_set(self.get_feature_set());
        options.language_out_is_default_strict = Some(true);
        options.set_define_to_number_literal("goog.FEATURESET_YEAR", year);
        options.set_define_to_boolean_literal("$jscomp.ASSUME_ES5", year > 2012);
        options.set_define_to_boolean_literal("$jscomp.ASSUME_ES6", year >= 2018);
        options.set_define_to_boolean_literal("$jscomp.ASSUME_ES2020", year >= 2021);
    }
    // port: CompilerOptions.BrowserFeaturesetYear#getFeatureSet
    pub fn get_feature_set(self) -> FeatureSet {
        BROWSER_FEATURESET_YEAR_DATA[self as usize].feature_set
    }
    // port: CompilerOptions.BrowserFeaturesetYear#getYear
    pub fn get_year(self) -> i32 {
        BROWSER_FEATURESET_YEAR_DATA[self as usize].year
    }
    // port: CompilerOptions.BrowserFeaturesetYear#minimumRequiredFor
    pub fn minimum_required_for(feature: Feature) -> Option<Self> {
        Self::YEAR_MAP
            .iter()
            .map(|(_, v)| *v)
            .find(|v| v.get_feature_set().contains(feature))
    }
}
impl InstrumentOption {
    // port: CompilerOptions.InstrumentOption#fromString
    pub fn from_string(value: Option<&str>) -> Option<Self> {
        match value? {
            "NONE" => Some(Self::NONE),
            "LINE" => Some(Self::LINE_ONLY),
            "BRANCH" => Some(Self::BRANCH_ONLY),
            "PRODUCTION" => Some(Self::PRODUCTION),
            _ => None,
        }
    }
}
impl LanguageMode {
    pub const STABLE_IN: Self = Self::ECMASCRIPT_NEXT;
    pub const STABLE_OUT: Self = Self::ECMASCRIPT5;
    // port: CompilerOptions.LanguageMode#isDefaultStrict
    pub fn is_default_strict(self) -> bool {
        !matches!(self, Self::ECMASCRIPT3 | Self::ECMASCRIPT5)
    }
    // port: CompilerOptions.LanguageMode#validCommandLineNames
    pub fn valid_command_line_names() -> Vec<String> {
        let mut names = Vec::new();
        for mode in Self::VALUES {
            if *mode != Self::UNSUPPORTED {
                let name = mode.to_string();
                names.push(name.clone());
                if let Some(suffix) = name.strip_prefix("ECMASCRIPT") {
                    names.push(format!("ES{suffix}"));
                }
            }
        }
        names.extend(["ECMASCRIPT6", "ES6", "ECMASCRIPT6_STRICT", "ES6_STRICT"].map(String::from));
        names
    }
    // port: CompilerOptions.LanguageMode#fromString
    pub fn from_string(value: Option<&str>) -> Option<Self> {
        let value = value?
            .trim_matches(|c: char| c as u32 <= 0x20)
            .to_ascii_uppercase();
        let canonicalized_name = value
            .strip_prefix("ES")
            .map_or_else(|| value.clone(), |rest| format!("ECMASCRIPT{rest}"));
        if canonicalized_name == "ECMASCRIPT6" || canonicalized_name == "ECMASCRIPT6_STRICT" {
            return Some(Self::ECMASCRIPT_2015);
        }
        Self::value_of(&canonicalized_name)
    }
    // port: CompilerOptions.LanguageMode#toFeatureSet
    pub fn to_feature_set(self) -> FeatureSet {
        match self {
            Self::ECMASCRIPT3 => FeatureSet::ES3,
            Self::ECMASCRIPT5 | Self::ECMASCRIPT5_STRICT => FeatureSet::ES5,
            Self::ECMASCRIPT_2015 => FeatureSet::ES2015_MODULES,
            Self::ECMASCRIPT_2016 => FeatureSet::ES2016_MODULES,
            Self::ECMASCRIPT_2017 => FeatureSet::ES2017_MODULES,
            Self::ECMASCRIPT_2018 => FeatureSet::ES2018_MODULES,
            Self::ECMASCRIPT_2019 => FeatureSet::ES2019_MODULES,
            Self::ECMASCRIPT_2020 => FeatureSet::ES2020_MODULES,
            Self::ECMASCRIPT_2021 => FeatureSet::ES2021_MODULES,
            Self::ECMASCRIPT_2022 => FeatureSet::ES2022_MODULES,
            Self::ECMASCRIPT_NEXT => FeatureSet::ES_NEXT,
            Self::NO_TRANSPILE | Self::UNSTABLE => FeatureSet::ES_UNSTABLE,
            Self::UNSUPPORTED => FeatureSet::ES_UNSUPPORTED,
            Self::STABLE => panic!(
                "STABLE has different feature sets for language in and out. Use STABLE_IN or STABLE_OUT."
            ),
        }
    }
}
impl TracerMode {
    // port: CompilerOptions.TracerMode#isOn
    pub fn is_on(self) -> bool {
        self != Self::OFF
    }
    // port: CompilerOptions.TracerMode#doPruningAnalysis
    pub fn do_pruning_analysis(self) -> bool {
        matches!(
            self,
            Self::AST_SIZE_AND_PRUNING | Self::ALL | Self::RAW_SIZE
        )
    }
}
impl TweakProcessing {
    // port: CompilerOptions.TweakProcessing#isOn
    pub fn is_on(self) -> bool {
        self != Self::OFF
    }
    // port: CompilerOptions.TweakProcessing#shouldStrip
    pub fn should_strip(self) -> bool {
        self == Self::STRIP
    }
}
impl J2clPassMode {
    // port: CompilerOptions.J2clPassMode#shouldAddJ2clPasses
    pub fn should_add_j2cl_passes(self) -> bool {
        self == Self::AUTO
    }
}

impl CompilerOptions {
    // Rust arena adapter: borrow the Java shared CodingConvention object.
    pub fn get_coding_convention_ref(&self) -> Option<&dyn CodingConvention> {
        self.coding_convention
            .as_ref()
            .map(|c| c.as_ref() as &dyn CodingConvention)
    }
}
impl fmt::Display for CompilerOptions {
    // port: CompilerOptions#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Object.toString components in Java carry an identity hash. As specified by this port's
        // API, their fully qualified class name is printed without the identity hash.
        let mut entries = Vec::<String>::new();
        let displayed = self.alias_strings_mode.to_string();
        entries.push(format!("aliasStringsMode={displayed}"));
        let displayed = self.ambiguate_properties.to_string();
        entries.push(format!("ambiguateProperties={displayed}"));
        let displayed = self.angular_pass.to_string();
        entries.push(format!("angularPass={displayed}"));
        let displayed = self.assume_closures_only_capture_references.to_string();
        entries.push(format!("assumeClosuresOnlyCaptureReferences={displayed}"));
        let displayed = self.assume_getters_are_pure.to_string();
        entries.push(format!("assumeGettersArePure={displayed}"));
        let displayed = self.assume_properties_are_statically_analyzable.to_string();
        entries.push(format!(
            "assumePropertiesAreStaticallyAnalyzable={displayed}"
        ));
        let displayed = self.assume_strict_this.to_string();
        entries.push(format!("assumeStrictThis={displayed}"));
        let displayed = format!(
            "{{{}}}",
            self.browser_resolver_prefix_replacements
                .iter()
                .map(|(key, value)| {
                    let rendered = value.to_string();
                    format!("{key}={rendered}")
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("browserResolverPrefixReplacements={displayed}"));
        let displayed = self.check_determinism.to_string();
        entries.push(format!("checkDeterminism={displayed}"));
        let displayed = self.check_suspicious_code.to_string();
        entries.push(format!("checkSuspiciousCode={displayed}"));
        let displayed = self.check_symbols.to_string();
        entries.push(format!("checkSymbols={displayed}"));
        let displayed = self.check_types.to_string();
        entries.push(format!("checkTypes={displayed}"));
        let displayed = self.checks_only.to_string();
        entries.push(format!("checksOnly={displayed}"));
        let displayed = format!(
            "[{}]",
            self.chunks_to_print_after_each_pass_regex_list
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("chunksToPrintAfterEachPassRegexList={displayed}"));
        let displayed = self.closure_pass.to_string();
        entries.push(format!("closurePass={displayed}"));
        let displayed = self.coalesce_variable_names.to_string();
        entries.push(format!("coalesceVariableNames={displayed}"));
        if let Some(_value) = &self.coding_convention {
            let displayed = {
                let convention = _value.as_ref() as &dyn std::any::Any;
                if convention.is::<crate::coding_conventions::DefaultCodingConvention>() {
                    "com.google.javascript.jscomp.CodingConventions$DefaultCodingConvention"
                } else if convention
                    .is::<crate::closure_coding_convention::ClosureCodingConvention>()
                {
                    "com.google.javascript.jscomp.ClosureCodingConvention"
                } else if convention.is::<crate::google_coding_convention::GoogleCodingConvention>()
                {
                    "com.google.javascript.jscomp.GoogleCodingConvention"
                } else {
                    "com.google.javascript.jscomp.CodingConventions$Proxy"
                }
            }
            .to_string();
            entries.push(format!("codingConvention={displayed}"));
        }
        let displayed = self.collapse_anonymous_functions.to_string();
        entries.push(format!("collapseAnonymousFunctions={displayed}"));
        let displayed = self.collapse_object_literals.to_string();
        entries.push(format!("collapseObjectLiterals={displayed}"));
        let displayed = self.collapse_properties_level.to_string();
        entries.push(format!("collapseProperties={displayed}"));
        let displayed = self.collapse_variable_declarations.to_string();
        entries.push(format!("collapseVariableDeclarations={displayed}"));
        let displayed = self.colorize_error_output.to_string();
        entries.push(format!("colorizeErrorOutput={displayed}"));
        let displayed = self.compute_function_side_effects.to_string();
        entries.push(format!("computeFunctionSideEffects={displayed}"));
        let displayed = format!(
            "[{}]",
            self.conformance_configs
                .iter()
                // AbstractMessage#toString is TextFormat.printer().printToString(this).
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("conformanceConfigs={displayed}"));
        let displayed = self
            .conformance_remove_regex_from_path
            .as_ref()
            .map_or_else(
                || "Optional.absent()".to_string(),
                |value| {
                    let rendered = value.to_string();
                    format!("Optional.of({rendered})")
                },
            );
        entries.push(format!("conformanceRemoveRegexFromPath={displayed}"));
        let displayed = self.conformance_reporting_mode.to_string();
        entries.push(format!("conformanceReportingMode={displayed}"));
        let displayed = self.continue_after_errors.to_string();
        entries.push(format!("continueAfterErrors={displayed}"));
        let displayed = self.convert_to_dotted_properties.to_string();
        entries.push(format!("convertToDottedProperties={displayed}"));
        let displayed = self.cross_chunk_code_motion.to_string();
        entries.push(format!("crossChunkCodeMotion={displayed}"));
        let displayed = self.cross_chunk_code_motion_no_stub_methods.to_string();
        entries.push(format!("crossChunkCodeMotionNoStubMethods={displayed}"));
        let displayed = self.cross_chunk_method_motion.to_string();
        entries.push(format!("crossChunkMethodMotion={displayed}"));
        if let Some(_value) = &self.css_renaming_map {
            let displayed = "com.google.javascript.jscomp.CssRenamingMap".to_string();
            entries.push(format!("cssRenamingMap={displayed}"));
        }
        if let Some(value) = &self.css_renaming_skiplist {
            let displayed = format!(
                "[{}]",
                value
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            entries.push(format!("cssRenamingSkiplist={displayed}"));
        }
        if let Some(value) = &self.custom_passes {
            let displayed = format!(
                "{{{}}}",
                value
                    .iter()
                    .map(|(key, value)| {
                        let rendered = format!(
                            "[{}]",
                            value
                                .iter()
                                .map(|_value| "com.google.javascript.jscomp.CompilerPass"
                                    .to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        format!("{key}={rendered}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            entries.push(format!("customPasses={displayed}"));
        }
        let displayed = self.dead_assignment_elimination.to_string();
        entries.push(format!("deadAssignmentElimination={displayed}"));
        if let Some(value) = &self.debug_log_directory {
            let displayed = value.display().to_string();
            entries.push(format!("debugLogDirectory={displayed}"));
        }
        let displayed = {
            let mut ast = Ast::new();
            let map = self.get_define_replacements(&mut ast);
            format!(
                "{{{}}}",
                map.iter()
                    .map(|(name, node)| format!("{name}={}", node.to_string(&ast)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        entries.push(format!("defineReplacements={displayed}"));
        let displayed = self.dependency_options.to_string();
        entries.push(format!("dependencyOptions={displayed}"));
        let displayed = self.dev_mode.to_string();
        entries.push(format!("devMode={displayed}"));
        let displayed = self.devirtualize_methods.to_string();
        entries.push(format!("devirtualizeMethods={displayed}"));
        let displayed = self.disambiguate_properties.to_string();
        entries.push(format!("disambiguateProperties={displayed}"));
        let displayed = self.emit_use_strict.as_ref().map_or_else(
            || "Optional.absent()".to_string(),
            |value| {
                let rendered = value.to_string();
                format!("Optional.of({rendered})")
            },
        );
        entries.push(format!("emitUseStrict={displayed}"));
        let displayed = self.enable_module_rewriting.to_string();
        entries.push(format!("enableModuleRewriting={displayed}"));
        let displayed = self.environment.to_string();
        entries.push(format!("environment={displayed}"));
        let displayed = format!("{:?}", self.error_format);
        entries.push(format!("errorFormat={displayed}"));
        if let Some(_value) = &self.error_handler {
            let displayed = "com.google.javascript.jscomp.ErrorHandler".to_string();
            entries.push(format!("errorHandler={displayed}"));
        }
        let displayed = self.es6_module_transpilation.to_string();
        entries.push(format!("es6ModuleTranspilation={displayed}"));
        let displayed = self.es6_subclass_transpilation.to_string();
        entries.push(format!("es6SubclassTranspilation={displayed}"));
        let displayed = self.export_local_property_definitions.to_string();
        entries.push(format!("exportLocalPropertyDefinitions={displayed}"));
        let displayed = self.export_test_functions.to_string();
        entries.push(format!("exportTestFunctions={displayed}"));
        if let Some(value) = &self.extern_exports_path {
            let displayed = value.to_string();
            entries.push(format!("externExportsPath={displayed}"));
        }
        if let Some(value) = &self.extra_annotation_names {
            let displayed = format!(
                "[{}]",
                value
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            entries.push(format!("extraAnnotationNames={displayed}"));
        }
        let displayed = self.extract_prototype_member_declarations.to_string();
        entries.push(format!("extractPrototypeMemberDeclarations={displayed}"));
        let displayed = format!(
            "[{}]",
            self.files_to_print_after_each_pass_regex_list
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("filesToPrintAfterEachPassRegexList={displayed}"));
        let displayed = self.flow_sensitive_inline_variables.to_string();
        entries.push(format!("flowSensitiveInlineVariables={displayed}"));
        let displayed = self.fold_constants.to_string();
        entries.push(format!("foldConstants={displayed}"));
        let displayed = format!(
            "[{}]",
            self.force_library_injection
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("forceLibraryInjection={displayed}"));
        let displayed = self.gather_css_names.to_string();
        entries.push(format!("gatherCssNames={displayed}"));
        let displayed = self.generate_exports.to_string();
        entries.push(format!("generateExports={displayed}"));
        let displayed = self.generate_pseudo_names.to_string();
        entries.push(format!("generatePseudoNames={displayed}"));
        let displayed = self.should_generate_typed_externs().to_string();
        entries.push(format!("generateTypedExterns={displayed}"));
        let displayed = format!(
            "{{{}}}",
            self.id_generators
                .iter()
                .map(|(key, value)| {
                    let rendered = if let Some(token) =
                        (value.as_ref() as &dyn std::any::Any).downcast_ref::<RenamingToken>()
                    {
                        token.to_string()
                    } else {
                        "com.google.javascript.jscomp.RenamingMap".to_string()
                    };
                    format!("{key}={rendered}")
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("idGenerators={displayed}"));
        if let Some(value) = &self.id_generators_map_serialized {
            let displayed = value.to_string();
            entries.push(format!("idGeneratorsMapSerialized={displayed}"));
        }
        let displayed = self.incremental_check_mode.to_string();
        entries.push(format!("incrementalCheckMode={displayed}"));
        let displayed = self.infer_consts.to_string();
        entries.push(format!("inferConsts={displayed}"));
        let displayed = self.infer_types.to_string();
        entries.push(format!("inferTypes={displayed}"));
        let displayed = self.inline_constant_vars.to_string();
        entries.push(format!("inlineConstantVars={displayed}"));
        let displayed = self.inline_functions_level.to_string();
        entries.push(format!("inlineFunctionsLevel={displayed}"));
        let displayed = self.inline_getters.to_string();
        entries.push(format!("inlineGetters={displayed}"));
        let displayed = self.inline_local_variables.to_string();
        entries.push(format!("inlineLocalVariables={displayed}"));
        let displayed = self.inline_properties.to_string();
        entries.push(format!("inlineProperties={displayed}"));
        let displayed = self.inline_variables.to_string();
        entries.push(format!("inlineVariables={displayed}"));
        let displayed = self.input_delimiter.to_string();
        entries.push(format!("inputDelimiter={displayed}"));
        if let Some(_value) = &self.input_property_map {
            let displayed = "com.google.javascript.jscomp.VariableMap".to_string();
            entries.push(format!("inputPropertyMap={displayed}"));
        }
        let displayed = format!(
            "{{{}}}",
            self.input_source_maps
                .iter()
                .map(|(key, _value)| {
                    let rendered = "com.google.javascript.jscomp.SourceMapInput".to_string();
                    format!("{key}={rendered}")
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("inputSourceMaps={displayed}"));
        if let Some(_value) = &self.input_variable_map {
            let displayed = "com.google.javascript.jscomp.VariableMap".to_string();
            entries.push(format!("inputVariableMap={displayed}"));
        }
        let displayed = self.instrument_for_coverage_only.to_string();
        entries.push(format!("instrumentForCoverageOnly={displayed}"));
        let displayed = self.instrument_for_coverage_option.to_string();
        entries.push(format!("instrumentForCoverageOption={displayed}"));
        let displayed = self.isolate_polyfills.to_string();
        entries.push(format!("isolatePolyfills={displayed}"));
        let displayed = self.j2cl_minifier_enabled.to_string();
        entries.push(format!("j2clMinifierEnabled={displayed}"));
        if let Some(value) = &self.j2cl_minifier_pruning_manifest {
            let displayed = value.to_string();
            entries.push(format!("j2clMinifierPruningManifest={displayed}"));
        }
        let displayed = self.j2cl_pass_mode.to_string();
        entries.push(format!("j2clPassMode={displayed}"));
        let displayed = self.label_renaming.to_string();
        entries.push(format!("labelRenaming={displayed}"));
        let displayed = self.language_in.to_string();
        entries.push(format!("languageIn={displayed}"));
        let displayed = self.language_out_is_default_strict.as_ref().map_or_else(
            || "Optional.absent()".to_string(),
            |value| {
                let rendered = value.to_string();
                format!("Optional.of({rendered})")
            },
        );
        entries.push(format!("languageOutIsDefaultStrict={displayed}"));
        let displayed = self.line_break.to_string();
        entries.push(format!("lineBreak={displayed}"));
        let displayed = self.line_length_threshold.to_string();
        entries.push(format!("lineLengthThreshold={displayed}"));
        if let Some(value) = &self.locale {
            let displayed = value.to_string();
            entries.push(format!("locale={displayed}"));
        }
        let displayed = self.mark_as_compiled.to_string();
        entries.push(format!("markAsCompiled={displayed}"));
        let displayed = self.max_function_size_after_inlining.to_string();
        entries.push(format!("maxFunctionSizeAfterInlining={displayed}"));
        if let Some(_value) = &self.message_bundle {
            let displayed = if (_value.as_ref() as &dyn std::any::Any).is::<EmptyMessageBundle>() {
                "com.google.javascript.jscomp.EmptyMessageBundle".to_string()
            } else {
                "com.google.javascript.jscomp.MessageBundle".to_string()
            };
            entries.push(format!("messageBundle={displayed}"));
        }
        let displayed = format!(
            "[{}]",
            self.module_roots
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("moduleRoots={displayed}"));
        let displayed = "com.google.javascript.jscomp.DefaultNameGenerator".to_string();
        entries.push(format!("nameGenerator={displayed}"));
        let displayed = self.num_parallel_threads.to_string();
        entries.push(format!("numParallelThreads={displayed}"));
        let displayed = self.optimize_calls.to_string();
        entries.push(format!("optimizeCalls={displayed}"));
        let displayed = self.optimize_es_class_constructors.to_string();
        entries.push(format!("optimizeESClassConstructors={displayed}"));
        let displayed = self
            .optimize_local_access_for_global_symbol_namespace
            .to_string();
        entries.push(format!(
            "optimizeLocalAccessForGlobalSymbolNamespace={displayed}"
        ));
        if let Some(value) = &self.output_charset {
            let displayed = value.name().to_string();
            entries.push(format!("outputCharset={displayed}"));
        }
        let displayed = self.output_feature_set.as_ref().map_or_else(
            || "Optional.absent()".to_string(),
            |value| {
                let rendered = value.to_string();
                format!("Optional.of({rendered})")
            },
        );
        entries.push(format!("outputFeatureSet={displayed}"));
        let displayed = self.output_js.to_string();
        entries.push(format!("outputJs={displayed}"));
        let displayed = self.output_js_string_usage.to_string();
        entries.push(format!("outputJsStringUsage={displayed}"));
        let displayed = self
            .parent_chunk_can_see_symbols_declared_in_children
            .to_string();
        entries.push(format!(
            "parentChunkCanSeeSymbolsDeclaredInChildren={displayed}"
        ));
        // Java Enum#toString is the constant name, which is the Debug form here.
        let displayed = format!("{:?}", self.parse_js_doc_documentation);
        entries.push(format!("parseJsDocDocumentation={displayed}"));
        let displayed = self.path_escaper.to_string();
        entries.push(format!("pathEscaper={displayed}"));
        let displayed = self.polymer_pass.to_string();
        entries.push(format!("polymerPass={displayed}"));
        let displayed = self.prefer_single_quotes.to_string();
        entries.push(format!("preferSingleQuotes={displayed}"));
        let displayed = self.prefer_stable_names.to_string();
        entries.push(format!("preferStableNames={displayed}"));
        let displayed = self.preserve_detailed_source_info.to_string();
        entries.push(format!("preserveDetailedSourceInfo={displayed}"));
        let displayed = self.preserve_closure_primitives.to_string();
        entries.push(format!("preserveGoogProvidesAndRequires={displayed}"));
        let displayed = self.preserve_non_jsdoc_comments.to_string();
        entries.push(format!("preserveNonJSDocComments={displayed}"));
        let displayed = self.preserve_type_annotations.to_string();
        entries.push(format!("preserveTypeAnnotations={displayed}"));
        let displayed = self.pretty_print.to_string();
        entries.push(format!("prettyPrint={displayed}"));
        let displayed = self.print_config.to_string();
        entries.push(format!("printConfig={displayed}"));
        let displayed = self.print_input_delimiter.to_string();
        entries.push(format!("printInputDelimiter={displayed}"));
        let displayed = self.print_source_after_each_pass.to_string();
        entries.push(format!("printSourceAfterEachPass={displayed}"));
        let displayed = self.process_common_js_modules.to_string();
        entries.push(format!("processCommonJSModules={displayed}"));
        let displayed = self.production_instrumentation_array_name.to_string();
        entries.push(format!("productionInstrumentationArrayName={displayed}"));
        let displayed = format!(
            "[{}]",
            self.properties_that_must_disambiguate
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("propertiesThatMustDisambiguate={displayed}"));
        let displayed = self.property_renaming.to_string();
        entries.push(format!("propertyRenaming={displayed}"));
        let displayed = self.property_renaming_only_compilation_mode.to_string();
        entries.push(format!("propertyRenamingOnlyCompilationMode={displayed}"));
        let displayed = self.protect_hidden_side_effects.to_string();
        entries.push(format!("protectHiddenSideEffects={displayed}"));
        let displayed = format!(
            "[{}]",
            self.qname_uses_to_print_after_each_pass_list
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!(
            "qnameUsesToPrintAfterEachPassRegexList={displayed}"
        ));
        let displayed = self.quote_keyword_properties.to_string();
        entries.push(format!("quoteKeywordProperties={displayed}"));
        let displayed = self.remove_abstract_methods.to_string();
        entries.push(format!("removeAbstractMethods={displayed}"));
        let displayed = self.remove_closure_asserts.to_string();
        entries.push(format!("removeClosureAsserts={displayed}"));
        let displayed = self.remove_j2cl_asserts.to_string();
        entries.push(format!("removeJ2clAsserts={displayed}"));
        let displayed = self.remove_unused_class_properties.to_string();
        entries.push(format!("removeUnusedClassProperties={displayed}"));
        let displayed = self.remove_unused_local_vars.to_string();
        entries.push(format!("removeUnusedLocalVars={displayed}"));
        let displayed = self.remove_unused_prototype_properties.to_string();
        entries.push(format!("removeUnusedPrototypeProperties={displayed}"));
        let displayed = self.remove_unused_vars.to_string();
        entries.push(format!("removeUnusedVars={displayed}"));
        if let Some(value) = &self.rename_prefix {
            let displayed = value.to_string();
            entries.push(format!("renamePrefix={displayed}"));
        }
        if let Some(value) = &self.rename_prefix_namespace {
            let displayed = value.to_string();
            entries.push(format!("renamePrefixNamespace={displayed}"));
        }
        let displayed = self
            .rename_prefix_namespace_assume_cross_chunk_names
            .to_string();
        entries.push(format!(
            "renamePrefixNamespaceAssumeCrossChunkNames={displayed}"
        ));
        let displayed = self.replace_id_generators.to_string();
        entries.push(format!("replaceIdGenerators={displayed}"));
        let displayed = self.replace_messages_with_chrome_i18n.to_string();
        entries.push(format!("replaceMessagesWithChromeI18n={displayed}"));
        let displayed = format!(
            "[{}]",
            self.replace_strings_function_descriptions
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("replaceStringsFunctionDescriptions={displayed}"));
        let displayed = self.replace_strings_placeholder_token.to_string();
        entries.push(format!("replaceStringsPlaceholderToken={displayed}"));
        let displayed = self.reserve_raw_exports.to_string();
        entries.push(format!("reserveRawExports={displayed}"));
        let displayed = self.rewrite_function_expressions.to_string();
        entries.push(format!("rewriteFunctionExpressions={displayed}"));
        let displayed = self
            .rewrite_global_declarations_for_try_catch_wrapping
            .to_string();
        entries.push(format!(
            "rewriteGlobalDeclarationsForTryCatchWrapping={displayed}"
        ));
        let displayed = self.rewrite_modules_before_typechecking.to_string();
        entries.push(format!("rewriteModulesBeforeTypechecking={displayed}"));
        let displayed = self.rewrite_polyfills.to_string();
        entries.push(format!("rewritePolyfills={displayed}"));
        let displayed = self.runtime_library_mode.to_string();
        entries.push(format!("runtimeLibraryMode={displayed}"));
        let displayed = self.skip_non_transpilation_passes.to_string();
        entries.push(format!("skipNonTranspilationPasses={displayed}"));
        let displayed = self.smart_name_removal.to_string();
        entries.push(format!("smartNameRemoval={displayed}"));
        let displayed = self.source_map_detail_level.to_string();
        entries.push(format!("sourceMapDetailLevel={displayed}"));
        let displayed = self.source_map_format.to_string();
        entries.push(format!("sourceMapFormat={displayed}"));
        let displayed = format!(
            "[{}]",
            self.source_map_location_mappings
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("sourceMapLocationMappings={displayed}"));
        if let Some(value) = &self.source_map_output_path {
            let displayed = value.to_string();
            entries.push(format!("sourceMapOutputPath={displayed}"));
        }
        let displayed = self.strict_message_replacement.to_string();
        entries.push(format!("strictMessageReplacement={displayed}"));
        let displayed = format!(
            "[{}]",
            self.strip_name_prefixes
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("stripNamePrefixes={displayed}"));
        let displayed = format!(
            "[{}]",
            self.strip_name_suffixes
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("stripNameSuffixes={displayed}"));
        let displayed = format!(
            "[{}]",
            self.strip_types
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        entries.push(format!("stripTypes={displayed}"));
        let displayed = self.summary_detail_level.to_string();
        entries.push(format!("summaryDetailLevel={displayed}"));
        if let Some(value) = &self.synthetic_block_end_marker {
            let displayed = value.to_string();
            entries.push(format!("syntheticBlockEndMarker={displayed}"));
        }
        if let Some(value) = &self.synthetic_block_start_marker {
            let displayed = value.to_string();
            entries.push(format!("syntheticBlockStartMarker={displayed}"));
        }
        if let Some(value) = &self.tc_project_id {
            let displayed = value.to_string();
            entries.push(format!("tcProjectId={displayed}"));
        }
        let displayed = self.tracer.to_string();
        entries.push(format!("tracer={displayed}"));
        let displayed = self.trusted_strings.to_string();
        entries.push(format!("trustedStrings={displayed}"));
        let displayed = self.tweak_processing.to_string();
        entries.push(format!("tweakProcessing={displayed}"));
        if let Some(value) = &self.unused_imports_to_remove {
            let displayed = format!(
                "[{}]",
                value
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            entries.push(format!("unusedImportsToRemove={displayed}"));
        }
        let displayed = self.use_types_for_local_optimization.to_string();
        entries.push(format!("useTypesForLocalOptimization={displayed}"));
        let displayed = self.variable_renaming.to_string();
        entries.push(format!("variableRenaming={displayed}"));
        let displayed = self
            .warnings_guard
            .get_guards()
            .iter()
            .map(|guard| {
                if guard
                    .as_any()
                    .is::<crate::show_by_path_warnings_guard::ShowByPathWarningsGuard>()
                {
                    "com.google.javascript.jscomp.ShowByPathWarningsGuard".to_string()
                } else {
                    guard.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        entries.push(format!("warningsGuard={displayed}"));
        let displayed = self.wrap_goog_modules_for_whitespace_only.to_string();
        entries.push(format!("wrapGoogModulesForWhitespaceOnly={displayed}"));
        write!(f, "CompilerOptions{{{}}}", entries.join(", "))
    }
}

// Each Java enum singleton owns these constructor-initialized fields.
struct BrowserFeaturesetYearData {
    year: i32,
    feature_set: FeatureSet,
}
impl BrowserFeaturesetYearData {
    // port: CompilerOptions.BrowserFeaturesetYear#BrowserFeaturesetYear
    fn new(year: i32, feature_set: FeatureSet) -> Self {
        Self { year, feature_set }
    }
}
static BROWSER_FEATURESET_YEAR_DATA: std::sync::LazyLock<[BrowserFeaturesetYearData; 10]> =
    std::sync::LazyLock::new(|| {
        [
            BrowserFeaturesetYearData::new(2012, LanguageMode::ECMASCRIPT5_STRICT.to_feature_set()),
            BrowserFeaturesetYearData::new(2018, LanguageMode::ECMASCRIPT_2016.to_feature_set()),
            BrowserFeaturesetYearData::new(2019, FeatureSet::BROWSER_2019),
            BrowserFeaturesetYearData::new(2020, FeatureSet::BROWSER_2020),
            BrowserFeaturesetYearData::new(2021, FeatureSet::BROWSER_2021),
            BrowserFeaturesetYearData::new(2022, FeatureSet::BROWSER_2022),
            BrowserFeaturesetYearData::new(2023, FeatureSet::BROWSER_2023),
            BrowserFeaturesetYearData::new(2024, FeatureSet::BROWSER_2024),
            BrowserFeaturesetYearData::new(2025, FeatureSet::BROWSER_2025),
            BrowserFeaturesetYearData::new(2026, FeatureSet::BROWSER_2026),
        ]
    });

// Test support: raw fields for which Java has no direct getter.
impl CompilerOptions {
    #[doc(hidden)]
    // port: CompilerOptions#emitUseStrict (test field access)
    pub fn __field_emit_use_strict(&self) -> &Option<bool> {
        &self.emit_use_strict
    }
    #[doc(hidden)]
    // port: CompilerOptions#outputFeatureSet (test field access)
    pub fn __field_output_feature_set(&self) -> &Option<FeatureSet> {
        &self.output_feature_set
    }
    #[doc(hidden)]
    // port: CompilerOptions#languageOutIsDefaultStrict (test field access)
    pub fn __field_language_out_is_default_strict(&self) -> &Option<bool> {
        &self.language_out_is_default_strict
    }
    #[doc(hidden)]
    // port: CompilerOptions#incrementalCheckMode (test field access)
    pub fn __field_incremental_check_mode(&self) -> &IncrementalCheckMode {
        &self.incremental_check_mode
    }
    #[doc(hidden)]
    // port: CompilerOptions#printExterns (test field access)
    pub fn __field_print_externs(&self) -> &bool {
        &self.print_externs
    }
    #[doc(hidden)]
    // port: CompilerOptions#deadPropertyAssignmentElimination (test field access)
    pub fn __field_dead_property_assignment_elimination(&self) -> &Tri {
        &self.dead_property_assignment_elimination
    }
    #[doc(hidden)]
    // port: CompilerOptions#assumeGlobalScopeIsIsolated (test field access)
    pub fn __field_assume_global_scope_is_isolated(&self) -> &bool {
        &self.assume_global_scope_is_isolated
    }
    #[doc(hidden)]
    // port: CompilerOptions#replaceMessagesWithChromeI18n (test field access)
    pub fn __field_replace_messages_with_chrome_i18n(&self) -> &bool {
        &self.replace_messages_with_chrome_i18n
    }
    #[doc(hidden)]
    // port: CompilerOptions#customPasses (test field access)
    pub fn __field_custom_passes(
        &self,
    ) -> &Option<IndexMap<CustomPassExecutionTime, IndexSet<CompilerPassRef>>> {
        &self.custom_passes
    }
    #[doc(hidden)]
    // port: CompilerOptions#defineReplacements (test field access)
    pub fn __field_define_replacements(&self) -> &IndexMap<String, DefineValue> {
        &self.define_replacements
    }
    #[doc(hidden)]
    // port: CompilerOptions#serializeExtraDebugInfo (test field access)
    pub fn __field_serialize_extra_debug_info(&self) -> &bool {
        &self.serialize_extra_debug_info
    }
    #[doc(hidden)]
    // port: CompilerOptions#quoteKeywordProperties (test field access)
    pub fn __field_quote_keyword_properties(&self) -> &bool {
        &self.quote_keyword_properties
    }
    #[doc(hidden)]
    // port: CompilerOptions#shouldAlwaysGatherSourceMapInfo (test field access)
    pub fn __field_should_always_gather_source_map_info(&self) -> &bool {
        &self.should_always_gather_source_map_info
    }
    #[doc(hidden)]
    // port: CompilerOptions#protectHiddenSideEffects (test field access)
    pub fn __field_protect_hidden_side_effects(&self) -> &bool {
        &self.protect_hidden_side_effects
    }
    #[doc(hidden)]
    // port: CompilerOptions#isStrictModeInput (test field access)
    pub fn __field_is_strict_mode_input(&self) -> &Option<bool> {
        &self.is_strict_mode_input
    }
    #[doc(hidden)]
    // port: CompilerOptions#rewriteModulesBeforeTypechecking (test field access)
    pub fn __field_rewrite_modules_before_typechecking(&self) -> &bool {
        &self.rewrite_modules_before_typechecking
    }
}
impl CompilerOptions {
    const POLYMER_PROPERTY_RESERVED_FIRST_CHARS: &'static str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ$";
    const POLYMER_PROPERTY_RESERVED_NON_FIRST_CHARS: &'static [u16] = &[b'_' as u16, b'$' as u16];
    const ANGULAR_PROPERTY_RESERVED_FIRST_CHARS: &'static [u16] = &[b'$' as u16];
}

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_compiler_options_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct CompilerOptionsReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct CompilerOptionsReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl CompilerOptions {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> CompilerOptionsReplayFields<'_> {
                CompilerOptionsReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> CompilerOptionsReplayFieldsMut<'_> {
                CompilerOptionsReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_compiler_options_fields!(
    emit_use_strict: Option<bool>,
    language_in: LanguageMode,
    output_feature_set: Option<FeatureSet>,
    experimental_output_feature_set: Option<ExperimentalOutputFeatureSet>,
    language_out_is_default_strict: Option<bool>,
    environment: Environment,
    browser_featureset_year: Option<BrowserFeaturesetYear>,
    instrument_for_coverage_only: bool,
    typed_ast_output_file: Option<PathBuf>,
    merged_precompiled_libraries: bool,
    infer_consts: bool,
    assume_strict_this: bool,
    preserve_detailed_source_info: bool,
    preserve_non_jsdoc_comments: bool,
    continue_after_errors: bool,
    incremental_check_mode: IncrementalCheckMode,
    parse_js_doc_documentation: JsDocParsing,
    print_externs: bool,
    infer_types: bool,
    skip_non_transpilation_passes: bool,
    dev_mode: DevMode,
    check_determinism: bool,
    dependency_options: DependencyOptions,
    message_bundle: Option<Arc<dyn MessageBundle + Send + Sync>>,
    strict_message_replacement: bool,
    check_symbols: bool,
    check_suspicious_code: bool,
    check_types: bool,
    extra_annotation_names: Option<IndexSet<String>>,
    num_parallel_threads: i32,
    fold_constants: bool,
    dead_assignment_elimination: bool,
    dead_property_assignment_elimination: Tri,
    inline_constant_vars: bool,
    max_function_size_after_inlining: i32,
    assume_closures_only_capture_references: bool,
    inline_properties: bool,
    cross_chunk_code_motion: bool,
    cross_chunk_code_motion_no_stub_methods: bool,
    parent_chunk_can_see_symbols_declared_in_children: bool,
    coalesce_variable_names: bool,
    optimize_let_and_const: bool,
    assume_global_scope_is_isolated: bool,
    cross_chunk_method_motion: bool,
    inline_getters: bool,
    inline_variables: bool,
    inline_local_variables: bool,
    flow_sensitive_inline_variables: bool,
    smart_name_removal: bool,
    extract_prototype_member_declarations: ExtractPrototypeMemberDeclarationsMode,
    remove_unused_prototype_properties: bool,
    remove_unused_class_properties: bool,
    remove_unused_vars: bool,
    remove_unused_local_vars: bool,
    collapse_variable_declarations: bool,
    collapse_anonymous_functions: bool,
    alias_strings_mode: AliasStringsMode,
    output_js_string_usage: bool,
    convert_to_dotted_properties: bool,
    rewrite_function_expressions: bool,
    optimize_calls: bool,
    optimize_es_class_constructors: bool,
    use_types_for_local_optimization: bool,
    use_size_heuristic_to_stop_optimization_loop: bool,
    optimization_loop_max_iterations: i32,
    variable_renaming: VariableRenamingPolicy,
    property_renaming: PropertyRenamingPolicy,
    property_renaming_only_compilation_mode: bool,
    label_renaming: bool,
    reserve_raw_exports: bool,
    prefer_stable_names: bool,
    generate_pseudo_names: bool,
    rename_prefix: Option<String>,
    rename_prefix_namespace: Option<String>,
    rename_prefix_namespace_assume_cross_chunk_names: bool,
    optimize_local_access_for_global_symbol_namespace: OptimizeLocalAccess,
    collapse_properties_level: PropertyCollapseLevel,
    collapse_object_literals: bool,
    devirtualize_methods: bool,
    compute_function_side_effects: bool,
    disambiguate_properties: bool,
    ambiguate_properties: bool,
    input_source_maps: IndexMap<String, Arc<SourceMapInput>>,
    input_variable_map: Option<Arc<VariableMap>>,
    input_property_map: Option<Arc<VariableMap>>,
    export_test_functions: bool,
    name_generator: Arc<dyn NameGenerator + Send + Sync>,
    replace_messages_with_chrome_i18n: bool,
    tc_project_id: Option<String>,
    coding_convention: Option<Arc<dyn CodingConvention + Send + Sync>>,
    synthetic_block_start_marker: Option<String>,
    synthetic_block_end_marker: Option<String>,
    locale: Option<String>,
    do_late_localization: bool,
    mark_as_compiled: bool,
    closure_pass: bool,
    preserve_closure_primitives: bool,
    angular_pass: bool,
    polymer_pass: bool,
    chrome_pass: bool,
    j2cl_pass_mode: J2clPassMode,
    j2cl_minifier_enabled: bool,
    j2cl_minifier_pruning_manifest: Option<String>,
    remove_abstract_methods: bool,
    remove_closure_asserts: bool,
    remove_j2cl_asserts: bool,
    gather_css_names: bool,
    strip_types: IndexSet<String>,
    strip_name_suffixes: IndexSet<String>,
    strip_name_prefixes: IndexSet<String>,
    custom_passes: Option<IndexMap<CustomPassExecutionTime, IndexSet<CompilerPassRef>>>,
    define_replacements: IndexMap<String, DefineValue>,
    tweak_processing: TweakProcessing,
    rewrite_global_declarations_for_try_catch_wrapping: bool,
    checks_only: bool,
    output_js: OutputJs,
    generate_exports: bool,
    export_local_property_definitions: bool,
    css_renaming_map: Option<Arc<dyn CssRenamingMap + Send + Sync>>,
    css_renaming_skiplist: Option<IndexSet<String>>,
    replace_id_generators: bool,
    id_generators: IndexMap<String, Arc<dyn RenamingMap + Send + Sync>>,
    xid_hash_function: Option<Arc<dyn HashFunction + Send + Sync>>,
    chunk_id_hash_function: Option<Arc<dyn HashFunction + Send + Sync>>,
    id_generators_map_serialized: Option<String>,
    replace_strings_function_descriptions: Vec<String>,
    replace_strings_placeholder_token: String,
    properties_that_must_disambiguate: IndexSet<String>,
    validate_require_inlining_annotation: Tri,
    process_common_js_modules: bool,
    module_roots: Vec<String>,
    rewrite_polyfills: bool,
    isolate_polyfills: bool,
    inject_polyfills_newer_than: Option<LanguageMode>,
    es6_subclass_transpilation: Es6SubclassTranspilation,
    instrument_async_context: bool,
    force_library_injection: Vec<String>,
    runtime_library_mode: RuntimeLibraryMode,
    assume_forward_declared_for_missing_types: bool,
    unused_imports_to_remove: Option<IndexSet<String>>,
    preserve_type_annotations: bool,
    gents_mode: bool,
    pretty_print: bool,
    line_break: bool,
    print_input_delimiter: bool,
    input_delimiter: String,
    debug_log_directory: Option<PathBuf>,
    debug_log_filter: Option<String>,
    serialize_extra_debug_info: bool,
    state_compression_wrapper: Option<OutputStreamWrapper>,
    state_decompression_wrapper: Option<InputStreamWrapper>,
    quote_keyword_properties: bool,
    prefer_single_quotes: bool,
    trusted_strings: bool,
    print_source_after_each_pass: bool,
    files_to_print_after_each_pass_regex_list: Vec<String>,
    chunks_to_print_after_each_pass_regex_list: Vec<String>,
    qname_uses_to_print_after_each_pass_list: Vec<String>,
    tracer: TracerMode,
    tracer_output: Option<PathBuf>,
    colorize_error_output: bool,
    error_format: ErrorFormat,
    warnings_guard: Arc<ComposeWarningsGuard>,
    summary_detail_level: i32,
    line_length_threshold: i32,
    use_original_names_in_output: bool,
    extern_exports_path: Option<String>,
    extra_report_generators: Vec<Arc<std::sync::Mutex<dyn ErrorReportGenerator + Send>>>,
    source_map_output_path: Option<String>,
    should_always_gather_source_map_info: bool,
    source_map_detail_level: DetailLevel,
    source_map_format: Format,
    parse_inline_source_maps: bool,
    apply_input_source_maps: bool,
    resolve_source_map_annotations: bool,
    source_map_location_mappings: Vec<Arc<dyn LocationMapping + Send + Sync>>,
    source_map_include_sources_content: bool,
    output_charset: Option<Charset>,
    protect_hidden_side_effects: bool,
    assume_getters_are_pure: bool,
    assume_properties_are_statically_analyzable: bool,
    assume_static_inheritance_is_not_used: bool,
    error_handler: Option<Arc<std::sync::Mutex<dyn ErrorHandler + Send>>>,
    instrument_for_coverage_option: InstrumentOption,
    production_instrumentation_array_name: String,
    conformance_configs: Vec<ConformanceConfig>,
    conformance_reporting_mode: ConformanceReportingMode,
    conformance_remove_regex_from_path: Option<Pattern>,
    wrap_goog_modules_for_whitespace_only: bool,
    print_config: bool,
    is_strict_mode_input: Option<bool>,
    rewrite_modules_before_typechecking: bool,
    enable_module_rewriting: bool,
    module_resolution_mode: ResolutionMode,
    browser_resolver_prefix_replacements: IndexMap<String, String>,
    path_escaper: PathEscaper,
    package_json_entry_names: Vec<String>,
    allow_dynamic_import: bool,
    dynamic_import_alias: Option<String>,
    chunk_output_type: ChunkOutputType,
    unknown_defines_to_ignore: Vec<String>,
    inline_functions_level: Reach,
    enable_zones_define_name: Option<String>,
    zone_input_pattern: Option<Pattern>,
    es6_module_transpilation: Es6ModuleTranspilation,
);
