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
//   test/com/google/javascript/jscomp/CommandLineRunnerTest.java.

mod common;
use closure_cli::command_line_runner::CommandLineRunner;
use closure_rhino::static_source_file::StaticSourceFile;
use common::Capture;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChunkPattern {
    None,
    Chain,
    Star,
}
struct Harness {
    use_chunks: ChunkPattern,
    use_string_comparison: bool,
    args: Vec<String>,
    out_reader: Capture,
    err_reader: Capture,
    exit_codes: Arc<Mutex<Vec<i32>>>,
    last_command_line_runner: Option<CommandLineRunner>,
    filenames: indexmap::IndexMap<usize, String>,
    externs: Vec<closure_jscomp::source_file::SourceFile>,
}
impl Harness {
    // port: CommandLineRunnerTest#setUp
    fn new() -> Self {
        Self {
            use_string_comparison: false,
            use_chunks: ChunkPattern::None,
            args: Vec::new(),
            out_reader: Capture::default(),
            err_reader: Capture::default(),
            exit_codes: Arc::new(Mutex::new(vec![])),
            last_command_line_runner: None,
            filenames: indexmap::IndexMap::new(),
            externs: default_externs(),
        }
    }
    // port: CommandLineRunnerTest#createCommandLineRunner
    fn create_command_line_runner(
        &mut self,
        original: &[&str],
    ) -> Result<CommandLineRunner, closure_cli::abstract_command_line_runner::FlagUsageException>
    {
        for i in 0..original.len() {
            self.args.push("--js".into());
            self.args.push(format!("/path/to/input{i}.js"));
            if self.use_chunks != ChunkPattern::None {
                self.args.push("--chunk".into());
                let dependency = if i == 0 {
                    String::new()
                } else {
                    format!(
                        ":m{}",
                        if self.use_chunks == ChunkPattern::Chain {
                            i - 1
                        } else {
                            0
                        }
                    )
                };
                self.args.push(format!("m{i}:1{dependency}"));
            }
        }
        CommandLineRunner::new(
            &self.args,
            Box::new(std::io::empty()),
            Box::new(self.out_reader.clone()),
            Box::new(self.err_reader.clone()),
        )
    }
    // port: CommandLineRunnerTest#compile(String,List)
    fn compile(
        &mut self,
        input: &str,
        args: &[String],
    ) -> Result<String, closure_cli::abstract_command_line_runner::RunnerException> {
        let output = Capture::default();
        let mut runner = CommandLineRunner::new(
            args,
            Box::new(std::io::Cursor::new(input.as_bytes().to_vec())),
            Box::new(output.clone()),
            Box::new(Capture::default()),
        )?;
        runner.do_run()?;
        Ok(output.text())
    }
    // port: CommandLineRunnerTest#compile(String[])
    fn compile_sources(&mut self, original: &[&str]) {
        let mut runner = self.create_command_line_runner(original).unwrap();
        assert!(runner.should_run_compiler(), "{}", self.err_reader.text());
        let inputs: Vec<_> = original
            .iter()
            .enumerate()
            .map(|(i, code)| {
                closure_jscomp::source_file::SourceFile::from_code(&self.get_filename(i), *code)
            })
            .collect();
        let mut inputs = Some(inputs);
        let mut chunks = Vec::new();
        if self.use_chunks != ChunkPattern::None {
            for (i, code) in original.iter().enumerate() {
                let chunk = closure_jscomp::js_chunk::JSChunk::new(format!("m{i}"));
                chunk.add_source_file(closure_jscomp::source_file::SourceFile::from_code(
                    &format!("i{i}.js"),
                    *code,
                ));
                chunks.push(chunk);
            }
            for i in 1..chunks.len() {
                chunks[i].add_dependency(
                    &chunks[if self.use_chunks == ChunkPattern::Chain {
                        i - 1
                    } else {
                        0
                    }],
                );
            }
        }
        let mut chunks = Some(chunks);
        let codes = self.exit_codes.clone();
        let externs: Vec<_> = self
            .externs
            .iter()
            .map(|file| (file.get_name().to_owned(), file.get_code().unwrap()))
            .collect();
        runner.base.enable_test_mode(
            Box::new(move || {
                externs
                    .iter()
                    .map(|(name, code)| {
                        closure_jscomp::source_file::SourceFile::from_code(name, code.clone())
                    })
                    .collect()
            }),
            if self.use_chunks == ChunkPattern::None {
                Some(Box::new(move || inputs.take().unwrap()))
            } else {
                None
            },
            if self.use_chunks == ChunkPattern::None {
                None
            } else {
                Some(Box::new(move || chunks.take().unwrap()))
            },
            Box::new(move |exit_code| codes.lock().unwrap().push(exit_code)),
        );
        runner.run();
        self.last_command_line_runner = Some(runner);
    }
    // port: CommandLineRunnerTest#getCompiler
    fn get_compiler(&mut self) -> &mut closure_jscomp::compiler::Compiler {
        self.last_command_line_runner
            .as_mut()
            .unwrap()
            .base
            .compiler
            .as_mut()
            .unwrap()
    }
    // port: CommandLineRunnerTest#setFilename
    fn set_filename(&mut self, i: usize, filename: &str) {
        self.filenames.insert(i, filename.into());
    }
    // port: CommandLineRunnerTest#getFilename
    fn get_filename(&self, i: usize) -> String {
        if self.filenames.is_empty() {
            format!("input{i}")
        } else {
            let name = self.filenames.get(&i).unwrap();
            assert!(!name.is_empty());
            name.clone()
        }
    }
    // port: CommandLineRunnerTest#parse
    fn parse(
        &self,
        original: &[&str],
        ast: closure_rhino::node::Ast,
    ) -> (
        closure_jscomp::compiler::Compiler,
        closure_rhino::node::NodeId,
    ) {
        let runner = CommandLineRunner::new(
            &self.args,
            Box::new(std::io::empty()),
            Box::new(std::io::sink()),
            Box::new(std::io::sink()),
        )
        .unwrap();
        let mut compiler = closure_jscomp::compiler::Compiler::new();
        compiler.ast = ast;
        let inputs: Vec<_> = original
            .iter()
            .enumerate()
            .map(|(i, code)| {
                Arc::new(closure_jscomp::source_file::SourceFile::from_code(
                    &self.get_filename(i),
                    *code,
                ))
            })
            .collect();
        let externs: Vec<_> = self
            .externs
            .iter()
            .map(|file| {
                Arc::new(closure_jscomp::source_file::SourceFile::from_code(
                    file.get_name(),
                    file.get_code().unwrap(),
                ))
            })
            .collect();
        let options = runner.create_options().unwrap();
        compiler.init(&externs, &inputs, options);
        let all = compiler.parse_inputs().unwrap();
        assert!(compiler.get_errors().is_empty());
        let root = all.get_last_child(&compiler).unwrap();
        (compiler, root)
    }
    // port: CommandLineRunnerTest#test(String[], String[])
    fn test(&mut self, original: &[&str], compiled: &[&str]) {
        self.test_with_warning(original, compiled, None);
    }
    // port: CommandLineRunnerTest#test(String[], String[], DiagnosticType)
    fn test_with_warning(&mut self, original: &[&str], compiled: &[&str], warning: Option<&str>) {
        self.exit_codes.lock().unwrap().clear();
        self.compile_sources(original);
        assert_eq!(
            *self.exit_codes.lock().unwrap(),
            [0],
            "{}",
            self.err_reader.text()
        );
        if let Some(warning) = warning {
            assert_eq!(self.get_compiler().get_warnings().len(), 1);
            assert_eq!(
                self.get_compiler().get_warnings()[0].get_type().key,
                warning
            );
        } else {
            assert!(self.get_compiler().get_errors().is_empty());
            assert!(self.get_compiler().get_warnings().is_empty());
        }
        if self.use_string_comparison {
            assert_eq!(
                self.get_compiler().to_source(),
                closure_rhino::js_string::JsString::from(compiled.join(""))
            );
            return;
        }
        let root = self
            .get_compiler()
            .get_root()
            .unwrap()
            .get_last_child(self.get_compiler())
            .unwrap();
        // Put both test trees in one arena while retaining the complete NodeSubject assertion.
        let ast = std::mem::take(&mut self.get_compiler().ast);
        let (mut expected, expected_root) = self.parse(compiled, ast);
        closure_rhino::testing::node_subject::assert_node(root)
            .is_equivalent_to(&expected.ast, expected_root);
        self.get_compiler().ast = std::mem::take(&mut expected.ast);
    }
    // port: CommandLineRunnerTest#test(String[], DiagnosticType)
    fn test_diagnostic(&mut self, original: &[&str], warning: &str) {
        self.compile_sources(original);
        let errors = self.get_compiler().get_errors();
        let warnings = self.get_compiler().get_warnings();
        assert_eq!(errors.len() + warnings.len(), 1);
        let codes = self.exit_codes.lock().unwrap();
        assert!(!codes.is_empty());
        if !errors.is_empty() {
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].get_type().key, warning);
            assert_eq!(*codes.last().unwrap(), 1);
        } else {
            assert_eq!(warnings.len(), 1);
            assert_eq!(warnings[0].get_type().key, warning);
            assert_eq!(*codes.last().unwrap(), 0);
        }
    }
    // port: CommandLineRunnerTest#compileArgs
    fn compile_args(&mut self, expected_output: &str) {
        let mut runner = CommandLineRunner::new(
            &self.args,
            Box::new(std::io::empty()),
            Box::new(self.out_reader.clone()),
            Box::new(self.err_reader.clone()),
        )
        .unwrap();
        runner.do_run().unwrap();
        let compiler = runner.base.compiler.as_mut().unwrap();
        assert!(compiler.get_errors().is_empty());
        assert!(compiler.get_warnings().is_empty());
        assert_eq!(
            compiler.to_source(),
            closure_rhino::js_string::JsString::from(expected_output)
        );
        self.last_command_line_runner = Some(runner);
    }
    // port: CommandLineRunnerTest#testSame(String[])
    fn test_same(&mut self, original: &[&str]) {
        self.test(original, original);
    }
}
// port: CommandLineRunnerTest#testUnknownDiagnosticGroupOnCommandLine
#[test]
fn test_unknown_diagnostic_group_on_command_line() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=unknownDiagnosticGroup".into());
    h.exit_codes.lock().unwrap().clear();
    let mut runner = h.create_command_line_runner(&["alert(1);"]).unwrap();
    let codes = h.exit_codes.clone();
    runner
        .base
        .set_exit_code_receiver(Box::new(move |code| codes.lock().unwrap().push(code)));
    runner.run();
    assert_eq!(*h.exit_codes.lock().unwrap(), [-1]);
    assert!(
        h.err_reader
            .text()
            .contains("Unknown diagnostic group: 'unknownDiagnosticGroup'")
    );
}
// port: CommandLineRunnerTest#testReflectedMethods
#[test]
fn test_reflected_methods() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--jscomp_off=checkVars".into());
    h.test(&["/** @constructor */\nfunction Foo() {}\nFoo.prototype.handle = function(x, y) {\n  alert(y);\n};\nvar x = goog.reflect.object(Foo, {handle: 1});\nfor (var i in x) {\n  x[i].call(x);\n}\nwindow['Foo'] = Foo;\n"],&["function a() {}\na.prototype.a = function(e, d) {\n  alert(d);\n};\nvar b = goog.c.b(a, {a: 1}), c;\nfor (c in b) {\n  b[c].call(b);\n}\nwindow.Foo = a;\n"]);
}

// port: CommandLineRunnerTest#testInlineVariables
#[test]
fn test_inline_variables() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--jscomp_off=checkVars".into());
    h.test(&["/** @constructor */ function F() { this.a = 0; }\nF.prototype.inc = function() { this.a++; return 10; };\nF.prototype.bar = function() {\n  var c = 3; var val = this.inc(); this.a += val + c;\n};\nwindow['f'] = new F();\nwindow['f']['inc'] = window['f'].inc;\nwindow['f']['bar'] = window['f'].bar;\nuse(window['f'].a)\n"],&["function a(){ this.a = 0; }\na.prototype.b = function(){ this.a++; return 10; };\na.prototype.c = function(){ var b=this.b(); this.a += b + 3; };\nwindow.f = new a;\nwindow.f.inc = window.f.b;\nwindow.f.bar = window.f.c;\nuse(window.f.a);\n"]);
}

// port: CommandLineRunnerTest#testCheckSymbolsOnForVerbose
#[test]
fn test_check_symbols_on_for_verbose() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkVars".into());
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(&["x = 3;"], "JSC_UNDEFINED_VARIABLE");
    h.test_diagnostic(&["var y; var y;"], "JSC_VAR_MULTIPLY_DECLARED_ERROR");
}

// port: CommandLineRunnerTest#testCheckSymbolsOverrideForQuiet
#[test]
fn test_check_symbols_override_for_quiet() {
    let mut h = Harness::new();
    h.args.push("--warning_level=QUIET".into());
    h.args.push("--jscomp_error=undefinedVars".into());
    h.test_diagnostic(&["x = 3;"], "JSC_UNDEFINED_VARIABLE");
}

// port: CommandLineRunnerTest#testIssue81
#[test]
fn test_issue81() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--jscomp_off=checkVars".into());
    h.use_string_comparison = true;
    h.test(
        &["eval('1'); var x = eval; x('2');"],
        &["eval(\"1\");(0,eval)(\"2\");"],
    );
}

// port: CommandLineRunnerTest#testHiddenSideEffect
#[test]
fn test_hidden_side_effect() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--jscomp_off=checkVars".into());
    h.test_with_warning(
        &["element.offsetWidth;"],
        &["element.offsetWidth"],
        Some("JSC_USELESS_CODE"),
    );
}

// port: CommandLineRunnerTest#testHelpFlag
#[test]
fn test_help_flag() {
    let mut h = Harness::new();
    h.args.push("--help".into());
    let runner = h.create_command_line_runner(&["function f() {}"]).unwrap();
    assert!(!runner.should_run_compiler());
    assert!(!runner.has_errors());
    let output = h.out_reader.text();
    assert!(output.contains(" --help "));
    assert!(output.contains(" --version "));
}
// port: CommandLineRunnerTest#testHoistedFunction1
#[test]
fn test_hoisted_function1() {
    let mut h = Harness::new();
    h.args.push("--jscomp_off=es5Strict".into());
    h.args.push("-W=VERBOSE".into());
    h.test_diagnostic(
        &["if (true) { f(); function f() {} }"],
        "JSC_REFERENCE_BEFORE_DECLARE",
    );
}

// port: CommandLineRunnerTest#testVersionFlag_firstArg
#[test]
fn test_version_flag_first_arg() {
    let mut h = Harness::new();
    h.args.push("--version".into());
    let mut runner = h.create_command_line_runner(&["function f() {}"]).unwrap();
    assert!(runner.should_run_compiler());
    assert!(!runner.has_errors());
    runner.do_run().unwrap();
    assert_eq!(h.out_reader.text(), runner.get_version_text() + "\n");
}
// port: CommandLineRunnerTest#testVersionFlag_lastArg
#[test]
fn test_version_flag_last_arg() {
    let mut h = Harness::new();
    h.args.extend(["--js".into(), "/path/to/input0.js".into()]);
    // The Java helper appends lastArg after the input arguments.
    h.args.push("--version".into());
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::empty()),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    assert!(runner.should_run_compiler());
    assert!(!runner.has_errors());
    runner.do_run().unwrap();
    assert_eq!(h.out_reader.text(), runner.get_version_text() + "\n");
}
// port: CommandLineRunnerTest#testPrintAstFlag
#[test]
fn test_print_ast_flag() {
    let mut h = Harness::new();
    h.args.push("--print_ast=true".into());
    h.test_same(&[""]);
    assert_eq!(
        h.out_reader.text(),
        concat!(
            "digraph AST {\n",
            "  node [color=lightblue2, style=filled];\n",
            "  node0 [label=\"ROOT\"];\n",
            "  node1 [label=\"SCRIPT\"];\n",
            "  node0 -> node1 [weight=1];\n",
            "  node1 -> RETURN [label=\"UNCOND\", fontcolor=\"red\", weight=0.01, color=\"red\"];\n",
            "  node0 -> node1 [label=\"UNCOND\", fontcolor=\"red\", weight=0.01, color=\"red\"];\n",
            "}\n",
            "\n",
        )
    );
}
// port: CommandLineRunnerTest#browserFeaturesetYearFlag_usedWithLanguageOutFlag
#[test]
fn browser_featureset_year_flag_used_with_language_out_flag() {
    let mut h = Harness::new();
    let args = vec![
        "--browser_featureset_year=2019".into(),
        "--language_out=ECMASCRIPT_2020".into(),
    ];
    let error = h.compile("", &args).unwrap_err();
    assert!(error.0.contains(
        "ERROR - both flags `--browser_featureset_year` and `--language_out` specified."
    ));
}
// port: CommandLineRunnerTest#invalidBrowserFeaturesetYearFlagGeneratesError1
#[test]
fn invalid_browser_featureset_year_flag_generates_error1() {
    check_invalid_year(2011);
}
// port: CommandLineRunnerTest#invalidBrowserFeaturesetYearFlagGeneratesError2
#[test]
fn invalid_browser_featureset_year_flag_generates_error2() {
    check_invalid_year(2015);
}
// port: CommandLineRunnerTest#invalidBrowserFeaturesetYearFlagGeneratesError3
#[test]
fn invalid_browser_featureset_year_flag_generates_error3() {
    let mut days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400;
    let mut year = 1970;
    loop {
        let length = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < length {
            break;
        }
        days -= length;
        year += 1;
    }
    check_invalid_year(year + 1);
}
fn check_invalid_year(year: i32) {
    let mut h = Harness::new();
    let error = h
        .compile("", &[format!("--browser_featureset_year={year}")])
        .unwrap_err();
    let prefix =
        format!("Illegal browser_featureset_year={year}. We support values 2012, or 2018..");
    let tail = error
        .0
        .strip_prefix(&prefix)
        .unwrap()
        .strip_suffix(" only")
        .unwrap();
    assert_eq!(tail.len(), 4);
    assert!(tail.bytes().all(|b| b.is_ascii_digit()));
}
// port: CommandLineRunnerTest#testSyntheticExterns
#[test]
fn test_synthetic_externs() {
    let mut h = Harness::new();
    h.externs = vec![closure_jscomp::source_file::SourceFile::from_code(
        "externs",
        "function Symbol() {}; myVar.property;",
    )];
    h.test_diagnostic(
        &["var theirVar = {}; var myVar = {}; var yourVar = {};"],
        "JSC_UNDEFINED_EXTERN_VAR_ERROR",
    );
    h.args.extend([
        "--jscomp_off=externsValidation".into(),
        "--warning_level=VERBOSE".into(),
    ]);
    h.test(
        &["var theirVar = {}; var myVar = {}; var yourVar = {};"],
        &["var theirVar={},myVar={},yourVar={};"],
    );
    h.args.extend([
        "--jscomp_off=externsValidation".into(),
        "--jscomp_error=checkVars".into(),
        "--warning_level=VERBOSE".into(),
    ]);
    h.test_diagnostic(
        &["var theirVar = {}; var myVar = {}; var myVar = {};"],
        "JSC_VAR_MULTIPLY_DECLARED_ERROR",
    );
}

