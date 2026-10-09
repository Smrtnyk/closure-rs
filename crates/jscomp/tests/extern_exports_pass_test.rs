/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ExternExportsPassTest.java.

//! Port of `ExternExportsPassTest.java`: tests for `ExternExportsPass`.
//!
//! `exportClassWithoutTypeCheck` is `@Ignore`d in Java (b/141729691) and is not ported.
use closure_jscomp::{compiler_options::CompilerOptions, extern_exports_pass::ExternExportsPass};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, MINIMAL_EXTERNS, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct ExternExportsPassTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: ExternExportsPassTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_extern_exports_path(Some("exports.js".into()));
        // Check types so we can make sure our exported externs have type information.
        options.set_check_symbols(true);
        Ok(options)
    }

    // port: ExternExportsPassTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass = ExternExportsPass::new(&mut compiler.borrow_mut());
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "ExternExportsPassTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Consumer = Box<dyn Fn(String)>;

/// The `(Postcondition) compiler -> {...}` lambda of `compileAndExportExterns`.
struct ExportPostcondition {
    consumer: Option<Consumer>,
}

impl NativeObject for ExportPostcondition {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.CompilerTestCase$Postcondition"
    }
    // port: ExternExportsPassTest#compileAndExportExterns (the Postcondition lambda)
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        assert_eq!(method, "verify");
        let [DslValue::Compiler(compiler)] = args.as_slice() else {
            panic!("Postcondition#verify takes the compiler");
        };
        if let Some(consumer) = &self.consumer {
            let extern_export = compiler.borrow().get_result().extern_export;
            consumer(extern_export.expect("NullPointerException"));
        }
        Ok(DslValue::Null)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: CompilerTestCase#MINIMAL_EXTERNS
fn minimal_externs() -> String {
    MINIMAL_EXTERNS.clone().unwrap().to_string()
}

impl ExternExportsPassTest {
    // port: ExternExportsPassTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_type_check().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "ExternExportsPassTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
            },
        }
    }

    // port: ExternExportsPassTest#compileAndCheck(String,String)
    fn compile_and_check(&mut self, js: &str, expected: &str) {
        self.compile_and_check_externs(&minimal_externs(), js, expected);
    }

    // port: ExternExportsPassTest#compileAndCheck(String,String,String)
    fn compile_and_check_externs(&mut self, externs: &str, js: &str, expected: &str) {
        let expected = expected.to_string();
        self.compile_and_export_externs_consumer(
            js,
            externs,
            Some(Box::new(move |generated_externs: String| {
                let fileoverview = "/**\n * @fileoverview Generated externs.\n * @externs\n */\n";
                // NOTE(sdh): The type checker just produces {?}.
                // For now we will not worry about this distinction and just normalize it.
                let generated_externs = generated_externs.replace("?=", "?");

                assert_eq!(generated_externs, format!("{fileoverview}{expected}"));
            })),
        );
    }

    /// Compiles the passed in JavaScript and returns the new externs exported by the this pass.
    // port: ExternExportsPassTest#compileAndExportExterns(String)
    fn compile_and_export_externs(&mut self, js: &str) {
        self.compile_and_export_externs_with_externs(js, &minimal_externs());
    }

    /// Compiles the passed in JavaScript with the passed in externs and returns the new externs
    /// exported by the this pass.
    // port: ExternExportsPassTest#compileAndExportExterns(String,String)
    fn compile_and_export_externs_with_externs(&mut self, js: &str, externs: &str) {
        self.compile_and_export_externs_consumer(js, externs, None);
    }

    /// Compiles the passed in JavaScript with the passed in externs and returns the new externs
    /// exported by the this pass.
    // port: ExternExportsPassTest#compileAndExportExterns(String,String,Consumer)
    fn compile_and_export_externs_consumer(
        &mut self,
        js: &str,
        externs: &str,
        consumer: Option<Consumer>,
    ) {
        let js = format!(
            "{}{js}",
            concat!(
                "/** @const */ var goog = {};\n",
                "goog.exportSymbol = function(a, b) {};\n",
                "goog.exportProperty = function(a, b, c) {};\n",
            )
        );

        self.harness
            .test(
                &mut self.hooks,
                vec![
                    TestPart::Externs(CompilerTestCase::externs(JsString::from(externs))),
                    TestPart::Sources(CompilerTestCase::srcs(JsString::from(js))),
                    TestPart::Postcondition(DslValue::Native(Rc::new(RefCell::new(
                        ExportPostcondition { consumer },
                    )))),
                ],
            )
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

// port: ExternExportsPassTest#testExportSymbol
#[test]
fn test_export_symbol() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\n/** @const */ a.b = {};\na.b.c = function(d, e, f) {};\nvar x;\n// Ensure we don't have a recurrence of a bug that caused us to consider the last\n// assignment containing 'a.b.c' to be its definition, even if it was on the rhs.\nx = a.b.c;\ngoog.exportSymbol('foobar', a.b.c)\n",
        "/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @return {undefined}\n */\nvar foobar = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#exportClassAsSymbolAndMethodAsProperty
#[test]
fn export_class_as_symbol_and_method_as_property() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\n/** @const */ a.b = {};\na.b.c = class {\n  constructor(d, e, f) {\n    /** @export {number} */ this.thisProp = 1;\n  }\n  /**\n   * @param {string} a\n   * @return {number}\n   */\n  method(a) {}\n\n  static staticMethod() {}\n};\ngoog.exportSymbol('foobar', a.b.c)\ngoog.exportProperty(a.b.c.prototype, 'exportedMethod', a.b.c.prototype.method)\ngoog.exportProperty(a.b.c, 'exportedStaticMethod', a.b.c.staticMethod)\n",
        // TODO(b/123352214): `@this {!a.b.c}` should be renamed to `foobar`
        "/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @constructor\n */\nvar foobar = function(d, e, f) {\n};\n/**\n * @return {undefined}\n * @this {(typeof a.b.c)}\n */\nfoobar.exportedStaticMethod = function() {\n};\n/**\n * @param {string} a\n * @return {number}\n * @this {!a.b.c}\n */\nfoobar.prototype.exportedMethod = function(a) {\n};\nfoobar.prototype.thisProp;\n");
}

