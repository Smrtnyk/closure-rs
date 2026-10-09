/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/StripCodeTest.java.

//! Port of `StripCodeTest.java`: tests for `StripCode`.
use closure_jscomp::{
    compiler_pass::CompilerPass, diagnostic_type::DiagnosticType, strip_code, strip_code::StripCode,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

// port: StripCodeTest#EXTERNS
const EXTERNS: &str = "";

struct StripCodeTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

/// Creates an instance for removing logging code.
// port: StripCodeTest#getStripCodeInstance
fn get_strip_code_instance() -> StripCode {
    let strip_types: IndexSet<String> = [
        "goog.debug.DebugWindow",
        "goog.debug.FancyWindow",
        "goog.debug.Formatter",
        "goog.debug.HtmlFormatter",
        "goog.debug.TextFormatter",
        "goog.debug.Logger",
        "goog.debug.LogManager",
        "goog.debug.LogRecord",
        "goog.net.BrowserChannel.LogSaver",
        "goog.log",
        "GA_GoogleDebugger",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let strip_name_suffixes: IndexSet<String> = [
        "logger",
        "logger_",
        "debugWindow",
        "debugWindow_",
        "logFormatter_",
        "logBuffer_",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let strip_name_prefixes: IndexSet<String> = ["trace"].into_iter().map(String::from).collect();

    StripCode::new(
        &strip_types,
        &strip_name_suffixes,
        &strip_name_prefixes,
        false,
    )
}

impl CompilerTestCaseHooks for Hooks {
    // port: StripCodeTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass: Box<dyn CompilerPass> = Box::new(get_strip_code_instance());
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "StripCodeTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl StripCodeTest {
    // port: StripCodeTest#StripCodeTest
    // port: StripCodeTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(EXTERNS);
        harness.set_up();
        harness.enable_normalize().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "StripCodeTest".into(),
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

    fn test_error(&mut self, js: &str, error: &'static DiagnosticType) {
        self.harness
            .test_error(&mut self.hooks, js, error)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

// port: StripCodeTest#testLowerCaseSuffixFunctionCallMatch
#[test]
fn test_lower_case_suffix_function_call_match() {
    let mut t = StripCodeTest::new();
    // should lower-case match with "logger"
    t.test("errorLogger();", "");
}

// port: StripCodeTest#testChildNameOfStrippedTypeIsStripped
#[test]
fn test_child_name_of_stripped_type_is_stripped() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.debug.FancyWindow.getStoredKeys_ = function() {};\ngoog.debug.FancyWindow.getStoredKeys_();\n",
        "");
    // It's safer to replace a variable's initial value with null instead of
    // removing the declaration.
    t.test(
        "var goog$debug$FancyWindow$getStoredKeys_ = function() {};\ngoog$debug$FancyWindow$getStoredKeys_();\n",
        "var goog$debug$FancyWindow$getStoredKeys_ = null;");
}

// port: StripCodeTest#testPartiallyCollapsedTypeIsStripped
#[test]
fn test_partially_collapsed_type_is_stripped() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog$debug.FancyWindow.getStoredKeys_ = function() {};\ngoog$debug.FancyWindow.getStoredKeys_();\n",
        "");
}

// port: StripCodeTest#testLoggerDefinedInConstructor
#[test]
fn test_logger_defined_in_constructor() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function() {\n  this.logger = goog.debug.Logger.getLogger('a.b.c');\n};\n",
        "a.b.c=function(){}",
    );
    t.test(
        "let a$b$c = function() {\n  this.logger = goog$debug$Logger.getLogger('a.b.c');\n};\n",
        "let a$b$c=function(){}",
    );
}

// port: StripCodeTest#testLoggerDefinedInConstructorEs6
#[test]
fn test_logger_defined_in_constructor_es6() {
    let mut t = StripCodeTest::new();
    t.test(
        "class A {\n  constructor() {\n    this.logger = goog.debug.Logger.getLogger('A');\n    this.otherProperty = 3;\n  }\n}\nlet a = new A;\na.logger.warning('foobar');\n",
        "class A {\n  constructor() {\n    this.otherProperty = 3;\n  }\n}\nlet a = new A;\n");
    t.test(
        "class A {\n  constructor() {\n    this.logger = goog$debug$Logger.getLogger('A');\n    this.otherProperty = 3;\n  }\n}\nlet a = new A;\na.logger.warning('foobar');\n",
        "class A {\n  constructor() {\n    this.otherProperty = 3;\n  }\n}\nlet a = new A;\n");
}

// port: StripCodeTest#testLoggerDefinedInPrototype1
#[test]
fn test_logger_defined_in_prototype1() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function() {}\na.b.c.prototype.logger = goog.debug.Logger.getLogger('a.b.c');\n",
        "a.b.c=function(){}",
    );
    t.test(
        "a$b$c = function() {};\na$b$c.prototype.logger = goog$debug$Logger.getLogger('a.b.c');\n",
        "a$b$c = function(){}",
    );
}

// port: StripCodeTest#testLoggerDefinedInPrototype2
#[test]
fn test_logger_defined_in_prototype2() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function() {};\na.b.c.prototype = {logger: goog.debug.Logger.getLogger('a.b.c')}\n",
        "a.b.c = function() {}\na.b.c.prototype = {}\n");
    t.test(
        "let a$b$c = function() {};\na$b$c.prototype = {logger: goog$debug$Logger.getLogger('a.b.c')}\n",
        "let a$b$c = function() {};\na$b$c.prototype = {}\n");
}

// port: StripCodeTest#testLoggerDefinedInPrototype3
#[test]
fn test_logger_defined_in_prototype3() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function() {}\na.b.c.prototype = {\n  get logger() {return goog.debug.Logger.getLogger('a.b.c')}\n}\n",
        "a.b.c = function() {}\na.b.c.prototype = {}\n");
    t.test(
        "let a$b$c = function() {};\na$b$c.prototype = {\n  get logger() {return goog$debug$Logger.getLogger('a.b.c')}\n}\n",
        "let a$b$c = function() {};\na$b$c.prototype = {}\n");
}

