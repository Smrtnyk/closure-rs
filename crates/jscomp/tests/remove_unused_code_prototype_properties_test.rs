/*
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/RemoveUnusedCodePrototypePropertiesTest.java.

//! Port of RemoveUnusedCodePrototypePropertiesTest.java: tests for `RemoveUnusedCode` that cover
//! removal of prototype properties and class properties. The tests run on the native
//! CompilerTestCase port of crates/testing.
use closure_jscomp::{
    check_level::CheckLevel, compiler_pass::CompilerPass, diagnostic_groups,
    remove_unused_code::RemoveUnusedCode,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    jscomp_api::{Compiler, CompilerOptions},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

/// Java's string concatenation `a + b + ...`.
fn cat(parts: &[&str]) -> String {
    parts.concat()
}

// port: RemoveUnusedCodePrototypePropertiesTest#EXTERNS
fn externs_js() -> String {
    cat(&[
        &CompilerTestCase::minimal_externs().unwrap().to_string(),
        r#"var window;
var Math = {};
Math.random = function() {};
function alert(x) {}
function externFunction() {}
externFunction.prototype.externPropName;
var mExtern;
mExtern.bExtern;
mExtern['cExtern'];

/** @const */
var goog = {};
goog.reflect.objectProperty = function(name) { };
"#,
    ])
}

struct RemoveUnusedCodePrototypePropertiesTest {
    ctx: Ctx,
    keep_locals: bool,
    keep_globals: bool,
}

impl CompilerTestCaseHooks for RemoveUnusedCodePrototypePropertiesTest {
    // port: RemoveUnusedCodePrototypePropertiesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let keep_locals = self.keep_locals;
        let keep_globals = self.keep_globals;
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                RemoveUnusedCode::builder(compiler)
                    .remove_local_vars(!keep_locals)
                    .remove_globals(!keep_globals)
                    .remove_unused_prototype_properties(true)
                    .build()
                    .process(compiler, externs, root);
            },
        )))))
    }

    // port: RemoveUnusedCodePrototypePropertiesTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options = harness.get_options()?;
        options.set_warning_level(diagnostic_groups::MODULE_LOAD.clone(), CheckLevel::OFF);
        Ok(options)
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// The JUnit instance: the CompilerTestCase harness and this test's fields.
struct Fixture {
    h: CompilerTestCase,
    t: RemoveUnusedCodePrototypePropertiesTest,
}

