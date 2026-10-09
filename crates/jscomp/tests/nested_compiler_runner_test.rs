/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/NestedCompilerRunnerTest.java.

//! Port of `NestedCompilerRunnerTest.java` (all 10 tests).
use closure_jscomp::{
    Compiler,
    check_level::CheckLevel,
    compilation_level::CompilationLevel,
    compiler_options::{CompilerOptions, LanguageMode, Reach},
    default_pass_config::DefaultPassConfig,
    diagnostic_groups,
    nested_compiler_runner::{Mode, NestedCompilerRunner},
    node_util::NodeUtil,
    pass_config::{PassConfig, PassConfigDelegate},
    pass_factory::PassFactory,
    pass_list_builder::PassListBuilder,
    property_renaming_policy::PropertyRenamingPolicy,
    pure_function_identifier::UNUSED_ARTIFICIAL_PURE_ANNOTATION,
    source_file::SourceFile,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::{
    ir::IR,
    node::{NodeId, ObjectProp, Prop},
    static_source_file::{SourceKind, StaticSourceFile},
    testing::node_subject::assert_node,
};
use std::sync::{Arc, Mutex};

struct Fixture {
    original: Compiler,
    options: CompilerOptions,
}

// port: NestedCompilerRunnerTest#setup
fn setup() -> Fixture {
    Fixture {
        original: Compiler::new(),
        options: CompilerOptions::new(),
    }
}

impl Fixture {
    // Java shares one CompilerOptions object between the test, the original compiler and the
    // pass config; Rust passes copies, so `init` takes the options as they are at that point.
    fn init(&mut self) {
        self.original.init(&[], &[], self.options.clone());
    }

    fn runner(&self, mode: Mode, pass_config: Box<dyn PassConfig>) -> NestedCompilerRunner {
        NestedCompilerRunner::create(self.options.clone(), mode, pass_config)
    }

    fn default_pass_config(&self) -> Box<dyn PassConfig> {
        Box::new(DefaultPassConfig::new(self.options.clone()))
    }

    // port: NestedCompilerRunnerTest#createScript
    fn create_script(&mut self, code: &str) -> NodeId {
        let c = &mut self.original;
        let file = Arc::new(SourceFile::from_code_with_kind(
            "test.js",
            code,
            SourceKind::STRONG,
        ));

        let parsed = c.parse_test_code(code);
        let script = IR::script(c);
        script.set_static_source_file(c, Some(file as Arc<dyn StaticSourceFile>));
        script.set_is_in_closure_unaware_subtree(c, true);
        let features = NodeUtil::get_feature_set_of_script(c, parsed);
        script.put_prop(
            c,
            Prop::FEATURE_SET,
            Some(ObjectProp::Opaque(Arc::new(
                features.unwrap_or(FeatureSet::BARE_MINIMUM),
            ))),
        );

        let children = parsed.remove_children(c);
        script.add_children_to_front(c, children);
        script
    }

    // port: NestedCompilerRunnerTest#parse
    fn parse(&mut self, src: &str) -> NodeId {
        self.original.parse_test_code(src)
    }

    // port: NestedCompilerRunnerTest#verifyCompilationSucceeded
    fn verify_compilation_succeeded(&self) {
        assert!(
            self.original.get_errors().is_empty(),
            "{:?}",
            self.original.get_errors()
        );
        assert!(
            self.original.get_warnings().is_empty(),
            "{:?}",
            self.original.get_warnings()
        );
    }

    fn assert_script_equals(&mut self, script: NodeId, expected_src: &str) {
        let expected = self.parse(expected_src);
        assert_node(script).is_equal_to(&self.original, expected);
    }
}

// port: NestedCompilerRunnerTest#emptyInputListSucceeds
#[test]
fn empty_input_list_succeeds() {
    let mut f = setup();
    f.init();

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner.compile(&mut f.original);

    f.verify_compilation_succeeded();
}

// port: NestedCompilerRunnerTest#singleEmptyInputSucceeds
#[test]
fn single_empty_input_succeeds() {
    let mut f = setup();
    f.init();
    let script = f.create_script("");

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    f.verify_compilation_succeeded();
    f.assert_script_equals(script, "");
}

// port: NestedCompilerRunnerTest#canRunPeepholeOptimizations
#[test]
fn can_run_peephole_optimizations() {
    let mut f = setup();
    f.options.set_fold_constants(true);
    f.init();

    let script = f.create_script("console.log(1 + 2)");

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    f.verify_compilation_succeeded();
    f.assert_script_equals(script, "console.log(3)");
}

// port: NestedCompilerRunnerTest#delegatesWarningsAndErrorsToOriginalCompiler
#[test]
fn delegates_warnings_and_errors_to_original_compiler() {
    let mut f = setup();
    f.options.set_compute_function_side_effects(true);
    f.options
        .set_warning_level(diagnostic_groups::CHECK_VARIABLES.clone(), CheckLevel::OFF);
    f.options.set_warning_level(
        diagnostic_groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION.clone(),
        CheckLevel::ERROR,
    );
    f.init();

    let script = f.create_script(
        r#"class A {
  /** @nosideeffects */
  error() { throw new Error('A'); }
}
class B {
  error() { throw new Error('B'); }
}
console.log((Math.random() < 0.5 ? new A() : new B()).error());
"#,
    );

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    assert!(f.original.get_warnings().is_empty());
    let errors = f.original.get_errors();
    assert_eq!(
        errors.iter().map(|e| e.get_type().key).collect::<Vec<_>>(),
        vec![UNUSED_ARTIFICIAL_PURE_ANNOTATION.key]
    );
}

