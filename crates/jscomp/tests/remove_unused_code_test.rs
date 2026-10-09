/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/RemoveUnusedCodeTest.java.

//! Port of RemoveUnusedCodeTest.java: test removal of unused global and local variables.
//!
//! These test cases for `RemoveUnusedCode` focus on removal of variables and do not enable the
//! options for removing individual properties, fields, or methods. See the other
//! `RemoveUnusedCode*Test` classes for that kind of code removal. The tests run on the native
//! CompilerTestCase port of crates/testing. The 5 `*_typed` tests call `enableTypeCheck()` (real
//! TypeCheck, type-check).
use closure_jscomp::{compiler_pass::CompilerPass, remove_unused_code::RemoveUnusedCode};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::Compiler,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    testing::{
        js_chunk_graph_builder::JSChunkGraphBuilder, test_externs_builder::TestExternsBuilder,
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

// port: RemoveUnusedCodeTest#RemoveUnusedCodeTest (the externs used in the test cases)
const EXTERNS: &str = r#"var undefined;
var goog = {};
/** @const {!Global} */ goog.global;
goog.reflect = {};
goog.reflect.object = function(obj, propertiesObj) {};
goog.reflect.objectProperty = function(prop, obj) {};
goog.weakUsage = function(nameArg) {};
function goog$inherits(subClass, superClass) {}
function valueType$mixin(dstPrototype, srcPrototype, flags, ...args) {}
function alert() {}
function use() {}
function externFunction() {}
var externVar;
function validate() {}
var userInput;
function gen() {}
var window;
var console = {};
console.log = function(var_args) {};
/** @constructor @return {!Array} */ function Array(/** ...* */ var_args) {}
/** @constructor @return {string} */ function String(/** *= */ opt_arg) {}
/** @constructor */ function Set() {}
/** @constructor */ function WeakMap() {}
"#;

// port: RemoveUnusedCodeTest#JSCOMP_POLYFILL
const JSCOMP_POLYFILL: &str = r#"var $jscomp = {};
$jscomp.polyfill = function(
    /** string */ name, /** Function */ func, /** string */ from, /** string */ to) {};
$jscomp.polyfillTypedArrayMethod = function(
    /** string */ name, /** Function */ func, /** string */ from, /** string */ to) {};
"#;

// port: RemoveUnusedCodeTest#JSCOMP_PATCH
const JSCOMP_PATCH: &str = r#"var $jscomp = {};
$jscomp.patch = function(/** string */ name, /** Function */ func) {};
"#;

struct RemoveUnusedCodeTest {
    ctx: Ctx,
    remove_global: bool,
    preserve_function_expression_names: bool,
}

impl CompilerTestCaseHooks for RemoveUnusedCodeTest {
    // port: RemoveUnusedCodeTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let remove_global = self.remove_global;
        let preserve_function_expression_names = self.preserve_function_expression_names;
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                RemoveUnusedCode::builder(compiler)
                    .remove_local_vars(true)
                    .remove_globals(remove_global)
                    .remove_unused_polyfills(true)
                    .preserve_function_expression_names(preserve_function_expression_names)
                    .build()
                    .process(compiler, externs, root);
            },
        )))))
    }

    // RemoveUnusedCodeTest#getOptions returns super.getOptions(): the default hook.

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// The JUnit instance: the CompilerTestCase harness and this test's fields.
struct Fixture {
    h: CompilerTestCase,
    t: RemoveUnusedCodeTest,
}