// port: CommandLineRunnerTest#testES3plusStrictModeChecks
#[test]
fn test_es3plus_strict_mode_checks() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT3".into());
    h.args.push("--language_out=ECMASCRIPT3".into());
    h.use_string_comparison = true;
    h.test_with_warning(
        &["var x = f.function"],
        &["var x=f[\"function\"];"],
        Some("JSC_INVALID_ES3_PROP_NAME"),
    );
    h.test_diagnostic(&["var let"], "JSC_PARSE_ERROR");
    h.test_diagnostic(&["function f(x) { delete x; }"], "JSC_DELETE_VARIABLE");
}

// port: CommandLineRunnerTest#testES5ChecksByDefault
#[test]
fn test_es5_checks_by_default() {
    let mut h = Harness::new();
    h.test_diagnostic(&["var x = 3; delete x;"], "JSC_DELETE_VARIABLE");
}

// port: CommandLineRunnerTest#testES5ChecksInVerbose
#[test]
fn test_es5_checks_in_verbose() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(&["function f(x) { delete x; }"], "JSC_DELETE_VARIABLE");
}

// port: CommandLineRunnerTest#testES5Strict
#[test]
fn test_es5_strict() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT5_STRICT".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.args.push("--emit_use_strict=true".into());
    h.test(
        &["var x = f.function"],
        &["'use strict';var x = f.function"],
    );
    h.test_diagnostic(&["var let"], "JSC_PARSE_ERROR");
    h.test_diagnostic(&["function f(x) { delete x; }"], "JSC_DELETE_VARIABLE");
}

// port: CommandLineRunnerTest#testInstrumentCodeMappingFileNotSet
#[test]
fn test_instrument_code_mapping_file_not_set() {
    let mut h = Harness::new();
    let error = h
        .compile("", &["--instrument_for_coverage_option=PRODUCTION".into()])
        .unwrap_err();
    assert_eq!(
        error.0,
        "Expected --instrument_mapping_report to be set when --instrument_for_coverage_option is set to Production"
    );
}
// port: CommandLineRunnerTest#testInstrumentCodeProductionArgNotSet
#[test]
fn test_instrument_code_production_arg_not_set() {
    let mut h = Harness::new();
    let error = h
        .compile("", &["--instrument_mapping_report=someFile.txt".into()])
        .unwrap_err();
    assert_eq!(
        error.0,
        "Expected --instrument_for_coverage_option to be passed with PRODUCTION when --instrument_mapping_report is set"
    );
}
// port: CommandLineRunnerTest#testInstrumentCodeProductionArrayArgNotSet
#[test]
fn test_instrument_code_production_array_arg_not_set() {
    let mut h = Harness::new();
    let error = h
        .compile(
            "",
            &[
                "--instrument_for_coverage_option=PRODUCTION".into(),
                "--instrument_mapping_report=someFile.txt".into(),
            ],
        )
        .unwrap_err();
    assert_eq!(
        error.0,
        "Expected --production_instrumentation_array_name to be set when --instrument_for_coverage_option is set to Production"
    );
}
// port: CommandLineRunnerTest#testOnlyClosureDependenciesEmptyEntryPoints
#[test]
fn test_only_closure_dependencies_empty_entry_points() {
    let mut h = Harness::new();
    h.args
        .extend(["--env=CUSTOM".into(), "--dependency_mode=PRUNE".into()]);
    let runner = h.create_command_line_runner(&[]).unwrap();
    assert!(runner.has_errors());
    assert!(!runner.should_run_compiler());
    assert!(
        h.err_reader
            .text()
            .contains("--dependency_mode=PRUNE requires --entry_point.")
    );
}
/// `assertThat(ImmutableSet.copyOf(mappings)).containsExactly(expected...)` with Java's
/// `Object#equals` (only a `PrefixLocationMapping` equals one).
fn assert_location_mappings_contain_exactly(
    mappings: &[std::sync::Arc<dyn closure_jscomp::source_map::LocationMapping + Send + Sync>],
    expected: &[closure_jscomp::source_map::PrefixLocationMapping],
) {
    use closure_jscomp::source_map::PrefixLocationMapping;
    // ImmutableSet.copyOf: drops later elements equal to an earlier one.
    let mut set: Vec<Option<&PrefixLocationMapping>> = Vec::new();
    for m in mappings {
        let m = m.as_any().downcast_ref::<PrefixLocationMapping>();
        if m.is_none() || !set.contains(&m) {
            set.push(m);
        }
    }
    let actual: Vec<String> = mappings.iter().map(|m| m.to_string()).collect();
    assert_eq!(set.len(), expected.len(), "mappings {actual:?}");
    for e in expected {
        assert!(set.contains(&Some(e)), "missing {e} in {actual:?}");
    }
}
// port: CommandLineRunnerTest#testSourceMapLocationsTranslations1
#[test]
fn test_source_map_locations_translations1() {
    let mut h = Harness::new();
    h.args.extend(
        [
            "--js_output_file",
            "/path/to/out.js",
            "--create_source_map=%outname%.map",
            "--source_map_location_mapping=foo/|http://bar",
        ]
        .map(str::to_string),
    );
    h.test_same(&["var x = 3;"]);

    let mappings = h
        .get_compiler()
        .get_options()
        .get_source_map_location_mappings()
        .clone();
    assert_location_mappings_contain_exactly(
        &mappings,
        &[closure_jscomp::source_map::PrefixLocationMapping::new(
            "foo/",
            "http://bar",
        )],
    );
}
// port: CommandLineRunnerTest#testSourceMapLocationsTranslations2
#[test]
fn test_source_map_locations_translations2() {
    let mut h = Harness::new();
    h.args.extend(
        [
            "--js_output_file",
            "/path/to/out.js",
            "--create_source_map=%outname%.map",
            "--source_map_location_mapping=foo/|http://bar",
            "--source_map_location_mapping=xxx/|http://yyy",
        ]
        .map(str::to_string),
    );
    h.test_same(&["var x = 3;"]);

    let mappings = h
        .get_compiler()
        .get_options()
        .get_source_map_location_mappings()
        .clone();
    assert_location_mappings_contain_exactly(
        &mappings,
        &[
            closure_jscomp::source_map::PrefixLocationMapping::new("foo/", "http://bar"),
            closure_jscomp::source_map::PrefixLocationMapping::new("xxx/", "http://yyy"),
        ],
    );
}
// port: CommandLineRunnerTest#testSourceMapLocationsTranslations3
#[test]
fn test_source_map_locations_translations3() {
    let mut h = Harness::new();
    h.args.extend(
        [
            "--env=CUSTOM",
            "--js_output_file",
            "/path/to/out.js",
            "--create_source_map=%outname%.map",
            "--source_map_location_mapping=foo/",
        ]
        .map(str::to_string),
    );
    let runner = h.create_command_line_runner(&[]).unwrap();
    assert!(!runner.should_run_compiler());
    assert!(
        h.err_reader
            .text()
            .contains("Bad value for --source_map_location_mapping")
    );
}
// port: CommandLineRunnerTest#testNoSrCFilesWithManifest
#[test]
fn test_no_sr_c_files_with_manifest() {
    let mut h = Harness::new();
    h.args
        .extend(["--env=CUSTOM".into(), "--output_manifest=test.MF".into()]);
    let mut runner = h.create_command_line_runner(&[]).unwrap();
    let error = runner.do_run().expect_err("Expected flag usage exception");
    assert_eq!(
        error.0,
        "Bad --js flag. Manifest files cannot be generated when the input is from stdin."
    );
}

fn default_externs() -> Vec<closure_jscomp::source_file::SourceFile> {
    vec![closure_jscomp::source_file::SourceFile::from_code(
        "externs",
        include_str!("data/default_externs.js"),
    )]
}
// port: CommandLineRunnerTest#testIssue504
#[test]
fn test_issue504() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test(&["void function() { alert('hi'); }();"], &["alert('hi');"]);
}

// port: CommandLineRunnerTest#testIssue601
#[test]
fn test_issue601() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=WHITESPACE_ONLY".into());
    h.test(
        &[r"function f() { return '\v' == 'v'; } window['f'] = f;"],
        &[r"function f(){return'\v'=='v'}window['f']=f"],
    );
}
// port: CommandLineRunnerTest#testSourceSortingOff
#[test]
fn test_source_sorting_off() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=WHITESPACE_ONLY".into());
    h.test_same(&["goog.require('beer');", "goog.provide('beer');"]);
}
// port: CommandLineRunnerTest#testDependencySortingWhitespaceMode
#[test]
fn test_dependency_sorting_whitespace_mode() {
    let mut h = Harness::new();
    h.args.extend(
        [
            "--dependency_mode=PRUNE_LEGACY",
            "--compilation_level=WHITESPACE_ONLY",
        ]
        .map(str::to_string),
    );
    h.test(
        &[
            "goog.require('beer');",
            "goog.provide('beer');\ngoog.require('hops');",
            "goog.provide('hops');",
        ],
        &[
            "goog.provide('hops');",
            "goog.provide('beer');\ngoog.require('hops');",
            "goog.require('beer');",
        ],
    );
}
// port: CommandLineRunnerTest#testChecksOnlyWithParseError
#[test]
fn test_checks_only_with_parse_error() {
    let mut h = Harness::new();
    h.args
        .extend(["--compilation_level=WHITESPACE_ONLY", "--checks_only"].map(str::to_string));
    h.compile_sources(&["val foo = 1;"]);
    let compiler = h.get_compiler();
    assert_eq!(
        compiler.get_errors().len() + compiler.get_warnings().len(),
        1
    );
    let errors = compiler.get_errors();
    let warnings = compiler.get_warnings();
    let error = errors.first().or_else(|| warnings.first()).unwrap();
    assert_eq!(
        error.get_type(),
        &closure_jscomp::rhino_error_reporter::PARSE_ERROR
    );
    assert!(!h.exit_codes.lock().unwrap().is_empty());
    if !errors.is_empty() {
        assert_eq!(errors.len(), 1);
        assert_eq!(*h.exit_codes.lock().unwrap().last().unwrap(), 1);
    } else {
        assert_eq!(warnings.len(), 1);
        assert_eq!(*h.exit_codes.lock().unwrap().last().unwrap(), 0);
    }
}
// port: CommandLineRunnerTest#testTwoParseErrors
#[test]
fn test_two_parse_errors() {
    let mut h = Harness::new();
    h.compile_sources(&["var a b;", "var b c;"]);
    assert_eq!(h.get_compiler().get_errors().len(), 2);
}

// port: CommandLineRunnerTest#testOutputWrapperFlag
#[test]
fn test_output_wrapper_flag() {
    let mut h = Harness::new();
    h.args.push("--output_wrapper=output".into());
    let runner = h.create_command_line_runner(&["function f() {}"]).unwrap();
    assert!(!runner.should_run_compiler());
    assert!(runner.has_errors());
}

// port: CommandLineRunnerTest#testWarningGuardOrdering1
#[test]
fn test_warning_guard_ordering1() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=globalThis".into());
    h.args.push("--jscomp_off=globalThis".into());
    h.test_same(&["function f() { this.a = 3; }"]);
}

// port: CommandLineRunnerTest#testWarningGuardOrdering3
#[test]
fn test_warning_guard_ordering3() {
    let mut h = Harness::new();
    h.args.push("--jscomp_warning=globalThis".into());
    h.args.push("--jscomp_off=globalThis".into());
    h.test_same(&["function f() { this.a = 3; }"]);
}

// port: CommandLineRunnerTest#testWarningGuardWildcardOrdering
#[test]
fn test_warning_guard_wildcard_ordering() {
    let mut h = Harness::new();
    h.args.push("--jscomp_warning=*".into());
    h.args.push("--jscomp_off=globalThis".into());
    h.test_same(&["/** @public */function f() { this.a = 3; }"]);
}

// port: CommandLineRunnerTest#testWarningGuardHideWarningsFor1
#[test]
fn test_warning_guard_hide_warnings_for1() {
    let mut h = Harness::new();
    h.args.push("--jscomp_warning=globalThis".into());
    h.args.push("--hide_warnings_for=foo/bar".into());
    h.set_filename(0, "foo/bar.baz");
    h.test_same(&["function f() { this.a = 3; }"]);
}

// port: CommandLineRunnerTest#testSimpleModeLeavesUnusedParams
#[test]
fn test_simple_mode_leaves_unused_params() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.test_same(&["window.f = function(a) {};"]);
}

// port: CommandLineRunnerTest#testAdvancedModeRemovesUnusedParams
#[test]
fn test_advanced_mode_removes_unused_params() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test(
        &["window.f = function(a) {};"],
        &["window.a = function() {};"],
    );
}

// port: CommandLineRunnerTest#testCheckGlobalThisOffByDefault
#[test]
fn test_check_global_this_off_by_default() {
    let mut h = Harness::new();
    h.test_same(&["function f() { this.a = 3; }"]);
}

// port: CommandLineRunnerTest#testCheckGlobalThisOff
#[test]
fn test_check_global_this_off() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.args.push("--jscomp_off=globalThis".into());
    h.test_same(&["function f() { this.a = 3; }"]);
}
// port: CommandLineRunnerTest#testTypeParsingOffByDefault
#[test]
fn test_type_parsing_off_by_default() {
    let mut h = Harness::new();
    h.test_same(&["/** @return {number */ function f(a) { return a; }"]);
}

// port: CommandLineRunnerTest#testTypeCheckOverride1
#[test]
fn test_type_check_override1() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.args.push("--jscomp_off=checkTypes".into());
    h.test_same(&["var x = x || {}; x.f = function() {}; x.f(3);"]);
}

// port: CommandLineRunnerTest#testCheckSymbolsOverrideForVerbose
#[test]
fn test_check_symbols_override_for_verbose() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.args.push("--jscomp_off=undefinedVars".into());
    h.test_same(&["x = 3;"]);
}

// port: CommandLineRunnerTest#testChecksOnlySkipsOptimizations
#[test]
fn test_checks_only_skips_optimizations() {
    let mut h = Harness::new();
    h.args.push("--checks_only".into());
    h.test(&["var foo = 1 + 1;"], &["var foo = 1 + 1;"]);
}

// port: CommandLineRunnerTest#testWithKeywordWithEs5ChecksOff
#[test]
fn test_with_keyword_with_es5_checks_off() {
    let mut h = Harness::new();
    h.args.push("--jscomp_off=es5Strict".into());
    h.args.push("--strict_mode_input=false".into());
    h.test_same(&["var x = {}; with (x) {}"]);
}

// port: CommandLineRunnerTest#testIsolationMode
#[test]
fn test_isolation_mode() {
    let mut h = Harness::new();
    h.args.push("--isolation_mode=IIFE".into());
    h.test_same(&["window.x = \"123\";"]);
    assert_eq!(
        h.out_reader.text(),
        "(function(){window.x=\"123\";}).call(this);\n"
    );
}