// port: ExternExportsPassTest#noAtThisIsGeneratedForClassMethodWhenExportedClassNameMatches
#[test]
fn no_at_this_is_generated_for_class_method_when_exported_class_name_matches() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "class Foo {\n  /**\n   * @param {string} a\n   * @return {number}\n   */\n  method(a) {}\n};\ngoog.exportSymbol('Foo', Foo)\ngoog.exportProperty(Foo.prototype, 'method', Foo.prototype.method)\n",
        // @this not generated on the method because exported and internal class names are the same
        "/**\n * @constructor\n */\nvar Foo = function() {\n};\n/**\n * @param {string} a\n * @return {number}\n */\nFoo.prototype.method = function(a) {\n};\n");
}

// port: ExternExportsPassTest#exportClassHierarchyAsSymbols
#[test]
fn export_class_hierarchy_as_symbols() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\n/** @const */ a.b = {};\na.b.c = class {\n  constructor(d, e, f) {}\n};\ngoog.exportSymbol('foobar', a.b.c)\na.b.d = class extends a.b.c {\n  constructor(d, e, f) {}\n};\ngoog.exportSymbol('bazboff', a.b.d)\n",
        // NOTE: foobar and bazboff end up sorted in ASCII order. This is expected.
        // TODO(b/123352214): @extends {a.b.c} should be updated
        "/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @extends {a.b.c}\n * @constructor\n */\nvar bazboff = function(d, e, f) {\n};\n/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @constructor\n */\nvar foobar = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#testInterface
#[test]
fn test_interface() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @interface */ function Iface() {};\ngoog.exportSymbol('Iface', Iface)\n",
        "/**\n * @interface\n */\nvar Iface = function() {\n};\n",
    );
}

// port: ExternExportsPassTest#exportInterfaceDefinedWithClass
#[test]
fn export_interface_defined_with_class() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @interface */ class Iface {};\ngoog.exportSymbol('Iface', Iface)\n",
        "/**\n * @interface\n */\nvar Iface = function() {\n};\n",
    );
}

// port: ExternExportsPassTest#exportInterfaceExtendedWithClass
#[test]
fn export_interface_extended_with_class() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @interface */ class Iface {};\n goog.exportSymbol('Iface', Iface)\n/** @interface */ class ExtendedIface extends Iface {};\n goog.exportSymbol('ExtendedIface', ExtendedIface)\n",
        // order of the interfaces is reversed, but that doesn't matter
        "/**\n * @extends {Iface}\n * @interface\n */\nvar ExtendedIface = function() {\n};\n/**\n * @interface\n */\nvar Iface = function() {\n};\n");
}

// port: ExternExportsPassTest#exportInterfaceDefinedWithClassThatExtendsUnexportedInterface
#[test]
fn export_interface_defined_with_class_that_extends_unexported_interface() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @interface */ class Iface {}; // not exported\n/** @interface */ class ExtendedIface extends Iface {};\n goog.exportSymbol('ExtendedIface', ExtendedIface)\n",
        "/**\n * @extends {Iface}\n * @interface\n */\nvar ExtendedIface = function() {\n};\n");
}

