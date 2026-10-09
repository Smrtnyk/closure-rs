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
//   src/com/google/javascript/jscomp/CommandLineRunner.java.

use closure_jscomp::{
    Compiler,
    compilation_level::CompilationLevel,
    compiler_options::{CompilerOptions, J2clPassMode, LanguageMode},
    js_error::JSError,
    source_file::SourceFile,
    warning_level::WarningLevel,
};
use serde_json::{Value, json};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

// port: CommandLineRunner#createOptions (CompilerWhitespace's three fixed flags)
fn options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
    options.set_language_out(LanguageMode::ECMASCRIPT_NEXT);
    CompilationLevel::WHITESPACE_ONLY.set_options_for_compilation_level(&mut options);
    CompilationLevel::WHITESPACE_ONLY.set_type_based_optimization_options(&mut options);
    options.set_generate_exports(true);
    options.set_export_local_property_definitions(true);
    WarningLevel::DEFAULT.set_options_for_warning_level(&mut options);
    options.set_closure_pass(true);
    options.set_j2cl_pass(J2clPassMode::AUTO);
    options.set_remove_j2cl_asserts(true);
    options.set_rewrite_polyfills(true);
    options.set_allow_dynamic_import(true);
    options.set_strict_mode_input(true);
    options.set_emit_use_strict(false);
    // AbstractCommandLineRunner#setRunOptions
    options.set_trusted_strings(true);
    options
}
fn files(case: &Value, field: &str) -> Vec<Arc<SourceFile>> {
    case[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| {
            Arc::new(SourceFile::from_code(
                file["name"].as_str().unwrap(),
                file["code"].as_str().unwrap(),
            ))
        })
        .collect()
}
fn diagnostics(errors: &[JSError]) -> Value {
    Value::Array(
        errors
            .iter()
            .map(|error| {
                json!({
                    "type": error.get_type().key,
                    "description": error.description(),
                    "source": error.source_name(),
                    "line": error.get_line_number(),
                    "column": error.get_charno(),
                    "length": error.get_length(),
                })
            })
            .collect(),
    )
}
pub fn compile(case: &Value) -> Value {
    let mut compiler = Compiler::new_with_output_stream(Some(Box::new(std::io::sink())));
    let result = catch_unwind(AssertUnwindSafe(|| {
        compiler.compile(&files(case, "externs"), &files(case, "inputs"), options());
        let source = compiler.to_source();
        match String::from_utf16(source.as_units()) {
            Ok(text) => json!(text),
            Err(_) => json!({"utf16": source.as_units()}),
        }
    }));
    let mut result = match result {
        Ok(output) => json!({"output": output}),
        Err(error) => json!({"exception": error.downcast_ref::<String>().map(String::as_str)
            .or_else(|| error.downcast_ref::<&str>().copied()).unwrap_or("non-string panic")}),
    };
    result["errors"] = diagnostics(&compiler.get_errors());
    result["warnings"] = diagnostics(&compiler.get_warnings());
    result
}