impl Fixture {
    // port: RemoveUnusedCodeTest#RemoveUnusedCodeTest
    // port: RemoveUnusedCodeTest#setUp
    fn new() -> Self {
        // Set up externs to be used in the test cases.
        let mut h = CompilerTestCase::new(EXTERNS);
        h.set_up();
        // Allow testing of features that aren't supported for output yet.
        h.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        h.enable_normalize_expected_output().unwrap();
        h.enable_gather_extern_properties().unwrap();
        let t = RemoveUnusedCodeTest {
            ctx: Ctx::new(
                "RemoveUnusedCodeTest".into(),
                object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            remove_global: true,
            preserve_function_expression_names: false,
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

    // port: CompilerTestCase#test(TestPart...)
    fn test_parts(&mut self, parts: Vec<TestPart>) {
        self.h.test(&mut self.t, parts).unwrap();
    }

    // port: CompilerTestCase#testSame(TestPart...)
    fn test_same_parts(&mut self, parts: Vec<TestPart>) {
        self.h.test_same(&mut self.t, parts).unwrap();
    }
}

/// Handles the boilerplate for testing whether a polyfill is removed or not.
// port: RemoveUnusedCodeTest.PolyfillRemovalTester
struct PolyfillRemovalTester {
    externs: Vec<String>,
    polyfills: IndexSet<String>,

    // Input source code, which will be prefixed with the polyfills.
    // A value of `null` just means this hasn't been set yet.
    input_source: Option<String>,

    // Expected output source code, which will be prefixed with the output version of each polyfill.
    // A value of `null` means this value hasn't been set yet or needs to be reset because
    // inputSource has changed.
    expected_source: Option<String>,

    // Set of polyfills that are expected to be removed by RemoveUnusedCode.
    // A value of `null` indicates that no expectation has been set since the last time inputSource
    // was modified.
    polyfills_expected_to_be_removed: Option<IndexSet<String>>,
}

/// Java's `String.valueOf` of a nullable string (`Stream.of(null)` joins as "null").
fn value_of(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("null")
}

impl PolyfillRemovalTester {
    fn new() -> Self {
        Self {
            externs: Vec::new(),
            polyfills: IndexSet::<_>::default(),
            input_source: None,
            expected_source: None,
            polyfills_expected_to_be_removed: Some(IndexSet::<_>::default()),
        }
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#addExterns
    fn add_externs(&mut self, more_externs: &str) -> &mut Self {
        self.externs.push(more_externs.to_string());
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#addPolyfill
    fn add_polyfill(&mut self, polyfill: &str) -> &mut Self {
        assert!(
            !self.polyfills.contains(polyfill),
            "duplicate polyfill added: >{polyfill}<"
        );
        self.polyfills.insert(polyfill.to_string());
        // force update of expectations whenever the polyfills list is updated
        self.polyfills_expected_to_be_removed = None;
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#inputSource
    fn input_source(&mut self, input_source: &str) -> &mut Self {
        self.input_source = Some(input_source.to_string());
        // Force updates for the expected output source and polyfills
        self.expected_source = None;
        self.polyfills_expected_to_be_removed = None;
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#expectSourceUnchanged
    fn expect_source_unchanged(&mut self) -> &mut Self {
        assert!(self.input_source.is_some(), "checkNotNull(inputSource)");
        self.expected_source = self.input_source.clone();
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#expectSource
    fn expect_source(&mut self, expected_source: &str) -> &mut Self {
        self.expected_source = Some(expected_source.to_string());
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#expectNoPolyfillsRemoved
    fn expect_no_polyfills_removed(&mut self) -> &mut Self {
        self.polyfills_expected_to_be_removed = Some(IndexSet::<_>::default());
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#expectPolyfillsRemoved
    fn expect_polyfills_removed(&mut self, removed_polyfills: &[&str]) -> &mut Self {
        for polyfill_to_remove in removed_polyfills {
            self.expect_polyfill_removed(polyfill_to_remove);
        }
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#expectPolyfillRemoved
    fn expect_polyfill_removed(&mut self, polyfill: &str) -> &mut Self {
        assert!(
            self.polyfills.contains(polyfill),
            "non-existent polyfill cannot be removed: >{polyfill}<"
        );
        match &mut self.polyfills_expected_to_be_removed {
            None => self.polyfills_expected_to_be_removed = Some(IndexSet::<_>::default()),
            Some(removed) => assert!(
                !removed.contains(polyfill),
                "polyfill cannot be removed twice: >{polyfill}<"
            ),
        }
        self.polyfills_expected_to_be_removed
            .as_mut()
            .unwrap()
            .insert(polyfill.to_string());
        self
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#test
    fn test(&mut self, f: &mut Fixture) {
        let externs_text = self.externs.join("\n");

        let input_source_text = self
            .polyfills
            .iter()
            .map(String::as_str)
            .chain([value_of(&self.input_source)])
            .collect::<Vec<_>>()
            .join("\n");

        let polyfills_expected_to_be_removed = self
            .polyfills_expected_to_be_removed
            .as_ref()
            .expect("unset/undefined polyfill removal behavior");
        let expected_polyfills = self
            .polyfills
            .iter()
            .filter(|polyfill| !polyfills_expected_to_be_removed.contains(*polyfill))
            .map(String::as_str);
        let expected_source_text = expected_polyfills
            .chain([value_of(&self.expected_source)])
            .collect::<Vec<_>>()
            .join("\n");

        f.test_parts(vec![
            TestPart::Externs(CompilerTestCase::externs(externs_text)),
            TestPart::Sources(CompilerTestCase::srcs(input_source_text)),
            TestPart::Expected(CompilerTestCase::expected(expected_source_text)),
        ]);
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#expectNoRemovalTest
    fn expect_no_removal_test(&mut self, f: &mut Fixture, src: &str) {
        /* no polyfills removed */
        self.input_source(src) //
            .expect_source_unchanged()
            .expect_no_polyfills_removed()
            .test(f);
    }

    // port: RemoveUnusedCodeTest.PolyfillRemovalTester#expectPolyfillsRemovedTest
    fn expect_polyfills_removed_test(
        &mut self,
        f: &mut Fixture,
        src: &str,
        removed_polyfills: &[&str],
    ) {
        self.input_source(src) //
            .expect_source_unchanged()
            .expect_polyfills_removed(removed_polyfills)
            .test(f);
    }
}

// port: RemoveUnusedCodeTest#testDoNotRemoveUnusedVarDeclaredInObjectPatternUsingRest
#[test]
fn test_do_not_remove_unused_var_declared_in_object_pattern_using_rest() {
    let mut f = Fixture::new();
    f.test_same(
        r#"const data = {
  hello: 'abc',
  world: 'def',
}

const {
// If `hello` were removed, then `rest` could end up getting a `hello` property.
  hello,
  ...rest
} = data

console.log(rest);
"#,
    );
}

// port: RemoveUnusedCodeTest#testUnusedPrototypeFieldReference
#[test]
fn test_unused_prototype_field_reference() {
    let mut f = Fixture::new();
    // Simply mentioning a prototype property without using it doesn't count as a reference.
    f.test("function C() {} C.prototype.x;", "");
}

// port: RemoveUnusedCodeTest#testLeaveZeroBehind
#[test]
fn test_leave_zero_behind() {
    let mut f = Fixture::new();
    // We don't need the assignment or the assigned value, but we need to keep the AST valid.
    // preserve formatting
    f.test(
        "var x; (x = 15, externFunction())",
        "                externFunction()",
    );
}

// port: RemoveUnusedCodeTest#testRemoveInBlock
#[test]
fn test_remove_in_block() {
    let mut f = Fixture::new();
    f.test(
        r#"if (true) {
  if (true) {
    var foo = function() {};
  }
}
"#,
        r#"if (true) {
  if (true) {
  }
}
"#,
    );
    f.test("if (true) { let foo = function() {} }", "if (true);");
}

// port: RemoveUnusedCodeTest#testDeclarationInSwitch
#[test]
fn test_declaration_in_switch() {
    let mut f = Fixture::new();
    f.test(
        r#"const x = 1;
const y = 2;
switch (x) {
  case 1:
    let y = 3;
    break;
  default:
    let z = 5;
    alert(z);
    break;
}
alert(y);
"#,
        r#"const x = 1;
const y = 2;
switch (x) {
  case 1:
    break;
  default:
    let z = 5;
    alert(z);
    break;
}
alert(y);
"#,
    );
}

// port: RemoveUnusedCodeTest#testPrototypeIsAliased
#[test]
fn test_prototype_is_aliased() {
    let mut f = Fixture::new();
    // without alias C is removed
    f.test("function C() {} C.prototype = {};", "");
    // with alias C must stay
    f.test_same("function C() {} C.prototype = {}; var x = C.prototype; x;");
    f.test_same("var x; function C() {} C.prototype = {}; x = C.prototype; x;");
    f.test_same("var x; function C() {} x = C.prototype = {}; x;");
}

// port: RemoveUnusedCodeTest#testWholePrototypeAssignment
#[test]
fn test_whole_prototype_assignment() {
    let mut f = Fixture::new();
    f.test("function C() {} C.prototype = { constructor: C };", "");
}

// port: RemoveUnusedCodeTest#testUsageBeforeDefinition
#[test]
fn test_usage_before_definition() {
    let mut f = Fixture::new();
    f.test(
        "function f(a) { x[a] = 1; } var x; x = {}; f();",
        "function f() {} f();",
    );
}

// port: RemoveUnusedCodeTest#testReferencedPropertiesOnUnreferencedVar
#[test]
fn test_referenced_properties_on_unreferenced_var() {
    let mut f = Fixture::new();
    // preserve format
    f.test(
        "var x = {}; x.a = 1; var y = {a: 2}; y.a;",
        "                     var y = {a: 2}; y.a;",
    );
}

// port: RemoveUnusedCodeTest#testPropertyValuesAddedAfterReferenceAreRemoved
#[test]
fn test_property_values_added_after_reference_are_removed() {
    let mut f = Fixture::new();
    // Make sure property assignments added after the first reference to the var are kept and their
    // values traversed.
    f.test_same("var x = 1; var y = {}; y; y.foo = x;");
}

// port: RemoveUnusedCodeTest#testReferenceInObjectLiteral
#[test]
fn test_reference_in_object_literal() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(a) {
  return {a: a};
}
f(1);
"#,
    );
}

// port: RemoveUnusedCodeTest#testSelfOverwrite
#[test]
fn test_self_overwrite() {
    let mut f = Fixture::new();
    // Test for possible ConcurrentModificationException
    // Reference to `a` triggers traversal of its function value, which recursively adds another
    // definition of `a` to consider.
    f.test_same("var a = function() { a = function() {}; }; a();");
}

// port: RemoveUnusedCodeTest#testPropertyReferenceAddsPropertyReference
#[test]
fn test_property_reference_adds_property_reference() {
    let mut f = Fixture::new();
    // Test for possible ConcurrentModificationException
    // Reference to `a.foo()` triggers traversal of its function value, which recursively adds
    // the `foo` property to another variable.
    f.test_same("var a = {}; a.foo = function() { b.foo = 1; }; var b = {}; a.foo(); b.foo;");
}

// port: RemoveUnusedCodeTest#testExternVarDestructuredAssign
#[test]
fn test_extern_var_destructured_assign() {
    let mut f = Fixture::new();
    f.test_same("({a:externVar} = {a:1});");
    f.test_same("({a:externVar = 1} = {});");
    f.test_same("({['a']:externVar} = {a:1});");
    f.test_same("({['a']:externVar = 1} = {});");
    f.test_same("[externVar] = [1];");
    f.test_same("[externVar = 1] = [];");
    f.test_same("[, ...externVar] = [1];");
    f.test_same("[, ...[externVar = 1]] = [1];");
}

// port: RemoveUnusedCodeTest#testRemoveVarDeclaration1
#[test]
fn test_remove_var_declaration1() {
    let mut f = Fixture::new();
    f.test("var a = 0, b = a = 1", "");
}

// port: RemoveUnusedCodeTest#testRemoveVarDeclaration2
#[test]
fn test_remove_var_declaration2() {
    let mut f = Fixture::new();
    f.test("var a;var b = 0, c = a = b = 1", "");
}

// port: RemoveUnusedCodeTest#testRemoveUnusedSymbol
#[test]
fn test_remove_unused_symbol() {
    let mut f = Fixture::new();
    let externs =
        CompilerTestCase::externs("var Symbol = function(opt_desc) {}; var foo = function() {};");
    f.test_parts(vec![
        TestPart::Externs(externs.clone()),
        TestPart::Sources(CompilerTestCase::srcs("var x = Symbol();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
    f.test_parts(vec![
        TestPart::Externs(externs.clone()),
        TestPart::Sources(CompilerTestCase::srcs("const x = Symbol('desc');")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
    f.test_parts(vec![
        TestPart::Externs(externs.clone()),
        TestPart::Sources(CompilerTestCase::srcs("let x = Symbol();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
    f.test_parts(vec![
        TestPart::Externs(externs.clone()),
        TestPart::Sources(CompilerTestCase::srcs("var x = Symbol(foo());")),
        TestPart::Expected(CompilerTestCase::expected("Symbol(foo());")),
    ]);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedVarsFn0
#[test]
fn test_remove_unused_vars_fn0() {
    let mut f = Fixture::new();
    // Test with function expressions in another function call
    f.test(
        "function A(){} if(0){function B(){}} window.setTimeout(function(){A()})",
        "function A(){} if(0);                window.setTimeout(function(){A()})",
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedVarsFn1s
#[test]
fn test_remove_unused_vars_fn1s() {
    let mut f = Fixture::new();
    // Test with function expressions in another function call
    f.test(
        r#"function A(){}
if(0){var B = function(){}}
A();
"#,
        r#"function A(){}
if(0){}
A();
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedVars1
#[test]
fn test_remove_unused_vars1() {
    let mut f = Fixture::new();
    f.test(
        r#"var a;
var b=3;
var c=function(){};
var x=A();
var y;
var z;
function A(){B()}
function B(){C(b)}
function C(){}
function X(){Y()}
function Y(z){Z(x)}
function Z(){y}
externVar = function(){A()};
try{0}catch(e){a}
"#,
        r#"var a;
var b=3;
A();
function A(){B()}
function B(){C(b)}
function C(){}
externVar = function(){A()};
try{0}catch(e){a}
"#,
    );
    // Test removal from if {} blocks
    // preserve alignment
    f.test(
        "var i=0; var j=0; if(i>0){var k=1;}",
        "var i=0;          if(i>0);",
    );
    // Test with for loop
    f.test(
        r#"var x = '';
for (var i in externVar) {
  if (i > 0) x += ', ';
  var arg = 'foo';
  if (arg.length > 40) {
    var unused = 'bar';
    arg = arg.substr(0, 40) + '...';
  }
  x += arg;
}
alert(x);
"#,
        r#"var x = '';
for(var i in externVar) {
  if (i > 0) x += ', ';
  var arg = 'foo';
  if (arg.length > 40) arg = arg.substr(0,40) + '...';
  x += arg
}
alert(x);
"#,
    );
    // Test with recursive functions
    // preserve alignment
    f.test(
        "function A(){A()}function B(){B()}B()",
        "                 function B(){B()}B()",
    );
    // Test with multiple var declarations.
    // preserve alignment
    f.test(
        "var x,y=2,z=3; alert(x); alert(z); var a,b,c=4;",
        "var x,z=3;     alert(x); alert(z);",
    );
    // Test with for loop declarations
    f.test(
        r#"for(var i=0,j=0;i<10;){}
for(var x=0,y=0;;y++){}
for(var a,b;;){a}
for(var c,d;;);
for(var item in externVar){}
"#,
        r#"for(var i=0;i<10;);
for(var y=0;;y++);
for(var a;;)a;
for(;;);
for(var item in externVar);
"#,
    );
    // Test multiple passes required
    f.test(
        "var a,b,c,d; var e=[b,c]; var x=e[3]; var f=[d]; alert(f[0])",
        "var       d;                          var f=[d]; alert(f[0])",
    );
    // Test proper scoping (static vs dynamic)
    f.test(
        "var x; function A() {var x; B()} function B(){ alert(x); } A();",
        "var x; function A() {       B()} function B(){ alert(x); } A();",
    );
    // Test closures in a return statement
    f.test_same("function A(){var x;return function(){alert(x)}}A()");
    // Test other closures, multiple passes
    f.test(
        r#"function A(){}
function B() {
  var c,d,e,f,g,h;
  function C(){ alert(c) }
  var handler = function(){ alert(d) };
  var handler2 = function(){ handler() };
  e = function(){ alert(e) };
  if (1) { function G(){ alert(g) } }
  externVar = [function(){ alert(h) }];
  return function(){ alert(f) };
}
B()
"#,
        r#"function B() {
  var f,h;
  if(1);
  externVar = [function(){ alert(h) }];
  return function(){ alert(f) }
}
B()
"#,
    );
    // Test exported names
    // preserve alignment
    f.test(
        "var a,b=1; function _A1() {this.foo(a)}",
        "var a;     function _A1() {this.foo(a)}",
    );
    // Test undefined (i.e. externally defined) names
    f.test_same("externVar = 1");
    // Test unused vars with side effects
    f.test(
        "var a, b = alert(), c = externVar++,     d; var e = alert(); var f; alert(d);",
        "           alert();     externVar++; var d;         alert();        alert(d);",
    );
    f.test("var a, b = alert()", "alert()");
    f.test("var b = alert(), a", "alert()");
    f.test("var a, b = alert(a)", "var a; alert(a);");
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval
#[test]
fn test_function_arg_removal() {
    let mut f = Fixture::new();
    // remove all function arguments
    // preserve alignment
    f.test(
        "var b=function(c,d){return};b(1,2)",
        "var b=function(   ){return};b(1,2)",
    );
    // remove no function arguments
    f.test_same("var b=function(c,d){return c+d};b(1,2)");
    f.test_same("var b=function(e,f,c,d){return c+d};b(1,2)");
    // remove some function arguments
    // preserve alignment
    f.test(
        "var b=function(c,d,e,f){return c+d};b(1,2)",
        "var b=function(c,d    ){return c+d};b(1,2)",
    );
    f.test(
        "var b=function(e,c,f,d,g){return c+d};b(1,2)",
        "var b=function(e,c,f,d){return c+d};b(1,2)",
    );
}

// port: RemoveUnusedCodeTest#testDollarSuperParameterNotRemoved
#[test]
fn test_dollar_super_parameter_not_removed() {
    let mut f = Fixture::new();
    // supports special parameter expected by the prototype open-source library
    f.test_same("function f($super) {} f();");
}

// port: RemoveUnusedCodeTest#testFunctionArgRemovalWithLeadingUnderscore
#[test]
fn test_function_arg_removal_with_leading_underscore() {
    let mut f = Fixture::new();
    // Coding convention usually prevents removal of variables beginning with a leading underscore,
    // but that makes no sense for parameter names.
    // preserve alignment
    f.test(
        "function f(__$jscomp$1) {__$jscomp$1 = 1;} f();",
        "function f(           ) {                } f();",
    );
}

// port: RemoveUnusedCodeTest#testComputedPropertyDestructuring
#[test]
fn test_computed_property_destructuring() {
    let mut f = Fixture::new();
    // Don't remove variables accessed by computed properties
    f.test_same("var {['a']:a, ['b']:b} = {a:1, b:2}; a; b;");
    // preserve alignment
    f.test(
        "var {['a']:a, ['b']:b} = {a:1, b:2};",
        "var {                } = {a:1, b:2};",
    );
    f.test_same("var {[alert()]:a, [alert()]:b} = {a:1, b:2};");
    f.test_same("var a = {['foo' + 1]:1, ['foo' + 2]:2}; alert(a.foo1);");
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval_defaultValue1
#[test]
fn test_function_arg_removal_default_value1() {
    let mut f = Fixture::new();
    // preserve alignment
    f.test(
        "function f(unusedParam = undefined) {}; f();",
        "function f(                       ) {}; f();",
    );
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval_defaultValue2
#[test]
fn test_function_arg_removal_default_value2() {
    let mut f = Fixture::new();
    // preserve alignment
    f.test(
        "function f(unusedParam = 0) {}; f();",
        "function f(               ) {}; f();",
    );
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval_defaultValue3
#[test]
fn test_function_arg_removal_default_value3() {
    let mut f = Fixture::new();
    // Parameters already encountered can be used by later parameters
    // preserve alignment
    f.test(
        "function f(x, y = x + 0) {}; f();",
        "function f(            ) {}; f();",
    );
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval_defaultValue4
#[test]
fn test_function_arg_removal_default_value4() {
    let mut f = Fixture::new();
    // Parameters already encountered can be used by later parameters
    f.test_same("function f(x, y = x + 0) { y; }; f();");
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval_defaultValue5
#[test]
fn test_function_arg_removal_default_value5() {
    let mut f = Fixture::new();
    // Default value inside arrow function param list
    // preserve alignment
    f.test(
        "var f = (unusedParam = 0) => {}; f();",
        "var f = (               ) => {}; f();",
    );
    f.test_same("var f = (usedParam = 0) => { usedParam; }; f();");
    f.test("var f = (usedParam = 0) => {usedParam;};", "");
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval_defaultValue6
#[test]
fn test_function_arg_removal_default_value6() {
    let mut f = Fixture::new();
    // Parameters already encountered can be used by later parameters
    f.test_same("var x = 2; function f(y = x) { use(y); }; f();");
}

// port: RemoveUnusedCodeTest#testFunctionArgRemoval_defaultValue7
#[test]
fn test_function_arg_removal_default_value7() {
    let mut f = Fixture::new();
    // Parameters already encountered can be used by later parameters
    // preserve alignment
    f.test(
        "var x = 2; function f(y = x) {}; f();",
        "           function f(     ) {}; f();",
    );
}

// port: RemoveUnusedCodeTest#testDestructuringParams
#[test]
fn test_destructuring_params() {
    let mut f = Fixture::new();
    // Default value not used
    f.test(
        "function f({a:{b:b}} = {a:{}}) { /* b is unused */ }; f();",
        "function f({a:{   }} = {a:{}}) { /* b is unused */ }; f();",
    );
    // Default value with nested value used in default value assignment
    f.test(
        "function f({a:{b:b}} = {a:{b:1}}) { /* b is unused */ }; f();",
        "function f({a:{   }} = {a:{b:1}}) { /* b is unused */ }; f();",
    );
    // Default value with nested value used in function body
    f.test_same("function f({a:{b:b}} = {a:{b:1}}) { b; }; f();");
    // preserve alignment
    f.test(
        "function f({a:{b:b}} = {a:{b:1}}) {}; f();",
        "function f({a:{   }} = {a:{b:1}}) {}; f();",
    );
    // preserve alignment
    f.test(
        "function f({a:{b:b}} = {a:{}}) {}; f();",
        "function f({a:{   }} = {a:{}}) {}; f();",
    );
    // Destructuring pattern not default and parameter not used
    f.test(
        "function f({a:{b:b}}) { /* b is unused */ }; f({c:{d:1}});",
        "function f({a:{   }}) { /* b is unused */ }; f({c:{d:1}});",
    );
    // Destructuring pattern not default and parameter used
    f.test_same("function f({a:{b:b}}) { b; }; f({c:{d:1}});");
}

// port: RemoveUnusedCodeTest#testMixedParamTypes
#[test]
fn test_mixed_param_types() {
    let mut f = Fixture::new();
    // Traditional and destructuring pattern
    // preserve alignment
    f.test(
        "function f({a:{b:b}}, c, d) { c; }; f();",
        "function f({a:{   }}, c   ) { c; }; f();",
    );
    // preserve alignment
    f.test(
        "function f({a:{b:b = 5}}, c, d) { c; }; f();",
        "function f({a:{       }}, c   ) { c; }; f()",
    );
    f.test_same("function f({}, c, {d:{e:e}}) { c; e; }; f();");
    // Parent is the parameter list
    // preserve alignment
    f.test(
        "function f({a}, b, {c}) {use(a)}; f({}, {});",
        "function f({a}        ) {use(a)}; f({}, {});",
    );
    // Default and traditional
    f.test(
        "function f(unusedParam = undefined, z) { z; }; f();",
        "function f(unusedParam, z            ) { z; }; f();",
    );
}

// port: RemoveUnusedCodeTest#testDefaultParams
#[test]
fn test_default_params() {
    let mut f = Fixture::new();
    // preserve alignment
    f.test(
        "function f(x = undefined) {}; f();",
        "function f(             ) {}; f();",
    );
    // preserve alignment
    f.test("function f(x = 0) {}; f();", "function f(     ) {}; f();");
    f.test_same("function f(x = 0, y = x) { alert(y); }; f();");
}

// port: RemoveUnusedCodeTest#testDefaultParamsInClass
#[test]
fn test_default_params_in_class() {
    let mut f = Fixture::new();
    f.test(
        "class Foo { constructor(value = undefined) {} }; new Foo;",
        "class Foo { constructor(                 ) {} }; new Foo;",
    );
    f.test_same("class Foo { constructor(value = undefined) { value; } }; new Foo;");
    f.test_same(
        r#"class Bar {}
class Foo extends Bar {
  constructor(value = undefined) { super(); value; }
}
new Foo;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDefaultParamsInClassThatReferencesArguments
#[test]
fn test_default_params_in_class_that_references_arguments() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class Bar {}
class Foo extends Bar {
  constructor(value = undefined) {
    super();
    if (arguments.length)
      value;
  }
};
new Foo;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDefaultParamsWithoutSideEffects0
#[test]
fn test_default_params_without_side_effects0() {
    let mut f = Fixture::new();
    // preserve alignment
    f.test("function f({} = {}){}; f()", "function f(       ){}; f()");
}

// port: RemoveUnusedCodeTest#testDefaultParamsWithoutSideEffects1
#[test]
fn test_default_params_without_side_effects1() {
    let mut f = Fixture::new();
    // preserve alignment
    f.test("function f(a = 1) {}; f();", "function f(     ) {}; f();");
    // preserve alignment
    f.test(
        "function f({a:b = 1} = 1){}; f();",
        "function f(             ){}; f();",
    );
    // preserve alignment
    f.test(
        "function f({a:b} = {}){}; f();",
        "function f(          ){}; f();",
    );
}

// port: RemoveUnusedCodeTest#testDefaultParamsWithSideEffects1
#[test]
fn test_default_params_with_side_effects1() {
    let mut f = Fixture::new();
    f.test_same("function f(a = alert('foo')) {}; f();");
    f.test_same("function f({} = alert('foo')){}; f()");
    // preserve alignment
    f.test(
        "function f(){var x; var {a:b} = x()}; f();",
        "function f(){var x; var {   } = x()}; f();",
    );
    f.test(
        "function f(){var {a:b} = alert('foo')}; f();",
        "function f(){var {} = alert('foo')}; f();",
    );
    // preserve alignment
    f.test(
        "function f({a:b} = alert('foo')){}; f();",
        "function f({   } = alert('foo')){}; f();",
    );
    f.test_same("function f({a:b = alert('bar')} = alert('foo')){}; f();");
}

// port: RemoveUnusedCodeTest#testDefaultParamsWithSideEffects2
#[test]
fn test_default_params_with_side_effects2() {
    let mut f = Fixture::new();
    // preserve alignment
    f.test(
        "function f({a:b} = alert('foo')){}; f();",
        "function f({   } = alert('foo')){}; f();",
    );
}

// port: RemoveUnusedCodeTest#testUnusedSetterParam_isRetained
#[test]
fn test_unused_setter_param_is_retained() {
    let mut f = Fixture::new();
    // These params are a syntactic requirement.
    f.test_same("class Foo { set foo(x)     { } }; use(new Foo);");
    f.test_same("class Foo { set ['foo'](x) { } }; use(new Foo);");
    f.test_same("class Foo { set 'foo'(x)   { } }; use(new Foo);");
}

// port: RemoveUnusedCodeTest#testArrayDestructuringParams
#[test]
fn test_array_destructuring_params() {
    let mut f = Fixture::new();
    // Default values in array pattern unused
    // preserve alignment
    f.test(
        "function f([x,y] = [1,2]) {}; f();",
        "function f(             ) {}; f();",
    );
    // preserve alignment
    f.test(
        "function f([x,y] = [1,2], z) { z; }; f();",
        "function f([   ] = [1,2], z) { z; }; f();",
    );
    // Default values in array pattern used
    // preserve alignment
    f.test(
        "function f([x,y,z] = [1,2,3]) { y; }; f();",
        "function f([ ,y  ] = [1,2,3]) { y; }; f();",
    );
    // Side effects
    // preserve alignment
    f.test(
        "function f([x] = [alert()]) {}; f();",
        "function f([ ] = [alert()]) {}; f();",
    );
}

// port: RemoveUnusedCodeTest#testRestPattern
#[test]
fn test_rest_pattern() {
    let mut f = Fixture::new();
    f.test_same("var x; [...x] = externVar;");
    f.test_same("var [...x] = externVar;");
    f.test_same("var x; [...x] = externVar; use(x);");
    f.test_same("var [...x] = externVar; use(x);");
    f.test_same("var x; [...x.y] = externVar;");
    f.test_same("var x; [...x['y']] = externVar;");
    f.test_same("var x; [...x().y] = externVar;");
    f.test_same("var x; [...x()['y']] = externVar;");
}

// port: RemoveUnusedCodeTest#testRestParams
#[test]
fn test_rest_params() {
    let mut f = Fixture::new();
    f.test(
        "function foo(...args) {/* rest param unused*/}; foo();",
        "function foo(       ) {/* rest param unused*/}; foo();",
    );
    f.test_same("function foo(a, ...args) { args[0]; }; foo();");
    // Rest param in pattern
    f.test_same(
        r#"function countArgs(x, ...{length}) {
  return length;
}
alert(countArgs(1, 1, 1, 1, 1));
"#,
    );
    f.test_same("function foo([...rest]) {/* rest unused*/}; foo();");
    f.test_same("function foo([x, ...rest]) { x; }; foo();");
}

// port: RemoveUnusedCodeTest#testFunctionsDeadButEscaped
#[test]
fn test_functions_dead_but_escaped() {
    let mut f = Fixture::new();
    f.test_same("function b(a) { a = 1; alert(arguments[0]) }; b(6)");
    f.test_same("function b(a) { var c = 2; a = c; alert(arguments[0]) }; b(6)");
}

// port: RemoveUnusedCodeTest#testVarInControlStructure
#[test]
fn test_var_in_control_structure() {
    let mut f = Fixture::new();
    f.test("if (true) var b = 3;", "if(true);");
    f.test("if (true) var b = 3; else var c = 5;", "if(true);else;");
    f.test("while (true) var b = 3;", "while(true);");
    f.test("for (;;) var b = 3;", "for(;;);");
    f.test("do var b = 3; while(true)", "do;while(true)");
    f.test("with (true) var b = 3;", "with(true);");
    f.test("f: var b = 3;", "f:{}");
}

// port: RemoveUnusedCodeTest#testRValueHoisting
#[test]
fn test_r_value_hoisting() {
    let mut f = Fixture::new();
    f.test("var x = alert();", "alert()");
    f.test("var x = {a: alert()};", "({a:alert()})");
    f.test("var x=function y(){}", "");
}

// port: RemoveUnusedCodeTest#testModule
#[test]
fn test_module() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Sources(CompilerTestCase::srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk(
                    r#"var unreferenced=1; function x() { foo(); }
function uncalled() { var x; return 2; }
"#,
                )
                .add_chunk("var a,b; function foo() { this.foo(a); } x()")
                .build(),
        )),
        TestPart::Expected(CompilerTestCase::expected_strings(&[
            JsString::from("function x(){foo()}"),
            JsString::from("var a;function foo(){this.foo(a)}x()"),
        ])),
    ]);
}

// port: RemoveUnusedCodeTest#testRecursiveFunction1
#[test]
fn test_recursive_function1() {
    let mut f = Fixture::new();
    f.test_same("(function x(){return x()})()");
}

// port: RemoveUnusedCodeTest#testRecursiveFunction2
#[test]
fn test_recursive_function2() {
    let mut f = Fixture::new();
    f.test(
        "var x = 3; (function          x() { return          x(); })();",
        "           (function x$jscomp$1() { return x$jscomp$1(); })()",
    );
}

// port: RemoveUnusedCodeTest#testFunctionWithName1
#[test]
fn test_function_with_name1() {
    let mut f = Fixture::new();
    // preserve alignment
    f.test("var x=function f(){};x()", "var x=function  (){};x()");
    f.t.preserve_function_expression_names = true;
    f.test_same("var x=function f(){};x()");
}

// port: RemoveUnusedCodeTest#testFunctionWithName2
#[test]
fn test_function_with_name2() {
    let mut f = Fixture::new();
    f.test("alert(function bar(){})", "alert(function(){})");
    f.t.preserve_function_expression_names = true;
    f.test_same("alert(function bar(){})");
}

// port: RemoveUnusedCodeTest#testRemoveGlobal1
#[test]
fn test_remove_global1() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test_same("var x=1");
    f.test("var y=function(x){var z;}", "var y=function(x){}");
}

// port: RemoveUnusedCodeTest#testRemoveGlobal2
#[test]
fn test_remove_global2() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test_same("var x=1");
    f.test("function y(x){var z;}", "function y(x){}");
}

// port: RemoveUnusedCodeTest#testRemoveGlobal3
#[test]
fn test_remove_global3() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test_same("var x=1");
    // preserve alignment
    f.test(
        "function x(){function y(x){var z;}y()}",
        "function x(){function y(x){      }y()}",
    );
}

// port: RemoveUnusedCodeTest#testRemoveGlobal4
#[test]
fn test_remove_global4() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test_same("var x=1");
    // preserve alignment
    f.test(
        "function x(){function y(x){var z;}}",
        "function x(){                     }",
    );
}

// port: RemoveUnusedCodeTest#testIssue168a
#[test]
fn test_issue168a() {
    let mut f = Fixture::new();
    f.test(
        r#"function _a(){
  (function(x){ _b(); })(1);
}
function _b(){
  _a();
}
"#,
        r#"function _a(){(function(){_b()})(1)}
function _b(){_a()}
"#,
    );
}

// port: RemoveUnusedCodeTest#testIssue168b
#[test]
fn test_issue168b() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test(
        r#"function a(){
  (function(x){ b(); })(1);
}
function b(){
  a();
}
"#,
        r#"function a(){(function(x){b()})(1)}
function b(){a()}
"#,
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign1
#[test]
fn test_unused_assign1() {
    let mut f = Fixture::new();
    f.test("var x = 3; x = 5;", "");
}

// port: RemoveUnusedCodeTest#testUnusedAssign2
#[test]
fn test_unused_assign2() {
    let mut f = Fixture::new();
    //
    f.test(
        "function f(a) { a = 3; } this.x = f;",
        "function f(){} this.x=f",
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign3
#[test]
fn test_unused_assign3() {
    let mut f = Fixture::new();
    // e can't be removed, so we don't try to remove the dead assign.
    // We might be able to improve on this case.
    //
    f.test(
        "try { throw ''; } catch (e) { e = 3; }",
        "try{throw\"\";}catch(e){e=3}",
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign4
#[test]
fn test_unused_assign4() {
    let mut f = Fixture::new();
    f.test(
        "function f(a, b) { this.foo(b); a = 3; } this.x = f;",
        "function f(a,b){this.foo(b);}this.x=f",
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign5
#[test]
fn test_unused_assign5() {
    let mut f = Fixture::new();
    //
    f.test(
        "var z = function f() { f = 3; }; z();",
        "var z=function(){};z()",
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign5b
#[test]
fn test_unused_assign5b() {
    let mut f = Fixture::new();
    //
    f.test(
        "var z = function f() { f = alert(); }; z();",
        "var z=function(){alert()};z()",
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign6
#[test]
fn test_unused_assign6() {
    let mut f = Fixture::new();
    f.test("var z; z = 3;", "");
}

// port: RemoveUnusedCodeTest#testUnusedAssign6b
#[test]
fn test_unused_assign6b() {
    let mut f = Fixture::new();
    f.test("var z; z = alert();", "alert()");
}

// port: RemoveUnusedCodeTest#testUnusedAssign_toString
#[test]
fn test_unused_assign_to_string() {
    let mut f = Fixture::new();
    f.test("var x = (10).toString(16);", "");
    f.test("var x = (10).toString();", "");
    f.test("var x = (10).toString(16, 2);", "(10).toString(16, 2);");
}

// port: RemoveUnusedCodeTest#testUnusedAssign7
#[test]
fn test_unused_assign7() {
    let mut f = Fixture::new();
    // This loop is normalized to "var i;for(i in..."
    // TODO(johnlenz): "i = a" should be removed here.
    f.test(
        "var a = 3; for (var i in {}) { i = a; }",
        "var a = 3; var i; for (i in {}) {i = a;}",
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign8
#[test]
fn test_unused_assign8() {
    let mut f = Fixture::new();
    // This loop is normalized to "var i;for(i in..."
    // TODO(johnlenz): "i = a" should be removed here.
    f.test(
        "var a = 3; for (var i in {}) { i = a; } alert(a);",
        "var a = 3; var i; for (i in {}) {i = a} alert(a);",
    );
}

// port: RemoveUnusedCodeTest#testUnusedAssign9a
#[test]
fn test_unused_assign9a() {
    let mut f = Fixture::new();
    f.test_same("function b(a) { a = 1; arguments; }; b(6)");
}

// port: RemoveUnusedCodeTest#testUnusedAssign9
#[test]
fn test_unused_assign9() {
    let mut f = Fixture::new();
    f.test_same("function b(a) { a = 1; arguments=1; }; b(6)");
}

// port: RemoveUnusedCodeTest#testES6ModuleExports
#[test]
fn test_es6_module_exports() {
    let mut f = Fixture::new();
    f.test("const X = 1; function f() {}", "");
    f.test(
        "const X = 1; export function f() {}",
        "function f() {} export {f as f}",
    );
    f.test(
        "const X = 1; export class C {}",
        "class C {} export { C as C }",
    );
    f.test(
        "const X = 1; export default function f() {};",
        "export default function f() {}",
    );
    f.test(
        "const X = 1; export default class C {}",
        "export default class C {}",
    );
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign1
#[test]
fn test_unused_prop_assign1() {
    let mut f = Fixture::new();
    f.test("var x = {}; x.foo = 3;", "");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign1b
#[test]
fn test_unused_prop_assign1b() {
    let mut f = Fixture::new();
    f.test("var x = {}; x.foo = alert();", "alert()");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign2
#[test]
fn test_unused_prop_assign2() {
    let mut f = Fixture::new();
    f.test("var x = {}; x['foo'] = 3;", "");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign2b
#[test]
fn test_unused_prop_assign2b() {
    let mut f = Fixture::new();
    f.test("var x = {}; x[alert()] = alert();", "alert(),alert()");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign3
#[test]
fn test_unused_prop_assign3() {
    let mut f = Fixture::new();
    f.test("var x = {}; x['foo'] = {}; x['bar'] = 3", "");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign3b
#[test]
fn test_unused_prop_assign3b() {
    let mut f = Fixture::new();
    f.test(
        "var x = {}; x[alert()] = alert(); x[alert() + alert()] = alert()",
        "alert(),alert();(alert() + alert()),alert()",
    );
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign4
#[test]
fn test_unused_prop_assign4() {
    let mut f = Fixture::new();
    f.test("var x = {foo: 3}; x['foo'] = 5;", "");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign5
#[test]
fn test_unused_prop_assign5() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var x = {foo: alert()}; x['foo'] = 5;",
        "       ({foo: alert()})             ;",
    );
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign6
#[test]
fn test_unused_prop_assign6() {
    let mut f = Fixture::new();
    f.test(
        "var x = function() {}; x.prototype.bar = function() {};",
        "",
    );
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign6b
#[test]
fn test_unused_prop_assign6b() {
    let mut f = Fixture::new();
    f.test("function x() {} x.prototype.bar = function() {};", "");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign6c
#[test]
fn test_unused_prop_assign6c() {
    let mut f = Fixture::new();
    f.test("function x() {} x.prototype['bar'] = function() {};", "");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign7
#[test]
fn test_unused_prop_assign7() {
    let mut f = Fixture::new();
    f.test("var x = {}; x[x.foo] = x.bar;", "");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign7b
#[test]
fn test_unused_prop_assign7b() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; x[x.foo] = alert(x.bar);");
}

// port: RemoveUnusedCodeTest#testUnusedPropAssign7c
#[test]
fn test_unused_prop_assign7c() {
    let mut f = Fixture::new();
    //
    f.test(
        "var x = {}; x[alert(x.foo)] = x.bar;",
        "var x={};x[alert(x.foo)]=x.bar",
    );
}

// port: RemoveUnusedCodeTest#testUsedPropAssign1
#[test]
fn test_used_prop_assign1() {
    let mut f = Fixture::new();
    f.test_same("function f(x) { x.bar = 3; } f({});");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign2
#[test]
fn test_used_prop_assign2() {
    let mut f = Fixture::new();
    f.test_same("try { throw {}; } catch (e) { e.bar = 3; }");
}

// port: RemoveUnusedCodeTest#testUsedPropInDestructuringCatch_objectPattern
#[test]
fn test_used_prop_in_destructuring_catch_object_pattern() {
    let mut f = Fixture::new();
    f.test_same("this.p = 1; try {} catch ({p}) { alert(p); }");
}

// port: RemoveUnusedCodeTest#testUsedPropInDestructuringCatch_arrayPattern
#[test]
fn test_used_prop_in_destructuring_catch_array_pattern() {
    let mut f = Fixture::new();
    f.test_same("this.p = 1; try {} catch ([{p}]) { alert(p); }");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign3
#[test]
fn test_used_prop_assign3() {
    let mut f = Fixture::new();
    // This pass does not do flow analysis.
    f.test_same("var x = {}; x.foo = 3; x = alert();");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign4
#[test]
fn test_used_prop_assign4() {
    let mut f = Fixture::new();
    f.test_same("var y = alert(); var x = {}; x.foo = 3; y[x.foo] = 5;");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign5
#[test]
fn test_used_prop_assign5() {
    let mut f = Fixture::new();
    f.test_same("var y = alert(); var x = 3; y[x] = 5;");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign6
#[test]
fn test_used_prop_assign6() {
    let mut f = Fixture::new();
    f.test_same("var x = alert(externVar); x.innerHTML = 'new text';");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign7
#[test]
fn test_used_prop_assign7() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; for (x in alert()) { x.foo = 3; }");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign8
#[test]
fn test_used_prop_assign8() {
    let mut f = Fixture::new();
    f.test_same("for (var x in alert()) { x.foo = 3; }");
}

// port: RemoveUnusedCodeTest#testUsedPropAssign9
#[test]
fn test_used_prop_assign9() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; x.foo = alert(externVar); x.foo.innerHTML = 'new test';");
}

// port: RemoveUnusedCodeTest#testUsedPropNotAssign
#[test]
fn test_used_prop_not_assign() {
    let mut f = Fixture::new();
    f.test_same("function f(x) { x.a; }; f(externVar);");
}

// port: RemoveUnusedCodeTest#testDependencies1
#[test]
fn test_dependencies1() {
    let mut f = Fixture::new();
    f.test("var a = 3; var b = function() { alert(a); };", "");
}

// port: RemoveUnusedCodeTest#testDependencies1b
#[test]
fn test_dependencies1b() {
    let mut f = Fixture::new();
    f.test(
        "var a = 3; var b = alert(function() { alert(a); });",
        "var a=3;alert(function(){alert(a)})",
    );
}

// port: RemoveUnusedCodeTest#testDependencies1c
#[test]
fn test_dependencies1c() {
    let mut f = Fixture::new();
    //
    f.test(
        "var a = 3; var _b = function() { alert(a); };",
        "var a=3;var _b=function(){alert(a)}",
    );
}

// port: RemoveUnusedCodeTest#testDependencies2
#[test]
fn test_dependencies2() {
    let mut f = Fixture::new();
    f.test("var a = 3; var b = 3; b = function() { alert(a); };", "");
}

// port: RemoveUnusedCodeTest#testDependencies2b
#[test]
fn test_dependencies2b() {
    let mut f = Fixture::new();
    f.test(
        "var a = 3; var b = 3; b = alert(function() { alert(a); });",
        "var a=3;alert(function(){alert(a)})",
    );
}

// port: RemoveUnusedCodeTest#testDependencies2c
#[test]
fn test_dependencies2c() {
    let mut f = Fixture::new();
    f.test_same("var a=3;var _b=3;_b=function(){alert(a)}");
}

// port: RemoveUnusedCodeTest#testGlobalVarReferencesLocalVar
#[test]
fn test_global_var_references_local_var() {
    let mut f = Fixture::new();
    f.test_same("var a=3;function f(){var b=4;a=b}alert(a + f())");
}

// port: RemoveUnusedCodeTest#testLocalVarReferencesGlobalVar1
#[test]
fn test_local_var_references_global_var1() {
    let mut f = Fixture::new();
    f.test_same("var a=3;function f(b, c){b=a; alert(b + c);} f();");
}

// port: RemoveUnusedCodeTest#testLocalVarReferencesGlobalVar2
#[test]
fn test_local_var_references_global_var2() {
    let mut f = Fixture::new();
    //
    f.test(
        "var a=3;function f(b, c){b=a; alert(c);} f();",
        "function f(b, c) { alert(c); } f();",
    );
}

// port: RemoveUnusedCodeTest#testNestedAssign1
#[test]
fn test_nested_assign1() {
    let mut f = Fixture::new();
    //
    f.test(
        "var b = null; var a = (b = 3); alert(a);",
        "var a = 3; alert(a);",
    );
}

// port: RemoveUnusedCodeTest#testNestedAssign2
#[test]
fn test_nested_assign2() {
    let mut f = Fixture::new();
    //
    f.test(
        "var a = 1; var b = 2; var c = (b = a); alert(c);",
        "var a = 1; var c = a; alert(c);",
    );
}

// port: RemoveUnusedCodeTest#testNestedAssign3
#[test]
fn test_nested_assign3() {
    let mut f = Fixture::new();
    //
    f.test(
        "var b = 0; var z; z = z = b = 1; alert(b);",
        "var b = 0; b = 1; alert(b);",
    );
}

// port: RemoveUnusedCodeTest#testWeakUsageRemoved
#[test]
fn test_weak_usage_removed() {
    let mut f = Fixture::new();
    // There are no other references to `a`, so it can be removed.
    //
    f.test(
        "var a=function(){return}; use(goog.weakUsage(a));",
        "use(void 0);",
    );
}

// port: RemoveUnusedCodeTest#testWeakUsageNotRemoved
#[test]
fn test_weak_usage_not_removed() {
    let mut f = Fixture::new();
    // There is another reference to `a`, so it cannot be removed.
    //
    // Normally the PeepholeReplaceKnownMethods pass would remove the call to `goog.weakUsage`
    // entirely, but in this unit test we are only testing RemoveUnusedCode.
    f.test_same("var a=function(){return}; use(goog.weakUsage(a)); use(a);");
}

// port: RemoveUnusedCodeTest#testCallSiteInteraction
#[test]
fn test_call_site_interaction() {
    let mut f = Fixture::new();
    f.test_same("var b=function(){return};b()");
    f.test_same("var b=function(c){return c};b(1)");
    //
    f.test(
        "var b=function(c){return};b(1)",
        "var b=function(){return};b(1)",
    );
    f.test(
        "var b=function(c){};b.call(null, externVar)",
        "var b=function( ){};b.call(null, externVar)",
    );
    f.test(
        "var b=function(c){};b.apply(null, externVar)",
        "var b=function( ){};b.apply(null, externVar)",
    );
    // Recursive calls
    f.test_same("var b=function(c,d){b(1, 2);return d};b(3,4);b(5,6)");
    f.test_same("var b=function(c){return arguments};b(1,2);b(3,4)");
}

// port: RemoveUnusedCodeTest#testCallSiteInteraction_constructors
#[test]
fn test_call_site_interaction_constructors() {
    let mut f = Fixture::new();
    // The third level tests that the functions which have already been looked
    // at get re-visited if they are changed by a call site removal.
    f.test(
        r#"var Ctor1=function(a, b){return a};
var Ctor2=function(x, y){Ctor1.call(this, x, y)};
goog$inherits(Ctor2, Ctor1);
new Ctor2(1, 2)
"#,
        r#"var Ctor1=function(a){return a};
var Ctor2=function(x, y){Ctor1.call(this, x, y)};
goog$inherits(Ctor2, Ctor1);
new Ctor2(1, 2)
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedVarsPossibleNpeCase
#[test]
fn test_remove_unused_vars_possible_npe_case() {
    let mut f = Fixture::new();
    f.test(
        r#"var a = [];
var register = function(callback) {a[0] = callback};
register(function(transformer) {});
register(function(transformer) {});
"#,
        r#"var register = function() {};
register(function() {});
register(function() {});
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotOptimizeJSCompiler_renameProperty
#[test]
fn test_do_not_optimize_js_compiler_rename_property() {
    let mut f = Fixture::new();
    // Only the function definition can be modified, none of the call sites.
    f.test(
        r#"function JSCompiler_renameProperty(a) {};
JSCompiler_renameProperty('a');
"#,
        r#"function JSCompiler_renameProperty() {};
JSCompiler_renameProperty('a');
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotOptimizeSetters
#[test]
fn test_do_not_optimize_setters() {
    let mut f = Fixture::new();
    f.test_same("({set s(a) {}})");
}

// port: RemoveUnusedCodeTest#testRemoveSingletonClass1
#[test]
fn test_remove_singleton_class1() {
    let mut f = Fixture::new();
    f.test(
        r#"function goog$addSingletonGetter(a){}
/**@constructor*/function a(){}
goog$addSingletonGetter(a);
"#,
        "",
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass1
#[test]
fn test_remove_inherited_class1() {
    let mut f = Fixture::new();
    f.test(
        r#"/**@constructor*/ function a() {}
/**@constructor*/ function b() {}
goog$inherits(b, a);
new a;
"#,
        r#"/**@constructor*/ function a() {}
new a;
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass2
#[test]
fn test_remove_inherited_class2() {
    let mut f = Fixture::new();
    f.test(
        r#"/**@constructor*/function a(){}
/**@constructor*/function b(){}
/**@constructor*/function c(){}
goog$inherits(b,a);
"#,
        "",
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass3
#[test]
fn test_remove_inherited_class3() {
    let mut f = Fixture::new();
    f.test_same(
        r#"/**@constructor*/function a(){}
/**@constructor*/function b(){}
goog$inherits(b,a); new b
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass4
#[test]
fn test_remove_inherited_class4() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function goog$inherits(){}
/**@constructor*/function a(){}
/**@constructor*/function b(){}
goog$inherits(b,a);
/**@constructor*/function c(){}
goog$inherits(c,b); new c
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass5
#[test]
fn test_remove_inherited_class5() {
    let mut f = Fixture::new();
    f.test(
        r#"function goog$inherits() {}
/**@constructor*/ function a() {}
/**@constructor*/ function b() {}
goog$inherits(b,a);
/**@constructor*/ function c() {}
goog$inherits(c,b); new b
"#,
        r#"function goog$inherits(){}
/**@constructor*/ function a(){}
/**@constructor*/ function b(){}
goog$inherits(b,a); new b
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass8
#[test]
fn test_remove_inherited_class8() {
    let mut f = Fixture::new();
    f.test_same(
        r#"/**@constructor*/function a(){}
/**@constructor*/function b(){}
/**@constructor*/function c(){}
b.inherits(a);c.mixin(b.prototype);new c
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass9
#[test]
fn test_remove_inherited_class9() {
    let mut f = Fixture::new();
    f.test(
        r#"function goog$inherits() {}
/**@constructor*/ function a() {}
/**@constructor*/ function b() {}
goog$inherits(b,a); new a;
var c = a; var d = a.g; new b
"#,
        r#"function goog$inherits(){}
/**@constructor*/ function a(){}
/**@constructor*/ function b(){}
goog$inherits(b,a);
new a; new b
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass11
#[test]
fn test_remove_inherited_class11() {
    let mut f = Fixture::new();
    f.test_same(
        r#"/**@constructor*/function a(){}
var b = {};
// goog$inherits not treated specially when derived class is a property
goog$inherits(b.foo, a)
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass12
#[test]
fn test_remove_inherited_class12() {
    let mut f = Fixture::new();
    // An inherits call must be a statement unto itself or the left side of a comma to be considered
    // specially. These calls have no return values, so this restriction avoids false positives.
    // It also simplifies removal logic.
    f.test_same(
        r#"function a(){}
function b(){}
goog$inherits(b, a) + 1;
"#,
    );
    // Although a human is unlikely to write code like this, some optimizations may end up
    // converting an inherits call statement into the left side of a comma. We should still
    // remove this case.
    f.test(
        r#"function a(){}
function b(){}
(goog$inherits(b, a), 1);
"#,
        "1",
    );
}

// port: RemoveUnusedCodeTest#testRemoveInheritedClass13
#[test]
fn test_remove_inherited_class13() {
    let mut f = Fixture::new();
    f.test(
        r#"class D {}
class C {}
valueType$mixin(C, D, 1, goog.reflect.objectProperty('a', C))
"#,
        "",
    );
}

// port: RemoveUnusedCodeTest#testReflectedMethods
#[test]
fn test_reflected_methods() {
    let mut f = Fixture::new();
    f.test_same(
        r#"/** @constructor */
function Foo() {}
Foo.prototype.handle = function(x, y) { alert(y); };
var x = goog.reflect.object(Foo, {handle: 1});
for (var i in x) { x[i].call(x); }
window['Foo'] = Foo;
"#,
    );
}

// port: RemoveUnusedCodeTest#testIssue618_1
#[test]
fn test_issue618_1() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test_same(
        r#"function f() {

  var a = [], b;

  a.push(b = []);

  b[0] = 1;

  return a;

}
"#,
    );
}

// port: RemoveUnusedCodeTest#testIssue618_2
#[test]
fn test_issue618_2() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test_same("var b; externVar.push(b = []); b[0] = 1;");
}

// port: RemoveUnusedCodeTest#testBug38457201
#[test]
fn test_bug38457201() {
    let mut f = Fixture::new();
    f.t.remove_global = true;
    f.test(
        r#"var DOCUMENT_MODE = 1;
var temp;
false || (temp = (Number(DOCUMENT_MODE) >= 9));
"#,
        "false || 0",
    );
}

// port: RemoveUnusedCodeTest#testBlockScoping
#[test]
fn test_block_scoping() {
    let mut f = Fixture::new();
    f.test("{let x;}", "{}");
    f.test("{const x = 1;}", "{}");
    f.test_same("{const x =1; alert(x)}");
    f.test_same("{let x; alert(x)}");
    f.test("let x = 1;", "");
    f.test("let x; x = 3;", "");
    f.test(
        r#"let x;
{ let x; } // unused in block
alert(x);
"#,
        r#"let x; // outer x kept
{} // inner x removed
alert(x);
"#,
    );
    f.test_same(
        r#"var x;
{
  var y = 1;
  alert(y); // keeps y alive
}
alert(x);
"#,
    );
    f.test_same(
        r#"let x;
{
  let x = 1;
  alert(x); // keeps inner x alive
}
alert(x);
"#,
    );
    f.test(
        r#"let x;
{ let x; } // inner x unused
let y; // outer y unused
alert(x);
"#,
        r#"let x; // only outer x was used
{}
alert(x);
"#,
    );
    f.test(
        r#"let x;
{
  let g;
  {
    const y = 1;
    alert(y); // keeps y alive
  }
}
let z;
alert(x);
"#,
        r#"let x; // z removed
{
  { // g removed
    const y = 1;
    alert(y);
  }
}
alert(x);
"#,
    );
    f.test(
        r#"let x;
{
  let x = 1;
  {
    let y;
  }
  alert(x); // keeps inner x alive
}
alert(x);
"#,
        r#"let x; // outer x kept
{
  let x = 1;  // inner x kept
  {} // y removed
  alert(x);
}
alert(x);
"#,
    );
    f.test(
        r#"let x;
{
  let x;
  alert(x); // keeps inner x alive
}
"#,
        r#"{
  let x$jscomp$1; // inner x was renamed
  alert(x$jscomp$1);
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testArrowFunctions
#[test]
fn test_arrow_functions() {
    let mut f = Fixture::new();
    f.test(
        r#"class C {
  g() {
    var x;
  }
}
new C
"#,
        r#"class C {
  g() {
  }
}
new C
"#,
    );
    f.test("() => {var x}", "() => {};");
    f.test("(x) => {}", "() => {};");
    f.test_same("(x) => {alert(x)}");
    f.test_same("(x) => {x + 1}");
}

// port: RemoveUnusedCodeTest#testClasses
#[test]
fn test_classes() {
    let mut f = Fixture::new();
    f.test("var C = class {};", "");
    f.test("class C {}", "");
    f.test("class C {constructor(){} g() {}}", "");
    f.test_same("class C {}; new C;");
    f.test("var C = class X {}; new C;", "var C = class {}; new C;");
    f.test_same(
        r#"class C{
  g() {
    var x;
    alert(x); // use in alert prevents removal of x
  }
}
new C
"#,
    );
    f.test_same(
        r#"class C{
  g() {
    let x; // same as above but with 'let'
    alert(x);
  }
}
new C
"#,
    );
    f.test(
        r#"class C {
  g() {
    let x; // unused x
  }
}
new C
"#,
        r#"class C {
  g() { // x is removed
  }
}
new C
"#,
    );
}

// port: RemoveUnusedCodeTest#testClassFields
#[test]
fn test_class_fields() {
    let mut f = Fixture::new();
    // Removal of individual fields is not enabled in this test class.
    // These tests confirm that the existence of fields does not prevent removal of a class that
    // is never instantiated.
    f.test_same(
        r#"class C {
  x;
  y = 2;
  z = 'hi';
  static x;
  static y = 2;
  static z = 'hi';
}
new C
"#,
    );
    f.test(
        r#"class C {
  x;
  y = 2;
  z = 'hi';
  static x;
  static y = 2;
  static z = 'hi';
}
"#,
        "",
    );
    f.test_same(
        r#"class C {
  ['x'];
  1 = 2
  'a' = 'foo';
  static ['x'];
  static 1 = 2
  static 'a' = 'foo';
}
new C
"#,
    );
    f.test(
        r#"class C {
  ['x'];
  1 = 2
  'a' = 'foo';
  static ['x'];
  static 1 = 2
  static 'a' = 'foo';
}
"#,
        "",
    );
}

// port: RemoveUnusedCodeTest#testClassStaticBlocksDoesRemove
#[test]
fn test_class_static_blocks_does_remove() {
    let mut f = Fixture::new();
    f.test(
        r#"class C {
  static {
  }
}
"#,
        "",
    );
    // TODO(bradfordcsmith): Would be nice to remove the whole class at this point
    f.test(
        r#"class C {
  static {
    var x = 1;
    let y = 2;
    const z = 3;
  }
}
"#,
        r#"class C {
  static {
  }
}
"#,
    );
    f.test(
        r#"class C {
  static {
    if (true) {
      if (true) {
        var foo = function() {};
      }
    }
  }
}
"#,
        r#"class C {
  static {
    if (true) {
      if (true) {
      }
    }
  }
}
"#,
    );
    f.test(
        r#"class C {
  static {
    function f() {}
  }
}
"#,
        r#"class C {
  static {
  }
}
"#,
    );
    f.test(
        r#"const x = 1;
class C {
  static {
    function f() {
      x;
    }
  }
}
"#,
        r#"class C {
  static {
  }
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testClassStaticBlockDoesntRemove
#[test]
fn test_class_static_block_doesnt_remove() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  static {
    let x;
    alert(x);
    console.log(x);
  }
}
"#,
    );
    f.test_same(
        r#"class C {
  static {
    this.x=1;
  }
}
"#,
    );
    f.test_same(
        r#"const x = 1;
class C {
  static {
    x; // reference prevents `const x = 1;` from being removed.
  }
}
new C;
"#,
    );
    f.test_same(
        r#"const x = 1;
class C {
  static {
// side-effect of `alert` prevents `x` from being removed
    alert(x); // reference prevents `x` from being removed.
  }
}
"#,
    );
    f.test_same(
        r#"const x = 1;
class C {
  static {
    function f() {
      x; // reference prevents `const x = 1;` from being removed.
    }
    f(); // call prevents f() from being removed.
  }
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testComputedPropSideEffects
#[test]
fn test_computed_prop_side_effects() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  [alert(1)](){}
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testNonstaticClassFieldWithSideEffectsDoesRemove
#[test]
fn test_nonstatic_class_field_with_side_effects_does_remove() {
    let mut f = Fixture::new();
    f.test(
        r#"class C {
  y = alert(1);
}
"#,
        "",
    );
    f.test(
        r#"class C {
  ['y'] = alert(1);
}
"#,
        "",
    );
}

// port: RemoveUnusedCodeTest#testStaticClassFieldDetectsSideEffects
#[test]
fn test_static_class_field_detects_side_effects() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  static y = alert(1);
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testComputedClassFieldDetectsSideEffects
#[test]
fn test_computed_class_field_detects_side_effects() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  [alert(1)];
}
"#,
    );
    f.test_same(
        r#"class C {
  [alert(1)] = 'str';
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testStaticComputedClassFieldDetectsSideEffects
#[test]
fn test_static_computed_class_field_detects_side_effects() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  static [alert(1)];
}
"#,
    );
    f.test_same(
        r#"class C {
  static [alert(1)] = 'str';
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testReferencesInClasses
#[test]
fn test_references_in_classes() {
    let mut f = Fixture::new();
    f.test_same(
        r#"const A = 15;
const C = class {
  constructor() {
    this.a = A;
  }
}
new C;
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveGlobalClasses
#[test]
fn test_remove_global_classes() {
    let mut f = Fixture::new();
    f.t.remove_global = false;
    f.test_same("class C{}");
}

// port: RemoveUnusedCodeTest#testSubClasses
#[test]
fn test_sub_classes() {
    let mut f = Fixture::new();
    f.test("class D {} class C extends D {}", "");
    f.test(
        "class D {} class C extends D {} new D;",
        "class D {} new D;",
    );
    f.test_same("class D {} class C extends D {} new C;");
}

// port: RemoveUnusedCodeTest#testGenerators
#[test]
fn test_generators() {
    let mut f = Fixture::new();
    f.test(
        r#"function* f() {
  var x;
  yield x;
}
"#,
        "",
    );
    f.test(
        r#"function* f() {
  var x;
  yield x;
}
"#,
        "",
    );
    f.test(
        r#"function* f() {
  var x;
  var y;
  yield x;
}
f();
"#,
        r#"function* f() {
  var x;
  yield x;
}
f()
"#,
    );
}

// port: RemoveUnusedCodeTest#testForLet
#[test]
fn test_for_let() {
    let mut f = Fixture::new();
    // Could be optimized so that unused lets in the for loop are removed
    f.test("for(let x; ;){}", "for(;;){}");
    f.test("for(let x, y; ;) {x}", "for(let x; ;) {x}");
    f.test("for(let x, y; ;) {y}", "for(let y; ;) {y}");
    f.test("for(let x=0,y=0;;y++){}", "for(let y=0;;y++){}");
}

// port: RemoveUnusedCodeTest#testForInLet
#[test]
fn test_for_in_let() {
    let mut f = Fixture::new();
    f.test_same("let item; for(item in externVar){}");
    f.test_same("for(let item in externVar){}");
}

// port: RemoveUnusedCodeTest#testForOf
#[test]
fn test_for_of() {
    let mut f = Fixture::new();
    f.test_same("let item; for(item of externVar){}");
    f.test_same("for(var item of externVar){}");
    f.test_same("for(let item of externVar){}");
    f.test_same(
        r#"var x;
for (var n of externVar) {
  if (x > n) {}; // keeps x alive
}
"#,
    );
    f.test(
        r#"var x; // no references to this
for (var n of externVar) {
  if (n) {};
}
"#,
        r#"for (var n of externVar) { // x removed
  if (n) {};
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testForAwaitOf
#[test]
fn test_for_await_of() {
    let mut f = Fixture::new();
    f.test_same("async () => { let item; for await (item of externVar){} }");
    f.test_same("async () => { for(var item of externVar){} }");
    f.test_same("async () => { for(let item of externVar){} }");
    f.test_same(
        r#"async () => {
  var x;
  for (var n of externVar) {
    if (x > n) {}; // keeps x alive
  }
};
"#,
    );
    f.test(
        r#"async () => {
  var x; // no references to this
  for (var n of externVar) {
    if (n) {};
  }
};
"#,
        r#"async () => {
  for (var n of externVar) { // x removed
    if (n) {};
  }
};
"#,
    );
}

// port: RemoveUnusedCodeTest#testEnhancedForWithExternVar
#[test]
fn test_enhanced_for_with_extern_var() {
    let mut f = Fixture::new();
    f.test_same("for(externVar in externVar){}");
    f.test_same("for(externVar of externVar){}");
}

// port: RemoveUnusedCodeTest#testExternVarNotRemovable
#[test]
fn test_extern_var_not_removable() {
    let mut f = Fixture::new();
    f.test_same("externVar = 5;");
}

// port: RemoveUnusedCodeTest#testTemplateStrings
#[test]
fn test_template_strings() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var name = 'foo';
`Hello ${name}`
"#,
    );
}

// port: RemoveUnusedCodeTest#testDestructuringArrayPattern0
#[test]
fn test_destructuring_array_pattern0() {
    let mut f = Fixture::new();
    f.test("function f(a) {} f();", "function f() {} f();");
    f.test("function f([a]) {} f();", "function f() {} f();");
    f.test("function f(...a) {} f();", "function f() {} f();");
    f.test("function f(...[a]) {} f();", "function f() {} f();");
    f.test_same("function f(...[...a]) {} f();");
    f.test("function f(...{length:a}) {} f();", "function f() {} f();");
    f.test("function f(a) {} f();", "function f() {} f();");
    f.test("function f([a] = 1) {} f();", "function f() {} f();");
    f.test(
        "function f([a] = alert()) {} f();",
        "function f([] = alert()) {} f();",
    );
    f.test("function f(a = 1) {} f();", "function f() {} f();");
    f.test("function f([a = 1]) {} f();", "function f() {} f();");
    f.test("function f(...[a = 1]) {} f();", "function f() {} f();");
    f.test(
        "function f(...{length:a = 1}) {} f();",
        "function f() {} f();",
    );
    f.test_same("function f(a = alert()) {} f();");
    f.test_same("function f([a = alert()]) {} f();");
    f.test_same("function f(...[a = alert()]) {} f();");
    f.test_same("function f(...{length:a = alert()}) {} f();");
    f.test("function f([a] = []) {} f();", "function f() {} f();");
    f.test("function f([a]) {} f();", "function f() {} f();");
    f.test("function f([a], b) {} f();", "function f() {} f();");
    f.test("function f([a], ...b) {} f();", "function f() {} f();");
    f.test_same("function f([...a]) {} f();");
    f.test("function f([[]]) {} f();", "function f([[]]) {} f();");
    f.test_same("function f([[],...a]) {} f();");
    f.test("var [a] = [];", "var [] = [];");
    f.test_same("var [...a] = [];");
    f.test_same("var [...[...a]] = [];");
    f.test("var [a, b] = [];", "var [] = [];");
    f.test("var [a, ...b] = [];", "var [ , ...b] = [];");
    f.test("var [a, ...[...b]] = [];", "var [ ,...[...b]] = [];");
    f.test(
        "var [a, b, c] = []; use(a, b);",
        "var [a,b  ] = []; use(a, b);",
    );
    f.test(
        "var [a, b, c] = []; use(a, c);",
        "var [a, ,c] = []; use(a, c);",
    );
    f.test(
        "var [a, b, c] = []; use(b, c);",
        "var [ ,b,c] = []; use(b, c);",
    );
    f.test("var [a, b, c] = []; use(a);", "var [a] = []; use(a);");
    f.test("var [a, b, c] = []; use(b);", "var [ ,b, ] = []; use(b);");
    f.test("var [a, b, c] = []; use(c);", "var [ , ,c] = []; use(c);");
    f.test(
        "var a, b, c; [a, b, c] = []; use(a);",
        "var a; [a] = []; use(a);",
    );
    f.test(
        "var a, b, c; [a, b, c] = []; use(b);",
        "var b; [ ,b, ] = []; use(b);",
    );
    f.test(
        "var a, b, c; [a, b, c] = []; use(c);",
        "var c; [ , ,c] = []; use(c);",
    );
    f.test(
        "var a, b, c; [[a, b, c]] = []; use(a);",
        "var a; [[a]] = []; use(a);",
    );
    f.test(
        "var a, b, c; [{a, b, c}] = []; use(a);",
        "var a; [{a}] = []; use(a);",
    );
    f.test(
        "var a, b, c; ({x:[a, b, c]} = []); use(a);",
        "var a; ({x:[a]} = []); use(a);",
    );
    f.test(
        "var a, b, c; [[a, b, c]] = []; use(b);",
        "var b; [[ ,b]] = []; use(b);",
    );
    f.test(
        "var a, b, c; [[a, b, c]] = []; use(c);",
        "var c; [[ , ,c]] = []; use(c);",
    );
    f.test(
        "var a, b, c; [a, b, ...c] = []; use(a);",
        "var a, c; [a,  , ...c] = []; use(a);",
    );
    f.test(
        "var a, b, c; [a, b, ...c] = []; use(b);",
        "var b, c; [ , b, ...c] = []; use(b);",
    );
    f.test(
        "var a, b, c; [a, b, ...c] = []; use(c);",
        "var c;    [ ,  , ...c] = []; use(c);",
    );
    f.test(
        "var a, b, c; [a=1, b=2, c=3] = []; use(a);",
        "var a; [a=1] = []; use(a);",
    );
    f.test(
        "var a, b, c; [a=1, b=2, c=3] = []; use(b);",
        "var b; [   ,b=2, ] = []; use(b);",
    );
    f.test(
        "var a, b, c; [a=1, b=2, c=3] = []; use(c);",
        "var c; [   ,   ,c=3] = []; use(c);",
    );
    f.test_same("var a, b, c; [a.x, b.y, c.z] = []; use(a);");
    // unnecessary retention of b,c
    f.test_same("var a, b, c; [a.x, b.y, c.z] = []; use(b);");
    // unnecessary retention of a,c
    f.test_same("var a, b, c; [a.x, b.y, c.z] = []; use(c);");
    // unnecessary retention of a,b
    f.test_same("var a, b, c; [a().x, b().y, c().z] = []; use(a);");
    f.test_same("var a, b, c; [a().x, b().y, c().z] = []; use(b);");
    f.test_same("var a, b, c; [a().x, b().y, c().z] = []; use(c);");
}

// port: RemoveUnusedCodeTest#testDestructuringArrayPattern1
#[test]
fn test_destructuring_array_pattern1() {
    let mut f = Fixture::new();
    f.test(
        r#"var a; var b
[a, b] = [1, 2]
"#,
        "[] = [1, 2]",
    );
    f.test(
        r#"var b; var a
[a, b] = [1, 2]
"#,
        "[] = [1, 2]",
    );
    f.test(
        r#"var a; var b;
[a] = [1]
"#,
        "[] = [1]",
    );
    f.test_same("var [a, b] = [1, 2]; alert(a); alert(b);");
}

// port: RemoveUnusedCodeTest#testDestructuringArrayPatternImpureRhs
#[test]
fn test_destructuring_array_pattern_impure_rhs() {
    let mut f = Fixture::new();
    f.test(
        "const [ok] = validate(userInput);",
        "const [] = validate(userInput);",
    );
    f.test("var [a, b, c] = gen(); use(a);", "var [a] = gen(); use(a);");
    f.test(
        "var a, b, c; [a, b, c] = gen(); use(a);",
        "var a; [a] = gen(); use(a);",
    );
    f.test(
        "var [a, b, c] = [1, 2, 3]; use(a);",
        "var [a] = [1, 2, 3]; use(a);",
    );
}

// port: RemoveUnusedCodeTest#testDestructuringObjectPattern0
#[test]
fn test_destructuring_object_pattern0() {
    let mut f = Fixture::new();
    f.test("function f({a}) {} f();", "function f() {} f();");
    f.test("function f({a:b}) {} f();", "function f() {} f();");
    f.test("function f({[a]:b}) {} f();", "function f() {} f();");
    f.test("function f({['a']:b}) {} f();", "function f() {} f();");
    f.test_same("function f({[alert()]:b}) {} f();");
    f.test("function f({a} = {}) {} f();", "function f() {} f();");
    f.test("function f({a:b} = {}) {} f();", "function f() {} f();");
    f.test_same("function f({[alert()]:b} = {}) {} f();");
    f.test(
        "function f({a} = alert()) {} f();",
        "function f({} = alert()) {} f();",
    );
    f.test(
        "function f({a:b} = alert()) {} f();",
        "function f({} = alert()) {} f();",
    );
    f.test_same("function f({[alert()]:b} = alert()) {} f();");
    f.test("function f({a = 1}) {} f();", "function f() {} f();");
    f.test("function f({a:b = 1}) {} f();", "function f() {} f();");
    f.test("function f({[a]:b = 1}) {} f();", "function f() {} f();");
    f.test("function f({['a']:b = 1}) {} f();", "function f() {} f();");
    f.test_same("function f({[alert()]:b = 1}) {} f();");
    // fix me, remove "= 1"
    f.test("function f({a = 1}) {} f();", "function f() {} f();");
    f.test_same("function f({a:b = alert()}) {} f();");
    f.test_same("function f({[externVar]:b = alert()}) {} f();");
    f.test_same("function f({['a']:b = alert()}) {} f();");
    f.test_same("function f({[alert()]:b = alert()}) {} f();");
    f.test(
        "function f({a}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test(
        "function f({a:b}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test(
        "function f({[externVar]:b}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test(
        "function f({['a']:b}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test_same("function f({[use()]:b}, c) {use(c)} f();");
    f.test(
        "function f({a} = {}, c) {use(c)} f();",
        "function f({} = {}, c) {use(c)} f();",
    );
    f.test(
        "function f({a:b} = {}, c) {use(c)} f();",
        "function f({} = {}, c) {use(c)} f();",
    );
    f.test_same("function f({[use()]:b} = {}, c) {use(c)} f();");
    // preserve alignment
    f.test(
        "function f({a} = use(), c) {use(c)} f();",
        "function f({ } = use(), c) {use(c)} f();",
    );
    // preserve alignment
    f.test(
        "function f({a:b} = use(), c) {use(c)} f();",
        "function f({   } = use(), c) {use(c)} f();",
    );
    f.test_same("function f({[use()]:b} = use(), c) {use(c)} f();");
    f.test(
        "function f({a = 1}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test(
        "function f({a:b = 1}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test(
        "function f({[a]:b = 1}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test(
        "function f({['a']:b = 1}, c) {use(c)} f();",
        "function f({}, c) {use(c)} f();",
    );
    f.test_same("function f({[use()]:b = 1}, c) {use(c)} f();");
    // fix me, remove "= 1"
    f.test("var {a} = {a:1};", "var {} = {a:1};");
    f.test("var {a:a} = {a:1};", "var {} = {a:1};");
    f.test("var {['a']:a} = {a:1};", "var {} = {a:1};");
    f.test("var {['a']:a = 1} = {a:1};", "var {} = {a:1};");
    f.test_same("var {[alert()]:a = 1} = {a:1};");
    f.test("var {a:a = 1} = {a:1};", "var {} = {a:1};");
    f.test_same("var {a:a = alert()} = {a:1};");
    f.test("var a; ({a} = {a:1});", "({} = {a:1});");
    f.test("var a; ({a:a} = {a:1});", "({} = {a:1});");
    f.test("var a; ({['a']:a} = {a:1});", "({} = {a:1});");
    f.test("var a; ({['a']:a = 1} = {a:1});", "({} = {a:1});");
    f.test_same("var a; ({[alert()]:a = 1} = {a:1});");
    f.test("var a; ({a:a = 1} = {a:1});", "({} = {a:1});");
    f.test_same("var a; ({a:a = alert()} = {a:1});");
    f.test_same("var a = {}; ({a:a.foo} = {a:1});");
    f.test_same("var a = {}; ({['a']:a.foo} = {a:1});");
    f.test_same("var a = {}; ({['a']:a.foo = 1} = {a:1});");
    f.test_same("var a = {}; ({[alert()]:a.foo = 1} = {a:1});");
    f.test_same("var a = {}; ({a:a.foo = 1} = {a:1});");
    f.test_same("var a = {}; ({a:a.foo = alert()} = {a:1});");
    f.test_same("function a() {} ({a:a().foo} = {a:1});");
    f.test_same("function a() {} ({['a']:a().foo} = {a:1});");
    f.test_same("function a() {} ({['a']:a().foo = 1} = {a:1});");
    f.test_same("function a() {} ({[alert()]:a().foo = 1} = {a:1});");
    f.test_same("function a() {} ({a:a().foo = 1} = {a:1});");
    f.test_same("function a() {} ({a:a().foo = alert()} = {a:1});");
}

// port: RemoveUnusedCodeTest#testDestructuringObjectPattern1
#[test]
fn test_destructuring_object_pattern1() {
    let mut f = Fixture::new();
    f.test("var a; var b; ({a, b} = {a:1, b:2})", "({} = {a:1, b:2});");
    f.test(
        "var a; var b; ({a:a, b:b} = {a:1, b:2})",
        "({} = {a:1, b:2});",
    );
    f.test_same("var a; var b; ({[alert()]:a, [alert()]:b} = {x:1, y:2})");
    f.test("var a; var b; ({a} = {a:1})", "({} = {a:1})");
    f.test(
        "var a; var b; ({a:a.foo} = {a:1})",
        "var a; ({a:a.foo} = {a:1})",
    );
    f.test_same("var {a, b} = {a:1, b:2}; alert(a); alert(b);");
    f.test_same("var {a} = {p:{}, q:{}}; a.q = 4;");
    // Nested Destructuring
    f.test(
        "const someObject = {a:{ b:1 }}; var {a: {b}} = someObject; someObject.a.b;",
        "const someObject = {a:{ b:1 }}; var {a: {}} = someObject; someObject.a.b;",
    );
    f.test_same("const someObject = {a:{ b:1 }}; var {a: {b}} = someObject; b;");
}

// port: RemoveUnusedCodeTest#testRemoveUnusedVarsDeclaredInDestructuring0
#[test]
fn test_remove_unused_vars_declared_in_destructuring0() {
    let mut f = Fixture::new();
    // Array destructuring
    f.test(
        "var [a, b] = [1, 2]; alert(a);",
        "var [a] = [1, 2]; alert(a);",
    );
    f.test("var [a, b] = [1, 2];", "var [] = [1, 2];");
    // Object pattern destructuring
    // comment to keep before/after alignment
    f.test(
        "var {a, b} = {a:1, b:2}; alert(a);",
        "var {a,  } = {a:1, b:2}; alert(a);",
    );
    // comment to keep before/after alignment
    f.test("var {a, b} = {a:1, b:2};", "var {    } = {a:1, b:2};");
}

// port: RemoveUnusedCodeTest#testRemoveUnusedVarsDeclaredInDestructuring1
#[test]
fn test_remove_unused_vars_declared_in_destructuring1() {
    let mut f = Fixture::new();
    // Nested pattern
    f.test(
        "var {a, b:{c}} = {a:1, b:{c:5}}; alert(a);",
        "var {a, b:{ }} = {a:1, b:{c:5}}; alert(a);",
    );
    f.test_same("var {a, b:{c}} = {a:1, b:{c:5}}; alert(a, c);");
    f.test("var {a, b:{c}} = externVar;", "var {b:{}} = externVar;");
    // Value may have side effects
    // preserve before / after alignment
    f.test(
        "var {a, b} = {a:alert(), b:alert()};",
        "var {    } = {a:alert(), b:alert()};",
    );
    // preserve before / after alignment
    f.test("var {a, b} = alert();", "var {    } = alert();");
    // Same as above case without the destructuring declaration
    f.test("var a, b = alert();", "alert();");
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_global_untyped
#[test]
fn test_remove_unused_polyfills_global_untyped() {
    let mut f = Fixture::new();
    remove_unused_polyfills_global_untyped(&mut f);
}

/// The body of `testRemoveUnusedPolyfills_global_untyped`, which `testRemoveUnusedPolyfills_global_typed` also runs after `enableTypeCheck()`.
fn remove_unused_polyfills_global_untyped(f: &mut Fixture) {
    let map_polyfill = "$jscomp.polyfill('Map', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_extra(&[JsString::from(JSCOMP_POLYFILL)])
                .add_map()
                .build()
                .to_string(),
        )
        .add_polyfill(map_polyfill);
    // unused polyfill is removed
    tester.expect_polyfills_removed_test(f, "console.log()", &[map_polyfill]);
    // used polyfill is not removed
    tester.expect_no_removal_test(f, "console.log(new Map())");
    // Local names shadowing global polyfills are not themselves polyfill references.
    tester.expect_polyfills_removed_test(
        f,
        r#"console.log(function(Map$jscomp$1) {
  console.log(new Map$jscomp$1());
});
"#,
        &[map_polyfill],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_propertyOfGlobalObject_untyped
#[test]
fn test_remove_unused_polyfills_property_of_global_object_untyped() {
    let mut f = Fixture::new();
    remove_unused_polyfills_property_of_global_object_untyped(&mut f);
}

/// The body of `testRemoveUnusedPolyfills_propertyOfGlobalObject_untyped`, which `testRemoveUnusedPolyfills_propertyOfGlobalObject_typed` also runs after `enableTypeCheck()`.
fn remove_unused_polyfills_property_of_global_object_untyped(f: &mut Fixture) {
    let map_polyfill = "$jscomp.polyfill('Map', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("/** @const */ var goog = {};"),
                    JsString::from("/** @const {!Global} */ goog.global;"),
                    JsString::from("/** @const */ goog.structs = {};"),
                    JsString::from("/** @constructor */ goog.structs.Map = function() {};"),
                    JsString::from("/** @type {!Global} */ var someGlobal;"),
                    JsString::from("/** @const */ var notGlobal = {};"),
                    JsString::from("/** @constructor */ notGlobal.Map = function() {};"),
                    JsString::from(""),
                ])
                .add_map()
                .build()
                .to_string(),
        )
        .add_polyfill(map_polyfill);
    // Global polyfills may be accessed as properties on the global object.
    tester.expect_no_removal_test(f, "console.log(new goog.global.Map());");
    // NOTE: Without type information we don't know whether a property access is on the global
    // object or not.
    tester.expect_no_removal_test(f, "console.log(new goog.structs.Map());");
    tester.expect_no_removal_test(f, "console.log(new notGlobal.Map());");
    tester.expect_no_removal_test(
        f,
        r#"var x = {Map: /** @constructor */ function() {}};
console.log(new x.Map());
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_propertyOfGlobalObject_withOptionalChain
#[test]
fn test_remove_unused_polyfills_property_of_global_object_with_optional_chain() {
    let mut f = Fixture::new();
    let promise_polyfill = "$jscomp.polyfill('Promise', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_promise()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("/** @const */ var goog = {};"),
                ])
                .build()
                .to_string(),
        )
        .add_polyfill(promise_polyfill);
    // The optional chain here isn't guarding against Promise being undefined,
    // just the object on which it is referenced.
    tester.expect_no_removal_test(&mut f, "console.log(goog.global?.Promise.resolve());");
    // The optional chain here _is_ guarding against Promise being undefined.
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(goog.global.Promise?.resolve());",
        &[promise_polyfill],
    );
    // The optional chain here _is_ guarding against Promise being undefined,
    // in addition to the object on which it is referenced.
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(goog.global?.Promise?.resolve());",
        &[promise_polyfill],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_staticProperty_untyped
#[test]
fn test_remove_unused_polyfills_static_property_untyped() {
    let mut f = Fixture::new();
    remove_unused_polyfills_static_property_untyped(&mut f);
}

/// The body of `testRemoveUnusedPolyfills_staticProperty_untyped`, which `testRemoveUnusedPolyfills_staticProperty_typed` also runs after `enableTypeCheck()`.
fn remove_unused_polyfills_static_property_untyped(f: &mut Fixture) {
    let array_dot_from_polyfill = "$jscomp.polyfill('Array.from', function() {}, 'es6', 'es3');";
    // NOTE: this is not a real API but it allows testing that we can tell it
    // apart.
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_array()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("/** @constructor */ function Set() {}"),
                    JsString::from("Set.from = function() {};"),
                    JsString::from("/** @const */ var goog = {};"),
                    JsString::from("/** @const {!Global} */ goog.global;"),
                    JsString::from(""),
                ])
                .build()
                .to_string(),
        )
        .add_polyfill(array_dot_from_polyfill);
    // Used polyfill is retained.
    tester.expect_no_removal_test(f, "console.log(Array.from([]));");
    // Used polyfill is retained, even if accessed as a property of global.
    tester.expect_no_removal_test(f, "console.log(goog.global.Array.from([]));");
    // Unused polyfill is not removed if there is another static property with the same name
    tester.expect_no_removal_test(
        f,
        r#"class NotArray { static from() {} }
console.log(NotArray.from());
"#,
    );
    // Without type information, we can't correctly remove this polyfill.
    tester.expect_no_removal_test(
        f,
        r#"var x = {Array: {from: function() {}}};
console.log(x.Array.from());
"#,
    );
    // Used polyfill via aliased owner: retains definition.
    tester.expect_no_removal_test(
        f,
        r#"/** @const */ var MyArray = Array;
// polyfill is kept even though called via an alias
console.log(MyArray.from([]));
"#,
    );
    // Used polyfill via subclass: retains definition.
    tester.expect_no_removal_test(
        f,
        r#"class SubArray extends Array {}
// polyfill is kept even though called via a subclass.
console.log(SubArray.from([]));
"#,
    );
    // Cannot distinguish between Set.from and Array.from,
    // so the Set.from polyfill will also be kept.
    let set_dot_from_polyfill = "$jscomp.polyfill('Set.from', function() {}, 'es6', 'es3');";
    tester.add_polyfill(set_dot_from_polyfill);
    tester.expect_no_removal_test(f, "console.log(Array.from([]));");
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_staticProperty_withOptionalChain
#[test]
fn test_remove_unused_polyfills_static_property_with_optional_chain() {
    let mut f = Fixture::new();
    let promise_polyfill = "$jscomp.polyfill('Promise', function() {}, 'es6', 'es3');";
    let all_settled_polyfill =
        "$jscomp.polyfill('Promise.allSettled', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_promise()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("/** @const */ var goog = {};"),
                ])
                .build()
                .to_string(),
        )
        .add_polyfill(promise_polyfill)
        .add_polyfill(all_settled_polyfill);
    tester.expect_no_removal_test(&mut f, "console.log(Promise.allSettled());");
    // neither guarded
    // `?.` guards the name on its left, allowing it to be removed
    // allSettled guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(Promise.allSettled?.());",
        &[all_settled_polyfill],
    );
    // allSettled guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(Promise.allSettled?.(Promise.allSettled));",
        &[all_settled_polyfill],
    );
    // TODO(b/163394833): Note that this behavior could result in the Promise polyfill being
    // removed, then the Promise.allSettled polyfill code having nowhere to hang its value.
    // At runtime the `$jscomp.polyfill('Promise.allSettled',...)` call will silently fail
    // to create the polyfill if `Promise` isn't defined.
    // This is consistent with the way guarding with `&&` would behave, as demonstrated by the
    // following test case.
    // Promise guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(Promise?.allSettled());",
        &[promise_polyfill],
    );
    // Promise guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(Promise && Promise.allSettled());",
        &[promise_polyfill],
    );
    // both guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(Promise?.allSettled?.([Promise.allSettled]));",
        &[promise_polyfill, all_settled_polyfill],
    );
    // Access via a (potentially) global variable should behave the same way, even if the
    // global is referenced via optional chaining
    tester.expect_no_removal_test(&mut f, "console.log(goog.global?.Promise.allSettled());");
    // neither guarded
    // allSettled guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(goog.global?.Promise.allSettled?.());",
        &[all_settled_polyfill],
    );
    // TODO(b/163394833): Note that this behavior could result in the Promise polyfill being
    // removed, then the Promise.allSettled polyfill code having nowhere to hang its value.
    // At runtime the `$jscomp.polyfill('Promise.allSettled',...)` call will silently fail
    // to create the polyfill if `Promise` isn't defined.
    // This is consistent with the way guarding with `&&` would behave, as demonstrated by the
    // following test case.
    // Promise guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(goog.global?.Promise?.allSettled());",
        &[promise_polyfill],
    );
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(goog.global && goog.global.Promise && goog.global.Promise.allSettled());",
        &[promise_polyfill],
    );
    // both guarded
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log(goog.global?.Promise?.allSettled?.());",
        &[promise_polyfill, all_settled_polyfill],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_prototypeProperty_untyped
#[test]
fn test_remove_unused_polyfills_prototype_property_untyped() {
    let mut f = Fixture::new();
    remove_unused_polyfills_prototype_property_untyped(&mut f);
}

/// The body of `testRemoveUnusedPolyfills_prototypeProperty_untyped`, which `testRemoveUnusedPolyfills_prototypeProperty_typed` also runs after `enableTypeCheck()`.
fn remove_unused_polyfills_prototype_property_untyped(f: &mut Fixture) {
    let string_repeat_polyfill =
        "$jscomp.polyfill('String.prototype.repeat', function() {}, 'es6', 'es3');";
    //
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_array()
                .add_console()
                .add_string()
                .add_function()
                .add_alert()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("function externFunction() {}"),
                    JsString::from(""),
                ])
                .build()
                .to_string(),
        )
        .add_polyfill(string_repeat_polyfill);
    // Unused polyfill is removed.
    tester.expect_polyfills_removed_test(f, "console.log();", &[string_repeat_polyfill]);
    // Used polyfill is retained.
    tester.expect_no_removal_test(f, "''.repeat(1);");
    tester.expect_no_removal_test(f, "''?.repeat(1);");
    // Guarded polyfill is removed.
    tester.expect_polyfills_removed_test(f, "''.repeat?.(1);", &[string_repeat_polyfill]);
    // Used polyfill (directly via String.prototype) is retained.
    tester.expect_no_removal_test(f, "String.prototype.repeat(1);");
    tester.expect_no_removal_test(f, "String.prototype?.repeat(1);");
    tester.expect_polyfills_removed_test(
        f,
        "String.prototype.repeat?.(1);",
        &[string_repeat_polyfill],
    );
    // Used polyfill (directly String.prototype and Function.prototype.call) is retained.
    tester.expect_no_removal_test(f, "String.prototype.repeat.call('', 1);");
    tester.expect_no_removal_test(f, "String.prototype.repeat.call?.('', 1);");
    tester.expect_polyfills_removed_test(
        f,
        "String.prototype.repeat?.call('', 1);",
        &[string_repeat_polyfill],
    );
    // Unused polyfill is not removed if there is another property with the same name on an unknown
    // type
    tester.expect_no_removal_test(
        f,
        r#"var x = externFunction();
x.repeat();
"#,
    );
    tester.expect_polyfills_removed_test(
        f,
        r#"var x = externFunction();
x.repeat?.();
"#,
        &[string_repeat_polyfill],
    );
    // Without type information, cannot remove the polyfill.
    tester.expect_no_removal_test(
        f,
        r#"class Repeatable {
  static repeat() {}
};
Repeatable.repeat();
"#,
    );
    // Without type information, cannot remove the polyfill.
    tester.expect_no_removal_test(
        f,
        r#"class Repeatable {
  repeat() {}
};
var x = new Repeatable();
x.repeat();
"#,
    );
    // Multiple same-name methods
    let string_includes_polyfill =
        "$jscomp.polyfill('String.prototype.includes', function() {}, 'es6', 'es3');";
    let array_includes_polyfill =
        "$jscomp.polyfill('Array.prototype.includes', function() {}, 'es6', 'es3');";
    //
    tester
        .add_polyfill(string_includes_polyfill)
        .add_polyfill(array_includes_polyfill);
    // The unused `String.prototype.repeat` polyfill is removed, but both of the
    // `(Array|String).prototype.includes` polyfills are kept, since we aren't using type
    // information to recognize which one is being called.
    tester.expect_polyfills_removed_test(f, "[].includes(5);", &[string_repeat_polyfill]);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_globalWithPrototypePolyfill_untyped
#[test]
fn test_remove_unused_polyfills_global_with_prototype_polyfill_untyped() {
    let mut f = Fixture::new();
    remove_unused_polyfills_global_with_prototype_polyfill_untyped(&mut f);
}

/// The body of `testRemoveUnusedPolyfills_globalWithPrototypePolyfill_untyped`, which `testRemoveUnusedPolyfills_globalWithPrototypePolyfill_typed` also runs after `enableTypeCheck()`.
fn remove_unused_polyfills_global_with_prototype_polyfill_untyped(f: &mut Fixture) {
    let promise_polyfill = "$jscomp.polyfill('Promise', function() {}, 'es6', 'es3');";
    let finally_polyfill =
        "$jscomp.polyfill('Promise.prototype.finally', function() {}, 'es8', 'es3');";
    //
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_promise()
                .add_console()
                .add_extra(&[JsString::from(JSCOMP_POLYFILL)])
                .build()
                .to_string(),
        )
        .add_polyfill(promise_polyfill)
        .add_polyfill(finally_polyfill);
    // Both the base polyfill and the extra method are removed when unused.
    tester.expect_polyfills_removed_test(
        f,
        "console.log();",
        &[promise_polyfill, finally_polyfill],
    );
    // The extra method polyfill is removed if not used, even when the base is retained.
    tester.expect_polyfills_removed_test(f, "console.log(Promise.resolve());", &[finally_polyfill]);
    // Promise is guarded by an optional chain.
    tester.expect_polyfills_removed_test(
        f,
        "console.log(Promise?.resolve());",
        &[promise_polyfill, finally_polyfill],
    );
    // Can't remove finally without type information
    // NOTE: In reality the Promise.prototype.finally polyfill references Promise, so
    // it would actually prevent the Promise polyfill from being removed.
    tester.expect_polyfills_removed_test(
        f,
        r#"const p = {finally() {}};
p.finally();
"#,
        &[promise_polyfill],
    );
    // `finally` is guarded by an optional chain
    tester.expect_polyfills_removed_test(
        f,
        r#"const p = {finally() {}};
p.finally?.();
"#,
        &[promise_polyfill, finally_polyfill],
    );
    // Retain both the base and the extra method when both are used.
    tester.expect_no_removal_test(f, "console.log(Promise.resolve().finally(() => {}));");
    tester.expect_polyfills_removed_test(
        f,
        "console.log(Promise?.resolve()?.finally(() => {}));",
        &[promise_polyfill],
    );
    // finally is not guarded
    tester.expect_polyfills_removed_test(
        f,
        "console.log(Promise?.resolve().finally?.(() => {}));",
        &[promise_polyfill, finally_polyfill],
    );
    // both are guarded
    // TODO(b/163394833): Note that this behavior could result in the Promise polyfill being
    // removed, then the Promise.prototype.finally polyfill code having nowhere to hang its value.
    // This is consistent with the way guarding with `&&` would behave, as demonstrated by the
    // following test case.
    // NOTE: In reality the Promise.prototype.finally polyfill references Promise, so
    // it would actually prevent the Promise polyfill from being removed.
    tester.expect_polyfills_removed_test(
        f,
        "console.log(Promise?.resolve().finally(() => {}));",
        &[promise_polyfill],
    );
    // only Promise guarded by optional chain
    tester.expect_polyfills_removed_test(
        f,
        "console.log(Promise && Promise.resolve().finally(() => {}));",
        &[promise_polyfill],
    );
    // only Promise guarded by optional chain
    tester.expect_polyfills_removed_test(
        f,
        "console.log(Promise.resolve().finally?.(() => {}));",
        &[finally_polyfill],
    );
    // only `finally` guarded by optional chain
    // The base polyfill is removed and the extra method retained if the constructor never shows up
    // anywhere in the source code.  NOTE: this is probably the wrong thing to do.  This situation
    // should only be possible if async function transpilation happens *after* RemoveUnusedCode
    // (since we have an async function).  The fact that we inserted the Promise polyfill in the
    // first case indicates the output language is < ES6.  When that later transpilation occurs, we
    // will end up adding uses of the Promise constructor.  We need to keep this in mind when moving
    // transpilation after optimizations.
    tester.expect_polyfills_removed_test(
        f,
        r#"async function f() {}
f().finally(() => {});
"#,
        &[promise_polyfill],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_typedArrayMethod_untyped
#[test]
fn test_remove_unused_polyfills_typed_array_method_untyped() {
    let mut f = Fixture::new();
    let typed_array_at_polyfill =
        "$jscomp.polyfillTypedArrayMethod('at', function() {}, 'es_2022', 'es5');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("/** @constructor */ function Int8Array() {}"),
                    JsString::from(
                        "/** @type {function(number):number} */ Int8Array.prototype.at;",
                    ),
                    JsString::from("/** @constructor */ function Float32Array() {}"),
                    JsString::from(
                        "/** @type {function(number):number} */ Float32Array.prototype.at;",
                    ),
                    JsString::from("/** @constructor */ function BigInt64Array() {}"),
                    JsString::from(
                        "/** @type {function(number):number} */ BigInt64Array.prototype.at;",
                    ),
                ])
                .build()
                .to_string(),
        )
        .add_polyfill(typed_array_at_polyfill);
    // unused polyfill is removed
    tester.expect_polyfills_removed_test(
        &mut f,
        "console.log('.at() is not called')",
        &[typed_array_at_polyfill],
    );
    // used polyfill is not removed - usage via one TypedArray type is sufficient to keep it
    tester.expect_no_removal_test(&mut f, "console.log(new Int8Array().at(0))");
    tester.expect_no_removal_test(&mut f, "console.log(new Float32Array().at(0))");
    tester.expect_no_removal_test(&mut f, "console.log(new BigInt64Array().at(0))");
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_chained
#[test]
fn test_remove_unused_polyfills_chained() {
    let mut f = Fixture::new();
    let weak_map_polyfill =
        "$jscomp.polyfill('WeakMap', function() { console.log(); }, 'es6', 'es3');";
    // Polyfill of Map depends on WeakMap
    let map_polyfill = "$jscomp.polyfill('Map', function() { new WeakMap(); }, 'es6', 'es3');";
    // Polyfill of Set depends on Map
    let set_polyfill = "$jscomp.polyfill('Set', function() { new Map(); }, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_map()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("/** @constructor */ function Set() {}"),
                    JsString::from("/** @constructor */ function WeakMap() {}"),
                ])
                .build()
                .to_string(),
        )
        .add_polyfill(weak_map_polyfill)
        .add_polyfill(map_polyfill)
        .add_polyfill(set_polyfill);
    // Removes polyfills that are only referenced in other (removed) polyfills' definitions.
    // Unused method gets removed, allowing all 3 polyfills to be removed.
    tester
        .input_source(
            r#"function unused() { new Set(); }
console.log();
"#,
        )
        .expect_source("console.log();")
        .expect_polyfills_removed(&[weak_map_polyfill, map_polyfill, set_polyfill])
        .test(&mut f);
    // Chains can be partially removed if just an outer-most symbol is unreferenced.
    tester.expect_polyfills_removed_test(&mut f, "console.log(new Map());", &[set_polyfill]);
    // Only requires a single reference to the outermost symbol to retain the whole chain.
    tester.expect_no_removal_test(&mut f, "console.log(new Set())");
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_continued
#[test]
fn test_remove_unused_polyfills_continued() {
    let mut f = Fixture::new();
    let externs = CompilerTestCase::externs(
        TestExternsBuilder::new()
            .add_console()
            .add_map()
            .add_extra(&[JsString::from(JSCOMP_POLYFILL)])
            .build(),
    );
    // Ensure that continuations occur so that retained polyfill definitions are still optimized.
    f.test_parts(vec![
        TestPart::Externs(externs.clone()),
        TestPart::Sources(CompilerTestCase::srcs(
            r#"$jscomp.polyfill('Map', function() { var x; }, 'es6', 'es3');
console.log(new Map());
"#,
        )),
        TestPart::Expected(CompilerTestCase::expected(
            r#"$jscomp.polyfill('Map', function() {        }, 'es6', 'es3');
console.log(new Map());
"#,
        )),
    ]);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_collapsedPolyfillFunction
#[test]
fn test_remove_unused_polyfills_collapsed_polyfill_function() {
    let mut f = Fixture::new();
    let externs = CompilerTestCase::externs(
        TestExternsBuilder::new()
            .add_console()
            .add_map()
            .add_extra(&[JsString::from("function $jscomp$polyfill() {}")])
            .build(),
    );
    // The pass should also work after CollapseProperties.
    f.test_parts(vec![
        TestPart::Externs(externs.clone()),
        TestPart::Sources(CompilerTestCase::srcs(
            r#"$jscomp$polyfill('Map', function() {}, 'es6', 'es3');
console.log();
"#,
        )),
        TestPart::Expected(CompilerTestCase::expected("console.log();")),
    ]);
    f.test_same_parts(vec![
        TestPart::Externs(externs.clone()),
        TestPart::Sources(CompilerTestCase::srcs(
            r#"$jscomp$polyfill('Map', function() {}, 'es6', 'es3');
console.log(new Map());
"#,
        )),
    ]);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_guardedGlobals
#[test]
fn test_remove_unused_polyfills_guarded_globals() {
    let mut f = Fixture::new();
    let map_polyfill = "$jscomp.polyfill('Map', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_map()
                .add_extra(&[
                    JsString::from(JSCOMP_POLYFILL),
                    JsString::from("/** @constructor */ function Promise() {}"),
                ])
                .build()
                .to_string(),
        )
        .add_polyfill(map_polyfill);
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"if (typeof Map !== 'undefined') {
  console.log(Map);
}
"#,
        &[map_polyfill],
    );
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"if (Map) {
  console.log(Map);
}
"#,
        &[map_polyfill],
    );
    let promise_polyfill = "$jscomp.polyfill('Promise', function() {}, 'es6', 'es3');";
    tester.add_polyfill(promise_polyfill);
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"if (typeof Map !== 'undefined') {
  console.log(Map);
  console.log(Promise);
}
"#,
        &[map_polyfill],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_guardedStatics
#[test]
fn test_remove_unused_polyfills_guarded_statics() {
    let mut f = Fixture::new();
    let array_from_polyfill = "$jscomp.polyfill('Array.from', function() {}, 'es6', 'es3');";
    let promise_polyfill = "$jscomp.polyfill('Promise', function() {}, 'es6', 'es3');";
    let all_settled_polyfill =
        "$jscomp.polyfill('Promise.allSettled', function() {}, 'es8', 'es3');";
    let symbol_polyfill = "$jscomp.polyfill('Symbol', function() {}, 'es6', 'es3');";
    let symbol_iterator_polyfill =
        "$jscomp.polyfill('Symbol.iterator', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_array()
                .add_promise()
                .add_extra(&[JsString::from(JSCOMP_POLYFILL)])
                .build()
                .to_string(),
        )
        .add_polyfill(array_from_polyfill)
        .add_polyfill(promise_polyfill)
        .add_polyfill(all_settled_polyfill)
        .add_polyfill(symbol_polyfill)
        .add_polyfill(symbol_iterator_polyfill);
    // guarded & all others unused
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"if (typeof Array.from !== 'undefined') {
  console.log(Array.from);
}
"#,
        &[
            array_from_polyfill,
            promise_polyfill,
            all_settled_polyfill,
            symbol_polyfill,
            symbol_iterator_polyfill,
        ],
    );
    // guarded
    // guarded
    // this and following unused
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"var a;
if (Promise && Promise.allSettled) {
  Promise.allSettled(a);
}
"#,
        &[
            promise_polyfill,
            all_settled_polyfill,
            array_from_polyfill,
            symbol_polyfill,
            symbol_iterator_polyfill,
        ],
    );
    // guarded
    // guarded
    // this and following unused
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"if (Symbol && Symbol.iterator) {
  console.log(Symbol.iterator);
}
"#,
        &[
            symbol_polyfill,
            symbol_iterator_polyfill,
            array_from_polyfill,
            promise_polyfill,
            all_settled_polyfill,
        ],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_guardedMethods
#[test]
fn test_remove_unused_polyfills_guarded_methods() {
    let mut f = Fixture::new();
    let array_find_polyfill =
        "$jscomp.polyfill('Array.prototype.find', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_array()
                .add_extra(&[JsString::from(JSCOMP_POLYFILL)])
                .build()
                .to_string(),
        )
        .add_polyfill(array_find_polyfill);
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"const arr = [];
if (typeof arr.find !== 'undefined') {
  console.log(arr.find(0));
}
"#,
        &[array_find_polyfill],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_unguardedAndGuarded
#[test]
fn test_remove_unused_polyfills_unguarded_and_guarded() {
    let mut f = Fixture::new();
    let map_polyfill = "$jscomp.polyfill('Map', function() {}, 'es6', 'es3');";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_map()
                .add_extra(&[JsString::from(JSCOMP_POLYFILL)])
                .build()
                .to_string(),
        )
        .add_polyfill(map_polyfill);
    // Map is not removed because it has an unguarded usage.
    tester.expect_no_removal_test(
        &mut f,
        r#"if (typeof Map == 'undefined') {
  console.log(Map);
}
console.log(Map);
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPatches
#[test]
fn test_remove_unused_patches() {
    let mut f = Fixture::new();
    let map_patch = "$jscomp.patch('Map', function() {});";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_extra(&[JsString::from(JSCOMP_PATCH)])
                .add_map()
                .build()
                .to_string(),
        )
        .add_polyfill(map_patch);
    // unused polyfill is removed
    tester.expect_polyfills_removed_test(&mut f, "console.log()", &[map_patch]);
    // used polyfill is not removed
    tester.expect_no_removal_test(&mut f, "console.log(new Map())");
    // Local names shadowing global polyfills are not themselves polyfill references.
    tester.expect_polyfills_removed_test(
        &mut f,
        r#"console.log(function(Map$jscomp$1) {
  console.log(new Map$jscomp$1());
});
"#,
        &[map_patch],
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPatches_guardedUsage
#[test]
fn test_remove_unused_patches_guarded_usage() {
    let mut f = Fixture::new();
    let map_patch = "$jscomp.patch('Map', function() {});";
    let mut tester = PolyfillRemovalTester::new();
    tester
        .add_externs(
            &TestExternsBuilder::new()
                .add_console()
                .add_extra(&[JsString::from(JSCOMP_PATCH)])
                .add_map()
                .build()
                .to_string(),
        )
        .add_polyfill(map_patch);
    // Map is not removed because it is a patch.
    tester.expect_no_removal_test(
        &mut f,
        r#"if (typeof Map == 'undefined') {
  console.log(Map);
}
"#,
    );
}

// port: RemoveUnusedCodeTest#testNoCatchBinding
#[test]
fn test_no_catch_binding() {
    let mut f = Fixture::new();
    f.test_same("function doNothing() {} try { doNothing(); } catch { doNothing(); }");
    f.test_same("function doNothing() {} try { throw 0; } catch { doNothing(); }");
    f.test_same("function doNothing() {} try { doNothing(); } catch { console.log('stuff'); }");
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetterAssignmentObject
#[test]
fn test_do_not_remove_setter_assignment_object() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {
  set property(x) {}
};
a.property = 1;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetterAssignmentClass
#[test]
fn test_do_not_remove_setter_assignment_class() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class Class {
  set property(x) {}
}
const a = new Class();
a.property = 1;
"#,
    );
    f.test_same(
        r#"class Class {
  set property(x) {}
}
new Class().property = 1;
"#,
    );
    f.test(
        r#"class Class {
  set property(x) {}
}
function foo() {}
foo().property = 1;
"#,
        r#"function foo() {}
foo().property = 1;
"#,
    );
    f.test(
        r#"class Class {
  set property(x) {}
}
var obj;
obj.property = 1;
"#,
        r#"var obj;
obj.property = 1;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveStaticSetterAssignment
#[test]
fn test_do_not_remove_static_setter_assignment() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class Class {
  static set property(x) {}
}
Class.property = 1;
"#,
    );
    f.test(
        r#"class Class {
  static set property(x) {}
}
function foo() {}
foo().property = 1;
"#,
        r#"function foo() {}
foo().property = 1;
"#,
    );
    f.test(
        r#"class Class {
  static set property(x) {}
}
var obj;
obj.property = 1;
"#,
        r#"var obj;
obj.property = 1;
"#,
    );
}

// port: RemoveUnusedCodeTest#testMethodCallingSetterHasSideEffects
#[test]
fn test_method_calling_setter_has_side_effects() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class Class {
  set property(x) {}
}
function foo() { new Class().property = 1; }
foo();
"#,
    );
    f.test_same(
        r#"class Class {
  static set property(x) {}
}
function foo() { Class.property = 1; }
foo();
"#,
    );
    f.test_same(
        r#"class Class {
  setProperty(v) { this.property = v; }
  set property(x) {}
}
new Class().setProperty(0);
"#,
    );
    f.test_same(
        r#"class Class {
  static setProperty(v) { this.property = v; }
  static set property(x) {}
}
Class.setProperty(0);
"#,
    );
    f.test_same(
        r#"class Class {
  setProperty(v) { Class.property = v; }
  static set property(x) {}
}
Class.setProperty(0);
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetter_fromObjectLiteral_inCompoundAssignment_onName
#[test]
fn test_do_not_remove_setter_from_object_literal_in_compound_assignment_on_name() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {
  set property(x) {}
};
a.property += 1;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetter_fromObjectLiteral_inCompoundAssignment_onThis
#[test]
fn test_do_not_remove_setter_from_object_literal_in_compound_assignment_on_this() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {
  set property(x) {},

  method() {
    this.property += 1;
 },
};
a.method();
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetterCompoundAssignmentClass
#[test]
fn test_do_not_remove_setter_compound_assignment_class() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class Class {
  set property(x) {}
}
const a = new Class();
a.property += 1;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetter_fromObjectLiteral_inUnaryOp_onName
#[test]
fn test_do_not_remove_setter_from_object_literal_in_unary_op_on_name() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {
  set property(x) {}
};
a.property++;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetter_fromObjectLiteral_inUnaryOp_onThis
#[test]
fn test_do_not_remove_setter_from_object_literal_in_unary_op_on_this() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {
  set property(x) {},

  method() {
    this.property++;
  },
};
a.method();
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveSetterUnaryOperatorClass
#[test]
fn test_do_not_remove_setter_unary_operator_class() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class Class {
  set property(x) {}
}
const a = new Class();
a.property++;
"#,
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveAssignmentIfOtherPropertyIsSetterObject
#[test]
fn test_do_not_remove_assignment_if_other_property_is_setter_object() {
    let mut f = Fixture::new();
    f.test(
        r#"var a = {
  set property(x) {}
};
var b = {
  property: 0
};
b.property = 1;
"#,
        r#"var b = {
  property: 0
};
b.property = 1;
"#,
    );
    // Test that this gets cleaned up on a second pass...
    f.test(
        r#"var b = {
  property: 0
};
b.property = 1;
"#,
        "",
    );
}

// port: RemoveUnusedCodeTest#testDoNotRemoveAssignmentIfOtherPropertyIsSetterClass
#[test]
fn test_do_not_remove_assignment_if_other_property_is_setter_class() {
    let mut f = Fixture::new();
    f.test(
        r#"class Class {
  set property(x) {}
}
var b = {
  property: 0
};
b.property = 1;
"#,
        r#"var b = {
  property: 0
};
b.property = 1;
"#,
    );
    // Test that this gets cleaned up on a second pass...
    f.test(
        r#"var b = {
  property: 0
};
b.property = 1;
"#,
        "",
    );
}