// port: NestedCompilerRunnerTest#canDeleteUnusedVariables
#[test]
fn can_delete_unused_variables() {
    let mut f = setup();
    CompilationLevel::SIMPLE_OPTIMIZATIONS.set_options_for_compilation_level(&mut f.options);
    // Java sets the two options below after original.init on the shared options object.
    f.options
        .set_warning_level(diagnostic_groups::CHECK_VARIABLES.clone(), CheckLevel::OFF);
    f.options.set_remove_unused_variables(Reach::ALL);
    f.init();

    let script = f.create_script(
        r#"const used = 0;
const unused = 1;
console.log(used);
"#,
    );

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    f.verify_compilation_succeeded();
    f.assert_script_equals(script, "const used = 0; console.log(used);");
}

// port: NestedCompilerRunnerTest#transpiles_exponentialOperatorToES2015
#[test]
fn transpiles_exponential_operator_to_es2015() {
    let mut f = setup();
    f.options.set_language_out(LanguageMode::ECMASCRIPT_2015);
    f.options.set_prevent_library_injection(true);
    f.init();

    // Use the exponential operator as a very trivial transpilation test case.
    let script = f.create_script("const x = 3 ** 4;");

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    f.verify_compilation_succeeded();
    f.assert_script_equals(script, "const x = Math.pow(3, 4);");
}

// port: NestedCompilerRunnerTest#transpiles_exponentialOperatorToES2016_inTranspileOnlyMode
#[test]
fn transpiles_exponential_operator_to_es2016_in_transpile_only_mode() {
    let mut f = setup();
    f.options.set_language_out(LanguageMode::ECMASCRIPT_2015);
    f.options.set_prevent_library_injection(true);
    f.init();

    // Use the exponential operator as a very trivial transpilation test case.
    let script = f.create_script("const x = 3 ** 4;");

    let mut runner = f.runner(Mode::TRANSPILE_ONLY, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    f.verify_compilation_succeeded();
    f.assert_script_equals(script, "const x = Math.pow(3, 4);");
}

// port: NestedCompilerRunnerTest#doesNotMangleOriginalExternProperties
#[test]
fn does_not_mangle_original_extern_properties() {
    let mut f = setup();
    f.options
        .set_property_renaming(PropertyRenamingPolicy::ALL_UNQUOTED);
    f.original
        .set_extern_properties(["external".to_owned()].into_iter().collect());
    f.init();

    let script = f.create_script("alert({external: 1, internal: 2});");

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    f.verify_compilation_succeeded();
    f.assert_script_equals(script, "alert({external: 1, a: 2})");
}

/// `new PassConfig.PassConfigDelegate(new DefaultPassConfig(options)) { getOptimizations() }`
struct AlwaysThrowsPassConfig {
    delegate: PassConfigDelegate,
}

impl PassConfig for AlwaysThrowsPassConfig {
    fn get_options(&self) -> &CompilerOptions {
        self.delegate.get_options()
    }
    fn as_pass_config(&self) -> &dyn PassConfig {
        self
    }
    fn get_delegate(&self) -> Option<&dyn PassConfig> {
        Some(&self.delegate)
    }
    fn get_whitespace_only_passes(&self) -> PassListBuilder {
        self.delegate.get_whitespace_only_passes()
    }
    fn get_checks(&self) -> PassListBuilder {
        self.delegate.get_checks()
    }
    fn get_optimizations(&self) -> PassListBuilder {
        let mut passes = PassListBuilder::new(self.get_options().clone());
        passes.maybe_add(
            PassFactory::builder()
                .set_name("throw".to_owned())
                .set_internal_factory(Arc::new(|_| panic!("foobarbaz")))
                .build(),
        );
        passes
    }
    fn get_finalizations(&self) -> PassListBuilder {
        self.delegate.get_finalizations()
    }
    fn get_transpile_only_passes(&self) -> PassListBuilder {
        self.delegate.get_transpile_only_passes()
    }
}

fn panic_message(err: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        err.downcast_ref::<&str>().unwrap().to_string()
    }
}

// port: NestedCompilerRunnerTest#stackTrace_specifiesInShadowAstCompilation
#[test]
fn stack_trace_specifies_in_shadow_ast_compilation() {
    let mut f = setup();
    let always_throws_pass_config = Box::new(AlwaysThrowsPassConfig {
        delegate: PassConfigDelegate::new(f.default_pass_config()),
    });
    f.init();
    let script = f.create_script("");
    let mut compiler_runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, always_throws_pass_config);
    compiler_runner.add_script(&mut f.original, script, "test");

    // Rust-only: a panic carries no cause; the panic hook records the inner panic ("the cause")
    // before CompilerExecutor wraps it.
    let messages = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorded = messages.clone();
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        recorded.lock().unwrap().push(panic_message(info.payload()));
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        compiler_runner.compile(&mut f.original)
    }));
    std::panic::set_hook(previous_hook);
    let err = result.unwrap_err();

    assert_eq!(
        panic_message(&*err),
        "Exception during compilation: <shadow AST compilation>"
    );
    assert!(
        messages.lock().unwrap().iter().any(|m| m == "foobarbaz"),
        "{:?}",
        messages.lock().unwrap()
    );
}

// TODO: lharker - add a test for debug logging.

// port: NestedCompilerRunnerTest#doesNotRunTypechecking
#[test]
fn does_not_run_typechecking() {
    let mut f = setup();
    f.options.set_check_types(true);
    f.init();

    let script = f.create_script("/** @type {string} */ const x = 0;");

    let mut runner = f.runner(Mode::TRANSPILE_AND_OPTIMIZE, f.default_pass_config());
    runner
        .add_script(&mut f.original, script, "test")
        .compile(&mut f.original);

    f.verify_compilation_succeeded();
    f.assert_script_equals(script, "const x = 0;");
}