// port: ExternExportsPassTest#testRecord
#[test]
fn test_record() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @record */ function Iface() {}; goog.exportSymbol('Iface', Iface)",
        "/**\n * @record\n */\nvar Iface = function() {\n};\n",
    );
}

// port: ExternExportsPassTest#exportRecordDefinedWithClass
#[test]
fn export_record_defined_with_class() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @record */ class Iface {};\ngoog.exportSymbol('Iface', Iface)\n",
        "/**\n * @record\n */\nvar Iface = function() {\n};\n",
    );
}

// port: ExternExportsPassTest#exportRecordExtendedWithClass
#[test]
fn export_record_extended_with_class() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @record */ class Iface {};\n goog.exportSymbol('Iface', Iface)\n/** @record */ class ExtendedIface extends Iface {};\n goog.exportSymbol('ExtendedIface', ExtendedIface)\n",
        // order of the interfaces is reversed, but that doesn't matter
        "/**\n * @extends {Iface}\n * @record\n */\nvar ExtendedIface = function() {\n};\n/**\n * @record\n */\nvar Iface = function() {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolDefinedInVar
#[test]
fn test_export_symbol_defined_in_var() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var a = function(d, e, f) {}; goog.exportSymbol('foobar', a)",
        "/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @return {undefined}\n */\nvar foobar = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#exportClassDefinedWithConst
#[test]
fn export_class_defined_with_const() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "const a = class {\n  constructor(d, e, f) {}\n};\ngoog.exportSymbol('foobar', a)\n",
        // const becomes a var because we only generate var definitions for now
        "/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @constructor\n */\nvar foobar = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#exportClassDefinedWithLet
#[test]
fn export_class_defined_with_let() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "let a = class {\n  constructor(d, e, f) {}\n};\ngoog.exportSymbol('foobar', a)\n",
        // The let becomes a var because we only generate var definitions for now
        "/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @constructor\n */\nvar foobar = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#testExportProperty
#[test]
fn test_export_property() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\n/** @const */ a.b = {};\na.b.c = function(d, e, f) {};\ngoog.exportProperty(a.b, 'cprop', a.b.c)\n",
        "var a;\na.b;\n/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @return {undefined}\n */\na.b.cprop = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#exportClassAsNamespaceProperty
#[test]
fn export_class_as_namespace_property() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "const a = {};\n/** @const */ a.b = {};\na.b.c = class {\n  constructor(d, e, f) {}\n};\ngoog.exportProperty(a.b, 'cprop', a.b.c)\n",
        "/**\n * @const\n * @suppress {const,duplicate}\n */\nvar a = {};\n/**\n * @const\n * @suppress {const,duplicate}\n */\na.b = {};\n/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @constructor\n */\na.b.cprop = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#exportQNameFunctionAsSymbolPlusPropertiesOnIt
#[test]
fn export_q_name_function_as_symbol_plus_properties_on_it() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\na.b = function(p1) {};\n/** @constructor */\na.b.c = function(d, e, f) {};\n/** @return {number} */\na.b.c.prototype.method = function(g, h, i) {};\ngoog.exportSymbol('a.b', a.b);\ngoog.exportProperty(a.b, 'c', a.b.c);\ngoog.exportProperty(a.b.c.prototype, 'exportedMethod', a.b.c.prototype.method);\n",
        "var a;\n/**\n * @param {?} p1\n * @return {undefined}\n */\na.b = function(p1) {\n};\n/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @constructor\n */\na.b.c = function(d, e, f) {\n};\n/**\n * @param {?} g\n * @param {?} h\n * @param {?} i\n * @return {number}\n */\na.b.c.prototype.exportedMethod = function(g, h, i) {\n};\n");
}

// port: ExternExportsPassTest#exportQNameClassAsSymbolPlusPropertiesOnIt
#[test]
fn export_q_name_class_as_symbol_plus_properties_on_it() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "const a = {};\na.b = class {\n  constructor(p1) {\n  }\n};\na.b.c = function(d, e, f) {};\na.b.prototype.c = function(g, h, i) {};\ngoog.exportSymbol('a.b', a.b);\ngoog.exportProperty(a.b, 'c', a.b.c);\ngoog.exportProperty(a.b.prototype, 'c', a.b.prototype.c);\n",
        "/**\n * @const\n * @suppress {const,duplicate}\n */\nvar a = {};\n/**\n * @param {?} p1\n * @constructor\n */\na.b = function(p1) {\n};\n/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @return {undefined}\n */\na.b.c = function(d, e, f) {\n};\n/**\n * @param {?} g\n * @param {?} h\n * @param {?} i\n * @return {undefined}\n */\na.b.prototype.c = function(g, h, i) {\n};\n");
}