// port: StripCodeTest#testLoggerDefinedInPrototype4
#[test]
fn test_logger_defined_in_prototype4() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function() {}\na.b.c.prototype = {\n  set logger(a) {this.x = goog.debug.Logger.getLogger('a.b.c')}\n}\n",
        "a.b.c = function() {}\na.b.c.prototype = {}\n");
    t.test(
        "let a$b$c = function() {};\na$b$c.prototype = {\n  set logger(a) {this.x = goog$debug$Logger.getLogger('a.b.c')}\n}\n",
        "let a$b$c = function() {};\na$b$c.prototype = {}\n");
}

// port: StripCodeTest#testLoggerDefinedInPrototype5
#[test]
fn test_logger_defined_in_prototype5() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function() {}\na.b.c.prototype = {\n  get f() {return this.x;},\n  set f(a) {this.x = goog.debug.Logger.getLogger('a.b.c')}\n}\n",
        "a.b.c = function() {}\na.b.c.prototype = {\n  get f() {return this.x;},\n  set f(a) {this.x = null}\n}\n");
    t.test(
        "let a$b$c = function() {}\na$b$c.prototype = {\n  get f() {return this.x;},\n  set f(a) {this.x = goog$debug$Logger.getLogger('a.b.c')}\n}\n",
        "let a$b$c = function() {}\na$b$c.prototype = {\n  get f() {return this.x;},\n  set f(a) {this.x = null}\n}\n");
}

// port: StripCodeTest#testLoggerDefinedInPrototype6
#[test]
fn test_logger_defined_in_prototype6() {
    let mut t = StripCodeTest::new();
    t.test(
        "var a = {};\na.b = function() {};\na.b.prototype = {\n  logger(a) {this.x = goog.debug.Logger.getLogger('a.b.c')}\n}\n",
        "var a = {}; a.b = function() {}; a.b.prototype = {}");
    t.test(
        "var a = {};\nvar a$b = function() {};\na$b.prototype = {\n  logger(a) {this.x = goog$debug$Logger.getLogger('a.b.c')}\n}\n",
        "var a = {}; var a$b = function() {}; a$b.prototype = {}");
}

// port: StripCodeTest#testLoggerDefinedInPrototype7
#[test]
fn test_logger_defined_in_prototype7() {
    let mut t = StripCodeTest::new();
    t.test(
        "var a = {};\na.b = function() {};\na.b.prototype = {\n  ['logger']: goog.debug.Logger.getLogger('a.b.c')\n}\n",
        "var a = {};\na.b = function() {};\na.b.prototype = {\n  ['logger']: null\n}\n");
    t.test(
        "var a = {};\nvar a$b = function() {};\na$b.prototype = {\n  ['logger']: goog$debug$Logger.getLogger('a.b.c')\n}\n",
        "var a = {};\nvar a$b = function() {};\na$b.prototype = {\n  ['logger']: null\n}\n");
}

// port: StripCodeTest#testLoggerDefinedStatically
#[test]
fn test_logger_defined_statically() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function() {}\na.b.c.logger = goog.debug.Logger.getLogger('a.b.c');\n",
        "a.b.c=function(){}",
    );
    t.test(
        "let a$b$c = function() {}\na$b$c.logger = goog$debug$Logger.getLogger('a.b.c');\n",
        "let a$b$c=function(){}",
    );
}

// port: StripCodeTest#testDeletedScopesAreReported
#[test]
fn test_deleted_scopes_are_reported() {
    let mut t = StripCodeTest::new();
    t.test("var nodeLogger = function () {};", "");
}

// port: StripCodeTest#testLoggerDefinedInObjectLiteral1
#[test]
fn test_logger_defined_in_object_literal1() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = {\n  x: 0,\n  logger: goog.debug.Logger.getLogger('a.b.c')\n};\n",
        "a.b.c={x:0}",
    );
    t.test(
        "let a$b$c = {\n  x: 0,\n  logger: goog$debug$Logger.getLogger('a.b.c')\n};\n",
        "let a$b$c={x:0}",
    );
}

// port: StripCodeTest#testLoggerDefinedInObjectLiteral2
#[test]
fn test_logger_defined_in_object_literal2() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = {\n  x: 0,\n  get logger() {return goog.debug.Logger.getLogger('a.b.c')}\n};\n",
        "a.b.c={x:0}",
    );
    t.test(
        "let a$b$c = {\n  x: 0,\n  get logger() {return goog$debug$Logger.getLogger('a.b.c')}\n};\n",
        "let a$b$c={x:0}");
}

// port: StripCodeTest#testLoggerDefinedInObjectLiteral3
#[test]
fn test_logger_defined_in_object_literal3() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = {\n  x: null,\n  get logger() {return this.x},\n  set logger(a) {this.x  = goog.debug.Logger.getLogger(a)}\n};\n",
        "a.b.c={x:null}");
    t.test(
        "let a$b$c = {\n  x: null,\n  get logger() {return this.x},\n  set logger(a) {this.x  = goog$debug$Logger.getLogger(a)}\n};\n",
        "let a$b$c={x:null}");
}

// port: StripCodeTest#testLoggerDefinedInObjectLiteral4
#[test]
fn test_logger_defined_in_object_literal4() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = {\n  x: null,\n  get y() {return this.x},\n  set y(a) {this.x  = goog.debug.Logger.getLogger(a)}\n};\n",
        "a.b.c = {\n  x: null,\n  get y() {return this.x},\n  set y(a) {this.x  = null}\n};\n");
    t.test(
        "let a$b$c = {\n  x: null,\n  get y() {return this.x},\n  set y(a) {this.x  = goog$debug$Logger.getLogger(a)}\n};\n",
        "let a$b$c = {\n  x: null,\n  get y() {return this.x},\n  set y(a) {this.x  = null}\n};\n");
}

