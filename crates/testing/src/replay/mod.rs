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

//! JSON-driven twins of the Java test harnesses.
pub mod api_passes_helpers;
pub mod ast_validator_test_helpers;
pub mod check_reg_exp_test_helpers;
pub mod closure_unaware_phase_optimizer_test_helpers;
pub mod combined_compiler_pass_natives;
pub mod create_synthetic_blocks_test_helpers;
pub mod cross_chunk;
pub mod cross_chunk_reference_collector_helpers;
pub mod disambiguate_test_helpers;
pub mod es6_rewrite_generators_test_helpers;
pub mod extern_exports_pass_test_helpers;
pub mod gather_extern_properties_test_helpers;
pub mod ijs_natives;
pub mod infer_consts_test_helpers;
pub mod inline_and_collapse_properties_helpers;
pub mod inline_functions_test_helpers;
pub mod inline_variables_helpers;
pub mod js_type_serialization_test_helpers;
pub mod locale_data_passes_test_helpers;
pub mod make_declared_names_unique_helpers;
pub mod manage_closure_unaware_code_test_helpers;
pub mod multi_pass_test_helpers;
pub mod native_access_controls;
pub mod native_closure_primitives;
pub mod native_closure_rewrite_module;
pub mod native_conformance;
pub mod native_dead_assignments;
pub mod native_es_modules;
pub mod native_finalize_rename;
pub mod native_lint;
pub mod native_peephole;
pub mod native_registry;
pub mod native_type_check;
pub mod native_type_inference;
pub mod native_unported;
pub mod node_util_test_helpers;
pub mod normalize_test_helpers;
pub mod optimize_calls_helpers;
pub mod optimize_calls_task_helpers;
pub mod options_fields;
pub mod options_values;
pub mod polyfills;
pub mod process_closure_provides_and_requires_test_helpers;
pub mod process_defines_test_helpers;
pub mod proto_values;
pub mod pure_function_identifier_helpers;
pub mod reference_collector_test_helpers;
pub mod registry;
pub mod remove_unused_code_helpers;
pub mod replace_css_names_test_helpers;
pub mod replace_id_generators_test_helpers;
pub mod replace_messages_helpers;
pub mod replace_strings_test_helpers;
pub mod replay_bridge;
pub mod replay_compiler_test;
pub mod replay_dsl;
pub mod replay_integration_test;
pub mod replay_main;
pub mod replay_options;
pub mod replay_type_check_test;
pub mod replay_values;
pub mod rewrite_dynamic_imports_test_helpers;
pub mod rewrite_goog_js_imports_helpers;
pub mod scoped_aliases_test_helpers;
pub mod serialize_typed_ast_pass_helpers;
pub mod suspicious_checks_natives;
pub mod type_check_function_check_test_helpers;
pub mod type_validator_test_helpers;
pub mod typed_ast_serializer_test_helpers;
pub mod typed_scope_creator_test_helpers;
pub mod var_checks_helpers;

pub mod alpha_integration_natives;
pub mod integration_test_helpers;
pub mod native_factories;
pub mod transpile_es2015_natives;
pub mod transpile_infra_natives;
pub mod transpile_misc_natives;
