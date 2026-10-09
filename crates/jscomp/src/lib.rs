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

//! Port of com.google.javascript.jscomp (compiler, code printer, passes).
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub mod abstract_message_formatter;
pub mod allow_list_warnings_guard;
pub mod annotations;
pub mod basic_error_manager;
pub mod black_hole_error_manager;
pub mod bundle;
pub mod by_path_warnings_guard;
pub mod check_level;
pub mod closure_unaware_code_warnings_guard;
pub mod closure_unaware_options;
pub mod code_consumer;
pub mod code_generator;
pub mod code_printer;
pub mod compile_metrics_recorder_interface;
pub mod compiler_input;
pub mod compiler_options;
pub mod compose_warnings_guard;
pub mod conformance_config;
pub mod convert_chunks_to_es_modules;
pub mod coverage_instrumentation_pass;
pub mod default_name_generator;
pub mod deps;
pub mod diagnostic;
pub mod diagnostic_group;
pub mod diagnostic_group_path_suppressing_warnings_guard;
pub mod diagnostic_group_warnings_guard;
pub mod diagnostic_type;
pub mod dummy_compile_metrics_recorder;
pub mod error_format;
pub mod error_handler;
pub mod error_manager;
pub mod find_module_dependencies;
pub mod flow_sensitive_inline_variables;
pub mod gather_module_metadata;
pub mod graph;
pub mod guava_ascii;
pub mod inline_variables;
pub mod js_chunk;
pub mod js_chunk_graph;
pub mod js_comp_zip_file_cache;
pub mod js_error;
pub mod lightweight_message_formatter;
pub mod locale_data_passes;
pub mod logger_error_manager;
pub mod manage_closure_unaware_code;
pub mod message_formatter;
pub mod name_generator;
pub mod nested_compiler_runner;
pub mod optimize_calls;
pub(crate) mod parallel_parse;
pub mod platform;
pub mod prebuild_ast;
pub mod prebuild_dependency_info;
pub mod print_stream_error_manager;
pub mod print_stream_error_report_generator;
pub mod protobuf;
pub mod pruning_analysis;
pub mod region;
pub mod remove_weak_sources;
pub mod rewrite_dynamic_imports;
pub mod rewrite_json_to_module;
pub mod serialization;
pub mod show_by_path_warnings_guard;
pub mod simple_region;
pub mod sorting_error_manager;
pub mod source_excerpt_provider;
pub mod source_file;
pub mod source_file_mapping;
pub mod source_information_annotator;
pub mod source_map;
pub mod sourcemap_mapping_placeholder;
pub mod strict_warnings_guard;
pub mod testing;
pub mod thread_safe_delegating_error_manager;
pub mod transpile;
pub mod typed_code_generator;
pub mod variable_map;
pub mod verbose_message_formatter;
pub mod warnings_guard;
pub use closure_rhino::jscomp_base as base;
pub mod node_util;

pub mod abstract_compiler;
pub mod compiler;
pub use abstract_compiler::AbstractCompiler;
pub use closure_rhino::jscomp_colors as colors;
pub mod combined_compiler_pass;
pub mod compiler_pass;
pub mod compiler_state_proto;
pub mod concretize_static_inheritance_for_inlining;
pub mod pass_factory;
pub mod pass_list_builder;
pub use compiler::Compiler;
pub mod coalesce_variable_names;
pub mod coding_convention;
pub mod control_flow_analysis;
pub mod control_flow_graph;
pub mod data_flow_analysis;
pub mod dead_assignments_elimination;
pub mod dead_property_assignment_elimination;
pub mod dot_formatter;
pub mod node_traversal;
pub mod scope;
pub mod scope_creator;
pub mod var;
pub use node_util::AllVarsDeclaredInFunction;
pub mod live_variables_analysis;
pub mod maybe_reaching_variable_use;
pub mod must_be_reaching_variable_def;