// port: StripCodeTest#testLoggerDefinedInObjectLiteral5
#[test]
fn test_logger_defined_in_object_literal5() {
    let mut t = StripCodeTest::new();
    t.test(
        "var a = {};\na.b = {\n  x: 0,\n  logger() { return goog.debug.Logger.getLogger('a.b.c'); }\n};\n",
        "var a = {}; a.b={x:0}");
    t.test(
        "var a = {};\nvar a$b = {\n  x: 0,\n  logger() { return goog$debug$Logger.getLogger('a.b.c'); }\n};\n",
        "var a = {}; var a$b={x:0}");
}

// port: StripCodeTest#testLoggerDefinedInObjectLiteral6
#[test]
fn test_logger_defined_in_object_literal6() {
    let mut t = StripCodeTest::new();
    t.test(
        "var a = {};\na.b = {\n  x: 0,\n  ['logger']: goog.debug.Logger.getLogger('a.b.c')\n};\na.b['logger']();\n",
        "var a = {};\na.b = {\n  x: 0,\n  ['logger']: null // Don't strip computed properties\n};\na.b['logger']()\n");
    t.test(
        "var a = {};\nvar a$b = {\n  x: 0,\n  ['logger']: goog$debug$Logger.getLogger('a.b.c')\n};\na$b['logger']();\n",
        "var a = {};\nvar a$b = {\n  x: 0,\n  ['logger']: null // Don't strip computed properties\n};\na$b['logger']()\n");
}

// port: StripCodeTest#testLoggerDefinedInPrototypeAndUsedInConstructor
#[test]
fn test_logger_defined_in_prototype_and_used_in_constructor() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function(level) {\n  if (!this.logger.isLoggable(level)) {\n    this.logger.setLevel(level);\n  }\n  this.logger.log(level, 'hi');\n};\na.b.c.prototype.logger = goog.debug.Logger.getLogger('a.b.c');\na.b.c.prototype.go = function() { this.logger.finer('x'); };\n",
        "a.b.c=function(level){if(!null);}\na.b.c.prototype.go=function(){}\n");
    t.test(
        "let a$b$c = function(level) {\n  if (!this.logger.isLoggable(level)) {\n    this.logger.setLevel(level);\n  }\n  this.logger.log(level, 'hi');\n};\na$b$c.prototype.logger = goog$debug$Logger.getLogger('a.b.c');\na$b$c.prototype.go = function() { this.logger.finer('x'); };\n",
        "let a$b$c=function(level){if(!null);}\na$b$c.prototype.go=function(){}\n");
}

// port: StripCodeTest#testLoggerDefinedStaticallyAndUsedInConstructor
#[test]
fn test_logger_defined_statically_and_used_in_constructor() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c = function(level) {\n  if (!a.b.c.logger.isLoggable(level)) {\n    a.b.c.logger.setLevel(level);\n  }\n  a.b.c.logger.log(level, 'hi');\n};\na.b.c.logger = goog.debug.Logger.getLogger('a.b.c');\n",
        "a.b.c=function(level){if(!null);}");
    t.test(
        "let a$b$c = function(level) {\n  if (!a$b$c$logger.isLoggable(level)) {\n    a$b$c$logger.setLevel(level);\n  }\n  a$b$c$logger.log(level, 'hi');\n};\nlet a$b$c$logger = goog$debug$Logger.getLogger('a.b.c');\n",
        "let a$b$c=function(level){if(!null);}\nlet a$b$c$logger = null\n");
}

// port: StripCodeTest#testLoggerVarDeclaration
#[test]
fn test_logger_var_declaration() {
    let mut t = StripCodeTest::new();
    t.test(
        "var logger = opt_logger || goog.debug.LogManager.getRoot();",
        "",
    );
    t.test(
        "var logger = opt_logger || goog$debug$LogManager.getRoot();",
        "",
    );
}

// port: StripCodeTest#testLoggerLetDeclaration
#[test]
fn test_logger_let_declaration() {
    let mut t = StripCodeTest::new();
    t.test(
        "let logger = opt_logger || goog.debug.LogManager.getRoot();",
        "",
    );
    t.test(
        "let logger = opt_logger || goog$debug$LogManager.getRoot();",
        "",
    );
}

// port: StripCodeTest#testLoggerConstDeclaration
#[test]
fn test_logger_const_declaration() {
    let mut t = StripCodeTest::new();
    t.test(
        "const logger = opt_logger || goog.debug.LogManager.getRoot();",
        "",
    );
    t.test(
        "const logger = opt_logger || goog$debug$LogManager.getRoot();",
        "",
    );
}

// port: StripCodeTest#testLoggerDestructuringDeclaration
#[test]
fn test_logger_destructuring_declaration() {
    let mut t = StripCodeTest::new();
    // NOTE: StripCode currently does not optimize code in destructuring patterns, even if
    // it contains "strip names" or "strip types", since it's unclear what the correct optimization
    // would be or how much it would help users.

    t.test_same("const {Logger} = goog.debug; Logger;");
    t.test_same("const {Logger} = goog$debug; Logger;");
    t.test_same("const {Logger: logger} = goog.debug; logger;");
    t.test_same("const {Logger: logger} = goog$debug; logger;");
    t.test_same("const [logger] = [1]; logger;");

    t.test(
        "const {getLogger} = goog.debug.Logger; const logger = opt_logger || getLogger('A');",
        "const {getLogger} = goog.debug.Logger;",
    );
    t.test(
        "const {getLogger} = goog$debug$Logger; const logger = opt_logger || getLogger('A');",
        "const {getLogger} = goog$debug$Logger;",
    );
}

// port: StripCodeTest#testLoggerMethodCallByVariableType_var
#[test]
fn test_logger_method_call_by_variable_type_var() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x = goog.debug.Logger.getLogger('a.b.c'); y.info(a); x.info(a);",
        "y.info(a)",
    );
    t.test(
        "var x = goog$debug$Logger.getLogger('a.b.c'); y.info(a); x.info(a);",
        "y.info(a)",
    );
}

// port: StripCodeTest#testLoggerMethodCallByVariableType_let
#[test]
fn test_logger_method_call_by_variable_type_let() {
    let mut t = StripCodeTest::new();
    t.test(
        "let x = goog.debug.Logger.getLogger('a.b.c'); y.info(a); x.info(a);",
        "y.info(a)",
    );
    t.test(
        "let x = goog$debug$Logger.getLogger('a.b.c'); y.info(a); x.info(a);",
        "y.info(a)",
    );
}