// port: ExternExportsPassTest#symbolsRenamedInExportAreAlsoRenamedInExportedChildQNames
#[test]
fn symbols_renamed_in_export_are_also_renamed_in_exported_child_q_names() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\na.b = function(p1) {};\na.b.c = function(d, e, f) {};\na.b.prototype.c = function(g, h, i) {};\ngoog.exportSymbol('hello', a);\ngoog.exportProperty(a.b, 'c', a.b.c);\ngoog.exportProperty(a.b.prototype, 'c', a.b.prototype.c);\n",
        "/** @type {{b: function(?): undefined}} */\nvar hello = {};\nhello.b;\n/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @return {undefined}\n */\nhello.b.c = function(d, e, f) {\n};\n/**\n * @param {?} g\n * @param {?} h\n * @param {?} i\n * @return {undefined}\n */\nhello.b.prototype.c = function(g, h, i) {\n};\n");
}

// port: ExternExportsPassTest#exportingQnameAsSimpleNameAffectsLaterPropertyExports
#[test]
fn exporting_qname_as_simple_name_affects_later_property_exports() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\na.b = function(p1) {};\na.b.c = function(d, e, f) {};\na.b.prototype.c = function(g, h, i) {};\ngoog.exportSymbol('prefix', a.b);\ngoog.exportProperty(a.b, 'c', a.b.c);\n",
        "/**\n * @param {?} p1\n * @return {undefined}\n */\nvar prefix = function(p1) {\n};\n/**\n * @param {?} d\n * @param {?} e\n * @param {?} f\n * @return {undefined}\n */\nprefix.c = function(d, e, f) {\n};\n");
}

// port: ExternExportsPassTest#testExportNonStaticSymbol
#[test]
fn test_export_non_static_symbol() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\n/** @const */ a.b = {};\n/** @const */ var d = {};\na.b.c = d;\ngoog.exportSymbol('foobar', a.b.c)\n",
        "var foobar;\n");
}

// port: ExternExportsPassTest#testExportNonStaticSymbol2
#[test]
fn test_export_non_static_symbol2() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */ var a = {};\n/** @const */ a.b = {};\nvar d = function() {};\na.b.c = d;\ngoog.exportSymbol('foobar', a.b.c())\n",
        "var foobar;\n");
}

// port: ExternExportsPassTest#testExportNonexistentProperty
#[test]
fn test_export_nonexistent_property() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @fileoverview @suppress {missingProperties} */\n/** @const */ var a = {};\n/** @const */ a.b = {};\na.b.c = function(d, e, f) {};\ngoog.exportProperty(a.b, 'none', a.b.none)\n",
        "var a;\na.b;\na.b.none;\n");
}

// port: ExternExportsPassTest#testExportSymbolWithTypeAnnotation
#[test]
fn test_export_symbol_with_type_annotation() {
    let mut t = ExternExportsPassTest::new();

    t.compile_and_check(
        "var internalName;\n/**\n * @param {string} param1\n * @param {number} param2\n * @return {string}\n */\ninternalName = function(param1, param2) {\nreturn param1 + param2;\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {string} param1\n * @param {number} param2\n * @return {string}\n */\nvar externalName = function(param1, param2) {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolWithTemplateAnnotation
#[test]
fn test_export_symbol_with_template_annotation() {
    let mut t = ExternExportsPassTest::new();

    t.compile_and_check(
        "var internalName;\n/**\n * @param {T} param1\n * @return {T}\n * @template T\n */\ninternalName = function(param1) {\nreturn param1;\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {T} param1\n * @return {T}\n * @template T\n */\nvar externalName = function(param1) {\n};\n");
}

// port: ExternExportsPassTest#exportSubClassWithoutConstructor
#[test]
fn export_sub_class_without_constructor() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "class SuperClass {\n  /**\n   * @param {number} num\n   */\n   constructor(num) {\n   }\n}\ngoog.exportSymbol('SuperClass', SuperClass);\nclass SubClass extends SuperClass {\n}\ngoog.exportSymbol('SubClass', SubClass);\n",
        // The "a" parameter name is automatically generated
        "/**\n * @param {number} a\n * @extends {SuperClass}\n * @constructor\n */\nvar SubClass = function(a) {\n};\n/**\n * @param {number} num\n * @constructor\n */\nvar SuperClass = function(num) {\n};\n");
}

