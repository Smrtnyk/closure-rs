/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ClosureUnawareOptions.java.

//! Port of `ClosureUnawareOptions.java`.
//!
//! Creates the CompilerOptions for the nested compilation of @closureUnaware code
//! (TranspileAndOptimizeClosureUnaware), deriving them from the options of the main compilation.
use crate::{
    check_level::CheckLevel,
    coding_conventions::CodingConventions,
    compilation_level::CompilationLevel,
    compiler_options::{CompilerOptions, Es6SubclassTranspilation, Reach},
    diagnostic_groups,
    js::runtime_js_lib_manager::RuntimeLibraryMode,
};

pub struct ClosureUnawareOptions<'a> {
    shadow_options: CompilerOptions,
    original: &'a CompilerOptions,
}

impl<'a> ClosureUnawareOptions<'a> {
    // port: ClosureUnawareOptions#convert
    pub fn convert(original: &CompilerOptions) -> CompilerOptions {
        let options = ClosureUnawareOptions::new(original);
        options.to_compiler_options()
    }

    // port: ClosureUnawareOptions#ClosureUnawareOptions
    fn new(original: &'a CompilerOptions) -> Self {
        Self {
            shadow_options: CompilerOptions::new(),
            original,
        }
    }

    // port: ClosureUnawareOptions#toCompilerOptions
    #[allow(clippy::wrong_self_convention)] // Java's name; consumes the builder.
    fn to_compiler_options(mut self) -> CompilerOptions {
        self.set_transpilation_options();
        self.set_safe_optimization_assumptions();
        self.copy_output_options();
        self.copy_debug_options();
        self.shadow_options
    }

    // port: ClosureUnawareOptions#setTranspilationOptions
    fn set_transpilation_options(&mut self) {
        let original = self.original;
        let shadow_options = &mut self.shadow_options;
        shadow_options.set_rewrite_polyfills(false); // for now, we ignore polyfills that may be needed.
        match original.get_runtime_library_mode() {
            RuntimeLibraryMode::NO_OP => {
                shadow_options.set_runtime_library_mode(RuntimeLibraryMode::NO_OP)
            }
            RuntimeLibraryMode::RECORD_ONLY
            | RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS
            | RuntimeLibraryMode::INJECT => {
                shadow_options.set_runtime_library_mode(RuntimeLibraryMode::EXTERN_FIELD_NAMES)
            }
            RuntimeLibraryMode::EXTERN_FIELD_NAMES => panic!("java.lang.AssertionError"),
        }
        shadow_options
            .set_skip_non_transpilation_passes(original.get_skip_non_transpilation_passes());
        shadow_options
            .set_es6_subclass_transpilation(Es6SubclassTranspilation::SAFE_REFLECT_CONSTRUCT);
        shadow_options.set_output_feature_set(original.get_output_feature_set());
        shadow_options.set_instrument_async_context(original.get_instrument_async_context());

        // To aid rollout of @closureUnaware transpilation, ignore warnings about untranspilable
        // features for now. (Any project using @closureUnaware today (i.e. early 2026) gets zero
        // transpilation, so rolling out with these suppressions is an incremental improvement.)

        // DiagnosticGroups.UNTRANSPILABLE_FEATURES: features JSCompiler will never transpile but
        // can pass through unchanged to the output, such as newer regex syntax.
        shadow_options.set_warning_level(
            diagnostic_groups::UNTRANSPILABLE_FEATURES.clone(),
            CheckLevel::OFF,
        );
    }

