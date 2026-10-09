/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/J2clSuppressWarningsGuardTest.java.

//! Port of `J2clSuppressWarningsGuardTest.java`.
use closure_jscomp::{
    check_level::CheckLevel::OFF, check_side_effects, diagnostic_type::DiagnosticType,
    j2cl_suppress_warnings_guard::J2clSuppressWarningsGuard, js_error::JSError, type_check,
    warnings_guard::WarningsGuard,
};
use closure_rhino::{node::Ast, token::Token};

// port: J2clSuppressWarningsGuardTest#testSuppress_j2cl
#[test]
fn test_suppress_j2cl() {
    let guard: Box<dyn WarningsGuard> = Box::new(J2clSuppressWarningsGuard::new());

    assert_eq!(guard.level(&make_error("hello.java.js")), None);
    assert_eq!(
        guard.level(&make_j2cl_suppressed_error("hello.java.js")),
        Some(OFF)
    );
    assert_eq!(
        guard.level(&make_j2cl_suppressed_error("hello.impl.java.js")),
        Some(OFF)
    );
    assert_eq!(
        guard.level(&make_j2cl_suppressed_error("foo/hello.impl.java.js")),
        Some(OFF)
    );
}

// port: J2clSuppressWarningsGuardTest#testSuppress_nonJ2cl
#[test]
fn test_suppress_non_j2cl() {
    let guard: Box<dyn WarningsGuard> = Box::new(J2clSuppressWarningsGuard::new());

    assert_eq!(guard.level(&make_error("hello.js")), None);
    assert_eq!(guard.level(&make_j2cl_suppressed_error("hello.js")), None);
    assert_eq!(guard.level(&make_j2cl_suppressed_error("kajava.js")), None);
}

fn make_with_type(source_path: &str, type_: &'static DiagnosticType) -> JSError {
    let mut ast = Ast::new();
    let n = ast.new_node(Token::EMPTY);
    n.set_source_file_for_testing(&mut ast, source_path);
    JSError::make(&ast, n, type_, &[])
}

// port: J2clSuppressWarningsGuardTest#makeError
fn make_error(source_path: &str) -> JSError {
    make_with_type(source_path, &type_check::INEXISTENT_PROPERTY)
}

// port: J2clSuppressWarningsGuardTest#makeJ2clSuppressedError
fn make_j2cl_suppressed_error(source_path: &str) -> JSError {
    make_with_type(source_path, &check_side_effects::USELESS_CODE_ERROR)
}
