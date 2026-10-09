/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractCommandLineRunner.java,
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/FlagUsageException.java.

use crate::java_io::{OutputWriter, PrintStream};
use closure_jscomp::check_level::CheckLevel;
use closure_jscomp::source_map::{DetailLevel, Format, PrefixLocationMapping};
use closure_jscomp::{
    coding_convention::CodingConvention,
    coding_conventions::CodingConventions,
    compiler_options::{DevMode, JsonStreamMode, TweakProcessing},
    dependency_options::DependencyOptions,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::java_lang::charset::Charset;
use closure_rhino::static_source_file::StaticSourceFile;
use std::io::{Read, Write};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagUsageException(pub String);
impl std::fmt::Display for FlagUsageException {
    // port: FlagUsageException#getMessage
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for FlagUsageException {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnerExceptionKind {
    FlagUsage,
    JavaException {
        class: String,
        frames: Vec<String>,
        cause: Option<Box<RunnerException>>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerException(pub String, pub RunnerExceptionKind);
impl From<FlagUsageException> for RunnerException {
    fn from(error: FlagUsageException) -> Self {
        Self(error.0, RunnerExceptionKind::FlagUsage)
    }
}
impl RunnerException {
    pub fn java_exception(class: &str, message: String, frames: Vec<String>) -> Self {
        Self(
            message,
            RunnerExceptionKind::JavaException {
                class: class.into(),
                frames,
                cause: None,
            },
        )
    }
    pub fn at_cli(self, class: &str, method: &str, line: u32) -> Self {
        self.at(&format!(
            "com.google.javascript.jscomp.{class}.{method}({class}.java:{line})"
        ))
    }
    pub fn at(mut self, frame: &str) -> Self {
        if let RunnerExceptionKind::JavaException { frames, cause, .. } = &mut self.1 {
            frames.push(frame.into());
            if let Some(cause) = cause {
                **cause = cause.clone().at(frame);
            }
        }
        self
    }
    pub fn caused_by(mut self, error: Self) -> Self {
        if let RunnerExceptionKind::JavaException { cause, .. } = &mut self.1 {
            *cause = Some(Box::new(error));
        }
        self
    }
}

// Throwable#printStackTrace and #printEnclosedStackTrace for RunnerException, ported from OpenJDK
// (GPL-2.0 with the Classpath exception), are in their own file.
#[path = "abstract_command_line_runner_jdk.rs"]
mod jdk;

impl std::fmt::Display for RunnerException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RunnerException {}
impl From<closure_rhino::java_lang::io_exception::IOException> for RunnerException {
    fn from(error: closure_rhino::java_lang::io_exception::IOException) -> Self {
        Self::java_exception("java.io.IOException", error.to_string(), vec![])
    }
}
impl From<std::fmt::Error> for RunnerException {
    fn from(error: std::fmt::Error) -> Self {
        // Writing to a String/Vec is infallible. An Appendable failure maps to IOException.
        Self::java_exception("java.io.IOException", error.to_string(), vec![])
    }
}
impl From<std::io::Error> for RunnerException {
    fn from(error: std::io::Error) -> Self {
        if let Some(java) = error
            .get_ref()
            .and_then(|e| e.downcast_ref::<RunnerException>())
        {
            return java.clone();
        }
        if let Some(flag) = error
            .get_ref()
            .and_then(|e| e.downcast_ref::<FlagUsageException>())
        {
            return flag.clone().into();
        }
        Self::java_exception(
            "java.io.IOException",
            crate::java_io::error_message(&error),
            vec![],
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ErrorFormatOption {
    STANDARD,
    JSON,
}
impl ErrorFormatOption {
    // port: Enum#valueOf
    pub fn value_of(value: &str) -> Option<Self> {
        match value {
            "STANDARD" => Some(Self::STANDARD),
            "JSON" => Some(Self::JSON),
            _ => None,
        }
    }
}
impl std::fmt::Display for ErrorFormatOption {
    // port: Enum#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum JsSourceType {
    EXTERN,
    JS,
    JS_ZIP,
    WEAKDEP,
    IJS,
}
impl JsSourceType {
    // port: AbstractCommandLineRunner.JsSourceType#JsSourceType
    pub fn flag_name(self) -> &'static str {
        match self {
            Self::EXTERN => "extern",
            Self::JS => "js",
            Self::JS_ZIP => "jszip",
            Self::WEAKDEP => "weakdep",
            Self::IJS => "ijs",
        }
    }
}
impl std::fmt::Display for JsSourceType {
    // port: Enum#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Clone, Debug)]
pub struct FlagEntry<T> {
    pub flag: T,
    pub value: String,
}
impl<T> FlagEntry<T> {
    // port: AbstractCommandLineRunner.FlagEntry#FlagEntry
    pub fn new(flag: T, value: String) -> Self {
        Self { flag, value }
    }
    // port: AbstractCommandLineRunner.FlagEntry#getFlag
    pub fn get_flag(&self) -> &T {
        &self.flag
    }
    // port: AbstractCommandLineRunner.FlagEntry#getValue
    pub fn get_value(&self) -> &str {
        &self.value
    }
}
impl<T: PartialEq> PartialEq for FlagEntry<T> {
    // port: AbstractCommandLineRunner.FlagEntry#equals
    fn eq(&self, other: &Self) -> bool {
        other.flag == self.flag && other.value == self.value
    }
}
impl<T: Eq> Eq for FlagEntry<T> {}
impl<T: closure_rhino::java_lang::JavaHashCode> FlagEntry<T> {
    // port: AbstractCommandLineRunner.FlagEntry#hashCode
    pub fn hash_code(&self) -> i32 {
        use closure_rhino::java_lang::JavaHashCode;
        self.flag.hash_code().wrapping_add(self.value.hash_code())
    }
}
impl<T: closure_rhino::java_lang::JavaHashCode> std::hash::Hash for FlagEntry<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash_code());
    }
}
impl<T: std::fmt::Display> std::fmt::Display for FlagEntry<T> {
    // port: AbstractCommandLineRunner.FlagEntry#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.flag, self.value)
    }
}
include!("command_line_config.rs");
impl CommandLineConfig {
    // port: AbstractCommandLineRunner.CommandLineConfig#setDefaultToStdin
    pub fn set_default_to_stdin(&mut self) -> &mut Self {
        self.default_to_stdin = true;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setOutputManifest
    pub fn set_output_manifest(&mut self, value: Vec<String>) -> &mut Self {
        self.output_manifests = value.into_iter().filter(|s| !s.is_empty()).collect();
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setContinueSavedCompilationFileName
    pub fn set_continue_saved_compilation_file_name(
        &mut self,
        file_name: Option<String>,
        stage: i32,
    ) -> &mut Self {
        if file_name.is_some() {
            assert!(stage > 0 && stage < 3, "invalid compilation stage: {stage}");
            self.continue_saved_compilation_file_name = file_name;
            self.restored_compilation_stage = stage;
        }
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#setSaveCompilationStateToFilename
    pub fn set_save_compilation_state_to_filename(
        &mut self,
        file_name: Option<String>,
        stage: i32,
    ) -> &mut Self {
        self.save_compilation_state_to_filename = file_name;
        self.save_after_compilation_stage = stage;
        self
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#getSaveCompilationStateToFilename
    pub fn get_save_compilation_state_to_filename(&self) -> Option<&str> {
        self.save_compilation_state_to_filename.as_deref()
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#getContinueSavedCompilationFileName
    pub fn get_continue_saved_compilation_file_name(&self) -> Option<&str> {
        self.continue_saved_compilation_file_name.as_deref()
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldSaveAfterStage1
    pub fn should_save_after_stage1(&self) -> bool {
        if self.save_compilation_state_to_filename.is_some()
            && self.save_after_compilation_stage == 1
        {
            assert!(
                self.restored_compilation_stage < 0,
                "cannot perform stage 1 after restoring from stage {}",
                self.restored_compilation_stage
            );
            assert!(
                self.continue_saved_compilation_file_name.is_none(),
                "cannot restore a saved compilation and also save after stage 1"
            );
            true
        } else {
            false
        }
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldContinueCompilation
    pub fn should_continue_compilation(&self) -> bool {
        if self.continue_saved_compilation_file_name.is_some() {
            assert!(
                self.restored_compilation_stage > 0 && self.restored_compilation_stage < 3,
                "invalid restored compilation stage: {}",
                self.restored_compilation_stage
            );
            true
        } else {
            false
        }
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldRestoreAndPerformStages2And3
    pub fn should_restore_and_perform_stages2_and3(&self) -> bool {
        self.should_continue_compilation()
            && self.restored_compilation_stage == 1
            && self.save_after_compilation_stage < 0
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldRestoreTypedAstsPerformStage2AndSave
    pub fn should_restore_typed_asts_perform_stage2_and_save(&self) -> bool {
        self.typed_ast_list_input_filename.is_some() && self.save_after_compilation_stage == 2
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldRestoreTypedAstsPerformStages2And3
    pub fn should_restore_typed_asts_perform_stages2_and3(&self) -> bool {
        self.typed_ast_list_input_filename.is_some() && self.save_after_compilation_stage == -1
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldRestoreAndPerformStage2AndSave
    pub fn should_restore_and_perform_stage2_and_save(&self) -> bool {
        self.should_continue_compilation()
            && self.restored_compilation_stage == 1
            && self.save_after_compilation_stage == 2
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldRestoreAndPerformStage3
    pub fn should_restore_and_perform_stage3(&self) -> bool {
        if self.should_continue_compilation() && self.restored_compilation_stage == 2 {
            assert!(
                self.save_after_compilation_stage < 0,
                "request to save after stage {} is invalid when restoring from stage 2",
                self.save_after_compilation_stage
            );
            true
        } else {
            false
        }
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldDoLateLocalization
    pub fn should_do_late_localization(&self) -> bool {
        self.should_restore_and_perform_stage2_and_save()
            || self.should_restore_and_perform_stage3()
            || self.should_restore_typed_asts_perform_stage2_and_save()
    }
    // port: AbstractCommandLineRunner.CommandLineConfig#shouldAlwaysGatherSourceMapInfo
    pub fn should_always_gather_source_map_info(&self) -> bool {
        self.should_save_after_stage1()
            || self.should_restore_and_perform_stage2_and_save()
            || self.should_restore_typed_asts_perform_stage2_and_save()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsChunkSpec {
    pub name: String,
    pub num_inputs: i32,
    pub deps: Vec<String>,
    pub num_js_files: i32,
}
impl JsChunkSpec {
    // port: AbstractCommandLineRunner.JsChunkSpec#JsChunkSpec
    pub fn new(name: String, num_inputs: i32, deps: Vec<String>) -> Self {
        Self {
            name,
            num_inputs,
            deps,
            num_js_files: num_inputs,
        }
    }
    // port: AbstractCommandLineRunner.JsChunkSpec#create
    pub fn create(spec: &str, first: bool) -> Result<Self, FlagUsageException> {
        let parts = java_split(spec, ':');
        if parts.len() < 2 || parts.len() > 4 {
            return Err(FlagUsageException(format!(
                "Expected 2-4 colon-delimited parts in chunk spec: {spec}"
            )));
        }
        let name = parts[0].to_string();
        let deps = if parts.len() > 2 && !parts[2].is_empty() {
            java_split(parts[2], ',')
                .into_iter()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };
        let num_inputs = crate::args4j::spi::int_option_handler::parse(parts[1]).unwrap_or(-1);
        if num_inputs < 0 {
            if parts.len() == 2 && parts[1] == "auto" {
                if !first {
                    return Err(FlagUsageException(format!(
                        "Invalid JS file count '{}' for chunk: {name}. Only the first chunk may specify a size of 'auto' and it must have no dependencies.",
                        parts[1]
                    )));
                }
            } else {
                return Err(FlagUsageException(format!(
                    "Invalid JS file count '{}' for chunk: {name}",
                    parts[1]
                )));
            }
        }
        Ok(Self::new(name, num_inputs, deps))
    }
    // port: AbstractCommandLineRunner.JsChunkSpec#getName
    pub fn get_name(&self) -> &str {
        &self.name
    }
    // port: AbstractCommandLineRunner.JsChunkSpec#getNumInputs
    pub fn get_num_inputs(&self) -> i32 {
        self.num_inputs
    }
    // port: AbstractCommandLineRunner.JsChunkSpec#getDeps
    pub fn get_deps(&self) -> &[String] {
        &self.deps
    }
    // port: AbstractCommandLineRunner.JsChunkSpec#getNumJsFiles
    pub fn get_num_js_files(&self) -> i32 {
        self.num_js_files
    }
}
// port: String#split (zero limit)
pub(crate) fn java_split(value: &str, delimiter: char) -> Vec<&str> {
    let mut parts: Vec<&str> = value.split(delimiter).collect();
    if !value.is_empty() {
        while parts.last() == Some(&"") {
            parts.pop();
        }
    }
    parts
}

// port: AbstractCommandLineRunner#OUTPUT_SAME_AS_INPUT_ERROR
pub static OUTPUT_SAME_AS_INPUT_ERROR: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error(
        "JSC_OUTPUT_SAME_AS_INPUT_ERROR",
        "Bad output file (already listed as input file): {0}",
    );
// port: AbstractCommandLineRunner#COULD_NOT_SERIALIZE_AST
pub static COULD_NOT_SERIALIZE_AST: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error(
        "JSC_COULD_NOT_SERIALIZE_AST",
        "Could not serialize ast to: {0}",
    );
// port: AbstractCommandLineRunner#COULD_NOT_DESERIALIZE_AST
pub static COULD_NOT_DESERIALIZE_AST: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error(
        "JSC_COULD_NOT_DESERIALIZE_AST",
        "Could not deserialize ast from: {0}",
    );
// port: AbstractCommandLineRunner#NO_TREE_GENERATED_ERROR
pub static NO_TREE_GENERATED_ERROR: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error(
        "JSC_NO_TREE_GENERATED_ERROR",
        "Code contains errors. No tree was generated.",
    );
// port: AbstractCommandLineRunner#INVALID_CHUNK_SOURCEMAP_PATTERN
pub static INVALID_CHUNK_SOURCEMAP_PATTERN: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error(
        "JSC_INVALID_CHUNK_SOURCEMAP_PATTERN",
        "When using --chunk or --module flags, the --create_source_map flag must contain %outname% in the value.",
    );
// port: AbstractCommandLineRunner#EXPECTED_DIAGNOSTIC_NOT_FOUND
pub static EXPECTED_DIAGNOSTIC_NOT_FOUND: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error(
        "JSC_EXPECTED_DIAGNOSTIC_NOT_FOUND",
        "Expected diagnostic not found: {0}",
    );
// port: AbstractCommandLineRunner#AMBIGUOUS_EXPECTATION
pub static AMBIGUOUS_EXPECTATION: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error(
        "JSC_AMBIGUOUS_EXPECTATION",
        "Multiple expected diagnostics matched the error: \"{0}\". Matches: \"{1}\"",
    );
pub const OUTPUT_MARKER: &str = "%output%";
pub const WAITING_FOR_INPUT_WARNING: &str = "The compiler is waiting for input via stdin.";

pub trait RunnerMethods<A, B> {
    fn create_compiler(&self) -> A;
    fn create_options(&self) -> Result<B, RunnerException>;
    fn get_version_text(&self) -> String;
    fn prep_for_bundle_and_append_to(
        &mut self,
        out: &mut dyn Write,
        input: &closure_jscomp::compiler_input::CompilerInput,
        content: closure_rhino::js_string::JsString,
    ) -> Result<(), RunnerException>;
    fn append_runtime_to(&mut self, out: &mut dyn Write) -> Result<(), RunnerException>;
    fn add_allow_list_warnings_guard(
        options: &mut closure_jscomp::compiler_options::CompilerOptions,
        file: &str,
    );
}
pub struct AbstractCommandLineRunner<A, B> {
    pub config: CommandLineConfig,
    pub input: Box<dyn Read>,
    pub default_js_output: PrintStream,
    pub err: PrintStream,
    pub compiler: Option<A>,
    pub input_charset: Option<Charset>,
    pub output_charset2: Option<Charset>,
    pub legacy_output_charset: Option<Charset>,
    pub test_mode: bool,
    pub externs_supplier_for_testing: Option<Box<dyn FnMut() -> Vec<SourceFile>>>,
    pub inputs_supplier_for_testing: Option<Box<dyn FnMut() -> Vec<SourceFile>>>,
    pub chunks_supplier_for_testing: Option<Box<dyn FnMut() -> Vec<JSChunk>>>,
    pub exit_code_receiver: Option<Box<dyn FnMut(i32)>>,
    pub root_relative_paths_map: Option<IndexMap<String, String>>,
    pub parsed_chunk_wrappers: Option<IndexMap<String, String>>,
    pub parsed_chunk_output_files: Option<IndexMap<String, String>>,
    pub parsed_chunk_conformance_files: Option<IndexMap<String, String>>,
    pub files_to_stream_out: Vec<JsonFileSpec>,
    marker: std::marker::PhantomData<B>,
}
impl<A, B> AbstractCommandLineRunner<A, B> {
    // port: AbstractCommandLineRunner#AbstractCommandLineRunner()
    pub fn new_system() -> Self {
        Self::new(
            Box::new(std::io::stdin()),
            Box::new(std::io::stdout()),
            Box::new(std::io::stderr()),
        )
    }
    // port: AbstractCommandLineRunner#AbstractCommandLineRunner(PrintStream, PrintStream)
    pub fn new_with_output_streams(out: Box<dyn Write>, err: Box<dyn Write>) -> Self {
        Self::new(Box::new(std::io::stdin()), out, err)
    }
    // port: AbstractCommandLineRunner#AbstractCommandLineRunner(InputStream, PrintStream, PrintStream)
    pub fn new(input: Box<dyn Read>, out: Box<dyn Write>, err: Box<dyn Write>) -> Self {
        Self {
            config: CommandLineConfig::default(),
            input,
            default_js_output: PrintStream::new(out),
            err: PrintStream::new(err),
            compiler: None,
            input_charset: None,
            output_charset2: None,
            legacy_output_charset: None,
            test_mode: false,
            externs_supplier_for_testing: None,
            inputs_supplier_for_testing: None,
            chunks_supplier_for_testing: None,
            exit_code_receiver: None,
            root_relative_paths_map: None,
            parsed_chunk_wrappers: None,
            parsed_chunk_output_files: None,
            parsed_chunk_conformance_files: None,
            files_to_stream_out: Vec::new(),
            marker: std::marker::PhantomData,
        }
    }
    // port: AbstractCommandLineRunner#enableTestMode
    pub fn enable_test_mode(
        &mut self,
        externs_supplier: Box<dyn FnMut() -> Vec<SourceFile>>,
        inputs_supplier: Option<Box<dyn FnMut() -> Vec<SourceFile>>>,
        chunks_supplier: Option<Box<dyn FnMut() -> Vec<JSChunk>>>,
        exit_code_receiver: Box<dyn FnMut(i32)>,
    ) {
        assert_ne!(inputs_supplier.is_none(), chunks_supplier.is_none());
        self.test_mode = true;
        self.externs_supplier_for_testing = Some(externs_supplier);
        self.inputs_supplier_for_testing = inputs_supplier;
        self.chunks_supplier_for_testing = chunks_supplier;
        self.exit_code_receiver = Some(exit_code_receiver);
    }
    // port: AbstractCommandLineRunner#setExitCodeReceiver
    pub fn set_exit_code_receiver(&mut self, receiver: Box<dyn FnMut(i32)>) {
        self.exit_code_receiver = Some(receiver);
    }
    // port: AbstractCommandLineRunner#isInTestMode
    pub fn is_in_test_mode(&self) -> bool {
        self.test_mode
    }
    // port: AbstractCommandLineRunner#isOutputInJson
    pub fn is_output_in_json(&self) -> bool {
        matches!(
            self.config.json_stream_mode,
            JsonStreamMode::OUT | JsonStreamMode::BOTH
        )
    }
    // port: AbstractCommandLineRunner#getCommandLineConfig
    pub fn get_command_line_config(&mut self) -> &mut CommandLineConfig {
        &mut self.config
    }
    // port: AbstractCommandLineRunner#getCompiler
    pub fn get_compiler(&self) -> Option<&A> {
        self.compiler.as_ref()
    }
    // port: AbstractCommandLineRunner#getErrorPrintStream
    pub fn get_error_print_stream(&mut self) -> &mut dyn Write {
        &mut self.err
    }
    // port: AbstractCommandLineRunner#parseChunkWrappers
    pub fn parse_chunk_wrappers(
        specs: &[String],
        chunks: &[String],
    ) -> Result<IndexMap<String, String>, FlagUsageException> {
        let mut wrappers: IndexMap<String, String> =
            chunks.iter().map(|c| (c.clone(), String::new())).collect();
        for spec in specs {
            let (name, wrapper) = spec.split_once(':').ok_or_else(|| {
                FlagUsageException(format!(
                    "Expected chunk wrapper to have <name>:<wrapper> format: {spec}"
                ))
            })?;
            if !wrappers.contains_key(name) {
                return Err(FlagUsageException(format!("Unknown chunk: '{name}'")));
            }
            let wrapper = wrapper.replace("%output%", "%s").replace("%n%", "\n");
            if !wrapper.contains("%s") {
                return Err(FlagUsageException(format!(
                    "No %s placeholder in chunk wrapper: '{wrapper}'"
                )));
            }
            wrappers.insert(name.into(), wrapper);
        }
        Ok(wrappers)
    }
    // port: AbstractCommandLineRunner#parseChunkOutputFiles
    pub fn parse_chunk_output_files(
        specs: &[String],
    ) -> Result<IndexMap<String, String>, FlagUsageException> {
        let mut outputs = IndexMap::<_, _>::default();
        for spec in specs {
            let (name, filename) = spec.split_once(':').ok_or_else(|| {
                FlagUsageException(format!(
                    "Expected chunk_output_file to have <name>:<output_file> format: {spec}"
                ))
            })?;
            if let Some(old) = outputs.insert(name.into(), filename.into()) {
                return Err(FlagUsageException(format!(
                    "Multiple entries with same key: {name}={filename} and {name}={old}"
                )));
            }
        }
        Ok(outputs)
    }
    // port: AbstractCommandLineRunner#getChunkOutputFileName
    pub fn get_chunk_output_file_name(&mut self, name: &str) -> Result<String, FlagUsageException> {
        if self.parsed_chunk_output_files.is_none() {
            self.parsed_chunk_output_files = Some(Self::parse_chunk_output_files(
                &self.config.chunk_output_files,
            )?);
        }
        Ok(format!(
            "{}{}",
            self.config.chunk_output_path_prefix,
            self.parsed_chunk_output_files
                .as_ref()
                .unwrap()
                .get(name)
                .cloned()
                .unwrap_or_else(|| format!("{name}.js"))
        ))
    }
    // port: AbstractCommandLineRunner#getChunkConformanceFileName
    pub fn get_chunk_conformance_file_name(
        &mut self,
        name: &str,
    ) -> Result<Option<String>, FlagUsageException> {
        if self.parsed_chunk_conformance_files.is_none() {
            self.parsed_chunk_conformance_files = Some(Self::parse_chunk_output_files(
                &self.config.chunk_conformance_files,
            )?);
        }
        Ok(self
            .parsed_chunk_conformance_files
            .as_ref()
            .unwrap()
            .get(name)
            .cloned())
    }
    // port: AbstractCommandLineRunner#expandCommandLinePath
    pub fn expand_command_line_path(
        &mut self,
        path: &str,
        chunk: Option<&str>,
    ) -> Result<String, FlagUsageException> {
        let sub = if let Some(chunk) = chunk {
            self.get_chunk_output_file_name(chunk)?
        } else if !self.config.chunk.is_empty() {
            self.config.chunk_output_path_prefix.clone()
        } else {
            self.config.js_output_file.clone()
        };
        Ok(path.replace("%outname%", &sub))
    }
    // port: AbstractCommandLineRunner#constructRootRelativePathsMap
    pub fn construct_root_relative_paths_map(&self) -> IndexMap<String, String> {
        let mut result = IndexMap::<_, _>::default();
        for value in &self.config.manifest_maps {
            let colon = value.find(':').expect("missing colon");
            assert!(colon > 0);
            let (exec, relative) = (&value[..colon], &value[colon + 1..]);
            assert!(!relative.contains(':'));
            result.insert(exec.into(), relative.into());
        }
        result
    }
    // port: AbstractCommandLineRunner#getInputCharset
    pub fn get_input_charset(&self) -> Result<Charset, FlagUsageException> {
        self.charset_or(Charset::UTF_8)
    }
    // port: AbstractCommandLineRunner#getLegacyOutputCharset
    pub fn get_legacy_output_charset(&self) -> Result<Charset, FlagUsageException> {
        self.charset_or(Charset::US_ASCII)
    }
    // port: AbstractCommandLineRunner#getOutputCharset2
    pub fn get_output_charset2(&self) -> Result<Charset, FlagUsageException> {
        self.charset_or(Charset::UTF_8)
    }
    fn charset_or(&self, default: Charset) -> Result<Charset, FlagUsageException> {
        if self.config.charset.is_empty() {
            return Ok(default);
        }
        // Charset.forName is the shared JDK decoder's alias table. Do not invent an encoding.
        if supported_charset(&self.config.charset) {
            Ok(Charset::for_name(&self.config.charset))
        } else {
            Err(FlagUsageException(format!(
                "{} is not a valid charset name.",
                self.config.charset
            )))
        }
    }
    // port: AbstractCommandLineRunner#parseJsonFilesFromInputStream
    pub fn parse_json_files_from_input_stream(
        &mut self,
    ) -> Result<Vec<Option<JsonFileSpec>>, crate::gson::json_reader::JsonError> {
        let mut bytes = Vec::new();
        self.input.read_to_end(&mut bytes).map_err(|e| {
            crate::gson::json_reader::JsonError(
                e.to_string(),
                "java.io.IOException".into(),
                Box::new(RunnerException::from(e)),
            )
        })?;
        let text = self
            .input_charset
            .unwrap_or(Charset::UTF_8)
            .decode(&bytes, true)
            .unwrap()
            .to_string_lossy();
        crate::gson::json_reader::parse_json_files(&text)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonFileSpec {
    pub src: Option<closure_rhino::js_string::JsString>,
    pub path: Option<closure_rhino::js_string::JsString>,
    pub source_map: Option<closure_rhino::js_string::JsString>,
    pub webpack_id: Option<closure_rhino::js_string::JsString>,
}
impl Default for JsonFileSpec {
    // port: AbstractCommandLineRunner.JsonFileSpec#JsonFileSpec()
    fn default() -> Self {
        Self::new(None, None, None, None)
    }
}
impl JsonFileSpec {
    // port: AbstractCommandLineRunner.JsonFileSpec#JsonFileSpec(String, String)
    pub fn new_with_src_and_path(
        src: Option<closure_rhino::js_string::JsString>,
        path: Option<closure_rhino::js_string::JsString>,
    ) -> Self {
        Self::new(src, path, None, None)
    }
    // port: AbstractCommandLineRunner.JsonFileSpec#JsonFileSpec(String, String, String)
    pub fn new_with_source_map(
        src: Option<closure_rhino::js_string::JsString>,
        path: Option<closure_rhino::js_string::JsString>,
        source_map: Option<closure_rhino::js_string::JsString>,
    ) -> Self {
        Self::new(src, path, source_map, None)
    }
    // port: AbstractCommandLineRunner.JsonFileSpec#JsonFileSpec(String, String, String, String)
    pub fn new(
        src: Option<closure_rhino::js_string::JsString>,
        path: Option<closure_rhino::js_string::JsString>,
        source_map: Option<closure_rhino::js_string::JsString>,
        webpack_id: Option<closure_rhino::js_string::JsString>,
    ) -> Self {
        Self {
            src,
            path,
            source_map,
            webpack_id,
        }
    }
    // port: AbstractCommandLineRunner.JsonFileSpec#getSrc
    pub fn get_src(&self) -> Option<&closure_rhino::js_string::JsString> {
        self.src.as_ref()
    }
    // port: AbstractCommandLineRunner.JsonFileSpec#getPath
    pub fn get_path(&self) -> Option<&closure_rhino::js_string::JsString> {
        self.path.as_ref()
    }
    // port: AbstractCommandLineRunner.JsonFileSpec#getSourceMap
    pub fn get_source_map(&self) -> Option<&closure_rhino::js_string::JsString> {
        self.source_map.as_ref()
    }
    // port: AbstractCommandLineRunner.JsonFileSpec#getWebpackId
    pub fn get_webpack_id(&self) -> Option<&closure_rhino::js_string::JsString> {
        self.webpack_id.as_ref()
    }
    // port: AbstractCommandLineRunner.JsonFileSpec#setSourceMap
    pub fn set_source_map(&mut self, map: closure_rhino::js_string::JsString) {
        self.source_map = Some(map);
    }
}
pub struct SystemExitCodeReceiver;
impl SystemExitCodeReceiver {
    pub const INSTANCE: Self = Self::new();
    // port: AbstractCommandLineRunner.SystemExitCodeReceiver#SystemExitCodeReceiver
    const fn new() -> Self {
        Self
    }
    // port: AbstractCommandLineRunner.SystemExitCodeReceiver#apply
    pub fn apply(code: i32) -> i32 {
        let byte = code as u8;
        if byte == 0 && code != 0 {
            255
        } else {
            i32::from(byte)
        }
    }
}

impl CommandLineConfig {
    // port: AbstractCommandLineRunner.CommandLineConfig#setSourceMapLocationMappings
    pub fn set_source_map_location_mappings(
        &mut self,
        value: Vec<PrefixLocationMapping>,
    ) -> &mut Self {
        self.source_map_location_mappings = value;
        self
    }
}

fn supported_charset(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "utf-8"
            | "utf8"
            | "unicode-1-1-utf-8"
            | "us-ascii"
            | "iso-ir-6"
            | "ansi_x3.4-1986"
            | "iso_646.irv:1991"
            | "ascii"
            | "iso646-us"
            | "us"
            | "ibm367"
            | "cp367"
            | "csascii"
            | "646"
            | "iso_646.irv:1983"
            | "ansi_x3.4-1968"
            | "ascii7"
            | "iso-8859-1"
            | "iso-ir-100"
            | "iso_8859-1"
            | "latin1"
            | "l1"
            | "ibm819"
            | "cp819"
            | "csisolatin1"
            | "819"
            | "ibm-819"
            | "iso8859_1"
            | "iso_8859-1:1987"
            | "iso_8859_1"
            | "8859_1"
            | "iso8859-1"
            | "utf-16"
            | "utf_16"
            | "utf16"
            | "unicode"
            | "unicodebig"
            | "utf-16be"
            | "utf_16be"
            | "iso-10646-ucs-2"
            | "x-utf-16be"
            | "unicodebigunmarked"
            | "utf-16le"
            | "utf_16le"
            | "x-utf-16le"
            | "unicodelittleunmarked"
    )
}
use closure_jscomp::compiler_options::CompilerOptions;
use closure_jscomp::{compiler_input::CompilerInput, js_chunk::JSChunk, source_file::SourceFile};
use closure_rhino::static_source_file::SourceKind;
impl<A, B> AbstractCommandLineRunner<A, B> {
    // port: AbstractCommandLineRunner#setWarningGuardOptions
    pub fn set_warning_guard_options(
        options: &mut CompilerOptions,
        guards: &[FlagEntry<CheckLevel>],
    ) -> Result<(), FlagUsageException> {
        use closure_jscomp::diagnostic_groups::{DiagnosticGroups, WILDCARD_EXCLUDED_GROUPS};
        let groups = DiagnosticGroups::get_registered_groups();
        for entry in guards {
            if entry.value == "*" {
                for (name, group) in &groups {
                    if !WILDCARD_EXCLUDED_GROUPS.contains(&name.as_str()) {
                        options.set_warning_level(group.clone(), entry.flag);
                    }
                }
            } else if let Some(group) = groups.get(&entry.value) {
                options.set_warning_level(group.clone(), entry.flag);
            } else {
                return Err(FlagUsageException(format!(
                    "Unknown diagnostic group: '{}'",
                    entry.value
                )));
            }
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#setRunOptions
    pub fn set_run_options(&mut self, options: &mut CompilerOptions) -> Result<(), RunnerException>
    where
        A: CliCompiler,
    {
        if self.config.should_save_after_stage1() || self.config.should_continue_compilation() {
            if options.is_checks_only() {
                return Err(FlagUsageException(
                    "checks_only mode is incompatible with multi-stage compilation".into(),
                )
                .into());
            }
            if options.get_extern_exports_path().is_some() {
                return Err(FlagUsageException(
                    "generating externs from exports is incompatible with multi-stage compilation"
                        .into(),
                )
                .into());
            }
        }
        Self::set_warning_guard_options(options, &self.config.warning_guards)?;
        if !self.config.warnings_allow_file.is_empty() {
            options.add_warnings_guard(Arc::new(
                closure_jscomp::allow_list_warnings_guard::AllowListWarningsGuard::from_file(
                    &self.config.warnings_allow_file,
                ),
            ));
        }
        if !self.config.hide_warnings_for.is_empty() {
            use closure_jscomp::show_by_path_warnings_guard::{ShowByPathWarningsGuard, ShowType};
            options.add_warnings_guard(Arc::new(
                ShowByPathWarningsGuard::new_with_paths_and_show_type(
                    &self
                        .config
                        .hide_warnings_for
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    ShowType::EXCLUDE,
                ),
            ));
        }
        if self.config.browser_featureset_year != 0 {
            if !closure_jscomp::compiler_options::BrowserFeaturesetYear::YEAR_MAP
                .iter()
                .any(|(year, _)| *year == self.config.browser_featureset_year)
            {
                return Err(FlagUsageException(format!(
                    "Illegal browser_featureset_year={}. We support values 2012, or 2018..2026 only",
                    self.config.browser_featureset_year
                )).into());
            }
            options.set_browser_featureset_year(self.config.browser_featureset_year);
        }
        Self::create_define_replacements(&self.config.define, options)
            .map_err(FlagUsageException)?;
        options.set_tweak_processing(self.config.tweak_processing);
        if let Some(dependency) = &self.config.dependency_options {
            options.set_dependency_options(dependency.clone());
        }
        options.set_dev_mode(self.config.jscomp_dev_mode);
        options.set_coding_convention(self.config.coding_convention.clone());
        options.set_summary_detail_level(self.config.summary_detail_level);
        options.set_trusted_strings(true);
        self.legacy_output_charset = Some(self.get_legacy_output_charset()?);
        options.set_output_charset(self.legacy_output_charset.unwrap_or(Charset::UTF_8));
        self.output_charset2 = Some(self.get_output_charset2()?);
        self.input_charset = Some(self.get_input_charset()?);
        if !self.config.js_output_file.is_empty() && self.config.skip_normal_outputs {
            return Err(FlagUsageException(
                "skip_normal_outputs and js_output_file cannot be used together.".into(),
            )
            .into());
        }
        if self.config.skip_normal_outputs && self.config.print_ast {
            return Err(FlagUsageException(
                "skip_normal_outputs and print_ast cannot be used together.".into(),
            )
            .into());
        }
        if self.config.skip_normal_outputs && self.config.print_tree {
            return Err(FlagUsageException(
                "skip_normal_outputs and print_tree cannot be used together.".into(),
            )
            .into());
        }
        if self.config.skip_normal_outputs && self.config.print_tree_json {
            return Err(FlagUsageException(
                "skip_normal_outputs and print_tree_json_path cannot be used together.".into(),
            )
            .into());
        }
        if !self.config.create_source_map.is_empty() {
            options.set_source_map_output_path(self.config.create_source_map.clone());
        } else if self.is_output_in_json() {
            options.set_source_map_output_path("%outname%".into());
        }
        options.set_source_map_detail_level(self.config.source_map_detail_level);
        options.set_source_map_format(self.config.source_map_format);
        options.set_source_map_location_mappings(
            self.config
                .source_map_location_mappings
                .iter()
                .map(|m| {
                    Arc::new(m.clone())
                        as Arc<dyn closure_jscomp::source_map::LocationMapping + Send + Sync>
                })
                .collect(),
        );
        options.set_parse_inline_source_maps(self.config.parse_inline_source_maps);
        options.set_apply_input_source_maps(self.config.apply_input_source_maps);
        let maps = self
            .config
            .source_map_input_files
            .iter()
            .map(|(name, path)| {
                let file = SourceFile::builder()
                    .with_kind(SourceKind::NON_CODE)
                    .with_path(path)
                    .build();
                (
                    name.clone(),
                    Arc::new(closure_jscomp::source_map_input::SourceMapInput::new(
                        Arc::new(file),
                    )),
                )
            })
            .collect();
        options.set_input_source_maps(maps);
        if !self.config.variable_map_input_file.is_empty() {
            options.set_input_variable_map(Arc::new(
                closure_jscomp::variable_map::VariableMap::load(
                    &self.config.variable_map_input_file,
                )
                .map_err(|e| {
                    crate::java_io::variable_map_load_error(e, &self.config.variable_map_input_file)
                        .at_cli("AbstractCommandLineRunner", "setRunOptions", 423)
                })?,
            ));
        }
        if !self.config.property_map_input_file.is_empty() {
            options.set_input_property_map(Arc::new(
                closure_jscomp::variable_map::VariableMap::load(
                    &self.config.property_map_input_file,
                )
                .map_err(|e| {
                    crate::java_io::variable_map_load_error(e, &self.config.property_map_input_file)
                        .at_cli("AbstractCommandLineRunner", "setRunOptions", 427)
                })?,
            ));
        }
        for (files, flag) in [
            (&self.config.output_manifests, "output_manifest"),
            (&self.config.output_bundles, "output_bundle"),
        ] {
            let mut unique = closure_rhino::fx_hash::IndexSet::<_>::default();
            for filename in files {
                if !unique.insert(filename) {
                    return Err(FlagUsageException(format!(
                        "{flag} flags specify duplicate file names: {filename}"
                    ))
                    .into());
                }
            }
        }
        options.set_process_common_js_modules(self.config.process_common_js_modules);
        options.set_module_roots(self.config.module_roots.clone());
        options.set_angular_pass(self.config.angular_pass);
        if self.config.print_tree || self.config.print_tree_json {
            options.set_parse_js_doc_documentation(
                closure_parsing::config::JsDocParsing::INCLUDE_ALL_COMMENTS,
            );
        }
        if !self.config.json_warnings_file.is_empty() {
            let stream = std::fs::File::create(&self.config.json_warnings_file)
                .map_err(RunnerException::from)?;
            options.add_report_generator(Arc::new(std::sync::Mutex::new(
                closure_jscomp::json_error_report_generator::JsonErrorReportGenerator::new(
                    Box::new(stream),
                    self.compiler
                        .as_mut()
                        .unwrap()
                        .as_merged_compiler()
                        .get_source_excerpt_provider(),
                ),
            )));
        }
        if self.config.error_format == ErrorFormatOption::JSON {
            let compiler = self.compiler.as_mut().unwrap().as_merged_compiler();
            // getErrorPrintStream(): the compiler's output stream is the runner's error stream.
            let generator =
                closure_jscomp::json_error_report_generator::JsonErrorReportGenerator::new(
                    compiler
                        .get_out_stream_writer()
                        .expect("createCompiler connects the error print stream"),
                    compiler.get_source_excerpt_provider(),
                );
            let mut manager = closure_jscomp::sorting_error_manager::SortingErrorManager::new(
                vec![Box::new(generator)],
            );
            manager.set_deferred_reports(compiler.get_deferred_reports());
            self.compiler
                .as_mut()
                .unwrap()
                .as_merged_compiler()
                .set_error_manager(Box::new(manager));
        }
        if self.config.skip_normal_outputs {
            self.compiler
                .as_mut()
                .unwrap()
                .as_merged_compiler()
                .set_prefer_regex_parser(true);
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#createDefineReplacements
    pub fn create_define_replacements(
        definitions: &[String],
        options: &mut CompilerOptions,
    ) -> Result<(), String> {
        for replacement in definitions {
            let (name, value) = replacement.split_once('=').unwrap_or((replacement, "true"));
            if !name.is_empty() {
                if value == "true" || value == "false" {
                    options.set_define_to_boolean_literal(name, value == "true");
                    continue;
                } else if value.len() > 1
                    && ((value.starts_with('\'') && value.ends_with('\''))
                        || (value.starts_with('"') && value.ends_with('"')))
                {
                    let inner = &value[1..value.len() - 1];
                    if !inner.contains(value.chars().next().unwrap()) {
                        options.set_define_to_string_literal(name, inner);
                        continue;
                    }
                } else {
                    if let Some(number) = parse_java_double(value) {
                        options.set_define_to_double_literal(name, number);
                        continue;
                    }
                    if !value.is_empty() {
                        options.set_define_to_string_literal(name, value);
                        continue;
                    }
                }
            }
            return Err(format!("--define flag syntax invalid: {replacement}"));
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#createInputs(List<FlagEntry<JsSourceType>>, boolean, List<JsChunkSpec>)
    pub fn create_inputs_without_json(
        &mut self,
        files: &[FlagEntry<JsSourceType>],
        allow_stdin: bool,
        specs: &mut [JsChunkSpec],
    ) -> Result<Vec<SourceFile>, RunnerException> {
        self.create_inputs(files, None, allow_stdin, specs)
    }
    // port: AbstractCommandLineRunner#createInputs(List<FlagEntry<JsSourceType>>, List<JsonFileSpec>, List<JsChunkSpec>)
    pub fn create_inputs_with_json(
        &mut self,
        files: &[FlagEntry<JsSourceType>],
        json_files: &[Option<JsonFileSpec>],
        specs: &mut [JsChunkSpec],
    ) -> Result<Vec<SourceFile>, RunnerException> {
        self.create_inputs(files, Some(json_files), false, specs)
    }
    // port: AbstractCommandLineRunner#createInputs(List<FlagEntry<JsSourceType>>, List<JsonFileSpec>, boolean, List<JsChunkSpec>)
    pub fn create_inputs(
        &mut self,
        files: &[FlagEntry<JsSourceType>],
        json_files: Option<&[Option<JsonFileSpec>]>,
        _allow_stdin: bool,
        chunk_specs: &mut [JsChunkSpec],
    ) -> Result<Vec<SourceFile>, RunnerException> {
        let mut inputs = Vec::new();
        let mut using_stdin = false;
        let mut chunk_index = 0;
        let mut cumulative = chunk_specs.first().map_or(i32::MAX, |s| s.num_inputs);
        for (i, file) in files.iter().enumerate() {
            let filename = &file.value;
            if file.flag == JsSourceType::JS_ZIP {
                if self.config.typed_ast_list_input_filename.is_some() {
                    return Err(FlagUsageException("Can't use TypedASTs with --zip.".into()).into());
                }
                if filename != "-" {
                    let new_files = SourceFile::from_zip_file(
                        filename,
                        self.input_charset.unwrap_or(Charset::UTF_8),
                    )
                    .map_err(RunnerException::from)?;
                    if let Some(map) = &mut self.root_relative_paths_map
                        && let Some(root) = map.get(filename).cloned()
                    {
                        for entry in &new_files {
                            let name = entry.get_name();
                            assert!(name.contains(filename));
                            map.insert(name.into(), name.replace(filename, &root));
                        }
                    }
                    if let Some(spec) = chunk_specs.get_mut(chunk_index) {
                        spec.num_js_files =
                            spec.num_js_files.wrapping_add(new_files.len() as i32 - 1);
                    }
                    inputs.extend(new_files);
                }
            } else if filename != "-" {
                let kind = if file.flag == JsSourceType::WEAKDEP {
                    SourceKind::WEAK
                } else {
                    SourceKind::STRONG
                };
                inputs.push(
                    SourceFile::builder()
                        .with_path(filename)
                        .with_charset(self.input_charset.unwrap_or(Charset::UTF_8))
                        .with_kind(kind)
                        .build(),
                );
            } else {
                if self.config.typed_ast_list_input_filename.is_some() {
                    return Err(FlagUsageException("Can't use TypedASTs with stdin.".into()).into());
                }
                if !self.config.default_to_stdin {
                    return Err(FlagUsageException("Can't specify stdin.".into()).into());
                }
                if using_stdin {
                    return Err(FlagUsageException("Can't specify stdin twice.".into()).into());
                }
                if !self.config.output_manifests.is_empty() {
                    return Err(FlagUsageException(
                        "Manifest files cannot be generated when the input is from stdin.".into(),
                    )
                    .into());
                }
                if !self.config.output_bundles.is_empty() {
                    return Err(FlagUsageException(
                        "Bundle files cannot be generated when the input is from stdin.".into(),
                    )
                    .into());
                }
                let _ = writeln!(self.err, "{WAITING_FOR_INPUT_WARNING}");
                let mut bytes = Vec::new();
                self.input
                    .read_to_end(&mut bytes)
                    .map_err(RunnerException::from)?;
                let code = self
                    .input_charset
                    .unwrap_or(Charset::UTF_8)
                    .decode(&bytes, true)
                    .map_err(RunnerException::from)?;
                inputs.push(
                    SourceFile::builder()
                        .with_path("stdin")
                        .with_content(code)
                        .build(),
                );
                using_stdin = true;
            }
            if i as i32 >= cumulative.wrapping_sub(1) {
                chunk_index += 1;
                if let Some(spec) = chunk_specs.get(chunk_index) {
                    cumulative = cumulative.wrapping_add(spec.num_inputs);
                }
            }
        }
        if let Some(files) = json_files {
            for file in files {
                let file = file.as_ref().expect("null JsonFileSpec");
                inputs.push(SourceFile::from_code(
                    &file.get_path().expect("null path").to_string_lossy(),
                    file.get_src().expect("null src").clone(),
                ));
            }
        }
        Ok(inputs)
    }
    // port: AbstractCommandLineRunner#createExterns
    pub fn create_externs(&mut self) -> Result<Vec<SourceFile>, RunnerException> {
        self.create_extern_inputs(&self.config.externs.clone())
    }
    // port: AbstractCommandLineRunner#createExternInputs
    pub fn create_extern_inputs(
        &mut self,
        files: &[String],
    ) -> Result<Vec<SourceFile>, RunnerException> {
        if self.is_in_test_mode() {
            return Ok(self.externs_supplier_for_testing.as_mut().unwrap()());
        }
        let files: Vec<_> = files
            .iter()
            .map(|file| FlagEntry::new(JsSourceType::EXTERN, file.clone()))
            .collect();
        self.create_inputs(&files, None, false, &mut [])
            .map_err(|mut e| {
                if e.1 == RunnerExceptionKind::FlagUsage {
                    e.0 = format!("Bad --externs flag. {e}");
                }
                e
            })
    }
    // port: AbstractCommandLineRunner#createSourceInputs
    pub fn create_source_inputs(
        &mut self,
        specs: &mut [JsChunkSpec],
        files: &[FlagEntry<JsSourceType>],
        json_files: Option<&[Option<JsonFileSpec>]>,
        module_roots: &[String],
    ) -> Result<Vec<SourceFile>, RunnerException>
    where
        A: CliCompiler,
    {
        if self.is_in_test_mode() {
            return Ok(self
                .inputs_supplier_for_testing
                .as_mut()
                .map_or_else(Vec::new, |supplier| supplier()));
        }
        let stdin = [FlagEntry::new(JsSourceType::JS, "-".into())];
        let files = if files.is_empty() && json_files.is_none() && self.config.default_to_stdin {
            &stdin
        } else {
            files
        };
        let mut files = files.to_vec();
        for error in Self::deduplicate_ijs_files(&mut files, module_roots, !specs.is_empty())? {
            self.compiler.as_mut().expect("null compiler").report(error);
        }
        self.create_inputs(&files, json_files, json_files.is_none(), specs)
            .map_err(|mut e| {
                if e.1 == RunnerExceptionKind::FlagUsage {
                    e.0 = format!("Bad --js flag. {e}");
                }
                e
            })
    }
    // port: AbstractCommandLineRunner#createJsChunks
    pub fn create_js_chunks(
        specs: &[JsChunkSpec],
        inputs: &[CompilerInput],
    ) -> Result<Vec<JSChunk>, FlagUsageException> {
        assert!(!specs.is_empty());
        let mut names = Vec::new();
        let mut chunks: IndexMap<String, JSChunk> = IndexMap::<_, _>::default();
        let mut counts = IndexMap::<_, _>::default();
        let mut expected = 0i32;
        let mut minimum = 0i32;
        for spec in specs {
            if chunks.contains_key(&spec.name) {
                return Err(FlagUsageException(format!(
                    "Duplicate chunk name: {}",
                    spec.name
                )));
            }
            let chunk = JSChunk::new(&spec.name);
            for dep in &spec.deps {
                let other=chunks.get(dep).ok_or_else(||FlagUsageException(format!("Chunk '{}' depends on unknown chunk '{dep}'. Be sure to list chunks in dependency order.",spec.name)))?;
                chunk.add_dependency(other);
            }
            if spec.num_js_files < 0 {
                expected = -1;
            } else {
                minimum = minimum.wrapping_add(spec.num_js_files);
            }
            if expected >= 0 {
                expected = expected.wrapping_add(spec.num_js_files);
            }
            names.insert(0, spec.name.clone());
            counts.insert(spec.name.clone(), spec.num_js_files);
            chunks.insert(spec.name.clone(), chunk);
        }
        let total = inputs.len() as i32;
        if expected >= 0 || minimum > total {
            if minimum > total {
                expected = minimum;
            }
            if expected > total {
                return Err(FlagUsageException(format!(
                    "Not enough JS files specified. Expected {expected} but found {total}"
                )));
            }
            if expected < total {
                return Err(FlagUsageException(format!(
                    "Too many JS files specified. Expected {expected} but found {total}"
                )));
            }
        }
        let mut left = total;
        for (i, name) in names.iter().enumerate() {
            let mut count = counts[name];
            if i == names.len() - 1 && count == -1 {
                count = left;
            }
            for input in &inputs[(left - count) as usize..left as usize] {
                chunks[name].add(input.clone());
            }
            left -= count;
        }
        Ok(chunks.into_values().collect())
    }
    // port: AbstractCommandLineRunner#shouldGenerateMapPerChunk
    pub fn should_generate_map_per_chunk(options: &CompilerOptions) -> bool {
        options.should_gather_source_map_info()
            && options
                .get_source_map_output_path()
                .is_some_and(|p| p.contains("%outname%"))
    }
}
// port: Double#parseDouble
fn parse_java_double(input: &str) -> Option<f64> {
    closure_rhino::java_lang::parse_double(&input.into()).ok()
}

impl CommandLineConfig {
    pub fn to_json(&self) -> serde_json::Value {
        let mut fields = serde_json::Map::new();
        fields.insert("printVersion".into(), serde_json::json!(self.print_version));
        fields.insert("printTree".into(), serde_json::json!(self.print_tree));
        fields.insert(
            "printTreeJson".into(),
            serde_json::json!(self.print_tree_json),
        );
        fields.insert("printAst".into(), serde_json::json!(self.print_ast));
        fields.insert(
            "jscompDevMode".into(),
            serde_json::json!(format!("{:?}", self.jscomp_dev_mode)),
        );
        fields.insert("loggingLevel".into(), serde_json::json!(self.logging_level));
        fields.insert("externs".into(), serde_json::json!(self.externs));
        fields.insert(
            "mixedJsSources".into(),
            serde_json::json!(
                self.mixed_js_sources
                    .iter()
                    .map(|v| serde_json::json!({"flag":format!("{:?}",v.flag),"value":v.value}))
                    .collect::<Vec<_>>()
            ),
        );
        fields.insert(
            "defaultToStdin".into(),
            serde_json::json!(self.default_to_stdin),
        );
        fields.insert(
            "jsOutputFile".into(),
            serde_json::json!(self.js_output_file),
        );
        fields.insert(
            "continueSavedCompilationFileName".into(),
            serde_json::json!(self.continue_saved_compilation_file_name),
        );
        fields.insert(
            "restoredCompilationStage".into(),
            serde_json::json!(self.restored_compilation_stage),
        );
        fields.insert(
            "saveAfterCompilationStage".into(),
            serde_json::json!(self.save_after_compilation_stage),
        );
        fields.insert(
            "typedAstListInputFilename".into(),
            serde_json::json!(self.typed_ast_list_input_filename),
        );
        fields.insert(
            "saveCompilationStateToFilename".into(),
            serde_json::json!(self.save_compilation_state_to_filename),
        );
        fields.insert("chunk".into(), serde_json::json!(self.chunk));
        fields.insert(
            "sourceMapInputFiles".into(),
            serde_json::Value::Object(
                self.source_map_input_files
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                    .collect(),
            ),
        );
        fields.insert(
            "parseInlineSourceMaps".into(),
            serde_json::json!(self.parse_inline_source_maps),
        );
        fields.insert(
            "expectedDiagnostics".into(),
            serde_json::json!(self.expected_diagnostics),
        );
        fields.insert(
            "variableMapInputFile".into(),
            serde_json::json!(self.variable_map_input_file),
        );
        fields.insert(
            "propertyMapInputFile".into(),
            serde_json::json!(self.property_map_input_file),
        );
        fields.insert(
            "variableMapOutputFile".into(),
            serde_json::json!(self.variable_map_output_file),
        );
        fields.insert(
            "createNameMapFiles".into(),
            serde_json::json!(self.create_name_map_files),
        );
        fields.insert(
            "propertyMapOutputFile".into(),
            serde_json::json!(self.property_map_output_file),
        );
        fields.insert(
            "stringMapOutputPath".into(),
            serde_json::json!(self.string_map_output_path),
        );
        fields.insert(
            "instrumentationMappingFile".into(),
            serde_json::json!(self.instrumentation_mapping_file),
        );
        fields.insert(
            "codingConvention".into(),
            serde_json::json!(crate::option_setup::coding_convention_class_name(
                self.coding_convention.as_ref()
            )),
        );
        fields.insert(
            "summaryDetailLevel".into(),
            serde_json::json!(self.summary_detail_level),
        );
        fields.insert(
            "outputWrapper".into(),
            serde_json::json!(self.output_wrapper),
        );
        fields.insert("chunkWrapper".into(), serde_json::json!(self.chunk_wrapper));
        fields.insert(
            "chunkOutputPathPrefix".into(),
            serde_json::json!(self.chunk_output_path_prefix),
        );
        fields.insert(
            "chunkOutputFiles".into(),
            serde_json::json!(self.chunk_output_files),
        );
        fields.insert(
            "chunkConformanceFiles".into(),
            serde_json::json!(self.chunk_conformance_files),
        );
        fields.insert(
            "createSourceMap".into(),
            serde_json::json!(self.create_source_map),
        );
        fields.insert(
            "sourceMapDetailLevel".into(),
            serde_json::json!(format!("{:?}", self.source_map_detail_level)),
        );
        fields.insert(
            "sourceMapFormat".into(),
            serde_json::json!(format!("{:?}", self.source_map_format)),
        );
        fields.insert(
            "sourceMapLocationMappings".into(),
            serde_json::json!(
                self.source_map_location_mappings
                    .iter()
                    .map(|v| serde_json::json!({"prefix":v.prefix.to_string_lossy(),"replacement":v.replacement.to_string_lossy()}))
                    .collect::<Vec<_>>()
            ),
        );
        fields.insert(
            "applyInputSourceMaps".into(),
            serde_json::json!(self.apply_input_source_maps),
        );
        fields.insert(
            "warningGuards".into(),
            serde_json::json!(
                self.warning_guards
                    .iter()
                    .map(|v| serde_json::json!({"flag":format!("{:?}",v.flag),"value":v.value}))
                    .collect::<Vec<_>>()
            ),
        );
        fields.insert("define".into(), serde_json::json!(self.define));
        fields.insert(
            "browserFeaturesetYear".into(),
            serde_json::json!(self.browser_featureset_year),
        );
        fields.insert(
            "tweakProcessing".into(),
            serde_json::json!(format!("{:?}", self.tweak_processing)),
        );
        fields.insert("charset".into(), serde_json::json!(self.charset));
        fields.insert(
            "dependencyOptions".into(),
            self.dependency_options
                .as_ref()
                .map(crate::option_setup::ValueForSetup::setup_json)
                .unwrap_or(serde_json::Value::Null),
        );
        fields.insert(
            "outputManifests".into(),
            serde_json::json!(self.output_manifests),
        );
        fields.insert(
            "outputChunkDependencies".into(),
            serde_json::json!(self.output_chunk_dependencies),
        );
        fields.insert(
            "outputBundles".into(),
            serde_json::json!(self.output_bundles),
        );
        fields.insert(
            "skipNormalOutputs".into(),
            serde_json::json!(self.skip_normal_outputs),
        );
        fields.insert("manifestMaps".into(), serde_json::json!(self.manifest_maps));
        fields.insert(
            "processCommonJSModules".into(),
            serde_json::json!(self.process_common_js_modules),
        );
        fields.insert("moduleRoots".into(), serde_json::json!(self.module_roots));
        fields.insert(
            "warningsAllowFile".into(),
            serde_json::json!(self.warnings_allow_file),
        );
        fields.insert(
            "hideWarningsFor".into(),
            serde_json::json!(self.hide_warnings_for),
        );
        fields.insert("angularPass".into(), serde_json::json!(self.angular_pass));
        fields.insert(
            "jsonStreamMode".into(),
            serde_json::json!(format!("{:?}", self.json_stream_mode)),
        );
        fields.insert(
            "errorFormat".into(),
            serde_json::json!(format!("{:?}", self.error_format)),
        );
        fields.insert(
            "jsonWarningsFile".into(),
            serde_json::json!(self.json_warnings_file),
        );
        fields.insert(
            "includeTrailingNewline".into(),
            serde_json::json!(self.include_trailing_newline),
        );
        serde_json::Value::Object(fields)
    }
}
impl<A, B> AbstractCommandLineRunner<A, B> {
    // port: AbstractCommandLineRunner#checkChunkName
    pub fn check_chunk_name(name: &str) -> Result<(), FlagUsageException> {
        if !closure_rhino::js_identifier::JSIdentifier::is_js_identifier(
            &closure_rhino::js_string::JsString::from(name),
        ) {
            return Err(FlagUsageException(format!("Invalid chunk name: '{name}'")));
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#getExitStatusForResult
    pub fn get_exit_status_for_result(result: &closure_jscomp::result::Result) -> i32 {
        std::cmp::min(result.errors.len(), 0x7f) as i32
    }
    // port: AbstractCommandLineRunner#maybeCreateDirsForPath
    pub fn maybe_create_dirs_for_path(path_prefix: &str) {
        if !path_prefix.is_empty()
            && let Some(parent) = std::path::Path::new(path_prefix).parent()
        {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    // port: AbstractCommandLineRunner#closeAppendable
    pub fn close_appendable(mut output: Box<dyn OutputWriter + '_>) -> std::io::Result<()> {
        output.flush()?;
        output.close()
    }
    // port: AbstractCommandLineRunner#filenameToOutputStream
    pub fn filename_to_output_stream(
        file_name: Option<&str>,
    ) -> std::io::Result<Option<Box<dyn Write>>> {
        file_name
            .map(|name| {
                std::fs::File::create(name)
                    .map(|file| Box::new(file) as Box<dyn Write>)
                    .map_err(|error| {
                        std::io::Error::other(
                            crate::java_io::file_not_found(error, name, true, true).at_cli(
                                "AbstractCommandLineRunner",
                                "filenameToOutputStream",
                                1899,
                            ),
                        )
                    })
            })
            .transpose()
    }
    // port: AbstractCommandLineRunner#createWriter
    pub fn create_writer<'a>(
        stream: Box<dyn Write + 'a>,
        charset: Charset,
    ) -> Box<dyn OutputWriter + 'a> {
        Box::new(crate::java_io::EncodedWriter::new(stream, charset))
    }
    // port: AbstractCommandLineRunner#streamToLegacyOutputWriter
    pub fn stream_to_legacy_output_writer<'a>(
        &self,
        stream: Box<dyn Write + 'a>,
    ) -> Box<dyn OutputWriter + 'a> {
        Self::create_writer(stream, self.legacy_output_charset.unwrap_or(Charset::UTF_8))
    }
    // port: AbstractCommandLineRunner#streamToOutputWriter2
    pub fn stream_to_output_writer2<'a>(
        &self,
        stream: Box<dyn Write + 'a>,
    ) -> Box<dyn OutputWriter + 'a> {
        Self::create_writer(stream, self.output_charset2.unwrap_or(Charset::UTF_8))
    }
    // port: AbstractCommandLineRunner#fileNameToLegacyOutputWriter
    pub fn file_name_to_legacy_output_writer(
        &self,
        file_name: Option<&str>,
    ) -> std::io::Result<Option<Box<dyn OutputWriter>>> {
        if file_name.is_none() {
            return Ok(None);
        }
        if self.is_in_test_mode() {
            return Ok(Some(Box::new(std::io::Cursor::new(Vec::<u8>::new()))));
        }
        Ok(Self::filename_to_output_stream(file_name)
            .map_err(|e| {
                std::io::Error::other(RunnerException::from(e).at_cli(
                    "AbstractCommandLineRunner",
                    "fileNameToLegacyOutputWriter",
                    1876,
                ))
            })?
            .map(|stream| self.stream_to_legacy_output_writer(stream)))
    }
    // port: AbstractCommandLineRunner#fileNameToOutputWriter2
    pub fn file_name_to_output_writer2(
        &self,
        file_name: Option<&str>,
    ) -> std::io::Result<Option<Box<dyn OutputWriter>>> {
        if file_name.is_none() {
            return Ok(None);
        }
        if self.is_in_test_mode() {
            return Ok(Some(Box::new(std::io::Cursor::new(Vec::<u8>::new()))));
        }
        Ok(Self::filename_to_output_stream(file_name)
            .map_err(|e| {
                std::io::Error::other(RunnerException::from(e).at_cli(
                    "AbstractCommandLineRunner",
                    "fileNameToOutputWriter2",
                    1890,
                ))
            })?
            .map(|stream| self.stream_to_output_writer2(stream)))
    }
    // port: AbstractCommandLineRunner#createDefaultOutput
    pub fn create_default_output(&mut self) -> std::io::Result<Box<dyn OutputWriter>> {
        if !self.config.js_output_file.is_empty() {
            Ok(self
                .file_name_to_legacy_output_writer(Some(&self.config.js_output_file))
                .map_err(|e| {
                    std::io::Error::other(RunnerException::from(e).at_cli(
                        "AbstractCommandLineRunner",
                        "createDefaultOutput",
                        1128,
                    ))
                })?
                .unwrap())
        } else {
            Ok(Box::new(crate::java_io::EncodedWriter::for_print_stream(
                self.default_js_output.clone(),
                self.legacy_output_charset.unwrap_or(Charset::UTF_8),
            )))
        }
    }
    // port: AbstractCommandLineRunner#expandSourceMapPath
    pub fn expand_source_map_path(
        &mut self,
        options: &CompilerOptions,
        for_chunk: Option<&str>,
    ) -> Result<Option<String>, FlagUsageException> {
        if !options.should_gather_source_map_info() {
            return Ok(None);
        }
        self.expand_command_line_path(options.get_source_map_output_path().unwrap(), for_chunk)
            .map(Some)
    }
    // port: AbstractCommandLineRunner#openExternExportsStream
    pub fn open_extern_exports_stream(
        &self,
        options: &CompilerOptions,
        path: &str,
    ) -> std::io::Result<Option<Box<dyn OutputWriter>>> {
        let Some(exports) = options.get_extern_exports_path() else {
            return Ok(None);
        };
        let mut exports = exports.to_owned();
        if !exports.contains('/') {
            let parent = std::path::Path::new(path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "null".into());
            exports = format!("{parent}/{exports}");
        }
        self.file_name_to_output_writer2(Some(&exports))
    }
    // port: AbstractCommandLineRunner#getMapPath
    pub fn get_map_path(&self, output_file: &str) -> String {
        if output_file.is_empty() {
            return if self.config.chunk_output_path_prefix.is_empty() {
                "jscompiler".into()
            } else {
                self.config.chunk_output_path_prefix.clone()
            };
        }
        let file = std::path::Path::new(output_file);
        let name = file
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = name.strip_suffix(".js").unwrap_or(&name);
        if let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
            format!("{}/{name}", parent.to_string_lossy())
        } else {
            name.into()
        }
    }
    // port: AbstractCommandLineRunner#shouldGenerateOutputPerChunk
    pub fn should_generate_output_per_chunk(&self, output: Option<&str>) -> bool {
        !self.config.chunk.is_empty() && output.is_some_and(|o| o.contains("%outname%"))
    }
    // port: AbstractCommandLineRunner#getJavascriptEscaper
    pub fn get_javascript_escaper(value: &closure_rhino::js_string::JsString) -> String {
        closure_jscomp::deps::source_code_escapers::SourceCodeEscapers::javascript_escaper()
            .escape(value)
    }
    // port: AbstractCommandLineRunner#outputJsonStream
    pub fn output_json_stream(&mut self) -> std::io::Result<()> {
        let values = self
            .files_to_stream_out
            .iter()
            .cloned()
            .map(Some)
            .collect::<Vec<_>>();
        let json = crate::gson::json_writer::to_json(&values);
        self.default_js_output
            .write_all(&crate::java_io::encode(&json, Charset::UTF_8, true))?;
        self.default_js_output.flush()
    }
    // port: AbstractCommandLineRunner#printManifestTo
    pub fn print_manifest_to(&self, chunk: &JSChunk, out: &mut dyn Write) -> std::io::Result<()> {
        for input in chunk.get_inputs() {
            let name = input.get_name();
            if closure_jscomp::abstract_compiler::AbstractCompiler::is_fill_file_name(name) {
                continue;
            }
            let relative = self
                .root_relative_paths_map
                .as_ref()
                .expect("null rootRelativePathsMap")
                .get(name);
            out.write_all(relative.map_or(name, String::as_str).as_bytes())?;
            out.write_all(b"\n")?;
        }
        Ok(())
    }
}
pub trait CliCompiler {
    fn as_merged_compiler(&mut self) -> &mut closure_jscomp::compiler::Compiler;
    fn report(&mut self, error: closure_jscomp::js_error::JSError);
}
impl CliCompiler for closure_jscomp::compiler::Compiler {
    fn as_merged_compiler(&mut self) -> &mut closure_jscomp::compiler::Compiler {
        self
    }
    // port: Compiler#report
    fn report(&mut self, error: closure_jscomp::js_error::JSError) {
        closure_jscomp::compiler::Compiler::report(self, error);
    }
}
impl<A, B> AbstractCommandLineRunner<A, B> {
    // port: AbstractCommandLineRunner#deduplicateIjsFiles
    pub fn deduplicate_ijs_files(
        files: &mut Vec<FlagEntry<JsSourceType>>,
        module_roots: &[String],
        has_chunk_specs: bool,
    ) -> Result<Vec<closure_jscomp::js_error::JSError>, FlagUsageException> {
        use closure_jscomp::ijs::ijs_errors::{BAD_IJS_FILE_NAME, CONFLICTING_IJS_FILE};
        use closure_jscomp::js_error::JSError;
        let mut errors = Vec::new();
        let mut relative_to_absolute_name = IndexMap::<_, _>::default();
        for file in files.iter() {
            if matches!(file.flag, JsSourceType::JS | JsSourceType::WEAKDEP) {
                let absolute_name = &file.value;
                let relative_name =
                    Self::get_module_root_relative_name(absolute_name, module_roots);
                relative_to_absolute_name.insert(relative_name, absolute_name.clone());
            }
        }
        let mut i = 0;
        while i < files.len() {
            let file = &files[i];
            if file.flag == JsSourceType::IJS {
                if has_chunk_specs {
                    return Err(FlagUsageException(
                        "--ijs is incompatible with --chunk or --module.".into(),
                    ));
                }
                let absolute_name = &file.value;
                if !absolute_name.ends_with(".i.js") {
                    errors.push(JSError::make_without_location(
                        &BAD_IJS_FILE_NAME,
                        &[absolute_name],
                    ));
                    i += 1;
                    continue;
                }
                let relative_name =
                    Self::get_module_root_relative_name(absolute_name, module_roots);
                let relative_non_ijs_name = &relative_name[..relative_name.len() - ".i.js".len()];
                if let Some(conflicting) = relative_to_absolute_name.get(relative_non_ijs_name) {
                    errors.push(JSError::make_without_location(
                        &CONFLICTING_IJS_FILE,
                        &[conflicting, absolute_name],
                    ));
                    files.remove(i);
                    continue;
                }
            }
            i += 1;
        }
        Ok(errors)
    }
    // port: AbstractCommandLineRunner#getModuleRootRelativeName
    pub fn get_module_root_relative_name(filename: &str, module_roots: &[String]) -> String {
        for root in module_roots {
            let prefix = format!("{root}/");
            if let Some(relative) = filename.strip_prefix(&prefix) {
                return relative.into();
            }
        }
        filename.into()
    }
    // port: AbstractCommandLineRunner#printChunkGraphJsonTo
    pub fn print_chunk_graph_json_to(
        graph: &closure_jscomp::js_chunk_graph::JSChunkGraph,
        out: &mut dyn Write,
    ) -> std::io::Result<()> {
        out.write_all(crate::gson::json_writer::element_to_json(&graph.to_json()).as_bytes())
    }
    // port: AbstractCommandLineRunner#printChunkGraphManifestOrBundleTo (manifest branch)
    pub fn print_chunk_graph_manifest_to(
        &self,
        graph: &closure_jscomp::js_chunk_graph::JSChunkGraph,
        out: &mut dyn Write,
    ) -> std::io::Result<()> {
        let mut requires_newline = false;
        for chunk in graph.get_all_chunks() {
            if requires_newline {
                out.write_all(b"\n")?;
            }
            let dependencies = chunk.get_sorted_dependency_names().join(",");
            writeln!(
                out,
                "{{{}{}}}",
                chunk.get_name(),
                if dependencies.is_empty() {
                    String::new()
                } else {
                    format!(":{dependencies}")
                }
            )?;
            self.print_manifest_to(chunk, out)?;
            requires_newline = true;
        }
        Ok(())
    }
}

impl
    AbstractCommandLineRunner<
        closure_jscomp::compiler::Compiler,
        closure_jscomp::compiler_options::CompilerOptions,
    >
{
    // port: AbstractCommandLineRunner#performCompilation
    pub fn perform_compilation(
        &mut self,
        metrics_recorder: &mut dyn closure_jscomp::compile_metrics_recorder_interface::CompileMetricsRecorderInterface,
    ) {
        self.initialize_state_before_compilation();
        self.run_compiler_passes(metrics_recorder);
        if !self.compiler.as_ref().unwrap().has_errors() {
            self.save_state();
        }
        self.compiler
            .as_mut()
            .unwrap()
            .perform_post_compilation_tasks();
    }
    // port: AbstractCommandLineRunner#runCompilerPasses
    pub fn run_compiler_passes(
        &mut self,
        metrics_recorder: &mut dyn closure_jscomp::compile_metrics_recorder_interface::CompileMetricsRecorderInterface,
    ) {
        use closure_jscomp::compiler_options::SegmentOfCompilationToRun;
        let mut run_stage1 = false;
        let mut run_stage2 = false;
        let mut run_stage3 = false;
        let mut instrument_for_coverage = false;
        let action_metrics_name;
        if self
            .compiler
            .as_ref()
            .unwrap()
            .get_options()
            .get_instrument_for_coverage_only()
        {
            action_metrics_name = "instrument for coverage";
            instrument_for_coverage = true;
        } else if self.config.should_save_after_stage1() {
            action_metrics_name = "stage 1";
            run_stage1 = true;
        } else if self
            .config
            .should_restore_typed_asts_perform_stage2_and_save()
        {
            action_metrics_name = "parse & optimize";
            run_stage2 = true;
        } else if self.config.should_restore_typed_asts_perform_stages2_and3() {
            action_metrics_name = "skip-checks compile";
            run_stage2 = true;
            run_stage3 = true;
        } else if self.config.should_restore_and_perform_stage2_and_save() {
            action_metrics_name = "stage 2/3";
            run_stage2 = true;
        } else if self.config.should_restore_and_perform_stages2_and3() {
            action_metrics_name = "stage 2/2";
            run_stage2 = true;
            run_stage3 = true;
        } else if self.config.should_restore_and_perform_stage3() {
            action_metrics_name = "stage 3/3";
            run_stage3 = true;
        } else {
            action_metrics_name = if self
                .compiler
                .as_ref()
                .unwrap()
                .get_options()
                .is_checks_only()
            {
                "checks-only"
            } else {
                "full compile"
            };
            run_stage1 = true;
            run_stage2 = true;
            run_stage3 = true;
        }
        metrics_recorder.record_action_name(action_metrics_name);
        let compiler = self.compiler.as_mut().unwrap();
        if compiler.has_errors() {
            return;
        }
        metrics_recorder.record_start_state(compiler);
        if run_stage1 {
            compiler.stage1_passes();
            if compiler.has_errors() {
                return;
            }
        }
        if run_stage2 {
            compiler.stage2_passes(SegmentOfCompilationToRun::OPTIMIZATIONS);
            if compiler.has_errors() {
                return;
            }
        }
        if run_stage3 {
            compiler.stage3_passes();
            if compiler.has_errors() {
                return;
            }
        }
        if instrument_for_coverage {
            compiler.instrument_for_coverage();
        }
    }
    // port: AbstractCommandLineRunner#initializeStateBeforeCompilation
    pub fn initialize_state_before_compilation(&mut self) {
        if self.config.restored_compilation_stage != -1 {
            self.restore_state();
        } else if self.config.typed_ast_list_input_filename.is_some() {
            // The typed AST filesystem was initialized before compilation.
        } else {
            self.compiler.as_mut().unwrap().parse_for_compilation();
        }
    }
    // port: AbstractCommandLineRunner#restoreState
    pub fn restore_state(&mut self) {
        let filename = self
            .config
            .continue_saved_compilation_file_name
            .as_ref()
            .unwrap();
        let result = std::fs::File::open(filename).and_then(|file| {
            self.compiler
                .as_mut()
                .unwrap()
                .restore_state(&mut std::io::BufReader::new(file))
        });
        if result.is_err() {
            self.compiler.as_mut().unwrap().report(
                closure_jscomp::js_error::JSError::make_without_location(
                    &COULD_NOT_DESERIALIZE_AST,
                    &[filename],
                ),
            );
        }
    }
    // port: AbstractCommandLineRunner#saveState
    pub fn save_state(&mut self) {
        let Some(filename) = &self.config.save_compilation_state_to_filename else {
            return;
        };
        let result = std::fs::File::create(filename).and_then(|file| {
            self.compiler
                .as_mut()
                .unwrap()
                .save_state(&mut std::io::BufWriter::new(file))
        });
        if result.is_err() {
            self.compiler.as_mut().unwrap().report(
                closure_jscomp::js_error::JSError::make_without_location(
                    &COULD_NOT_SERIALIZE_AST,
                    &[filename],
                ),
            );
        }
    }
    // port: AbstractCommandLineRunner#writeOutput(Appendable, Compiler, String, String, String, Function, String)
    #[allow(clippy::too_many_arguments)] // Preserve the Java overload's parameter order.
    pub fn write_output_code(
        &self,
        out: &mut dyn crate::java_io::JavaAppendable,
        compiler: Option<&mut closure_jscomp::compiler::Compiler>,
        code: &closure_rhino::js_string::JsString,
        wrapper: &str,
        code_placeholder: &str,
        escaper: Option<fn(&closure_rhino::js_string::JsString) -> String>,
        _filename: &str,
    ) -> std::io::Result<()> {
        if let Some(pos) = wrapper.find(code_placeholder) {
            let prefix = &wrapper[..pos];
            if pos > 0 {
                out.append(&prefix.into())?;
            }
            if let Some(escaper) = escaper {
                out.append(&escaper(code).into())?;
            } else {
                out.append(code)?;
            }
            let suffix_start = pos + code_placeholder.len();
            if suffix_start != wrapper.len() {
                out.append(&wrapper[suffix_start..].into())?;
            }
            if self.config.include_trailing_newline {
                out.append(&"\n".into())?;
            }
            if let Some(compiler) = compiler
                && let Some(map) = compiler.get_source_map()
            {
                map.lock().unwrap().set_wrapper_prefix(&prefix.into());
            }
        } else {
            out.append(code)?;
            if self.config.include_trailing_newline {
                out.append(&"\n".into())?;
            }
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#outputSourceMap
    pub fn output_source_map(
        &mut self,
        options: &closure_jscomp::compiler_options::CompilerOptions,
        associated_name: &str,
    ) -> std::io::Result<()> {
        if !options.should_gather_source_map_info()
            || options.get_source_map_output_path() == Some("/dev/null")
        {
            return Ok(());
        }
        let out_name = self
            .expand_source_map_path(options, None)
            .map_err(std::io::Error::other)?
            .unwrap();
        Self::maybe_create_dirs_for_path(&out_name);
        let mut out = self.file_name_to_output_writer2(Some(&out_name))?.unwrap();
        let mut text = String::new();
        self.compiler
            .as_ref()
            .unwrap()
            .get_source_map()
            .unwrap()
            .lock()
            .unwrap()
            .append_to(&mut text, associated_name)
            .map_err(std::io::Error::other)?;
        out.write_all(text.as_bytes())?;
        out.flush()
    }
    // port: AbstractCommandLineRunner#outputNameMaps
    pub fn output_name_maps(&self) -> Result<(), RunnerException> {
        let mut property_map_output_path = None;
        let mut variable_map_output_path = None;
        if self.config.create_name_map_files {
            let base_path = self.get_map_path(&self.config.js_output_file);
            property_map_output_path = Some(format!("{base_path}_props_map.out"));
            variable_map_output_path = Some(format!("{base_path}_vars_map.out"));
        }
        if !self.config.variable_map_output_file.is_empty() {
            if variable_map_output_path.is_some() {
                return Err(FlagUsageException("The flags variable_map_output_file and create_name_map_files cannot both be used simultaneously.".into()).into());
            }
            variable_map_output_path = Some(self.config.variable_map_output_file.clone());
        }
        if !self.config.property_map_output_file.is_empty() {
            if property_map_output_path.is_some() {
                return Err(FlagUsageException("The flags property_map_output_file and create_name_map_files cannot both be used simultaneously.".into()).into());
            }
            property_map_output_path = Some(self.config.property_map_output_file.clone());
        }
        let compiler = self.compiler.as_ref().unwrap();
        if let Some(path) = variable_map_output_path
            && let Some(map) = compiler.get_variable_map()
        {
            map.save(&path).map_err(RunnerException::from)?;
        }
        if let Some(path) = property_map_output_path
            && let Some(map) = compiler.get_property_map()
        {
            map.save(&path).map_err(RunnerException::from)?;
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#outputStringMap
    pub fn output_string_map(&self) -> std::io::Result<()> {
        if !self.config.string_map_output_path.is_empty() {
            if let Some(map) = self.compiler.as_ref().unwrap().get_string_map() {
                map.save(&self.config.string_map_output_path)?;
            } else {
                // Java File.createNewFile returns false if the path already exists.
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&self.config.string_map_output_path)
                {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        return Err(std::io::Error::other(format!(
                            "Could not create file: {}",
                            self.config.string_map_output_path
                        )));
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#outputInstrumentationMapping
    pub fn output_instrumentation_mapping(&mut self) -> std::io::Result<()> {
        if !self.config.instrumentation_mapping_file.is_empty() {
            let path = self
                .expand_command_line_path(&self.config.instrumentation_mapping_file.clone(), None)
                .map_err(std::io::Error::other)?;
            self.compiler
                .as_ref()
                .unwrap()
                .get_instrumentation_mapping()
                .unwrap()
                .save(&path)?;
        }
        Ok(())
    }
}

impl AbstractCommandLineRunner<closure_jscomp::compiler::Compiler, CompilerOptions> {
    // port: AbstractCommandLineRunner#writeOutput(Appendable, Compiler, LicenseTracker, JSChunk, String, String, Function, String)
    #[allow(clippy::too_many_arguments)] // Preserve the Java overload's parameter order.
    pub fn write_compiler_output(
        &mut self,
        out: &mut dyn crate::java_io::JavaAppendable,
        license_tracker: &mut (dyn closure_jscomp::code_printer::LicenseTracker + Send),
        chunk: Option<&JSChunk>,
        wrapper: &str,
        code_placeholder: &str,
        escaper: Option<fn(&closure_rhino::js_string::JsString) -> String>,
        filename: &str,
    ) -> std::io::Result<()> {
        use closure_jscomp::compiler_options::OutputJs;
        if self
            .compiler
            .as_ref()
            .unwrap()
            .get_options()
            .get_output_js()
            == OutputJs::SENTINEL
        {
            out.append(
                &"// No JS output because the compiler was run in checks-only mode.\n".into(),
            )?;
            return Ok(());
        }
        assert_eq!(
            self.compiler
                .as_ref()
                .unwrap()
                .get_options()
                .get_output_js(),
            OutputJs::NORMAL
        );
        let mut compiler = self.compiler.take().unwrap();
        let code = match chunk {
            None => compiler.to_source(),
            Some(chunk) => compiler.to_source_for_chunk_with_tracker(license_tracker, chunk),
        };
        let result = self.write_output_code(
            out,
            Some(&mut compiler),
            &code,
            wrapper,
            code_placeholder,
            escaper,
            filename,
        );
        self.compiler = Some(compiler);
        result
    }
    // port: AbstractCommandLineRunner#writeChunkOutput
    pub fn write_chunk_output(
        &mut self,
        filename: &str,
        out: &mut dyn crate::java_io::JavaAppendable,
        license_tracker: &mut (dyn closure_jscomp::code_printer::LicenseTracker + Send),
        chunk: &JSChunk,
    ) -> Result<(), RunnerException> {
        if self.parsed_chunk_wrappers.is_none() {
            self.parsed_chunk_wrappers = Some(Self::parse_chunk_wrappers(
                &self.config.chunk_wrapper,
                self.compiler
                    .as_ref()
                    .unwrap()
                    .get_chunk_graph()
                    .unwrap()
                    .get_all_chunks()
                    .iter()
                    .map(JSChunk::get_name)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )?);
        }
        if !self.is_output_in_json() {
            Self::maybe_create_dirs_for_path(filename);
        }
        let basename = std::path::Path::new(filename)
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        let wrapper = self
            .parsed_chunk_wrappers
            .as_ref()
            .unwrap()
            .get(&chunk.get_name())
            .unwrap()
            .replace("%basename%", &basename);
        self.write_compiler_output(
            out,
            license_tracker,
            Some(chunk),
            &wrapper,
            "%s",
            None,
            filename,
        )
        .map_err(RunnerException::from)
    }
    // port: AbstractCommandLineRunner#outputSingleBinary
    pub fn output_single_binary(
        &mut self,
        options: &CompilerOptions,
    ) -> Result<(), RunnerException> {
        let mut marker = OUTPUT_MARKER;
        let mut escaper = None;
        if self.config.output_wrapper.contains("%output|jsstring%") {
            marker = "%output|jsstring%";
            escaper = Some(
                Self::get_javascript_escaper as fn(&closure_rhino::js_string::JsString) -> String,
            );
        }
        if self.is_output_in_json() {
            let file = self.create_json_file(options, marker, escaper)?;
            self.files_to_stream_out.push(file);
        } else {
            if !self.config.js_output_file.is_empty() {
                Self::maybe_create_dirs_for_path(&self.config.js_output_file);
            }
            let mut out: Box<dyn OutputWriter> = if self.config.js_output_file.is_empty() {
                Box::new(crate::java_io::EncodedWriter::for_print_stream(
                    self.default_js_output.clone(),
                    self.legacy_output_charset.unwrap_or(Charset::UTF_8),
                ))
            } else {
                self.file_name_to_legacy_output_writer(Some(&self.config.js_output_file))
                    .map_err(|e| {
                        RunnerException::from(e)
                            .at_cli("AbstractCommandLineRunner", "createDefaultOutput", 1128)
                            .at_cli("AbstractCommandLineRunner", "outputSingleBinary", 1605)
                    })?
                    .unwrap()
            };
            let mut tracker =
                closure_jscomp::compiler_license_tracker::SingleBinaryLicenseTracker::new(
                    self.compiler.as_ref().unwrap(),
                );
            self.write_compiler_output(
                &mut out,
                &mut tracker,
                None,
                &self.config.output_wrapper.clone(),
                marker,
                escaper,
                &self.config.js_output_file.clone(),
            )
            .map_err(RunnerException::from)?;
            Self::close_appendable(out).map_err(RunnerException::from)?;
        }
        Ok(())
    }
    // port: AbstractCommandLineRunner#createJsonFile
    pub fn create_json_file(
        &mut self,
        options: &CompilerOptions,
        output_marker: &str,
        escaper: Option<fn(&closure_rhino::js_string::JsString) -> String>,
    ) -> Result<JsonFileSpec, RunnerException> {
        let mut output = crate::java_io::StringBuilder::default();
        let mut tracker = closure_jscomp::compiler_license_tracker::SingleBinaryLicenseTracker::new(
            self.compiler.as_ref().unwrap(),
        );
        self.write_compiler_output(
            &mut output,
            &mut tracker,
            None,
            &self.config.output_wrapper.clone(),
            output_marker,
            escaper,
            &self.config.js_output_file.clone(),
        )
        .map_err(RunnerException::from)?;
        let path = if self.config.js_output_file.is_empty() {
            "compiled.js"
        } else {
            &self.config.js_output_file
        };
        let mut file =
            JsonFileSpec::new(Some(output.into_js_string()), Some(path.into()), None, None);
        if options.should_gather_source_map_info() {
            let mut source_map = String::new();
            self.compiler
                .as_ref()
                .unwrap()
                .get_source_map()
                .unwrap()
                .lock()
                .unwrap()
                .append_to(&mut source_map, path)
                .map_err(RunnerException::from)?;
            file.set_source_map(source_map.into());
        }
        Ok(file)
    }
    // port: AbstractCommandLineRunner#outputChunkBinaryAndSourceMaps
    pub fn output_chunk_binary_and_source_maps(
        &mut self,
        chunks: &[JSChunk],
        options: &CompilerOptions,
    ) -> Result<Option<&'static closure_jscomp::diagnostic_type::DiagnosticType>, RunnerException>
    {
        self.parsed_chunk_wrappers = Some(Self::parse_chunk_wrappers(
            &self.config.chunk_wrapper,
            &chunks.iter().map(JSChunk::get_name).collect::<Vec<_>>(),
        )?);
        if !self.is_output_in_json() {
            Self::maybe_create_dirs_for_path(&format!(
                "{}dummy",
                self.config.chunk_output_path_prefix
            ));
        }
        let mut map_file_out: Option<Box<dyn OutputWriter>> = None;
        if !(Self::should_generate_map_per_chunk(options)
            || !options.should_gather_source_map_info()
            || self.is_output_in_json())
        {
            return Ok(Some(&INVALID_CHUNK_SOURCEMAP_PATTERN));
        }
        let mut tracker =
            closure_jscomp::compiler_license_tracker::ChunkGraphAwareLicenseTracker::new(
                self.compiler.as_ref().unwrap(),
            );
        for chunk in chunks {
            if chunk.get_name() == JSChunk::WEAK_CHUNK_NAME {
                continue;
            }
            if self.is_output_in_json() {
                let file = self.create_json_file_from_chunk(chunk)?;
                self.files_to_stream_out.push(file);
            } else {
                if Self::should_generate_map_per_chunk(options) {
                    let path = self.expand_source_map_path(options, Some(&chunk.get_name()))?;
                    map_file_out = self
                        .file_name_to_output_writer2(path.as_deref())
                        .map_err(RunnerException::from)?;
                }
                let filename = self.get_chunk_output_file_name(&chunk.get_name())?;
                Self::maybe_create_dirs_for_path(&filename);
                let mut out = self
                    .file_name_to_legacy_output_writer(Some(&filename))
                    .map_err(RunnerException::from)?
                    .unwrap();
                if options.should_gather_source_map_info() {
                    self.compiler
                        .as_mut()
                        .unwrap()
                        .reset_and_intitialize_source_map();
                }
                tracker.set_current_chunk_context(chunk.clone());
                self.write_chunk_output(&filename, &mut out, &mut tracker, chunk)?;
                if options.should_gather_source_map_info() {
                    let mut text = String::new();
                    self.compiler
                        .as_ref()
                        .unwrap()
                        .get_source_map()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .append_to(&mut text, filename.as_str())
                        .map_err(RunnerException::from)?;
                    map_file_out
                        .as_mut()
                        .unwrap()
                        .write_all(text.as_bytes())
                        .map_err(RunnerException::from)?;
                }
                Self::close_appendable(out).map_err(RunnerException::from)?;
                if Self::should_generate_map_per_chunk(options)
                    && let Some(map) = map_file_out.take()
                {
                    Self::close_appendable(map).map_err(RunnerException::from)?;
                }
            }
        }
        if let Some(map) = map_file_out {
            Self::close_appendable(map).map_err(RunnerException::from)?;
        }
        Ok(None)
    }
    // port: AbstractCommandLineRunner#createJsonFileFromChunk
    pub fn create_json_file_from_chunk(
        &mut self,
        chunk: &JSChunk,
    ) -> Result<JsonFileSpec, RunnerException> {
        self.compiler
            .as_mut()
            .unwrap()
            .reset_and_intitialize_source_map();
        let filename = self.get_chunk_output_file_name(&chunk.get_name())?;
        let mut output = crate::java_io::StringBuilder::default();
        let mut tracker =
            closure_jscomp::compiler_license_tracker::ScriptNodeLicensesOnlyTracker::new(
                self.compiler.as_ref().unwrap(),
            );
        self.write_chunk_output(&filename, &mut output, &mut tracker, chunk)?;
        let mut file = JsonFileSpec::new(
            Some(output.into_js_string()),
            Some(filename.clone().into()),
            None,
            None,
        );
        let mut source_map = String::new();
        self.compiler
            .as_ref()
            .unwrap()
            .get_source_map()
            .unwrap()
            .lock()
            .unwrap()
            .append_to(&mut source_map, filename.as_str())
            .map_err(RunnerException::from)?;
        file.set_source_map(source_map.into());
        Ok(file)
    }
}

impl AbstractCommandLineRunner<closure_jscomp::compiler::Compiler, CompilerOptions> {
    // port: AbstractCommandLineRunner#initWithTypedAstFilesystem
    pub fn init_with_typed_ast_filesystem(
        &mut self,
        externs: &[Arc<SourceFile>],
        sources: &[Arc<SourceFile>],
        options: CompilerOptions,
        filename: &str,
    ) {
        let result = std::fs::File::open(filename).and_then(|file| {
            let mut stream = flate2::read::GzDecoder::new(std::io::BufReader::with_capacity(
                8 * 1024 * 1024,
                file,
            ));
            if stream.header().is_none() {
                return Err(std::io::Error::other("Not in GZIP format"));
            }
            self.compiler
                .as_mut()
                .unwrap()
                .init_with_typed_ast_filesystem(externs, sources, options, &mut stream);
            Ok(())
        });
        if result.is_err() {
            self.compiler.as_mut().unwrap().report(
                closure_jscomp::js_error::JSError::make_without_location(
                    &COULD_NOT_DESERIALIZE_AST,
                    &[filename],
                ),
            );
        }
    }
    // port: AbstractCommandLineRunner#initChunksWithTypedAstFilesystem
    pub fn init_chunks_with_typed_ast_filesystem(
        &mut self,
        externs: &[Arc<SourceFile>],
        chunks: &[JSChunk],
        options: CompilerOptions,
        filename: &str,
    ) {
        let result = std::fs::File::open(filename).and_then(|file| {
            let mut stream = flate2::read::GzDecoder::new(std::io::BufReader::with_capacity(
                8 * 1024 * 1024,
                file,
            ));
            if stream.header().is_none() {
                return Err(std::io::Error::other("Not in GZIP format"));
            }
            self.compiler
                .as_mut()
                .unwrap()
                .init_chunks_with_typed_ast_filesystem(
                    externs,
                    chunks.to_vec(),
                    options,
                    &mut stream,
                );
            Ok(())
        });
        if result.is_err() {
            self.compiler.as_mut().unwrap().report(
                closure_jscomp::js_error::JSError::make_without_location(
                    &COULD_NOT_DESERIALIZE_AST,
                    &[filename],
                ),
            );
        }
    }
}

impl AbstractCommandLineRunner<closure_jscomp::compiler::Compiler, CompilerOptions> {
    // port: AbstractCommandLineRunner#getDiagnosticGroups
    pub fn get_diagnostic_groups(&self) -> closure_jscomp::diagnostic_groups::DiagnosticGroups {
        self.compiler.as_ref().map_or_else(
            closure_jscomp::diagnostic_groups::DiagnosticGroups::new,
            |compiler| compiler.get_diagnostic_groups(),
        )
    }
}
