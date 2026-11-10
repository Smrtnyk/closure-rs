/*
 * Copyright 2026 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 51d8099:
//   test/com/google/javascript/jscomp/ClosureExportTypeClassifierTest.java.

//! Port of ClosureExportTypeClassifierTest. Each case classifies the goog.module 'm'. The expected
//! values match what Clutz's type-based classification produces for the same code as of October
//! 2026, except in the cases under "Known divergences from Clutz", which pin the syntactic answer
//! and say what Clutz returns instead.
//!
//! The Java test is parameterized: every case runs on a fresh test instance (`setUp`), as each
//! iteration below does with a fresh harness.
use closure_jscomp::{
    closure_export_type_classifier::{ClosureExportTypeClassifier, TsExportType},
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    gather_module_metadata::GatherModuleMetadata,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    jscomp_api::Compiler,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

const DEFAULT: TsExportType = TsExportType::DEFAULT;
const NAMED: TsExportType = TsExportType::NAMED;

/// Returns the source of the goog.module 'm' with the given body.
// port: ClosureExportTypeClassifierTest#module
fn module(body: &str) -> String {
    format!("goog.module('m');\n{body}")
}

// port: ClosureExportTypeClassifierTest#cases
fn cases() -> Vec<(&'static str, TsExportType, String)> {
    vec![
        // goog.module: named exports
        (
            "objectLiteralShorthand",
            NAMED,
            module("const A = 1; class B {} exports = {A, B};"),
        ),
        (
            "objectLiteralArbitraryValues",
            NAMED,
            module("exports = {a: 1, f() {}, 'quoted': 2};"),
        ),
        (
            "exportsDotAssignments",
            NAMED,
            module("exports.A = 1; exports.B = function() {};"),
        ),
        (
            "typedefStubOnly",
            NAMED,
            module("/** @typedef {number} */ exports.T;"),
        ),
        (
            "emptyObjectThenProps",
            NAMED,
            module("exports = {}; exports.a = 1;"),
        ),
        (
            "localNamespaceAlias",
            NAMED,
            module("const ns = {}; ns.a = 1; ns.b = function() {}; exports = ns;"),
        ),
        (
            "localConstNamespaceAlias",
            NAMED,
            module("/** @const */ const ns = {}; /** @const */ ns.x = 1; exports = ns;"),
        ),
        (
            "localObjectLiteralAlias",
            NAMED,
            module("const ns = {a: 1}; exports = ns;"),
        ),
        (
            "aliasChain2",
            NAMED,
            module("const a = {x: 1}; const b = a; exports = b;"),
        ),
        (
            "aliasChain3",
            NAMED,
            module("const a = {x: 1}; const b = a; const c = b; exports = c;"),
        ),
        (
            "aliasChain4",
            NAMED,
            module("const a = {x: 1}; const b = a; const c = b; const d = c; exports = d;"),
        ),
        // The alias chain is too deep, so this falls back to DEFAULT.
        (
            "aliasChain5_tooDeep",
            DEFAULT,
            module(
                "const a = {x: 1};
const b = a;
const c = b;
const d = c;
const e = d;
exports = e;
",
            ),
        ),
        (
            "recordTypeDeclaration",
            NAMED,
            module("/** @const {{a: number}} */ const X = {a: 1}; exports = X;"),
        ),
        (
            "legacyNamespace",
            NAMED,
            module("goog.module.declareLegacyNamespace(); exports = {a: 1};"),
        ),
        // goog.module: default exports
        ("class", DEFAULT, module("class Foo {} exports = Foo;")),
        ("classExpression", DEFAULT, module("exports = class {};")),
        (
            "classWithStaticProps",
            DEFAULT,
            module("class Foo {} Foo.Bar = 1; exports = Foo;"),
        ),
        (
            "classThenExportsProps",
            DEFAULT,
            module("exports = class {}; exports.Bar = 1;"),
        ),
        ("function", DEFAULT, module("exports = function() {};")),
        (
            "functionDeclaration",
            DEFAULT,
            module("function f() {} f.x = 1; exports = f;"),
        ),
        (
            "interface",
            DEFAULT,
            module("/** @interface */ class I {} exports = I;"),
        ),
        (
            "enum",
            DEFAULT,
            module("/** @enum {string} */ const E = {X: 'x'}; exports = E;"),
        ),
        (
            "enumOnExports",
            DEFAULT,
            module("/** @enum {string} */ exports = {X: 'x'};"),
        ),
        (
            "typedef",
            DEFAULT,
            module("/** @typedef {{a: number}} */ let T; exports = T;"),
        ),
        ("number", DEFAULT, module("exports = 5;")),
        ("string", DEFAULT, module("exports = 'hello';")),
        (
            "instance",
            DEFAULT,
            module("class Foo {} exports = new Foo();"),
        ),
        ("emptyObject", DEFAULT, module("exports = {};")),
        ("noExports", DEFAULT, module("const x = 1;")),
        (
            "callObjectFreeze",
            DEFAULT,
            module("exports = Object.freeze({a: 1, b: 2});"),
        ),
        (
            "callGlobalFunction",
            DEFAULT,
            module("exports = someGlobalFn();"),
        ),
        (
            "callGoogDefineClass",
            DEFAULT,
            module("exports = goog.defineClass(null, {constructor: function() {}});"),
        ),
        (
            "reexportOfRequireOutsideLibrary",
            DEFAULT,
            module("const Other = goog.require('other'); exports = Other;"),
        ),
        // The exports object itself is still an object literal with properties.
        (
            "destructuredRequireInObjectLiteral",
            NAMED,
            module("const {A} = goog.require('other'); exports = {A};"),
        ),
        (
            "reexportOfDestructuredRequire",
            DEFAULT,
            module("const {A} = goog.require('other'); exports = A;"),
        ),
        (
            "declaredObjectMapType",
            DEFAULT,
            module("/** @const {!Object<string, number>} */ const X = {a: 1}; exports = X;"),
        ),
        (
            "unknownType",
            DEFAULT,
            module("/** @type {?} */ const x = foo(); exports = x;"),
        ),
        (
            "castToRecordType",
            NAMED,
            module("exports = /** @type {{a: number}} */ (foo());"),
        ),
        (
            "castToNamedType",
            DEFAULT,
            module("exports = /** @type {!Foo} */ ({a: 1});"),
        ),
        (
            "selfReferentialAliasTerminates",
            DEFAULT,
            module("let a = b; let b = a; exports = a;"),
        ),
        // @legacyNamedTsExport
        // The annotation only has an effect on the goog.module call.
        (
            "overrideOnExportsAssignment_hasNoEffect",
            DEFAULT,
            module(
                "const Other = goog.require('other');
/** @legacyNamedTsExport */
exports = Other;
",
            ),
        ),
        (
            "overrideOnGoogModuleCall",
            NAMED,
            "/** @legacyNamedTsExport */
goog.module('m');
const a = goog.require('x');
exports = a;
"
            .to_string(),
        ),
        // Patterns found while comparing against Clutz on real code.
        // No named own properties.
        (
            "computedKeysOnly",
            DEFAULT,
            module("const k = 'a'; exports = {[k]: 1};"),
        ),
        // The named key 'b' is the single named export.
        (
            "computedAndNamedKeys",
            NAMED,
            module("const k = 'a'; exports = {[k]: 1, b: 2};"),
        ),
        (
            "reassignedLet_objectThenFunction",
            DEFAULT,
            module("function f() {} let m = {}; m = f; exports = m;"),
        ),
        (
            "reassignedLet_objectThenNumber",
            DEFAULT,
            module("let m = {a: 1}; m = 5; exports = m;"),
        ),
        (
            "reassignedLet_numberThenObject",
            NAMED,
            module("let m = 5; m = {a: 1}; exports = m;"),
        ),
        (
            "reassignedLet_uninitializedThenObject",
            NAMED,
            module("let m; m = {a: 1}; exports = m;"),
        ),
        (
            "reassignedLet_objectThenCall",
            DEFAULT,
            module("let m = {}; m = foo(); exports = m;"),
        ),
        (
            "localTypedef",
            DEFAULT,
            module(
                "/** @typedef {number} */
let Num;
/** @const {!Num} */
const X = 1;
exports = X;
",
            ),
        ),
        // Module rewriting turns this into `exports.X = X;`, dropping the @type.
        (
            "namedExportsLiteralIgnoresDeclaredType",
            NAMED,
            module("const X = {}; /** @type {!Object<string, number>} */ exports = {X};"),
        ),
        (
            "namedExportsLiteralIgnoresTypedef",
            NAMED,
            module("const X = 1; /** @typedef {number} */ exports = {X};"),
        ),
        // Not a named exports literal: the declared type wins.
        (
            "objectLiteralWithDeclaredType",
            DEFAULT,
            module("/** @type {!Object<string, number>} */ exports = {X: 1};"),
        ),
        // Only top-level statements are scanned.
        (
            "controlFlowNotScanned_object",
            DEFAULT,
            module("let X = {}; if (goog.DEBUG) { X = {a: 1}; } exports = X;"),
        ),
        (
            "controlFlowNotScanned_number",
            DEFAULT,
            module("let X = 1; if (goog.DEBUG) { X = {a: 1}; } exports = X;"),
        ),
        // Branches not covered elsewhere.
        (
            "declaredRecordType",
            NAMED,
            module("/** @const {{a: number}} */ const x = {a: 1}; exports = x;"),
        ),
        (
            "declaredNonNullableRecordType",
            DEFAULT,
            // This is equivalent to the declaredRecordType case, but the explicit non-nullability
            // makes it a DEFAULT export. If necessary we can add support for normalizing away the
            // `!` later.
            module("/** @type {!{a: number}} */ const x = {a: 1}; exports = x;"),
        ),
        (
            "declaredNullableRecordType",
            DEFAULT,
            module("/** @type {{a: number}|null} */ const x = {a: 1}; exports = x;"),
        ),
        ("uninitializedLocal", DEFAULT, module("let x; exports = x;")),
        (
            "varRedeclarationKeepsValue",
            NAMED,
            module("var ns = {a: 1}; var ns; exports = ns;"),
        ),
        (
            "spreadOnlyObjectLiteral",
            DEFAULT,
            module("const o = {a: 1}; const x = {...o}; exports = x;"),
        ),
        (
            "reassignedLocal_alias",
            NAMED,
            module("const o = {a: 1}; let m = {}; m = o; exports = m;"),
        ),
        (
            "reassignedLocal_cast",
            DEFAULT,
            module("let m = {}; m = /** @type {?} */ ({}); exports = m;"),
        ),
        (
            "reassignedFunction",
            DEFAULT,
            module("let m = function() {}; m = function(x) {}; exports = m;"),
        ),
        (
            "reassignedFunctionToClass",
            DEFAULT,
            module("let m = function() {}; m = class {}; exports = m;"),
        ),
        (
            "reassignedInstance",
            DEFAULT,
            module("class Foo {} let m = new Foo(); m = new Foo(); exports = m;"),
        ),
        (
            "reassignedArray",
            DEFAULT,
            module("let m = [1]; m = [2]; exports = m;"),
        ),
        // Known divergences from Clutz. Each pins the syntactic answer; Clutz's mismatch check
        // reports them, and affected code is annotated with @legacyNamedTsExport or restructured.
        (
            "knownDivergence_reassignmentDropsDeclaredType",
            NAMED,
            module("/** @type {!Object} */ let m = {a: 1}; m = {b: 2}; exports = m;"),
        ),
        (
            "knownDivergence_conditionalExpression",
            DEFAULT,
            module("exports = Math.random() ? {a: 1} : {b: 2};"),
        ),
        (
            "knownDivergence_commaOperator",
            DEFAULT,
            module("exports = (0, {a: 1});"),
        ),
        (
            "knownDivergence_nameAssignedInControlFlow",
            DEFAULT,
            module("let X; if (goog.DEBUG) { X = {a: 1}; } exports = X;"),
        ),
        (
            "knownDivergence_typedefOnExportsObjectLiteral",
            DEFAULT,
            module("/** @typedef {number} */ exports = {x: 1};"),
        ),
        (
            "knownDivergence_typedefOnLocalObjectLiteral",
            DEFAULT,
            module("/** @typedef {number} */ const X = {x: 1}; exports = X;"),
        ),
        (
            "knownDivergence_recordTypedef",
            DEFAULT,
            module(
                "/** @typedef {{a: number}} */
let Rec;
/** @const {!Rec} */
const X = {a: 1};
exports = X;
",
            ),
        ),
        (
            "knownDivergence_unionOfRecordTypes",
            DEFAULT,
            module("/** @const {{a: number}|{b: number}} */ exports = {a: 1};"),
        ),
    ]
}

