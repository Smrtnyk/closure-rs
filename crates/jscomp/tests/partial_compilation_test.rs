/*
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/BasicErrorManager.java,
//   test/com/google/javascript/jscomp/PartialCompilationTest.java.

//! Port of PartialCompilationTest: tests exercising
//! CompilerOptions#assumeForwardDeclaredForMissingTypes and
//! DiagnosticGroups#MISSING_SOURCES_WARNINGS.
use closure_jscomp::{
    Compiler, check_level::CheckLevel, compilation_level::CompilationLevel,
    compiler_options::CompilerOptions, diagnostic_group::DiagnosticGroup, diagnostic_groups,
    error_handler::ErrorHandler, error_manager::ErrorManager,
    function_type_builder::RESOLVED_TAG_EMPTY, js_error::JSError,
    sorting_error_manager::SortingErrorManager, source_file::SourceFile,
    type_check::CONFLICTING_EXTENDED_TYPE, typed_scope::TypedScope,
};
use closure_jstype::{
    JSTypeRegistry, TypeId, function_type::FunctionType, js_type::JSType, object_type::ObjectType,
    union_type::UnionType,
};
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::sync::Arc;

/// The anonymous `BasicErrorManager` subclass of assertPartialCompilationSucceeds: every report
/// is recorded as an ERROR, and nothing is printed.
struct ErrorsOnlyErrorManager {
    base: SortingErrorManager,
}

impl ErrorHandler for ErrorsOnlyErrorManager {
    // port: PartialCompilationTest#assertPartialCompilationSucceeds (anonymous BasicErrorManager#report)
    fn report(&mut self, _level: CheckLevel, error: JSError) {
        self.base.report(CheckLevel::ERROR, error);
    }
}

impl ErrorManager for ErrorsOnlyErrorManager {
    // port: BasicErrorManager#generateReport
    fn generate_report(&mut self, _ast: &Ast) {
        // println(level, error) and printSummary() are no-ops.
        for _message in self.base.get_sorted_diagnostics() { /* no-op */ }
        /* no-op */
    }
    fn get_error_count(&self) -> i32 {
        self.base.get_error_count()
    }
    fn get_warning_count(&self) -> i32 {
        self.base.get_warning_count()
    }
    fn get_errors(&self) -> Vec<JSError> {
        self.base.get_errors()
    }
    fn get_warnings(&self) -> Vec<JSError> {
        self.base.get_warnings()
    }
    fn set_typed_percent(&mut self, typed_percent: f64) {
        self.base.set_typed_percent(typed_percent);
    }
    fn get_typed_percent(&self) -> f64 {
        self.base.get_typed_percent()
    }
    fn has_halting_errors(&self) -> bool {
        self.base.has_halting_errors()
    }
}

/// Asserts that the given lines of code compile and only give errors matching the
/// DiagnosticGroups#MISSING_SOURCES_WARNINGS category.
// port: PartialCompilationTest#assertPartialCompilationSucceeds(String)
fn assert_partial_compilation_succeeds(code: &str) -> Compiler {
    assert_partial_compilation_succeeds_with(code, create_compiler_options())
}

// port: PartialCompilationTest#assertPartialCompilationSucceeds(String,CompilerOptions)
fn assert_partial_compilation_succeeds_with(code: &str, options: CompilerOptions) -> Compiler {
    let mut compiler = Compiler::new();
    compiler.set_error_manager(Box::new(ErrorsOnlyErrorManager {
        base: SortingErrorManager::new(Vec::new()),
    }));
    compiler.init(
        &[],
        &[Arc::new(SourceFile::from_code("input.js", code))],
        options,
    );
    compiler.parse();
    compiler.check();

    assert_eq!(compiler.get_errors(), vec![], "Expected no errors");
    assert_eq!(compiler.get_warnings(), vec![], "Expected no warnings");
    compiler
}

// port: PartialCompilationTest#createCompilerOptions
fn create_compiler_options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_assume_forward_declared_for_missing_types(true);
    options.set_closure_pass(true);
    options.set_strict_mode_input(true);
    options.set_preserve_detailed_source_info(true);
    options.set_warning_level(
        diagnostic_groups::UNDEFINED_VARIABLES.clone(),
        CheckLevel::OFF,
    );
    options.set_warning_level(
        diagnostic_groups::MISSING_SOURCES_WARNINGS.clone(),
        CheckLevel::OFF,
    );
    options.set_warning_level(
        DiagnosticGroup::for_type(&RESOLVED_TAG_EMPTY),
        CheckLevel::OFF,
    );
    options.set_warning_level(
        DiagnosticGroup::for_type(&CONFLICTING_EXTENDED_TYPE),
        CheckLevel::OFF,
    );
    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    options
}

/// `scope.getSlot(name).getType()`, asserting the slot exists.
fn slot_type(compiler: &mut Compiler, scope: TypedScope, name: &str) -> TypeId {
    let var = scope
        .get_slot(compiler, JsString::from(name))
        .unwrap_or_else(|| panic!("no slot {name}"));
    var.get_type(compiler).expect("type")
}

