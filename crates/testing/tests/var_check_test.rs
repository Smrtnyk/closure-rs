/*
 * Copyright 2005 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/testing/TestExternsBuilder.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/VarCheckTest.java.

//! Port of VarCheckTest (a CompilerTestCase) on the Rust CompilerTestCase port.
mod var_checks_support;

use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    check_level::CheckLevel,
    closure_primitive_errors::MISSING_MODULE_OR_PROVIDE,
    compiler_options::CompilerOptions,
    compiler_pass::CompilerPass,
    diagnostic_groups::{EXTERNS_VALIDATION, MODULE_LOAD, STRICT_MODULE_DEP_CHECK},
    diagnostic_type::DiagnosticType,
    var_check::{
        BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR, MISSING_CHUNK_DEP_ERROR,
        NAME_REFERENCE_IN_EXTERNS_ERROR, STRICT_CHUNK_DEP_ERROR, UNDEFINED_EXTERN_VAR_ERROR,
        UNDEFINED_VAR_ERROR, VAR_ARGUMENTS_SHADOWED_ERROR, VAR_MULTIPLY_DECLARED_ERROR,
        VIOLATED_CHUNK_DEP_ERROR, VarCheck,
    },
};
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, TestPart},
    jscomp_api::{JSChunk, SourceFile, SourceKind},
    replay::var_checks_helpers::VariableTestCheck,
    testing::{
        js_chunk_graph_builder::JSChunkGraphBuilder, test_externs_builder::TestExternsBuilder,
    },
};
use std::{cell::RefCell, rc::Rc};
use var_checks_support::{
    Hooks, diagnostic, error, expected, expected_n, externs, srcs, srcs_n, test_error,
    test_error_parts, test_extern_changes_parts, test_parts, test_same, test_same_parts,
    test_warning_parts, warning,
};

// port: VarCheckTest#EXTERNS
const EXTERNS: &str = "var window; function alert() {}";

/// The VarCheckTest fields that its getOptions and getProcessor overrides read.
struct Fields {
    // port: VarCheckTest#strictChunkDepErrorLevel
    strict_chunk_dep_error_level: CheckLevel,
    // port: VarCheckTest#validityCheck
    validity_check: bool,
    // port: VarCheckTest#externValidationErrorLevel
    extern_validation_error_level: Option<CheckLevel>,
    // port: VarCheckTest#declarationCheck
    declaration_check: bool,
}

type SharedFields = Rc<RefCell<Fields>>;

/// The anonymous CompilerPass of VarCheckTest#getProcessor; it reads the test's fields when it
/// runs.
struct GetProcessorPass {
    fields: SharedFields,
}

impl CompilerPass for GetProcessorPass {
    // port: VarCheckTest#getProcessor (the anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let validity_check = self.fields.borrow().validity_check;
        VarCheck::new_with_validity_check(compiler, validity_check)
            .process(compiler, externs, root);
        if !validity_check && !compiler.has_errors() {
            // If the original test turned off sanity check, make sure our synthesized
            // code passes it.
            VarCheck::new_with_validity_check(compiler, true).process(compiler, externs, root);
        }
        if self.fields.borrow().declaration_check {
            VariableTestCheck.process(compiler, externs, root);
        }
    }
}

// port: VarCheckTest#getOptions
fn get_options(fields: &SharedFields, options: &mut CompilerOptions) {
    let fields = fields.borrow();
    options.set_closure_pass(true);
    options.set_warning_level((*MODULE_LOAD).clone(), CheckLevel::OFF);
    options.set_warning_level(
        (*STRICT_MODULE_DEP_CHECK).clone(),
        fields.strict_chunk_dep_error_level,
    );
    if let Some(level) = fields.extern_validation_error_level {
        options.set_warning_level((*EXTERNS_VALIDATION).clone(), level);
    }
}

// port: VarCheckTest#VarCheckTest and VarCheckTest#setUp (with #getOptions and #getProcessor)
fn set_up() -> (CompilerTestCase, Hooks, SharedFields) {
    let mut harness = CompilerTestCase::new(EXTERNS);
    harness.set_up();
    // Setup value set by individual tests to the appropriate defaults.
    harness.allow_externs_changes().unwrap();
    let fields = Rc::new(RefCell::new(Fields {
        strict_chunk_dep_error_level: CheckLevel::OFF,
        extern_validation_error_level: None,
        validity_check: false,
        declaration_check: false,
    }));
    let for_pass = fields.clone();
    let for_options = fields.clone();
    let hooks = Hooks::new("VarCheckTest", move |_| {
        Box::new(GetProcessorPass {
            fields: for_pass.clone(),
        })
    })
    .with_options(move |options| get_options(&for_options, options));
    (harness, hooks, fields)
}

// port: CompilerTestCase#VAR_CHECK_EXTERNS
fn var_check_externs() -> String {
    var_checks_support::var_check_externs()
}

// port: TestExternsBuilder#getClosureExternsAsSource
fn closure_externs() -> String {
    TestExternsBuilder::get_closure_externs_as_source().to_string()
}

// port: CompilerTestCase#srcs(JSChunk...)
fn srcs_chunks(chunks: Vec<JSChunk>) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_chunks(chunks))
}

// port: CompilerTestCase#testNoWarning(Externs,Sources)
fn test_no_warning_parts(t: &mut CompilerTestCase, h: &mut Hooks, parts: Vec<TestPart>) {
    let [TestPart::Externs(ext), TestPart::Sources(src)] = <[TestPart; 2]>::try_from(parts)
        .ok()
        .expect("testNoWarning(Externs, Sources)")
    else {
        panic!("testNoWarning(Externs, Sources)");
    };
    t.test_no_warning_externs(h, ext, src).unwrap();
}

// port: VarCheckTest#testDependentChunks(String,String,DiagnosticType)
fn test_dependent_chunks(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    f: &SharedFields,
    code1: &str,
    code2: &str,
    error: Option<&'static DiagnosticType>,
) {
    test_dependent_chunks_with_warning(t, h, f, code1, code2, error, None);
}

// port: VarCheckTest#testDependentChunks(String,String,DiagnosticType,DiagnosticType)
fn test_dependent_chunks_with_warning(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    f: &SharedFields,
    code1: &str,
    code2: &str,
    error: Option<&'static DiagnosticType>,
    warning: Option<&'static DiagnosticType>,
) {
    test_two_chunks(t, h, f, code1, code2, true, error, warning);
}

// port: VarCheckTest#testIndependentChunks
fn test_independent_chunks(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    f: &SharedFields,
    code1: &str,
    code2: &str,
    error: Option<&'static DiagnosticType>,
    warning: Option<&'static DiagnosticType>,
) {
    test_two_chunks(t, h, f, code1, code2, false, error, warning);
}

// port: VarCheckTest#testTwoChunks
#[allow(clippy::too_many_arguments)]
fn test_two_chunks(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    _f: &SharedFields,
    code1: &str,
    code2: &str,
    m2_depends_onm1: bool,
    error_: Option<&'static DiagnosticType>,
    warning_: Option<&'static DiagnosticType>,
) {
    let m1 = JSChunk::new("m1");
    m1.add_source_file(SourceFile::from_code("input1", code1));
    let m2 = JSChunk::new("m2");
    m2.add_source_file(SourceFile::from_code("input2", code2));
    if m2_depends_onm1 {
        m2.add_dependency(&m1);
    }
    match (error_, warning_) {
        (None, None) => test_parts(
            t,
            h,
            vec![srcs_chunks(vec![m1, m2]), expected_n(&[code1, code2])],
        ),
        (None, Some(warning_)) => test_parts(
            t,
            h,
            vec![
                srcs_chunks(vec![m1, m2]),
                expected_n(&[code1, code2]),
                warning(warning_),
            ],
        ),
        (Some(error_), _) => test_error_parts(t, h, vec![srcs_chunks(vec![m1, m2]), error(error_)]),
    }
}

// port: VarCheckTest#checkSynthesizedExtern(Sources,String)
fn check_synthesized_extern(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    f: &SharedFields,
    input: TestPart,
    expected_extern: &str,
) {
    check_synthesized_extern_with(t, h, f, externs(""), input, expected_extern, vec![]);
}

// port: VarCheckTest#checkSynthesizedExtern(Externs,Sources,String,Diagnostic...)
fn check_synthesized_extern_with(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    f: &SharedFields,
    extern_: TestPart,
    input: TestPart,
    expected_extern: &str,
    warnings: Vec<TestPart>,
) {
    {
        let mut fields = f.borrow_mut();
        fields.declaration_check = !fields.validity_check;
    }
    t.disable_compare_as_tree().unwrap();
    let (TestPart::Externs(extern_), TestPart::Sources(input)) = (extern_, input) else {
        panic!("checkSynthesizedExtern(Externs, Sources, ...)");
    };
    let TestPart::Expected(expected_) = expected(expected_extern) else {
        unreachable!()
    };
    let warnings: Vec<_> = warnings.into_iter().map(diagnostic).collect();
    t.test_extern_changes(h, &extern_, &input, &expected_, &warnings, None)
        .unwrap();
}

#[test]
fn test_shorthand_obj_lit() {
    let (mut t, mut h, _f) = set_up();
    test_error(&mut t, &mut h, "var x = {y};", &UNDEFINED_VAR_ERROR);
    test_same(&mut t, &mut h, "var {x} = {x: 5}; let y = x;");
    test_error(&mut t, &mut h, "var {...x} = {...y};", &UNDEFINED_VAR_ERROR);
    test_same(&mut t, &mut h, "let y; var {...x} = {...y} = {};");
    test_same(&mut t, &mut h, "var {...x} = {x: 5}; let y = x;");
}

#[test]
fn test_break() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "a: while(1) break a;");
}

#[test]
fn test_continue() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "a: while(1) continue a;");
}

#[test]
fn test_referenced_var_not_defined() {
    let (mut t, mut h, _f) = set_up();
    test_error(&mut t, &mut h, "x = 0;", &UNDEFINED_VAR_ERROR);
}

#[test]
fn test_referenced_var_not_defined_typeof() {
    let (mut t, mut h, _f) = set_up();
    // typeof x !== 'undefined' is the strict mode compliant way to check for a variable's
    // existence.
    test_same(
        &mut t,
        &mut h,
        "if (typeof undeclaredVariable !== 'undefined') {}",
    );
    // Regression test: the exact pattern emitted by TypeScript to reference possibly declared
    // generic type variables in decorator emit.
    test_same(
        &mut t,
        &mut h,
        "var _a; typeof (_a = typeof Value !== \"undefined\" && Value) === \"function\"",
    );
}

#[test]
fn test_referenced_var_not_defined_arrow_function_body() {
    let (mut t, mut h, _f) = set_up();
    test_error(&mut t, &mut h, "() => y", &UNDEFINED_VAR_ERROR);
}

#[test]
fn test_referenced_let_not_defined() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "{ let x = 1; } var y = x;",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_referenced_let_not_defined_with_es6_modules() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "export function f() { { let x = 1; } var y = x; }",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_referenced_let_defined1() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "let x; x = 1;");
}

#[test]
fn test_nullish_coalesce() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "let x; x = 0 ?? true");
    test_error(
        &mut t,
        &mut h,
        "let x; x = a ?? \"hi\"",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_logical_assignment() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "let x; x ||= 2");
    test_error(&mut t, &mut h, "let x; x ||= a", &UNDEFINED_VAR_ERROR);
    // References let, not defined
    test_error(
        &mut t,
        &mut h,
        "{ let x = 1; } var y; y ||= x;",
        &UNDEFINED_VAR_ERROR,
    );
    // Multiple declared vars
    test_same(&mut t, &mut h, "try { var x = 1; x ||= 2; } catch (x) {}");
    // In externs
    test_parts(
        &mut t,
        &mut h,
        vec![
            externs("x ||= new Klass();"),
            srcs("class Klass{}"),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_referenced_let_defined1_with_es6_modules() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "export let x; x = 1;");
}

#[test]
fn test_referenced_let_defined2() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "let x; function y() {x = 1;}");
}

#[test]
fn test_referenced_const_defined2() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "const x = 1; var y = x + 1;");
}

#[test]
fn test_referenced_var_defined1() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "var x, y; x=1;");
}

#[test]
fn test_referenced_var_defined2() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "var x; function y() {x=1;}");
}

#[test]
fn test_referenced_vars_externally_defined() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "var x = window; alert(x);");
}

#[test]
fn test_multiply_declared_vars1() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "var x = 1; var x = 2;",
        &VAR_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_multiply_declared_vars2() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        "var y; try { y=1 } catch (x) {} try { y=1 } catch (x) {}",
    );
}

#[test]
fn test_multiply_declared_vars3() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "try { var x = 1; x *=2; } catch (x) {}");
}

#[test]
fn test_multiply_declared_vars4() {
    let (mut t, mut h, _f) = set_up();
    test_parts(
        &mut t,
        &mut h,
        vec![
            externs("x;"),
            srcs("var x = 1; var x = 2;"),
            error(&VAR_MULTIPLY_DECLARED_ERROR),
        ],
    );
}

#[test]
fn test_multiply_declared_lets() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "let x = 1; let x = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "let x = 1; var x = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "var x = 1; let x = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "let {x} = 1; let {x} = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "let {x} = 1; var {x} = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "var {x} = 1; let {x} = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_multiply_declared_consts() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "const x = 1; const x = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "const x = 1; var x = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "var x = 1; const x = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "const {x} = 1; const {x} = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "const {x} = 1; var {x} = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "var {x} = 1; const {x} = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_multiply_declared_consts_with_es6_modules() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "export function f() { const x = 1; const x = 2; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "export const x = 1; export var x = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "export const a = 1, a = 2;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_multiply_declare_lets_in_different_scope() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "let x = 1; if (123) {let x = 2;}");
    test_same(&mut t, &mut h, "try {let x = 1;} catch(x){}");
}

#[test]
fn test_multiply_declared_catch_and_var() {
    let (mut t, mut h, _f) = set_up();
    // Note: This is technically valid code, but it's difficult to model the scoping rules in the
    // compiler and inconsistent across browsers. We forbid it in VariableReferenceCheck.
    test_same(&mut t, &mut h, "try {} catch (x) { var x = 1; }");
}

#[test]
fn test_multiply_declared_catch_and_let() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "try {} catch (x) { let x = 1; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_multiply_declared_catch_and_const() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "try {} catch (x) { const x = 1; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_referenced_var_defined_class() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "var x; class x{ }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "let x; class x{ }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "const x = 1; class x{ }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "class x{ } let x;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "export default class x{ } let x;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_messages_point_to_first_occurance() {
    let (mut t, mut h, _f) = set_up();
    test_parts(
        &mut t,
        &mut h,
        vec![
            srcs("var x = 1; var x = 2;"),
            TestPart::Diagnostic(
                diagnostic(error(&VAR_MULTIPLY_DECLARED_ERROR))
                    .with_message_containing("First occurrence: testcode:1:4")
                    .unwrap(),
            ),
        ],
    );
    test_parts(
        &mut t,
        &mut h,
        vec![
            srcs("var x; class x{ }"),
            TestPart::Diagnostic(
                diagnostic(error(&BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR))
                    .with_message_containing("First occurrence: testcode:1:4")
                    .unwrap(),
            ),
        ],
    );
}

#[test]
fn test_named_class() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "class x {}");
    test_same(&mut t, &mut h, "var x = class x {};");
    test_same(&mut t, &mut h, "var y = class x {};");
    test_same(
        &mut t,
        &mut h,
        "var y = class x { foo() { return new x; } };",
    );
    test_error(
        &mut t,
        &mut h,
        "var Foo = class extends Bar {};",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_var_reference_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("asdf;"),
            srcs("var /** @suppress {duplicate} */ asdf;"),
            warning(&NAME_REFERENCE_IN_EXTERNS_ERROR),
        ],
    );
}