/// The overrides of ClosureExportTypeClassifierTest (getProcessor).
struct ClosureExportTypeClassifierTest {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for ClosureExportTypeClassifierTest {
    // port: ClosureExportTypeClassifierTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                GatherModuleMetadata::new(
                    /* process_common_js_modules= */ false,
                    ResolutionMode::BROWSER,
                )
                .process(compiler, externs, root);
            },
        )))))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: ClosureExportTypeClassifierTest#setUp
fn set_up() -> (CompilerTestCase, ClosureExportTypeClassifierTest) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness
        .set_language_out(DslValue::Enum {
            class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
            name: "ECMASCRIPT5".into(),
        })
        .unwrap();
    let hooks = ClosureExportTypeClassifierTest {
        ctx: Ctx::new(
            "ClosureExportTypeClassifierTest".into(),
            closure_testing::replay::replay_values::object([]),
            IndexMap::<_, _>::default(),
            Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
        ),
    };
    (harness, hooks)
}

// port: ClosureExportTypeClassifierTest#testExportType
#[test]
fn test_export_type() {
    for (description, expected, code) in cases() {
        let (mut harness, mut hooks) = set_up();
        if let Err(t) = harness.test_same_string(&mut hooks, code.as_str()) {
            panic!("{description}: {t:?}");
        }
        let compiler = harness.get_last_compiler().unwrap();
        let compiler = compiler.borrow();
        let m = compiler
            .get_module_metadata_map()
            .unwrap()
            .get_modules_by_goog_namespace()
            .get(&JsString::from("m"))
            .cloned();
        let m = m.unwrap_or_else(|| panic!("{description}: no module for m"));
        assert_eq!(
            ClosureExportTypeClassifier::classify(&compiler, m.root_node().unwrap(), "m"),
            expected,
            "{description}: export type of m in:\n{code}"
        );
    }
}
