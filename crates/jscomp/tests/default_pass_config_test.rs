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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CommandLineRunner.java.
// Ported from closure-rs' own Java oracle tooling: crates/jscomp/tests/java/CompilerPassLists.java.

use closure_jscomp::{
    check_level::CheckLevel,
    compilation_level::CompilationLevel,
    compiler_options::{CompilerOptions, J2clPassMode, LanguageMode},
    default_pass_config::DefaultPassConfig,
    diagnostic_groups::{self, DiagnosticGroups, WILDCARD_EXCLUDED_GROUPS},
    empty_message_bundle::EmptyMessageBundle,
    pass_list_builder::PassListBuilder,
    warning_level::WarningLevel,
};
use serde_json::{Value, json};
use std::sync::Arc;

// port: CommandLineRunner#createOptions (configurations in CompilerPassLists)
fn options_for_case(case: &Value) -> CompilerOptions {
    let mut options = CompilerOptions::new();
    let name = case["configuration"].as_str().unwrap();
    if let Some(level) = name.strip_prefix("default_") {
        CompilationLevel::from_string(Some(level))
            .unwrap()
            .set_options_for_compilation_level(&mut options);
        return options;
    }
    let flags: Vec<&str> = case["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    options.set_language_in(LanguageMode::STABLE);
    let language_out = flags
        .iter()
        .find_map(|f| f.strip_prefix("--language_out="))
        .unwrap_or("ECMASCRIPT_NEXT");
    options.set_language_out(LanguageMode::from_string(Some(language_out)).unwrap());
    let level = CompilationLevel::from_string(
        flags
            .iter()
            .find_map(|f| f.strip_prefix("--compilation_level=")),
    )
    .unwrap();
    level.set_options_for_compilation_level(&mut options);
    level.set_type_based_optimization_options(&mut options);
    options.set_generate_exports(true);
    options.set_export_local_property_definitions(true);
    let warning_level = flags
        .iter()
        .find_map(|f| f.strip_prefix("--warning_level="))
        .unwrap_or("DEFAULT");
    WarningLevel::value_of(warning_level)
        .unwrap()
        .set_options_for_warning_level(&mut options);
    options.set_closure_pass(true);
    options.set_j2cl_pass(J2clPassMode::AUTO);
    options.set_remove_j2cl_asserts(true);
    options.set_rewrite_polyfills(true);
    if level == CompilationLevel::ADVANCED_OPTIMIZATIONS {
        options.set_message_bundle(Arc::new(EmptyMessageBundle));
        options.set_warning_level(diagnostic_groups::MSG_CONVENTIONS.clone(), CheckLevel::OFF);
    }
    options.set_allow_dynamic_import(true);
    options.set_strict_mode_input(true);
    options.set_emit_use_strict(false);
    if flags.contains(&"--formatting=PRETTY_PRINT") {
        options.set_pretty_print(true);
    }
    if let Some(path) = flags
        .iter()
        .find_map(|f| f.strip_prefix("--create_source_map="))
    {
        options.set_source_map_output_path(path.to_owned());
    }
    if flags.contains(&"--jscomp_error=*") {
        for (name, group) in DiagnosticGroups::get_registered_groups() {
            if !WILDCARD_EXCLUDED_GROUPS.contains(&name.as_str()) {
                options.set_warning_level(group, CheckLevel::ERROR);
            }
        }
    }
    options
}
fn list(builder: PassListBuilder) -> Value {
    Value::Array(
        builder
            .build_including_unported()
            .iter()
            .map(|p| json!({"name":p.get_name(),"loop":p.is_run_in_fixed_point_loop()}))
            .collect(),
    )
}

// port: CompilerPassLists#capture
#[test]
fn every_independent_java_pass_list_matches() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("data/compiler_pass_lists.json")).unwrap();
    assert_eq!(cases.len(), 14);
    let mut compared = 0;
    for case in cases {
        let config = DefaultPassConfig::new(options_for_case(&case));
        for (key, actual) in [
            ("whitespace", list(config.get_whitespace_only_passes())),
            ("transpile", list(config.get_transpile_only_passes())),
            ("optimizations", list(config.get_optimizations())),
            ("finalizations", list(config.get_finalizations())),
        ] {
            assert_eq!(actual, case[key], "{} {key}", case["configuration"]);
            compared += 1;
        }
        if case["checks"].is_array() {
            assert_eq!(
                list(config.get_checks()),
                case["checks"],
                "{} checks",
                case["configuration"]
            );
        } else {
            let err =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| config.get_checks()))
                    .err()
                    .unwrap();
            let message = err
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap();
            assert_eq!(message, case["checks"]["error"].as_str().unwrap());
        }
        compared += 1;
    }
    assert_eq!(compared, 70);
}