#[test]
fn test_var_reference_in_externs2() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("asdf;"),
            srcs("(function() { var asdf; })()\nvar /** @suppress {duplicate} */ asdf;\n"),
            warning(&NAME_REFERENCE_IN_EXTERNS_ERROR),
        ],
    );
}

#[test]
fn test_namespace_declaration_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("/** @const */ var $jscomp = $jscomp || {};"),
            srcs(""),
        ],
    );
}

#[test]
fn test_call_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("yz();"),
            srcs("function /** @suppress {duplicate} */ yz() {}"),
            warning(&NAME_REFERENCE_IN_EXTERNS_ERROR),
        ],
    );
}

#[test]
fn test_destructuring_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![externs("function externalFunction({x, y}) {}"), srcs("")],
    );
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("function externalFunction({x, y:{z}}) {}"),
            srcs(""),
        ],
    );
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("function externalFunction({x:localName}) {}"),
            srcs(""),
        ],
    );
    test_same_parts(
        &mut t,
        &mut h,
        vec![externs("function externalFunction([a, b, c]) {}"), srcs("")],
    );
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("function externalFunction([[...a], b, c = 5, ...d]) {}"),
            srcs(""),
        ],
    );
}

#[test]
fn test_var_reference_in_externs_with_es6_modules() {
    let (mut t, mut h, _f) = set_up();
    // vars in ES6 modules are not in global scope, so foo is undefined.
    test_error_parts(
        &mut t,
        &mut h,
        vec![
            externs("foo;"),
            srcs("export var foo;"),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_var_declaration_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(&mut t, &mut h, vec![externs("var asdf;"), srcs("asdf;")]);
}

#[test]
fn test_var_reference_in_type_summary() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs(
                "/** @typeSummary */\nvar goog;\ngoog.addSingletonGetter;\nclass Foo {}\ngoog.addSingletonGetter(Foo);\n",
            ),
            srcs("Foo.getInstance();"),
        ],
    );
}