// port: StripCodeTest#testLoggerMethodCallByVariableType_const
#[test]
fn test_logger_method_call_by_variable_type_const() {
    let mut t = StripCodeTest::new();
    t.test(
        "const x = goog.debug.Logger.getLogger('a.b.c'); y.info(a); x.info(a);",
        "y.info(a)",
    );
    t.test(
        "const x = goog$debug$Logger.getLogger('a.b.c'); y.info(a); x.info(a);",
        "y.info(a)",
    );
}

// port: StripCodeTest#testLoggerMethodCallByIndirection
#[test]
fn test_logger_method_call_by_indirection() {
    let mut t = StripCodeTest::new();
    t.test(
        "const x = goog.debug.Logger.getLogger('a.b.c');\n(0, goog.log.error)(x, 'oh noes');\n",
        "",
    );
}

// port: StripCodeTest#testLoggerMethodCallByIndirection_inVarAssignment
#[test]
fn test_logger_method_call_by_indirection_in_var_assignment() {
    let mut t = StripCodeTest::new();
    t.test("const x = (0, goog.log.getLogger)('a.b.c');", "");
}

// port: StripCodeTest#testLoggerMethodCallByIndirection_functionRemovedBeforeCalled
#[test]
fn test_logger_method_call_by_indirection_function_removed_before_called() {
    let mut t = StripCodeTest::new();
    t.test(
        "var goog$log$getLogger = function() {};\nvar module$x = (0, goog$log$getLogger)('a.b.c');\n",
        "var goog$log$getLogger = null;\nvar module$x = null;\n");
}

// port: StripCodeTest#testLoggerMethodCallByIndirection_removedFunctionIsNotLastChild
#[test]
fn test_logger_method_call_by_indirection_removed_function_is_not_last_child() {
    let mut t = StripCodeTest::new();
    t.test(
        "var goog$log$getLogger = function() {};\nvar module$x = (goog$log$getLogger, function() {})('a.b.c');\n",
        "var goog$log$getLogger = null;\nvar module$x = (null, function() {})('a.b.c');\n");
}

// port: StripCodeTest#testLoggerMethodCallByIndirection_removedFunctionNotInCall
#[test]
fn test_logger_method_call_by_indirection_removed_function_not_in_call() {
    let mut t = StripCodeTest::new();
    t.test(
        "var goog$log$getLogger = function() {};\nvar module$x = (0, goog$log$getLogger);\n",
        "var goog$log$getLogger = null;\nvar module$x = (0, null);\n",
    );
}

// port: StripCodeTest#testLoggerMethodCallByIndirection_otherCommaExpressionsStayAsTheyAre
#[test]
fn test_logger_method_call_by_indirection_other_comma_expressions_stay_as_they_are() {
    let mut t = StripCodeTest::new();
    t.test(
        "var anotherFunc = function() {};\nvar x = (0, anotherFunc);\n",
        "var anotherFunc = function() {};\nvar x = (0, anotherFunc);\n",
    );
}

// port: StripCodeTest#testSubPropertyAccessByVariableName_var
#[test]
fn test_sub_property_access_by_variable_name_var() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x, y = goog.debug.Logger.getLogger('a.b.c')\nvar logger = x;\nvar curlevel = logger.level_ ? logger.getLevel().name : 3;\n",
        "var x;var curlevel=null?null:3");
    t.test(
        "var x, y = goog$debug$Logger.getLogger('a.b.c')\nvar logger = x;\nvar curlevel = logger.level_ ? logger.getLevel().name : 3;\n",
        "var x;var curlevel=null?null:3");
}

// port: StripCodeTest#testSubPropertyAccessByVariableName_let
#[test]
fn test_sub_property_access_by_variable_name_let() {
    let mut t = StripCodeTest::new();
    t.test(
        "let x, y = goog.debug.Logger.getLogger('a.b.c');\nlet logger = x;\nlet curlevel = logger.level_ ? logger.getLevel().name : 3;\n",
        "let x; let curlevel=null?null:3");
    t.test(
        "let x, y = goog$debug$Logger.getLogger('a.b.c');\nlet logger = x;\nlet curlevel = logger.level_ ? logger.getLevel().name : 3;\n",
        "let x; let curlevel=null?null:3");
}

// port: StripCodeTest#testSubPropertyAccessByVariableName_const
#[test]
fn test_sub_property_access_by_variable_name_const() {
    let mut t = StripCodeTest::new();
    t.test(
        "const x = undefined, y = goog.debug.Logger.getLogger('a.b.c');\nconst logger = x;\nconst curlevel = logger.level_ ? logger.getLevel().name : 3;\n",
        "const x = undefined; const curlevel=null?null:3");
    t.test(
        "const x = undefined, y = goog$debug$Logger.getLogger('a.b.c');\nconst logger = x;\nconst curlevel = logger.level_ ? logger.getLevel().name : 3;\n",
        "const x = undefined; const curlevel=null?null:3");
}

// port: StripCodeTest#testPrefixedVariableName
#[test]
fn test_prefixed_variable_name() {
    let mut t = StripCodeTest::new();
    t.test(
        "this.blcLogger_ = goog.debug.Logger.getLogger('a.b.c')\nthis.blcLogger_.fine('Raised dirty states.');\n",
        "");
    t.test(
        "this.blcLogger_ = goog$debug$Logger.getLogger('a.b.c')\nthis.blcLogger_.fine('Raised dirty states.');\n",
        "");
}

// port: StripCodeTest#testPrefixedPropertyName
#[test]
fn test_prefixed_property_name() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.c.staticLogger_ = goog.debug.Logger.getLogger('a.b.c')\na.b.c.staticLogger_.fine('-' + a.b.c.d_())\n",
        "");
    t.test(
        "a$b$c$staticLogger_ = goog$debug$Logger.getLogger('a.b.c')\na$b$c$staticLogger_.fine('-' + a$b$c$d_())\n",
        "");
}