/// Runs `f` with the compiler's type registry and AST.
fn with<T>(compiler: &mut Compiler, f: impl FnOnce(&mut JSTypeRegistry, &Ast) -> T) -> T {
    let (registry, ast) = compiler.get_type_registry_and_ast();
    f(registry, ast)
}

/// `type.getReferenceName()` as a Rust string.
fn reference_name(registry: &JSTypeRegistry, t: TypeId) -> String {
    t.get_reference_name(registry)
        .map(|name| name.to_string_lossy())
        .unwrap_or_default()
}

// port: PartialCompilationTest#testUsesMissingCode
#[test]
fn test_uses_missing_code() {
    assert_partial_compilation_succeeds(
        "goog.provide('missing_code_user');
goog.require('some.thing.Missing');
missing_code_user.fnUsesMissingNs = function() {
  missing_code_user.missingNamespace.foo();
  missingTopLevelNamespace.bar();
};
",
    );
}

// port: PartialCompilationTest#testMissingType_variable
#[test]
fn test_missing_type_variable() {
    assert_partial_compilation_succeeds(
        "/** @type {!some.thing.Missing} */ var foo;
",
    );
}

// port: PartialCompilationTest#testMissingType_assignment
#[test]
fn test_missing_type_assignment() {
    assert_partial_compilation_succeeds(
        "/** @type {!some.thing.Missing} */ var foo;
/** @suppress {checkTypes} @type {number} */ var bar = foo;
",
    );
}

// port: PartialCompilationTest#testMissingRequire
#[test]
fn test_missing_require() {
    assert_partial_compilation_succeeds(
        "goog.provide('missing_extends');
goog.require('some.thing.Missing');
",
    );
}

// port: PartialCompilationTest#testMissingExtends
#[test]
fn test_missing_extends() {
    assert_partial_compilation_succeeds(
        "goog.provide('missing_extends');
/** @constructor @extends {some.thing.Missing} */
missing_extends.Extends = function() {}
",
    );
}

// port: PartialCompilationTest#testMissingExtends_template
#[test]
fn test_missing_extends_template() {
    assert_partial_compilation_succeeds(
        "goog.provide('missing_extends');
/** @constructor @extends {some.thing.Missing<string>} x */
missing_extends.Extends = function() {}
",
    );
}

// port: PartialCompilationTest#testMissingType_typedefAlias
#[test]
fn test_missing_type_typedef_alias() {
    assert_partial_compilation_succeeds(
        "/** @typedef {string} */ var typedef;
",
    );
}

// port: PartialCompilationTest#testMissingType_typedefField
#[test]
fn test_missing_type_typedef_field() {
    assert_partial_compilation_succeeds(
        "/** @typedef {some.thing.Missing} */ var typedef;
",
    );
}

// port: PartialCompilationTest#testMissingEs6Externs
#[test]
fn test_missing_es6_externs() {
    assert_partial_compilation_succeeds(
        "let foo = {a, b};
",
    );
}

// port: PartialCompilationTest#testUnresolvedGenerics
#[test]
fn test_unresolved_generics() {
    let mut compiler = assert_partial_compilation_succeeds(
        "/** @type {!some.thing.Missing<string, !AlsoMissing<!More>>} */
var x;
",
    );
    let top_scope = compiler.get_top_scope().expect("topScope");
    let x = slot_type(&mut compiler, top_scope, "x");
    with(&mut compiler, |registry, ast| {
        assert!(
            x.is_no_resolved_type(registry),
            "type {}",
            x.to_string(registry, ast)
        );
        let templatized_type = x.assert_object_type(registry, ast);
        assert_eq!(
            reference_name(registry, templatized_type),
            "some.thing.Missing"
        );
        let template_types = templatized_type
            .get_template_types(registry)
            .unwrap_or_default();
        assert!(template_types[0].is_string(registry, ast));
        assert!(template_types[1].is_object(registry, ast));
        let also_missing = template_types[1];
        assert_eq!(reference_name(registry, also_missing), "AlsoMissing");
        let also_missing_template_types = also_missing
            .get_template_types(registry)
            .unwrap_or_default();
        assert_eq!(also_missing_template_types.len(), 1);
        let more = also_missing_template_types[0];
        assert_eq!(reference_name(registry, more), "More");
    });
}

// port: PartialCompilationTest#testUnresolvedUnions
#[test]
fn test_unresolved_unions() {
    let mut compiler = assert_partial_compilation_succeeds(
        "/** @type {some.thing.Foo|some.thing.Bar} */
var x;
",
    );
    let top_scope = compiler.get_top_scope().expect("topScope");
    let x = slot_type(&mut compiler, top_scope, "x");
    with(&mut compiler, |registry, ast| {
        assert!(
            x.is_union_type(registry),
            "type {}",
            x.to_string(registry, ast)
        );
        let union_type = x;

        let alternatives = union_type.get_alternates(registry, ast);
        assert_eq!(alternatives.len(), 3);

        let mut null_type_count = 0;
        let mut named_types: Vec<String> = Vec::new();
        for &alternative in alternatives.iter() {
            assert!(alternative.is_named_type(registry) || alternative.is_null_type(registry));
            if alternative.is_named_type(registry) {
                assert!(alternative.is_no_resolved_type(registry));
                named_types.push(reference_name(registry, alternative));
            }
            if alternative.is_null_type(registry) {
                null_type_count += 1;
            }
        }
        assert_eq!(null_type_count, 1);
        named_types.sort();
        assert_eq!(named_types, vec!["some.thing.Bar", "some.thing.Foo"]);
    });
}

