/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Shared harness glue for the var-checks CompilerTestCase ports (VarCheckTest,
//! VariableReferenceCheckTest, UnusedLocalsCheckTest, ConstCheckTest, StrictModeCheckTest,
//! DeclaredGlobalExternsOnWindowTest, RemoveUnnecessarySyntheticExternsTest, ValidityCheckTest):
//! the CompilerTestCase hooks (getProcessor / getOptions overrides) and the inherited
//! CompilerTestCase convenience methods each Java test calls unqualified.
#![allow(dead_code)]

use closure_jscomp::{
    compiler_options::CompilerOptions, compiler_pass::CompilerPass, diagnostic_type::DiagnosticType,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{
        CompilerTestCase, CompilerTestCaseHooks, Diagnostic, Expected, Externs, Sources, TestPart,
    },
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

type Processor = Box<dyn FnMut(CompilerHandle) -> Box<dyn CompilerPass>>;
type OptionsOverride = Box<dyn FnMut(&mut CompilerOptions)>;

/// The Java test class's overrides of CompilerTestCase#getProcessor and #getOptions.
pub struct Hooks {
    ctx: Ctx,
    processor: Processor,
    options: Option<OptionsOverride>,
}

impl Hooks {
    // port: ReplayDsl.Ctx#Ctx (native test context)
    pub fn new(
        class: &str,
        processor: impl FnMut(CompilerHandle) -> Box<dyn CompilerPass> + 'static,
    ) -> Self {
        Self {
            ctx: Ctx::new(
                class.into(),
                closure_testing::replay::replay_values::object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            processor: Box::new(processor),
            options: None,
        }
    }

    /// The test class's getOptions override: `super.getOptions()` then `apply`.
    pub fn with_options(mut self, apply: impl FnMut(&mut CompilerOptions) + 'static) -> Self {
        self.options = Some(Box::new(apply));
        self
    }
}

impl CompilerTestCaseHooks for Hooks {
    // port: CompilerTestCase#getProcessor (the test class's override)
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass = (self.processor)(compiler);
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }
    // port: CompilerTestCase#getOptions (the test class's override)
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        if let Some(apply) = self.options.as_mut() {
            apply(&mut options);
        }
        Ok(options)
    }
    // port: ReplayDsl.Ctx#Ctx
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: CompilerTestCase#MINIMAL_EXTERNS
pub fn minimal_externs() -> String {
    CompilerTestCase::minimal_externs().unwrap().to_string()
}

// port: CompilerTestCase#DEFAULT_EXTERNS
pub fn default_externs() -> String {
    CompilerTestCase::default_externs().unwrap().to_string()
}

// port: CompilerTestCase#VAR_CHECK_EXTERNS
pub fn var_check_externs() -> String {
    CompilerTestCase::var_check_externs().unwrap().to_string()
}

// port: CompilerTestCase#srcs(String)
pub fn srcs(code: &str) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs(code))
}

// port: CompilerTestCase#srcs(String...)
pub fn srcs_n(code: &[&str]) -> TestPart {
    TestPart::Sources(sources_n(code))
}

pub fn sources_n(code: &[&str]) -> Sources {
    let code: Vec<JsString> = code.iter().map(|c| JsString::from(*c)).collect();
    CompilerTestCase::srcs_strings(&code)
}

// port: CompilerTestCase#externs(String)
pub fn externs(code: &str) -> TestPart {
    TestPart::Externs(CompilerTestCase::externs(code))
}

pub fn externs_of(code: &str) -> Externs {
    CompilerTestCase::externs(code)
}

// port: CompilerTestCase#expected(String)
pub fn expected(code: &str) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected(code))
}

// port: CompilerTestCase#expected(String...)
pub fn expected_n(code: &[&str]) -> TestPart {
    TestPart::Expected(expected_of_n(code))
}

pub fn expected_of_n(code: &[&str]) -> Expected {
    let code: Vec<JsString> = code.iter().map(|c| JsString::from(*c)).collect();
    CompilerTestCase::expected_strings(&code)
}