// port: ExternExportsPassTest#exportClassWithTemplateAnnotation
#[test]
fn export_class_with_template_annotation() {
    let mut t = ExternExportsPassTest::new();

    t.compile_and_check(
        "var internalName;\n/**\n * @template T\n */\ninternalName = class {\n  /**\n   * @param {T} param1\n   */\n  constructor(param1) {\n    /** @const */\n    this.data = param1;\n  }\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {T} param1\n * @constructor\n * @template T\n */\nvar externalName = function(param1) {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolWithMultipleTemplateAnnotation
#[test]
fn test_export_symbol_with_multiple_template_annotation() {
    let mut t = ExternExportsPassTest::new();

    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {K} param1\n * @return {V}\n * @template K,V\n */\ninternalName = function(param1) {\n  return /** @type {?} */ (param1);\n};\ngoog.exportSymbol('externalName', internalName);\n",
        "/**\n * @param {K} param1\n * @return {V}\n * @template K,V\n */\nvar externalName = function(param1) {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolWithoutTypeCheck
#[test]
fn test_export_symbol_without_type_check() {
    let mut t = ExternExportsPassTest::new();
    // ExternExportsPass should not emit annotations
    // if there is no type information available.
    t.harness.disable_type_check().unwrap();

    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {string} param1\n * @param {number} param2\n * @return {string}\n */\ninternalName = function(param1, param2) {\nreturn param1 + param2;\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "var externalName = function(param1, param2) {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolWithConstructor
#[test]
fn test_export_symbol_with_constructor() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @constructor\n */\ninternalName = function() {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @constructor\n */\nvar externalName = function() {\n};\n");
}

// port: ExternExportsPassTest#exportClass
#[test]
fn export_class() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\ninternalName = class {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @constructor\n */\nvar externalName = function() {\n};\n");
}

// port: ExternExportsPassTest#testNonNullTypes
#[test]
fn test_non_null_types() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/**\n * @constructor\n */\nfunction Foo() {}\ngoog.exportSymbol('Foo', Foo);\n/**\n * @param {!Foo} x\n * @return {!Foo}\n */\nFoo.f = function(x) { return x; };\ngoog.exportProperty(Foo, 'f', Foo.f);\n",
        "/**\n * @constructor\n */\nvar Foo = function() {\n};\n/**\n * @param {!Foo} x\n * @return {!Foo}\n */\nFoo.f = function(x) {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolWithConstructorWithoutTypeCheck
#[test]
fn test_export_symbol_with_constructor_without_type_check() {
    let mut t = ExternExportsPassTest::new();
    // For now, skipping type checking should prevent generating
    // annotations of any kind, so, e.g., @constructor is not preserved.
    // This is probably not ideal, but since JSDocInfo for functions is attached
    // to JSTypes and not Nodes (and no JSTypes are created when checkTypes
    // is false), we don't really have a choice.

    t.harness.disable_type_check().unwrap();

    t.compile_and_check(
        "var internalName;\n/**\n * @constructor\n */\ninternalName = function() {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "var externalName = function() {\n};\n");
}

// port: ExternExportsPassTest#testExportPrototypePropsWithoutConstructor
#[test]
fn test_export_prototype_props_without_constructor() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @constructor */\nx.Y = function() {};\nx.Y.prototype.z = function() {};\ngoog.exportProperty(x.Y.prototype, 'z', x.Y.prototype.z);\n",
        "var x;\nx.Y;\n/**\n * @return {undefined}\n */\nx.Y.prototype.z = function() {\n};\n");
}

// x.Y is present in the generated externs but lacks the @constructor annotation.
// port: ExternExportsPassTest#exportMethodButNotTheClass
#[test]
fn export_method_but_not_the_class() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "x.Y = class {\n  z() {};\n};\ngoog.exportProperty(x.Y.prototype, 'z', x.Y.prototype.z);\n",
        "var x;\nx.Y;\n/**\n * @return {undefined}\n */\nx.Y.prototype.z = function() {\n};\n",
    );
}

// port: ExternExportsPassTest#testExportFunctionWithOptionalArguments1
#[test]
fn test_export_function_with_optional_arguments1() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number=} a\n */\ninternalName = function(a) {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number=} a\n * @return {undefined}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#testExportFunctionWithOptionalArguments2
#[test]
fn test_export_function_with_optional_arguments2() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number=} a\n */\ninternalName = function(a) {\n  return /** @type {?} */ (6);\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number=} a\n * @return {?}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#testExportFunctionWithOptionalArguments3
#[test]
fn test_export_function_with_optional_arguments3() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number=} a\n */\ninternalName = function(a) {\n  return /** @type {?} */ (a);\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number=} a\n * @return {?}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#testExportFunctionWithVariableArguments
#[test]
fn test_export_function_with_variable_arguments() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {...number} a\n * @return {number}\n */\ninternalName = function(a) {\n  return 6;\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {...number} a\n * @return {number}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#exportArrowFunction
#[test]
fn export_arrow_function() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number} a\n * @return {number}\n */\ninternalName = (a) => a;\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number} a\n * @return {number}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#exportGeneratorFunction
#[test]
fn export_generator_function() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number} a\n * @return {!IteratorLike<number>}\n */\ninternalName = function *(a) { yield a; };\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number} a\n * @return {!IteratorLike<number,?,?>}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#exportAsyncGeneratorFunction
#[test]
fn export_async_generator_function() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number} a\n * @return {!AsyncGenerator<number>}\n */\ninternalName = async function *(a) { yield a; };\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number} a\n * @return {!AsyncGenerator<number,?,?>}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#exportAsyncFunction
#[test]
fn export_async_function() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number} a\n * @return {!Promise<number>}\n */\ninternalName = async function(a) { return a; };\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number} a\n * @return {!Promise<number>}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#exportFunctionWithDefaultParameter
#[test]
fn export_function_with_default_parameter() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {number=} a\n */\ninternalName = function(a = 1) {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {number=} a\n * @return {undefined}\n */\nvar externalName = function(a) {\n};\n");
}