// port: PartialCompilationTest#testUnresolvedGenerics_defined
#[test]
fn test_unresolved_generics_defined() {
    assert_partial_compilation_succeeds(
        "/** @param {!some.thing.Missing<string>} x */
function useMissing(x) {}
/** @const {!some.thing.Missing<string>} */
var x;
/** @constructor @template T */
some.thing.Missing = function () {}
function missingInside() {
  useMissing(new some.thing.Missing());
}
",
    );
}

// port: PartialCompilationTest#nonMonotonicDataFlowBug
#[test]
fn non_monotonic_data_flow_bug() {
    // Regression test for b/156014526
    assert_partial_compilation_succeeds(
        "class AggregatedColumn {
  /**  @param {ForwardDeclaredType} col */
  constructor(col) {
    this.child = col;
  }
}

/** @param {!AggregatedColumn} col */
function getColumnChain(col) {
    var currentColumn = col;
    while (currentColumn instanceof AggregatedColumn) {
      currentColumn = currentColumn.child;
    }
}
",
    );
}

/// `derived.getType().toMaybeFunctionType().getPrototype().getImplicitPrototype()`, asserted to
/// be a NoResolvedType named `expected_name` (`what` labels the messages).
fn assert_missing_super(compiler: &mut Compiler, derived: TypeId, what: &str, expected_name: &str) {
    with(compiler, |registry, ast| {
        let ctor = derived
            .to_maybe_function_type(registry)
            .expect("function type");
        let prototype = ctor.get_prototype(registry, ast);
        let super_type = prototype.get_implicit_prototype(registry, ast);
        let super_type = super_type.unwrap_or_else(|| panic!("{what} null"));
        assert!(
            super_type.is_no_resolved_type(registry),
            "{what} {}",
            super_type.to_string(registry, ast)
        );
        assert_eq!(reference_name(registry, super_type), expected_name);
    });
}

// port: PartialCompilationTest#testMissingModuleImportExtends_keepOriginalModuleExportNames
#[test]
fn test_missing_module_import_extends_keep_original_module_export_names() {
    let mut options = create_compiler_options();
    options.set_enable_module_rewriting(false);
    options.set_checks_only(true);
    options.set_preserve_detailed_source_info(true);

    let mut compiler = assert_partial_compilation_succeeds_with(
        "goog.module('test.module');
const MissingDefaultBase = goog.require('another.missing.module');
const {MissingDestructuredBase} = goog.require('some.missing.module');
const {MissingRenamedBase: NewName} = goog.require('some.missing.module');

class DerivedDefault extends MissingDefaultBase {}
class DerivedDestructured extends MissingDestructuredBase {}
class DerivedRenamed extends NewName {}
",
        options,
    );

    let file: NodeId = compiler
        .get_js_root()
        .expect("jsRoot")
        .get_last_child(&compiler)
        .expect("file");
    check_state!(
        file.is_script(&compiler)
            && file
                .get_first_child(&compiler)
                .is_some_and(|c| c.is_module_body(&compiler)),
        "%s",
        file.to_string(&compiler)
    );
    let module_body = file.get_first_child(&compiler);
    let module_scope: TypedScope = {
        let tsc = compiler.get_typed_scope_creator();
        tsc.get_node_to_scope_mapper()(module_body).expect("moduleScope")
    };

    let missing_default = slot_type(&mut compiler, module_scope, "MissingDefaultBase");
    assert!(with(&mut compiler, |registry, _| {
        missing_default.is_no_resolved_type(registry)
    }));

    let missing_destructured = slot_type(&mut compiler, module_scope, "MissingDestructuredBase");
    assert!(with(&mut compiler, |registry, _| {
        missing_destructured.is_no_resolved_type(registry)
    }));

    let missing_renamed = slot_type(&mut compiler, module_scope, "NewName");
    assert!(with(&mut compiler, |registry, _| {
        missing_renamed.is_no_resolved_type(registry)
    }));

    let derived_default = slot_type(&mut compiler, module_scope, "DerivedDefault");
    assert_missing_super(
        &mut compiler,
        derived_default,
        "defaultSuper",
        "MissingDefaultBase",
    );

    let derived_destructured = slot_type(&mut compiler, module_scope, "DerivedDestructured");
    assert_missing_super(
        &mut compiler,
        derived_destructured,
        "destructuredSuper",
        "MissingDestructuredBase",
    );

    let derived_renamed = slot_type(&mut compiler, module_scope, "DerivedRenamed");
    assert_missing_super(&mut compiler, derived_renamed, "renamedSuper", "NewName");
}