// port: CompilerTestCase#warning(DiagnosticType)
pub fn warning(type_: &'static DiagnosticType) -> TestPart {
    TestPart::Diagnostic(CompilerTestCase::warning(type_))
}

// port: CompilerTestCase#error(DiagnosticType)
pub fn error(type_: &'static DiagnosticType) -> TestPart {
    TestPart::Diagnostic(CompilerTestCase::error(type_))
}

pub fn diagnostic(part: TestPart) -> Diagnostic {
    match part {
        TestPart::Diagnostic(d) => d,
        _ => panic!("not a diagnostic"),
    }
}

// port: CompilerTestCase#test(String,String)
pub fn test(t: &mut CompilerTestCase, h: &mut Hooks, js: &str, expected: &str) {
    t.test_strings(h, js, expected).unwrap();
}

// port: CompilerTestCase#test(TestPart...)
pub fn test_parts(t: &mut CompilerTestCase, h: &mut Hooks, parts: Vec<TestPart>) {
    t.test(h, parts).unwrap();
}

// port: CompilerTestCase#testSame(String)
pub fn test_same(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    t.test_same_string(h, js).unwrap();
}

// port: CompilerTestCase#testSame(TestPart...)
pub fn test_same_parts(t: &mut CompilerTestCase, h: &mut Hooks, parts: Vec<TestPart>) {
    t.test_same(h, parts).unwrap();
}

// port: CompilerTestCase#testError(String,DiagnosticType)
pub fn test_error(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    js: &str,
    error: &'static DiagnosticType,
) {
    t.test_error(h, js, error).unwrap();
}

// port: CompilerTestCase#testError(TestPart...) (Sources/Externs and the error)
pub fn test_error_parts(t: &mut CompilerTestCase, h: &mut Hooks, parts: Vec<TestPart>) {
    t.test(h, parts).unwrap();
}

// port: CompilerTestCase#testWarning(String,DiagnosticType)
pub fn test_warning(
    t: &mut CompilerTestCase,
    h: &mut Hooks,
    js: &str,
    warning: &'static DiagnosticType,
) {
    t.test_warning(h, js, warning).unwrap();
}

// port: CompilerTestCase#testWarning(TestPart...)
pub fn test_warning_parts(t: &mut CompilerTestCase, h: &mut Hooks, parts: Vec<TestPart>) {
    t.test(h, parts).unwrap();
}

// port: CompilerTestCase#testNoWarning(String)
pub fn test_no_warning(t: &mut CompilerTestCase, h: &mut Hooks, js: &str) {
    t.test_no_warning(h, js).unwrap();
}

// port: CompilerTestCase#testExternChanges(Externs,Sources,Expected,Diagnostic...)
pub fn test_extern_changes_parts(t: &mut CompilerTestCase, h: &mut Hooks, parts: Vec<TestPart>) {
    let (mut ext, mut src, mut exp, mut diags) = (None, None, None, vec![]);
    for part in parts {
        match part {
            TestPart::Externs(e) => ext = Some(e),
            TestPart::Sources(s) => src = Some(s),
            TestPart::Expected(e) => exp = Some(e),
            TestPart::Diagnostic(d) => diags.push(d),
            TestPart::Postcondition(_) => panic!("testExternChanges takes no postcondition"),
        }
    }
    match ext {
        Some(ext) => t.test_extern_changes(h, &ext, &src.unwrap(), &exp.unwrap(), &diags, None),
        None => t.test_extern_changes_default(h, &src.unwrap(), &exp.unwrap(), &diags),
    }
    .unwrap();
}

// port: CompilerTestCase#srcs(SourceFile...)
pub fn srcs_files(files: Vec<closure_testing::jscomp_api::SourceFile>) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_files(
        files.into_iter().map(std::sync::Arc::new).collect(),
    ))
}

// port: CompilerTestCase#error(DiagnosticType) + ErrorDiagnostic#withMessage
pub fn error_with_message(type_: &'static DiagnosticType, message: &str) -> TestPart {
    TestPart::Diagnostic(
        CompilerTestCase::error(type_)
            .with_message(message)
            .unwrap(),
    )
}