pub mod abstract_scope;
pub mod abstract_var;
pub mod access_control_utils;
pub mod alias_strings;
pub mod ast_analyzer;
pub mod change_tracker;
pub mod change_verifier;
pub mod closure_coding_convention;
pub mod code_change_handler;
pub mod coding_conventions;
pub mod compilation_level;
pub mod compiler_input_provider;
pub mod css_renaming_map;
pub mod custom_pass_execution_time;
pub mod dependency_options;
pub mod diagnostic_groups;
pub mod empty_message_bundle;
pub mod flag_usage_exception;
pub mod google_coding_convention;
pub mod make_declared_names_unique;
pub mod memoized_scope_creator;
pub mod modules;
pub mod node_printing;
pub mod pass_names;
pub mod performance_tracker;
pub mod phase_optimizer;
pub mod recent_change;
pub mod reference_collector;
pub mod remove_cast_nodes;
pub mod scoped_name;
pub mod syntactic_scope_creator;
pub mod timeline;
pub mod tracer;

pub mod js;
pub mod message_bundle;
pub mod module_identifier;
pub mod pass_config;
pub mod property_renaming_policy;
pub mod renaming_map;
pub mod renaming_token;
pub mod result;
pub mod variable_renaming_policy;
pub mod xid;

pub mod source_map_input;

pub mod abstract_peephole_optimization;
pub mod ast_manipulations;
pub mod default_pass_config;
pub mod destructuring_global_name_extractor;
pub mod es6_rewrite_generators;
pub mod exploit_assigns;
pub mod inline_cost_estimator;
pub mod js_iterables;
pub mod minimized_condition;
pub mod module_renaming;
pub mod node_iterators;
pub mod optimize_let_and_const_peephole;
pub mod output_charset_encoder;
pub mod preprocessor_symbol_table;
pub mod promises;
pub mod rewrite_async_functions;
pub mod rewrite_async_iteration;
pub mod rhino_error_reporter;
pub mod statement_fusion;
pub mod transpilation_passes;
pub mod transpilation_util;
pub mod transpile_and_optimize_closure_unaware;

pub mod warning_level;

pub mod ast_validator;
pub mod basic_block;
pub mod error_pass;
pub mod forbidden_change;
pub mod gather_extern_properties;
pub mod infer_consts;
pub mod invalidating_types;
pub mod reference;
pub mod reference_collection;
pub mod reference_map;
pub mod source_info_check;
mod var_cfg_hash;

pub mod source_map_resolver;

pub mod chainable_reverse_abstract_interpreter;
pub mod destructured_target;
pub mod flow_scope;
pub mod linked_flow_scope;
pub mod module_import_resolver;
pub mod typed_scope;

pub mod compiler_executor;
pub mod compiler_options_preprocessor;

mod compiler_warnings_guard;
pub mod suppress_doc_warnings_guard;

pub mod compiler_license_tracker;
mod compiler_source_excerpt_provider;
pub mod function_argument_injector;
pub mod function_injector;
pub mod function_to_block_mutator;
pub mod inline_functions;
pub mod rename_labels;

pub mod accessor_summary;
pub mod angular_pass;
pub mod ast_factory;
pub mod closure_reverse_abstract_interpreter;
pub mod expression_decomposer;
pub mod extra_require_remover;
pub mod global_namespace;
pub mod id_generator;
pub mod index_provider;
pub mod json_error_report_generator;
pub mod jvm_metrics;
pub mod lazy_parsed_dependency_info;
pub mod optional_chain_rewriter;
pub mod reverse_abstract_interpreter;
pub mod semantic_reverse_abstract_interpreter;
pub mod type_mismatch;
pub mod type_validator;
pub mod typed_scope_creator;
pub mod unique_id_supplier;

pub mod performance_tracker_code_size_estimator;

pub mod js_doc_info_printer;

