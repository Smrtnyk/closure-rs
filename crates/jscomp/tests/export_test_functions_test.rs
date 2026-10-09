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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/ExportTestFunctionsTest.java.

//! Port of `ExportTestFunctionsTest.java`: tests for `ExportTestFunctions`.
use closure_jscomp::{compiler_pass::CompilerPass, export_test_functions::ExportTestFunctions};
use closure_rhino::fast_hash::IndexMap;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

// port: ExportTestFunctionsTest#EXTERNS
const EXTERNS: &str =
    "function google_exportSymbol(a, b) {}; function google_exportProperty(a, b, c) {};";

// port: ExportTestFunctionsTest#TEST_FUNCTIONS_WITH_NAMES
const TEST_FUNCTIONS_WITH_NAMES: &str = "function Foo(arg) {};\nfunction setUp(arg3) {};\nfunction tearDown(arg, arg2) {};\nfunction testBar(arg) {};\nfunction test$(arg) {};\nfunction test$foo(arg) {}\n";

struct ExportTestFunctionsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: ExportTestFunctionsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass: Box<dyn CompilerPass> = Box::new(ExportTestFunctions::new(
            "google_exportSymbol",
            Some("google_exportProperty".into()),
        ));
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "ExportTestFunctionsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl ExportTestFunctionsTest {
    // port: ExportTestFunctionsTest#ExportTestFunctionsTest
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(EXTERNS);
        harness.set_up();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "ExportTestFunctionsTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
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
}

// port: ExportTestFunctionsTest#testFunctionsAreExported
#[test]
fn test_functions_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        TEST_FUNCTIONS_WITH_NAMES,
        "function Foo(arg){};\nfunction setUp(arg3){} google_exportSymbol(\"setUp\",setUp);;\nfunction tearDown(arg,arg2) {}\ngoogle_exportSymbol(\"tearDown\",tearDown);;\nfunction testBar(arg){} google_exportSymbol(\"testBar\",testBar);;\nfunction test$(arg){} google_exportSymbol(\"test$\",test$);;\nfunction test$foo(arg){} google_exportSymbol(\"test$foo\",test$foo)\n");
}

// Helper functions
// port: ExportTestFunctionsTest#testBasicTestFunctionsAreExported
#[test]
fn test_basic_test_functions_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("function Foo() {function testA(){}}");
    t.test(
        "function setUp() {}",
        "function setUp(){} google_exportSymbol('setUp',setUp)",
    );
    t.test(
        "function setUpPage() {}",
        "function setUpPage(){} google_exportSymbol('setUpPage',setUpPage)",
    );
    t.test(
        "function shouldRunTests() {}",
        "function shouldRunTests(){}\ngoogle_exportSymbol('shouldRunTests',shouldRunTests)\n",
    );
    t.test(
        "function tearDown() {}",
        "function tearDown(){} google_exportSymbol('tearDown',tearDown)",
    );
    t.test(
        "function tearDownPage() {}",
        "function tearDownPage(){} google_exportSymbol('tearDownPage',\ntearDownPage)\n",
    );
    t.test(
        "function testBar() { function testB() {}}",
        "function testBar(){function testB(){}}\ngoogle_exportSymbol('testBar',testBar)\n",
    );
    t.test_same("var testCase = {}; testCase.setUpPage = function() {}");
}

// port: ExportTestFunctionsTest#testBasicTestFunctionsAreExported_inGoogModule
#[test]
fn test_basic_test_functions_are_exported_in_goog_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("goog.module('test'); function Foo() {function testA(){}}");
    t.test(
        "goog.module('test'); function setUp() {}",
        "goog.module('test'); function setUp(){} google_exportSymbol('setUp',setUp)",
    );
}

/**
 * Make sure this works for global functions declared as function expressions:
 *
 * <pre>
 * var testFunctionName = function() {
 *   // Implementation
 * };
 * </pre>
 *
 * This format should be supported in addition to function statements.
 */
// port: ExportTestFunctionsTest#testFunctionExpressionsAreExported
#[test]
fn test_function_expressions_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("var Foo = function() {var testA = function() {}}");
    t.test(
        "var setUp = function() {}",
        "var setUp = function() {};\ngoogle_exportSymbol('setUp',setUp)\n",
    );
    t.test(
        "var setUpPage = function() {}",
        "var setUpPage = function() {};\ngoogle_exportSymbol('setUpPage',setUpPage)\n",
    );
    t.test(
        "var shouldRunTests = function() {}",
        "var shouldRunTests = function() {};\ngoogle_exportSymbol('shouldRunTests',shouldRunTests)\n");
    t.test(
        "var tearDown = function() {}",
        "var tearDown = function() {};\ngoogle_exportSymbol('tearDown',tearDown)\n",
    );
    t.test(
        "var tearDownPage = function() {}",
        "var tearDownPage = function() {};\ngoogle_exportSymbol('tearDownPage', tearDownPage)\n",
    );
    t.test(
        "var testBar = function() { var testB = function() {}}",
        "var testBar = function(){ var testB = function() {}};\ngoogle_exportSymbol('testBar',testBar)\n");
}