#[test]
fn test_function_declaration_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![externs("function foo(x = 7) {}"), srcs("foo();")],
    );
    test_same_parts(
        &mut t,
        &mut h,
        vec![externs("function foo(...rest) {}"), srcs("foo(1,2,3);")],
    );
}

#[test]
fn test_var_assignment_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("/** @type{{foo:string}} */ var foo; var asdf = foo;"),
            srcs("asdf.foo;"),
        ],
    );
}

#[test]
fn test_aliases_in_externs() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().extern_validation_error_level = Some(CheckLevel::ERROR);
    test_same_parts(
        &mut t,
        &mut h,
        vec![externs("var foo; /** @const */ var asdf = foo;"), srcs("")],
    );
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("var Foo; var ns = {}; /** @const */ ns.FooAlias = Foo;"),
            srcs(""),
        ],
    );
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs(
                "var ns = {}; /** @constructor */ ns.Foo = function() {};\nvar ns2 = {}; /** @const */ ns2.Bar = ns.Foo;\n",
            ),
            srcs(""),
        ],
    );
}

#[test]
fn test_duplicate_namespace_in_externs() {
    let (mut t, mut h, _f) = set_up();
    // Compare as tree does not allow multiple externs, but VarCheck forcibly inserts them.
    t.disable_compare_as_tree().unwrap();
    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![
            externs("/** @const */ var ns = {}; /** @const */ var ns = {};"),
            srcs(""),
            expected(&[var_check_externs().as_str(), "/** @const */ var ns = {};"].concat()),
        ],
    );
}