// port: StripCodeTest#testPrefixedClassName
#[test]
fn test_prefixed_class_name() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.MyLogger = function(logger) {\n  this.logger_ = logger;\n};\na.b.MyLogger.prototype.shout = function(msg, opt_x) {\n  this.logger_.log(goog.debug.Logger.Level.SHOUT, msg, opt_x);\n};\n",
        "a.b.MyLogger=function(logger){}\na.b.MyLogger.prototype.shout=function(msg,opt_x){}\n");
    t.test(
        "a$b$MyLogger = function(logger) {\n  this.logger_ = logger;\n};\na$b$MyLogger.prototype.shout = function(msg, opt_x) {\n  this.logger_.log(goog$debug$Logger.Level.SHOUT, msg, opt_x);\n};\n",
        "a$b$MyLogger=function(logger){}\na$b$MyLogger.prototype.shout=function(msg,opt_x){}\n");
}

// port: StripCodeTest#testLoggerClassDefinition
#[test]
fn test_logger_class_definition() {
    let mut t = StripCodeTest::new();
    t.test("goog.debug.Logger=function(name){this.name_=name}", "");
    t.test("goog$debug$Logger=function(name){this.name_=name}", "");
}

// port: StripCodeTest#testStaticLoggerPropertyDefinition
#[test]
fn test_static_logger_property_definition() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.debug.Logger.Level.SHOUT=\nnew goog.debug.Logger.Level(x,1200)\n",
        "",
    );
    t.test(
        "goog$debug$Logger$Level$SHOUT=\nnew goog$debug$Logger$Level(x,1200)\n",
        "",
    );
}

// port: StripCodeTest#testStaticLoggerMethodDefinition
#[test]
fn test_static_logger_method_definition() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.debug.Logger.getLogger=function(name){\nreturn goog.debug.LogManager.getLogger(name)\n};\n",
        "");
    t.test(
        "goog$debug$Logger$getLogger=function(name){\nreturn goog$debug$LogManager$getLogger(name)\n};\n",
        "");
}

// port: StripCodeTest#testPrototypeFieldDefinition
#[test]
fn test_prototype_field_definition() {
    let mut t = StripCodeTest::new();
    t.test("goog.debug.Logger.prototype.level_=null;", "");
    t.test("goog$debug$Logger.prototype.level_=null;", "");
}

// port: StripCodeTest#testPrototypeFieldDefinitionWithoutAssignment
#[test]
fn test_prototype_field_definition_without_assignment() {
    let mut t = StripCodeTest::new();
    t.test("goog.debug.Logger.prototype.level_;", "");
    t.test("goog$debug$Logger.prototype.level_;", "");
}

// port: StripCodeTest#testPrototypeMethodDefinition
#[test]
fn test_prototype_method_definition() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.debug.Logger.prototype.addHandler=\nfunction(handler){this.handlers_.push(handler)};\n",
        "");
    t.test(
        "goog$debug$Logger.prototype.addHandler=\nfunction(handler){this.handlers_.push(handler)};\n",
        "");
}

// port: StripCodeTest#testPublicPropertyAssignment
#[test]
fn test_public_property_assignment() {
    let mut t = StripCodeTest::new();
    // Eliminate property assignments on vars/properties that we
    // remove as otherwise we create invalid code.
    t.test("goog.debug.Logger = 1; goog.debug.Logger.prop=2; ", "");
    t.test("goog$debug$Logger = 1; goog$debug$Logger.prop=2; ", "");
    t.test("this.blcLogger_.level=x", "");
    t.test("goog.ui.Component.logger.prop=y", "");
    t.test("goog$ui$Component$logger$prop=y", "");
    t.test("goog.ui.Component.logger.prop.foo.bar=baz", "");
    t.test("goog$ui$Component$logger$prop$foo$bar=baz", "");
}

// port: StripCodeTest#testGlobalCallWithStrippedType
#[test]
fn test_global_call_with_stripped_type() {
    let mut t = StripCodeTest::new();
    t.test_same("window.alert(goog.debug.Logger)");
    t.test_same("window.alert(goog$debug$Logger)");
}

// port: StripCodeTest#testClassDefiningCallWithStripType1
#[test]
fn test_class_defining_call_with_strip_type1() {
    let mut t = StripCodeTest::new();
    t.test("goog.debug.Logger.inherits(Object)", "");
    t.test("goog$debug$Logger.inherits(Object)", "");
}

// port: StripCodeTest#testClassDefiningCallWithStripType2
#[test]
fn test_class_defining_call_with_strip_type2() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.formatter=function(){};\ngoog.inherits(goog.debug.Formatter,goog.formatter)\n",
        "goog.formatter=function(){}",
    );
    t.test(
        "let goog$formatter=function(){};\ngoog$inherits(goog$debug$Formatter,goog$formatter)\n",
        "let goog$formatter=function(){}",
    );
}

// port: StripCodeTest#testClassDefiningCallWithStripType3
#[test]
fn test_class_defining_call_with_strip_type3() {
    let mut t = StripCodeTest::new();
    t.test_error(
        "goog.formatter=function(){}\ngoog.inherits(goog.formatter,goog.debug.Formatter)\n",
        &strip_code::STRIP_TYPE_INHERIT_ERROR,
    );
    t.test_error(
        "let goog$formatter=function(){}\ngoog$inherits(goog$formatter,goog$debug$Formatter)\n",
        &strip_code::STRIP_TYPE_INHERIT_ERROR,
    );
}

// port: StripCodeTest#testClassDefiningCallWithStripType4
#[test]
fn test_class_defining_call_with_strip_type4() {
    let mut t = StripCodeTest::new();
    t.test_same("goog.formatter=function(){}; goog.formatter.inherits(goog.debug.FormatterFoo)");
    t.test_same(
        "let goog$formatter=function(){}; goog$formatter$inherits(goog$debug$FormatterFoo)",
    );
}

