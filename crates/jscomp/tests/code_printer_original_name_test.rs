/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CodePrinterTest.java.

//! The CodePrinterTest methods that print through `checkWithOriginalName`: a full
//! `Compiler#compile` (checkTypes, checkSymbols, transpile-only passes) and then a pretty print
//! with `useOriginalNamesInOutput`.
use closure_jscomp::code_printer;
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_options::{CompilerOptions, LanguageMode};
use closure_jscomp::source_file::SourceFile;
use closure_rhino::js_string::JsString;
use std::sync::Arc;

// port: CodePrinterTest#checkWithOriginalName
fn check_with_original_name(code: &str, expected_code: &str, compiler_options: &CompilerOptions) {
    let mut compiler_options = compiler_options.clone();
    compiler_options.set_check_symbols(true);
    compiler_options.set_check_types(true);
    compiler_options.set_preserve_detailed_source_info(true);
    compiler_options.set_preserve_closure_primitives(true);
    compiler_options.set_closure_pass(true);
    let mut compiler = Compiler::new();
    compiler.disable_threads();
    compiler.compile(
        &[], // Externs
        &[Arc::new(SourceFile::from_code("test", code))],
        compiler_options,
    );
    let node = compiler
        .get_root()
        .unwrap()
        .get_last_child(&compiler.ast)
        .unwrap()
        .get_first_child(&compiler.ast)
        .unwrap();

    let mut code_printer_options = CompilerOptions::new();
    code_printer_options.set_prefer_single_quotes(true);
    code_printer_options.set_line_length_threshold(80);
    code_printer_options.set_use_original_names_in_output(true);
    assert_eq!(
        code_printer::Builder::new(node)
            .set_compiler_options(&code_printer_options)
            .set_pretty_print(true)
            .set_line_break(true)
            .build(&compiler.ast),
        JsString::from(expected_code)
    );
}

fn es5_skip_all_options() -> CompilerOptions {
    let mut compiler_options = CompilerOptions::new();
    compiler_options.skip_all_compiler_passes();
    compiler_options.set_language_out(LanguageMode::ECMASCRIPT5);
    compiler_options
}

// port: CodePrinterTest#testEs6ArrowFunctionSetsOriginalNameForThis
#[test]
fn test_es6_arrow_function_sets_original_name_for_this() {
    let code = "(x)=>{this.foo[0](3);}";
    // TODO(tomnguyen): Avoid printing the `$jscomp$this$3556498$0 = this` line.
    // TODO(tomnguyen): `function(x) {` should print as an `=>` function.
    let expected_code =
        "var $jscomp$this$3556498$0 = this;\n(function(x) {\n  this.foo[0](3);\n});\n";
    check_with_original_name(code, expected_code, &es5_skip_all_options());
}

// port: CodePrinterTest#testEs6ArrowFunctionSetsOriginalNameForArguments
#[test]
fn test_es6_arrow_function_sets_original_name_for_arguments() {
    // With original names in output set, the end result is not correct code, but the "this" is
    // not rewritten.
    let code = "(x)=>{arguments[0]();}";
    let expected_code =
        "var $jscomp$arguments$3556498$0 = arguments;\n(function(x) {\n  arguments[0]();\n});\n";
    check_with_original_name(code, expected_code, &es5_skip_all_options());
}

// port: CodePrinterTest#testEscapeDollarInTemplateLiteralInOutput
#[test]
fn test_escape_dollar_in_template_literal_in_output() {
    let mut compiler_options = CompilerOptions::new();
    compiler_options.skip_all_compiler_passes();
    compiler_options.set_language_out(LanguageMode::ECMASCRIPT_2015);

    check_with_original_name(
        "let Foo; const x = `${Foo}`;",
        "let Foo;\nconst x = `${Foo}`;\n",
        &compiler_options,
    );

    check_with_original_name(
        "const x = `\\${Foo}`;",
        "const x = `\\${Foo}`;\n",
        &compiler_options,
    );

    check_with_original_name(
        "let Foo; const x = `${Foo}\\${Foo}`;",
        "let Foo;\nconst x = `${Foo}\\${Foo}`;\n",
        &compiler_options,
    );

    check_with_original_name(
        "let Foo; const x = `\\${Foo}${Foo}`;",
        "let Foo;\nconst x = `\\${Foo}${Foo}`;\n",
        &compiler_options,
    );
}

// port: CodePrinterTest#testEscapeDollarInTemplateLiteralEs5Output
#[test]
fn test_escape_dollar_in_template_literal_es5_output() {
    let compiler_options = es5_skip_all_options();

    check_with_original_name(
        "let Foo; const x = `${Foo}`;",
        "var Foo;\nvar x = '' + Foo;\n",
        &compiler_options,
    );

    check_with_original_name(
        "const x = `\\${Foo}`;",
        "var x = '${Foo}';\n",
        &compiler_options,
    );

    check_with_original_name(
        "let Foo; const x = `${Foo}\\${Foo}`;",
        "var Foo;\nvar x = Foo + '${Foo}';\n",
        &compiler_options,
    );
    check_with_original_name(
        "let Foo; const x = `\\${Foo}${Foo}`;",
        "var Foo;\nvar x = '${Foo}' + Foo;\n",
        &compiler_options,
    );
}

// port: CodePrinterTest#testDoNotEscapeDollarInRegex
#[test]
fn test_do_not_escape_dollar_in_regex() {
    let compiler_options = es5_skip_all_options();
    check_with_original_name(
        "var x = /\\$qux/;",
        "var x = /\\$qux/;\n",
        &compiler_options,
    );
    check_with_original_name("var x = /$qux/;", "var x = /$qux/;\n", &compiler_options);
}

// port: CodePrinterTest#testDoNotEscapeDollarInStringLiteral
#[test]
fn test_do_not_escape_dollar_in_string_literal() {
    let code = "var x = '\\$qux';";
    let expected_code = "var x = '$qux';\n";
    let compiler_options = es5_skip_all_options();
    check_with_original_name(code, expected_code, &compiler_options);
    check_with_original_name("var x = '\\$qux';", "var x = '$qux';\n", &compiler_options);
    check_with_original_name("var x = '$qux';", "var x = '$qux';\n", &compiler_options);
}
