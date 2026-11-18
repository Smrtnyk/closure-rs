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

mod support;
use closure_jscomp::{
    compilation_level::CompilationLevel,
    compiler_options::{CompilerOptions, LanguageMode},
    dependency_options::{DependencyMode, DependencyOptions},
    warning_level::WarningLevel,
};
use closure_rhino::js_string::JsString;
use closure_testing::{
    corpus::load_options_defaults,
    json::{JsonValue as Value, parse_json},
};
// port: OptionsConfigurationsDump#main
#[test]
fn every_java_configuration_matches_every_field() {
    let (_, raw_defaults) = load_options_defaults().unwrap();
    let cases = parse_json(include_str!("data/compiler_options_configs.json")).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 317);
    let mut option_count = 0;
    let mut dependency_count = 0;
    for case in cases.as_array().unwrap() {
        let config = case.get("config").unwrap();
        let kind = config
            .get("kind")
            .unwrap()
            .as_js_string()
            .unwrap()
            .to_string_lossy();
        let value = config.get("value").unwrap();
        let value_text = value.as_js_string().map(|v| v.to_string_lossy());
        if kind == "dependency" {
            dependency_count += 1;
            let mode = value_text.as_deref().and_then(DependencyMode::value_of);
            let entries = config
                .get("entries")
                .unwrap()
                .as_number()
                .unwrap()
                .as_i32()
                .unwrap();
            let legacy = config
                .get("legacy")
                .unwrap()
                .as_number()
                .unwrap()
                .as_i32()
                .unwrap();
            let ep = if entries & 1 != 0 {
                vec!["goog:entry.ns".into(), "src/../entry.js".into()]
            } else {
                vec![]
            };
            let cep = if entries & 2 != 0 {
                vec!["goog:chunk:entry.ns".into()]
            } else {
                vec![]
            };
            let common = (entries & 4 != 0).then_some("common.js");
            let result = DependencyOptions::from_flags(
                mode,
                &ep,
                &cep,
                common,
                legacy & 1 != 0,
                legacy & 2 != 0,
            );
            if case.get("error").is_some() {
                let error = result.unwrap_err().to_string();
                assert_eq!(
                    Value::str(&error),
                    *case.get("message").unwrap(),
                    "{config:?}"
                );
            } else {
                let result = result
                    .unwrap()
                    .map_or(Value::Null, |d| support::dependency_fields(&d));
                assert_eq!(result, *case.get("result").unwrap(), "{config:?}");
            }
            continue;
        }
        option_count += 1;
        let mut options = CompilerOptions::new();
        let value_text = value_text.as_deref().unwrap();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match kind.as_str() {
                "setOptionsForCompilationLevel" => CompilationLevel::value_of(value_text)
                    .unwrap()
                    .set_options_for_compilation_level(&mut options),
                "setDebugOptionsForCompilationLevel" => CompilationLevel::value_of(value_text)
                    .unwrap()
                    .set_debug_options_for_compilation_level(&mut options),
                "setTypeBasedOptimizationOptions" => CompilationLevel::value_of(value_text)
                    .unwrap()
                    .set_type_based_optimization_options(&mut options),
                "setWrappedOutputOptimizations" => CompilationLevel::value_of(value_text)
                    .unwrap()
                    .set_wrapped_output_optimizations(&mut options),
                "cli" => {
                    let level = CompilationLevel::value_of(value_text).unwrap();
                    level.set_options_for_compilation_level(&mut options);
                    level.set_debug_options_for_compilation_level(&mut options);
                    level.set_type_based_optimization_options(&mut options);
                    level.set_wrapped_output_optimizations(&mut options);
                }
                "warning" => {
                    if let Some(l) = config.get("compilation") {
                        CompilationLevel::value_of(&l.as_js_string().unwrap().to_string_lossy())
                            .unwrap()
                            .set_options_for_compilation_level(&mut options);
                    }
                    WarningLevel::value_of(value_text)
                        .unwrap()
                        .set_options_for_warning_level(&mut options);
                }
                "browser" => options.set_browser_featureset_year(value_text.parse().unwrap()),
                "setLanguage" => options.set_language(LanguageMode::value_of(value_text).unwrap()),
                "setLanguageIn" => {
                    options.set_language_in(LanguageMode::value_of(value_text).unwrap())
                }
                "setLanguageOut" => {
                    options.set_language_out(LanguageMode::value_of(value_text).unwrap())
                }
                "setEmitUseStrict" => {
                    options.set_emit_use_strict(value_text.parse().unwrap());
                }
                "setTranspilePublicClassFields" => {
                    options.set_transpile_public_class_fields(value_text.parse().unwrap())
                }
                "setDefineToBooleanLiteral" => {
                    options.set_define_to_boolean_literal("sample", value_text.parse().unwrap())
                }
                "setDefineToNumberLiteral" => {
                    options.set_define_to_number_literal("sample", value_text.parse().unwrap())
                }
                "setDefineToDoubleLiteral" => options.set_define_to_double_literal(
                    "sample",
                    match value_text {
                        "NaN" => f64::NAN,
                        "Infinity" => f64::INFINITY,
                        "-Infinity" => f64::NEG_INFINITY,
                        s => s.parse().unwrap(),
                    },
                ),
                "setDefineToStringLiteral" => options.set_define_to_string_literal(
                    "sample",
                    JsString::from_units(value.as_js_string().unwrap().as_units().to_vec()),
                ),
                _ => panic!("Unknown configuration {kind}"),
            }));
        assert_eq!(
            result.is_err(),
            case.get("error").is_some(),
            "exception for {config:?}"
        );
        let mut expected = raw_defaults.get("fields").unwrap().clone();
        if let Value::Object(fields) = &mut expected {
            fields.extend(case.get("diff").unwrap().as_object().unwrap().clone());
        }
        let mut actual = support::options_fields(&options);
        support::normalize_standins(&mut expected);
        support::normalize_standins(&mut actual);
        assert_eq!(actual.as_object().unwrap().len(), 205);
        for (name, value) in actual.as_object().unwrap() {
            assert_eq!(
                value,
                expected.get(name).unwrap(),
                "configuration {config:?}, field {name}"
            );
        }
    }
    assert_eq!(option_count, 125);
    assert_eq!(dependency_count, 192);
}
