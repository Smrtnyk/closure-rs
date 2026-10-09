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
//   test/com/google/javascript/jscomp/js/RuntimeJsLibManagerTest.java.

//! Port of `js/RuntimeJsLibManagerTest.java`.
//!
//! Rust note: Java parses every stub library and every expected tree with its own `new Compiler()`
//! and gives the manager a fresh `ChangeTracker`. Rust nodes live in one arena, so each test uses a
//! single `Compiler` for its script, the stub libraries and the expected trees, and the manager
//! uses that compiler's `ChangeTracker`.

use closure_jscomp::{
    Compiler,
    js::runtime_js_lib_manager::{ResourceProvider, RuntimeJsLibManager, RuntimeLibraryMode},
    source_file::SourceFile,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    ir::IR,
    node::NodeId,
    static_source_file::{SourceKind, StaticSourceFile},
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

// port: RuntimeJsLibManagerTest.StubResourceLoader
fn stub_resource_loader(resources: &[(&str, &str)]) -> ResourceProvider {
    let resources: IndexMap<String, String> = resources
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    // port: RuntimeJsLibManagerTest.StubResourceLoader#parse
    Box::new(
        move |compiler: &mut Compiler, resource_name: &str, _unused: &str| {
            if !resources.contains_key(resource_name) {
                return None;
            }
            Some(compiler.parse_synthetic_code("lib.js", resources[resource_name].as_str()))
        },
    )
}

fn create(
    mode: RuntimeLibraryMode,
    resources: &[(&str, &str)],
    script: Option<NodeId>,
) -> RuntimeJsLibManager {
    RuntimeJsLibManager::create(
        mode,
        stub_resource_loader(resources),
        Compiler::get_change_tracker_and_ast,
        match script {
            Some(script) => Box::new(move |_: &mut Compiler| script),
            None => Box::new(|compiler: &mut Compiler| IR::script(compiler)),
        },
    )
}

// port: RuntimeJsLibManagerTest#js
fn js(compiler: &mut Compiler, code: &str) -> NodeId {
    compiler.parse_synthetic_code("lib.js", code)
}

fn assert_node_equal(compiler: &Compiler, actual: NodeId, expected: NodeId) {
    assert!(
        actual.is_equivalent_to(compiler, expected),
        "expected:\n{}\nactual:\n{}",
        expected.to_string_tree(compiler),
        actual.to_string_tree(compiler)
    );
}

/// Truth `containsExactly` (order-insensitive, without `.inOrder()`).
fn assert_contains_exactly(actual: Vec<String>, expected: &[&str]) {
    let mut sorted_actual = actual.clone();
    sorted_actual.sort();
    let mut sorted_expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    sorted_expected.sort();
    assert_eq!(sorted_actual, sorted_expected, "{actual:?}");
}

fn panic_message(f: impl FnOnce()) -> String {
    let err = catch_unwind(AssertUnwindSafe(f)).expect_err("expected an exception");
    if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    }
}

// port: RuntimeJsLibManagerTest#JSCOMP_INHERITS
fn jscomp_inherits(compiler: &mut Compiler) -> NodeId {
    let name = IR::name(compiler, "$jscomp");
    IR::getprop(compiler, name, "inherits")
}

// port: RuntimeJsLibManagerTest#noOpMode_doesNotRecordOrInject
#[test]
fn no_op_mode_does_not_record_or_inject() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    let mut manager = create(RuntimeLibraryMode::NO_OP, &[("lib", "0")], Some(script));

    manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);

    assert!(manager.get_injected_libraries().is_empty());
}

// port: RuntimeJsLibManagerTest#noOpMode_doesRecordAndInject_withForceInjection
#[test]
fn no_op_mode_does_record_and_inject_with_force_injection() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    let mut manager = create(RuntimeLibraryMode::NO_OP, &[("lib", "0")], Some(script));

    manager.ensure_library_injected(&mut compiler, "lib", /* force= */ true);

    assert_eq!(manager.get_injected_libraries(), vec!["lib"]);
}

// port: RuntimeJsLibManagerTest#recordOnlyMode_doesNotInject_butRecordsLibraries
#[test]
fn record_only_mode_does_not_inject_but_records_libraries() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[("lib", "0")], None);

    let injected = manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);

    assert!(injected.is_none());
    assert_eq!(manager.get_injected_libraries(), vec!["lib"]);
}

// port: RuntimeJsLibManagerTest#recordOnlyMode_doesRecordAndInject_withForceInjection
#[test]
fn record_only_mode_does_record_and_inject_with_force_injection() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[("lib", "0")], None);

    manager.ensure_library_injected(&mut compiler, "lib", /* force= */ true);

    assert_eq!(manager.get_injected_libraries(), vec!["lib"]);
}