#[test]
fn test_let_declaration_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(&mut t, &mut h, vec![externs("let asdf;"), srcs("asdf;")]);
}

#[test]
fn test_const_declaration_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![externs("const asdf = 1;"), srcs("asdf;")],
    );
}

#[test]
fn test_new_in_externs() {
    let (mut t, mut h, _f) = set_up();
    // Class is not hoisted.
    test_parts(
        &mut t,
        &mut h,
        vec![
            externs("x = new Klass();"),
            srcs("class Klass{}"),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_prop_reference_in_externs1() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("asdf.foo;"),
            srcs("var /** @suppress {duplicate} */ asdf;"),
            warning(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
}

#[test]
fn test_prop_reference_in_externs2() {
    let (mut t, mut h, _f) = set_up();
    test_parts(
        &mut t,
        &mut h,
        vec![externs("asdf.foo;"), srcs(""), error(&UNDEFINED_VAR_ERROR)],
    );
}

#[test]
fn test_prop_reference_in_externs3() {
    let (mut t, mut h, f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("asdf.foo;"),
            srcs("var /** @suppress {duplicate} */ asdf;"),
            warning(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
    f.borrow_mut().extern_validation_error_level = Some(CheckLevel::ERROR);
    test_parts(
        &mut t,
        &mut h,
        vec![
            externs("asdf.foo;"),
            srcs("var asdf;"),
            error(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
    f.borrow_mut().extern_validation_error_level = Some(CheckLevel::OFF);
    test_same_parts(
        &mut t,
        &mut h,
        vec![externs("asdf.foo;"), srcs("var asdf;")],
    );
}

#[test]
fn test_prop_reference_in_externs4() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("asdf.foo;"),
            srcs("let asdf;"),
            warning(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
}

#[test]
fn test_prop_reference_in_externs5() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            externs("asdf.foo;"),
            srcs("class asdf {}"),
            warning(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
}

#[test]
fn test_var_in_with_block() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "var a = {b:5}; with (a){b;}",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_function_declared_in_block() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "if (true) {function foo() {}} foo();",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "foo(); if (true) {function foo() {}}",
        &UNDEFINED_VAR_ERROR,
    );
    test_same(&mut t, &mut h, "if (true) {var foo = ()=>{}} foo();");
    test_error(
        &mut t,
        &mut h,
        "if (true) {let foo = ()=>{}} foo();",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "if (true) {const foo = ()=>{}} foo();",
        &UNDEFINED_VAR_ERROR,
    );
    test_same(&mut t, &mut h, "foo(); if (true) {var foo = ()=>{}}");
    test_error(
        &mut t,
        &mut h,
        "foo(); if (true) {let foo = ()=>{}}",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "foo(); if (true) {const foo = ()=>{}}",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_valid_function_expr() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "(function() {});");
}

#[test]
fn test_recursive_function() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "(function a() { return a(); })();");
}

#[test]
fn test_recursive_function2() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        "var a = 3; (function a() { return a(); })();",
    );
}

#[test]
fn test_param() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "function fn(a){ var b = a; }");
    test_same(&mut t, &mut h, "function fn(a){ var a = 2; }");
    test_error(
        &mut t,
        &mut h,
        "function fn(){ var b = a; }",
        &UNDEFINED_VAR_ERROR,
    );
    // Default parameters
    test_error(
        &mut t,
        &mut h,
        "function fn(a = b) { function g(a = 3) { var b; } }",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x=a) { let a; }",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x=a) { { let a; } }",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x=b) { function a(x=1) { var b; } }",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x=a) { var a; }",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x=a()) { function a() {} }",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x=[a]) { var a; }",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x = new foo.bar()) {}",
        &UNDEFINED_VAR_ERROR,
    );
    test_same(
        &mut t,
        &mut h,
        "var foo = {}; foo.bar = class {}; function f(x = new foo.bar()) {}",
    );
    test_same(&mut t, &mut h, "function fn(a = 2){ var b = a; }");
    test_same(&mut t, &mut h, "function fn(a = 2){ var a = 3; }");
    test_same(&mut t, &mut h, "function fn({a, b}){ var c = a; }");
    test_same(&mut t, &mut h, "function fn({a, b}){ var a = 3; }");
}

