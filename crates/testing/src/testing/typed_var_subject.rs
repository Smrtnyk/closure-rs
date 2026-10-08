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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/testing/TypedVarSubject.java.

//! Port of testing/TypedVarSubject.java: a Truth Subject for TypedVar.
use closure_jscomp::{abstract_compiler::AbstractCompiler, typed_var::TypedVar};
use closure_jstype::testing::type_subject::TypeSubject;

// port: TypedVarSubject
pub struct TypedVarSubject<'a> {
    compiler: &'a AbstractCompiler,
    actual: Option<TypedVar>,
}

// port: TypedVarSubject#assertThat
pub fn assert_that(compiler: &AbstractCompiler, var: Option<TypedVar>) -> TypedVarSubject<'_> {
    TypedVarSubject::assert_that(compiler, var)
}

impl<'a> TypedVarSubject<'a> {
    // port: TypedVarSubject#assertThat
    pub fn assert_that(compiler: &'a AbstractCompiler, var: Option<TypedVar>) -> Self {
        Self::new(compiler, var)
    }

    // port: TypedVarSubject#TypedVarSubject
    fn new(compiler: &'a AbstractCompiler, var: Option<TypedVar>) -> Self {
        Self {
            compiler,
            actual: var,
        }
    }

    // port: TypedVarSubject#actualNonNull
    fn actual_non_null(&self) -> TypedVar {
        // isNotNull()
        self.actual
            .unwrap_or_else(|| panic!("expected not to be: null"))
    }

    // port: TypedVarSubject#hasJSTypeThat
    pub fn has_js_type_that(&self) -> TypeSubject {
        TypeSubject::assert_type(self.actual_non_null().get_type(self.compiler))
    }

    // port: TypedVarSubject#isInferred
    pub fn is_inferred(&self) {
        let var = self.actual_non_null();
        assert!(
            var.is_type_inferred(self.compiler),
            "value of: isTypeInferred()\nexpected to be true\nfor var: {}",
            var.get_name(self.compiler)
        );
    }

    // port: TypedVarSubject#isNotInferred
    pub fn is_not_inferred(&self) {
        let var = self.actual_non_null();
        assert!(
            !var.is_type_inferred(self.compiler),
            "value of: isTypeInferred()\nexpected to be false\nfor var: {}",
            var.get_name(self.compiler)
        );
    }
}
