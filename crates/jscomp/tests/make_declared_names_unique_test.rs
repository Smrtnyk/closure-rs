/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/MakeDeclaredNamesUniqueTest.java.

//! Port of MakeDeclaredNamesUniqueTest. A local fixture reproduces the part of CompilerTestCase
//! these tests use (`test`, `testSame`, `testNoWarning` with `srcs`, `expected` and `externs`) on
//! the real Compiler: the inputs are parsed, the test's processor runs on the externs and main
//! roots, no error or warning may be reported, and the printed result must equal the printed
//! parse of the expected code. The corpus records (MakeDeclaredNamesUniqueTest, 164) exercise the
//! full CompilerTestCase port in crates/testing.
use closure_jscomp::{
    Compiler,
    abstract_compiler::AbstractCompiler,
    check_level::CheckLevel,
    closure_coding_convention::ClosureCodingConvention,
    code_printer::Builder,
    coding_convention::CodingConvention,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    diagnostic_groups,
    google_coding_convention::GoogleCodingConvention,
    make_declared_names_unique::{InlineRenamer, MakeDeclaredNamesUnique},
    node_traversal::NodeTraversal,
    source_file::SourceFile,
};
use closure_rhino::{node::NodeId, testing::node_subject::assert_node};
use std::sync::Arc;

// port: CompilerTestCase#GENERATED_SRC_NAME
const GENERATED_SRC_NAME: &str = "testcode";
// port: CompilerTestCase#GENERATED_EXTERNS_NAME
const GENERATED_EXTERNS_NAME: &str = "externs";
// port: MakeDeclaredNamesUniqueTest#LOCAL_NAME_PREFIX
const LOCAL_NAME_PREFIX: &str = "unique_";

/// CompilerTestCase's `Sources` / `Expected` / `Externs`. Java picks the one-String overload
/// (`srcs(String)`: one file named `<name>`) for a single argument and the varargs overload
/// (files named `<name><i>`) otherwise.
struct Files(Vec<String>);

// port: CompilerTestCase#srcs(String) / srcs(String...)
fn srcs(texts: &[&str]) -> Files {
    Files(texts.iter().map(|t| t.to_string()).collect())
}

// port: CompilerTestCase#expected(String) / expected(String...)
fn expected(texts: &[&str]) -> Files {
    srcs(texts)
}

// port: CompilerTestCase#externs(String) / externs(String...)
fn externs(texts: &[&str]) -> Files {
    srcs(texts)
}

/// One code argument of a CompilerTestCase method: a single String (`srcs(String)`, one file
/// named `testcode`), a String[] or a `Sources`/`Expected` list (files named `testcode0`, ...).
trait Code {
    fn files(&self, name: &str) -> Vec<Arc<SourceFile>>;
}

impl Code for &str {
    // port: CompilerTestCase#maybeCreateSources
    fn files(&self, name: &str) -> Vec<Arc<SourceFile>> {
        vec![Arc::new(SourceFile::from_code(name, *self))]
    }
}

impl Code for String {
    fn files(&self, name: &str) -> Vec<Arc<SourceFile>> {
        self.as_str().files(name)
    }
}

impl Code for &[&str] {
    // port: CompilerTestCase#createSources(String, List)
    fn files(&self, name: &str) -> Vec<Arc<SourceFile>> {
        self.iter()
            .enumerate()
            .map(|(i, s)| Arc::new(SourceFile::from_code(&format!("{name}{i}"), *s)))
            .collect()
    }
}

impl<const N: usize> Code for &[&str; N] {
    fn files(&self, name: &str) -> Vec<Arc<SourceFile>> {
        self.as_slice().files(name)
    }
}

impl Code for &Files {
    fn files(&self, name: &str) -> Vec<Arc<SourceFile>> {
        if let [single] = self.0.as_slice() {
            return single.as_str().files(name);
        }
        let texts: Vec<&str> = self.0.iter().map(String::as_str).collect();
        texts.as_slice().files(name)
    }
}

impl Code for Files {
    fn files(&self, name: &str) -> Vec<Arc<SourceFile>> {
        (&self).files(name)
    }
}

/// The anonymous CompilerPass of MakeDeclaredNamesUniqueTest#getProcessor (`invert` false); it
/// reads the test's fields when the test calls `test`, as the Java pass reads them when it runs.
struct RenamingPass {
    use_default_renamer: bool,
    remove_const: bool,
    assert_on_change: bool,
}

impl CompilerPass for RenamingPass {
    // port: MakeDeclaredNamesUniqueTest#getProcessor (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut renamer =
            MakeDeclaredNamesUnique::builder().with_assert_on_change(self.assert_on_change);
        if !self.use_default_renamer {
            // Compiler#getCodingConvention: the options' convention, else the compiler's default.
            let convention: Arc<dyn CodingConvention> =
                match compiler.get_options().get_coding_convention().clone() {
                    Some(convention) => convention,
                    None => Arc::new(ClosureCodingConvention::new()),
                };
            renamer = renamer.with_renamer(InlineRenamer::new(
                convention,
                compiler.get_unique_name_id_supplier(),
                LOCAL_NAME_PREFIX,
                self.remove_const,
                true,
                None,
            ));
        }
        let mut pass = renamer.build();
        NodeTraversal::traverse_roots(compiler, &mut pass, externs, root);
    }
}

/// MakeDeclaredNamesUniqueTest's fields and the CompilerTestCase methods it calls.
struct Fixture {
    // this.useDefaultRenamer = true; invokes the ContextualRenamer
    // this.useDefaultRenamer = false; invokes the InlineRenamer
    use_default_renamer: bool,
    // invert = true; treats JavaScript input as normalized code and inverts the renaming
    // invert = false; conducts renaming
    invert: bool,
    // removeConst = true; removes const-ness of a name (e.g. If the variable name is CONST)
    remove_const: bool,
    // whether to throw an exception on any names newly made unique.
    assert_on_change: bool,
}

impl Fixture {
    // port: MakeDeclaredNamesUniqueTest#customSetUp
    fn new() -> Self {
        Self {
            remove_const: false,
            invert: false,
            use_default_renamer: false,
            assert_on_change: false,
        }
    }

    // port: MakeDeclaredNamesUniqueTest#getProcessor
    fn get_processor(&self, compiler: &mut Compiler) -> Box<dyn CompilerPass> {
        if !self.invert {
            Box::new(RenamingPass {
                use_default_renamer: self.use_default_renamer,
                remove_const: self.remove_const,
                assert_on_change: self.assert_on_change,
            })
        } else {
            MakeDeclaredNamesUnique::get_contextual_rename_inverter(compiler)
        }
    }