// port: ExportTestFunctionsTest#testFunctionExpressionsAreExported_inGoogModule
#[test]
fn test_function_expressions_are_exported_in_goog_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("goog.module('test'); var Foo = function() {var testA = function() {}}");
    t.test(
        "goog.module('test'); var setUp = function() {}",
        "goog.module('test'); var setUp = function() {}; google_exportSymbol('setUp',setUp)",
    );
}

// https://github.com/google/closure-compiler/issues/2563
// port: ExportTestFunctionsTest#testFunctionExpressionsInAssignAreExported
#[test]
fn test_function_expressions_in_assign_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "testBar = function() {};",
        "testBar = function() {}; google_exportSymbol('testBar',testBar)",
    );
}

// port: ExportTestFunctionsTest#testFunctionExpressionsInAssignAreExported_inGoogModule
#[test]
fn test_function_expressions_in_assign_are_exported_in_goog_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "goog.module('test'); testBar = function() {};",
        "goog.module('test'); testBar = function() {}; google_exportSymbol('testBar',testBar)",
    );
}

// port: ExportTestFunctionsTest#testFunctionExpressionsByLetAreExported
#[test]
fn test_function_expressions_by_let_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("let Foo = function() {var testA = function() {}}");
    t.test(
        "let setUp = function() {}",
        "let setUp = function() {}; google_exportSymbol('setUp', setUp)",
    );
    t.test(
        "let testBar = function() {}",
        "let testBar = function() {}; google_exportSymbol('testBar', testBar)",
    );
    t.test(
        "let tearDown = function() {}",
        "let tearDown = function() {};\ngoogle_exportSymbol('tearDown', tearDown)\n",
    );
}

// port: ExportTestFunctionsTest#testFunctionExpressionsByConstAreExported
#[test]
fn test_function_expressions_by_const_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("const Foo = function() {var testA = function() {}}");
    t.test(
        "const setUp = function() {}",
        "const setUp = function() {};\ngoogle_exportSymbol('setUp', setUp)\n",
    );
    t.test(
        "const testBar = function() {}",
        "const testBar = function() {};\ngoogle_exportSymbol('testBar', testBar)\n",
    );
    t.test(
        "const tearDown = function() {}",
        "const tearDown = function() {};\ngoogle_exportSymbol('tearDown', tearDown)\n",
    );
}

// port: ExportTestFunctionsTest#testArrowFunctionExpressionsAreExported
#[test]
fn test_arrow_function_expressions_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("var Foo = ()=>{var testA = function() {}}");
    t.test(
        "var setUp = ()=>{}",
        "var setUp = ()=>{};\ngoogle_exportSymbol('setUp', setUp)\n",
    );
    t.test(
        "var testBar = ()=>{}",
        "var testBar = ()=>{};\ngoogle_exportSymbol('testBar', testBar)\n",
    );
    t.test(
        "var tearDown = ()=>{}",
        "var tearDown = ()=>{};\ngoogle_exportSymbol('tearDown', tearDown)\n",
    );
}

// port: ExportTestFunctionsTest#testFunctionAssignmentsAreExported
#[test]
fn test_function_assignments_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("Foo = {}; Foo.prototype.bar = function() {};");

    t.test(
        "Foo = {}; Foo.prototype.setUpPage = function() {};",
        "Foo = {}; Foo.prototype.setUpPage = function() {};\ngoogle_exportProperty(Foo.prototype, 'setUpPage',\nFoo.prototype.setUpPage);\n");

    t.test(
        "Foo = {}; Foo.prototype.shouldRunTests = function() {};",
        "Foo = {}; Foo.prototype.shouldRunTests = function() {};\ngoogle_exportProperty(Foo.prototype, 'shouldRunTests',\nFoo.prototype.shouldRunTests);\n");

    t.test(
        "Foo = {}; Foo.prototype.testBar = function() {};",
        "Foo = {}; Foo.prototype.testBar = function() {};\ngoogle_exportProperty(Foo.prototype, 'testBar',\nFoo.prototype.testBar);\n");

    t.test(
        "window.testBar = function() {};",
        "window.testBar = function() {};\ngoogle_exportProperty(window, 'testBar',\nwindow.testBar);\n");

    t.test(
        "Foo = {}; Foo.prototype.testBar = function()\n{ var testBaz = function() {}};\n",
        "Foo = {}; Foo.prototype.testBar = function()\n{ var testBaz = function() {}};\ngoogle_exportProperty(Foo.prototype, 'testBar',\nFoo.prototype.testBar);\n");

    t.test(
        "Foo = {}; Foo.baz.prototype.testBar = function()\n{ var testBaz = function() {}};\n",
        "Foo = {}; Foo.baz.prototype.testBar = function()\n{ var testBaz = function() {}};\ngoogle_exportProperty(Foo.baz.prototype, 'testBar',\nFoo.baz.prototype.testBar);\n");
}

