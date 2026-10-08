/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/ProcessTweaksTest.java.

//! Port of `ProcessTweaksTest.java`: tests for `ProcessTweaks`.
use closure_jscomp::{AbstractCompiler, strip_code::StripCode};
use closure_jscomp::{
    compiler_pass::CompilerPass, diagnostic_type::DiagnosticType, process_tweaks,
    process_tweaks::ProcessTweaks,
};
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use indexmap::{IndexMap, IndexSet};
use std::{cell::RefCell, rc::Rc};

struct ProcessTweaksTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    strip_tweaks: bool,
}

/// The anonymous pass of `ProcessTweaksTest#getProcessor`.
struct ProcessTweaksTestPass {
    strip_tweaks: bool,
}

impl CompilerPass for ProcessTweaksTestPass {
    // port: ProcessTweaksTest#getProcessor.process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut process_tweak = ProcessTweaks::new(self.strip_tweaks);
        process_tweak.process(compiler, externs, root);

        if self.strip_tweaks {
            let empty_set: IndexSet<String> = IndexSet::new();
            let mut strip_code = StripCode::new(
                &empty_set, &empty_set, &empty_set, /* enable_tweak_stripping= */ true,
            );
            strip_code.process(compiler, externs, root);
        }
    }
}

impl CompilerTestCaseHooks for Hooks {
    // port: ProcessTweaksTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass: Box<dyn CompilerPass> = Box::new(ProcessTweaksTestPass {
            strip_tweaks: self.strip_tweaks,
        });
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "ProcessTweaksTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl ProcessTweaksTest {
    // port: ProcessTweaksTest#ProcessTweaksTest
    // port: ProcessTweaksTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("function alert(arg) {}");
        harness.set_up();
        harness.enable_normalize().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "ProcessTweaksTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::new(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                strip_tweaks: false,
            },
        }
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_same(&mut self, js: &str) {
        self.harness
            .test_same_string(&mut self.hooks, js)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_same_warning(&mut self, js: &str, warning: &'static DiagnosticType) {
        self.harness
            .test_same_warning(&mut self.hooks, js, warning)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_error(&mut self, js: &str, error: &'static DiagnosticType) {
        self.harness
            .test_error(&mut self.hooks, js, error)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

// port: ProcessTweaksTest#testBasicTweak1
#[test]
fn test_basic_tweak1() {
    let mut t = ProcessTweaksTest::new();
    t.test_same(
        "goog.tweak.registerBoolean('Foo', 'Description');\ngoog.tweak.getBoolean('Foo')\n",
    );
}

// port: ProcessTweaksTest#testBasicTweak2
#[test]
fn test_basic_tweak2() {
    let mut t = ProcessTweaksTest::new();
    t.test_same("goog.tweak.registerString('Foo', 'Description');\ngoog.tweak.getString('Foo')\n");
}

// port: ProcessTweaksTest#testBasicTweak3
#[test]
fn test_basic_tweak3() {
    let mut t = ProcessTweaksTest::new();
    t.test_same("goog.tweak.registerNumber('Foo', 'Description');\ngoog.tweak.getNumber('Foo')\n");
}

// port: ProcessTweaksTest#testBasicTweak4
#[test]
fn test_basic_tweak4() {
    let mut t = ProcessTweaksTest::new();
    t.test_same("goog.tweak.registerButton('Foo', 'Description', function() {})");
}

// port: ProcessTweaksTest#testBasicTweak5
#[test]
fn test_basic_tweak5() {
    let mut t = ProcessTweaksTest::new();
    t.test_same(
        "goog.tweak.registerBoolean('A.b_7', 'Description', true,\n{ requiresRestart:false })\n",
    );
}

// port: ProcessTweaksTest#testBasicTweak6
#[test]
fn test_basic_tweak6() {
    let mut t = ProcessTweaksTest::new();
    t.test_same(
        "var opts = { requiresRestart:false };\ngoog.tweak.registerBoolean('Foo', 'Description', true, opts)\n");
}

// port: ProcessTweaksTest#testNonLiteralId1
#[test]
fn test_non_literal_id1() {
    let mut t = ProcessTweaksTest::new();
    t.test_error(
        "goog.tweak.registerBoolean(3, 'Description')",
        &process_tweaks::NON_LITERAL_TWEAK_ID_ERROR,
    );
}

// port: ProcessTweaksTest#testNonLiteralId2
#[test]
fn test_non_literal_id2() {
    let mut t = ProcessTweaksTest::new();
    t.test_error(
        "goog.tweak.getBoolean('a' + 'b')",
        &process_tweaks::NON_LITERAL_TWEAK_ID_ERROR,
    );
}

// port: ProcessTweaksTest#testInvalidId
#[test]
fn test_invalid_id() {
    let mut t = ProcessTweaksTest::new();
    t.test_error(
        "goog.tweak.registerBoolean('Some ID', 'a')",
        &process_tweaks::INVALID_TWEAK_ID_ERROR,
    );
}

// port: ProcessTweaksTest#testInvalidDefaultValue1
#[test]
fn test_invalid_default_value1() {
    let mut t = ProcessTweaksTest::new();
    t.test_same_warning(
        "var val = true; goog.tweak.registerBoolean('Foo', 'desc', val)",
        &process_tweaks::INVALID_TWEAK_DEFAULT_VALUE_WARNING,
    );
}

// port: ProcessTweaksTest#testDuplicateTweak
#[test]
fn test_duplicate_tweak() {
    let mut t = ProcessTweaksTest::new();
    t.test_error(
        "goog.tweak.registerBoolean('TweakA', 'desc');\ngoog.tweak.registerBoolean('TweakA', 'desc')\n",
        &process_tweaks::TWEAK_MULTIPLY_REGISTERED_ERROR);
}

// port: ProcessTweaksTest#testRegisterInNonGlobalScope
#[test]
fn test_register_in_non_global_scope() {
    let mut t = ProcessTweaksTest::new();
    t.test_error(
        "function foo() {goog.tweak.registerBoolean('TweakA', 'desc');};",
        &process_tweaks::NON_GLOBAL_TWEAK_INIT_ERROR,
    );
}

// port: ProcessTweaksTest#testRegisterInIf
#[test]
fn test_register_in_if() {
    let mut t = ProcessTweaksTest::new();
    t.test_same("if (true) {goog.tweak.registerBoolean('TweakA', 'desc');};");
}

// port: ProcessTweaksTest#testWrongGetter1
#[test]
fn test_wrong_getter1() {
    let mut t = ProcessTweaksTest::new();
    t.test_same_warning(
        "goog.tweak.registerBoolean('TweakA', 'desc');\ngoog.tweak.getString('TweakA')\n",
        &process_tweaks::TWEAK_WRONG_GETTER_TYPE_WARNING,
    );
}

// port: ProcessTweaksTest#testWrongGetter2
#[test]
fn test_wrong_getter2() {
    let mut t = ProcessTweaksTest::new();
    t.test_same_warning(
        "goog.tweak.registerString('TweakA', 'desc');\ngoog.tweak.getNumber('TweakA')\n",
        &process_tweaks::TWEAK_WRONG_GETTER_TYPE_WARNING,
    );
}

// port: ProcessTweaksTest#testWrongGetter3
#[test]
fn test_wrong_getter3() {
    let mut t = ProcessTweaksTest::new();
    t.test_same_warning(
        "goog.tweak.registerNumber('TweakA', 'desc');\ngoog.tweak.getBoolean('TweakA')\n",
        &process_tweaks::TWEAK_WRONG_GETTER_TYPE_WARNING,
    );
}

// port: ProcessTweaksTest#testWithNoTweaks
#[test]
fn test_with_no_tweaks() {
    let mut t = ProcessTweaksTest::new();
    t.test_same("var DEF=true;var x={};x.foo={}");
}

// port: ProcessTweaksTest#testStrippingWithImplicitDefaultValues
#[test]
fn test_stripping_with_implicit_default_values() {
    let mut t = ProcessTweaksTest::new();
    t.hooks.strip_tweaks = true;
    t.test(
        "goog.tweak.registerNumber('TweakA', 'desc');\ngoog.tweak.registerBoolean('TweakB', 'desc');\ngoog.tweak.registerString('TweakC', 'desc');\nalert(goog.tweak.getNumber('TweakA'));\nalert(goog.tweak.getBoolean('TweakB'));\nalert(goog.tweak.getString('TweakC'));\n",
        "void 0; void 0; void 0; alert(0); alert(false); alert('')",
    );
}

// port: ProcessTweaksTest#testStrippingWithExplicitDefaultValues
#[test]
fn test_stripping_with_explicit_default_values() {
    let mut t = ProcessTweaksTest::new();
    t.hooks.strip_tweaks = true;
    t.test(
        "goog.tweak.registerNumber('TweakA', 'desc', 5);\ngoog.tweak.registerBoolean('TweakB', 'desc', true);\ngoog.tweak.registerString('TweakC', 'desc', '!');\nalert(goog.tweak.getNumber('TweakA'));\nalert(goog.tweak.getBoolean('TweakB'));\nalert(goog.tweak.getString('TweakC'));\n",
        "void 0; void 0; void 0; alert(5); alert(true); alert('!')",
    );
}

// port: ProcessTweaksTest#testStrippingWithInCodeOverrides
#[test]
fn test_stripping_with_in_code_overrides() {
    let mut t = ProcessTweaksTest::new();
    t.hooks.strip_tweaks = true;
    t.test(
        "goog.tweak.registerNumber('TweakA', 'desc');\ngoog.tweak.registerBoolean('TweakB', 'desc');\ngoog.tweak.registerString('TweakC', 'desc', 'foo');\nalert(goog.tweak.getNumber('TweakA'));\nalert(goog.tweak.getBoolean('TweakB'));\nalert(goog.tweak.getString('TweakC'));\n",
        "void 0; void 0; void 0;\nalert(0); alert(false); alert('foo');\n",
    );
}

// port: ProcessTweaksTest#testStrippingOfManuallyRegistered1
#[test]
fn test_stripping_of_manually_registered1() {
    let mut t = ProcessTweaksTest::new();
    t.hooks.strip_tweaks = true;
    t.test(
        "var reg = goog.tweak.getRegistry();\nif (reg) {\n  reg.register(new goog.tweak.BooleanSetting('foo', 'desc'));\n  reg.getEntry('foo').setDefaultValue(1);\n}\n",
        "if (null);",
    );
}