pub mod check_access_controls;
pub mod check_closure_imports;
pub mod check_conformance;
pub mod check_debugger_statement;
pub mod check_global_this;
pub mod check_jsdoc;
pub mod check_missing_requires;
pub mod check_missing_return;
pub mod check_reg_exp;
pub mod check_side_effects;
pub mod check_super;
pub mod check_suspicious_code;
pub mod check_type_import_code_references;
pub mod check_unreachable_code;
pub mod closure_check_module;
pub mod closure_code_removal;
pub mod closure_optimize_primitives;
pub mod closure_primitive_errors;
pub mod closure_rewrite_module;
pub mod collect_file_overview_visibility;
pub mod conformance_pass_config;
pub mod conformance_rules;
pub mod const_check;
pub mod create_synthetic_blocks;
pub mod es6_check_module;
pub mod export_test_functions;
pub mod extern_exports_pass;
pub mod function_rewriter;
pub mod function_type_builder;
pub mod gather_getter_and_setter_properties;
pub mod guarded_callback;
pub mod implicit_nullability_check;
pub mod inject_closure_unaware_runtime_libraries;
pub mod inject_runtime_libraries;
pub mod inject_transpilation_runtime_libraries;
pub mod inline_and_collapse_properties;
pub mod isolate_polyfills;
pub mod j2cl_checks_pass;
pub mod j2cl_equality_same_rewriter_pass;
pub mod j2cl_source_file_checker;
pub mod j2cl_source_utils;
pub mod j2cl_string_value_of_rewriter_pass;
pub mod j2cl_suppress_warnings_guard;
pub mod j2cl_undefined_checks_rewriter_pass;
pub mod js_message_visitor;
pub mod minimize_exit_points;
pub mod peephole_collect_property_assignments;
pub mod peephole_fold_constants;
pub mod peephole_minimize_conditions;
pub mod peephole_optimizations_pass;
pub mod peephole_remove_dead_code;
pub mod peephole_replace_known_methods;
pub mod peephole_substitute_alternate_syntax;
pub mod polyfill_usage_finder;
pub mod polymer_pass_errors;
pub mod process_closure_primitives;
pub mod process_common_js_modules;
pub mod process_defines;
pub mod process_tweaks;
pub mod pure_function_identifier;
pub mod remove_unused_code;
pub mod report_untranspilable_features;
pub mod rewrite_caller_code_location;
pub mod rewrite_goog_js_imports;
pub mod rewrite_polyfills;
pub mod strict_mode_check;
pub mod strip_code;
pub mod symbol_table;
pub mod template_ast_matcher;
pub mod type_check;
pub mod type_matching_strategy;
pub mod var_check;
pub mod variable_reference_check;

pub mod disambiguate {
    pub mod ambiguate_properties;
    pub mod cluster_propagator;
    pub mod color_find_property_references;
    pub mod color_graph_builder;
    pub mod color_graph_node;
    pub mod color_graph_node_factory;
    pub mod disambiguate_properties;
    pub mod invalidation;
    pub mod property_clustering;
    pub mod use_site_renamer;
}

pub mod ijs {
    pub mod class_util;
    pub mod convert_to_typed_interface;
    pub mod file_info;
    pub mod ijs_errors;
    pub mod jsdoc_util;
    pub mod potential_declaration;
    pub mod process_const_jsdoc_callback;
}

pub mod lint {
    pub mod check_array_with_goog_object;
    pub mod check_const_private_properties;
    pub mod check_constant_case_names;
    pub mod check_duplicate_case;
    pub mod check_empty_statements;
    pub mod check_enums;
    pub mod check_es6_module_file_structure;
    pub mod check_es6_modules;
    pub mod check_extra_requires;
    pub mod check_goog_module_type_script_name;
    pub mod check_interfaces;
    pub mod check_jsdoc_style;
    pub mod check_missing_semicolon;
    pub mod check_nested_names;
    pub mod check_no_mutated_es6_exports;
    pub mod check_nullability_modifiers;
    pub mod check_primitive_as_object;
    pub mod check_prototype_properties;
    pub mod check_provides_sorted;
    pub mod check_requires_sorted;
    pub mod check_unused_labels;
    pub mod check_unused_private_properties;
    pub mod check_useless_blocks;
    pub mod check_var;
}
pub mod collapse_anonymous_functions;
pub mod collapse_variable_declarations;
pub mod convert_to_dotted_properties;
pub mod es6_rewrite_scripts_to_modules;
pub mod extract_prototype_member_declarations;
pub mod gather_raw_exports;
pub mod remove_property_renaming_calls;
pub mod rename_properties;
pub mod rename_vars;
pub mod substitute_es6_syntax;
pub mod whitespace_wrap_goog_modules;