// port: CommandLineRunnerTest#testES5
#[test]
fn test_es5() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT5".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.args.push("--strict_mode_input=false".into());
    h.test(&["var x = f.function"], &["var x = f.function"]);
    h.test(&["var let"], &["var let"]);
}

// port: CommandLineRunnerTest#testIssue846
#[test]
fn test_issue846() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test_same(&["try { new Function('this is an error'); } catch(a) { alert('x'); }"]);
}

// port: CommandLineRunnerTest#testSideEffectIntegration
#[test]
fn test_side_effect_integration() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test(&["/** @constructor */\nvar Foo = function() {};\nFoo.prototype.blah = function() {\n  Foo.bar_(this)\n};\nFoo.bar_ = function(f) {\n  f.x = 5;\n};\nvar y = new Foo();\nFoo.bar_({});\n// We used to strip this too due to bad side-effect propagation.\ny.blah();\nalert(y);\n"],&["var a = new function(){}; a.a = 5; alert(a);"]);
}

// port: CommandLineRunnerTest#testDebugFlag1
#[test]
fn test_debug_flag1() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.args.push("--debug=false".into());
    h.test(&["function foo(a) {}"], &["function foo(a) {}"]);
}

// port: CommandLineRunnerTest#testEscapeDollarInTemplateLiteralInOutput
#[test]
fn test_escape_dollar_in_template_literal_in_output() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT6".into());
    h.test(
        &["let Foo; const x = `${Foo}`;"],
        &["let Foo; const x = `${Foo}`;"],
    );
    h.test(&["const x = `\\${Foo}`;"], &["const x = '\\${Foo}'"]);
    h.test(
        &["let Foo; const x = `${Foo}\\${Foo}`;"],
        &["let Foo; const x = `${Foo}\\${Foo}`;"],
    );
    h.test(
        &["let Foo; const x = `\\${Foo}${Foo}`;"],
        &["let Foo; const x = `\\${Foo}${Foo}`;"],
    );
}

// port: CommandLineRunnerTest#testWarningGuardQuotedValue
#[test]
fn test_warning_guard_quoted_value() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error='\"*\"'".into());
    h.args.push("--jscomp_warning=\"'*'\"".into());
    h.args.push("--jscomp_off='\"*\"'".into());
    h.test_same(&["alert('hello world')"]);
}
// port: CommandLineRunnerTest#testOutputSourceMapContainsSourcesContent
#[test]
fn test_output_source_map_contains_sources_content() {
    let mut h = Harness::new();
    h.args.extend([
        "--json_streams=BOTH".into(),
        "--chunk=m1:1".into(),
        "--chunk=m2:1:m1".into(),
        "--source_map_include_content".into(),
    ]);
    let mut runner = CommandLineRunner::new(&h.args, Box::new(std::io::Cursor::new("[{\"src\": \"\", \"path\": \"base.js\"},{\"src\": \"alert('foo');\", \"path\":\"foo.js\"}]".as_bytes().to_vec())), Box::new(h.out_reader.clone()), Box::new(h.err_reader.clone())).unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"\\n\",\"path\":\"./m1.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"./m1.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\";\\\",\\n\\\"sources\\\":[],\\n\\\"names\\\":[]\\n}\\n\"},{\"src\":\"alert(\\\"foo\\\");\\n\",\"path\":\"./m2.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"./m2.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAAA,KAAA,CAAM,KAAN;\\\",\\n\\\"sources\\\":[\\\"foo.js\\\"],\\n\\\"sourcesContent\\\":[\\\"alert('foo');\\\"],\\n\\\"names\\\":[\\\"alert\\\"]\\n}\\n\"}]"
    );
}

// port: CommandLineRunnerTest#testUnknownAnnotation
#[test]
fn test_unknown_annotation() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(
        &["/** @unknownTag */ function f() {}"],
        "JSC_BAD_JSDOC_ANNOTATION",
    );
    h.args.push("--extra_annotation_name=unknownTag".into());
    h.test_same(&["/** @unknownTag */ function f() {}"]);
}
// port: CommandLineRunnerTest#testQuietMode
#[test]
fn test_quiet_mode() {
    let mut h = Harness::new();
    h.args.push("--warning_level=DEFAULT".into());
    h.test_diagnostic(&["/** @const \n * @const */ var x;"], "JSC_PARSE_ERROR");
    h.args.push("--warning_level=QUIET".into());
    h.test_same(&["/** @const \n * @const */ var x;"]);
}
// port: CommandLineRunnerTest#testCssNameWiring
#[test]
fn test_css_name_wiring() {
    let mut h = Harness::new();
    h.test(&["var goog = {}; goog.getCssName = function() {};\ngoog.setCssNameMapping = function() {};\ngoog.setCssNameMapping({'goog': 'a', 'button': 'b'});\nvar a = goog.getCssName('goog-button');\nvar b = goog.getCssName('css-button');\nvar c = goog.getCssName('goog-menu');\nvar d = goog.getCssName('css-menu');\n"],&["var goog = { getCssName: function() {},\n             setCssNameMapping: function() {} },\n    a = 'a-b',\n    b = 'css-b',\n    c = 'a-menu',\n    d = 'css-menu';\n"]);
}

// port: CommandLineRunnerTest#testIssue70a
#[test]
fn test_issue70a() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT5".into());
    h.test_diagnostic(&["function foo({}) {}"], "JSC_LANGUAGE_FEATURE");
}
// port: CommandLineRunnerTest#testIssue70b
#[test]
fn test_issue70b() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT5".into());
    h.test_diagnostic(&["function foo([]) {}"], "JSC_LANGUAGE_FEATURE");
}
// port: CommandLineRunnerTest#testSourcePruningOn5
#[test]
fn test_source_pruning_on5() {
    let mut h = Harness::new();
    h.args.push("--entry_point=goog:shiraz".into());
    h.test_diagnostic(
        &[
            "goog.provide('guinness');\ngoog.require('beer');",
            "goog.provide('beer');",
            "goog.provide('scotch'); var x = 3;",
        ],
        "JSC_MISSING_ENTRY_ERROR",
    );
}
// port: CommandLineRunnerTest#testNoCompile
#[test]
fn test_no_compile() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test(
        &[
            "/** @nocompile */\ngoog.provide('x');\nvar dupeVar;\n",
            "var dupeVar;",
        ],
        &["var dupeVar;"],
    );
}
// port: CommandLineRunnerTest#testOnlyClosureDependenciesOneEntryPoint
#[test]
fn test_only_closure_dependencies_one_entry_point() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--entry_point=goog:beer".into());
    h.test(
        &[
            "goog.require('beer'); var beerRequired = 1;",
            "goog.provide('beer');\ngoog.require('hops');\nvar beerProvided = 1;",
            "goog.provide('hops'); var hopsProvided = 1;",
            "goog.provide('scotch'); var scotchProvided = 1;",
            "goog.require('scotch');\nvar includeFileWithoutProvides = 1;",
            "/** This is base.js @provideGoog */ var COMPILED = false;",
        ],
        &[
            "var COMPILED = !1;",
            "var hops = {}, hopsProvided = 1;",
            "var beer = {}, beerProvided = 1;",
        ],
    );
}

// port: CommandLineRunnerTest#testOnlyClosureDependenciesOneEntryPoint_pruneAllowNoEntryPoints
#[test]
fn test_only_closure_dependencies_one_entry_point_prune_allow_no_entry_points() {
    let mut h = Harness::new();
    h.args
        .push("--dependency_mode=PRUNE_ALLOW_NO_ENTRY_POINTS".into());
    h.args.push("--entry_point=goog:beer".into());
    h.test(
        &[
            "goog.require('beer'); var beerRequired = 1;",
            "goog.provide('beer');\ngoog.require('hops');\nvar beerProvided = 1;",
            "goog.provide('hops'); var hopsProvided = 1;",
            "goog.provide('scotch'); var scotchProvided = 1;",
            "goog.require('scotch');\nvar includeFileWithoutProvides = 1;",
            "/** This is base.js @provideGoog */ var COMPILED = false;",
        ],
        &[
            "var COMPILED = !1;",
            "var hops = {}, hopsProvided = 1;",
            "var beer = {}, beerProvided = 1;",
        ],
    );
}

// port: CommandLineRunnerTest#testPruneAllowNoEntryPoints_noEntryPointsPrunesEverything
#[test]
fn test_prune_allow_no_entry_points_no_entry_points_prunes_everything() {
    let mut h = Harness::new();
    h.args
        .push("--dependency_mode=PRUNE_ALLOW_NO_ENTRY_POINTS".into());
    h.test(
        &[
            "goog.require('beer'); var beerRequired = 1;",
            "goog.provide('beer');\ngoog.require('hops');\nvar beerProvided = 1;",
            "goog.provide('hops'); var hopsProvided = 1;",
            "goog.provide('scotch'); var scotchProvided = 1;",
            "goog.require('scotch');\nvar includeFileWithoutProvides = 1;",
        ],
        &[],
    );
}
// port: CommandLineRunnerTest#testES3
#[test]
fn test_es3() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT3".into());
    h.args.push("--language_out=ECMASCRIPT3".into());
    h.args.push("--strict_mode_input=false".into());
    h.use_string_comparison = true;
    h.test_with_warning(
        &["var x = f.function"],
        &["var x=f[\"function\"];"],
        Some("JSC_INVALID_ES3_PROP_NAME"),
    );
    h.test_same(&["var let;"]);
}
// port: CommandLineRunnerTest#testParamModification1
#[test]
fn test_param_modification1() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=ADVANCED".into());
    h.test(&["function substr (value, begin, end) {\n  return value.slice(begin, end)\n}\nwindow.bug = function (s, i) {\n  return substr(s, i, i = 5);\n}\n"],&["window.a=function(b,c){return b.slice(c,5);};"]);
}

// port: CommandLineRunnerTest#testParamModification2
#[test]
fn test_param_modification2() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=ADVANCED".into());
    h.test(&["function substr (value, begin, end) {\n  return value.slice(begin, end)\n}\nwindow.bug = function (s, i) {\n  return substr(s, i, (s='',i=5));\n}\n"],&["window.a=function(b,c){return b.slice(c,5)}"]);
}

// port: CommandLineRunnerTest#testParamModification3
#[test]
fn test_param_modification3() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=SIMPLE".into());
    h.test(&["function substr (value, begin, end) {\n  return value.slice(begin, end)\n}\nwindow.bug = function (s, i) {\n  return substr(s, i, (s='',i=5));\n}\n"],&["function substr(a,b,c){\nreturn a.slice(b,c)}\nwindow.bug=function(a,b){return substr(a,b,5)}\n"]);
}

// port: CommandLineRunnerTest#testParamModification4
#[test]
fn test_param_modification4() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=ADVANCED".into());
    h.test(&["function substr (value, begin, end, a, b, c) {\n  return value.slice(begin, end, a, b, c)\n}\nwindow.bug = function (s, i) {\n  return substr(s, i, i=5, i, i=7, i);\n}\n"],&["window.a=function(c,b){var d=b,e=b=5,f=b,g=b=7;return c.slice(d,e,f,g,b)}"]);
}

// port: CommandLineRunnerTest#testParamModification5
#[test]
fn test_param_modification5() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=ADVANCED".into());
    h.test(&["function substr (value, begin, end) {\n  return value.slice(begin, end)\n}\nwindow.bug = function (a, b, c) {\n  return substr(a, b+1, b=c);\n}\n"],&["window.a=function(b,c,d){return b.slice(c+1,d)}"]);
}

// port: CommandLineRunnerTest#testTranspileOnlyModeSyntaxError
#[test]
fn test_transpile_only_mode_syntax_error() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=TRANSPILE_ONLY".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.test_diagnostic(&["const x = {"], "JSC_PARSE_ERROR");
}

// port: CommandLineRunnerTest#testAssumeStaticInheritanceIsNotUsed
#[test]
fn test_assume_static_inheritance_is_not_used() {
    let mut h = Harness::new();
    h.test_same(&[""]);
    assert!(
        h.get_compiler()
            .get_options()
            .get_assume_static_inheritance_is_not_used()
    );
    h.args
        .push("--assume_static_inheritance_is_not_used=false".into());
    h.test_same(&[""]);
    assert!(
        !h.get_compiler()
            .get_options()
            .get_assume_static_inheritance_is_not_used()
    );
}
// port: CommandLineRunnerTest#testChunkOutputFiles
#[test]
fn test_chunk_output_files() {
    let mut h = Harness::new();
    let temp = std::env::temp_dir().join(format!("closure-cli-chunks-{}", std::process::id()));
    std::fs::create_dir_all(temp.join("in")).unwrap();
    std::fs::create_dir_all(temp.join("out")).unwrap();
    let input1 = temp.join("in/input1.js");
    let input2 = temp.join("in/input2.js");
    let source1 = "var x=1;\n";
    let source2 = "var y=2;\n";
    std::fs::write(&input1, source1).unwrap();
    std::fs::write(&input2, source2).unwrap();
    h.args.extend([
        "--chunk_output_path_prefix".into(),
        format!("{}/", temp.join("out").display()),
        "--chunk=a:1".into(),
        "--chunk=b:1".into(),
        "--js".into(),
        input1.display().to_string(),
        "--js".into(),
        input2.display().to_string(),
    ]);
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::empty()),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        std::fs::read_to_string(temp.join("out/a.js")).unwrap(),
        source1
    );
    assert_eq!(
        std::fs::read_to_string(temp.join("out/b.js")).unwrap(),
        source2
    );
    assert!(
        !temp
            .join(format!(
                "out/{}.js",
                closure_jscomp::js_chunk::JSChunk::WEAK_CHUNK_NAME
            ))
            .exists()
    );
    std::fs::remove_dir_all(temp).unwrap();
}

// port: CommandLineRunnerTest#testDuplicateParams
#[test]
fn test_duplicate_params() {
    let mut h = Harness::new();
    h.test_diagnostic(&["function f(a, a) {}"], "JSC_DUPLICATE_PARAM");
    assert!(h.get_compiler().has_halting_errors());
}
// port: CommandLineRunnerTest#googFeatureSetYearIsNotDefinedWhenBrowserFeaturesetYearFlagIsNotSupplied
#[test]
fn goog_feature_set_year_is_not_defined_when_browser_featureset_year_flag_is_not_supplied() {
    let mut h = Harness::new();
    h.test_same(&["var x = 3"]);
    let options = h.get_compiler().get_options().clone();
    assert!(
        !options
            .get_define_replacements(&mut h.get_compiler().ast)
            .contains_key("goog.FEATURESET_YEAR")
    );
}
// port: CommandLineRunnerTest#testSourceMapExpansion1
#[test]
fn test_source_map_expansion1() {
    let mut h = Harness::new();
    h.args.extend([
        "--js_output_file".into(),
        "/path/to/out.js".into(),
        "--create_source_map=%outname%.map".into(),
    ]);
    h.test_same(&["var x = 3;"]);
    let options = h.get_compiler().get_options().clone();
    assert_eq!(
        h.last_command_line_runner
            .as_mut()
            .unwrap()
            .base
            .expand_source_map_path(&options, None)
            .unwrap(),
        Some("/path/to/out.js.map".into())
    );
}
// port: CommandLineRunnerTest#testSourceMapExpansion2
#[test]
fn test_source_map_expansion2() {
    let mut h = Harness::new();
    h.use_chunks = ChunkPattern::Chain;
    h.args.extend([
        "--create_source_map=%outname%.map".into(),
        "--chunk_output_path_prefix=foo".into(),
    ]);
    h.test_same(&["var x = 3;", "var y = 5;"]);
    let options = h.get_compiler().get_options().clone();
    assert_eq!(
        h.last_command_line_runner
            .as_mut()
            .unwrap()
            .base
            .expand_source_map_path(&options, None)
            .unwrap(),
        Some("foo.map".into())
    );
}
// port: CommandLineRunnerTest#testSourceMapExpansion3
#[test]
fn test_source_map_expansion3() {
    let mut h = Harness::new();
    h.use_chunks = ChunkPattern::Chain;
    h.args.extend([
        "--create_source_map=%outname%.map".into(),
        "--chunk_output_path_prefix=foo_".into(),
    ]);
    h.test_same(&["var x = 3;", "var y = 5;"]);
    let options = h.get_compiler().get_options().clone();
    let root = h.get_compiler().get_chunk_graph().unwrap().get_root_chunk();
    assert_eq!(
        h.last_command_line_runner
            .as_mut()
            .unwrap()
            .base
            .expand_source_map_path(&options, Some(&root.get_name()))
            .unwrap(),
        Some("foo_m0.js.map".into())
    );
}
// port: CommandLineRunnerTest#testInvalidSourceMapPattern
#[test]
fn test_invalid_source_map_pattern() {
    let mut h = Harness::new();
    h.use_chunks = ChunkPattern::Chain;
    h.args.extend([
        "--create_source_map=out.map".into(),
        "--chunk_output_path_prefix=foo_".into(),
    ]);
    h.test_diagnostic(
        &["var x = 3;", "var y = 5;"],
        "JSC_INVALID_CHUNK_SOURCEMAP_PATTERN",
    );
}
// port: CommandLineRunnerTest#testSourceMapFormat1
#[test]
fn test_source_map_format1() {
    let mut h = Harness::new();
    h.args
        .extend(["--js_output_file".into(), "/path/to/out.js".into()]);
    h.test_same(&["var x = 3;"]);
    assert_eq!(
        h.get_compiler().get_options().get_source_map_format(),
        closure_jscomp::source_map::Format::DEFAULT
    );
}
// port: CommandLineRunnerTest#testSourceMapFormat2
#[test]
fn test_source_map_format2() {
    let mut h = Harness::new();
    h.args.extend([
        "--js_output_file".into(),
        "/path/to/out.js".into(),
        "--source_map_format=V3".into(),
    ]);
    h.test_same(&["var x = 3;"]);
    assert_eq!(
        h.get_compiler().get_options().get_source_map_format(),
        closure_jscomp::source_map::Format::V3
    );
}
// port: CommandLineRunnerTest#testCharSetExpansion
#[test]
fn test_char_set_expansion() {
    let mut h = Harness::new();
    h.test_same(&[""]);
    assert_eq!(
        h.get_compiler().get_options().get_output_charset(),
        Some(closure_rhino::java_lang::charset::Charset::US_ASCII)
    );
    h.args.push("--charset=UTF-8".into());
    h.test_same(&[""]);
    assert_eq!(
        h.get_compiler().get_options().get_output_charset(),
        Some(closure_rhino::java_lang::charset::Charset::UTF_8)
    );
}
// port: CommandLineRunnerTest#testProcessCJSWithPackageJsonBrowserField
#[test]
fn test_processcjs_with_package_json_browser_field() {
    let mut h = Harness::new();
    h.use_string_comparison = true;
    h.args.push("--process_common_js_modules".into());
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--entry_point=app".into());
    h.args.push("--module_resolution=NODE".into());
    h.args
        .push("--package_json_entry_names=browser,main".into());
    h.set_filename(0, "app.js");
    h.set_filename(1, "node_modules/foo/package.json");
    h.set_filename(2, "node_modules/foo/browser.js");
    h.test(&["var Foo = require('foo');","{\"browser\":\"browser.js\",\"name\":\"foo\"}","function Foo() {}\nFoo.prototype = {\n  bar: function () {\n    return 4 + 4;\n  }\n};\nmodule.exports = Foo;\n"],&["var module$node_modules$foo$browser={default:function(){}};","module$node_modules$foo$browser.default.prototype={bar:function(){return 8}};","var Foo=module$node_modules$foo$browser.default;"]);
}

