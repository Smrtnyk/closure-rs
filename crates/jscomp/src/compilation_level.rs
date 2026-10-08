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
//   src/com/google/javascript/jscomp/CompilationLevel.java.

use crate::{
    check_level::CheckLevel,
    compiler_options::{CompilerOptions, PropertyCollapseLevel, Reach},
    dependency_options::DependencyOptions,
    diagnostic_groups,
    property_renaming_policy::PropertyRenamingPolicy,
    variable_renaming_policy::VariableRenamingPolicy,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompilationLevel {
    BUNDLE,
    WHITESPACE_ONLY,
    SIMPLE_OPTIMIZATIONS,
    ADVANCED_OPTIMIZATIONS,
    TRANSPILE_ONLY,
}
impl std::fmt::Display for CompilationLevel {
    // port: CompilationLevel#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl CompilationLevel {
    pub const VALUES: &'static [Self] = &[
        Self::BUNDLE,
        Self::WHITESPACE_ONLY,
        Self::SIMPLE_OPTIMIZATIONS,
        Self::ADVANCED_OPTIMIZATIONS,
        Self::TRANSPILE_ONLY,
    ];
    // port: CompilationLevel#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
    // port: CompilationLevel#fromString
    pub fn from_string(value: Option<&str>) -> Option<Self> {
        match value? {
            "BUNDLE" => Some(Self::BUNDLE),
            "WHITESPACE_ONLY" | "WHITESPACE" => Some(Self::WHITESPACE_ONLY),
            "TRANSPILE_ONLY" => Some(Self::TRANSPILE_ONLY),
            "SIMPLE_OPTIMIZATIONS" | "SIMPLE" => Some(Self::SIMPLE_OPTIMIZATIONS),
            "ADVANCED_OPTIMIZATIONS" | "ADVANCED" => Some(Self::ADVANCED_OPTIMIZATIONS),
            _ => None,
        }
    }
    // port: CompilationLevel#CompilationLevel
    // Java enum constructor has no body.
    // port: CompilationLevel#setOptionsForCompilationLevel
    pub fn set_options_for_compilation_level(self, options: &mut CompilerOptions) {
        match self {
            Self::BUNDLE => {}
            Self::WHITESPACE_ONLY => Self::apply_basic_compilation_options(options),
            Self::SIMPLE_OPTIMIZATIONS => Self::apply_safe_compilation_options(options),
            Self::TRANSPILE_ONLY => Self::apply_transpile_only_options(options),
            Self::ADVANCED_OPTIMIZATIONS => Self::apply_full_compilation_options(options),
        }
    }
    // port: CompilationLevel#setDebugOptionsForCompilationLevel
    pub fn set_debug_options_for_compilation_level(self, options: &mut CompilerOptions) {
        options.set_generate_pseudo_names(true);
        options.set_remove_closure_asserts(false);
        options.set_remove_j2cl_asserts(true);
    }
    // port: CompilationLevel#applyBasicCompilationOptions
    pub fn apply_basic_compilation_options(options: &mut CompilerOptions) {
        options.skip_all_compiler_passes();
    }
    // port: CompilationLevel#applyTranspileOnlyOptions
    pub fn apply_transpile_only_options(options: &mut CompilerOptions) {
        options.set_replace_id_generators(false);

        options.set_closure_pass(true);
        options.set_renaming_policy(VariableRenamingPolicy::OFF, PropertyRenamingPolicy::OFF);
        options.set_inline_variables(Reach::NONE);
        options.set_inline_functions(Reach::NONE);
        options.set_fold_constants(false);
        options.set_coalesce_variable_names(false);
        options.set_dead_assignment_elimination(false);
        options.set_collapse_variable_declarations(false);
        options.set_convert_to_dotted_properties(false);
        options.set_label_renaming(false);
        options.set_remove_unused_variables(Reach::NONE);
        options.set_collapse_object_literals(false);

        options.set_protect_hidden_side_effects(false);
    }
    // port: CompilationLevel#applySafeCompilationOptions
    pub fn apply_safe_compilation_options(options: &mut CompilerOptions) {
        options.set_dependency_options(DependencyOptions::sort_only());

        options.set_replace_id_generators(false);

        options.set_closure_pass(true);
        options.set_renaming_policy(VariableRenamingPolicy::LOCAL, PropertyRenamingPolicy::OFF);
        options.set_inline_variables(Reach::LOCAL_ONLY);
        options.set_inline_functions(Reach::LOCAL_ONLY);
        options.set_assume_closures_only_capture_references(false);
        options.set_warning_level(diagnostic_groups::GLOBAL_THIS.clone(), CheckLevel::OFF);
        options.set_fold_constants(true);
        options.set_coalesce_variable_names(true);
        options.set_dead_assignment_elimination(true);
        options.set_dead_property_assignment_elimination(false);
        options.set_collapse_variable_declarations(true);
        options.set_convert_to_dotted_properties(true);
        options.set_label_renaming(true);
        options.set_remove_unused_variables(Reach::LOCAL_ONLY);
        options.set_collapse_object_literals(true);
        options.set_protect_hidden_side_effects(true);
    }
    // port: CompilationLevel#applyFullCompilationOptions
    pub fn apply_full_compilation_options(options: &mut CompilerOptions) {
        options.set_dependency_options(DependencyOptions::sort_only());

        options.set_check_symbols(true);
        options.set_check_types(true);

        options.set_closure_pass(true);
        options.set_fold_constants(true);
        options.set_coalesce_variable_names(true);
        options.set_dead_assignment_elimination(true);
        options.set_extract_prototype_member_declarations(true);
        options.set_collapse_variable_declarations(true);
        options.set_convert_to_dotted_properties(true);
        options.set_label_renaming(true);
        options.set_collapse_object_literals(true);
        options.set_protect_hidden_side_effects(true);

        options.set_remove_closure_asserts(true);
        options.set_remove_abstract_methods(true);
        options.set_reserve_raw_exports(true);
        options.set_renaming_policy(
            VariableRenamingPolicy::ALL,
            PropertyRenamingPolicy::ALL_UNQUOTED,
        );
        options.set_remove_unused_prototype_properties(true);
        options.set_remove_unused_class_properties(true);
        options.set_collapse_anonymous_functions(true);
        options.set_collapse_properties_level(PropertyCollapseLevel::ALL);
        options.set_warning_level(diagnostic_groups::GLOBAL_THIS.clone(), CheckLevel::WARNING);
        options.set_rewrite_function_expressions(false);
        options.set_smart_name_removal(true);
        options.set_inline_constant_vars(true);
        options.set_inline_functions(Reach::ALL);
        options.set_assume_closures_only_capture_references(false);
        options.set_inline_variables(Reach::ALL);
        options.set_compute_function_side_effects(true);
        options.set_assume_strict_this(true);

        options.set_remove_unused_variables(Reach::ALL);

        options.set_cross_chunk_code_motion(true);
        options.set_cross_chunk_method_motion(true);

        options.set_devirtualize_methods(true);
        options.set_optimize_calls(true);
        options.set_optimize_es_class_constructors(true);
    }
    // port: CompilationLevel#setTypeBasedOptimizationOptions
    pub fn set_type_based_optimization_options(self, options: &mut CompilerOptions) {
        match self {
            Self::ADVANCED_OPTIMIZATIONS => {
                options.set_disambiguate_properties(true);
                options.set_ambiguate_properties(true);
                options.set_inline_properties(true);
                options.set_use_types_for_local_optimization(true);
            }
            Self::SIMPLE_OPTIMIZATIONS
            | Self::WHITESPACE_ONLY
            | Self::BUNDLE
            | Self::TRANSPILE_ONLY => {}
        }
    }
    // port: CompilationLevel#setWrappedOutputOptimizations
    pub fn set_wrapped_output_optimizations(self, options: &mut CompilerOptions) {
        options.set_reserve_raw_exports(false);
        match self {
            Self::SIMPLE_OPTIMIZATIONS => {
                options.set_variable_renaming(VariableRenamingPolicy::ALL);
                options.set_collapse_properties_level(PropertyCollapseLevel::MODULE_EXPORT);
                options.set_collapse_anonymous_functions(true);
                options.set_inline_constant_vars(true);
                options.set_inline_functions(Reach::ALL);
                options.set_inline_variables(Reach::ALL);
                options.set_remove_unused_variables(Reach::ALL);
            }
            Self::ADVANCED_OPTIMIZATIONS
            | Self::WHITESPACE_ONLY
            | Self::BUNDLE
            | Self::TRANSPILE_ONLY => {}
        }
    }
}