#[test]
fn test_param_arrow_function() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "(a) => { var b = a; }");
    test_error(&mut t, &mut h, "() => { var b = a; }", &UNDEFINED_VAR_ERROR);
    test_error(&mut t, &mut h, "(x=a) => { let a; }", &UNDEFINED_VAR_ERROR);
    // Arrow function nested
    test_error(
        &mut t,
        &mut h,
        "function FUNC() {\n  {\n    () => { var b = a; }\n  }\n}\n",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_legal_var_reference_between_chunks() {
    let (mut t, mut h, f) = set_up();
    test_dependent_chunks(&mut t, &mut h, &f, "var x = 10;", "var y = x++;", None);
}

#[test]
fn test_legal_let_reference_between_chunks() {
    let (mut t, mut h, f) = set_up();
    test_dependent_chunks(&mut t, &mut h, &f, "let x = 10;", "let y = x++;", None);
}

#[test]
fn test_legal_const_reference_between_chunks() {
    let (mut t, mut h, f) = set_up();
    test_dependent_chunks(
        &mut t,
        &mut h,
        &f,
        "const x = 10;",
        "const y = x + 1;",
        None,
    );
}

#[test]
fn test_missing_chunk_dependency_default() {
    let (mut t, mut h, f) = set_up();
    test_independent_chunks(
        &mut t,
        &mut h,
        &f,
        "var x = 10;",
        "var y = x++;",
        None,
        Some(&MISSING_CHUNK_DEP_ERROR),
    );
}

#[test]
fn test_missing_chunk_dependency_let_and_const() {
    let (mut t, mut h, f) = set_up();
    test_independent_chunks(
        &mut t,
        &mut h,
        &f,
        "let x = 10;",
        "let y = x++;",
        None,
        Some(&MISSING_CHUNK_DEP_ERROR),
    );
    test_independent_chunks(
        &mut t,
        &mut h,
        &f,
        "const x = 10;",
        "const y = x + 1;",
        None,
        Some(&MISSING_CHUNK_DEP_ERROR),
    );
}

#[test]
fn test_violated_chunk_dependency_default() {
    let (mut t, mut h, f) = set_up();
    test_dependent_chunks(
        &mut t,
        &mut h,
        &f,
        "var y = x++;",
        "var x = 10;",
        Some(&VIOLATED_CHUNK_DEP_ERROR),
    );
}

#[test]
fn test_violated_chunk_dependency_let_and_const() {
    let (mut t, mut h, f) = set_up();
    test_dependent_chunks(
        &mut t,
        &mut h,
        &f,
        "let y = x++;",
        "let x = 10;",
        Some(&VIOLATED_CHUNK_DEP_ERROR),
    );
    test_dependent_chunks(
        &mut t,
        &mut h,
        &f,
        "const y = x + 1;",
        "const x = 10;",
        Some(&VIOLATED_CHUNK_DEP_ERROR),
    );
}

#[test]
fn test_missing_chunk_dependency_skip_non_strict() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().validity_check = true;
    test_independent_chunks(
        &mut t,
        &mut h,
        &f,
        "var x = 10;",
        "var y = x++;",
        None,
        None,
    );
}

#[test]
fn test_violated_chunk_dependency_skip_non_strict() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().validity_check = true;
    test_dependent_chunks(&mut t, &mut h, &f, "var y = x++;", "var x = 10;", None);
}

#[test]
fn test_missing_chunk_dependency_skip_non_strict_not_promoted() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().validity_check = true;
    f.borrow_mut().strict_chunk_dep_error_level = CheckLevel::ERROR;
    test_independent_chunks(
        &mut t,
        &mut h,
        &f,
        "var x = 10;",
        "var y = x++;",
        None,
        None,
    );
}

#[test]
fn test_violated_chunk_dependency_non_strict_not_promoted() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().validity_check = true;
    f.borrow_mut().strict_chunk_dep_error_level = CheckLevel::ERROR;
    test_dependent_chunks(&mut t, &mut h, &f, "var y = x++;", "var x = 10;", None);
}

#[test]
fn test_dependent_strict_chunk_dependency_check() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().strict_chunk_dep_error_level = CheckLevel::ERROR;
    test_dependent_chunks(
        &mut t,
        &mut h,
        &f,
        "var f = function() {return new B();};",
        "var B = function() {}",
        Some(&STRICT_CHUNK_DEP_ERROR),
    );
}

#[test]
fn test_independent_strict_chunk_dependency_check() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().strict_chunk_dep_error_level = CheckLevel::ERROR;
    test_independent_chunks(
        &mut t,
        &mut h,
        &f,
        "var f = function() {return new B();};",
        "var B = function() {}",
        Some(&STRICT_CHUNK_DEP_ERROR),
        None,
    );
}

#[test]
fn test_star_strict_chunk_dependency_check() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().strict_chunk_dep_error_level = CheckLevel::WARNING;
    test_same_parts(
        &mut t,
        &mut h,
        vec![
            srcs_chunks(
                JSChunkGraphBuilder::for_star()
                    .add_chunk("function a() {}")
                    .add_chunk("function b() { a(); c(); }")
                    .add_chunk("function c() { a(); }")
                    .build(),
            ),
            warning(&STRICT_CHUNK_DEP_ERROR),
        ],
    );
}

#[test]
fn test_forward_var_reference_in_local_scope1() {
    let (mut t, mut h, f) = set_up();
    test_dependent_chunks(
        &mut t,
        &mut h,
        &f,
        "var x = 10; function a() {y++;}",
        "var y = 11; a();",
        None,
    );
}

#[test]
fn test_forward_var_reference_in_local_scope2() {
    let (mut t, mut h, f) = set_up();
    // It would be nice if this pass could use a call graph to flag this case
    // as an error, but it currently doesn't.
    test_dependent_chunks(
        &mut t,
        &mut h,
        &f,
        "var x = 10; function a() {y++;} a();",
        "var y = 11;",
        None,
    );
}

#[test]
fn test_simple() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern(
        &mut t,
        &mut h,
        &f,
        srcs("x"),
        &["var x;", var_check_externs().as_str()].concat(),
    );
    check_synthesized_extern(
        &mut t,
        &mut h,
        &f,
        srcs("var x"),
        var_check_externs().as_str(),
    );
}

#[test]
fn test_simple_validity_check() {
    let (mut t, mut h, f) = set_up();
    f.borrow_mut().validity_check = true;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_synthesized_extern(&mut t, &mut h, &f, srcs("x"), "");
    }));
    let Err(e) = result else {
        panic!("Expected RuntimeException");
    };
    let message = e
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        message.contains("Unexpected variable x"),
        "unexpected message: {message}"
    );
}

#[test]
fn test_parameter() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern(
        &mut t,
        &mut h,
        &f,
        srcs("function f(x){}"),
        var_check_externs().as_str(),
    );
}

#[test]
fn test_local_var() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern(
        &mut t,
        &mut h,
        &f,
        srcs("function f(){x}"),
        &["var x;", var_check_externs().as_str()].concat(),
    );
}

#[test]
fn test_two_local_vars() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern(
        &mut t,
        &mut h,
        &f,
        srcs("function f(){x}function g() {x}"),
        &["var x;", var_check_externs().as_str()].concat(),
    );
}

