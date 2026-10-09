/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/PhaseOptimizer.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/CompilerTypeTestCase.java,
//   test/com/google/javascript/jscomp/TypeCheckTestCase.java,
//   test/com/google/javascript/jscomp/integration/IntegrationTestCase.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{
        CompilerTestCase, CompilerTestCaseHooks, Diagnostic, Externs, tree_equal_across,
    },
    harness_passes,
    integration::integration_test_case::IntegrationTestCase,
    jscomp_api::{Compiler, CompilerOptions, DiagnosticType, JSError, SourceFile},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

static WARNING: DiagnosticType = DiagnosticType::warning("JSC_HARNESS_WARNING", "warning {0}");
struct Hooks {
    ctx: Ctx,
    /// Whether the fixture's processor renames the extern `before` to `after`.
    rename_extern: bool,
}
impl Hooks {
    // port: CompilerTestCase#getProcessor (native fixture setup)
    fn new() -> Self {
        Self {
            ctx: Ctx::new(
                "Fixture".into(),
                closure_testing::replay::replay_values::object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            rename_extern: true,
        }
    }
}
impl CompilerTestCaseHooks for Hooks {
    // port: CompilerTestCase#getProcessor (native fixture processor)
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let rename_extern = self.rename_extern;
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, _root: NodeId| {
                if !rename_extern {
                    return;
                }
                let name = externs
                    .get_first_child(compiler)
                    .unwrap()
                    .get_first_child(compiler)
                    .unwrap()
                    .get_first_child(compiler)
                    .unwrap();
                assert_eq!(name.get_string(compiler), JsString::from("before"));
                name.set_string(compiler, "after");
                compiler.report_change_to_enclosing_scope(name);
            },
        )))))
    }
    // port: ReplayDsl.Ctx#Ctx (native fixture context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}