// port: RuntimeJsLibManagerTest#injectMode_doesRecordAndInject_andReturnsLastStatementInLibrary
#[test]
fn inject_mode_does_record_and_inject_and_returns_last_statement_in_library() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    let mut manager = create(
        RuntimeLibraryMode::INJECT,
        &[("lib", "function foo() {}\n0;\n")],
        Some(script),
    );

    let injected = manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);

    assert_eq!(manager.get_injected_libraries(), vec!["lib"]);
    let expected = js(&mut compiler, "function foo() {} 0;");
    assert_node_equal(&compiler, script, expected);
    assert_eq!(script.get_second_child(&compiler), injected);
}

// port: RuntimeJsLibManagerTest#doesNotInjectSameLibraryTwice
#[test]
fn does_not_inject_same_library_twice() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    let mut manager = create(RuntimeLibraryMode::INJECT, &[("lib", "0")], Some(script));

    let injected1 = manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);
    let injected2 = manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);

    assert_eq!(injected1, injected2);
    assert!(script.has_one_child(&compiler));
}

// port: RuntimeJsLibManagerTest#injectsBeforeExistingCode
#[test]
fn injects_before_existing_code() {
    let mut compiler = Compiler::new();
    let existing = IR::block(&mut compiler);
    let script = IR::script_with_children(&mut compiler, &[existing]);

    let mut manager = create(RuntimeLibraryMode::INJECT, &[("lib", "0")], Some(script));

    let injected = manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);

    assert_node_equal(
        &compiler,
        script.get_first_child(&compiler).unwrap(),
        injected.unwrap(),
    );
    assert_eq!(script.get_second_child(&compiler), Some(existing));
}

// port: RuntimeJsLibManagerTest#injectsAfterPreviouslyInjectedLib
#[test]
fn injects_after_previously_injected_lib() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    let mut manager = create(
        RuntimeLibraryMode::INJECT,
        &[("lib0", "0"), ("lib1", "1")],
        Some(script),
    );

    let injected0 = manager.ensure_library_injected(&mut compiler, "lib0", /* force= */ false);
    let injected1 = manager.ensure_library_injected(&mut compiler, "lib1", /* force= */ false);

    assert_eq!(script.get_first_child(&compiler), injected0);
    assert_eq!(script.get_second_child(&compiler), injected1);
    let expected = js(&mut compiler, "0; 1;");
    assert_node_equal(&compiler, script, expected);
}

// port: RuntimeJsLibManagerTest#injectsRequiredLibsOnlyOnce
#[test]
fn injects_required_libs_only_once() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    // Create a dependency tree:
    //       (base)
    //      /      \
    //  (childA)   (childB)
    let resources_map = [
        ("base.js", "print('in BASE');"),
        ("childA.js", "'require base.js';\nprint('in CHILD_A');\n"),
        ("childB.js", "'require base.js';\nprint('in CHILD_B');\n"),
    ];
    let mut manager = create(RuntimeLibraryMode::INJECT, &resources_map, Some(script));

    let child_a =
        manager.ensure_library_injected(&mut compiler, "childA.js", /* force= */ false);
    let child_b =
        manager.ensure_library_injected(&mut compiler, "childB.js", /* force= */ false);

    assert_contains_exactly(
        manager.get_injected_libraries(),
        &["base.js", "childA.js", "childB.js"],
    );
    assert_eq!(script.get_second_child(&compiler), child_a);
    assert_eq!(script.get_last_child(&compiler), child_b);
    let expected = js(
        &mut compiler,
        "print('in BASE');\nprint('in CHILD_A');\nprint('in CHILD_B');\n",
    );
    assert_node_equal(&compiler, script, expected);
}

// port: RuntimeJsLibManagerTest#injectMode_doesNotPassAlongUseStrictDirective
#[test]
fn inject_mode_does_not_pass_along_use_strict_directive() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    let mut manager = create(
        RuntimeLibraryMode::INJECT,
        &[("lib", "'use strict';\n0;\n")],
        Some(script),
    );

    let injected = manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);

    let expected = js(&mut compiler, "0;").get_only_child(&compiler);
    assert_node_equal(&compiler, injected.unwrap(), expected);
    assert!(script.has_one_child(&compiler));
    assert_eq!(manager.get_injected_libraries(), vec!["lib"]);
}

// port: RuntimeJsLibManagerTest#injectingMissingLib_throwsException
#[test]
fn injecting_missing_lib_throws_exception() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::INJECT, &[], None);

    let message = panic_message(|| {
        manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);
    });
    assert!(message.contains("NullPointerException"), "{message}");
}