// port: RemoveUnusedCodeTest#testRemovePropertyUnrelatedFromSetterObject
#[test]
fn test_remove_property_unrelated_from_setter_object() {
    let mut f = Fixture::new();
    f.test(
        r#"var a = {
  set property(x) {},
  aUnrelated: 0
};
var b = {
  unrelated: 0
};
a.property = 1;
a.aUnrelated = 1;
b.unrelated = 1;
"#,
        r#"var a = {
  set property(x) {},
  aUnrelated: 0
};
a.property = 1;
a.aUnrelated = 1;
"#,
    );
}

// port: RemoveUnusedCodeTest#testFunctionCallReferencesGetterIsNotRemoved
#[test]
fn test_function_call_references_getter_is_not_removed() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {
  get property() {}
};
function foo() { a.property; }
foo();
"#,
    );
}

// port: RemoveUnusedCodeTest#testFunctionCallReferencesSetterIsNotRemoved
#[test]
fn test_function_call_references_setter_is_not_removed() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {
  set property(v) {}
};
function foo() { a.property = 0; }
foo();
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedGettersAndSetters
#[test]
fn test_remove_unused_getters_and_setters() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  get usedProperty() {}
  set usedProperty(v) {}
  get unUsedProperty() {}
  set unUsedProperty(v) {}
};
function foo() {
  const c = new C();
  c.usedProperty = 0;
  return c.usedProperty;
}
foo();
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemovalFromRHSOfComma
#[test]
fn test_removal_from_rhs_of_comma() {
    let mut f = Fixture::new();
    // This is the repro for github issue 3612
    f.test(
        r#"function a() {
    var a = {}, b = null;
    a.a = 1,
    b.a = 2, // Note the comma here.
    Object.defineProperties(a, b);
};
alert(a);
"#,
        r#"function a() {
};
alert(a);
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemovalFromRHSOfAND
#[test]
fn test_removal_from_rhs_of_and() {
    let mut f = Fixture::new();
    f.test(
        r#"function a() {
    var a = {};
    var CONDITION = true;
    CONDITION && Object.defineProperties(a, b);
};
alert(a);
"#,
        r#"function a() {
  var CONDITION = true;
  CONDITION;
};
alert(a);
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemovalFromRHSOfOR
#[test]
fn test_removal_from_rhs_of_or() {
    let mut f = Fixture::new();
    f.test(
        r#"function a() {
    var a = {};
    var CONDITION = true;
    CONDITION || Object.defineProperties(a, b);
};
alert(a);
"#,
        r#"function a() {
  var CONDITION = true;
  CONDITION;
};
alert(a);
"#,
    );
}