// port: CommandLineRunnerTest#testFormattingSingleQuote
#[test]
fn test_formatting_single_quote() {
    let mut h = Harness::new();
    h.test_same(&["var x = '';"]);
    assert_eq!(
        h.get_compiler().to_source(),
        closure_rhino::js_string::JsString::from("var x=\"\";")
    );
    h.args.push("--formatting=SINGLE_QUOTES".into());
    h.test_same(&["var x = '';"]);
    assert_eq!(
        h.get_compiler().to_source(),
        closure_rhino::js_string::JsString::from("var x='';")
    );
}
// port: CommandLineRunnerTest#testRewriteAndIsolatePolyfills
#[test]
fn test_rewrite_and_isolate_polyfills() {
    let mut h = Harness::new();
    h.test_same(&[""]);
    assert!(h.get_compiler().get_options().get_rewrite_polyfills());
    assert!(!h.get_compiler().get_options().get_isolate_polyfills());
    h.args.push("--isolate_polyfills=true".into());
    h.test_same(&[""]);
    assert!(h.get_compiler().get_options().get_rewrite_polyfills());
    assert!(h.get_compiler().get_options().get_isolate_polyfills());
}
// port: CommandLineRunnerTest#testCrossChunkCodeMotionNoStubMethods
#[test]
fn test_cross_chunk_code_motion_no_stub_methods() {
    let mut h = Harness::new();
    h.test_same(&[""]);
    assert!(
        !h.get_compiler()
            .get_options()
            .get_cross_chunk_code_motion_no_stub_methods()
    );
    h.args
        .push("--assume_no_prototype_method_enumeration=true".into());
    h.test_same(&[""]);
    assert!(
        h.get_compiler()
            .get_options()
            .get_cross_chunk_code_motion_no_stub_methods()
    );
}
// port: CommandLineRunnerTest#testSourceMapInputs
#[test]
fn test_source_map_inputs() {
    let mut h = Harness::new();
    h.args.extend([
        "--js_output_file".into(),
        "/path/to/out.js".into(),
        "--source_map_input=input1|input1.sourcemap".into(),
        "--source_map_input=input2|input2.sourcemap".into(),
    ]);
    h.test_same(&["var x = 3;"]);
    let maps = h.get_compiler().get_options().get_input_source_maps();
    assert_eq!(maps.len(), 2);
    assert_eq!(maps["input1"].get_original_path(), "input1.sourcemap");
    assert_eq!(maps["input2"].get_original_path(), "input2.sourcemap");
}

struct TempFiles(std::path::PathBuf);
impl TempFiles {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Java's TemporaryFolder: a fresh directory under Cargo's temporary directory for tests.
        let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "closure-cli-java-tests-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    // port: CommandLineRunnerTest#createJsFile
    fn create_js_file(&self, filename: &str, contents: &str) -> String {
        let dir = self.0.join(filename);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{filename}.js"));
        std::fs::write(&path, contents).unwrap();
        path.display().to_string()
    }
    // port: CommandLineRunnerTest#createZipFile
    fn create_zip_file(&self, filename: &str, contents: &[(&str, &str)]) -> String {
        use std::io::Write;
        let path = self.0.join(format!("{filename}.js.zip"));
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, content) in contents {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        path.display().to_string()
    }
}
impl Drop for TempFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
// port: CommandLineRunnerTest#testInputOneZip
#[test]
fn test_input_one_zip() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let zip1 = files.create_zip_file("zip1", &[("run.js", "console.log(\"Hello World\");")]);
    h.args.push(format!("--jszip={zip1}"));
    h.compile_args("console.log(\"Hello World\");");
}
// port: CommandLineRunnerTest#testInputMultipleZips
#[test]
fn test_input_multiple_zips() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let zip1 = files.create_zip_file("zip1", &[("run.js", "console.log(\"Hello World\");")]);
    let zip2 = files.create_zip_file("zip2", &[("run1.js", "window.alert(\"Hi Browser\");")]);
    h.args
        .extend([format!("--jszip={zip1}"), format!("--jszip={zip2}")]);
    h.compile_args("console.log(\"Hello World\");window.alert(\"Hi Browser\");");
}
// port: CommandLineRunnerTest#testInputMultipleContents
#[test]
fn test_input_multiple_contents() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let zip1 = files.create_zip_file(
        "zip1",
        &[
            ("a.js", "console.log(\"File A\");"),
            ("b.js", "console.log(\"File B\");"),
            ("c.js", "console.log(\"File C\");"),
        ],
    );
    h.args.push(format!("--jszip={zip1}"));
    h.compile_args("console.log(\"File A\");console.log(\"File B\");console.log(\"File C\");");
}
// port: CommandLineRunnerTest#testInputMultipleFiles
#[test]
fn test_input_multiple_files() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let zip1 = files.create_zip_file("zip1", &[("run.js", "console.log(\"Hello World\");")]);
    let js1 = files.create_js_file("testjsfile", "var a;");
    let zip2 = files.create_zip_file("zip2", &[("run1.js", "window.alert(\"Hi Browser\");")]);
    h.args.extend([
        format!("--jszip={zip1}"),
        format!("--js={js1}"),
        format!("--jszip={zip2}"),
    ]);
    h.compile_args("console.log(\"Hello World\");var a;window.alert(\"Hi Browser\");");
}
// port: CommandLineRunnerTest#testInputMultipleJsFilesWithOneJsFlag
#[test]
fn test_input_multiple_js_files_with_one_js_flag() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let js3 = files.create_js_file("test3", "var c;");
    h.args.extend(["--js".into(), js3, js2, js1]);
    h.compile_args("var c;var b;var a;");
}

// port: CommandLineRunnerTest#testGlobJs1
#[test]
fn test_glob_js1() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let parent = std::path::Path::new(&js1).parent().unwrap();
    assert!(std::fs::rename(&js2, parent.join("utest2.js")).is_ok());
    let glob = format!("{}/**.js", parent.display());
    h.args.push(format!("--js={glob}"));
    h.compile_args("var a;var b;");
}

// port: CommandLineRunnerTest#testGlobJs2
#[test]
fn test_glob_js2() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let parent = std::path::Path::new(&js1).parent().unwrap();
    assert!(std::fs::rename(&js2, parent.join("utest2.js")).is_ok());
    let glob = format!("{}/*test*.js", parent.display());
    h.args.push(format!("--js={glob}"));
    h.compile_args("var a;var b;");
}

// port: CommandLineRunnerTest#testGlobJs3
#[test]
fn test_glob_js3() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let parent = std::path::Path::new(&js1).parent().unwrap();
    assert!(std::fs::rename(&js2, parent.join("test2.js")).is_ok());
    let glob = format!("{}/**.js", parent.display());
    h.args.extend([
        format!("--js={glob}"),
        format!("--js=!{}/**test2.js", parent.display()),
    ]);
    h.compile_args("var a;");
}

// port: CommandLineRunnerTest#testGlobJs4
#[test]
fn test_glob_js4() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let parent = std::path::Path::new(&js1).parent().unwrap();
    assert!(std::fs::rename(&js2, parent.join("test2.js")).is_ok());
    let glob = format!("{}/**.js", parent.display());
    h.args.extend([
        format!("--js=!{}/**test2.js", parent.display()),
        format!("--js={glob}"),
    ]);
    h.compile_args("var a;");
}

// port: CommandLineRunnerTest#testGlobJs5
#[test]
fn test_glob_js5() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let temp1 = files.0.join("dir1");
    let temp2 = files.0.join("dir2");
    std::fs::create_dir(&temp1).unwrap();
    std::fs::create_dir(&temp2).unwrap();
    let parent = std::path::Path::new(&js1).parent().unwrap();
    let new1 = parent.join("temp1");
    let new2 = parent.join("temp2");
    assert!(std::fs::rename(&temp1, &new1).is_ok());
    assert!(std::fs::rename(&temp2, &new2).is_ok());
    std::fs::rename(&js1, new1.join("test1.js")).unwrap();
    std::fs::rename(&js2, new2.join("test2.js")).unwrap();
    h.args.push(format!("--js={}/**/*.js", parent.display()));
    h.compile_args("var a;var b;");
}
// port: CommandLineRunnerTest#testSymlink
#[test]
fn test_symlink() {
    let files = TempFiles::new();
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let target = std::path::Path::new(&js1).parent().unwrap();
    let symlink = files.0.join("symlink1");
    std::os::unix::fs::symlink(target, &symlink).unwrap();
    h.args.push(format!("--js={}", symlink.display()));
    h.compile_args("var a;");
}
// port: CommandLineRunnerTest#testOutputChunkGraphJson
#[test]
fn test_output_chunk_graph_json() {
    let mut h = Harness::new();
    h.use_chunks = ChunkPattern::Star;
    h.test_same(&["var x = 3;", "var y = 5;", "var z = 7;", "var a = 9;"]);
    let mut builder = Vec::new();
    closure_cli::abstract_command_line_runner::AbstractCommandLineRunner::<
        closure_jscomp::compiler::Compiler,
        closure_jscomp::compiler_options::CompilerOptions,
    >::print_chunk_graph_json_to(h.get_compiler().get_chunk_graph().unwrap(), &mut builder)
    .unwrap();
    assert!(
        String::from_utf8(builder)
            .unwrap()
            .contains("transitive-dependencies")
    );
}

// port: CommandLineRunnerTest#testChainChunkManifest
#[test]
fn test_chain_chunk_manifest() {
    let mut h = Harness::new();
    h.use_chunks = ChunkPattern::Chain;
    h.test_same(&["var x = 3;", "var y = 5;", "var z = 7;", "var a = 9;"]);
    let chunks = h
        .get_compiler()
        .get_chunk_graph()
        .unwrap()
        .get_all_chunks()
        .to_vec();
    let mut builder = Vec::new();
    h.last_command_line_runner
        .as_mut()
        .unwrap()
        .print_chunk_graph_manifest_or_bundle_to(&chunks, &mut builder, true)
        .unwrap();
    assert_eq!(
        String::from_utf8(builder).unwrap(),
        "{m0}\ni0.js\n\n{m1:m0}\ni1.js\n\n{m2:m1}\ni2.js\n\n{m3:m2}\ni3.js\n\n{$weak$:m0,m1,m2,m3}\n"
    );
}

// port: CommandLineRunnerTest#testStarChunkManifest
#[test]
fn test_star_chunk_manifest() {
    let mut h = Harness::new();
    h.use_chunks = ChunkPattern::Star;
    h.test_same(&["var x = 3;", "var y = 5;", "var z = 7;", "var a = 9;"]);
    let chunks = h
        .get_compiler()
        .get_chunk_graph()
        .unwrap()
        .get_all_chunks()
        .to_vec();
    let mut builder = Vec::new();
    h.last_command_line_runner
        .as_mut()
        .unwrap()
        .print_chunk_graph_manifest_or_bundle_to(&chunks, &mut builder, true)
        .unwrap();
    assert_eq!(
        String::from_utf8(builder).unwrap(),
        "{m0}\ni0.js\n\n{m1:m0}\ni1.js\n\n{m2:m0}\ni2.js\n\n{m3:m0}\ni3.js\n\n{$weak$:m0,m1,m2,m3}\n"
    );
}