// port: CompilerTestCase#createAndInitializeCompiler (native fixture)
fn parsed(harness: &mut CompilerTestCase, hooks: &mut Hooks, code: &str) -> CompilerHandle {
    let c = harness
        .create_and_initialize_compiler(
            hooks,
            &Externs::new(vec![]),
            &CompilerTestCase::srcs_strings(&[code.into()]),
        )
        .unwrap();
    c.borrow_mut().parse_inputs().unwrap();
    c
}
// port: CompilerTestCase#getOptions / createCompiler / createAndInitializeCompiler
#[test]
fn initialization_uses_real_options_compiler_and_chunks() {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    let options = harness.get_options().unwrap();
    assert!(!options.should_emit_use_strict());
    assert!(options.should_preserve_type_annotations());
    assert!(!options.get_assume_getters_are_pure());
    assert!(
        options
            .get_coding_convention()
            .as_ref()
            .unwrap()
            .is_constant(&"CONST".into())
    );
    let mut hooks = Hooks::new();
    let c = parsed(&mut harness, &mut hooks, "var input = 1;");
    assert_eq!(
        c.borrow()
            .get_js_root()
            .unwrap()
            .get_child_count(&c.borrow()),
        1
    );
    assert_eq!(c.borrow().get_error_count(), 0);
    let js_root = c.borrow().get_js_root().unwrap();
    assert_eq!(
        harness_passes::to_source(&mut c.borrow_mut(), js_root).unwrap(),
        "var input=1"
    );
}
// port: CompilerTestCase#parseExpectedJs / compareExpectedToActualAsTree
#[test]
fn expected_js_keeps_its_compiler_arena_and_uses_real_serializer() {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    let mut hooks = Hooks::new();
    let c = parsed(&mut harness, &mut hooks, "var input = 1;");
    let actual = c.borrow().get_js_root().unwrap();
    let (expected, root) = harness
        .parse_expected_js_string(&mut hooks, &c, "var input = 1;")
        .unwrap();
    assert!(!Rc::ptr_eq(&c, &expected));
    tree_equal_across(
        &c.borrow(),
        actual,
        &expected.borrow(),
        root,
        true,
        &IndexMap::<_, _>::default(),
    )
    .unwrap();
    let (different, root) = harness
        .parse_expected_js_string(&mut hooks, &c, "var input = 2;")
        .unwrap();
    let failure = tree_equal_across(
        &c.borrow(),
        actual,
        &different.borrow(),
        root,
        true,
        &IndexMap::<_, _>::default(),
    )
    .unwrap_err();
    assert!(
        matches!(failure,Throwable::Assertion { message: ref s } if s.contains("input") && s.contains('2') && s.contains('1'))
    );
}
// port: CompilerTestCase#testExternChanges / compareExpectedToActualAsStrings
#[test]
fn extern_processor_executes_on_real_compiler_and_compares_both_ways() {
    for compare_as_tree in [true, false] {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.compare_as_tree = compare_as_tree;
        harness
            .test_extern_changes(
                &mut Hooks::new(),
                &Externs::new(vec![Arc::new(SourceFile::from_code(
                    "extern.js",
                    "var before;",
                ))]),
                &CompilerTestCase::srcs_strings(&["".into()]),
                &CompilerTestCase::expected("var after;"),
                &[],
                None,
            )
            .unwrap();
    }
}
// port: CompilerTestCase#validateWarnings / validateSourceLocation
#[test]
fn warning_validation_reads_live_compiler_diagnostics() {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    let mut hooks = Hooks::new();
    let c = parsed(&mut harness, &mut hooks, "var input;");
    let name = c
        .borrow()
        .get_js_root()
        .unwrap()
        .get_first_child(&c.borrow())
        .unwrap()
        .get_first_child(&c.borrow())
        .unwrap()
        .get_first_child(&c.borrow())
        .unwrap();
    let warning = JSError::make(&c.borrow(), name, &WARNING, &["one"]);
    c.borrow_mut().report(warning);
    harness.last_compiler = Some(c.clone());
    let warnings = c.borrow().get_warnings();
    let failure = harness
        .validate_warnings(&c.borrow(), &[], &[], &warnings, 1)
        .unwrap_err();
    assert_eq!(failure, Throwable::Assertion {
        message: "aggregate warnings: testcode0:1:4: WARNING - [JSC_HARNESS_WARNING] warning one\nvar input;\n    ^^^^^\n".into(),
    });
    let expected = Diagnostic::new(closure_testing::jscomp_api::CheckLevel::WARNING, &WARNING)
        .with_message("warning one")
        .unwrap();
    harness
        .validate_warnings(
            &c.borrow(),
            std::slice::from_ref(&expected),
            std::slice::from_ref(&warnings),
            &warnings,
            1,
        )
        .unwrap();
    let failure = harness
        .validate_warnings(
            &c.borrow(),
            &[expected],
            std::slice::from_ref(&warnings),
            &warnings,
            2,
        )
        .unwrap_err();
    assert_eq!(
        failure,
        Throwable::Assertion {
            message: format!(
                "There should be 1 warnings, repeated 2 time(s). Warnings: \n{}",
                warnings[0]
            ),
        }
    );
}
// port: IntegrationTestCase#parseExpectedCode / toSource
#[test]
fn integration_expected_parsing_uses_real_compiler() {
    let integration = IntegrationTestCase::set_up(vec![]);
    let actual = Rc::new(RefCell::new(Compiler::new()));
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    let (expected, root) = integration
        .parse_expected_code(&actual, &["var input = 1;".into()], &mut options)
        .unwrap();
    assert!(!Rc::ptr_eq(&actual, &expected));
    assert_eq!(
        harness_passes::to_source(&mut expected.borrow_mut(), root).unwrap(),
        "var input=1"
    );
}

// port: IntegrationTestCase#compile (--jscomp_dev_mode EVERY_PASS: the validity check runs after
// every pass)
#[test]
fn integration_compile_runs_the_validity_check_after_every_pass() {
    let mut integration = IntegrationTestCase::set_up(vec![]);
    let mut options = CompilerOptions::new();
    options.set_dev_mode(closure_testing::jscomp_api::DevMode::EVERY_PASS);
    let result = integration.compile_string(options, &"var input = 1;".into());
    // ValidityCheck (var-checks) is real: every pass of the default options' pipeline runs and
    // passes the check.
    assert!(result.is_ok(), "{:?}", result.as_ref().err());
    assert_eq!(result.unwrap().borrow().get_error_count(), 0);
    let compiler = integration.last_compiler.unwrap();
    assert_eq!(
        compiler
            .borrow()
            .get_js_root()
            .unwrap()
            .get_child_count(&compiler.borrow()),
        1
    );
}