// port: ExternExportsPassTest#exportFunctionWithRestParameter
#[test]
fn export_function_with_rest_parameter() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {...number} numbers\n */\ninternalName = function(...numbers) {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {...number} numbers\n * @return {undefined}\n */\nvar externalName = function(numbers) {\n};\n");
}

// port: ExternExportsPassTest#exportFunctionWithPatternParameters
#[test]
fn export_function_with_pattern_parameters() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\n/**\n * @param {{p: number, q: string}} options\n * @param {number} a\n * @param {!Array<number>} moreOptions\n */\ninternalName = function({p, q}, a, [x, y]) {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {{p: number, q: string}} b\n * @param {number} a\n * @param {!Array<number>} c\n * @return {undefined}\n */\nvar externalName = function(b, a, c) {\n};\n");
}

// port: ExternExportsPassTest#exportFunctionWithInlineDeclaredPatternParameters
#[test]
fn export_function_with_inline_declared_pattern_parameters() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var internalName;\n\ninternalName = function(\n    /** !Array<string> */ [p, q],\n    /** number */ a, // generated names start with 'a', make sure we don't conflict\n    /** !{x: number, y: string} */ {x, y}) {\n};\ngoog.exportSymbol('externalName', internalName)\n",
        // Param names are "b, a, c" because name 'a' was taken by a named parameter
        "/**\n * @param {!Array<string>} b\n * @param {number} a\n * @param {{x: number, y: string}} c\n * @return {undefined}\n */\nvar externalName = function(b, a, c) {\n};\n");
}

/** Enums are not currently handled. */
// port: ExternExportsPassTest#testExportEnum
#[test]
fn test_export_enum() {
    let mut t = ExternExportsPassTest::new();
    // We don't care what the values of the object properties are.
    // They're ignored by the type checker, and even if they weren't, it'd
    // be incomputable to get them correct in all cases
    // (think complex objects).
    t.compile_and_check(
        "/**\n * @enum {string}\n * @export\n */\nvar E = {A:'a', B:'b'};\ngoog.exportSymbol('E', E);\n",
        "/** @enum {string} */\nvar E = {A:1, B:2};\n");
}

// port: ExternExportsPassTest#testExportWithReferenceToEnum
#[test]
fn test_export_with_reference_to_enum() {
    let mut t = ExternExportsPassTest::new();
    let js = "/**\n * @enum {number}\n * @export\n */\nvar E = {A:1, B:2};\ngoog.exportSymbol('E', E);\n\n/**\n * @param {!E} e\n * @export\n */\nfunction f(e) {}\ngoog.exportSymbol('f', f);\n";
    let expected = "/** @enum {number} */\nvar E = {A:1, B:2};\n/**\n * @param {E} e\n * @return {undefined}\n */\nvar f = function(e) {\n};\n";

    // NOTE: The type should print {E} for the @param, but is not.
    t.compile_and_check(js, &expected.replace("{E}", "{number}"));
}

/**
 * If we export a property with "prototype" as a path component, there is no need to emit the
 * initializer for prototype because every namespace has one automatically.
 */
// port: ExternExportsPassTest#exportDontEmitPrototypePathPrefixForClassMethod
#[test]
fn export_dont_emit_prototype_path_prefix_for_class_method() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var Foo = class {\n  /**\n   * @return {number}\n   */\n  m() {return 6;};\n};\ngoog.exportSymbol('Foo', Foo);\ngoog.exportProperty(Foo.prototype, 'm', Foo.prototype.m);\n",
        "/**\n * @constructor\n */\nvar Foo = function() {\n};\n/**\n * @return {number}\n */\nFoo.prototype.m = function() {\n};\n");
}

/**
 * Test the workflow of creating an externs file for a library via the export pass and then using
 * that externs file in a client.
 *
 * <p>There should be no warnings in the client if the library includes type information for the
 * exported functions and the client uses them correctly.
 */