#[test]
fn test_inner_function_local_var() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern(
        &mut t,
        &mut h,
        &f,
        srcs("function f(){function g() {x}}"),
        &["var x;", var_check_externs().as_str()].concat(),
    );
}

#[test]
fn test_no_create_vars_for_labels() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern(
        &mut t,
        &mut h,
        &f,
        srcs("x:var y"),
        var_check_externs().as_str(),
    );
}

#[test]
fn test_variable_in_normal_code_used_in_externs1() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern_with(
        &mut t,
        &mut h,
        &f,
        externs("x.foo;"),
        srcs("var x;"),
        &["var x; ", var_check_externs().as_str(), " x.foo;"].concat(),
        vec![warning(&UNDEFINED_EXTERN_VAR_ERROR)],
    );
}

#[test]
fn test_variable_in_normal_code_used_in_externs2() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern_with(
        &mut t,
        &mut h,
        &f,
        externs("x;"),
        srcs("var x;"),
        &["var x; ", var_check_externs().as_str(), " x;"].concat(),
        vec![warning(&NAME_REFERENCE_IN_EXTERNS_ERROR)],
    );
}

#[test]
fn test_variable_in_normal_code_used_in_externs3() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern_with(
        &mut t,
        &mut h,
        &f,
        externs("x.foo;"),
        srcs("function x() {}"),
        &["var x; ", var_check_externs().as_str(), " x.foo;"].concat(),
        vec![warning(&UNDEFINED_EXTERN_VAR_ERROR)],
    );
}

#[test]
fn test_variable_in_normal_code_used_in_externs4() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern_with(
        &mut t,
        &mut h,
        &f,
        externs("x;"),
        srcs("function x() {}"),
        &["var x; ", var_check_externs().as_str(), " x;"].concat(),
        vec![warning(&NAME_REFERENCE_IN_EXTERNS_ERROR)],
    );
}

#[test]
fn test_redeclaration1() {
    let (mut t, mut h, _f) = set_up();
    let js = "var a; var a;";
    test_error(&mut t, &mut h, js, &VAR_MULTIPLY_DECLARED_ERROR);
}

#[test]
fn test_redeclaration2() {
    let (mut t, mut h, _f) = set_up();
    let js = "var a; /** @suppress {duplicate} */ var a;";
    test_same(&mut t, &mut h, js);
}

#[test]
fn test_redeclaration3() {
    let (mut t, mut h, _f) = set_up();
    let js = " /** @suppress {duplicate} */ var a; var a; ";
    test_same(&mut t, &mut h, js);
}

#[test]
fn test_redeclaration4() {
    let (mut t, mut h, _f) = set_up();
    let js = "/** @fileoverview @suppress {duplicate} */\n/** @type {string} */ var a;\nvar a; \n";
    test_same(&mut t, &mut h, js);
}

#[test]
fn test_suppression_with_inline_js_doc() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        "/** @suppress {duplicate} */ var /** number */ a; var a;",
    );
}

#[test]
fn test_duplicate_var() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "/** @define {boolean} */ var DEF = false; var DEF = true;",
        &VAR_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_dont_allow_suppress_dupe_on_let() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "let a; /** @suppress {duplicate} */ let a; ",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f() { let a; /** @suppress {duplicate} */ let a; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_duplicate_block_scoped_declaration_in_switch() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "function f(x) {\n  switch (x) {\n    case 'a':\n      let z = 123;\n      break;\n    case 'b':\n      let z = 234;\n      break;\n  }\n}\n",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f(x) {\n  switch (x) {\n    case 'a':\n      class C {}\n      break;\n    case 'b':\n      class C {}\n      break;\n  }\n}\n",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_let_const_redeclare_with_functions_with_es6_modules() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "function f() {} let f = 1; export {f};",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "let f = 1; function f() {}  export {f};",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "const f = 1; function f() {} export {f};",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "function f() {} const f = 1;  export {f};",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "export default function f() {}; let f = 5;",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_function_scope_arguments() {
    let (mut t, mut h, f) = set_up();
    // A var declaration doesn't mask arguments
    test_same(&mut t, &mut h, "function f() {var arguments}");
    test_error(
        &mut t,
        &mut h,
        "var f = function arguments() {}",
        &VAR_ARGUMENTS_SHADOWED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "var f = function (arguments) {}",
        &VAR_ARGUMENTS_SHADOWED_ERROR,
    );
    test_same(&mut t, &mut h, "function f() {try {} catch(arguments) {}}");
    f.borrow_mut().validity_check = true;
    test_same(&mut t, &mut h, "function f() {var arguments}");
}

#[test]
fn test_function_redeclared_global() {
    let (mut t, mut h, _f) = set_up();
    // Redeclaration in global scope is allowed.
    test_same(
        &mut t,
        &mut h,
        "/** @fileoverview @suppress {duplicate} */ function f() {};function f() {};",
    );
}

#[test]
fn test_function_redeclared1() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "{ function f() {}; function f() {}; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "if (0) { function f() {}; function f() {}; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "try { function f() {}; function f() {}; } catch (e) {}",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_function_redeclared2() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "{ let f = 0; function f() {}; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "if (0) { const f = 1; function f() {}; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "try { class f {}; function f() {}; } catch (e) {}",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_function_redeclared3() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "{ function f() {}; let f = 0; }",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "if (0) { function f() {}; const f = 0;}",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
    test_error(
        &mut t,
        &mut h,
        "try { function f() {}; class f {}; } catch (e) {}",
        &BLOCK_SCOPED_DECL_MULTIPLY_DECLARED_ERROR,
    );
}

#[test]
fn test_undeclared_var_for_goog() {
    let (mut t, mut h, _f) = set_up();
    t.enable_closure_pass().unwrap();
    t.enable_rewrite_closure_code().unwrap();
    // We don't want to get goog as an undeclared var here.
    test_parts(
        &mut t,
        &mut h,
        vec![
            srcs("goog.require('namespace.Class1');\n"),
            error(&MISSING_MODULE_OR_PROVIDE),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_imported_name_collision() {
    let (mut t, mut h, _f) = set_up();
    // TODO(tbreisacher): This should throw a duplicate declaration error.
    test_same(
        &mut t,
        &mut h,
        "import foo from './foo'; foo('hello'); var foo = 5;",
    );
}

#[test]
fn test_import_star() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "import * as foo from './foo.js';");
}

#[test]
fn test_export_as_alias() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "let a = 1; export {a as b};");
    test_error(
        &mut t,
        &mut h,
        "let a = 1; export {b as a};",
        &UNDEFINED_VAR_ERROR,
    );
    test_error(&mut t, &mut h, "export {a as a};", &UNDEFINED_VAR_ERROR);
    // Make sure non-aliased exports still work correctly.
    test_same(&mut t, &mut h, "let a = 1; export {a}");
    test_error(
        &mut t,
        &mut h,
        "let a = 1; export {b};",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_import_as_alias() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        "import {b as a} from './foo.js'; let c = a;",
    );
    test_error(
        &mut t,
        &mut h,
        "import {b as a} from './foo.js'; let c = b;",
        &UNDEFINED_VAR_ERROR,
    );
    test_same(&mut t, &mut h, "import {a} from './foo.js'; let c = a;");
}