// port: CompilerTypeTestCase#defaultOptions / TypeCheckTestCase.TypeTestBuilder#newTest
#[test]
fn type_builder_initializes_real_options_and_parses_before_module_setup() {
    use closure_testing::{
        compiler_type_test_case::CompilerTypeTestCase, type_check_test_case::TypeTestBuilder,
    };
    let options = CompilerTypeTestCase::default_options().unwrap();
    assert_eq!(
        options.get_language_in(),
        closure_testing::jscomp_api::LanguageMode::UNSUPPORTED
    );
    assert!(options.get_coding_convention().is_some());
    let mut builder = TypeTestBuilder::new_test("var external;".into()).unwrap();
    builder
        .include_default_externs()
        .add_source("var input = 1;");
    // GatherModuleMetadata, ModuleMapCreator (modules-core), InferConsts (harness-prereqs), the
    // type registry's Compiler integration (ast-support), SemanticReverseAbstractInterpreter and
    // TypeCheck (type-check) are real: the source type-checks without diagnostics.
    let first = builder.run();
    assert!(first.is_ok(), "{first:?}");
    assert!(builder.has_run);
    assert_eq!(builder.compiler.borrow().get_error_count(), 0);
    assert_eq!(
        builder
            .compiler
            .borrow()
            .get_js_root()
            .unwrap()
            .get_child_count(&builder.compiler.borrow()),
        1
    );
    assert!(
        matches!(builder.run(), Err(Throwable::Exception { message: Some(message), .. }) if message == "Cannot run the same test twice")
    );
}

// port: CompilerTestCase#testInternal (native setup passes, processor and comparison)
#[test]
fn test_internal_runs_setup_passes_and_processor() {
    // AstValidator, SourceInfoCheck (harness-prereqs), GatherGetterAndSetterProperties
    // (ast-support), GatherModuleMetadata and ModuleMapCreator (modules-core) are real: the
    // fixture's processor runs on the parsed externs and the result matches the expected code;
    // an extern change the test does not expect fails the comparison.
    for rename_extern in [false, true] {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        let mut hooks = Hooks::new();
        hooks.rename_extern = rename_extern;
        let result = harness.test_internal(
            &mut hooks,
            &Externs::new(vec![Arc::new(SourceFile::from_code(
                "extern.js",
                "var before;",
            ))]),
            &CompilerTestCase::srcs_strings(&["var input = 1;".into()]),
            Some(&CompilerTestCase::expected("var input = 1;")),
            &[],
            &[],
            None,
        );
        if rename_extern {
            assert!(
                matches!(&result, Err(Throwable::Assertion { message }) if message.contains("var after;")),
                "{result:?}"
            );
            continue;
        }
        assert!(result.is_ok(), "{result:?}");
        let c = harness.last_compiler.unwrap();
        assert_eq!(c.borrow().get_error_count(), 0);
        assert_eq!(
            harness_passes::to_source_all(&mut c.borrow_mut()).unwrap(),
            "var input=1;"
        );
    }
}

// port: CompilerTestCase#testInternal (validator, accessor collection and processor)
#[test]
fn test_internal_runs_validator_accessor_collection_and_processor() {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.allow_externs_changes().unwrap();
    let result = harness.test_internal(
        &mut Hooks::new(),
        &Externs::new(vec![Arc::new(SourceFile::from_code(
            "extern.js",
            "var before;",
        ))]),
        &CompilerTestCase::srcs_strings(&["var input = 1;".into()]),
        Some(&CompilerTestCase::expected("var input = 1;")),
        &[],
        &[],
        None,
    );
    // AstValidator (harness-prereqs) and the accessor collection (GatherGetterAndSetterProperties,
    // ast-support) are real: testInternal runs the fixture processor, which renames the extern.
    assert!(result.is_ok(), "{result:?}");
    let c = harness.last_compiler.unwrap();
    assert_eq!(c.borrow().get_error_count(), 0);
    assert_eq!(
        harness_passes::to_source_all(&mut c.borrow_mut()).unwrap(),
        "var input=1;"
    );
    let compiler = c.borrow();
    let extern_name = compiler
        .get_externs_root()
        .unwrap()
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert_eq!(extern_name.get_string(&compiler), JsString::from("after"));
}