// port: ExportTestFunctionsTest#testExportTestSuite
#[test]
fn test_export_test_suite() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("goog.testing.testSuite({'a': function() {}, 'b': function() {}});");
    t.test(
        "goog.testing.testSuite({a: function() {}, b: function() {}});",
        "goog.testing.testSuite({'a': function() {}, 'b': function() {}});",
    );
}

// port: ExportTestFunctionsTest#testExportTestSuite_whereGoogIsDefined
#[test]
fn test_export_test_suite_where_goog_is_defined() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("var goog = {}; goog.testing.testSuite({'a': function() {}, 'b': function() {}});");
    t.test(
        "var goog = {}; goog.testing.testSuite({a: function() {}, b: function() {}});",
        "var goog = {}; goog.testing.testSuite({'a': function() {}, 'b': function() {}});",
    );
}

// port: ExportTestFunctionsTest#testExportTestSuite_inGoogModule
#[test]
fn test_export_test_suite_in_goog_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "goog.module('my.test');\nconst testSuite = goog.require('goog.testing.testSuite');\ntestSuite({a: function() {}, b: function() {}});\n",
        "goog.module('my.test');\nconst testSuite = goog.require('goog.testing.testSuite');\ntestSuite({'a': function() {}, 'b': function() {}});\n");
}

// port: ExportTestFunctionsTest#testNonGoogTestingTestSuiteNotRewrittenInModule
#[test]
fn test_non_goog_testing_test_suite_not_rewritten_in_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same(
        "goog.module('my.test');\nconst testSuite = goog.require('some.other.testSuite');\ntestSuite({a: function() {}, b: function() {}});\n");
}

// port: ExportTestFunctionsTest#testExportTestSuite_differentNameInGoogModule
#[test]
fn test_export_test_suite_different_name_in_goog_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "goog.module('my.test');\nconst someRandomName = goog.require('goog.testing.testSuite');\nsomeRandomName({a: function() {}, b: function() {}});\n",
        "goog.module('my.test');\nconst someRandomName = goog.require('goog.testing.testSuite');\nsomeRandomName({'a': function() {}, 'b': function() {}});\n");
}

// port: ExportTestFunctionsTest#testExportTestSuite_inEsModule
#[test]
fn test_export_test_suite_in_es_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "const testSuite = goog.require('goog.testing.testSuite');\ntestSuite({a: function() {}, b: function() {}});\nexport {};\n", // Add `export {}` to make this an ES module.
        "const testSuite = goog.require('goog.testing.testSuite');\ntestSuite({'a': function() {}, 'b': function() {}});\nexport {};\n");
}

// port: ExportTestFunctionsTest#testMemberDefInObjLitInTestSuite_becomesStringKey
#[test]
fn test_member_def_in_obj_lit_in_test_suite_becomes_string_key() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "goog.testing.testSuite({a() {}, b() {}});",
        "goog.testing.testSuite({'a': function() {}, 'b': function() {}});",
    );
}

// port: ExportTestFunctionsTest#testMemberDefInObjLitInTestSuite_becomesStringKey_withSameJSDoc
#[test]
fn test_member_def_in_obj_lit_in_test_suite_becomes_string_key_with_same_js_doc() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "goog.testing.testSuite({\n  /** @suppress {checkTypes} */ a() {},\n  b() {}\n});\n",
        "goog.testing.testSuite({\n  /** @suppress {checkTypes} */\n  'a': function() {},\n  'b': function() {}\n});\n");
}

// port: ExportTestFunctionsTest#testComputedPropInObjLitInTestSuite_doesNotChange
#[test]
fn test_computed_prop_in_obj_lit_in_test_suite_does_not_change() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same(
        "goog.testing.testSuite({\n  /** @suppress {checkTypes} */ ['a']() {},\n  ['b']() {}\n});\n");
}