// port: ExternExportsPassTest#useExportsAsExternsWithClass
#[test]
fn use_exports_as_externs_with_class() {
    let mut t = ExternExportsPassTest::new();
    let library_source = "var InternalName = class {\n  /**\n   * @param {number} n\n   */\n  constructor(n) {\n  }\n};\ngoog.exportSymbol('ExternalName', InternalName)\n";

    let client_source = "var foo = new ExternalName(6);\n/**\n * @param {ExternalName} x\n */\nvar bar = function(x) {};\n";

    // Java runs the client compile inside the library compile's Postcondition; the Rust harness
    // is busy with the outer test(...) then, so the consumer keeps the generated externs and the
    // nested compileAndExportExterns runs right after the outer test returns (the Postcondition is
    // its last step).
    let generated = Rc::new(RefCell::new(None));
    let sink = Rc::clone(&generated);
    t.compile_and_export_externs_consumer(
        library_source,
        &minimal_externs(),
        Some(Box::new(move |generated_externs: String| {
            *sink.borrow_mut() = Some(generated_externs);
        })),
    );
    let generated_externs = generated.borrow_mut().take().unwrap();
    t.compile_and_export_externs_with_externs(
        client_source,
        &(minimal_externs() + &generated_externs),
    );
}

// port: ExternExportsPassTest#testDontWarnOnExportFunctionWithUnknownReturnType
#[test]
fn test_dont_warn_on_export_function_with_unknown_return_type() {
    let mut t = ExternExportsPassTest::new();
    let library_source = "var InternalName = function() {\n  return 6;\n};\ngoog.exportSymbol('ExternalName', InternalName)\n";

    t.compile_and_export_externs(library_source);
}

// port: ExternExportsPassTest#testDontWarnOnExportConstructorWithUnknownReturnType
#[test]
fn test_dont_warn_on_export_constructor_with_unknown_return_type() {
    let mut t = ExternExportsPassTest::new();
    let library_source = "/**\n * @constructor\n */\nvar InternalName = function() {\n};\ngoog.exportSymbol('ExternalName', InternalName)\n";

    t.compile_and_export_externs(library_source);
}

// port: ExternExportsPassTest#testTypedef
#[test]
fn test_typedef() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @typedef {{x: number, y: number}} */ var Coord;\n/**\n * @param {Coord} a\n * @export\n */\nvar fn = function(a) {};\ngoog.exportSymbol('fn', fn);\n",
        "/**\n * @param {{x: number, y: number}} a\n * @return {undefined}\n */\nvar fn = function(a) {\n};\n");
}

// port: ExternExportsPassTest#testExportParamWithNull
#[test]
fn test_export_param_with_null() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @param {string|null=} d */\nvar f = function(d) {};\ngoog.exportSymbol('foobar', f)\n",
        "/**\n * @param {(null|string)=} d\n * @return {undefined}\n */\nvar foobar = function(d) {\n};\n");
}

// port: ExternExportsPassTest#testExportConstructor
#[test]
fn test_export_constructor() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @constructor */ var a = function() {}; goog.exportSymbol('foobar', a)",
        "/**\n * @constructor\n */\nvar foobar = function() {\n};\n",
    );
}

// port: ExternExportsPassTest#exportEs5ClassHierarchy
#[test]
fn export_es5_class_hierarchy() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @constructor */\nfunction SuperClass() {}\ngoog.exportSymbol('Foo', SuperClass);\n\n/**\n * @constructor\n * @extends {SuperClass}\n */\nfunction SubClass() {}\ngoog.exportSymbol('Bar', SubClass);\n",
        // TODO(b/123352214): SuperClass should be called Foo in exported @extends annotation
        "/**\n * @extends {SuperClass}\n * @constructor\n */\nvar Bar = function() {\n};\n/**\n * @constructor\n */\nvar Foo = function() {\n};\n");
}

// port: ExternExportsPassTest#testExportLocalPropertyInConstructor
#[test]
fn test_export_local_property_in_constructor() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @constructor */function F() { /** @export */ this.x = 5;} goog.exportSymbol('F', F);",
        "/**\n * @constructor\n */\nvar F = function() {\n};\nF.prototype.x;\n",
    );
}

// port: ExternExportsPassTest#testExportLocalPropertyInConstructor2
#[test]
fn test_export_local_property_in_constructor2() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @constructor */function F() { /** @export */ this.x = 5;}\ngoog.exportSymbol('F', F);\ngoog.exportProperty(F.prototype, 'x', F.prototype.x);\n",
        "/**\n * @constructor\n */\nvar F = function() {\n};\nF.prototype.x;\n");
}

// port: ExternExportsPassTest#testExportLocalPropertyInConstructor3
#[test]
fn test_export_local_property_in_constructor3() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @constructor */function F() { /** @export */ this.x;} goog.exportSymbol('F', F);",
        "/**\n * @constructor\n */\nvar F = function() {\n};\nF.prototype.x;\n",
    );
}