    // port: ClosureUnawareOptions#setSafeOptimizationAssumptions
    fn set_safe_optimization_assumptions(&mut self) {
        let original = self.original;
        let shadow_options = &mut self.shadow_options;
        CompilationLevel::SIMPLE_OPTIMIZATIONS.set_options_for_compilation_level(shadow_options);
        shadow_options.set_remove_unused_variables(Reach::ALL);

        shadow_options.set_compute_function_side_effects(false); // unsafe for properties.
        shadow_options.set_coding_convention(CodingConventions::get_default());
        shadow_options.set_remove_closure_asserts(false);

        shadow_options.set_assume_closures_only_capture_references(false);
        shadow_options.set_assume_getters_are_pure(false);
        shadow_options.set_assume_properties_are_statically_analyzable(false);
        shadow_options.set_assume_static_inheritance_is_not_used(false);
        shadow_options.set_assume_strict_this(false);

        if original.get_max_function_size_after_inlining()
            != CompilerOptions::UNLIMITED_FUN_SIZE_AFTER_INLINING
        {
            shadow_options.set_max_function_size_after_inlining(
                original.get_max_function_size_after_inlining(),
            );
        }
        shadow_options.set_use_size_heuristic_to_stop_optimization_loop(
            original.should_use_size_heuristic_to_stop_optimization_loop(),
        );
        shadow_options
            .set_max_optimization_loop_iterations(original.get_max_optimization_loop_iterations());

        shadow_options.set_prefer_stable_names(original.should_prefer_stable_names());
        // Java passes null through; a new CompilerOptions already holds null there.
        if let Some(map) = original.get_input_property_map() {
            shadow_options.set_input_property_map(map.clone());
        }
        if let Some(map) = original.get_input_variable_map() {
            shadow_options.set_input_variable_map(map.clone());
        }
    }

    // port: ClosureUnawareOptions#copyOutputOptions
    fn copy_output_options(&mut self) {
        let original = self.original;
        let shadow_options = &mut self.shadow_options;
        shadow_options.set_emit_use_strict(original.should_emit_use_strict());
        // Java passes null through; a new CompilerOptions already holds null there.
        if let Some(charset) = original.get_output_charset() {
            shadow_options.set_output_charset(charset);
        }

        shadow_options.set_generate_pseudo_names(original.should_generate_pseudo_names());
        shadow_options.set_line_break(original.should_add_line_break());
        shadow_options.set_line_length_threshold(original.get_line_length_threshold());
        shadow_options.set_prefer_single_quotes(original.should_prefer_single_quotes());
        shadow_options.set_pretty_print(original.is_pretty_print());

        shadow_options.set_error_format(original.get_error_format());
        if let Some(handler) = original.get_error_handler() {
            shadow_options.set_error_handler(handler.clone());
        }
        shadow_options.set_colorize_error_output(original.should_colorize_error_output());
    }

    // port: ClosureUnawareOptions#copyDebugOptions
    fn copy_debug_options(&mut self) {
        let original = self.original;
        let shadow_options = &mut self.shadow_options;
        shadow_options.set_tracer_mode(original.get_tracer_mode());
        if let Some(out) = original.get_tracer_output() {
            shadow_options.set_tracer_output(out.to_path_buf());
        }
        shadow_options.set_dev_mode(original.get_dev_mode());

        shadow_options
            .set_print_source_after_each_pass(original.should_print_source_after_each_pass());
        shadow_options.set_files_to_print_after_each_pass_regex_list(
            original
                .get_files_to_print_after_each_pass_regex_list()
                .clone(),
        );
        shadow_options.set_chunks_to_print_after_each_pass_regex_list(
            original
                .get_chunks_to_print_after_each_pass_regex_list()
                .clone(),
        );
        shadow_options.set_print_input_delimiter(original.should_print_input_delimiter());
        shadow_options.set_qname_uses_to_print_after_each_pass_list(
            original
                .get_qname_uses_to_print_after_each_pass_list()
                .clone(),
        );

        shadow_options.set_input_delimiter(original.get_input_delimiter().to_string());
        shadow_options
            .set_use_original_names_in_output(original.get_use_original_names_in_output());

        if let Some(filter) = original.get_debug_log_filter() {
            shadow_options.set_debug_log_filter(filter.to_string());
        }
        let debug_log_directory = original.get_debug_log_directory();
        if let Some(debug_log_directory) = debug_log_directory {
            shadow_options.set_debug_log_directory(Some(
                debug_log_directory.join("./TranspileAndOptimizeClosureUnaware"),
            ));
        }
    }
}
