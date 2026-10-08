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

use closure_jscomp::{
    closure_coding_convention::ClosureCodingConvention, coding_conventions::CodingConventions,
    compilation_level::CompilationLevel, compiler_options::CompilerOptions,
    empty_message_bundle::EmptyMessageBundle, google_coding_convention::GoogleCodingConvention,
    warning_level::WarningLevel,
};
use closure_rhino::java_lang::charset::Charset;
use std::sync::Arc;
// port: OptionsStringsDump#main
#[test]
fn java_to_string_field_order_and_values() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("data/compiler_options_strings.json")).unwrap();
    assert_eq!(rows.len(), 15);
    for row in rows {
        let mut options = CompilerOptions::new();
        let kind = row["kind"].as_str().unwrap();
        match kind {
            "default" => {}
            "DefaultCodingConvention" => {
                options.set_coding_convention(CodingConventions::get_default())
            }
            "ClosureCodingConvention" => {
                options.set_coding_convention(Arc::new(ClosureCodingConvention::new()))
            }
            "GoogleCodingConvention" => {
                options.set_coding_convention(Arc::new(GoogleCodingConvention::new()))
            }
            "defines" => {
                options.set_define_to_boolean_literal("bool", true);
                options.set_define_to_number_literal("int", -3);
                options.set_define_to_double_literal("double", 1.25);
                options.set_define_to_string_literal("string", "abc");
            }
            "charset" => options.set_output_charset(Charset::UTF_8),
            "message" => options.set_message_bundle(Arc::new(EmptyMessageBundle)),
            _ => {
                let (category, name) = kind.split_once(':').unwrap();
                match category {
                    "level" => CompilationLevel::value_of(name)
                        .unwrap()
                        .set_options_for_compilation_level(&mut options),
                    "warning" => WarningLevel::value_of(name)
                        .unwrap()
                        .set_options_for_warning_level(&mut options),
                    _ => panic!("unknown case {kind}"),
                }
            }
        }
        assert_eq!(
            options.to_string(),
            row["expected"].as_str().unwrap(),
            "{kind}"
        );
    }
}