    // port: MakeDeclaredNamesUniqueTest#getOptions (CompilerTestCase#getOptions defaults)
    fn get_options(&self) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        options.set_language_in(LanguageMode::UNSUPPORTED);
        options.set_emit_use_strict(false);
        options.set_language_out(LanguageMode::NO_TRANSPILE);
        options.set_preserve_type_annotations(true);
        options.set_assume_getters_are_pure(false);
        options.set_check_symbols(true);
        options.set_warning_level(
            diagnostic_groups::INVALID_CASTS.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(
            diagnostic_groups::MISPLACED_MSG_ANNOTATION.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(
            diagnostic_groups::MISSING_PROPERTIES.clone(),
            CheckLevel::WARNING,
        );
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        options.set_polymer_version(Some(1));
        options.set_warning_level(diagnostic_groups::MODULE_LOAD.clone(), CheckLevel::OFF);
        options
    }

    fn parse(&self, externs: &[Arc<SourceFile>], inputs: &[Arc<SourceFile>]) -> Compiler {
        let mut compiler = Compiler::new();
        compiler.init(externs, inputs, self.get_options());
        compiler.parse_inputs();
        let descriptions = |list: Vec<closure_jscomp::js_error::JSError>| -> Vec<String> {
            list.iter().map(|e| e.description().to_string()).collect()
        };
        let errors = descriptions(compiler.get_errors());
        assert!(errors.is_empty(), "Unexpected parse error(s): {errors:?}");
        let warnings = descriptions(compiler.get_warnings());
        assert!(
            warnings.is_empty(),
            "Unexpected parse warning(s): {warnings:?}"
        );
        compiler
    }

    // port: CompilerTestCase#testInternal (processor run, no diagnostics, output comparison)
    fn test_internal(&self, externs: &[Arc<SourceFile>], inputs: impl Code, expected: impl Code) {
        let mut compiler = self.parse(externs, &inputs.files(GENERATED_SRC_NAME));
        let externs_root = compiler.get_externs_root().unwrap();
        let main_root = compiler.get_js_root().unwrap();
        let mut processor = self.get_processor(&mut compiler);
        processor.process(&mut compiler, externs_root, main_root);
        let errors = compiler.get_errors();
        assert!(errors.is_empty(), "Unexpected error(s): {}", errors.len());
        let warnings = compiler.get_warnings();
        assert!(
            warnings.is_empty(),
            "Unexpected warning(s): {}",
            warnings.len()
        );
        // CompilerTestCase#parseExpectedJs
        let expected_compiler = self.parse(externs, &expected.files(GENERATED_SRC_NAME));
        let expected_root = expected_compiler.get_js_root().unwrap();
        // compareJsDoc is true by default: NodeSubject#isEqualIncludingJsDocTo.
        if let Err(message) = assert_node(main_root).check_equal_to_across(
            &compiler,
            &expected_compiler,
            expected_root,
            true,
        ) {
            panic!(
                "{message}\nResult:   {}\nExpected: {}",
                Builder::new(main_root).build(&compiler).to_string_lossy(),
                Builder::new(expected_root)
                    .build(&expected_compiler)
                    .to_string_lossy()
            );
        }
    }

    // port: CompilerTestCase#test(String, String) / test(Sources, Expected)
    fn test(&self, js: impl Code, expected: impl Code) {
        // CompilerTestCase#CompilerTestCase(): `this("")`, one empty externs file.
        let externs = "".files(GENERATED_EXTERNS_NAME);
        self.test_internal(&externs, js, expected);
    }

    // port: CompilerTestCase#testSame(String) / testSame(Sources)
    fn test_same(&self, js: impl Code + Copy) {
        self.test(js, js);
    }

    // port: CompilerTestCase#testSame(Externs, Sources)
    fn test_same_externs(&self, externs: Files, js: Files) {
        let externs = externs.files(GENERATED_EXTERNS_NAME);
        self.test_internal(&externs, &js, &js);
    }

    // port: CompilerTestCase#testNoWarning(String)
    fn test_no_warning(&self, js: &str) {
        self.test(js, js);
    }

    // port: MakeDeclaredNamesUniqueTest#testWithInversion
    fn test_with_inversion(&mut self, original: impl Code + Copy, expected: impl Code + Copy) {
        self.invert = false;
        self.test(original, expected);
        self.invert = true;
        self.test(expected, original);
        self.invert = false;
    }

    // port: MakeDeclaredNamesUniqueTest#testSameWithInversion(String, String)
    fn test_same_with_inversion_externs(&mut self, externs_code: &str, original: &str) {
        let externs = externs_code.files(GENERATED_EXTERNS_NAME);
        self.invert = false;
        self.test_internal(&externs, original, original);
        self.invert = true;
        self.test_internal(&externs, original, original);
        self.invert = false;
    }

    // port: MakeDeclaredNamesUniqueTest#testSameWithInversion(String)
    fn test_same_with_inversion(&mut self, original: &str) {
        self.test_same_with_inversion_externs("", original);
    }

    // port: MakeDeclaredNamesUniqueTest#testInFunctionWithInversion
    fn test_in_function_with_inversion(&mut self, original: &str, expected: &str) {
        let original = wrap_in_function(original);
        let expected = wrap_in_function(expected);
        self.test_with_inversion(original.as_str(), expected.as_str());
    }
}

// port: MakeDeclaredNamesUniqueTest#wrapInFunction
fn wrap_in_function(s: &str) -> String {
    format!("function f(){{{s}}}")
}

// port: MakeDeclaredNamesUniqueTest#testMakeDeclaredNamesUniqueNullishCoalesce
#[test]
fn test_make_declared_names_unique_nullish_coalesce() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;

    t.test(
        "var foo; var x = function foo(){var foo = false ?? {};}",
        "var foo; var x = function foo$jscomp$1(){var foo$jscomp$2 = false ?? {}}",
    );
    t.test_same_with_inversion("var a = b ?? c;");
}

