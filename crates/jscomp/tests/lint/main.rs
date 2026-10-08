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

//! Ports of the Java tests under test/com/google/javascript/jscomp/lint.

#[allow(dead_code)]
mod support;

mod check_const_private_properties_test;
mod check_constant_case_names_test;
mod check_empty_statements_test;
mod check_enums_test;
mod check_es6_module_file_structure_test;
mod check_es6_modules_test;
mod check_extra_requires_test;
mod check_extra_requires_with_remove_list_test;
mod check_goog_module_type_script_name_test;
mod check_interfaces_test;
mod check_jsdoc_style_test;
mod check_missing_semicolon_test;
mod check_no_mutated_es6_exports_test;
mod check_nullability_modifiers_test;
mod check_primitive_as_object_test;
mod check_prototype_properties_test;
mod check_provides_sorted_test;
mod check_requires_sorted_test;
mod check_unused_labels_test;
mod check_unused_private_properties_test;
mod check_useless_blocks_test;
mod check_var_test;