// port: StripCodeTest#testClassDefiningCallWithStripType5
#[test]
fn test_class_defining_call_with_strip_type5() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.inherits(goog.debug.TextFormatter, goog.debug.Formatter)",
        "",
    );
    t.test(
        "goog$inherits(goog$debug$TextFormatter, goog$debug$Formatter)",
        "",
    );
}

// port: StripCodeTest#testClassDefiningCallWithStripType6
#[test]
fn test_class_defining_call_with_strip_type6() {
    let mut t = StripCodeTest::new();
    // listed types should be removed.
    t.test("goog.debug.DebugWindow = function(){}", "");
    t.test("goog$debug$DebugWindow = function(){}", "");
    t.test("goog.inherits(goog.debug.DebugWindow,Base)", "");
    t.test("goog$inherits(goog$debug$DebugWindow,Base)", "");
    t.test("goog.debug.DebugWindow = class {}", "");
    t.test("goog$debug$DebugWindow = class {}", "");
    t.test("class GA_GoogleDebugger {}", "");
    t.test("if (class GA_GoogleDebugger {}) {}", "if (null) {}");

    // types that happen to have strip types as prefix should not be
    // stripped.
    t.test_same("goog.debug.DebugWindowFoo=function(){}");
    t.test_same("goog$debug$DebugWindowFoo=function(){}");
    t.test_same("goog.inherits(goog.debug.DebugWindowFoo,Base)");
    t.test_same("goog$inherits(goog$debug$DebugWindowFoo,Base)");
    t.test_same("goog.debug.DebugWindowFoo");
    t.test_same("goog$debug$DebugWindowFoo");
    t.test_same("goog.debug.DebugWindowFoo=1");
    t.test_same("goog$debug$DebugWindowFoo=1");
    t.test_same("goog.debug.DebugWindowFoo = class {}");
    t.test_same("goog$debug$DebugWindowFoo = class {}");

    // qualified subtypes should be removed.
    t.test("goog.debug.DebugWindow.Foo=function(){}", "");
    t.test("goog$debug$DebugWindow$Foo=function(){}", "");
    t.test("goog.inherits(goog.debug.DebugWindow.Foo,Base)", "");
    t.test("goog$inherits(goog$debug$DebugWindow$Foo,Base)", "");
    t.test("goog.debug.DebugWindow.Foo", "");
    t.test("goog$debug$DebugWindow$Foo", "");
    t.test("goog.debug.DebugWindow.Foo=1", "");
    t.test("goog$debug$DebugWindow$Foo=1", "");
    t.test("goog.debug.DebugWindow.Foo = class {}", "");
    t.test("goog$debug$DebugWindow$Foo = class {}", "");
}

// port: StripCodeTest#testStrippedES5ChildClass
#[test]
fn test_stripped_es5_child_class() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.debug.Logger.Child = function() {};\ngoog.inherits(goog.debug.Logger.Child, goog.debug.Logger);\n",
        "");
    t.test(
        "var goog$debug$Logger$Child = function() {};\ngoog$inherits(goog$debug$Logger$Child, goog$debug$Logger);\n",
        "var goog$debug$Logger$Child = null;");
}

// port: StripCodeTest#testClassInheritanceFromStripType1
#[test]
fn test_class_inheritance_from_strip_type1() {
    let mut t = StripCodeTest::new();
    // Formatter is not a strip name or type, so cannot extend a strip type.
    t.test_error(
        "class Formatter extends goog.debug.Formatter {}",
        &strip_code::STRIP_TYPE_INHERIT_ERROR,
    );
    t.test_error(
        "class Formatter extends goog$debug$Formatter {}",
        &strip_code::STRIP_TYPE_INHERIT_ERROR,
    );
}

// port: StripCodeTest#testClassInheritanceFromStripType2
#[test]
fn test_class_inheritance_from_strip_type2() {
    let mut t = StripCodeTest::new();
    t.test_error(
        "let Formatter = class extends goog.debug.Formatter {}",
        &strip_code::STRIP_TYPE_INHERIT_ERROR,
    );
    t.test_error(
        "let Formatter = class extends goog$debug$Formatter {}",
        &strip_code::STRIP_TYPE_INHERIT_ERROR,
    );
}

// port: StripCodeTest#testClassInheritanceFromStripType3
#[test]
fn test_class_inheritance_from_strip_type3() {
    let mut t = StripCodeTest::new();
    // Both subclass and superclass are strip types, so this is okay.
    t.test(
        "goog.debug.HtmlFormatter = class extends goog.debug.Formatter {}",
        "",
    );
    t.test(
        "let goog$debug$HtmlFormatter = class extends goog$debug$Formatter {}",
        "let goog$debug$HtmlFormatter = null;",
    );
}

// port: StripCodeTest#testPropertyWithEmptyStringKey
#[test]
fn test_property_with_empty_string_key() {
    let mut t = StripCodeTest::new();
    t.test(
        "goog.format.NUMERIC_SCALES_BINARY_ = {'': 1};",
        "goog.format.NUMERIC_SCALES_BINARY_={\"\":1}",
    );
    t.test(
        "let goog$format$NUMERIC_SCALES_BINARY_ = {'': 1};",
        "let goog$format$NUMERIC_SCALES_BINARY_={\"\":1}",
    );
}

// port: StripCodeTest#testVarinIf
#[test]
fn test_varin_if() {
    let mut t = StripCodeTest::new();
    t.test("if(x)var logger=null;else foo()", "if(x);else foo()");
}

// port: StripCodeTest#testGetElemInIf
#[test]
fn test_get_elem_in_if() {
    let mut t = StripCodeTest::new();
    t.test(
        "var logger=null;if(x)logger[f];else foo()",
        "if(x);else foo()",
    );
}

// port: StripCodeTest#testAssignInIf
#[test]
fn test_assign_in_if() {
    let mut t = StripCodeTest::new();
    t.test(
        "var logger=null;if(x)logger=1;else foo()",
        "if(x);else foo()",
    );
}

// port: StripCodeTest#testNamePrefix
#[test]
fn test_name_prefix() {
    let mut t = StripCodeTest::new();
    t.test(
        "a = function(traceZZZ) {}; a.prototype.traceXXX = {x: 1}\na.prototype.z = function() { this.traceXXX.f(); };\nvar traceYYY = 0;\n",
        "a=function(traceZZZ){};a.prototype.z=function(){}");
}