// port: RemoveUnusedCodeTest#testVariableAssignedToItselfInORNode
#[test]
fn test_variable_assigned_to_itself_in_or_node() {
    let mut f = Fixture::new();
    // Fix for b/359932022, where variable is assigned to itself in an OR node.
    // 1) the node to which a value is being assigned
    // 2) the value being assigned and the
    // Handle the case in an OR node,where both (1) and (2) are the same variable.
    f.test("var a = a || {}; a[\"hi\"] = true;", "");
    f.test("var a = {}; a[\"hi\"] = true;", "");
    // without the OR node
    // test RHS of OR is not removable
    f.test_same("var not_removable = 5; var a = a || not_removable ; a[\"hi\"] = true;");
}

// port: RemoveUnusedCodeTest#testRemovalFromExpression
#[test]
fn test_removal_from_expression() {
    let mut f = Fixture::new();
    f.test(
        r#"function a() {
    var a = {};
    var CONDITION = true;
    CONDITION ? Object.defineProperties(a, b) : 'something';
};
alert(a);
"#,
        r#"function a() {
  var CONDITION = true;
  CONDITION ? 0 : 'something';
};
alert(a);
"#,
    );
}

// port: RemoveUnusedCodeTest#testPreserveDestructuringWithObjectRest
#[test]
fn test_preserve_destructuring_with_object_rest() {
    let mut f = Fixture::new();
    f.test_same(
        r#"const obj = {'one': 1, 'two': 2};
const {['one']: unused, ...remaining} = obj;
console.log(remaining);
"#,
    );
}