pub mod analyze_prototype_properties;
pub mod cross_chunk_code_motion;
pub mod cross_chunk_method_motion;
pub mod cross_chunk_reference_collector;
pub mod es6_relativize_import_paths;
pub mod es6_rename_type_references;
pub mod es6_rewrite_modules;
pub mod es6_rewrite_modules_to_common_js_modules;
pub mod forbid_dynamic_import_usage;

pub mod normalize;
pub mod rewrite_logical_assignment_operators_helper;
pub mod validity_check;

// Modules from port/type-check, port/global-namespace, port/transpile-infra and port/transpile-es2015.
pub mod abstract_peephole_transpilation;
pub mod denormalize;
pub mod es6_convert_super;
pub mod es6_convert_super_constructor_calls;
pub mod es6_normalize_classes;
pub mod es6_normalize_shorthand_properties;
pub mod es6_rewrite_arrow_function;
pub mod es6_rewrite_class;
pub mod es6_rewrite_destructuring;
pub mod es6_rewrite_rest_parameters;
pub mod es6_rewrite_spread_expressions;
pub mod find_exportable_nodes;
pub mod generate_exports;
pub mod google_js_message_id_generator;
pub mod icu_template_definition;
pub mod id_mapping_util;
pub mod infer_js_doc_info;
pub mod invocation_template_type_matcher;
pub mod js_message;
pub mod js_message_definition;
pub mod peephole_transpilations_pass;
pub mod process_closure_provides_and_requires;
pub mod replace_css_names;
pub mod replace_id_generators;
pub mod replace_messages;
pub mod replace_messages_constants;
pub mod replace_strings;
pub mod replace_toggles;
pub mod rewrite_catch_with_no_binding;
// chunk-output: --rename_prefix_namespace and --chunk_output_type=ES_MODULES.
pub mod rescope_global_symbols;
pub mod rescope_global_symbols_rewrite_callback;
pub mod rewrite_global_declarations_for_try_catch_wrapping;
pub mod rewrite_new_dot_target;
pub mod scoped_aliases;
pub mod synthesize_explicit_constructors;
pub mod this_and_arguments_reference_updater;
pub mod transpilation_namespace;
pub mod type_inference;
pub mod type_inference_pass;
pub mod type_transformation;
pub mod typed_var;

pub mod declared_global_externs_on_window;
pub mod remove_unnecessary_synthetic_externs;

pub mod devirtualize_methods;
pub mod inline_object_literals;
pub mod inline_properties;
pub mod inline_simple_methods;
pub mod invocations_callback;
pub mod optimize_constructors;
pub mod optimize_parameters;
pub mod optimize_returns;

// port/transpile-misc: block scoping, for-of, late ES3 conversion and ES2016-2021 lowering.
pub mod es6_for_of_converter;
pub mod es6_rewrite_block_scoped_declaration;
pub mod es6_rewrite_block_scoped_function_declaration;
pub mod es6_template_literals;
pub mod es7_rewrite_exponential_operator;
pub mod instrument_async_context;
pub mod late_es6_to_es3_converter;
pub mod rewrite_logical_assignment_operators_pass;
pub mod rewrite_nullish_coalesce_operator;
pub mod rewrite_object_spread;
pub mod rewrite_optional_chaining_operator;

// port/messages: goog.getMsg replacement and the JsMessage model.
pub mod xtb_message_bundle;

// port/access-controls: visibility/const access checks.