// port: CommandLineRunnerTest#testJsonStreamInputFlag
#[test]
fn test_json_stream_input_flag() {
    let mut h = Harness::new();
    h.args.extend(["--json_streams=IN".into()]);
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::Cursor::new(
            "[{\"src\": \"alert('foo');\", \"path\":\"foo.js\"}]"
                .as_bytes()
                .to_vec(),
        )),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        runner.base.compiler.as_mut().unwrap().to_source(),
        closure_rhino::js_string::JsString::from("alert(\"foo\");")
    );
}
// port: CommandLineRunnerTest#testJsonStreamOutputFlag
#[test]
fn test_json_stream_output_flag() {
    let mut h = Harness::new();
    h.args.extend(["--json_streams=OUT".into()]);
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::Cursor::new("alert('foo');".as_bytes().to_vec())),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"alert(\\\"foo\\\");\\n\",\"path\":\"compiled.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"compiled.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAAA,KAAA,CAAM,KAAN;\\\",\\n\\\"sources\\\":[\\\"stdin\\\"],\\n\\\"names\\\":[\\\"alert\\\"]\\n}\\n\"}]"
    );
}
// port: CommandLineRunnerTest#testJsonStreamBothFlag
#[test]
fn test_json_stream_both_flag() {
    let mut h = Harness::new();
    h.args.extend([
        "--json_streams=BOTH".into(),
        "--js_output_file=bar.js".into(),
    ]);
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::Cursor::new(
            "[{\"src\": \"alert('foo');\", \"path\":\"foo.js\"}]"
                .as_bytes()
                .to_vec(),
        )),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"alert(\\\"foo\\\");\\n\",\"path\":\"bar.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"bar.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAAA,KAAA,CAAM,KAAN;\\\",\\n\\\"sources\\\":[\\\"foo.js\\\"],\\n\\\"names\\\":[\\\"alert\\\"]\\n}\\n\"}]"
    );
}
// port: CommandLineRunnerTest#testJsonStreamSourceMap
#[test]
fn test_json_stream_source_map() {
    let mut h = Harness::new();
    h.args.extend([
        "--json_streams=BOTH".into(),
        "--js_output_file=bar.js".into(),
        "--apply_input_source_maps".into(),
    ]);
    let mut runner = CommandLineRunner::new(&h.args, Box::new(std::io::Cursor::new("[{\"src\": \"function log(a){console.log(a)}log(\\\"one.js\\\");\", \"path\":\"one.out.js\", \"sourceMap\": \"{\n\\\"version\\\":3,\n\\\"file\\\":\\\"one.out.js\\\",\n\\\"lineCount\\\":1,\n\\\"mappings\\\":\\\"AAAAA,QAASA,IAAG,CAACC,CAAD,CAAI,CACdC,OAAAF,IAAA,CAAYC,CAAZ,CADc,CAGhBD,GAAA,CAAI,QAAJ;\\\",\n\\\"sources\\\":[\\\"one.js\\\"],\n\\\"names\\\":[\\\"log\\\",\\\"a\\\",\\\"console\\\"]\n}\" }]".as_bytes().to_vec())), Box::new(h.out_reader.clone()), Box::new(h.err_reader.clone())).unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"function log(a){console.log(a)}log(\\\"one.js\\\");\\n\",\"path\":\"bar.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"bar.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAAA,QAASA,IAAG,CAACC,CAAD,CAAI,CACdC,OAAAF,CAAAA,GAAAE,CAAYD,CAAZC,CADc,CAGhBF,GAAAA,CAAI,QAAJA;\\\",\\n\\\"sources\\\":[\\\"one.js\\\"],\\n\\\"names\\\":[\\\"log\\\",\\\"a\\\",\\\"console\\\"]\\n}\\n\"}]"
    );
}
// port: CommandLineRunnerTest#testJsonStreamSourceMapUnderscore
#[test]
fn test_json_stream_source_map_underscore() {
    let mut h = Harness::new();
    h.args.extend([
        "--json_streams=BOTH".into(),
        "--js_output_file=bar.js".into(),
        "--apply_input_source_maps".into(),
    ]);
    let mut runner = CommandLineRunner::new(&h.args, Box::new(std::io::Cursor::new("[{\"src\": \"function log(a){console.log(a)}log(\\\"one.js\\\");\", \"path\":\"one.out.js\", \"source_map\": \"{\n\\\"version\\\":3,\n\\\"file\\\":\\\"one.out.js\\\",\n\\\"lineCount\\\":1,\n\\\"mappings\\\":\\\"AAAAA,QAASA,IAAG,CAACC,CAAD,CAAI,CACdC,OAAAF,IAAA,CAAYC,CAAZ,CADc,CAGhBD,GAAA,CAAI,QAAJ;\\\",\n\\\"sources\\\":[\\\"one.js\\\"],\n\\\"names\\\":[\\\"log\\\",\\\"a\\\",\\\"console\\\"]\n}\" }]".as_bytes().to_vec())), Box::new(h.out_reader.clone()), Box::new(h.err_reader.clone())).unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"function log(a){console.log(a)}log(\\\"one.js\\\");\\n\",\"path\":\"bar.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"bar.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAAA,QAASA,IAAG,CAACC,CAAD,CAAI,CACdC,OAAAF,CAAAA,GAAAE,CAAYD,CAAZC,CADc,CAGhBF,GAAAA,CAAI,QAAJA;\\\",\\n\\\"sources\\\":[\\\"one.js\\\"],\\n\\\"names\\\":[\\\"log\\\",\\\"a\\\",\\\"console\\\"]\\n}\\n\"}]"
    );
}
// port: CommandLineRunnerTest#testJsonStreamAllowsAnyChunkName
#[test]
fn test_json_stream_allows_any_chunk_name() {
    let mut h = Harness::new();
    h.args
        .extend(["--json_streams=BOTH".into(), "--chunk=foo/bar/baz:1".into()]);
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::Cursor::new(
            "[{\"src\": \"alert('foo');\", \"path\":\"foo.js\"}]"
                .as_bytes()
                .to_vec(),
        )),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"alert(\\\"foo\\\");\\n\",\"path\":\"./foo/bar/baz.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"./foo/bar/baz.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAAA,KAAA,CAAM,KAAN;\\\",\\n\\\"sources\\\":[\\\"foo.js\\\"],\\n\\\"names\\\":[\\\"alert\\\"]\\n}\\n\"}]"
    );
}
// port: CommandLineRunnerTest#testOutputModuleNaming
#[test]
fn test_output_module_naming() {
    let mut h = Harness::new();
    h.args.extend([
        "--json_streams=BOTH".into(),
        "--chunk=foo--bar.baz:1".into(),
    ]);
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::Cursor::new(
            "[{\"src\": \"alert('foo');\", \"path\":\"foo.js\"}]"
                .as_bytes()
                .to_vec(),
        )),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"alert(\\\"foo\\\");\\n\",\"path\":\"./foo--bar.baz.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"./foo--bar.baz.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAAA,KAAA,CAAM,KAAN;\\\",\\n\\\"sources\\\":[\\\"foo.js\\\"],\\n\\\"names\\\":[\\\"alert\\\"]\\n}\\n\"}]"
    );
}
// port: CommandLineRunnerTest#testES5StrictUseStrict
#[test]
fn test_es5_strict_use_strict() {
    let mut h = Harness::new();
    h.args.extend([
        "--language_in=ECMASCRIPT5_STRICT".into(),
        "--language_out=ECMASCRIPT5".into(),
        "--emit_use_strict=true".into(),
    ]);
    h.compile_sources(&["var x = f.function"]);
    assert!(
        h.get_compiler()
            .to_source()
            .starts_with(&closure_rhino::js_string::JsString::from("'use strict'"))
    );
}
// port: CommandLineRunnerTest#testES5StrictUseStrictMultipleInputs
#[test]
fn test_es5_strict_use_strict_multiple_inputs() {
    let mut h = Harness::new();
    h.args.extend([
        "--language_in=ECMASCRIPT5_STRICT".into(),
        "--language_out=ECMASCRIPT5".into(),
        "--emit_use_strict=true".into(),
    ]);
    h.compile_sources(&[
        "var x = f.function",
        "var y = f.function",
        "var z = f.function",
    ]);
    let output = h.get_compiler().to_source();
    assert!(output.starts_with(&closure_rhino::js_string::JsString::from("'use strict'")));
    assert!(
        !output
            .substring_from(13)
            .to_string_lossy()
            .contains("'use strict'")
    );
}

// port: CommandLineRunnerTest#testOutputSameAsInput
#[test]
fn test_output_same_as_input() {
    let mut h = Harness::new();
    h.args
        .push(format!("--js_output_file={}", h.get_filename(0)));
    h.test_diagnostic(&[""], "JSC_OUTPUT_SAME_AS_INPUT_ERROR");
}
// port: CommandLineRunnerTest#testChunkWrapperBaseNameExpansion
#[test]
fn test_chunk_wrapper_base_name_expansion() {
    let mut h = Harness::new();
    h.args
        .extend(["--chunk_wrapper=m0:%s // %basename%".into()]);
    h.use_chunks = ChunkPattern::Chain;
    h.test_same(&["var x = 3;", "var y = 4;"]);
    let mut tracker = closure_jscomp::compiler_license_tracker::ScriptNodeLicensesOnlyTracker::new(
        h.get_compiler(),
    );
    let chunk = h.get_compiler().get_chunk_graph().unwrap().get_root_chunk();
    let runner = h.last_command_line_runner.as_mut().unwrap();
    let filename = runner
        .base
        .get_chunk_output_file_name(&chunk.get_name())
        .unwrap();
    let mut builder = Vec::new();
    runner
        .base
        .write_chunk_output(&filename, &mut builder, &mut tracker, &chunk)
        .unwrap();
    assert_eq!(String::from_utf8(builder).unwrap(), "var x=3; // m0.js\n");
}
// port: CommandLineRunnerTest#testChunkWrapperExpansion
#[test]
fn test_chunk_wrapper_expansion() {
    let mut h = Harness::new();
    h.args
        .extend(["--chunk_wrapper=m0:%output%%n%//# SourceMappingUrl=%basename%.map".into()]);
    h.use_chunks = ChunkPattern::Chain;
    h.test_same(&["var x = 3;", "var y = 4;"]);
    let mut tracker = closure_jscomp::compiler_license_tracker::ScriptNodeLicensesOnlyTracker::new(
        h.get_compiler(),
    );
    let chunk = h.get_compiler().get_chunk_graph().unwrap().get_root_chunk();
    let runner = h.last_command_line_runner.as_mut().unwrap();
    let filename = runner
        .base
        .get_chunk_output_file_name(&chunk.get_name())
        .unwrap();
    let mut builder = Vec::new();
    runner
        .base
        .write_chunk_output(&filename, &mut builder, &mut tracker, &chunk)
        .unwrap();
    assert_eq!(
        String::from_utf8(builder).unwrap(),
        "var x=3;\n//# SourceMappingUrl=m0.js.map\n"
    );
}
// port: CommandLineRunnerTest#testBundleOutput_bundlesGoogModule
#[test]
fn test_bundle_output_bundles_goog_module() {
    let mut h = Harness::new();
    let files = TempFiles::new();
    let bundle = files.0.join("bundle.js");
    std::fs::write(&bundle, "").unwrap();
    let first = files.create_js_file("test1", "var a;");
    let second = files.create_js_file("test2", "goog.module('foo'); var b;");
    h.args.extend([
        "--compilation_level=BUNDLE".into(),
        "--dependency_mode=NONE".into(),
        "--js_output_file".into(),
        bundle.display().to_string(),
        format!("--js={first}"),
    ]);
    h.args.push(format!("--js={second}"));
    h.compile_args("");
    let output = std::fs::read_to_string(bundle).unwrap();
    assert_eq!(
        output,
        format!(
            "//{first}\nvar a;\n//{second}\ngoog.loadModule(function(exports) {{'use strict';goog.module('foo'); var b;\n;return exports;}});\n\n"
        )
    );
}
// port: CommandLineRunnerTest#testBundleOutput_ignoresSyntaxErrors
#[test]
fn test_bundle_output_ignores_syntax_errors() {
    let mut h = Harness::new();
    let files = TempFiles::new();
    let bundle = files.0.join("bundle.js");
    std::fs::write(&bundle, "").unwrap();
    let first = files.create_js_file("test1", "var a; syntax error!");
    h.args.extend([
        "--compilation_level=BUNDLE".into(),
        "--dependency_mode=NONE".into(),
        "--js_output_file".into(),
        bundle.display().to_string(),
        format!("--js={first}"),
    ]);
    h.compile_args("");
    let output = std::fs::read_to_string(bundle).unwrap();
    assert_eq!(output, format!("//{first}\nvar a; syntax error!\n"));
}

// port: CommandLineRunnerTest#testGlobJs6
#[test]
fn test_glob_js6() {
    let files = TempFiles::new();
    if std::env::var("CLOSURE_CLI_GLOB_CHILD").as_deref() != Ok("6") {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "test_glob_js6", "--nocapture"])
            .env("CLOSURE_CLI_GLOB_CHILD", "6")
            .current_dir(&files.0)
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let ignored = std::path::Path::new("./ignored.js");
    assert!(std::fs::rename(&js2, ignored).is_ok());
    let glob1 = "!**\\./ignored**.js";
    let glob2 = format!(
        "{}/**.js",
        std::path::Path::new(&js1).parent().unwrap().display()
    );
    h.args
        .extend([format!("--js={glob1}"), format!("--js={glob2}")]);
    h.compile_args("var a;");
    std::fs::remove_file(ignored).unwrap();
}
// port: CommandLineRunnerTest#testGlobJs7
#[test]
fn test_glob_js7() {
    let files = TempFiles::new();
    if std::env::var("CLOSURE_CLI_GLOB_CHILD").as_deref() != Ok("7") {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "test_glob_js7", "--nocapture"])
            .env("CLOSURE_CLI_GLOB_CHILD", "7")
            .current_dir(&files.0)
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut h = Harness::new();
    let js1 = files.create_js_file("test1", "var a;");
    let js2 = files.create_js_file("test2", "var b;");
    let taken = std::path::Path::new("./globTestTaken.js");
    let ignored = std::path::Path::new("./globTestIgnored.js");
    assert!(std::fs::rename(&js1, taken).is_ok());
    assert!(std::fs::rename(&js2, ignored).is_ok());
    let glob1 = format!("{}/**Taken.js", std::env::current_dir().unwrap().display());
    let glob2 = "!**Ignored.js";
    h.args
        .extend([format!("--js={glob1}"), format!("--js={glob2}")]);
    h.compile_args("var a;");
    std::fs::remove_file(taken).unwrap();
    std::fs::remove_file(ignored).unwrap();
}
// port: CommandLineRunnerTest#testForOfTypecheck
#[test]
fn test_for_of_typecheck() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    h.args.push("--language_in=ES6_STRICT".into());
    h.args.push("--language_out=ES3".into());
    h.externs = vec![
        closure_testing::testing::test_externs_builder::TestExternsBuilder::new()
            .add_array()
            .add_arguments()
            .build_externs_file("externs"),
    ];
    h.test_diagnostic(
        &["class Cat {meow() {}}\nclass Dog {}\n\n/** @type {!Array<!Dog>} */\nvar dogs = [];\n\nfor (var dog of dogs) {\n  dog.meow(); // type error\n}\n"],
        "JSC_INEXISTENT_PROPERTY",
    );
}
// port: CommandLineRunnerTest#testES6ImportOfFileWithoutImportsOrExports
#[test]
fn test_es6_import_of_file_without_imports_or_exports() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--entry_point='./app.js'".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.set_filename(0, "foo.js");
    h.set_filename(1, "app.js");
    h.test(
        &["function foo() { alert('foo'); }\nfoo();\n", "import './foo.js';"],
        &[
            "function foo$$module$foo(){ alert('foo'); }\nfoo$$module$foo();\n/** @const */ var module$foo = {}\n",
            "/** @const */ var module$app = {};",
        ],
    );
}
// port: CommandLineRunnerTest#testTranspileOnlyModeDoesNotDoOptimizations
#[test]
fn test_transpile_only_mode_does_not_do_optimizations() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=TRANSPILE_ONLY".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.test(
        &["const x = () => { return 1; }; const y = x();"],
        &["var x = function() {\n  return 1;\n};\nvar y = x();\n"],
    );
}
// port: CommandLineRunnerTest#testTranspileOnlyModePreservesComments
#[test]
fn test_transpile_only_mode_preserves_comments() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=TRANSPILE_ONLY".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.test(
        &["// This is a comment\nconst x = () => {};"],
        &["// This is a comment\nvar x = function() {};\n"],
    );
}
// port: CommandLineRunnerTest#testTranspileOnlyModePreservesTypeAnnotations
#[test]
fn test_transpile_only_mode_preserves_type_annotations() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=TRANSPILE_ONLY".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.test(
        &["/** @type {number} */ const x = 1;"],
        &["/** @type {number} */\nvar x = 1;\n"],
    );
}
// port: CommandLineRunnerTest#testHoistedFunction2
#[test]
fn test_hoisted_function2() {
    let mut h = Harness::new();
    h.args.push("--language_out=STABLE".into());
    h.test(
        &["if (window) { f(); function f() {} }"],
        &["if (window) { var f = function() {}; f(); }"],
    );
}
// port: CommandLineRunnerTest#testES6NotTranspiledByDefault
#[test]
fn test_es6_not_transpiled_by_default() {
    let mut h = Harness::new();
    h.test_same(&["var x = class {};"]);
    h.args.push("--language_out=STABLE".into());
    h.test(&["var x = class {};"], &["var x = function() {};"]);
}

const TEMPLATE_TAG_RUNTIME: &str = "var $jscomp=$jscomp||{};$jscomp.scope={};\n$jscomp.createTemplateTagFirstArg=function(a){\n  return $jscomp.createTemplateTagFirstArgWithRaw(a,a)\n};\n$jscomp.createTemplateTagFirstArgWithRaw=function(a,b){\n  a.raw=b;\n  Object.freeze && (Object.freeze(a), Object.freeze(b));\n  return a\n}\n";

// port: CommandLineRunnerTest#testEscapeDollarInTemplateLiteralEs5Output
#[test]
fn test_escape_dollar_in_template_literal_es5_output() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());

    h.test(
        &["let Foo; const x = `${Foo}`;"],
        &[&format!("{TEMPLATE_TAG_RUNTIME}var Foo,x=\"\"+Foo\n")],
    );

    h.test(
        &["const x = `\\${Foo}`;"],
        &[&format!("{TEMPLATE_TAG_RUNTIME}var x=\"${{Foo}}\"\n")],
    );

    h.test(
        &["let Foo; const x = `${Foo}\\${Foo}`;"],
        &[&format!(
            "{TEMPLATE_TAG_RUNTIME}var Foo,x=Foo+\"${{Foo}}\"\n"
        )],
    );
    h.test(
        &["let Foo; const x = `\\${Foo}${Foo}`;"],
        &[&format!(
            "{TEMPLATE_TAG_RUNTIME}var Foo,x=\"${{Foo}}\"+Foo\n"
        )],
    );
}

// port: CommandLineRunnerTest#testOptionalCatch
#[test]
fn test_optional_catch() {
    let mut h = Harness::new();
    h.args.push("--language_in=ECMASCRIPT_2019".into());
    h.args.push("--language_out=ECMASCRIPT_2018".into());
    h.test(&["try { x(); } catch {}"], &["try{x()}catch(a){}"]);
}