// port: MakeDeclaredNamesUniqueTest#testShadowedBleedingName
#[test]
fn test_shadowed_bleeding_name() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;

    t.test(
        "var foo; var x = function foo(){var foo;}",
        "var foo; var x = function foo$jscomp$1(){var foo$jscomp$2}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext1
#[test]
fn test_make_local_names_unique_with_context1() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;

    t.invert = true;
    t.test(
        "var a;function foo(){var a$jscomp$inline_1; a = 1}",
        "var a;function foo(){var a$jscomp$0       ; a = 1}",
    );
    t.test(
        "var a;function foo(){var a$jscomp$inline_1;}", //
        "var a;function foo(){var a                ;}",
    );

    t.test(
        "let a;function foo(){let a$jscomp$inline_1; a = 1}",
        "let a;function foo(){let a$jscomp$0       ; a = 1}",
    );
    t.test(
        "const a = 1;function foo(){let a$jscomp$inline_1;}", //
        "const a = 1;function foo(){let a                ;}",
    );
    t.test(
        "class A {} function foo(){class A$jscomp$inline_1 {}}",
        "class A {} function foo(){class A {}}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext2
#[test]
fn test_make_local_names_unique_with_context2() {
    let mut t = Fixture::new();
    // Set the test type

    t.use_default_renamer = true;

    // Verify global names are untouched.

    t.test_same_with_inversion("var a;");
    t.test_same_with_inversion("let a;");
    t.test_same_with_inversion("const a = 0;");

    // Verify global names are untouched.

    t.test_same_with_inversion("a;");

    // Local names are made unique.

    t.test_with_inversion(
        "var a;function foo(a){var b;a}",
        "var a;function foo(a$jscomp$1){var b;a$jscomp$1}",
    );
    t.test_with_inversion(
        "var a;function foo(){var b;a}function boo(){var b;a}",
        "var a;function foo(){var b;a}function boo(){var b$jscomp$1;a}",
    );
    t.test_with_inversion(
        concat!("function foo(a){var b}\n", "function boo(a){var b}\n"),
        concat!(
            "function foo(a){var b}\n",
            "function boo(a$jscomp$1){var b$jscomp$1}\n"
        ),
    );
    // variable b is left untouched because it is only declared once

    t.test_with_inversion(
        "let a;function foo(a){let b;a}",
        "let a;function foo(a$jscomp$1){let b;a$jscomp$1}",
    );
    t.test_with_inversion(
        "let a;function foo(){let b;a}function boo(){let b;a}",
        "let a;function foo(){let b;a}function boo(){let b$jscomp$1;a}",
    );
    t.test_with_inversion(
        concat!("function foo(a){let b}\n", "function boo(a){let b}\n"),
        concat!(
            "function foo(a){let b}\n",
            "function boo(a$jscomp$1){let b$jscomp$1}\n"
        ),
    );

    // Verify functions expressions are renamed.

    t.test_with_inversion(
        "var a = function foo(){foo()};var b = function foo(){foo()};",
        "var a = function foo(){foo()};var b = function foo$jscomp$1(){foo$jscomp$1()};",
    );
    t.test_with_inversion(
        "let a = function foo(){foo()};let b = function foo(){foo()};",
        "let a = function foo(){foo()};let b = function foo$jscomp$1(){foo$jscomp$1()};",
    );

    // Verify catch exceptions names are made unique

    t.test_same_with_inversion("try { } catch(e) {e;}");

    // Inversion does not handle exceptions correctly.

    t.test(
        "try { } catch(e) {e;}; try { } catch(e) {e;}",
        "try { } catch(e) {e;}; try { } catch(e$jscomp$1) {e$jscomp$1;}",
    );
    t.test(
        "try { } catch(e) {e; try { } catch(e) {e;}};",
        "try { } catch(e) {e; try { } catch(e$jscomp$1) {e$jscomp$1;} }; ",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext3
#[test]
fn test_make_local_names_unique_with_context3() {
    let mut t = Fixture::new();
    // Set the test type

    t.use_default_renamer = true;

    let externs_code = "var extern1 = {};";

    // Verify global names are untouched.

    t.test_same_with_inversion_externs(externs_code, "var extern1 = extern1 || {};");

    // Verify global names are untouched.

    t.test_same_externs(
        externs(&[externs_code]),
        srcs(&["var extern1 = extern1 || {};"]),
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext4
#[test]
fn test_make_local_names_unique_with_context4() {
    let mut t = Fixture::new();
    // Set the test type

    t.use_default_renamer = true;

    t.test_in_function_with_inversion(
        "var e; try { } catch(e) {e;}; try { } catch(e) {e;}",
        "var e; try { } catch(e$jscomp$1) {e$jscomp$1;}; try { } catch(e$jscomp$2) {e$jscomp$2;}",
    );
    t.test_in_function_with_inversion(
        "var e; try { } catch(e         ) {         e; try { } catch(e         ) {e;         } };",
        "var e; try { } catch(e$jscomp$1) {e$jscomp$1; try { } catch(e$jscomp$2) {e$jscomp$2;} };",
    );
    t.test_in_function_with_inversion(
        "try { } catch(e         ) {e         ;}; try { } catch(e         ) {e         ;}; var e;",
        "try { } catch(e$jscomp$1) {e$jscomp$1;}; try { } catch(e$jscomp$2) {e$jscomp$2;}; var e;",
    );
    t.test_in_function_with_inversion(
        "try { } catch(e         ) {e         ; try { } catch(e         ) {e         ;} }; var e;",
        "try { } catch(e$jscomp$1) {e$jscomp$1; try { } catch(e$jscomp$2) {e$jscomp$2;} }; var e;",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext5
#[test]
fn test_make_local_names_unique_with_context5() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        "function f(){var f; f = 1}",
        "function f(){var f$jscomp$1; f$jscomp$1 = 1}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext6
#[test]
fn test_make_local_names_unique_with_context6() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        "function f(f){f = 1}",
        "function f(f$jscomp$1){f$jscomp$1 = 1}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext7
#[test]
fn test_make_local_names_unique_with_context7() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        "function f(f){var f; f = 1}",
        "function f(f$jscomp$1){var f$jscomp$1; f$jscomp$1 = 1}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext8
#[test]
fn test_make_local_names_unique_with_context8() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test(
        "var fn = function f(){var f; f = 1}",
        "var fn = function f(){var f$jscomp$1; f$jscomp$1 = 1}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext9
#[test]
fn test_make_local_names_unique_with_context9() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same("var fn = function f(f){f = 1}");
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext10
#[test]
fn test_make_local_names_unique_with_context10() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same("var fn = function f(f){var f; f = 1}");
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithContext11
#[test]
fn test_make_local_names_unique_with_context11() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    // Changes the parameter name if it's the same as loop object name

    t.test(
concat!("var loopObjName = {};\n", "for(;;) {\n", "   var fn = (function f(loopObjName){ return function() {}; })(loopObjName);\n", "}\n"),
concat!("var loopObjName = {};\n", "for(;;) {\n", "   var fn = (function f(loopObjName$jscomp$1){ return function() {}; })(loopObjName);\n", "}\n"));

    // parameter name is already unique; no change

    t.test_same(concat!(
        "var foo = {};\n",
        "for(;;) {\n",
        "   var fn = (function f(bar){ return function() {}; })(foo);}\n"
    ));
}

// port: MakeDeclaredNamesUniqueTest#testMakeFunctionsUniqueWithContext
#[test]
fn test_make_functions_unique_with_context() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same("function f(){} function f(){}");
    t.test_same("var x = function() {function f(){} function f(){}};");
}

// port: MakeDeclaredNamesUniqueTest#testMakeFunctionsUniqueWithContext1
#[test]
fn test_make_functions_unique_with_context1() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test(
        "if (1) { function f(){} } else { function f(){} }",
        "if (1) { function f(){} } else { function f$jscomp$1(){} }",
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeFunctionsUniqueWithContext2
#[test]
fn test_make_functions_unique_with_context2() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same("if (1) { function f(){} function f(){} }");
}

// port: MakeDeclaredNamesUniqueTest#testMakeFunctionsUniqueWithContext3
#[test]
fn test_make_functions_unique_with_context3() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test(
        "function f() {} if (1) { function f(){} function f(){} }",
        "function f() {} if (1) { function f$jscomp$1(){} function f$jscomp$1(){} }",
    );
}

// port: MakeDeclaredNamesUniqueTest#testArguments
#[test]
fn test_arguments() {
    let mut t = Fixture::new();
    // Set the test type

    t.use_default_renamer = true;

    // Don't distinguish between "arguments", it can't be made unique.

    t.test_same_with_inversion("function foo(){var arguments;function bar(){var arguments;}}");

    t.invert = true;

    // Don't introduce new references to arguments, it is special.

    // Still try to rename it to a name that depends on the shape of the AST rather than

    // the process we happened to follow to reach that shape.

    t.test(
        "function foo(){var arguments$jscomp$1;}", //
        "function foo(){var arguments$jscomp$0;}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testClassInForLoop
#[test]
fn test_class_in_for_loop() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same("for (class a {};;) { break; }");
}

// port: MakeDeclaredNamesUniqueTest#testFunctionInForLoop
#[test]
fn test_function_in_for_loop() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same("for (function a() {};;) { break; }");
}

// port: MakeDeclaredNamesUniqueTest#testLetsInSeparateBlocks
#[test]
fn test_lets_in_separate_blocks() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test(
        concat!(
            "if (x) {\n",
            "  let e;\n",
            "  alert(e);\n",
            "}\n",
            "if (y) {\n",
            "  let e;\n",
            "  alert(e);\n",
            "}\n"
        ),
        concat!(
            "if (x) {\n",
            "  let e;\n",
            "  alert(e);\n",
            "}\n",
            "if (y) {\n",
            "  let e$jscomp$1;\n",
            "  alert(e$jscomp$1);\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testConstInGlobalHoistScope
#[test]
fn test_const_in_global_hoist_scope() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same(concat!(
        "if (true) {\n",
        "  const x = 1; alert(x);\n",
        "}\n"
    ));

    t.test(
        concat!(
            "if (true) {\n",
            "  const x = 1; alert(x);\n",
            "} else {\n",
            "  const x = 1; alert(x);\n",
            "}\n"
        ),
        concat!(
            "if (true) {\n",
            "  const x = 1; alert(x);\n",
            "} else {\n",
            "  const x$jscomp$1 = 1; alert(x$jscomp$1);\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithoutContext
#[test]
fn test_make_local_names_unique_without_context() {
    let mut t = Fixture::new();
    t.use_default_renamer = false;

    t.test("var a;", "var a$jscomp$unique_0");
    t.test("let a;", "let a$jscomp$unique_0");

    // Verify undeclared names are untouched.

    t.test_same("a;");

    // Local names are made unique.

    t.test(
        concat!("var a;\n", "function foo(a){var b;a}\n"),
        concat!(
            "var a$jscomp$unique_0;\n",
            "function foo$jscomp$unique_1(a$jscomp$unique_2){\n",
            "  var b$jscomp$unique_3;a$jscomp$unique_2}\n"
        ),
    );
    t.test(
        concat!(
            "var a;\n",
            "function foo(){var b;a}\n",
            "function boo(){var b;a}\n"
        ),
        concat!(
            "var a$jscomp$unique_0;\n",
            "function foo$jscomp$unique_1(){var b$jscomp$unique_3;a$jscomp$unique_0}\n",
            "function boo$jscomp$unique_2(){var b$jscomp$unique_4;a$jscomp$unique_0}\n"
        ),
    );

    t.test(
        "let a; function foo(a) {let b; a; }",
        concat!(
            "let a$jscomp$unique_0;\n",
            "function foo$jscomp$unique_1(a$jscomp$unique_2) {\n",
            "  let b$jscomp$unique_3;\n",
            "  a$jscomp$unique_2;\n",
            "}\n"
        ),
    );

    t.test(
        concat!(
            "let a;\n",
            "function foo() { let b; a; }\n",
            "function boo() { let b; a; }\n"
        ),
        concat!(
            "let a$jscomp$unique_0;\n",
            "function foo$jscomp$unique_1() {\n",
            "  let b$jscomp$unique_3;\n",
            "  a$jscomp$unique_0;\n",
            "}\n",
            "function boo$jscomp$unique_2() {\n",
            "  let b$jscomp$unique_4;\n",
            "  a$jscomp$unique_0;\n",
            "}\n"
        ),
    );

    // Verify function expressions are renamed.

    t.test(
        "var a = function foo(){foo()};",
        "var a$jscomp$unique_0 = function foo$jscomp$unique_1(){foo$jscomp$unique_1()};",
    );
    t.test(
        "const a = function foo(){foo()};",
        "const a$jscomp$unique_0 = function foo$jscomp$unique_1(){foo$jscomp$unique_1()};",
    );

    // Verify catch exceptions names are made unique

    t.test(
        "try { } catch(e) {e;}",
        "try { } catch(e$jscomp$unique_0) {e$jscomp$unique_0;}",
    );
    t.test(
        concat!("try { } catch(e) {e;};\n", "try { } catch(e) {e;}\n"),
        concat!(
            "try { } catch(e$jscomp$unique_0) {e$jscomp$unique_0;};\n",
            "try { } catch(e$jscomp$unique_1) {e$jscomp$unique_1;}\n"
        ),
    );
    t.test(
        concat!("try { } catch(e) {e;\n", "try { } catch(e) {e;}};\n"),
        concat!(
            "try { } catch(e$jscomp$unique_0) {e$jscomp$unique_0;\n",
            "try { } catch(e$jscomp$unique_1) {e$jscomp$unique_1;} }; \n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testMakeLocalNamesUniqueWithoutContext2
#[test]
fn test_make_local_names_unique_without_context2() {
    let mut t = Fixture::new();
    // Set the test type

    t.use_default_renamer = false;

    t.test("var _a;", "var JSCompiler__a$jscomp$unique_0");
    t.test(
        "var _a = function _b(_c) { var _d; };",
        concat!(
            "var JSCompiler__a$jscomp$unique_0 = function JSCompiler__b$jscomp$unique_2(\n",
            "JSCompiler__c$jscomp$unique_1) { var JSCompiler__d$jscomp$unique_3; };\n"
        ),
    );

    t.test("let _a;", "let JSCompiler__a$jscomp$unique_0");
    t.test(
        "const _a = function _b(_c) { let _d; };",
        concat!(
            "const JSCompiler__a$jscomp$unique_0 = function JSCompiler__b$jscomp$unique_2(\n",
            "JSCompiler__c$jscomp$unique_1) { let JSCompiler__d$jscomp$unique_3; };\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testOnlyInversion
#[test]
fn test_only_inversion() {
    let mut t = Fixture::new();
    t.invert = true;
    t.test(
        "function f(a, a$jscomp$1) {}", //
        "function f(a, a$jscomp$0) {}",
    );
    t.test(
        "function f(a$jscomp$1, b$jscomp$2) {}", //
        "function f(a         , b         ) {}",
    );
    t.test(
        "function f(a$jscomp$1, a$jscomp$2) {}", //
        "function f(a         , a$jscomp$0) {}",
    );
    t.test(
        "try { } catch(e) {e; try { } catch(e$jscomp$1) {e$jscomp$1;} }; ",
        "try { } catch(e) {e; try { } catch(e) {e;} }; ",
    );
    t.test_same("var a$jscomp$1;");
    t.test_same("const a$jscomp$1 = 1;");
    t.test_same("function f() { var $jscomp$; }");
    t.test_same("var CONST = 3; var b = CONST;");
    t.test(
        "function f() {var CONST = 3; var ACONST$jscomp$1 = 2;}",
        "function f() {var CONST = 3; var ACONST = 2;}",
    );
    t.test(
        "function f() {const CONST = 3; const ACONST$jscomp$1 = 2;}",
        "function f() {const CONST = 3; const ACONST = 2;}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testOnlyInversion2
#[test]
fn test_only_inversion2() {
    let mut t = Fixture::new();
    t.invert = true;
    t.test(
        "function f() {try { } catch(e) {e;}; try { } catch(e$jscomp$0) {e$jscomp$0;}}",
        "function f() {try { } catch(e) {e;}; try { } catch(e) {e;}}",
    );
}

// port: MakeDeclaredNamesUniqueTest#testOnlyInversion3
#[test]
fn test_only_inversion3() {
    let mut t = Fixture::new();
    t.invert = true;
    t.test(
        concat!(
            "function x1() {\n",
            "  var a$jscomp$1;\n",
            "  function x2() {\n",
            "    var a$jscomp$2;\n",
            "  }\n",
            "  function x3() {\n",
            "    var a$jscomp$3;\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "function x1() {\n",
            "  var a;\n",
            "  function x2() {\n",
            "    var a;\n",
            "  }\n",
            "  function x3() {\n",
            "    var a;\n",
            "  }\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testOnlyInversion4
#[test]
fn test_only_inversion4() {
    let mut t = Fixture::new();
    t.invert = true;
    t.test_same(concat!(
        "function x1() {\n",
        "// The attempt to rename will re-generate this same exact name.\n",
        "// The purpose of this test is to make sure we don't accidentally report\n",
        "// this \"renaming\" to the same name as a change.\n",
        "  var a$jscomp$0;\n",
        "  function x2() {\n",
        "    var a;a$jscomp$0++\n",
        "  }\n",
        "}\n"
    ));
}

// port: MakeDeclaredNamesUniqueTest#testOnlyInversion5
#[test]
fn test_only_inversion5() {
    let mut t = Fixture::new();
    t.invert = true;
    t.test(
        concat!(
            "function x1() {\n",
            "  const a$jscomp$1 = 0;\n",
            "  function x2() {\n",
            "    const b$jscomp$1 = 0;\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "function x1() {\n",
            "  const a = 0;\n",
            "  function x2() {\n",
            "    const b = 0;\n",
            "  }\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testConstRemovingRename1
#[test]
fn test_const_removing_rename1() {
    let mut t = Fixture::new();
    t.remove_const = true;
    t.test(
        "(function () {var CONST = 3; var ACONST$jscomp$1 = 2;})",
        "(function () {var CONST$jscomp$unique_0 = 3; var ACONST$jscomp$unique_1 = 2;})",
    );
}

// port: MakeDeclaredNamesUniqueTest#testConstRemovingRename2
#[test]
fn test_const_removing_rename2() {
    let mut t = Fixture::new();
    t.remove_const = true;
    t.test(
        "var CONST = 3; var b = CONST;",
        "var CONST$jscomp$unique_0 = 3; var b$jscomp$unique_1 = CONST$jscomp$unique_0;",
    );
}

// port: MakeDeclaredNamesUniqueTest#testConstRemovingRenameAlsoRemovesAnnotation
#[test]
fn test_const_removing_rename_also_removes_annotation() {
    let mut t = Fixture::new();
    t.remove_const = true;
    t.test(
        "/** @const */ var c = 3; var b = c;",
        "/** blank */ var c$jscomp$unique_0 = 3; var b$jscomp$unique_1 = c$jscomp$unique_0;",
    );
}

// port: MakeDeclaredNamesUniqueTest#testRestParamWithoutContext
#[test]
fn test_rest_param_without_context() {
    let t = Fixture::new();
    t.test(
        "function f(...x) { x; }",
        "function f$jscomp$unique_0(...x$jscomp$unique_1) { x$jscomp$unique_1; }",
    );
}

// port: MakeDeclaredNamesUniqueTest#testRestParamWithContextWithInversion
#[test]
fn test_rest_param_with_context_with_inversion() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        concat!(
            "let x = 0;\n",
            "function foo(...x) {\n",
            "  return x[0];\n",
            "}\n"
        ),
        concat!(
            "let x = 0;\n",
            "function foo(...x$jscomp$1) {\n",
            "  return x$jscomp$1[0]\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testVarParamSameName0
#[test]
fn test_var_param_same_name0() {
    let t = Fixture::new();
    t.test(
        concat!("function f(x) {\n", "  if (!x) var x = 6;\n", "}\n"),
        concat!(
            "function f$jscomp$unique_0(x$jscomp$unique_1) {\n",
            "  if (!x$jscomp$unique_1) var x$jscomp$unique_1 = 6;\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testVarParamSameName1
#[test]
fn test_var_param_same_name1() {
    let t = Fixture::new();
    t.test(
        concat!("function f(x) {\n", "  if (!x) x = 6;\n", "}\n"),
        concat!(
            "function f$jscomp$unique_0(x$jscomp$unique_1) {\n",
            "  if (!x$jscomp$unique_1) x$jscomp$unique_1 = 6;\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testVarParamSameAsLet0
#[test]
fn test_var_param_same_as_let0() {
    let t = Fixture::new();
    t.test(
        concat!("function f(x) {\n", "  if (!x) { let x = 6; }\n", "}\n"),
        concat!(
            "function f$jscomp$unique_0(x$jscomp$unique_1) {\n",
            "  if (!x$jscomp$unique_1) { let x$jscomp$unique_2 = 6; }\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testObjectProperties
#[test]
fn test_object_properties() {
    let t = Fixture::new();
    t.test("var a = {x : 'a'};", "var a$jscomp$unique_0 = {x : 'a'};");
    t.test("let a = {x : 'a'};", "let a$jscomp$unique_0 = {x : 'a'};");
    t.test(
        "const a = {x : 'a'};",
        "const a$jscomp$unique_0 = {x : 'a'};",
    );
    t.test(
        "var a = {x : 'a'}; a.x",
        "var a$jscomp$unique_0 = {x : 'a'}; a$jscomp$unique_0.x",
    );
}

// port: MakeDeclaredNamesUniqueTest#testClassesWithContextWithInversion
#[test]
fn test_classes_with_context_with_inversion() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        concat!(
            "var a;\n",
            "class Foo {\n",
            "  constructor(a) {\n",
            "    this.a = a;\n",
            "  }\n",
            "  f() {\n",
            "    var x = 1;\n",
            "    return a + x;\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "var a;\n",
            "class Foo {\n",
            "  constructor(a$jscomp$1) {\n",
            "    this.a = a$jscomp$1;\n",
            "  }\n",
            "  f() {\n",
            "    var x = 1;\n",
            "    return a + x;\n",
            "  }\n",
            "}\n"
        ),
    );

    // class declarations are block-scoped but not hoisted.

    t.test_same_with_inversion(concat!(
        "{\n",
        "  let x = new Foo(); // ReferenceError\n",
        "  class Foo {}\n",
        "}\n"
    ));
}

// port: MakeDeclaredNamesUniqueTest#testBlockScopesWithContextWithInversion1
#[test]
fn test_block_scopes_with_context_with_inversion1() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        concat!("{let a;\n", "  {\n", "    let a;\n", "  }}\n"),
        concat!("{let a;\n", "  {\n", "  let a$jscomp$1;\n", "  }}\n"),
    );
}

// port: MakeDeclaredNamesUniqueTest#testBlockScopesWithContextWithInversion2
#[test]
fn test_block_scopes_with_context_with_inversion2() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    // function declarations are block-scoped

    t.test_with_inversion(
        concat!(
            "function foo() {\n",
            "  function bar() {\n",
            "    return 1;\n",
            "  }\n",
            "}\n",
            "function boo() {\n",
            "  function bar() {\n",
            "    return 2;\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "function foo() {\n",
            "  function bar() {\n",
            "    return 1;\n",
            "  }\n",
            "}\n",
            "function boo() {\n",
            "  function bar$jscomp$1() {\n",
            "    return 2;\n",
            "  }\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testBlockScopesWithContextWithInversion3
#[test]
fn test_block_scopes_with_context_with_inversion3() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test(
        concat!(
            "function foo() {\n",
            "  function bar() {\n",
            "    return 1;\n",
            "  }\n",
            "  if (true) {\n",
            "    function bar() {\n",
            "      return 2;\n",
            "    }\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "function foo() {\n",
            "  function bar() {\n",
            "    return 1;\n",
            "  }\n",
            "  if (true) {\n",
            "    function bar$jscomp$1() {\n",
            "      return 2;\n",
            "    }\n",
            "  }\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testBlockScopesWithContextWithInversion4
#[test]
fn test_block_scopes_with_context_with_inversion4() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test(
        concat!(
            "var f1=function(){\n",
            "  var x\n",
            "};\n",
            "(function() {\n",
            "  function f2() {\n",
            "    alert(x)\n",
            "  }\n",
            "  {\n",
            "    var x=0\n",
            "  }\n",
            "  f2()\n",
            "})()\n"
        ),
        concat!(
            "var f1=function(){\n",
            "  var x\n",
            "};\n",
            "(function() {\n",
            "  function f2() {\n",
            "    alert(x$jscomp$1)\n",
            "  }\n",
            "  {\n",
            "    var x$jscomp$1=0\n",
            "  }\n",
            "  f2()\n",
            "})()\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testBlockScopesWithContextWithInversion5
#[test]
fn test_block_scopes_with_context_with_inversion5() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same(concat!(
        "if (true) {\n",
        "  function f(){};\n",
        "}\n",
        "f();\n"
    ));
}

// port: MakeDeclaredNamesUniqueTest#testBlockScopesWithoutContext
#[test]
fn test_block_scopes_without_context() {
    let mut t = Fixture::new();
    t.use_default_renamer = false;
    t.test(
        concat!(
            "{\n",
            "  function foo() {return 1;}\n",
            "  if (true) {\n",
            "    function foo() {return 2;}\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "{\n",
            "  function foo$jscomp$unique_0() {return 1;}\n",
            "  if (true) {\n",
            "    function foo$jscomp$unique_1() {return 2;}\n",
            "  }\n",
            "}\n"
        ),
    );

    t.test(
        concat!("function foo(x) {\n", "  return foo(x) - 1;\n", "}\n"),
        concat!(
            "function foo$jscomp$unique_0(x$jscomp$unique_1) {\n",
            "  return foo$jscomp$unique_0(x$jscomp$unique_1) - 1;\n",
            "}\n"
        ),
    );

    t.test(
        concat!(
            "export function foo(x) {\n",
            "  return foo(x) - 1;\n",
            "}\n"
        ),
        concat!(
            "export function foo$jscomp$unique_0(x$jscomp$unique_1) {\n",
            "  return foo$jscomp$unique_0(x$jscomp$unique_1) - 1;\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testRecursiveFunctionsWithContextWithInversion
#[test]
fn test_recursive_functions_with_context_with_inversion() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_same_with_inversion(concat!(
        "function foo(x) {\n",
        "  return foo(x) - 1;\n",
        "}\n"
    ));
}

// port: MakeDeclaredNamesUniqueTest#testInvertShadowedParameterNames
#[test]
fn test_invert_shadowed_parameter_names() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        concat!(
            "var p;\n",
            "function f(p) {\n",
            "  return function g(p) {\n",
            "    return p;\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "var p;\n",
            "function f(p$jscomp$1) {\n",
            "  return function g(p$jscomp$2) {\n",
            "    return p$jscomp$2;\n",
            "  }\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testArrowFunctionWithContextWithInversion
#[test]
fn test_arrow_function_with_context_with_inversion() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        concat!(
            "function foo() {\n",
            "  var f = (x) => x;\n",
            "  return f(1);\n",
            "}\n",
            "function boo() {\n",
            "  var f = (x) => x;\n",
            "  return f(2);\n",
            "}\n"
        ),
        concat!(
            "function foo() {\n",
            "  var f = (x) => x;\n",
            "  return f(1);\n",
            "}\n",
            "function boo() {\n",
            "  var f$jscomp$1 = (x$jscomp$1) => x$jscomp$1;\n",
            "  return f$jscomp$1(2);\n",
            "}\n"
        ),
    );

    t.test_with_inversion(
        concat!(
            "function foo() {\n",
            "  var f = (x, ...y) => x + y[0];\n",
            "  return f(1, 2);\n",
            "}\n",
            "function boo() {\n",
            "  var f = (x, ...y) => x + y[0];\n",
            "  return f(1, 2);\n",
            "}\n"
        ),
        concat!(
            "function foo() {\n",
            "  var f = (x, ...y) => x + y[0];\n",
            "  return f(1, 2);\n",
            "}\n",
            "function boo() {\n",
            "  var f$jscomp$1 = (x$jscomp$1, ...y$jscomp$1) => x$jscomp$1 + y$jscomp$1[0];\n",
            "  return f$jscomp$1(1, 2);\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testDefaultParameterWithContextWithInversion1
#[test]
fn test_default_parameter_with_context_with_inversion1() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        concat!(
            "function foo(x = 1) {\n",
            "  return x;\n",
            "}\n",
            "function boo(x = 1) {\n",
            "  return x;\n",
            "}\n"
        ),
        concat!(
            "function foo(x = 1) {\n",
            "  return x;\n",
            "}\n",
            "function boo(x$jscomp$1 = 1) {\n",
            "  return x$jscomp$1;\n",
            "}\n"
        ),
    );

    t.test_same_with_inversion(concat!(
        "function foo(x = 1, y = x) {\n",
        "  return x + y;\n",
        "}\n"
    ));
}

// port: MakeDeclaredNamesUniqueTest#testDefaultParameterWithContextWithInversion2
#[test]
fn test_default_parameter_with_context_with_inversion2() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;

    // Parameter default values don't see the scope of the body

    // Methods or functions defined "inside" parameter default values don't see the local variables

    // of the body.

    t.test_with_inversion(
        concat!(
            "let x = 'outer';\n",
            "function foo(bar = baz => x) {\n",
            "  let x = 'inner';\n",
            "  console.log(bar());\n",
            "}\n"
        ),
        concat!(
            "let x = 'outer';\n",
            "function foo(bar = baz => x) {\n",
            "  let x$jscomp$1 = 'inner';\n",
            "  console.log(bar());\n",
            "}\n"
        ),
    );

    t.test_with_inversion(
        concat!(
            "const x = 'outer';\n",
            "function foo(a = x) {\n",
            "  const x = 'inner';\n",
            "  return a;\n",
            "}\n"
        ),
        concat!(
            "const x = 'outer';\n",
            "function foo(a = x) {\n",
            "  const x$jscomp$1 = 'inner';\n",
            "  return a;\n",
            "}\n"
        ),
    );

    t.test_with_inversion(
        concat!(
            "const x = 'outerouter';\n",
            "{\n",
            "  const x = 'outer';\n",
            "  function foo(a = x) {\n",
            "    return a;\n",
            "  }\n",
            "foo();\n",
            "}\n"
        ),
        concat!(
            "const x = 'outerouter';\n",
            "{\n",
            "  const x$jscomp$1 = 'outer';\n",
            "  function foo(a = x$jscomp$1) {\n",
            "    return a;\n",
            "  }\n",
            "foo();\n",
            "}\n"
        ),
    );

    t.test_same_with_inversion(concat!(
        "function foo(x, y = x) {\n",
        "  return x + y;\n",
        "}\n"
    ));
}

// port: MakeDeclaredNamesUniqueTest#testObjectLiteralsWithContextWithInversion
#[test]
fn test_object_literals_with_context_with_inversion() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        concat!(
            "function foo({x:y}) {\n",
            "  return y;\n",
            "}\n",
            "function boo({x:y}) {\n",
            "  return y;\n",
            "}\n"
        ),
        concat!(
            "function foo({x:y}) {\n",
            "  return y;\n",
            "}\n",
            "function boo({x:y$jscomp$1}) {\n",
            "  return y$jscomp$1\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testExportedOrImportedNamesAreUntouched
#[test]
fn test_exported_or_imported_names_are_untouched() {
    let mut t = Fixture::new();
    // The eventual desired behavior is that none of the 'a's in the following test cases

    // are renamed to a$jscomp$1. Rewrite this test after that behavior is implemented.

    t.use_default_renamer = true;
    t.test(
        srcs(&["var a;", "let a; export {a as a};"]),
        expected(&["var a;", "let a$jscomp$1; export {a$jscomp$1 as a};"]),
    );

    t.test(
        srcs(&["var a;", "import {a as a} from './foo.js'; let b = a;"]),
        expected(&[
            "var a;",
            "import {a as a$jscomp$1} from './foo.js'; let b = a$jscomp$1;",
        ]),
    );
}

// port: MakeDeclaredNamesUniqueTest#testTwoMethodsInTheSameFileWithSameLocalNames
#[test]
fn test_two_methods_in_the_same_file_with_same_local_names() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    // Verify same local names in 2 different files get different new names.

    // The ContextualRenamer renames an "oldName" by adding "$jscomp$\n" + "id" as a suffix string.

    // So, when another declaration containing "$jscomp$id" exists at any other source location in

    // the entire JS program (due to a prior renaming), the ContextualRenamer should not generate

    // that same name when renaming this declaration.

    t.test(
        "function foo() {var a; a;} function bar() {let a; let a$jscomp$1; a + a$jscomp$1;}",
        concat!(
            "function foo() {var a; a;}\n",
            "function bar() {\n",
            "  let a$jscomp$1; let a$jscomp$1$jscomp$1;\n",
            "  a$jscomp$1 + a$jscomp$1$jscomp$1;\n",
            "}\n"
        ),
    );

    t.test(
        "function bar() {let a; let a$jscomp$1; a + a$jscomp$1;} function foo() {var a; a;}",
        concat!(
            "function bar() {\n",
            "  let a; let a$jscomp$1;\n",
            "  a + a$jscomp$1;\n",
            "}\n",
            "function foo() {var a$jscomp$2; a$jscomp$2;}\n"
        ),
    );

    // tests when name with $jscomp$1 suffix comes first

    t.test(
        "function bar() {let a$jscomp$1; let a; a + a$jscomp$1;} function foo() {var a; a;}",
        concat!(
            "function bar() {\n",
            "  let a$jscomp$1; let a;\n",
            "  a + a$jscomp$1;\n",
            "}\n",
            "function foo() {var a$jscomp$2; a$jscomp$2;}\n"
        ),
    );

    t.test(
        concat!(
            "function bar() {\n",
            "  let a; let a$jscomp$1;\n",
            "  a + a$jscomp$1;\n",
            "}\n",
            "function foo() {\n",
            "// tests when a$jscomp$2 declared later in the same scope\n",
            "  var a; a; var a$jscomp$2; a$jscomp$2;\n",
            "}\n"
        ),
        concat!(
            "function bar() {\n",
            "  let a; let a$jscomp$1; a + a$jscomp$1;\n",
            "}\n",
            "function foo() {\n",
            "  var a$jscomp$2; a$jscomp$2;\n",
            "  var a$jscomp$2$jscomp$1; a$jscomp$2$jscomp$1;\n",
            "}\n"
        ),
    );

    t.test(
        concat!(
            "function bar() {\n",
            "  let a; let a$jscomp$1; a + a$jscomp$1;\n",
            "}\n",
            "function foo() {\n",
            "// tests when a$jscomp$2 declared first in the same scope\n",
            "  var a$jscomp$2; a$jscomp$2; var a; a;\n",
            "}\n"
        ),
        concat!(
            "function bar() {\n",
            "  let a; let a$jscomp$1; a + a$jscomp$1;\n",
            "}\n",
            "function foo() {\n",
            "  var a$jscomp$2; a$jscomp$2; var a$jscomp$3; a$jscomp$3;\n",
            "}\n"
        ),
    );

    t.test(
        concat!(
            "function bar() {\n",
            "  let a; let a$jscomp$1; a + a$jscomp$1;\n",
            "}\n",
            "function foo() {\n",
            "// tests when a$jscomp$2 is declared in another scope\n",
            "  var a; a;\n",
            "}\n",
            "function baz() {\n",
            "  var a$jscomp$2; a$jscomp$2;\n",
            "}\n"
        ),
        concat!(
            "function bar() {\n",
            "  let a; let a$jscomp$1; a + a$jscomp$1;\n",
            "}\n",
            "function foo() {\n",
            "  var a$jscomp$2; a$jscomp$2;\n",
            "}\n",
            "function baz() {\n",
            "  var a$jscomp$2$jscomp$1; a$jscomp$2$jscomp$1;\n",
            "}\n"
        ),
    );
}

// port: MakeDeclaredNamesUniqueTest#testTwoFilesWithSameLocalNames
#[test]
fn test_two_files_with_same_local_names() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    // Verify same local names in 2 different files get different new names

    t.test(
        srcs(&[
            "function foo() {var a; a;}",
            "function bar() {let a; let a$jscomp$1; a + a$jscomp$1;}",
        ]),
        expected(&[
            "function foo() {var a; a;}",
            concat!(
                "function bar() {let a$jscomp$1; let a$jscomp$1$jscomp$1; a$jscomp$1 +\n",
                " a$jscomp$1$jscomp$1;}\n"
            ),
        ]),
    );
}

// port: MakeDeclaredNamesUniqueTest#testImportStarWithInversion
#[test]
fn test_import_star_with_inversion() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.test_with_inversion(
        &[
            "let a = 5;",
            "import * as a          from './a.js'; const TAU = 2 * a.PI;",
        ],
        &[
            "let a = 5;",
            "import * as a$jscomp$1 from './a.js'; const TAU = 2 * a$jscomp$1.PI",
        ],
    );
}

// port: MakeDeclaredNamesUniqueTest#assertOnChange_throwsException
#[test]
fn assert_on_change_throws_exception() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.assert_on_change = true;

    let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.test_no_warning("var a; function foo() { var a = 1; } ")
    }))
    .expect_err("expected RuntimeException");
    let message = e
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| e.downcast_ref::<&str>().copied())
        .expect("panic message");
    assert!(message.contains("NAME a"), "{message}");
}

// port: MakeDeclaredNamesUniqueTest#assertOnChange_noExceptionIfNothingChanges
#[test]
fn assert_on_change_no_exception_if_nothing_changes() {
    let mut t = Fixture::new();
    t.use_default_renamer = true;
    t.assert_on_change = true;

    t.test_same("const x = 1; function foo() { const y = 2; }");
}