// port: ReplayDsl#invoke (native Node and IR registrations)
#[test]
fn native_node_signatures_execute_in_the_current_arena() {
    use closure_testing::{dsl::Expr, replay::replay_dsl::eval};
    let tsv = "descriptor\tlookup\tdeclaringClass\tsignature\twidened\nFixture\tcom.google.javascript.rhino.IR.name\tcom.google.javascript.rhino.IR\tname(java.lang.String)\nFixture\tcom.google.javascript.rhino.IR.var\tcom.google.javascript.rhino.IR\tvar(com.google.javascript.rhino.Node)\nFixture\tcom.google.javascript.rhino.Node.cloneTree\tcom.google.javascript.rhino.Node\tcloneTree()\n";
    let c = Rc::new(RefCell::new(Compiler::new()));
    let mut ctx = Ctx::new(
        "Fixture".into(),
        closure_testing::replay::replay_values::object([]),
        IndexMap::<_, _>::default(),
        Registry::from_tsv(tsv).unwrap(),
    );
    ctx.compiler = Some(c.clone());
    let expr = Expr::Static {
        class: "com.google.javascript.rhino.IR".into(),
        method: "var".into(),
        args: Some(vec![Expr::Static {
            class: "com.google.javascript.rhino.IR".into(),
            method: "name".into(),
            args: Some(vec![Expr::String("input".into())]),
        }]),
    };
    let DslValue::Node(root) = eval(&expr, &mut ctx).unwrap() else {
        panic!("native IR node");
    };
    ctx.vars.insert("node".into(), DslValue::Node(root));
    let clone = Expr::Call {
        target: Box::new(Expr::Var("node".into())),
        method: "cloneTree".into(),
        args: None,
    };
    let DslValue::Node(other) = eval(&clone, &mut ctx).unwrap() else {
        panic!("native Node clone");
    };
    assert_ne!(root, other);
    assert!(root.is_equivalent_to(&c.borrow(), other));
    assert_eq!(
        root.get_first_child(&c.borrow())
            .unwrap()
            .get_string(&c.borrow()),
        JsString::from("input")
    );
}