// port: CommandLineRunnerTest#testTranspileOnlyModePolyfillInjection
#[test]
fn test_transpile_only_mode_polyfill_injection() {
    let mut h = Harness::new();
    h.args.push("--compilation_level=TRANSPILE_ONLY".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.test(
        &["const arr = [1, 2, 3];\nconst found = arr.find((element) => element > 10);\n"],
        &[include_str!(
            "data/transpile_only_mode_polyfill_injection_expected.js"
        )],
    );
}

// port: CommandLineRunnerTest#test3StageCompile
#[test]
fn test3_stage_compile() {
    let h = Harness::new();
    let files = TempFiles::new();

    // Create a message bundle to use
    let msg_bundle = files.0.join("messages.xtb");
    write_file(
        &msg_bundle,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE translationbundle SYSTEM "translationbundle.dtd">
<translationbundle lang="es">
<translation id="6289482750305328564">hola</translation>
</translationbundle>
"#,
    );

    // Create test externs with a definition for goog.getMsg().
    let externs_file = files.0.join("externs.js");
    write_file(
        &externs_file,
        "/**\n * @fileoverview test externs\n * @externs\n */\nvar goog = {};\n/**\n * @nosideeffects\n * @param {string} msg\n * @param {Object=} placeholderReplacements\n * @param {Object=} options\n * @return {string}\n */\ngoog.getMsg = function(msg, placeholderReplacements, options) {};\n",
    );

    // Create an input file
    let src_file = files.0.join("input.js");
    write_file(
        &src_file,
        "/** @desc greeting */\nconst MSG_HELLO = goog.getMsg('hello');\nconsole.log(MSG_HELLO);\n",
    );

    // Create a path for the stage 1 output
    let stage1_save = files.0.join("stage1.save");
    write_file(&stage1_save, "");

    let common_flags: Vec<String> = vec![
        "--compilation_level=ADVANCED_OPTIMIZATIONS".into(),
        "--source_map_include_content".into(),
        "--translations_file".into(),
        msg_bundle.display().to_string(),
        "--externs".into(),
        externs_file.display().to_string(),
        "--js".into(),
        src_file.display().to_string(),
    ];

    // Run the compiler to generate the stage 1 save file
    let stage1_flags = create_string_list(
        &common_flags,
        &[
            "--filename_to_save_to",
            &stage1_save.display().to_string(),
            "--segment_of_compilation_to_run",
            "CHECKS",
        ],
    );
    verify_flags_are_incompatible_with_checks_only(&stage1_flags);
    let mut runner = CommandLineRunner::new(
        &stage1_flags,
        Box::new(std::io::empty()),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    assert_eq!(runner.do_run().unwrap(), 0);
    assert_eq!(h.out_reader.text(), "");

    assert_eq!(
        runner.base.compiler.as_mut().unwrap().to_source(),
        "const MSG_HELLO=goog.getMsg(\"hello\");console.log(MSG_HELLO);"
    );

    // Create a path for the stage 2 output
    let stage2_save = files.0.join("stage2.save");
    write_file(&stage2_save, "");
    // run the compiler to generate the stage 2 save file
    let stage2_flags = create_string_list(
        &common_flags,
        &[
            "--filename_to_restore_from",
            &stage1_save.display().to_string(),
            "--filename_to_save_to",
            &stage2_save.display().to_string(),
            "--segment_of_compilation_to_run",
            "OPTIMIZATIONS",
        ],
    );
    verify_flags_are_incompatible_with_checks_only(&stage2_flags);
    let mut runner = new_command_line_runner(&stage2_flags);
    assert_eq!(runner.do_run().unwrap(), 0);

    // During stage 2 the message is wrapped in a function call to protect it from mangling by
    // optimizations.
    assert_eq!(
        runner.base.compiler.as_mut().unwrap().to_source(),
        [
            "console.log(",
            "__jscomp_define_msg__({\"key\":\"MSG_HELLO\",\"msg_text\":\"hello\"})",
            ");",
        ]
        .concat()
        .as_str()
    );

    // Create a path for the final output
    let compiled_file = files.0.join("compiled.js");
    write_file(&compiled_file, "");
    // Create a path for the output source map
    let source_map_file = files.0.join("compiled.sourcemap");
    write_file(&source_map_file, "");

    // run the compiler to generate the final output
    let stage3_flags = create_string_list(
        &common_flags,
        &[
            "--filename_to_restore_from",
            &stage2_save.display().to_string(),
            "--segment_of_compilation_to_run",
            "FINALIZATIONS",
            "--js_output_file",
            &compiled_file.display().to_string(),
            "--create_source_map",
            &source_map_file.display().to_string(),
        ],
    );
    verify_flags_are_incompatible_with_checks_only(&stage3_flags);
    let mut runner = new_command_line_runner(&stage3_flags);
    assert_eq!(runner.do_run().unwrap(), 0);

    // During stage 3 the message is actually replaced and the output written to the compiled
    // output file.
    let compiled_js = std::fs::read_to_string(&compiled_file).unwrap();
    assert_eq!(compiled_js, "console.log(\"hola\");\n");

    let expected_source_map = serde_json::json!({
        "version": 3,
        "file": std::path::absolute(&compiled_file).unwrap().display().to_string(),
        "lineCount": 1,
        "mappings": "AAEAA,OAAQC,CAAAA,GAAR,CADkBC,MAClB;",
        "sources": [std::path::absolute(&src_file).unwrap().display().to_string()],
        "sourcesContent": [std::fs::read_to_string(&src_file).unwrap()],
        "names": ["console", "log", "MSG_HELLO"],
    });

    let source_map_text = std::fs::read_to_string(&source_map_file).unwrap();
    let actual_source_map: serde_json::Value = serde_json::from_str(&source_map_text).unwrap();
    assert_eq!(actual_source_map, expected_source_map);
}

/// Java `new CommandLineRunner(args)`: the runner writes to System.out and System.err.
fn new_command_line_runner(args: &[String]) -> CommandLineRunner {
    CommandLineRunner::new(
        args,
        Box::new(std::io::empty()),
        Box::new(std::io::stdout()),
        Box::new(std::io::stderr()),
    )
    .unwrap()
}

// port: CommandLineRunnerTest#verifyFlagsAreIncompatibleWithChecksOnly
/// The given flags should be incompatible with `--checks_only`.
fn verify_flags_are_incompatible_with_checks_only(flags: &[String]) {
    let additional_flag = "--checks_only";
    verify_flag_conflict_is_reported(flags, additional_flag);
}

// port: CommandLineRunnerTest#verifyFlagConflictIsReported
fn verify_flag_conflict_is_reported(flags: &[String], additional_flag: &str) {
    let combined_flags = create_string_list(flags, &[additional_flag]);
    let mut checks_only_runner = new_command_line_runner(&combined_flags);
    let error = checks_only_runner.do_run().unwrap_err();
    assert_eq!(
        error.1,
        closure_cli::abstract_command_line_runner::RunnerExceptionKind::FlagUsage
    );
}

// port: CommandLineRunnerTest#createStringList
fn create_string_list(some_strings: &[String], additional_strings: &[&str]) -> Vec<String> {
    some_strings
        .iter()
        .cloned()
        .chain(additional_strings.iter().map(|s| (*s).to_string()))
        .collect()
}

// port: CommandLineRunnerTest#writeFile
fn write_file(file: &std::path::Path, content: &str) {
    std::fs::write(file, content).unwrap();
}

// port: CommandLineRunnerTest#testStage1ErrorExitStatus
#[test]
fn test_stage1_error_exit_status() {
    let mut h = Harness::new();
    let files = TempFiles::new();
    let src = files.0.join("input.js");
    std::fs::write(&src, "/** @type {undefined} */\nconst x = 1;\n").unwrap();
    let save = files.0.join("stage1.save");
    std::fs::write(&save, "").unwrap();
    h.args.extend([
        "--jscomp_error=checkTypes".into(),
        "--js".into(),
        src.display().to_string(),
        "--filename_to_save_to".into(),
        save.display().to_string(),
        "--segment_of_compilation_to_run".into(),
        "CHECKS".into(),
    ]);
    let mut runner = CommandLineRunner::new(
        &h.args,
        Box::new(std::io::empty()),
        Box::new(h.out_reader.clone()),
        Box::new(h.err_reader.clone()),
    )
    .unwrap();
    assert_eq!(runner.do_run().unwrap(), 1);
    assert_eq!(h.out_reader.text(), "");
}

// port: CommandLineRunnerTest#testWarningGuardOrdering2
#[test]
fn test_warning_guard_ordering2() {
    let mut h = Harness::new();
    h.args.push("--jscomp_off=globalThis".into());
    h.args.push("--jscomp_error=globalThis".into());
    h.test_diagnostic(&["function f() { this.a = 3; }"], "JSC_USED_GLOBAL_THIS");
}

// port: CommandLineRunnerTest#testWarningGuardOrdering4
#[test]
fn test_warning_guard_ordering4() {
    let mut h = Harness::new();
    h.args.push("--jscomp_off=globalThis".into());
    h.args.push("--jscomp_warning=globalThis".into());
    h.test_diagnostic(&["function f() { this.a = 3; }"], "JSC_USED_GLOBAL_THIS");
}

// port: CommandLineRunnerTest#testWarningGuardWildcard1
#[test]
fn test_warning_guard_wildcard1() {
    let mut h = Harness::new();
    h.args.push("--jscomp_warning=*".into());
    h.test_diagnostic(
        &["/** @public */function f() { this.a = 3; }"],
        "JSC_USED_GLOBAL_THIS",
    );
}

// port: CommandLineRunnerTest#testWarningGuardHideWarningsFor2
#[test]
fn test_warning_guard_hide_warnings_for2() {
    let mut h = Harness::new();
    h.args.push("--jscomp_warning=globalThis".into());
    h.args.push("--hide_warnings_for=bar/baz".into());
    h.set_filename(0, "foo/bar.baz");
    h.test_diagnostic(&["function f() { this.a = 3; }"], "JSC_USED_GLOBAL_THIS");
}

// port: CommandLineRunnerTest#testCheckGlobalThisOnWithAdvancedMode
#[test]
fn test_check_global_this_on_with_advanced_mode() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test_diagnostic(&["function f() { this.a = 3; }"], "JSC_USED_GLOBAL_THIS");
}

// port: CommandLineRunnerTest#testCheckGlobalThisOnWithAdvanced
#[test]
fn test_check_global_this_on_with_advanced() {
    let mut h = Harness::new();
    h.args.push("-O=ADVANCED".into());
    h.test_diagnostic(&["function f() { this.a = 3; }"], "JSC_USED_GLOBAL_THIS");
}

// port: CommandLineRunnerTest#testCheckGlobalThisOnWithErrorFlag
#[test]
fn test_check_global_this_on_with_error_flag() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=globalThis".into());
    h.test_diagnostic(&["function f() { this.a = 3; }"], "JSC_USED_GLOBAL_THIS");
}

// port: CommandLineRunnerTest#testTypeCheckingOffByDefault
#[test]
fn test_type_checking_off_by_default() {
    let mut h = Harness::new();
    h.test(
        &["function f(x) { return x; } f();"],
        &["function f(a) { return a; } f();"],
    );
}

// port: CommandLineRunnerTest#testTypedAdvanced
#[test]
fn test_typed_advanced() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--jscomp_warning=checkTypes".into());
    h.test(&["/** @constructor */\nfunction Foo() {}\nFoo.prototype.handle1 = function(x, y) { alert(y); };\n/** @constructor */\nfunction Bar() {}\nBar.prototype.handle1 = function(x, y) {};\nnew Foo().handle1(1, 2);\nnew Bar().handle1(1, 2);\n"],&["alert(2)"]);
}

// port: CommandLineRunnerTest#testTypedDisabledAdvanced
#[test]
fn test_typed_disabled_advanced() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--use_types_for_optimization=false".into());
    h.test(&["/** @constructor */\nfunction Foo() {}\nFoo.prototype.handle1 = function(x, y) { alert(y); };\n/** @constructor */\nfunction Bar() {}\nBar.prototype.handle1 = function(x, y) {};\nnew Foo().handle1(1, 2);\nnew Bar().handle1(1, 2);\n"],&["function a() {}\na.prototype.a = function(c) { alert(c); };\nfunction b() {}\nb.prototype.a = function() {};\n(new a).a(2);\n(new b).a(2);\n"]);
}

// port: CommandLineRunnerTest#testTypeCheckingOnWithVerbose
#[test]
fn test_type_checking_on_with_verbose() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(
        &["function f(x) { return x; } f();"],
        "JSC_WRONG_ARGUMENT_COUNT",
    );
}

// port: CommandLineRunnerTest#testTypeCheckingOnWithWVerbose
#[test]
fn test_type_checking_on_withw_verbose() {
    let mut h = Harness::new();
    h.args.push("-W=VERBOSE".into());
    h.test_diagnostic(
        &["function f(x) { return x; } f();"],
        "JSC_WRONG_ARGUMENT_COUNT",
    );
}

// port: CommandLineRunnerTest#testTypeParsingOnWithVerbose
#[test]
fn test_type_parsing_on_with_verbose() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(
        &["/** @return {number */ function f(a) { return a; }"],
        "JSC_TYPE_PARSE_ERROR",
    );
    h.test_diagnostic(
        &["/** @return {n} */ function f(a) { return a; }"],
        "JSC_UNRECOGNIZED_TYPE_ERROR",
    );
}

// port: CommandLineRunnerTest#testTypeCheckOverride2
#[test]
fn test_type_check_override2() {
    let mut h = Harness::new();
    h.args.push("--warning_level=DEFAULT".into());
    h.test_same(&["var x = x || {}; x.f = function() {}; x.f(3);"]);
    h.args.push("--jscomp_warning=checkTypes".into());
    h.test_diagnostic(
        &["var x = x || {}; x.f = function() {}; x.f(3);"],
        "JSC_WRONG_ARGUMENT_COUNT",
    );
}

// port: CommandLineRunnerTest#testCheckSymbolsOffForDefault
#[test]
fn test_check_symbols_off_for_default() {
    let mut h = Harness::new();
    h.args.push("--warning_level=DEFAULT".into());
    h.test(&["x = 3; var y; var y;"], &["x=3; var y;"]);
}

// port: CommandLineRunnerTest#testCheckUndefinedProperties1
#[test]
fn test_check_undefined_properties1() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.args.push("--jscomp_error=missingProperties".into());
    h.test_diagnostic(&["var x = {}; var y = x.bar;"], "JSC_INEXISTENT_PROPERTY");
}

// port: CommandLineRunnerTest#testCheckUndefinedProperties3
#[test]
fn test_check_undefined_properties3() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(
        &["function f() {var x = {}; var y = x.bar;}"],
        "JSC_INEXISTENT_PROPERTY",
    );
}

// port: CommandLineRunnerTest#testScriptStrictModeNoWarning
#[test]
fn test_script_strict_mode_no_warning() {
    let mut h = Harness::new();
    h.test(&["'use strict';"], &[""]);
    h.test_diagnostic(&["'no use strict';"], "JSC_USELESS_CODE");
}

// port: CommandLineRunnerTest#testFunctionStrictModeNoWarning
#[test]
fn test_function_strict_mode_no_warning() {
    let mut h = Harness::new();
    h.test(&["function f() {'use strict';}"], &["function f() {}"]);
    h.test_diagnostic(&["function f() {'no use strict';}"], "JSC_USELESS_CODE");
}

// port: CommandLineRunnerTest#testProcessClosurePrimitives
#[test]
fn test_process_closure_primitives() {
    let mut h = Harness::new();
    h.test(
        &["var goog = {}; goog.provide('goog.dom');"],
        &["var goog = {dom:{}};"],
    );
    h.args.push("--process_closure_primitives=false".into());
    h.test_same(&["var goog = {}; goog.provide('goog.dom');"]);
}

// port: CommandLineRunnerTest#testGetMsgWiring
#[test]
fn test_get_msg_wiring() {
    let mut h = Harness::new();
    h.test(&["var goog = {}; goog.getMsg = function(x) { return x; };\n/** @desc A real foo. */ var MSG_FOO = goog.getMsg('foo');\n"],&["var goog={getMsg:function(a){return a}}, MSG_FOO=goog.getMsg('foo');\n"]);
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test(&["var goog = {}; goog.getMsg = function(x) { return x; };\n/** @desc A real foo. */ var MSG_FOO = goog.getMsg('foo');\nwindow['foo'] = MSG_FOO;\n"],&["window.foo = 'foo';"]);
}

// port: CommandLineRunnerTest#testGetMsgWiringNoWarnings
#[test]
fn test_get_msg_wiring_no_warnings() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test(&["/** @desc A bad foo. */ var MSG_FOO = 1;"], &[""]);
}

// port: CommandLineRunnerTest#testIssue115
#[test]
fn test_issue115() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.args.push("--jscomp_off=es5Strict".into());
    h.args.push("--strict_mode_input=false".into());
    h.args.push("--warning_level=VERBOSE".into());
    h.test(&["function f() {\n  var arguments = Array.prototype.slice.call(arguments, 0);\n  return arguments[0];\n}\n"],&["function f() {\n  arguments = Array.prototype.slice.call(arguments, 0);\n  return arguments[0];\n}\n"]);
}

// port: CommandLineRunnerTest#testIssue297
#[test]
fn test_issue297() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.test(&["function f(p) {\n  var x;\n  return ((x=p.id) && (x=parseInt(x.substr(1)))) && x>0;\n}\n"],&["function f(b) {\n  var a;\n  return ((a=b.id) && (a=parseInt(a.substr(1)))) && a>0;\n}\n"]);
}