// port: ExternExportsPassTest#testExportLocalPropertyInConstructor4
#[test]
fn test_export_local_property_in_constructor4() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @constructor */\nfunction F() { /** @export */ this.x = function(/** string */ x){};}\ngoog.exportSymbol('F', F);\n",
        "/**\n * @constructor\n */\nvar F = function() {\n};\nF.prototype.x;\n");
}

// port: ExternExportsPassTest#testExportLocalPropertyNotInConstructor
#[test]
fn test_export_local_property_not_in_constructor() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @this {?} */ function f() { /** @export */ this.x = 5;} goog.exportSymbol('f', f);",
        "/**\n * @return {undefined}\n */\nvar f = function() {\n};\n",
    );
}

// port: ExternExportsPassTest#testExportParamWithSymbolDefinedInFunction
#[test]
fn test_export_param_with_symbol_defined_in_function() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "var id = function() {return /** @type {?} */ ('id')};\nvar ft = function() {\n  var id;\n  return 1;\n};\ngoog.exportSymbol('id', id);\n",
        "/**\n * @return {?}\n */\nvar id = function() {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolWithFunctionDefinedAsFunction
#[test]
fn test_export_symbol_with_function_defined_as_function() {
    let mut t = ExternExportsPassTest::new();

    t.compile_and_check(
        "/**\n * @param {string} param1\n * @return {string}\n */\nfunction internalName(param1) {\n  return param1\n};\ngoog.exportSymbol('externalName', internalName)\n",
        "/**\n * @param {string} param1\n * @return {string}\n */\nvar externalName = function(param1) {\n};\n");
}

// port: ExternExportsPassTest#testExportSymbolWithFunctionAlias
#[test]
fn test_export_symbol_with_function_alias() {
    let mut t = ExternExportsPassTest::new();

    t.compile_and_check(
        "/**\n * @param {string} param1\n */\nvar y = function(param1) {\n};\n/**\n * @param {string} param1\n * @param {string} param2\n */\nvar x = function y(param1, param2) {\n};\ngoog.exportSymbol('externalName', y)\n",
        "/**\n * @param {string} param1\n * @return {undefined}\n */\nvar externalName = function(param1) {\n};\n");
}

// port: ExternExportsPassTest#testNamespaceDefinitionInExterns
#[test]
fn test_namespace_definition_in_externs() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @const */\nvar ns = {};\n/** @const */\nns.subns = {};\n/** @constructor */\nns.subns.Foo = function() {};\ngoog.exportSymbol('ns.subns.Foo', ns.subns.Foo);\n",
        "/**\n * @const\n * @suppress {const,duplicate}\n */\nvar ns = {};\n/**\n * @const\n * @suppress {const,duplicate}\n */\nns.subns = {};\n/**\n * @constructor\n */\nns.subns.Foo = function() {\n};\n");
}

// port: ExternExportsPassTest#testNullabilityInFunctionTypes
#[test]
fn test_nullability_in_function_types() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/**\n * @param {function(Object)} takesNullable\n * @param {function(!Object)} takesNonNullable\n */\nfunction x(takesNullable, takesNonNullable) {}\ngoog.exportSymbol('x', x);\n",
        "/**\n * @param {function((Object|null)): ?} takesNullable\n * @param {function(!Object): ?} takesNonNullable\n * @return {undefined}\n */\nvar x = function(takesNullable, takesNonNullable) {\n};\n");
}

// port: ExternExportsPassTest#testNullabilityInRecordTypes
#[test]
fn test_nullability_in_record_types() {
    let mut t = ExternExportsPassTest::new();
    t.compile_and_check(
        "/** @typedef {{ nonNullable: !Object, nullable: Object }} */\nvar foo;\n/** @param {foo} record */\nfunction x(record) {}\ngoog.exportSymbol('x', x);\n",
        "/**\n * @param {{nonNullable: !Object, nullable: (Object|null)}} record\n * @return {undefined}\n */\nvar x = function(record) {\n};\n");
}

// port: ExternExportsPassTest#testDontWarnOnExportFunctionWithUnknownParameterTypes
#[test]
fn test_dont_warn_on_export_function_with_unknown_parameter_types() {
    let mut t = ExternExportsPassTest::new();
    /* This source is missing types for the b and c parameters */
    let library_source = concat!(
        "/**\n",
        " * @param {number} a\n",
        " * @return {number}\n",
        " */\n",
        "var InternalName = function(a,b,c) {\n",
        "  return 6;\n",
        "};\n",
        "goog.exportSymbol('ExternalName', InternalName)\n",
    );

    t.compile_and_export_externs(library_source);
}