// port: ExportTestFunctionsTest#testEs6Class_testMethod
#[test]
fn test_es6_class_test_method() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "class MyTest {testFoo() {}} goog.testing.testSuite(new MyTest());",
        "class MyTest {testFoo() {}}\ngoogle_exportProperty(MyTest.prototype, 'testFoo', MyTest.prototype.testFoo);\ngoog.testing.testSuite(new MyTest());\n");
}

// port: ExportTestFunctionsTest#testEs6Class_testMethod_inGoogModule
#[test]
fn test_es6_class_test_method_in_goog_module() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "goog.module('test'); class MyTest {testFoo() {}} goog.testing.testSuite(new MyTest());",
        "goog.module('test');\nclass MyTest {testFoo() {}}\ngoogle_exportProperty(MyTest.prototype, 'testFoo', MyTest.prototype.testFoo);\ngoog.testing.testSuite(new MyTest());\n");
}

// port: ExportTestFunctionsTest#testEs6Class_lifeCycleMethods
#[test]
fn test_es6_class_life_cycle_methods() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "class MyTest {\ntestFoo(){} setUp(){} tearDown(){} setUpPage(){} tearDownPage(){} notExported(){}\n}\ngoog.testing.testSuite(new MyTest());\n",
        "class MyTest {\ntestFoo(){} setUp(){} tearDown(){} setUpPage(){} tearDownPage(){} notExported(){}\n}\ngoogle_exportProperty(MyTest.prototype, 'testFoo', MyTest.prototype.testFoo);\ngoogle_exportProperty(MyTest.prototype, 'setUp', MyTest.prototype.setUp);\ngoogle_exportProperty(MyTest.prototype, 'tearDown', MyTest.prototype.tearDown);\ngoogle_exportProperty(MyTest.prototype, 'setUpPage', MyTest.prototype.setUpPage);\ngoogle_exportProperty(MyTest.prototype, 'tearDownPage',\nMyTest.prototype.tearDownPage);\ngoog.testing.testSuite(new MyTest());\n");
}

// https://github.com/google/closure-compiler/issues/2563
// port: ExportTestFunctionsTest#testES6ClassAssignmentsAreExported
#[test]
fn test_es6_class_assignments_are_exported() {
    let mut t = ExportTestFunctionsTest::new();
    t.test_same("Foo = class {bar() {}}");

    t.test(
        "Foo = class {testBar() {}}",
        "Foo = class {testBar() {}};\ngoogle_exportProperty(Foo.prototype, 'testBar', Foo.prototype.testBar);\n");
}

// port: ExportTestFunctionsTest#testEs6Class_testClassExpressionMethod
#[test]
fn test_es6_class_test_class_expression_method() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "var MyTest=class{testFoo() {}}; goog.testing.testSuite(new MyTest());",
        "var MyTest=class{testFoo() {}};\ngoogle_exportProperty(MyTest.prototype, 'testFoo', MyTest.prototype.testFoo);\ngoog.testing.testSuite(new MyTest());\n");
}

// port: ExportTestFunctionsTest#testEs6Class_testClassExpressionByLetMethod
#[test]
fn test_es6_class_test_class_expression_by_let_method() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "let MyTest=class{testFoo() {}}; goog.testing.testSuite(new MyTest());",
        "let MyTest=class{testFoo() {}};\ngoogle_exportProperty(MyTest.prototype, 'testFoo', MyTest.prototype.testFoo);\ngoog.testing.testSuite(new MyTest());\n");
}

// port: ExportTestFunctionsTest#testEs6Class_testClassExpressionByConstMethod
#[test]
fn test_es6_class_test_class_expression_by_const_method() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "const MyTest=class{testFoo() {}}; goog.testing.testSuite(new MyTest());",
        "const MyTest=class{testFoo() {}};\ngoogle_exportProperty(MyTest.prototype, 'testFoo', MyTest.prototype.testFoo);\ngoog.testing.testSuite(new MyTest());\n");
}

// port: ExportTestFunctionsTest#testCommaOperatorRecognized
#[test]
fn test_comma_operator_recognized() {
    let mut t = ExportTestFunctionsTest::new();
    t.test(
        "(0, goog.testing.testSuite)({setUp(){console.error('hello');}});",
        "(0, goog.testing.testSuite)({'setUp': function (){console.error('hello');}});",
    );

    t.test(
        "(0, goog.testing.testSuite)({testFunction(){console.error('hello');}});",
        "(0, goog.testing.testSuite)({'testFunction': function (){console.error('hello');}});", //
    );
}