// port: CommandLineRunnerTest#testIssue601b
#[test]
fn test_issue601b() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test(
        &["function f() { return '\\v' == 'v'; } window['f'] = f;"],
        &["window.f=function(){return'\\v'=='v'}"],
    );
}

// port: CommandLineRunnerTest#testIssue601c
#[test]
fn test_issue601c() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.test(
        &["function f() { return '\\u000B' == 'v'; } window['f'] = f;"],
        &["window.f=function(){return'\\u000B'=='v'}"],
    );
}

// port: CommandLineRunnerTest#testDebugFlag2
#[test]
fn test_debug_flag2() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.args.push("--debug=true".into());
    h.test(
        &["function foo(a) {alert(a)}"],
        &["function foo($a$$) {alert($a$$)}"],
    );
}

// port: CommandLineRunnerTest#testDebugFlag3
#[test]
fn test_debug_flag3() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--warning_level=QUIET".into());
    h.args.push("--debug=false".into());
    h.test(
        &["function Foo() {}\nFoo.x = 1;\nfunction f() {throw new Foo().x;} f();\n"],
        &["throw (new function() {}).a;"],
    );
}

// port: CommandLineRunnerTest#testDebugFlag4
#[test]
fn test_debug_flag4() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--warning_level=QUIET".into());
    h.args.push("--debug=true".into());
    h.test(
        &["function Foo() {}\nFoo.x = 1;\nfunction f() {throw new Foo().x;} f();\n"],
        &["throw (new function() {}).$x$;"],
    );
}

// port: CommandLineRunnerTest#testBooleanFlag1
#[test]
fn test_boolean_flag1() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.args.push("--debug".into());
    h.test(
        &["function foo(a) {alert(a)}"],
        &["function foo($a$$) {alert($a$$)}"],
    );
}

// port: CommandLineRunnerTest#testBooleanFlag2
#[test]
fn test_boolean_flag2() {
    let mut h = Harness::new();
    h.args.push("--debug".into());
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.test(
        &["function foo(a) {alert(a)}"],
        &["function foo($a$$) {alert($a$$)}"],
    );
}

// port: CommandLineRunnerTest#testExternsLifting1
#[test]
fn test_externs_lifting1() {
    let mut h = Harness::new();
    let code = "/** @externs */ function f() {}";
    h.test(&[code], &[]);
    let compiler = h.get_compiler();
    let externs = compiler.get_externs_for_testing();
    assert_eq!(externs.len(), 3);
    let external = &externs[2];
    assert!(external.get_chunk().is_none());
    assert!(external.is_extern());
    assert_eq!(
        external.get_code().unwrap(),
        closure_rhino::js_string::JsString::from(code)
    );
    let inputs = compiler.get_inputs_for_testing().unwrap();
    assert_eq!(inputs.len(), 1);
    let input = &inputs[0];
    assert!(input.get_chunk().is_some());
    assert!(!input.is_extern());
    assert!(input.get_code().unwrap().is_empty());
}

// port: CommandLineRunnerTest#testExternsLifting2
#[test]
fn test_externs_lifting2() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_with_warning(
        &["/** @externs */ function f() {}", "f(3);"],
        &["f(3);"],
        Some("JSC_WRONG_ARGUMENT_COUNT"),
    );
}

// port: CommandLineRunnerTest#testSourceSortingOn
#[test]
fn test_source_sorting_on() {
    let mut h = Harness::new();
    h.test(
        &["goog.require('beer');", "goog.provide('beer');"],
        &["var beer = {};", ""],
    );
}

// port: CommandLineRunnerTest#testSourceSortingOn2
#[test]
fn test_source_sorting_on2() {
    let mut h = Harness::new();
    h.test(
        &["goog.provide('a');", "goog.require('a');"],
        &["var a={};", ""],
    );
}

// port: CommandLineRunnerTest#testSourceSortingOn3
#[test]
fn test_source_sorting_on3() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE_LEGACY".into());
    h.args.push("--language_in=ECMASCRIPT5".into());
    h.test(
        &[
            "goog.addDependency('sym', [], []);\nvar x = 3;",
            "/** This is base.js @provideGoog */ var COMPILED = false;",
        ],
        &["var COMPILED = !1;", "var x = 3;"],
    );
}

// port: CommandLineRunnerTest#testSourceSortingCircularDeps1
#[test]
fn test_source_sorting_circular_deps1() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE_LEGACY".into());
    h.args.push("--language_in=ECMASCRIPT5".into());
    h.test_diagnostic(
        &[
            "goog.provide('gin'); goog.require('tonic'); var gin = {};",
            "goog.provide('tonic'); goog.require('gin'); var tonic = {};",
            "goog.require('gin'); goog.require('tonic');",
        ],
        "JSC_LATE_PROVIDE_ERROR",
    );
}

// port: CommandLineRunnerTest#testSourceSortingCircularDeps2
#[test]
fn test_source_sorting_circular_deps2() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE_LEGACY".into());
    h.args.push("--language_in=ECMASCRIPT5".into());
    h.test_diagnostic(
        &[
            "goog.provide('roses.lime.juice');",
            "goog.provide('gin'); goog.require('tonic'); var gin = {};",
            "goog.provide('tonic'); goog.require('gin'); var tonic = {};",
            "goog.require('gin'); goog.require('tonic');",
            "goog.provide('gimlet'); goog.require('gin'); goog.require('roses.lime.juice');",
        ],
        "JSC_LATE_PROVIDE_ERROR",
    );
}

// port: CommandLineRunnerTest#testSourcePruningOn1
#[test]
fn test_source_pruning_on1() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE_LEGACY".into());
    h.args.push("--language_in=ECMASCRIPT5".into());
    h.test(
        &[
            "goog.require('beer');",
            "goog.provide('beer');",
            "goog.provide('scotch'); var x = 3;",
        ],
        &["var beer = {};", ""],
    );
}

// port: CommandLineRunnerTest#testSourcePruningOn2
#[test]
fn test_source_pruning_on2() {
    let mut h = Harness::new();
    h.args.push("--entry_point=goog:guinness".into());
    h.test(
        &[
            "goog.provide('guinness');\ngoog.require('beer');",
            "goog.provide('beer');",
            "goog.provide('scotch'); var x = 3;",
        ],
        &["var beer = {};", "var guinness = {};"],
    );
}

// port: CommandLineRunnerTest#testSourcePruningOn3
#[test]
fn test_source_pruning_on3() {
    let mut h = Harness::new();
    h.args.push("--entry_point=goog:scotch".into());
    h.test(
        &[
            "goog.provide('guinness');\ngoog.require('beer');",
            "goog.provide('beer');",
            "goog.provide('scotch'); var x = 3;",
        ],
        &["var scotch = {}, x = 3;"],
    );
}

// port: CommandLineRunnerTest#testSourcePruningOn4
#[test]
fn test_source_pruning_on4() {
    let mut h = Harness::new();
    h.args.push("--entry_point=goog:scotch".into());
    h.args.push("--entry_point=goog:beer".into());
    h.test(
        &[
            "goog.provide('guinness');\ngoog.require('beer');",
            "goog.provide('beer');",
            "goog.provide('scotch'); var x = 3;",
        ],
        &["var scotch = {}, x = 3;", "var beer = {};"],
    );
}

// port: CommandLineRunnerTest#testSourcePruningOn6
#[test]
fn test_source_pruning_on6() {
    let mut h = Harness::new();
    h.args.extend(["--entry_point=goog:scotch".into()]);
    h.test(
        &[
            "goog.require('beer');",
            "goog.provide('beer');",
            "goog.provide('scotch'); var x = 3;",
        ],
        &["var beer = {};", "", "var scotch = {}, x = 3;"],
    );
    assert_eq!(
        h.get_compiler().get_options().get_dependency_options(),
        &closure_jscomp::dependency_options::DependencyOptions::prune_legacy_for_entry_points(
            vec![closure_jscomp::module_identifier::ModuleIdentifier::for_closure("scotch")]
        )
    );
}

// port: CommandLineRunnerTest#testSourcePruningOn7
#[test]
fn test_source_pruning_on7() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE_LEGACY".into());
    h.test(
        &["/** This is base.js @provideGoog */ var COMPILED = false;"],
        &["var COMPILED = !1;"],
    );
}

// port: CommandLineRunnerTest#testSourcePruningOn8
#[test]
fn test_source_pruning_on8() {
    let mut h = Harness::new();
    h.args.extend([
        "--dependency_mode=PRUNE".into(),
        "--entry_point=goog:scotch".into(),
        "--warning_level=VERBOSE".into(),
    ]);
    h.test(&["/** @externs */\nvar externVar;\n/**\n * @fileoverview\n * @externs\n */\n\n/** @const */ var goog = {};\ngoog.module = function(ns) {};\ngoog.module.declareLegacyNamespace = function() {};\ngoog.module.preventModuleExportSealing = function() {};\n/** @return {?} */\ngoog.module.get = function(ns) {};\ngoog.provide = function(ns) {};\n/** @return {?} */\ngoog.require = function(ns) {};\n/** @return {?} */\ngoog.requireType = function(ns) {};\ngoog.requireDynamic = function(ns) {};\ngoog.loadModule = function(ns) {};\n/** @return {?} */\ngoog.forwardDeclare = function(ns) {};\ngoog.setTestOnly = function() {};\ngoog.scope = function(fn) {};\ngoog.declareModuleId = function(ns) {};\n/**\n * @param {string} name\n * @param {T} defaultValue\n * @return {T}\n * @template T\n */\ngoog.define = function(name, defaultValue) {};\ngoog.exportSymbol = function(publicName, symbol) {};\ngoog.inherits = function(childCtor, parentCtor) {\n  childCtor.superClass_ = parentCtor.prototype;\n};\ngoog.getMsg = function(str) {};\n/**\n * @param {T} symbol\n * @return {T|undefined}\n * @template T\n * @noinline\n */\ngoog.weakUsage = function(symbol) {};\n","goog.provide('scotch');\nvar x = externVar;\n"],&["var scotch = {}, x = externVar;"]);
}

// port: CommandLineRunnerTest#testChunkEntryPoint
#[test]
fn test_chunk_entry_point() {
    let mut h = Harness::new();
    h.args.extend([
        "--dependency_mode=PRUNE".into(),
        "--entry_point=goog:m1:a".into(),
    ]);
    h.use_chunks = ChunkPattern::Star;
    h.test(
        &["goog.provide('a');", "goog.provide('b');"],
        &["", "var a = {};"],
    );
}

// port: CommandLineRunnerTest#testMultistageCompilation
#[test]
fn test_multistage_compilation() {
    let mut h = Harness::new();
    let files = TempFiles::new();
    let save = files.0.join("serialized.state");
    std::fs::write(&save, "").unwrap();
    let input = "[{\"src\": \"alert('foo');\", \"path\":\"foo.js\"}]";
    h.args.extend([
        "--json_streams=BOTH".into(),
        "--chunk=foo--bar.baz:1".into(),
    ]);
    let mut stage1 = h.args.clone();
    stage1.extend([
        format!("--filename_to_save_to={}", save.display()),
        "--segment_of_compilation_to_run=CHECKS".into(),
    ]);
    h.compile(input, &stage1).unwrap();
    let mut stage2 = h.args.clone();
    stage2.extend([
        format!("--filename_to_restore_from={}", save.display()),
        "--segment_of_compilation_to_run=OPTIMIZATIONS".into(),
    ]);
    let multiple = h.compile(input, &stage2).unwrap();
    let single = h.compile(input, &h.args.clone()).unwrap();
    assert_eq!(multiple, single);
}

// port: CommandLineRunnerTest#testGoogAssertStripping
#[test]
fn test_goog_assert_stripping() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=ADVANCED_OPTIMIZATIONS".into());
    h.args.push("--jscomp_off=checkVars".into());
    h.test(&["goog.asserts.assert(false)"], &[""]);
    h.args.push("--debug".into());
    h.test(
        &["goog.asserts.assert(false)"],
        &["goog.$asserts$.$assert$(!1)"],
    );
}

// port: CommandLineRunnerTest#testMissingReturnCheckOnWithVerbose
#[test]
fn test_missing_return_check_on_with_verbose() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(
        &["/** @return {number} */ function f() {f()} f();"],
        "JSC_MISSING_RETURN_STATEMENT",
    );
}

// port: CommandLineRunnerTest#testProcessCJS
#[test]
fn test_processcj_s() {
    let mut h = Harness::new();
    h.args.extend([
        "--process_common_js_modules".into(),
        "--entry_point=foo/bar".into(),
    ]);
    h.set_filename(0, "foo/bar.js");
    h.use_string_comparison = true;
    h.test(
        &["exports.test = 1"],
        &["var module$foo$bar={default:{}};module$foo$bar.default.test=1;"],
    );
    assert_eq!(
        h.out_reader.text(),
        "var module$foo$bar={default:{}};module$foo$bar.default.test=1;\n"
    );
}

// port: CommandLineRunnerTest#testProcessCJSWithChunkOutput
#[test]
fn test_processcjs_with_chunk_output() {
    let mut h = Harness::new();
    h.args.extend([
        "--process_common_js_modules".into(),
        "--entry_point=foo/bar".into(),
        "--chunk=auto".into(),
    ]);
    h.set_filename(0, "foo/bar.js");
    h.test(
        &["exports.test = 1"],
        &["var module$foo$bar={default: {}}; module$foo$bar.default.test = 1;"],
    );
    assert_eq!(h.out_reader.text(), "");
}

// port: CommandLineRunnerTest#testSimpleJsonFileInclusionCompiles
#[test]
fn test_simple_json_file_inclusion_compiles() {
    let mut h = Harness::new();
    h.args.push("--module_resolution=NODE".into());
    h.args.push("--compilation_level=ADVANCED".into());
    h.set_filename(0, "index.js");
    h.set_filename(1, "package.json");
    h.test_diagnostic(
        &[
            "const /** string */ typeError = 0;",
            "{\n  \"name\": \"test\"\n}\n",
        ],
        "JSC_TYPE_MISMATCH",
    );
}

// port: CommandLineRunnerTest#testProcessCJSWithClosureRequires
#[test]
fn test_processcjs_with_closure_requires() {
    let mut h = Harness::new();
    h.args.push("--process_common_js_modules".into());
    h.args.push("--entry_point=app".into());
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--module_resolution=NODE".into());
    h.set_filename(0, "base.js");
    h.set_filename(1, "array.js");
    h.set_filename(2, "Baz.js");
    h.set_filename(3, "app.js");
    h.test(&["/** @provideGoog */\n/** @const */ var goog = goog || {};\nvar COMPILED = false;\ngoog.provide = function (arg) {};\ngoog.require = function (arg) {};\n","goog.provide('goog.array');","goog.require('goog.array');\nfunction Baz() {}\nBaz.prototype = {\n  baz: function() {\n    return goog.array.last(['asdf','asd','baz']);\n  },\n  bar: function () {\n    return 4 + 4;\n  }\n};\nmodule.exports = Baz;\n","var Baz = require('./Baz');\nvar baz = new Baz();\nconsole.log(baz.baz());\nconsole.log(baz.bar());\n"],&["var goog=goog||{},COMPILED=!1;\ngoog.provide=function(a){};goog.require=function(a){};\n","goog.array={};","var module$Baz = {/** @constructor */ default: function (){} };\nmodule$Baz.default.prototype={\n  baz:function(){return goog.array.last(['asdf','asd','baz'])},\n  bar:function(){return 8}\n};\n","var Baz = module$Baz.default,\n    baz = new module$Baz.default();\nconsole.log(baz.baz());\nconsole.log(baz.bar());\n"]);
}

// port: CommandLineRunnerTest#testProcessCJSWithClosureRequires2
#[test]
fn test_processcjs_with_closure_requires2() {
    let mut h = Harness::new();
    h.args.push("--process_common_js_modules".into());
    h.args.push("--dependency_mode=PRUNE_LEGACY".into());
    h.args.push("--entry_point=app".into());
    h.args.push("--module_resolution=NODE".into());
    h.set_filename(0, "base.js");
    h.set_filename(1, "array.js");
    h.set_filename(2, "Baz.js");
    h.set_filename(3, "app.js");
    h.test(&["/** @provideGoog */\n/** @const */ var goog = goog || {};\nvar COMPILED = false;\ngoog.provide = function (arg) {};\ngoog.require = function (arg) {};\n","goog.provide('goog.array');","goog.require('goog.array');\nfunction Baz() {}\nBaz.prototype = {\n  baz: function() {\n    return goog.array.last(['asdf','asd','baz']);\n  },\n  bar: function () {\n    return 4 + 4;\n  }\n};\nmodule.exports = Baz;\n","var Baz = require('./Baz');\nvar baz = new Baz();\nconsole.log(baz.baz());\nconsole.log(baz.bar());\n"],&["var goog=goog||{},COMPILED=!1;\ngoog.provide=function(a){};goog.require=function(a){};\n","goog.array={};","var module$Baz = {default: function (){}};\nmodule$Baz.default.prototype={\n  baz:function(){return goog.array.last([\"asdf\",\"asd\",\"baz\"])},\n  bar:function(){return 8}\n};\n","var Baz = module$Baz.default,\n    baz = new module$Baz.default();\nconsole.log(baz.baz());\nconsole.log(baz.bar());\n"]);
}