// port: RuntimeJsLibManagerTest#injectLibForField_addsCorrespondingLibrary
#[test]
fn inject_lib_for_field_adds_corresponding_library() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[], None);

    manager.inject_lib_for_field(&mut compiler, "$jscomp.inherits");

    assert_eq!(manager.get_injected_libraries(), vec!["es6/util/inherits"]);
}

// port: RuntimeJsLibManagerTest#injectLibForField_crashesIfNonQualifiedName
#[test]
fn inject_lib_for_field_crashes_if_non_qualified_name() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[], None);

    let message = panic_message(|| manager.inject_lib_for_field(&mut compiler, "3 + 4"));
    assert_eq!(message, "Field name must start with $jscomp., found 3 + 4");
}

// port: RuntimeJsLibManagerTest#injectLibForField_crashesIfPassedNonJscompName
#[test]
fn inject_lib_for_field_crashes_if_passed_non_jscomp_name() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[], None);

    let message = panic_message(|| manager.inject_lib_for_field(&mut compiler, "foobar"));
    assert_eq!(message, "Field name must start with $jscomp., found foobar");
}

// port: RuntimeJsLibManagerTest#injectLibForField_crashesIfPassedJustJscomp
#[test]
fn inject_lib_for_field_crashes_if_passed_just_jscomp() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[], None);

    let message = panic_message(|| manager.inject_lib_for_field(&mut compiler, "$jscomp"));
    assert_eq!(
        message,
        "Field name must start with $jscomp., found $jscomp"
    );
}

// port: RuntimeJsLibManagerTest#injectLibForField_crashesIfPassedMissingProperty
#[test]
fn inject_lib_for_field_crashes_if_passed_missing_property() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[], None);

    let message =
        panic_message(|| manager.inject_lib_for_field(&mut compiler, "$jscomp.doesNotExist"));
    assert!(
        message.contains("Cannot find definition of $jscomp.doesNotExist"),
        "{message}"
    );
}

// port: RuntimeJsLibManagerTest#injectLibForField_thenAssertInjectedAndGetQualifiedName_passes
#[test]
fn inject_lib_for_field_then_assert_injected_and_get_qualified_name_passes() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[], None);

    manager.inject_lib_for_field(&mut compiler, "$jscomp.inherits");
    let field = manager.get_js_lib_field("$jscomp.inherits");

    assert_eq!(field.assert_injected().qualified_name(), "$jscomp.inherits");
}

// port: RuntimeJsLibManagerTest#noinjectLibForField_thenAssertInjected_recordAndValidateFieldsMode_fails
#[test]
fn noinject_lib_for_field_then_assert_injected_record_and_validate_fields_mode_fails() {
    let mut manager = create(RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS, &[], None);

    let field = manager.get_js_lib_field("$jscomp.inherits");

    let message = panic_message(|| {
        field.assert_injected();
    });
    assert_eq!(message, "Field $jscomp.inherits is not injected");
}

// port: RuntimeJsLibManagerTest#noinjectLibForField_thenAssertInjected_injectMode_fails
#[test]
fn noinject_lib_for_field_then_assert_injected_inject_mode_fails() {
    let mut manager = create(RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS, &[], None);

    let field = manager.get_js_lib_field("$jscomp.inherits");

    let message = panic_message(|| {
        field.assert_injected();
    });
    assert_eq!(message, "Field $jscomp.inherits is not injected");
}

// port: RuntimeJsLibManagerTest#noinjectLibForField_thenAssertInjectedAndGetQualifiedName_recordOnlyMode_succeeds
#[test]
fn noinject_lib_for_field_then_assert_injected_and_get_qualified_name_record_only_mode_succeeds() {
    let mut manager = create(RuntimeLibraryMode::RECORD_ONLY, &[], None);

    let field = manager.get_js_lib_field("$jscomp.inherits");

    let _unused = field.assert_injected().qualified_name();
}

// port: RuntimeJsLibManagerTest#noinjectLibForField_thenAssertInjectedAndGetQualifiedName_noOpMode_succeeds
#[test]
fn noinject_lib_for_field_then_assert_injected_and_get_qualified_name_no_op_mode_succeeds() {
    let mut manager = create(RuntimeLibraryMode::NO_OP, &[], None);

    let field = manager.get_js_lib_field("$jscomp.inherits");

    let _unused = field.assert_injected().qualified_name();
}