// port: StripCodeTest#testTypePrefix
#[test]
fn test_type_prefix() {
    let mut t = StripCodeTest::new();
    t.test_same("e.f.TraceXXX = function() {};\ne.f.TraceXXX.prototype.yyy = 2;\n");
    t.test_same("let e$f$TraceXXX = function() {};\ne$f$TraceXXX.prototype.yyy = 2;\n");
}

// port: StripCodeTest#testStripCallsToStrippedNames1
#[test]
fn test_strip_calls_to_stripped_names1() {
    let mut t = StripCodeTest::new();
    t.test(
        "a = function() { this.logger_ = function(msg){}; }\na.prototype.b = function() { this.logger_('hi'); }\n",
        "a=function(){};a.prototype.b=function(){}");
    t.test(
        "a = function() {}\na.prototype.logger_ = function(msg) {};\na.prototype.b = function() { this.logger_('hi'); }\n",
        "a=function(){};a.prototype.b=function(){}");
}

// port: StripCodeTest#testStripCallsToStrippedNames2
#[test]
fn test_strip_calls_to_stripped_names2() {
    let mut t = StripCodeTest::new();
    t.test(
        "a = function() {}\na.prototype.logger_ = function(msg) {};\na.prototype.b = function() { this.logger_('hi'); }\n",
        "a=function(){};a.prototype.b=function(){}");
}

// port: StripCodeTest#testStripCallsToStrippedNames3
#[test]
fn test_strip_calls_to_stripped_names3() {
    let mut t = StripCodeTest::new();
    t.test(
        "a = function() { this.logger_ = function(msg){}; }\na.prototype.b = function() { this.logger_('hi').foo = 2; }\n",
        "a=function(){};a.prototype.b=function(){2;}");
}

// port: StripCodeTest#testStripCallsToStrippedNames4
#[test]
fn test_strip_calls_to_stripped_names4() {
    let mut t = StripCodeTest::new();
    t.test("a = this.logger_().foo;", "a = null;");
}

// port: StripCodeTest#testStripVarsInitializedFromStrippedNames1
#[test]
fn test_strip_vars_initialized_from_stripped_names1() {
    let mut t = StripCodeTest::new();
    t.test(
        "a = function() { this.logger_ = function() { return 1; }; }\na.prototype.b = function() {\n  var one = this.logger_(); if (one) foo() }\n",
        "a=function(){};a.prototype.b=function(){if(null)foo()}");
}

// port: StripCodeTest#testStripVarsInitializedFromStrippedNames2
#[test]
fn test_strip_vars_initialized_from_stripped_names2() {
    let mut t = StripCodeTest::new();
    t.test(
        "a = function() { this.logger_ = function() { return 1; }; }\na.prototype.b = function() {\n  var one = this.logger_.foo.bar(); if (one) foo() }\n",
        "a=function(){};a.prototype.b=function(){if(null)foo()}");
}

// port: StripCodeTest#testReportErrorOnStripInNestedAssignment
#[test]
fn test_report_error_on_strip_in_nested_assignment() {
    let mut t = StripCodeTest::new();
    // Strip name
    t.test_error("(foo.logger_ = 7) + 8", &strip_code::STRIP_ASSIGNMENT_ERROR);

    // Strip namespaced type
    t.test_error(
        "(goog.debug.Logger.foo = 7) + 8",
        &strip_code::STRIP_ASSIGNMENT_ERROR,
    );
    t.test_error(
        "(goog$debug$Logger$foo = 7) + 8",
        &strip_code::STRIP_ASSIGNMENT_ERROR,
    );

    // Strip non-namespaced type
    t.test_error(
        "(GA_GoogleDebugger.foo = 7) + 8",
        &strip_code::STRIP_ASSIGNMENT_ERROR,
    );
    t.test_error(
        "(GA_GoogleDebugger$foo = 7) + 8",
        &strip_code::STRIP_ASSIGNMENT_ERROR,
    );
}

// port: StripCodeTest#testNewOperator1
#[test]
fn test_new_operator1() {
    let mut t = StripCodeTest::new();
    t.test(
        "function foo() {} foo.bar = new goog.debug.Logger();",
        "function foo() {} foo.bar = null;",
    );
    t.test(
        "function foo() {} foo.bar = new goog$debug$Logger();",
        "function foo() {} foo.bar = null;",
    );
}

// port: StripCodeTest#testNewOperator2
#[test]
fn test_new_operator2() {
    let mut t = StripCodeTest::new();
    t.test(
        "function foo() {} foo.bar = (new goog.debug.Logger()).foo();",
        "function foo() {} foo.bar = null;",
    );
    t.test(
        "function foo() {} let foo$bar = (new goog$debug$Logger()).foo();",
        "function foo() {} let foo$bar = null;",
    );
}

// port: StripCodeTest#testNewOperator3
#[test]
fn test_new_operator3() {
    let mut t = StripCodeTest::new();
    t.test("(new goog.debug.Logger()).foo().bar = 2;", "2;");
    t.test("(new goog$debug$Logger()).foo().bar = 2;", "2;");
}

// port: StripCodeTest#testCrazyNesting1
#[test]
fn test_crazy_nesting1() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x = {}; x[new goog.debug.Logger()] = 3;",
        "var x = {}; x[null] = 3;",
    );
    t.test(
        "var x = {}; x[new goog$debug$Logger()] = 3;",
        "var x = {}; x[null] = 3;",
    );
}

// port: StripCodeTest#testCrazyNesting2
#[test]
fn test_crazy_nesting2() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x = {}; x[goog.debug.Logger.getLogger()] = 3;",
        "var x = {}; x[null] = 3;",
    );
    t.test(
        "var x = {}; x[goog$debug$Logger.getLogger()] = 3;",
        "var x = {}; x[null] = 3;",
    );
}