// port: CommandLineRunnerTest#testProcessCJSWithES6Export
#[test]
fn test_process_cjs_with_es6_export() {
    let mut h = Harness::new();
    h.args.push("--process_common_js_modules".into());
    h.args.push("--entry_point=app".into());
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.args.push("--module_resolution=NODE".into());
    h.set_filename(0, "foo.js");
    h.set_filename(1, "app.js");
    h.test(
        &[
            "export default class Foo {\n  bar() { console.log('bar'); }\n}\n",
            "var FooBar = require('./foo').default;\nvar baz = new FooBar();\nconsole.log(baz.bar());\n",
        ],
        &[
            "var Foo$$module$foo=function(){};\nFoo$$module$foo.prototype.bar=function(){console.log(\"bar\")};\nvar module$foo={};\n/** @const */ module$foo.default=Foo$$module$foo;\n",
            "var FooBar = Foo$$module$foo,\n    baz = new Foo$$module$foo();\nconsole.log(baz.bar());\n",
        ],
    );
}
// port: CommandLineRunnerTest#testES6ImportOfCJS
#[test]
fn test_es6_import_of_cjs() {
    let mut h = Harness::new();
    h.args.push("--process_common_js_modules".into());
    h.args.push("--entry_point=app".into());
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--language_out=ECMASCRIPT5".into());
    h.args.push("--module_resolution=NODE".into());
    h.set_filename(0, "foo.js");
    h.set_filename(1, "app.js");
    h.test(
        &[
            "/** @constructor */ function Foo () {}\nFoo.prototype.bar = function() { console.log('bar'); };\nmodule.exports = Foo;\n",
            "import * as FooBar from './foo';\nvar baz = new FooBar();\nconsole.log(baz.bar());\n",
        ],
        &[
            "/** @const */ var module$foo = {/** @constructor */ default: function(){} };\nmodule$foo.default.prototype.bar=function(){console.log('bar')};\n",
            "var baz$$module$app = new module$foo();\nconsole.log(baz$$module$app.bar());\n/** @const */ var module$app = {};\n",
        ],
    );
}
// port: CommandLineRunnerTest#testES6ImportOfFileWithImportsButNoExports
#[test]
fn testes6_import_of_file_with_imports_but_no_exports() {
    let mut h = Harness::new();
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--entry_point='./app.js'".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.set_filename(0, "message.js");
    h.set_filename(1, "foo.js");
    h.set_filename(2, "app.js");
    h.test(&["export default 'message';","import message from './message.js';\nfunction foo() { alert(message); }\nfoo();","import './foo.js';"],&["var $jscompDefaultExport$$module$message = 'message', module$message = {};\n/** @const */ module$message.default = $jscompDefaultExport$$module$message;\n","function foo$$module$foo(){\n  alert($jscompDefaultExport$$module$message);\n}\nfoo$$module$foo();\n/** @const */ var module$foo = {};\n","/** @const */ var module$app = {};"]);
}

// port: CommandLineRunnerTest#testCommonJSRequireOfFileWithoutExports
#[test]
fn test_commonjs_require_of_file_without_exports() {
    let mut h = Harness::new();
    h.args.push("--process_common_js_modules".into());
    h.args.push("--dependency_mode=PRUNE".into());
    h.args.push("--entry_point='./app.js'".into());
    h.args.push("--language_in=ECMASCRIPT6".into());
    h.args.push("--module_resolution=NODE".into());
    h.set_filename(0, "foo.js");
    h.set_filename(1, "app.js");
    h.test(&["function foo() { alert('foo'); }\nfoo();\n","require('./foo');"],&["/** @const */ var module$foo = {/** @const */ default: {}};\nfunction foo$$module$foo(){ alert('foo'); }\nfoo$$module$foo();\n","'use strict';\n\n"]);
}

// port: CommandLineRunnerTest#testChunkJSON
#[test]
fn test_chunkjso_n() {
    let mut h = Harness::new();
    h.args.push("--process_common_js_modules".into());
    h.args.push("--entry_point=foo/bar".into());
    h.args.push("--output_chunk_dependencies=test.json".into());
    h.set_filename(0, "foo/bar.js");
    h.test(&["module.exports = {foo: 1};"],&["/** @const */ var module$foo$bar = {/** @const */ default: {}};\nmodule$foo$bar.default.foo = 1;\n"]);
}

// port: CommandLineRunnerTest#testAssumeFunctionWrapper
#[test]
fn test_assume_function_wrapper() {
    let mut h = Harness::new();
    h.args
        .push("--compilation_level=SIMPLE_OPTIMIZATIONS".into());
    h.args.push("--assume_function_wrapper".into());
    h.test(&["var someName = function(a) {};"], &[""]);
    h.test(
        &["var someName = function() {return 'hi'};alert(someName())"],
        &["alert('hi')"],
    );
    h.test(
        &["var someName = function() {return 'hi'};alert(someName);alert(someName)"],
        &["function a() {return 'hi'}alert(a);alert(a)"],
    );
}

// port: CommandLineRunnerTest#testWebpackModuleIds
#[test]
fn test_webpack_module_ids() {
    let mut h = Harness::new();
    h.args.extend([
        "--json_streams=BOTH".into(),
        "--module_resolution=WEBPACK".into(),
        "--process_common_js_modules".into(),
        "--entry_point=foo.js".into(),
        "--dependency_mode=PRUNE".into(),
        "--js_output_file=out.js".into(),
    ]);
    let mut runner = CommandLineRunner::new(&h.args,Box::new(std::io::Cursor::new("[\n  {\"src\": \"__webpack_require__(2);\", \"path\":\"foo.js\", \"webpackId\": \"1\"},\n  {\"src\": \"console.log('bar');\", \"path\":\"bar.js\", \"webpackId\": \"2\"}\n]\n".as_bytes().to_vec())),Box::new(h.out_reader.clone()),Box::new(h.err_reader.clone())).unwrap();
    runner.do_run().unwrap();
    assert_eq!(
        h.out_reader.text(),
        "[{\"src\":\"var module$bar={default:{}};console.log(\\\"bar\\\");var module$foo={default:{}};\\n\",\"path\":\"out.js\",\"source_map\":\"{\\n\\\"version\\\":3,\\n\\\"file\\\":\\\"out.js\\\",\\n\\\"lineCount\\\":1,\\n\\\"mappings\\\":\\\"AAAA,IAAA,WAAA,CAAA,QAAA,EAAA,CAAAA,QAAQC,CAAAA,GAAR,CAAY,KAAZ,C,CCAA,IAAA,WAAA,CAAA,QAAA,EAAA;\\\",\\n\\\"sources\\\":[\\\"bar.js\\\",\\\"foo.js\\\"],\\n\\\"names\\\":[\\\"console\\\",\\\"log\\\"]\\n}\\n\"}]"
    );
}

// port: CommandLineRunnerTest#testExpectedDiagnostics
fn expected_diagnostics(
    h: &mut Harness,
    input: &str,
    expected_diagnostics: &[&str],
) -> Vec<closure_jscomp::js_error::JSError> {
    for diag in expected_diagnostics {
        h.args.push(format!("--expected_diagnostics={diag}"));
    }
    h.compile_sources(&[input]);

    let compiler = h.get_compiler();
    let mut actual = compiler.get_errors();
    actual.extend(compiler.get_warnings());
    actual
}

// port: CommandLineRunnerTest#testExpectedDiagnostics_matchErrorAndWarning
#[test]
fn test_expected_diagnostics_match_error_and_warning() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    h.args.push("--jscomp_warning=uselessCode".into());
    let actual = expected_diagnostics(
        &mut h,
        "/** @type {string} */ var x = 1; var y = 1; y;",
        &[
            "input0:1:30: ERROR - \\[JSC_TYPE_MISMATCH\\] initializing variable\nfound   : number\nrequired: string",
            "input0:1:44: WARNING - \\[JSC_USELESS_CODE\\] Suspicious code. This code lacks side-effects. Is there a bug?",
        ],
    );
    assert!(actual.is_empty());
}

// port: CommandLineRunnerTest#testExpectedDiagnostics_partialMatch
#[test]
fn test_expected_diagnostics_partial_match() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    let actual = expected_diagnostics(
        &mut h,
        "/** @type {string} */ var x = 1;",
        &["JSC_TYPE_MISMATCH"],
    );
    assert!(actual.is_empty());
}

// port: CommandLineRunnerTest#testExpectedDiagnostics_regexMatch
#[test]
fn test_expected_diagnostics_regex_match() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    let actual = expected_diagnostics(
        &mut h,
        "/** @type {string} */ var x = 1;",
        &["input0:.* ERROR - \\[JSC_TYPE_MISMATCH\\] .*"],
    );
    assert!(actual.is_empty());
}

// port: CommandLineRunnerTest#testExpectedDiagnostics_unmatchedExpectation
#[test]
fn test_expected_diagnostics_unmatched_expectation() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    let actual = expected_diagnostics(
        &mut h,
        "/** @type {string} */ var x = 1;",
        &[".*JSC_OTHER_ERROR.*"],
    );
    assert_eq!(actual.len(), 2);
    assert_eq!(
        actual[0].get_type().key,
        "JSC_EXPECTED_DIAGNOSTIC_NOT_FOUND"
    );
    assert_eq!(
        actual[0].get_description(),
        "Expected diagnostic not found: .*JSC_OTHER_ERROR.*"
    );
    assert_eq!(actual[1].get_type().key, "JSC_TYPE_MISMATCH");
}

// port: CommandLineRunnerTest#testExpectedDiagnostics_ambiguousMatch
#[test]
fn test_expected_diagnostics_ambiguous_match() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    let actual = expected_diagnostics(
        &mut h,
        "/** @type {string} */ var x = 1;",
        &[".*JSC_TYPE_MISMATCH.*", ".*initializing variable.*"],
    );
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].get_type().key, "JSC_AMBIGUOUS_EXPECTATION");
    assert_eq!(
        actual[0].get_description(),
        "Multiple expected diagnostics matched the error: \"input0:1:30: ERROR - [JSC_TYPE_MISMATCH] initializing variable\nfound   : number\nrequired: string\n\". Matches: \".*JSC_TYPE_MISMATCH.*\""
    );
}

// port: CommandLineRunnerTest#testExpectedDiagnostics_identicalExpectations
#[test]
fn test_expected_diagnostics_identical_expectations() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    let actual = expected_diagnostics(
        &mut h,
        "/** @type {string} */ var x = 1; /** @type {number} */ var y = 'a';",
        &[".*JSC_TYPE_MISMATCH.*", ".*JSC_TYPE_MISMATCH.*"],
    );
    assert!(actual.is_empty());
}

// port: CommandLineRunnerTest#testExpectedDiagnostics_singleRegexMultipleActuals
#[test]
fn test_expected_diagnostics_single_regex_multiple_actuals() {
    let mut h = Harness::new();
    h.args.push("--jscomp_error=checkTypes".into());
    let actual = expected_diagnostics(
        &mut h,
        "/** @type {string} */ var x = 1; /** @type {number} */ var y = 'a';",
        &[".*JSC_TYPE_MISMATCH.*"],
    );
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].get_type().key, "JSC_TYPE_MISMATCH");
    assert_eq!(
        actual[0].get_description(),
        "initializing variable\nfound   : string\nrequired: number"
    );
}
// port: CommandLineRunnerTest#browserFeaturesetYearFlagDefinesGoogFeaturesetYear
#[test]
fn browser_featureset_year_flag_defines_goog_featureset_year() {
    let mut h = Harness::new();
    h.args.push("--browser_featureset_year=2019".into());
    let original = FEATURESET_YEAR_ORIGINAL;
    let expected = "goog.FEATURESET_YEAR=2019";
    h.test(&[original], &[expected]);
    let options = h.get_compiler().get_options().clone();
    let define_replacements = options.get_define_replacements(&mut h.get_compiler().ast);
    assert!(define_replacements.contains_key("goog.FEATURESET_YEAR"));
    let n = define_replacements["goog.FEATURESET_YEAR"];
    assert_eq!(n.get_double(&h.get_compiler().ast), 2019.0);
}
// port: CommandLineRunnerTest#browserFeatureSetYearSetsLanguageOut1
#[test]
fn browser_feature_set_year_sets_language_out1() {
    let mut h = Harness::new();
    h.args.push("--browser_featureset_year=2012".into());
    let original = FEATURESET_YEAR_ORIGINAL;
    let expected = "goog.FEATURESET_YEAR=2012";
    h.test(&[original], &[expected]);
    /* Browser's year is not expected to match output language's year
    Flag value --browser_featureset_year=2012 corresponds to output ECMASCRIPT5_STRICT */
    assert_eq!(
        h.get_compiler().get_options().get_output_feature_set(),
        closure_jscomp::compiler_options::LanguageMode::ECMASCRIPT5_STRICT.to_feature_set()
    );
}
// port: CommandLineRunnerTest#browserFeatureSetYearSetsLanguageOut2
#[test]
fn browser_feature_set_year_sets_language_out2() {
    let mut h = Harness::new();
    h.args.push("--browser_featureset_year=2019".into());
    let original = FEATURESET_YEAR_ORIGINAL;
    let expected = "goog.FEATURESET_YEAR=2019";
    h.test(&[original], &[expected]);
    /* Browser's year is not expected to match output language's year
    Flag value --browser_featureset_year=2019 corresponds to output ECMASCRIPT_2017 */
    assert_eq!(
        h.get_compiler().get_options().get_output_feature_set(),
        closure_jscomp::compiler_options::LanguageMode::ECMASCRIPT_2017.to_feature_set()
    );
}
// port: CommandLineRunnerTest#browserFeatureSetYearSetsLanguageOut3
#[test]
fn browser_feature_set_year_sets_language_out3() {
    let mut h = Harness::new();
    h.args.push("--browser_featureset_year=2018".into());
    let original = FEATURESET_YEAR_ORIGINAL;
    let expected = "goog.FEATURESET_YEAR=2018";
    h.test(&[original], &[expected]);
    /* Browser's year is not expected to match output language's year
    Flag value --browser_featureset_year=2018 corresponds to output ECMASCRIPT_2016 */
    assert_eq!(
        h.get_compiler().get_options().get_output_feature_set(),
        closure_jscomp::compiler_options::LanguageMode::ECMASCRIPT_2016.to_feature_set()
    );
}
const FEATURESET_YEAR_ORIGINAL: &str =
    "/** @define {number} */\ngoog.FEATURESET_YEAR = goog.define('goog.FEATURESET_YEAR', 2012);\n";

// port: CommandLineRunnerTest#testDefineFlag
#[test]
fn test_define_flag() {
    let mut h = Harness::new();
    h.args.push("--define=FOO".into());
    h.args.push("--define=\"BAR=5\"".into());
    h.args.push("--D".into());
    h.args.push("CCC".into());
    h.args.push("-D".into());
    h.args.push("DDD".into());
    h.test(&["/** @define {boolean} */ var FOO = false;\n/** @define {number} */ var BAR = 3;\n/** @define {boolean} */ var CCC = false;\n/** @define {boolean} */ var DDD = false;\n"],&["var FOO = !0, BAR = 5, CCC = !0, DDD = !0;"]);
}

// port: CommandLineRunnerTest#testDefineFlag2
#[test]
fn test_define_flag2() {
    let mut h = Harness::new();
    h.args.push("--define=FOO='x\"'".into());
    h.test(
        &["/** @define {string} */ var FOO = \"a\";"],
        &["var FOO = \"x\\\"\";"],
    );
}

// port: CommandLineRunnerTest#testDefineFlag3
#[test]
fn test_define_flag3() {
    let mut h = Harness::new();
    h.args.push("--define=FOO=\"x'\"".into());
    h.test(
        &["/** @define {string} */ var FOO = \"a\";"],
        &["var FOO = \"x'\";"],
    );
}

// port: CommandLineRunnerTest#testGenerateExports
#[test]
fn test_generate_exports() {
    let mut h = Harness::new();
    h.args.push("--generate_exports=true".into());
    h.test(&["var goog; /** @export */ foo.prototype.x = function() {};"],&["var goog; foo.prototype.x=function(){};\ngoog.exportProperty(foo.prototype,\"x\",foo.prototype.x);\n"]);
}

// port: CommandLineRunnerTest#testChecksOnlyWithWarning
#[test]
fn test_checks_only_with_warning() {
    let mut h = Harness::new();
    h.args.push("--checks_only".into());
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(
        &["/** @deprecated */function foo() {}; foo();"],
        "JSC_DEPRECATED_VAR",
    );
}

// port: CommandLineRunnerTest#testDepreciationWithVerbose
#[test]
fn test_depreciation_with_verbose() {
    let mut h = Harness::new();
    h.args.push("--warning_level=VERBOSE".into());
    h.test_diagnostic(
        &["/** @deprecated */ function f() {}; f()"],
        "JSC_DEPRECATED_VAR",
    );
}
