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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/AbstractCommandLineRunner.java.

// Generated from AbstractCommandLineRunner.CommandLineConfig declarations.
#[derive(Clone, Debug)]
pub struct CommandLineConfig {
    pub print_version: bool,
    pub print_tree: bool,
    pub print_tree_json: bool,
    pub print_ast: bool,
    pub jscomp_dev_mode: DevMode,
    pub logging_level: String,
    pub externs: Vec<String>,
    pub mixed_js_sources: Vec<FlagEntry<JsSourceType>>,
    pub default_to_stdin: bool,
    pub js_output_file: String,
    pub continue_saved_compilation_file_name: Option<String>,
    pub restored_compilation_stage: i32,
    pub save_after_compilation_stage: i32,
    pub typed_ast_list_input_filename: Option<String>,
    pub save_compilation_state_to_filename: Option<String>,
    pub chunk: Vec<String>,
    pub source_map_input_files: IndexMap<String, String>,
    pub parse_inline_source_maps: bool,
    pub expected_diagnostics: Vec<String>,
    pub variable_map_input_file: String,
    pub property_map_input_file: String,
    pub variable_map_output_file: String,
    pub create_name_map_files: bool,
    pub property_map_output_file: String,
    pub string_map_output_path: String,
    pub instrumentation_mapping_file: String,
    pub coding_convention: Arc<dyn CodingConvention>,
    pub summary_detail_level: i32,
    pub output_wrapper: String,
    pub chunk_wrapper: Vec<String>,
    pub chunk_output_path_prefix: String,
    pub chunk_output_files: Vec<String>,
    pub chunk_conformance_files: Vec<String>,
    pub create_source_map: String,
    pub source_map_detail_level: DetailLevel,
    pub source_map_format: Format,
    pub source_map_location_mappings: Vec<PrefixLocationMapping>,
    pub apply_input_source_maps: bool,
    pub warning_guards: Vec<FlagEntry<CheckLevel>>,
    pub define: Vec<String>,
    pub browser_featureset_year: i32,
    pub tweak_processing: TweakProcessing,
    pub charset: String,
    pub dependency_options: Option<DependencyOptions>,
    pub output_manifests: Vec<String>,
    pub output_chunk_dependencies: Option<String>,
    pub output_bundles: Vec<String>,
    pub skip_normal_outputs: bool,
    pub manifest_maps: Vec<String>,
    pub process_common_js_modules: bool,
    pub module_roots: Vec<String>,
    pub warnings_allow_file: String,
    pub hide_warnings_for: Vec<String>,
    pub angular_pass: bool,
    pub json_stream_mode: JsonStreamMode,
    pub error_format: ErrorFormatOption,
    pub json_warnings_file: String,
    pub include_trailing_newline: bool,
}
impl Default for CommandLineConfig {
    // port: AbstractCommandLineRunner.CommandLineConfig#CommandLineConfig
    fn default() -> Self {
        Self {
            print_version: false,
            print_tree: false,
            print_tree_json: false,
            print_ast: false,
            jscomp_dev_mode: DevMode::OFF,
            logging_level: "WARNING".into(),
            externs: Vec::new(),
            mixed_js_sources: Vec::new(),
            default_to_stdin: false,
            js_output_file: "".into(),
            continue_saved_compilation_file_name: None,
            restored_compilation_stage: -1,
            save_after_compilation_stage: -1,
            typed_ast_list_input_filename: None,
            save_compilation_state_to_filename: None,
            chunk: Vec::new(),
            source_map_input_files: IndexMap::<_, _>::default(),
            parse_inline_source_maps: false,
            expected_diagnostics: Vec::new(),
            variable_map_input_file: "".into(),
            property_map_input_file: "".into(),
            variable_map_output_file: "".into(),
            create_name_map_files: false,
            property_map_output_file: "".into(),
            string_map_output_path: "".into(),
            instrumentation_mapping_file: "".into(),
            coding_convention: CodingConventions::get_default(),
            summary_detail_level: 1,
            output_wrapper: "".into(),
            chunk_wrapper: Vec::new(),
            chunk_output_path_prefix: "".into(),
            chunk_output_files: Vec::new(),
            chunk_conformance_files: Vec::new(),
            create_source_map: "".into(),
            source_map_detail_level: DetailLevel::ALL,
            source_map_format: Format::DEFAULT,
            source_map_location_mappings: Vec::new(),
            apply_input_source_maps: false,
            warning_guards: Vec::new(),
            define: Vec::new(),
            browser_featureset_year: 0,
            tweak_processing: TweakProcessing::OFF,
            charset: "".into(),
            dependency_options: None,
            output_manifests: Vec::new(),
            output_chunk_dependencies: None,
            output_bundles: Vec::new(),
            skip_normal_outputs: false,
            manifest_maps: Vec::new(),
            process_common_js_modules: false,
            module_roots: vec!["./".into()],
            warnings_allow_file: "".into(),
            hide_warnings_for: Vec::new(),
            angular_pass: false,
            json_stream_mode: JsonStreamMode::NONE,
            error_format: ErrorFormatOption::STANDARD,
            json_warnings_file: "".into(),
            include_trailing_newline: true,
        }
    }
}
impl CommandLineConfig {
    // port: AbstractCommandLineRunner.CommandLineConfig#setPrintVersion
    pub fn set_print_version(&mut self, value: bool) -> &mut Self {
        self.print_version = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setPrintTree
    pub fn set_print_tree(&mut self, value: bool) -> &mut Self {
        self.print_tree = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setPrintTreeJson
    pub fn set_print_tree_json(&mut self, value: bool) -> &mut Self {
        self.print_tree_json = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setPrintAst
    pub fn set_print_ast(&mut self, value: bool) -> &mut Self {
        self.print_ast = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setJscompDevMode
    pub fn set_jscomp_dev_mode(&mut self, value: DevMode) -> &mut Self {
        self.jscomp_dev_mode = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setLoggingLevel
    pub fn set_logging_level(&mut self, value: String) -> &mut Self {
        self.logging_level = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setExterns
    pub fn set_externs(&mut self, value: Vec<String>) -> &mut Self {
        self.externs = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setMixedJsSources
    pub fn set_mixed_js_sources(&mut self, value: Vec<FlagEntry<JsSourceType>>) -> &mut Self {
        self.mixed_js_sources = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setJsOutputFile
    pub fn set_js_output_file(&mut self, value: String) -> &mut Self {
        self.js_output_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setTypedAstListInputFilename
    pub fn set_typed_ast_list_input_filename(&mut self, value: Option<String>) -> &mut Self {
        self.typed_ast_list_input_filename = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setChunk
    pub fn set_chunk(&mut self, value: Vec<String>) -> &mut Self {
        self.chunk = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setSourceMapInputFiles
    pub fn set_source_map_input_files(&mut self, value: IndexMap<String, String>) -> &mut Self {
        self.source_map_input_files = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setParseInlineSourceMaps
    pub fn set_parse_inline_source_maps(&mut self, value: bool) -> &mut Self {
        self.parse_inline_source_maps = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setExpectedDiagnostics
    pub fn set_expected_diagnostics(&mut self, value: Vec<String>) -> &mut Self {
        self.expected_diagnostics = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setVariableMapInputFile
    pub fn set_variable_map_input_file(&mut self, value: String) -> &mut Self {
        self.variable_map_input_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setPropertyMapInputFile
    pub fn set_property_map_input_file(&mut self, value: String) -> &mut Self {
        self.property_map_input_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setVariableMapOutputFile
    pub fn set_variable_map_output_file(&mut self, value: String) -> &mut Self {
        self.variable_map_output_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setCreateNameMapFiles
    pub fn set_create_name_map_files(&mut self, value: bool) -> &mut Self {
        self.create_name_map_files = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setPropertyMapOutputFile
    pub fn set_property_map_output_file(&mut self, value: String) -> &mut Self {
        self.property_map_output_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setStringMapOutputFile
    pub fn set_string_map_output_file(&mut self, value: String) -> &mut Self {
        self.string_map_output_path = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setInstrumentationMappingFile
    pub fn set_instrumentation_mapping_file(&mut self, value: String) -> &mut Self {
        self.instrumentation_mapping_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setCodingConvention
    pub fn set_coding_convention(&mut self, value: Arc<dyn CodingConvention>) -> &mut Self {
        self.coding_convention = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setSummaryDetailLevel
    pub fn set_summary_detail_level(&mut self, value: i32) -> &mut Self {
        self.summary_detail_level = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setOutputWrapper
    pub fn set_output_wrapper(&mut self, value: String) -> &mut Self {
        self.output_wrapper = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setChunkWrapper
    pub fn set_chunk_wrapper(&mut self, value: Vec<String>) -> &mut Self {
        self.chunk_wrapper = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setChunkOutputPathPrefix
    pub fn set_chunk_output_path_prefix(&mut self, value: String) -> &mut Self {
        self.chunk_output_path_prefix = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setChunkOutputFiles
    pub fn set_chunk_output_files(&mut self, value: Vec<String>) -> &mut Self {
        self.chunk_output_files = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setChunkConformanceFiles
    pub fn set_chunk_conformance_files(&mut self, value: Vec<String>) -> &mut Self {
        self.chunk_conformance_files = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setCreateSourceMap
    pub fn set_create_source_map(&mut self, value: String) -> &mut Self {
        self.create_source_map = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setSourceMapDetailLevel
    pub fn set_source_map_detail_level(&mut self, value: DetailLevel) -> &mut Self {
        self.source_map_detail_level = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setSourceMapFormat
    pub fn set_source_map_format(&mut self, value: Format) -> &mut Self {
        self.source_map_format = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setApplyInputSourceMaps
    pub fn set_apply_input_source_maps(&mut self, value: bool) -> &mut Self {
        self.apply_input_source_maps = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setWarningGuards
    pub fn set_warning_guards(&mut self, value: Vec<FlagEntry<CheckLevel>>) -> &mut Self {
        self.warning_guards = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setDefine
    pub fn set_define(&mut self, value: Vec<String>) -> &mut Self {
        self.define = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setBrowserFeaturesetYear
    pub fn set_browser_featureset_year(&mut self, value: i32) -> &mut Self {
        self.browser_featureset_year = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setTweakProcessing
    pub fn set_tweak_processing(&mut self, value: TweakProcessing) -> &mut Self {
        self.tweak_processing = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setCharset
    pub fn set_charset(&mut self, value: String) -> &mut Self {
        self.charset = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setDependencyOptions
    pub fn set_dependency_options(&mut self, value: Option<DependencyOptions>) -> &mut Self {
        self.dependency_options = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setOutputChunkDependencies
    pub fn set_output_chunk_dependencies(&mut self, value: Option<String>) -> &mut Self {
        self.output_chunk_dependencies = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setOutputBundle
    pub fn set_output_bundle(&mut self, value: Vec<String>) -> &mut Self {
        self.output_bundles = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setSkipNormalOutputs
    pub fn set_skip_normal_outputs(&mut self, value: bool) -> &mut Self {
        self.skip_normal_outputs = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setManifestMaps
    pub fn set_manifest_maps(&mut self, value: Vec<String>) -> &mut Self {
        self.manifest_maps = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setProcessCommonJSModules
    pub fn set_process_common_js_modules(&mut self, value: bool) -> &mut Self {
        self.process_common_js_modules = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setModuleRoots
    pub fn set_module_roots(&mut self, value: Vec<String>) -> &mut Self {
        self.module_roots = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setWarningsAllowlistFile
    pub fn set_warnings_allow_list_file(&mut self, value: String) -> &mut Self {
        self.warnings_allow_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setHideWarningsFor
    pub fn set_hide_warnings_for(&mut self, value: Vec<String>) -> &mut Self {
        self.hide_warnings_for = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setAngularPass
    pub fn set_angular_pass(&mut self, value: bool) -> &mut Self {
        self.angular_pass = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setJsonStreamMode
    pub fn set_json_stream_mode(&mut self, value: JsonStreamMode) -> &mut Self {
        self.json_stream_mode = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setErrorFormat
    pub fn set_error_format(&mut self, value: ErrorFormatOption) -> &mut Self {
        self.error_format = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setJsonWarningsFile
    pub fn set_json_warnings_file(&mut self, value: String) -> &mut Self {
        self.json_warnings_file = value;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setIncludeTrailingNewline
    pub fn set_include_trailing_newline(&mut self, value: bool) -> &mut Self {
        self.include_trailing_newline = value;
        self
    }
}