// port: CompilerTestCase#makePassFactory / PhaseOptimizer#process (native factory callbacks)
#[test]
fn phase_factory_borrows_the_executing_compiler_and_preserves_replay_state() {
    use closure_testing::{
        dsl::Expr,
        replay::replay_dsl::{Lambda, eval, process},
    };
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    let mut hooks = Hooks::new();
    let c = parsed(&mut harness, &mut hooks, "var input;");
    let tsv = "descriptor\tlookup\tdeclaringClass\tsignature\twidened\nFixture\tcom.google.javascript.jscomp.PhaseOptimizer\tcom.google.javascript.jscomp.PhaseOptimizer\t<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.PerformanceTracker)\nFixture\tcom.google.javascript.jscomp.Compiler.getRoot\tcom.google.javascript.jscomp.Compiler\tgetRoot()\n";
    let mut ctx = Ctx::new(
        "Fixture".into(),
        closure_testing::replay::replay_values::object([]),
        IndexMap::<_, _>::default(),
        Registry::from_tsv(tsv).unwrap(),
    );
    ctx.compiler = Some(c.clone());
    let calls = Rc::new(std::cell::Cell::new(0));
    let called = calls.clone();
    let pass = DslValue::Pass(Rc::new(RefCell::new(Box::new(
        move |compiler: &mut Compiler, _externs: NodeId, root: NodeId| {
            assert!(root.is_root(compiler));
            called.set(called.get() + 1);
        },
    ))));
    let factory = CompilerTestCase::make_pass_factory(
        "nativeFixture".into(),
        Rc::new(Lambda {
            params: vec!["comp".into()],
            iface: "java.util.function.Function".into(),
            body: Expr::Do {
                effects: vec![
                    Expr::Call {
                        target: Box::new(Expr::Var("comp".into())),
                        method: "getRoot".into(),
                        args: None,
                    },
                    Expr::GetField {
                        target: Box::new(Expr::Options),
                        name: "checkTypes".into(),
                    },
                    Expr::Once {
                        key: "factoryWasEvaluated".into(),
                        value: Box::new(Expr::Bool(true)),
                    },
                ],
                value: Box::new(Expr::Var("pass".into())),
            },
            captured: IndexMap::<_, _>::from_iter([("pass".into(), pass)]),
        }),
    );
    let optimizer = eval(
        &Expr::New {
            class: "com.google.javascript.jscomp.PhaseOptimizer".into(),
            args: Some(vec![Expr::Compiler, Expr::Null]),
        },
        &mut ctx,
    )
    .unwrap();
    let DslValue::Native(native) = &optimizer else {
        panic!("PhaseOptimizer native adapter");
    };
    native
        .borrow_mut()
        .call("addOneTimePass", vec![factory])
        .unwrap();
    let externs = c.borrow().get_externs_root().unwrap();
    let root = c.borrow().get_js_root().unwrap();
    process(&optimizer, c, externs, root, &mut ctx).unwrap();
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        ctx.once["factoryWasEvaluated"],
        DslValue::Bool(true)
    ));
}

// port: TypeCheckTestCase#typeCheck / makeTypeCheck / castAny
// port: TypeCheckTestCase.TypeCheckResult#TypeCheckResult
#[test]
fn direct_type_check_wraps_nodes_and_runs_the_checker() {
    use closure_rhino::{ir::IR, token::Token};
    use closure_testing::{
        compiler_type_test_case::CompilerTypeTestCase,
        type_check_test_case::{TypeCheckResult, TypeCheckTestCase, cast_any},
    };
    for token in [Token::ROOT, Token::SCRIPT, Token::EMPTY] {
        let c = CompilerTypeTestCase::initialize_new_compiler(
            CompilerTypeTestCase::default_options().unwrap(),
        )
        .unwrap();
        let n = match token {
            Token::ROOT => IR::root(&mut c.borrow_mut(), &[]),
            Token::SCRIPT => {
                // TypedScopeCreator requires every SCRIPT to carry an input id (as parsed scripts do).
                let script = IR::script(&mut c.borrow_mut());
                script.set_input_id(
                    &mut c.borrow_mut(),
                    Some(Arc::new(closure_rhino::input_id::InputId::new("script"))),
                );
                script
            }
            _ => c.borrow_mut().new_node(token),
        };
        let harness = TypeCheckTestCase {
            base: CompilerTypeTestCase {
                compiler: Some(c.clone()),
                registry: Some(
                    closure_testing::harness_passes::compiler_type_registry(&c).unwrap(),
                ),
                ..Default::default()
            },
        };
        let checked = harness.type_check(n);
        assert!(matches!(checked, Ok(checked) if checked == n));
        let c = c.borrow();
        let js_root = match token {
            Token::ROOT => n,
            Token::SCRIPT => n.get_parent(&c).unwrap(),
            _ => {
                let script = n.get_parent(&c).unwrap();
                assert!(script.is_script(&c));
                assert_eq!(script.get_input_id(&c).unwrap().get_id_name(), "test");
                script.get_parent(&c).unwrap()
            }
        };
        assert!(js_root.is_root(&c));
        let root = js_root.get_parent(&c).unwrap();
        assert_eq!(root.get_child_count(&c), 2);
        assert!(root.get_first_child(&c).unwrap().is_root(&c));
        assert_eq!(root.get_second_child(&c), Some(js_root));
        let result = TypeCheckResult::new(Some(n), DslValue::Null);
        assert_eq!(cast_any(result).root, Some(n));
    }
}