impl Fixture {
    // port: RemoveUnusedCodePrototypePropertiesTest#RemoveUnusedCodePrototypePropertiesTest
    // port: RemoveUnusedCodePrototypePropertiesTest#setUp
    fn new() -> Self {
        let mut h = CompilerTestCase::new(externs_js());
        h.set_up();
        // Allow testing of features that aren't fully supported for output yet.
        h.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        h.enable_normalize_expected_output().unwrap();
        h.enable_gather_extern_properties().unwrap();
        let t = RemoveUnusedCodePrototypePropertiesTest {
            ctx: Ctx::new(
                "RemoveUnusedCodePrototypePropertiesTest".into(),
                object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            keep_locals: true,
            keep_globals: false,
        };
        Self { h, t }
    }

    // port: CompilerTestCase#test(String,String)
    fn test(&mut self, js: &str, expected: &str) {
        self.h.test_strings(&mut self.t, js, expected).unwrap();
    }

    // port: CompilerTestCase#testSame(String)
    fn test_same(&mut self, js: &str) {
        self.h.test_same_string(&mut self.t, js).unwrap();
    }
}

// port: RemoveUnusedCodePrototypePropertiesTest#testClassPropertiesNotRemoved
#[test]
fn test_class_properties_not_removed() {
    let mut f = Fixture::new();
    f.t.keep_globals = true;
    // This whole test class runs with removeUnusedClassProperties disabled.
    f.test_same("/** @constructor */ function C() {} C.unused = 3;");
    f.test_same(
        "/** @constructor */ function C() {} Object.defineProperties(C, {unused: {value: 3}});",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testUnusedPrototypeFieldReference
#[test]
fn test_unused_prototype_field_reference() {
    let mut f = Fixture::new();
    // x is not actually read
    f.test(
        "function C() {} C.prototype.x; new C();",
        "function C() {}                new C();",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testUnusedReferenceToFieldWithGetter
#[test]
fn test_unused_reference_to_field_with_getter() {
    let mut f = Fixture::new();
    // Reference to a field with a getter should not be removed unless we know it has no side
    // effects.
    // TODO(bradfordcsmith): Implement removal for the no-side-effect cases.
    f.test_same("function C() {} C.prototype = { get x() {} }; new C().x");
    f.test_same("function C() {} C.prototype = { get x() { alert('x'); } }; new C().x");
    f.test_same("class C { get x() {} } new C().x;");
    f.test_same("class C { get x() { alert('x'); } } new C().x");
    f.test_same("let c = { get x() {} }; c.x;");
    f.test_same("let c = { get x() { alert('x'); } }; c.x;");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAnonymousPrototypePropertyRemoved
#[test]
fn test_anonymous_prototype_property_removed() {
    let mut f = Fixture::new();
    f.test(
        "({}.prototype.x = 5, externFunction())",
        "externFunction();",
    );
    f.test("({}).prototype.x = 5;", "");
    f.test("({}).prototype.x = externFunction();", "externFunction();");
    f.test("externFunction({}.prototype.x = 5);", "externFunction(5);");
    f.test("externFunction().prototype.x = 5;", "externFunction();");
    f.test(
        "externFunction().prototype.x = externFunction();",
        "externFunction(), externFunction();",
    );
    // make sure an expression with '.prototype' is traversed when it should be and not when it
    // shouldn't.
    // preserve format
    f.test(
        "function C() {} externFunction(C).prototype.x = 5;",
        "function C() {} externFunction(C);",
    );
    f.test("function C() {} ({ C: C }).prototype.x = 5;", "");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAnonymousPrototypePropertyNoRemoveSideEffect1
#[test]
fn test_anonymous_prototype_property_no_remove_side_effect1() {
    let mut f = Fixture::new();
    f.test(
        r#"function A() { // preserve format
  externFunction('me');
  return function(){}
}
A().prototype.foo = function() {};
"#,
        r#"function A() { // preserve format
  externFunction('me');
  return function(){}
}
A();
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAnonymousPrototypePropertyNoRemoveSideEffect2
#[test]
fn test_anonymous_prototype_property_no_remove_side_effect2() {
    let mut f = Fixture::new();
    f.test(
        "function A() { externFunction('me'); return function(){}; } A().prototype.foo++;",
        "function A() { externFunction('me'); return function(){}; } A();",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testIncPrototype
#[test]
fn test_inc_prototype() {
    let mut f = Fixture::new();
    f.test("function A() {} A.prototype.x = 1; A.prototype.x++;", "");
    f.test(
        "function A() {} A.prototype.x = 1; A.prototype.x++; new A();",
        "function A() {}                                     new A();",
    );
    f.test("externFunction().prototype.x++", "externFunction()");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testRenamePropertyFunctionTest
#[test]
fn test_rename_property_function_test() {
    let mut f = Fixture::new();
    f.test(
        r#"function C() {} // preserve formatting
C.prototype.unreferenced = function() {};
C.prototype.renamed = function() {};
JSCompiler_renameProperty('renamed');
new C();
"#,
        r#"function C() {} // preserve formatting
C.prototype.renamed = function() {};
JSCompiler_renameProperty('renamed');
new C();
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testNonPrototypePropertiesAreKept
#[test]
fn test_non_prototype_properties_are_kept() {
    let mut f = Fixture::new();
    // foo cannot be removed because it is called
    // x cannot be removed because a property is set on it and we don't know where it comes from
    // x.a cannot be removed because we don't know where x comes from.
    // x.prototype.b *can* be removed because we consider it safe to remove prototype properties
    // that have no references.
    f.test(
        "function foo(x) { x.a = 1; x.prototype.b = 2; }; foo({});",
        "function foo(x) { x.a = 1; }; foo({});",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAnalyzePrototypeProperties
#[test]
fn test_analyze_prototype_properties() {
    let mut f = Fixture::new();
    // Basic removal for prototype properties
    f.test(
        r#"function e(){}
e.prototype.a = function(){};
e.prototype.b = function(){};
var x = new e; x.a()
"#,
        r#"function e(){}
e.prototype.a = function(){};
var x = new e; x.a()
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testObjectLiteralPrototype
#[test]
fn test_object_literal_prototype() {
    let mut f = Fixture::new();
    f.test(
        "function e(){} e.prototype = {a: function(){}, b: function(){}}; var x = new e; x.a()",
        "function e(){} e.prototype = {a: function(){}                 }; var x = new e; x.a()",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testObjectLiteralPrototypeUnusedPropDefinitionWithSideEffects
#[test]
fn test_object_literal_prototype_unused_prop_definition_with_side_effects() {
    let mut f = Fixture::new();
    f.test(
        "function e(){} e.prototype = {a: alert('a'), b: function(){}}; new e;",
        "function e(){} e.prototype = {a: alert('a')                 }; new e;",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testPropertiesDefinedInExterns
#[test]
fn test_properties_defined_in_externs() {
    let mut f = Fixture::new();
    f.test(
        r#"function e(){}
e.prototype.a = function(){};
e.prototype.bExtern = function(){};
var x = new e;x.a()
"#,
        r#"function e(){}
e.prototype.a = function(){};
e.prototype.bExtern = function(){};
var x = new e; x.a()
"#,
    );
    f.test_same(
        r#"function e(){}
e.prototype = {a: function(){}, bExtern: function(){}};
var x = new e; x.a()
"#,
    );
    f.test_same(
        r#"class C {
  constructor() {}
  bExtern() {} // property name defined in externs.
}
new C();
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAliasing1
#[test]
fn test_aliasing1() {
    let mut f = Fixture::new();
    // Aliasing a property is not enough for it to count as used
    f.test(
        r#"function e(){}
e.prototype.method1 = function(){};
e.prototype.method2 = function(){};
// aliases
e.prototype.alias1 = e.prototype.method1;
e.prototype.alias2 = e.prototype.method2;
var x = new e; x.method1()
"#,
        r#"function e(){}
e.prototype.method1 = function(){};
var x = new e; x.method1()
"#,
    );
    // Using an alias should keep it
    f.test(
        r#"function e(){}
e.prototype.method1 = function(){};
e.prototype.method2 = function(){};
// aliases
e.prototype.alias1 = e.prototype.method1;
e.prototype.alias2 = e.prototype.method2;
var x=new e; x.alias1()
"#,
        r#"function e(){}
e.prototype.method1 = function(){};
e.prototype.alias1 = e.prototype.method1;
var x = new e; x.alias1()
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAliasing2
#[test]
fn test_aliasing2() {
    let mut f = Fixture::new();
    // Aliasing a property is not enough for it to count as used
    f.test(
        r#"function e(){}
e.prototype.method1 = function(){};
// aliases
e.prototype.alias1 = e.prototype.method1;
(new e).method1()
"#,
        r#"function e(){}
e.prototype.method1 = function(){};
(new e).method1()
"#,
    );
    // Using an alias should keep it
    f.test_same(
        r#"function e(){}
e.prototype.method1 = function(){};
// aliases
e.prototype.alias1 = e.prototype.method1;
(new e).alias1()
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAliasing3
#[test]
fn test_aliasing3() {
    let mut f = Fixture::new();
    // Aliasing a property is not enough for it to count as used
    f.test_same(
        r#"function e(){}
e.prototype.method1 = function(){};
e.prototype.method2 = function(){};
// aliases
e.prototype['alias1'] = e.prototype.method1;
e.prototype['alias2'] = e.prototype.method2;
new e;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAliasing4
#[test]
fn test_aliasing4() {
    let mut f = Fixture::new();
    // Aliasing a property is not enough for it to count as used
    f.test(
        r#"function e(){}
e.prototype['alias1'] = e.prototype.method1 = function(){};
e.prototype['alias2'] = e.prototype.method2 = function(){};
new e;
"#,
        r#"function e(){}
e.prototype['alias1'] = function(){};
e.prototype['alias2'] = function(){};
new e;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAliasing5
#[test]
fn test_aliasing5() {
    let mut f = Fixture::new();
    // An exported alias must preserved any referenced values in the
    // referenced function.
    f.test_same(
        r#"function e(){}
e.prototype.method1 = function(){this.method2()};
e.prototype.method2 = function(){};
// aliases
e.prototype['alias1'] = e.prototype.method1;
new e;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAliasing6
#[test]
fn test_aliasing6() {
    let mut f = Fixture::new();
    // An exported alias must preserved any referenced values in the
    // referenced function.
    f.test(
        r#"function e(){}
e.prototype.method1 = function(){this.method2()};
e.prototype.method2 = function(){};
// aliases
window['alias1'] = e.prototype.method1;
"#,
        r#"function e(){}
e.prototype.method1=function(){this.method2()};
e.prototype.method2=function(){};
window['alias1']=e.prototype.method1;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAliasing7
#[test]
fn test_aliasing7() {
    let mut f = Fixture::new();
    // An exported alias must preserved any referenced values in the
    // referenced function.
    f.test(
        r#"function e(){}
e.prototype['alias1'] = e.prototype.method1 = function(){this.method2()};
e.prototype.method2 = function(){};
new e;
"#,
        r#"function e(){}
e.prototype['alias1'] = function(){this.method2()};
e.prototype.method2 = function(){};
new e;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testExportedMethodsByNamingConvention
#[test]
fn test_exported_methods_by_naming_convention() {
    let mut f = Fixture::new();
    let class_and_its_method_aliased_as_extern = r#"function Foo() {}
Foo.prototype.method = function() {};
// not removed
Foo.prototype.unused = function() {};
// removed
var _externInstance = new Foo();
Foo.prototype._externMethod = Foo.prototype.method // aliased here
"#;
    let compiled = r#"function Foo(){}
Foo.prototype.method = function(){};
var _externInstance = new Foo;
Foo.prototype._externMethod = Foo.prototype.method
"#;
    f.test(class_and_its_method_aliased_as_extern, compiled);
}

// port: RemoveUnusedCodePrototypePropertiesTest#testExportedMethodsByNamingConventionAlwaysExported
#[test]
fn test_exported_methods_by_naming_convention_always_exported() {
    let mut f = Fixture::new();
    let class_and_its_method_aliased_as_extern = r#"function Foo() {}
Foo.prototype.method = function() {};
// not removed
Foo.prototype.unused = function() {};
// removed
var _externInstance = new Foo();
Foo.prototype._externMethod = Foo.prototype.method // aliased here
"#;
    let compiled = r#"function Foo(){}
Foo.prototype.method = function(){};
var _externInstance = new Foo;
Foo.prototype._externMethod = Foo.prototype.method
"#;
    f.test(class_and_its_method_aliased_as_extern, compiled);
}

// port: RemoveUnusedCodePrototypePropertiesTest#testExternMethodsFromExternsFile
#[test]
fn test_extern_methods_from_externs_file() {
    let mut f = Fixture::new();
    let class_and_its_method_aliased_as_extern = r#"function Foo() {}
Foo.prototype.bar_ = function() {};
// not removed
Foo.prototype.unused = function() {};
// removed
var instance = new Foo;
Foo.prototype.externPropName = Foo.prototype.bar_ // aliased here
"#;
    let compiled = r#"function Foo(){}
Foo.prototype.bar_ = function(){};
new Foo;
Foo.prototype.externPropName = Foo.prototype.bar_
"#;
    f.test(class_and_its_method_aliased_as_extern, compiled);
}

// port: RemoveUnusedCodePrototypePropertiesTest#testPropertyReferenceGraph
#[test]
fn test_property_reference_graph() {
    let mut f = Fixture::new();
    // test a prototype property graph that looks like so:
    // b -> a, c -> b, c -> a, d -> c, e -> a, e -> f
    let constructor = "function Foo() {}";
    let def_a = "Foo.prototype.a = function() { Foo.superClass_.a.call(this); };";
    let def_b = "Foo.prototype.b = function() { this.a(); };";
    let def_c = r#"Foo.prototype.c = function() {
Foo.superClass_.c.call(this); this.b(); this.a(); };
"#;
    let def_d = "Foo.prototype.d = function() { this.c(); };";
    let def_e = "Foo.prototype.e = function() { this.a(); this.f(); };";
    let def_f = "Foo.prototype.f = function() { };";
    let full_class_def = cat(&[constructor, def_a, def_b, def_c, def_d, def_e, def_f]);
    // ensure that all prototypes are compiled out if none are used
    f.test(&full_class_def, "");
    // make sure that the right prototypes are called for each use
    let call_a = "(new Foo()).a();";
    let call_b = "(new Foo()).b();";
    let call_c = "(new Foo()).c();";
    let call_d = "(new Foo()).d();";
    let call_e = "(new Foo()).e();";
    let call_f = "(new Foo()).f();";
    f.test(
        &cat(&[&full_class_def, call_a]),
        &cat(&[constructor, def_a, call_a]),
    );
    f.test(
        &cat(&[&full_class_def, call_b]),
        &cat(&[constructor, def_a, def_b, call_b]),
    );
    f.test(
        &cat(&[&full_class_def, call_c]),
        &cat(&[constructor, def_a, def_b, def_c, call_c]),
    );
    f.test(
        &cat(&[&full_class_def, call_d]),
        &cat(&[constructor, def_a, def_b, def_c, def_d, call_d]),
    );
    f.test(
        &cat(&[&full_class_def, call_e]),
        &cat(&[constructor, def_a, def_e, def_f, call_e]),
    );
    f.test(
        &cat(&[&full_class_def, call_f]),
        &cat(&[constructor, def_f, call_f]),
    );
    f.test(
        &cat(&[&full_class_def, call_a, call_c]),
        &cat(&[constructor, def_a, def_b, def_c, call_a, call_c]),
    );
    f.test(
        &cat(&[&full_class_def, call_b, call_c]),
        &cat(&[constructor, def_a, def_b, def_c, call_b, call_c]),
    );
    f.test(
        &cat(&[&full_class_def, call_a, call_b, call_c]),
        &cat(&[constructor, def_a, def_b, def_c, call_a, call_b, call_c]),
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testPropertiesDefinedWithGetElem
#[test]
fn test_properties_defined_with_get_elem() {
    let mut f = Fixture::new();
    f.test_same("function Foo() {} Foo.prototype['elem'] = function() {}; new Foo;");
    f.test_same("function Foo() {} Foo.prototype[1 + 1] = function() {}; new Foo;");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testQuotedProperties
#[test]
fn test_quoted_properties() {
    let mut f = Fixture::new();
    // Basic removal for prototype replacement
    f.test_same("function e(){} e.prototype = {'a': function(){}, 'b': function(){}}; new e;");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testNeverRemoveImplicitlyUsedProperties
#[test]
fn test_never_remove_implicitly_used_properties() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}
Foo.prototype.length = 3;
Foo.prototype.toString = function() { return 'Foo'; };
Foo.prototype.valueOf = function() { return 'Foo'; };
new Foo;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testPropertyDefinedInBranch
#[test]
fn test_property_defined_in_branch() {
    let mut f = Fixture::new();
    f.test(
        "function Foo() {} if (true) Foo.prototype.baz = function() {};",
        "if (true);",
    );
    f.test(
        "function Foo() {} while (true) Foo.prototype.baz = function() {};",
        "while (true);",
    );
    f.test(
        "function Foo() {} for (;;) Foo.prototype.baz = function() {};",
        "for (;;);",
    );
    f.test(
        "function Foo() {} do Foo.prototype.baz = function() {}; while(true);",
        "do; while(true);",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testUsingAnonymousObjectsToDefeatRemoval
#[test]
fn test_using_anonymous_objects_to_defeat_removal() {
    let mut f = Fixture::new();
    f.test(
        "function Foo() {} Foo.prototype.baz = 3; new Foo;",
        "function Foo() {} new Foo;",
    );
    f.test_same("function Foo() {} Foo.prototype.baz = 3; new Foo; var x = {}; x.baz;");
    f.test_same("function Foo() {} Foo.prototype.baz = 3; new Foo; var x = {baz: 5}; x;");
    // quoted properties still prevent removal
    f.test_same("function Foo() {} Foo.prototype.baz = 3; new Foo; var x = {'baz': 5}; x;");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph
#[test]
fn test_global_functions_in_graph() {
    let mut f = Fixture::new();
    f.test(
        r#"var x = function() { (new Foo).baz(); };
var y = function() { x(); };
function Foo() {}
Foo.prototype.baz = function() { y(); };
"#,
        "",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph2
#[test]
fn test_global_functions_in_graph2() {
    let mut f = Fixture::new();
    f.test(
        r#"var x = function() { (new Foo).baz(); };
var y = function() { x(); };
function Foo() { this.baz(); }
Foo.prototype.baz = function() { y(); };
"#,
        "",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph3
#[test]
fn test_global_functions_in_graph3() {
    let mut f = Fixture::new();
    f.test(
        r#"var x = function() { (new Foo).baz(); };
var y = function() { x(); };
function Foo() { this.baz(); }
Foo.prototype.baz = function() { x(); };
"#,
        "",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph4
#[test]
fn test_global_functions_in_graph4() {
    let mut f = Fixture::new();
    f.test(
        r#"var x = function() { (new Foo).baz(); };
var y = function() { x(); };
function Foo() { Foo.prototype.baz = function() { y(); }; }
"#,
        "",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph5
#[test]
fn test_global_functions_in_graph5() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo() {}
Foo.prototype.methodA = function() {};
function x() { (new Foo).methodA(); }
Foo.prototype.methodB = function() { x(); };
"#,
        "",
    );
    f.t.keep_globals = true;
    f.test(
        r#"function Foo() {}
Foo.prototype.methodA = function() {};
function x() { (new Foo).methodA(); }
Foo.prototype.methodB = function() { x(); };
"#,
        r#"function Foo() {}
Foo.prototype.methodA = function() {};
function x() { (new Foo).methodA(); }
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph6
#[test]
fn test_global_functions_in_graph6() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}
Foo.prototype.methodA = function() {};
function x() { (new Foo).methodA(); }
Foo.prototype.methodB = function() { x(); };
(new Foo).methodB();
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph7
#[test]
fn test_global_functions_in_graph7() {
    let mut f = Fixture::new();
    f.t.keep_globals = true;
    f.test_same("function Foo() {} Foo.prototype.methodA = function() {}; this.methodA();");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGlobalFunctionsInGraph8
#[test]
fn test_global_functions_in_graph8() {
    let mut f = Fixture::new();
    f.test(
        r#"let x = function() { (new Foo).baz(); };
const y = function() { x(); };
function Foo() { Foo.prototype.baz = function() { y(); }; }
"#,
        "",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGetterBaseline
#[test]
fn test_getter_baseline() {
    let mut f = Fixture::new();
    f.t.keep_globals = true;
    f.test(
        r#"function Foo() {}
Foo.prototype = {
  methodA: function() {},
  methodB: function() { x(); }
};
function x() { (new Foo).methodA(); }
"#,
        r#"function Foo() {}
Foo.prototype = {
  methodA: function() {}
};
function x() { (new Foo).methodA(); }
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGetter1
#[test]
fn test_getter1() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo() {}
Foo.prototype = {
  get methodA() {},
  get methodB() { x(); }
};
function x() { (new Foo).methodA; }
new Foo();
"#,
        r#"function Foo() {}
// x() and all methods of Foo removed.
Foo.prototype = {};
new Foo();
"#,
    );
    f.t.keep_globals = true;
    f.test(
        r#"function Foo() {}
Foo.prototype = {
  get methodA() {},
  get methodB() { x(); }
};
function x() { (new Foo).methodA; }
"#,
        r#"function Foo() {}
Foo.prototype = {
  get methodA() {}
};
// x() keeps methodA alive
function x() { (new Foo).methodA; }
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testGetter2
#[test]
fn test_getter2() {
    let mut f = Fixture::new();
    f.t.keep_globals = true;
    f.test(
        r#"function Foo() {}
Foo.prototype = {
  get methodA() {},
  set methodA(a) {},
  get methodB() { x(); },
  set methodB(a) { x(); }
};
function x() { (new Foo).methodA; }
"#,
        r#"function Foo() {}
Foo.prototype = {
  get methodA() {},
  set methodA(a) {}
};
function x() { (new Foo).methodA; }
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testHook1
#[test]
fn test_hook1() {
    let mut f = Fixture::new();
    f.test(
        r#"/** @constructor */ function Foo() {}
Foo.prototype.method1 =
    Math.random()
        ? function() { this.method2(); }
        : function() { this.method3(); };
Foo.prototype.method2 = function() {};
Foo.prototype.method3 = function() {};
"#,
        "",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testHook2
#[test]
fn test_hook2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"/** @constructor */ function Foo() {}
Foo.prototype.method1 =
    Math.random()
        ? function() { this.method2(); }
        : function() { this.method3(); };
Foo.prototype.method2 = function() {};
Foo.prototype.method3 = function() {};
(new Foo()).method1();
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testDestructuringProperty
#[test]
fn test_destructuring_property() {
    let mut f = Fixture::new();
    // Makes the cases below shorter because we don't have to add references
    // to globals to keep them around and just test prototype property removal.
    f.t.keep_globals = true;
    f.test(
        "function Foo() {} Foo.prototype.a = function() {}; var {} = new Foo();",
        "function Foo() {} var {} = new Foo();",
    );
    f.test(
        r#"function Foo() {}
Foo.prototype.a = function() {};
Foo.prototype.b = function() {}
var {a} = new Foo();
"#,
        r#"function Foo() {}
Foo.prototype.a = function() {};
var {a} = new Foo();
"#,
    );
    f.test(
        r#"function Foo() {}
Foo.prototype.a = function() {};
Foo.prototype.b = function() {}
var {a:x} = new Foo();
"#,
        r#"function Foo() {}
Foo.prototype.a = function() {};
var {a:x} = new Foo();
"#,
    );
    f.test_same(
        r#"function Foo() {}
Foo.prototype.a = function() {};
Foo.prototype.b = function() {}
var {a, b} = new Foo();
"#,
    );
    f.test_same(
        r#"function Foo() {}
Foo.prototype.a = function() {};
Foo.prototype.b = function() {}
var {a:x, b:y} = new Foo();
"#,
    );
    f.test_same(
        r#"function Foo() {} // preserve newlines
Foo.prototype.a = function() {};
let x;
({a:x} = new Foo());
"#,
    );
    f.test_same(
        r#"function Foo() {}
Foo.prototype.a = function() {};
function f({a:x}) { x; }; f(new Foo());
"#,
    );
    f.test_same(
        r#"function Foo() {}
Foo.prototype.a = function() {};
var {a : x = 3} = new Foo();
"#,
    );
    f.test(
        r#"function Foo() {}
Foo.prototype.a = function() {};
Foo.prototype.b = function() {}
var {a : a = 3} = new Foo();
"#,
        r#"function Foo() {}
Foo.prototype.a = function() {};
var {a : a = 3} = new Foo();
"#,
    );
    f.test_same(
        r#"function Foo() {}
Foo.prototype.a = function() {};
let { a : [b, c, d] } = new Foo();
"#,
    );
    f.test_same(
        r#"function Foo() {}
Foo.prototype.a = function() {};
const { a : { b : { c : d = '' }}} = new Foo();
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testDestructuringRest
#[test]
fn test_destructuring_rest() {
    let mut f = Fixture::new();
    // Makes the cases below shorter because we don't have to add references
    // to globals to keep them around and just test prototype property removal.
    f.t.keep_globals = true;
    f.test_same(
        r#"function Foo() {}
Foo.prototype.a = function() {};
({ ...new Foo().a.b } = 0);
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testOptionalGetPropPreventsRemoval
#[test]
fn test_optional_get_prop_prevents_removal() {
    let mut f = Fixture::new();
    f.test(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  optChainGetPropRef() {}
  optChainCallRef() {}
  unreferenced() {}
}
var c = new C;
c?.optChainGetPropRef()
c.optChainCallRef?.()
// no call to unreferenced()
"#,
        r#"class C {
  constructor() {
    this.x = 1;
  }
  optChainGetPropRef() {} // kept
  optChainCallRef() {} // kept
// unreferenced() removed
}
var c = new C;
c?.optChainGetPropRef()
c.optChainCallRef?.()
// no call to unreferenced()
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testEs6Class
#[test]
fn test_es6_class() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  constructor() { // constructor is not removable
    this.x = 1;
  }
}
new C();
"#,
    );
    f.test(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  foo() {}
}
var c = new C
"#,
        r#"class C {
  constructor() { // constructor is not removable
    this.x = 1;
  }
}
new C();
"#,
    );
    f.test_same(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  foo() {}
}
var c = new C
c.foo()
"#,
    );
    f.test(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  static foo() {}
}
new C;
"#,
        r#"class C {
  constructor() { // constructor is not removable
    this.x = 1;
  }
// TODO(b/139319709): Remove this. static method removal is disabled.
  static foo() {}
}
new C();
"#,
    );
    f.test(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  get foo() {}
  set foo(val) {}
}
var c = new C
"#,
        "class C { constructor() { this.x = 1; } } new C",
    );
    f.test_same(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  get foo() {}
  set foo(val) {}
}
var c = new C;
c.foo = 3;
"#,
    );
    f.test_same(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  get foo() {}
  set foo(val) {}
}
var c = new C;
c.foo;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testEs6Extends
#[test]
fn test_es6_extends() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  constructor() {
    this.x = 1;
  }
}
class D extends C {
  constructor() {}
}
new D();
"#,
    );
    f.test_same(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  foo() {}
}
class D extends C {
  constructor() {}
  foo() {
     return super.foo()
  }
}
var d = new D
d.foo()
"#,
    );
    f.test(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  foo() {}
}
class D extends C {
  constructor() {}
  foo() {
     return super.foo()
  }
}
var d = new D;
"#,
        r#"class C {
  constructor() {
    this.x = 1;
  }
}
class D extends C {
  constructor() {}
}
new D;
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testAnonClasses
#[test]
fn test_anon_classes() {
    let mut f = Fixture::new();
    // Make sure class expression names are removed.
    f.t.keep_locals = false;
    f.test(
        r#"var C = class InnerC {
  constructor() {
    this.x = 1;
  }
  foo() {}
};
new C;
"#,
        "var C = class { constructor() { this.x = 1; } }; new C;",
    );
    f.test_same(
        r#"var C = class {
  constructor() {
    this.x = 1;
  }
  foo() {}
}
var c = new C()
c.foo()
"#,
    );
    f.test(
        r#"var C = class {}
C.D = class {
  constructor() {
    this.x = 1;
  }
  foo() {}
}
new C.D();
"#,
        r#"var C = class {}
C.D = class{
  constructor() {
    this.x = 1;
  }
}
new C.D();
"#,
    );
    f.test(
        "externFunction(class C { constructor() { } externPropName() { } })",
        "externFunction(class   { constructor() { } externPropName() { } })",
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testBaseClassExpressionHasSideEffects
#[test]
fn test_base_class_expression_has_side_effects() {
    let mut f = Fixture::new();
    // Make sure names are removed from class expressions.
    f.t.keep_locals = false;
    f.test_same(
        r#"function getBaseClass() { return class {}; }
class C extends getBaseClass() {}
"#,
    );
    f.test(
        r#"function getBaseClass() { return class {}; }
const C = class InnerC extends getBaseClass() {};
"#,
        r#"function getBaseClass() { return class {}; }
(class extends getBaseClass() {})
"#,
    );
    f.test(
        r#"function getBaseClass() { return class {}; }
let C;
C = class InnerC extends getBaseClass() {}
"#,
        r#"function getBaseClass() { return class {}; }
(class extends getBaseClass() {})
"#,
    );
    f.test(
        r#"function getBaseClass() { return class {}; }
externFunction(class InnerC extends getBaseClass() {})
"#,
        r#"function getBaseClass() { return class {}; }
externFunction(class extends getBaseClass() {})
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testModules
#[test]
fn test_modules() {
    let mut f = Fixture::new();
    f.test_same("export default function(){}");
    f.test_same("export class C {};");
    f.test_same("class Bar {} export {Bar}");
    f.test_same("import { square, diag } from '/lib';");
    f.test_same("import * as lib from '/lib';");
}

// port: RemoveUnusedCodePrototypePropertiesTest#testReflection_reflectProperty_pinsReflectedName
#[test]
fn test_reflection_reflect_property_pins_reflected_name() {
    let mut f = Fixture::new();
    f.test_same(
        r#"/** @constructor */
function Foo() {}
Foo.prototype.handle = function(x, y) { alert(y); };

goog.reflect.objectProperty('handle');
alert(new Foo());
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testReflection_reflectProperty_onlyPinsReflectedName
#[test]
fn test_reflection_reflect_property_only_pins_reflected_name() {
    let mut f = Fixture::new();
    f.test(
        r#"/** @constructor */
function Foo() {}
Foo.prototype.handle = function(x, y) { alert(y); };

goog.reflect.objectProperty('not_handle');
alert(new Foo());
"#,
        r#"/** @constructor */
function Foo() {}

goog.reflect.objectProperty('not_handle');
alert(new Foo());
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testReflection_reflectProperty_onlyPinsReflectedName_whenNameMissing
#[test]
fn test_reflection_reflect_property_only_pins_reflected_name_when_name_missing() {
    let mut f = Fixture::new();
    f.test(
        r#"/** @constructor */
function Foo() {}
Foo.prototype.handle = function(x, y) { alert(y); };

goog.reflect.objectProperty();
alert(new Foo());
"#,
        r#"/** @constructor */
function Foo() {}

goog.reflect.objectProperty();
alert(new Foo());
"#,
    );
}

// port: RemoveUnusedCodePrototypePropertiesTest#testPureOrBreakMyCode
#[test]
fn test_pure_or_break_my_code() {
    let mut f = Fixture::new();
    f.test(
        r#"/** @constructor */
function Foo() {}
Foo.prototype.used = /** @pureOrBreakMyCode */(alert());
Foo.prototype.unused = /** @pureOrBreakMyCode */(alert());
function foo() {
  return new Foo().used;
}
foo();
"#,
        r#"/** @constructor */
function Foo() {}
Foo.prototype.used = /** @pureOrBreakMyCode */(alert());
function foo() {
  return new Foo().used;
}
foo();
"#,
    );
}