// port: StripCodeTest#testCrazyNesting3
#[test]
fn test_crazy_nesting3() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x = function() {}; x(new goog.debug.Logger());",
        "var x = function() {}; x(null);",
    );
    t.test(
        "var x = function() {}; x(new goog$debug$Logger());",
        "var x = function() {}; x(null);",
    );
}

// port: StripCodeTest#testCrazyNesting4
#[test]
fn test_crazy_nesting4() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x = function() {}; x(goog.debug.Logger.getLogger());",
        "var x = function() {}; x(null);",
    );
    t.test(
        "var x = function() {}; x(goog$debug$Logger.getLogger());",
        "var x = function() {}; x(null);",
    );
}

// port: StripCodeTest#testCrazyNesting5
#[test]
fn test_crazy_nesting5() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x = function() {}; var y = {};\nvar z = goog.debug.Logger.getLogger(); x(y[z['foo']]);\n",
        "var x = function() {}; var y = {}; x(y[null]);");
    t.test(
        "var x = function() {}; var y = {};\nvar z = goog$debug$Logger.getLogger(); x(y[z['foo']]);\n",
        "var x = function() {}; var y = {}; x(y[null]);");
}

// port: StripCodeTest#testNamespace1
#[test]
fn test_namespace1() {
    let mut t = StripCodeTest::new();
    t.test(
        "var x = {};x.traceutil = {};x.traceutil.FOO = 1;",
        "var x = {};",
    );
}

// port: StripCodeTest#testMethodCallTriggersRemoval
#[test]
fn test_method_call_triggers_removal() {
    let mut t = StripCodeTest::new();
    t.test("this.logger_.foo.bar();", "");
}

// port: StripCodeTest#testRemoveExpressionByName
#[test]
fn test_remove_expression_by_name() {
    let mut t = StripCodeTest::new();
    t.test("this.logger_.foo.bar;", "");
}

// port: StripCodeTest#testAliasOfRemovedVar
#[test]
fn test_alias_of_removed_var() {
    let mut t = StripCodeTest::new();
    t.test(
        "var logger_ = goog.debug.Logger.getLogger(); var alias; alias = logger_;",
        "                                             var alias; alias = null;",
    );
    t.test(
        "var logger_ = goog$debug$Logger.getLogger(); var alias; alias = logger_;",
        "                                             var alias; alias = null;",
    );
}

// port: StripCodeTest#testComplexExpression
#[test]
fn test_complex_expression() {
    let mut t = StripCodeTest::new();
    t.test(
        "var logger_ = goog.debug.Logger.getLogger(); var alias; alias = (logger_ = 3) + 4;",
        "                                             var alias; alias = 3 + 4;",
    );
    t.test(
        "var logger_ = goog$debug$Logger.getLogger(); var alias; alias = (logger_ = 3) + 4;",
        "                                             var alias; alias = 3 + 4;",
    );
}

// port: StripCodeTest#testStrippedFunctionCallAssignedToNonStrippedStaticField
#[test]
fn test_stripped_function_call_assigned_to_non_stripped_static_field() {
    let mut t = StripCodeTest::new();
    t.test(
        "a.b.C = function() {\n  this.nonStripped_ = a.b.C.nonStripped_;\n}\na.b.C.prototype.method = function() {\n  goog.log(this.nonStripped_, 'some message');\n}\na.b.C.nonStripped_ = goog.debug.Logger.getLogger();\n",
        "a.b.C = function() {\n  this.nonStripped_ = a.b.C.nonStripped_;\n}\na.b.C.prototype.method = function() {\n}\na.b.C.nonStripped_ = null;\n");
    t.test(
        "let a$b$C = function() {\n  this.nonStripped_ = a$b$C$nonStripped_;\n}\na$b$C.prototype.method = function() {\n  goog$log(this.nonStripped_, 'some message');\n}\nvar a$b$C$nonStripped_ = goog$debug$Logger$getLogger();\n",
        "let a$b$C = function() {\n  this.nonStripped_ = a$b$C$nonStripped_;\n}\na$b$C.prototype.method = function() {\n}\nvar a$b$C$nonStripped_ = null;\n");
}

// port: StripCodeTest#testVarsIntroducedByCollapsePropertiesInitializedToNull
#[test]
fn test_vars_introduced_by_collapse_properties_initialized_to_null() {
    let mut t = StripCodeTest::new();
    // There's known code where `a.b.strippableName` appears in the AST earlier
    // than where it is assigned a value. CollapseProperties will turn this into
    // a variable before StripCode sees it, so the reference will appear in the AST
    // earlier than the declaration.
    //
    // For such cases, we need to make sure we initialize the variable
    // to `null` instead of removing the declaration. Otherwise, we'll get an
    // error later due to the reference to a variable without a declaration.
    //
    // It would be nicer if we replaced all references to the variable with `null`
    // and removed its declaration, but doing that makes some other things we
    // want to remove harder to recognize when we visit them. (e.g. `goog$inherits(null, null)`
    // where we need to see `goog$inherits(strippable$Child, strippable$Parent)`).
    t.test(
        "let a$b$C = function() {\n  this.nonStripped_ = a$b$C$logger_;\n}\na$b$C.prototype.method = function() {\n  goog$log(this.nonStripped_, 'some message');\n  this.obj[a$b$C$logger_];\n}\nvar a$b$C$logger_ = {};\n",
        "let a$b$C = function() {\n  this.nonStripped_ = a$b$C$logger_;\n}\na$b$C.prototype.method = function() {\n  this.obj[a$b$C$logger_];\n}\nvar a$b$C$logger_ = null;\n");
    // Also handle the case where there's no initial value
    // assigned to the variable.
    t.test(
        "let a$b$C = function() {\n  this.nonStripped_ = a$b$C$logger_;\n}\na$b$C.prototype.method = function() {\n  goog$log(this.nonStripped_, 'some message');\n  this.obj[a$b$C$logger_];\n}\nvar a$b$C$logger_;\n",
        "let a$b$C = function() {\n  this.nonStripped_ = a$b$C$logger_;\n}\na$b$C.prototype.method = function() {\n  this.obj[a$b$C$logger_];\n}\nvar a$b$C$logger_ = null;\n");
}