#[test]
fn test_computed_property_with_named_function() {
    let (mut t, mut h, _f) = set_up();
    test_same(&mut t, &mut h, "({[0]: function f() {}})");
}

#[test]
fn test_es_module_with_undefined_exports_ref() {
    let (mut t, mut h, _f) = set_up();
    test_error(
        &mut t,
        &mut h,
        "exports = function() {}; export {exports};",
        &UNDEFINED_VAR_ERROR,
    );
}

#[test]
fn test_goog_module_with_default_exports() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_n(&[
            closure_externs().as_str(),
            "goog.module('a.b'); exports = function() {};",
        ])],
    );
}

#[test]
fn test_goog_module_with_exports_ref_in_function() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_n(&[
            closure_externs().as_str(),
            "goog.module('a.b'); exports.f = function() { exports.f(); };",
        ])],
    );
}

#[test]
fn test_goog_module_with_named_exports() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_n(&[
            closure_externs().as_str(),
            "goog.module('a.b'); exports.f = function() {}; exports.x = 0;",
        ])],
    );
}

#[test]
fn test_goog_provide_simple_name() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "goog.provide('A'); var A = class {};",
        ]
        .concat(),
    );
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "goog.provide('A'); A = class {};",
        ]
        .concat(),
    );
}

#[test]
fn test_goog_provide_simple_name_early_reference() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "A.B = class {}; goog.provide('A');",
        ]
        .concat(),
    );
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "A.B = class {}; goog.provide('A'); var A = class {};",
        ]
        .concat(),
    );
}

#[test]
fn test_goog_provide_multiple_roots_in_same_file() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "goog.provide('A'); goog.provide('B'); var A = class {}; var B = class {};",
        ]
        .concat(),
    );
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "goog.provide('A.a'); goog.provide('B.b'); A.a = 0; B.b = 1",
        ]
        .concat(),
    );
}

#[test]
fn test_goog_provide_complex_name() {
    let (mut t, mut h, _f) = set_up();
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "goog.provide('foo.A'); foo.A = class {};",
        ]
        .concat(),
    );
    test_same(
        &mut t,
        &mut h,
        &[
            closure_externs().as_str(),
            "goog.provide('foo.bar.A'); foo.bar.A = class {};",
        ]
        .concat(),
    );
}

#[test]
fn test_goog_legacy_module() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_n(&[
            closure_externs().as_str(),
            "goog.module('foo.A'); goog.module.declareLegacyNamespace(); exports = class {};",
            "new foo.A();",
        ])],
    );
}