// port: RuntimeJsLibManagerTest#field_withoutInjection_allowsCallingMatches
#[test]
fn field_without_injection_allows_calling_matches() {
    let mut compiler = Compiler::new();
    let mut manager = create(RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS, &[], None);

    let field = manager.get_js_lib_field("$jscomp.inherits");

    let foo = IR::name(&mut compiler, "foo");
    assert!(!field.matches(&compiler, foo));
    let inherits = jscomp_inherits(&mut compiler);
    assert!(field.matches(&compiler, inherits));
}

// port: RuntimeJsLibManagerTest#externFieldNamesMode_doesNotInject_butRecordsLibraries
#[test]
fn extern_field_names_mode_does_not_inject_but_records_libraries() {
    let mut compiler = Compiler::new();
    let mut manager = create(
        RuntimeLibraryMode::EXTERN_FIELD_NAMES,
        &[("lib", "0")],
        None,
    );

    let injected = manager.ensure_library_injected(&mut compiler, "lib", /* force= */ false);

    assert!(injected.is_none());
    assert_eq!(manager.get_injected_libraries(), vec!["lib"]);
}

// port: RuntimeJsLibManagerTest#externFieldNamesMode_doesRecordAndInject_withForceInjection
#[test]
fn extern_field_names_mode_does_record_and_inject_with_force_injection() {
    let mut compiler = Compiler::new();
    let script = IR::script(&mut compiler);
    let mut manager = create(
        RuntimeLibraryMode::EXTERN_FIELD_NAMES,
        &[("lib", "0")],
        Some(script),
    );

    manager.ensure_library_injected(&mut compiler, "lib", /* force= */ true);

    assert_eq!(manager.get_injected_libraries(), vec!["lib"]);
    let expected = js(&mut compiler, "0;");
    assert_node_equal(&compiler, script, expected);
}

fn externs_script(compiler: &mut Compiler) -> NodeId {
    let externs = IR::script(compiler);
    let file: Arc<dyn StaticSourceFile> = Arc::new(SourceFile::from_code_with_kind(
        "ex.js",
        "",
        SourceKind::EXTERN,
    ));
    externs.set_static_source_file(compiler, Some(file));
    externs
}

// port: RuntimeJsLibManagerTest#externFieldNamesMode_injectLibForField_recordsAndAddsExternDecl
#[test]
fn extern_field_names_mode_inject_lib_for_field_records_and_adds_extern_decl() {
    let mut compiler = Compiler::new();
    let externs = externs_script(&mut compiler);
    let mut manager = create(RuntimeLibraryMode::EXTERN_FIELD_NAMES, &[], Some(externs));

    manager.inject_lib_for_field(&mut compiler, "$jscomp.inherits");

    assert_eq!(manager.get_injected_libraries(), vec!["es6/util/inherits"]);
    let externed_fields = manager.get_externed_fields();
    assert_eq!(externed_fields.len(), 1);
    assert_eq!(externed_fields[0].uncompiled_name(), "$jscomp.inherits");
    assert_eq!(externed_fields[0].qualified_name(), "$jscomp_inherits");
    let expected = js(&mut compiler, "var $jscomp_inherits;");
    assert_node_equal(&compiler, externs, expected);
    assert!(
        externs
            .get_only_child(&compiler)
            .get_only_child(&compiler)
            .get_boolean_prop(&compiler, NodeId::IS_CONSTANT_NAME)
    );
}

// port: RuntimeJsLibManagerTest#externFieldNamesMode_injectLibForFieldTwice_doesNotAddDuplicateExterns
#[test]
fn extern_field_names_mode_inject_lib_for_field_twice_does_not_add_duplicate_externs() {
    let mut compiler = Compiler::new();
    let externs = externs_script(&mut compiler);
    let mut manager = create(RuntimeLibraryMode::EXTERN_FIELD_NAMES, &[], Some(externs));

    manager.inject_lib_for_field(&mut compiler, "$jscomp.inherits");
    manager.inject_lib_for_field(&mut compiler, "$jscomp.inherits");

    let expected = js(&mut compiler, "var $jscomp_inherits;");
    assert_node_equal(&compiler, externs, expected);
}

// port: RuntimeJsLibManagerTest#externFieldNamesMode_noinjectLibForField_thenAssertInjected_fails
#[test]
fn extern_field_names_mode_noinject_lib_for_field_then_assert_injected_fails() {
    let mut manager = create(RuntimeLibraryMode::EXTERN_FIELD_NAMES, &[], None);

    let field = manager.get_js_lib_field("$jscomp.inherits");

    let message = panic_message(|| {
        field.assert_injected();
    });
    assert_eq!(message, "Field $jscomp.inherits is not injected");
}