// port: RemoveUnusedCodeTest#testArrayReferenceInForLoopNameDeclarations
#[test]
fn test_array_reference_in_for_loop_name_declarations() {
    let mut f = Fixture::new();
    f.test_same(
        r#"const arr=[];
for (let i = 0, ref = arr; i<5; i++)
  ref[i] = "test";
console.log(arr)
"#,
    );
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_global_typed
#[test]
fn test_remove_unused_polyfills_global_typed() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // Type information is no longer used to make decisions for polyfill removal.
    remove_unused_polyfills_global_untyped(&mut f);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_propertyOfGlobalObject_typed
#[test]
fn test_remove_unused_polyfills_property_of_global_object_typed() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // We no longer use type information to make polyfill removal decisions
    remove_unused_polyfills_property_of_global_object_untyped(&mut f);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_staticProperty_typed
#[test]
fn test_remove_unused_polyfills_static_property_typed() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // NOTE: We no longer use type information to make polyfill removal decisions.
    remove_unused_polyfills_static_property_untyped(&mut f);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_prototypeProperty_typed
#[test]
fn test_remove_unused_polyfills_prototype_property_typed() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // We no longer use type information to make polyfill removal decisions.
    remove_unused_polyfills_prototype_property_untyped(&mut f);
}

// port: RemoveUnusedCodeTest#testRemoveUnusedPolyfills_globalWithPrototypePolyfill_typed
#[test]
fn test_remove_unused_polyfills_global_with_prototype_polyfill_typed() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // We no longer use type information to make polyfill removal decisions.
    remove_unused_polyfills_global_with_prototype_polyfill_untyped(&mut f);
}