#[test]
fn test_goog_non_legacy_module() {
    let (mut t, mut h, _f) = set_up();
    test_error_parts(
        &mut t,
        &mut h,
        vec![
            srcs_n(&[
                closure_externs().as_str(),
                "goog.module('foo.A');",
                "foo.A();",
            ]),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
    test_error_parts(
        &mut t,
        &mut h,
        vec![
            srcs_n(&[
                closure_externs().as_str(),
                "goog.module('foo.A'); exports = class {};",
                "new foo.A();",
            ]),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_goog_legacy_module_in_load_module() {
    let (mut t, mut h, _f) = set_up();
    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_n(&[
            closure_externs().as_str(),
            "goog.loadModule(function(exports) {\n  goog.module('foo.A');\n  goog.module.declareLegacyNamespace();\n  exports = class {};\n  return exports;\n});\n",
            "new foo.A();",
        ])],
    );
}

#[test]
fn test_goog_non_legacy_module_in_load_module() {
    let (mut t, mut h, _f) = set_up();
    test_error_parts(
        &mut t,
        &mut h,
        vec![
            srcs_n(&[
                closure_externs().as_str(),
                "goog.loadModule(function(exports) {\n  goog.module('foo.A');\n  exports = class {};\n  return exports;\n});\n",
                "new foo.A();",
            ]),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_goog_provide_externs_adds_decl_for_namespace() {
    let (mut t, mut h, f) = set_up();
    check_synthesized_extern_with(
        &mut t,
        &mut h,
        &f,
        externs("var goog; goog.provide('a.b'); a.b.C = class {};"),
        srcs(""),
        &[
            var_check_externs().as_str(),
            "var goog;goog.provide('a.b');a.b.C = class {};",
        ]
        .concat(),
        vec![],
    );
}

#[test]
fn test_goog_forward_declare_can_be_referenced_in_externs_if_in_code() {
    let (mut t, mut h, f) = set_up();
    test_no_warning_parts(
        &mut t,
        &mut h,
        vec![
            externs("goog.forwardDeclare('a.b.C');"),
            srcs("var /** !a.b.C */ c; var goog;"),
        ],
    );
    check_synthesized_extern_with(
        &mut t,
        &mut h,
        &f,
        externs("goog.forwardDeclare('a.b.C');"),
        srcs("var goog;"),
        &[
            var_check_externs().as_str(),
            "goog.forwardDeclare('a.b.C');",
        ]
        .concat(),
        vec![],
    );
}

#[test]
fn test_arbitrary_property_named_forward_declare_cannot_be_referenced_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_warning_parts(
        &mut t,
        &mut h,
        vec![
            externs("notGoog.forwardDeclare('a.b.C');"),
            srcs("var notGoog;"),
            warning(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
}

#[test]
fn test_goog_provide_cannot_be_referenced_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_warning_parts(
        &mut t,
        &mut h,
        vec![
            externs("goog.provide('a.b');"),
            srcs("var goog;"),
            warning(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
}

#[test]
fn test_goog_module_cannot_be_referenced_in_externs() {
    let (mut t, mut h, _f) = set_up();
    test_warning_parts(
        &mut t,
        &mut h,
        vec![
            externs("goog.module('a.b');"),
            srcs("var goog;"),
            warning(&UNDEFINED_EXTERN_VAR_ERROR),
        ],
    );
}

#[test]
fn test_reference_to_weak_var_from_strong_file() {
    let (mut t, mut h, _f) = set_up();
    let weak_chunk = JSChunk::new(JSChunk::WEAK_CHUNK_NAME);
    let strong_chunk = JSChunk::new(JSChunk::STRONG_CHUNK_NAME);
    weak_chunk.add_dependency(&strong_chunk);

    weak_chunk.add_source_file(SourceFile::from_code_with_kind(
        "weak.js",
        "var weakVar = 0;",
        SourceKind::WEAK,
    ));

    strong_chunk.add_source_file(SourceFile::from_code_with_kind(
        "strong.js",
        "weakVar();",
        SourceKind::STRONG,
    ));

    test_parts(
        &mut t,
        &mut h,
        vec![
            srcs_chunks(vec![strong_chunk, weak_chunk]),
            error(&VIOLATED_CHUNK_DEP_ERROR),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_reference_to_weak_var_from_weak_file() {
    let (mut t, mut h, _f) = set_up();
    let weak_chunk = JSChunk::new(JSChunk::WEAK_CHUNK_NAME);
    weak_chunk.add_source_file(SourceFile::from_code_with_kind(
        "weak.js",
        "var weakVar = 0;\nweakVar();\n",
        SourceKind::WEAK,
    ));

    test_same_parts(&mut t, &mut h, vec![srcs_chunks(vec![weak_chunk])]);
}

#[test]
fn test_reference_to_weak_namespace_root_from_strong_file() {
    let (mut t, mut h, _f) = set_up();
    let weak_chunk = JSChunk::new(JSChunk::WEAK_CHUNK_NAME);
    let strong_chunk = JSChunk::new(JSChunk::STRONG_CHUNK_NAME);
    weak_chunk.add_dependency(&strong_chunk);

    weak_chunk.add_source_file(SourceFile::from_code_with_kind(
        "weak.js",
        [closure_externs().as_str(), "goog.provide('foo.bar');"].concat(),
        SourceKind::WEAK,
    ));

    strong_chunk.add_source_file(SourceFile::from_code_with_kind(
        "strong.js",
        "foo();",
        SourceKind::STRONG,
    ));

    test_parts(
        &mut t,
        &mut h,
        vec![
            srcs_chunks(vec![strong_chunk, weak_chunk]),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_reference_to_weak_namespace_root_from_strong_file_synthesizes_extern() {
    let (mut t, mut h, _f) = set_up();
    let weak_chunk = JSChunk::new(JSChunk::WEAK_CHUNK_NAME);
    let strong_chunk = JSChunk::new(JSChunk::STRONG_CHUNK_NAME);
    weak_chunk.add_dependency(&strong_chunk);

    weak_chunk.add_source_file(SourceFile::from_code_with_kind(
        "weak.js",
        [closure_externs().as_str(), "goog.provide('foo.bar');"].concat(),
        SourceKind::WEAK,
    ));

    strong_chunk.add_source_file(SourceFile::from_code_with_kind(
        "strong.js",
        "/** @suppress {undefinedVars} */\nfoo();\n",
        SourceKind::STRONG,
    ));

    test_extern_changes_parts(
        &mut t,
        &mut h,
        vec![
            externs(""),
            srcs_chunks(vec![strong_chunk, weak_chunk]),
            expected(&["var foo;", var_check_externs().as_str()].concat()),
        ],
    );
}

#[test]
fn test_reference_toweak_chunk_namespace_root_from_strong_file() {
    let (mut t, mut h, _f) = set_up();
    let weak_chunk = JSChunk::new(JSChunk::WEAK_CHUNK_NAME);
    let strong_chunk = JSChunk::new(JSChunk::STRONG_CHUNK_NAME);
    weak_chunk.add_dependency(&strong_chunk);

    weak_chunk.add_source_file(SourceFile::from_code_with_kind(
        "weak.js",
        [
            closure_externs().as_str(),
            "goog.module('foo.bar'); goog.module.declareLegacyNamespace();",
        ]
        .concat(),
        SourceKind::WEAK,
    ));

    strong_chunk.add_source_file(SourceFile::from_code_with_kind(
        "strong.js",
        "foo();",
        SourceKind::STRONG,
    ));

    test_parts(
        &mut t,
        &mut h,
        vec![
            srcs_chunks(vec![strong_chunk, weak_chunk]),
            error(&UNDEFINED_VAR_ERROR),
        ],
    );
}

#[test]
fn test_reference_to_strong_namespace_root_with_additional_weak_provide_from_strong_file() {
    let (mut t, mut h, _f) = set_up();
    let weak_chunk = JSChunk::new(JSChunk::WEAK_CHUNK_NAME);
    let strong_chunk = JSChunk::new(JSChunk::STRONG_CHUNK_NAME);
    weak_chunk.add_dependency(&strong_chunk);

    weak_chunk.add_source_file(SourceFile::from_code_with_kind(
        "weak.js",
        "goog.provide('foo.bar');",
        SourceKind::WEAK,
    ));

    strong_chunk.add_source_file(SourceFile::from_code_with_kind(
        "strong0.js",
        [closure_externs().as_str(), "goog.provide('foo.qux');"].concat(),
        SourceKind::STRONG,
    ));
    strong_chunk.add_source_file(SourceFile::from_code_with_kind(
        "strong1.js",
        "foo();",
        SourceKind::STRONG,
    ));

    test_same_parts(
        &mut t,
        &mut h,
        vec![srcs_chunks(vec![strong_chunk, weak_chunk])],
    );
}
