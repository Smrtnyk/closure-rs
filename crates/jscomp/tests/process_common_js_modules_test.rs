/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/ModulesTestUtils.java,
//   test/com/google/javascript/jscomp/ProcessCommonJSModulesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of ProcessCommonJSModulesTest. The tests run through the native CompilerTestCase port of
//! crates/testing (`test(srcs(..), expected(..))`), with ModulesTestUtils#testModules and
//! #testModulesError as fixture methods.
use closure_jscomp::{
    compiler_options::CompilerOptions,
    deps::module_loader::{self, ResolutionMode},
    diagnostic_type::DiagnosticType,
    js_chunk::JSChunk,
    process_common_js_modules::{ProcessCommonJSModules, SUSPICIOUS_EXPORTS_ASSIGNMENT},
    source_file::SourceFile,
};
use closure_rhino::fast_hash::IndexMap;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, Expected, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct ProcessCommonJSModulesTest {
    ctx: Ctx,
    module_roots: Option<Vec<String>>,
    resolution_mode: ResolutionMode,
}

impl CompilerTestCaseHooks for ProcessCommonJSModulesTest {
    // port: ProcessCommonJSModulesTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_process_common_js_modules(true);
        options.set_module_resolution_mode(self.resolution_mode);

        if let Some(module_roots) = &self.module_roots {
            options.set_module_roots(module_roots.clone());
        }

        Ok(options)
    }

    // port: ProcessCommonJSModulesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            ProcessCommonJSModules::new(),
        )))))
    }

    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl ProcessCommonJSModulesTest {
    // port: CompilerTestCase#setUp (ProcessCommonJSModulesTest has no setUp of its own)
    fn set_up() -> (CompilerTestCase, Self) {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        let test = Self {
            ctx: Ctx::new(
                "ProcessCommonJSModulesTest".into(),
                object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            module_roots: None,
            resolution_mode: ResolutionMode::NODE,
        };
        (harness, test)
    }

    // port: ProcessCommonJSModulesTest#testModules
    fn test_modules(
        &mut self,
        h: &mut CompilerTestCase,
        filename: &str,
        input: &str,
        expected: &str,
    ) {
        modules_test_utils_test_modules(h, self, filename, input, expected);
    }

    // port: ModulesTestUtils#testModulesError (as called by ProcessCommonJSModulesTest)
    fn test_modules_error(
        &mut self,
        h: &mut CompilerTestCase,
        input: &str,
        error: &'static DiagnosticType,
    ) {
        let inputs = vec![
            Arc::new(SourceFile::from_code("other.js", "")),
            Arc::new(SourceFile::from_code("testcode.js", input)),
        ];
        h.test_error_sources(
            self,
            CompilerTestCase::srcs_files(inputs),
            CompilerTestCase::error(error),
        )
        .unwrap();
    }
}

// port: ModulesTestUtils#testModules
fn modules_test_utils_test_modules(
    test: &mut CompilerTestCase,
    hooks: &mut ProcessCommonJSModulesTest,
    file_name: &str,
    input: &str,
    expected: &str,
) {
    // Shared with ProcessCommonJSModulesTest.
    let inputs = vec![
        Arc::new(SourceFile::from_code(
            "other.js",
            "goog.provide('module$other');",
        )),
        Arc::new(SourceFile::from_code(
            "yet_another.js",
            "goog.provide('module$yet_another');",
        )),
        Arc::new(SourceFile::from_code(file_name, input)),
    ];
    let expecteds = vec![
        Arc::new(SourceFile::from_code(
            "other.js",
            "goog.provide('module$other');",
        )),
        Arc::new(SourceFile::from_code(
            "yet_another.js",
            "goog.provide('module$yet_another');",
        )),
        Arc::new(SourceFile::from_code(file_name, expected)),
    ];
    test.test(
        hooks,
        vec![
            TestPart::Sources(CompilerTestCase::srcs_files(inputs)),
            TestPart::Expected(CompilerTestCase::expected_files(expecteds)),
        ],
    )
    .unwrap();
}

/// `SourceFile.fromCode(name, code)` for each pair.
fn source_files(files: &[(&str, &str)]) -> Vec<Arc<SourceFile>> {
    files
        .iter()
        .map(|(name, code)| Arc::new(SourceFile::from_code(name, *code)))
        .collect()
}

// port: CompilerTestCase#srcs(SourceFile...)
fn srcs(files: &[(&str, &str)]) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_files(source_files(files)))
}

// port: CompilerTestCase#expected(SourceFile...)
fn expected(files: &[(&str, &str)]) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected_files(source_files(files)))
}

// ImmutableMap.of(k1, v1, ...)
fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// port: ProcessCommonJSModulesTest#testMultipleAssignments
#[test]
fn test_multiple_assignments() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();

    let chunk = JSChunk::new("out");
    chunk.add_source_file(SourceFile::from_code(
        "other.js",
        "goog.provide('module$other');",
    ));
    chunk.add_source_file(SourceFile::from_code(
        "yet_another.js",
        "goog.provide('module$yet_another');",
    ));
    chunk.add_source_file(SourceFile::from_code(
        "test",
        r#"/** @constructor */ function Hello() {}
module.exports = Hello;
/** @constructor */ function Bar() {}
Bar.prototype.foobar = function() { alert('foobar'); };
exports = Bar;
"#,
    ));
    let chunks = vec![chunk];
    h.test(
        &mut t,
        vec![
            TestPart::Sources(CompilerTestCase::srcs_chunks(chunks)),
            TestPart::Expected(Expected::new(None)),
            TestPart::Diagnostic(CompilerTestCase::warning(&SUSPICIOUS_EXPORTS_ASSIGNMENT)),
        ],
    )
    .unwrap();
}

// port: ProcessCommonJSModulesTest#testWebpackAMDModuleShim
#[test]
fn test_webpack_amd_module_shim() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    let webpack_modules_by_id = map(&[("1", "test.js"), ("2", "/webpack/buildin/module.js")]);
    h.set_webpack_modules_by_id(webpack_modules_by_id);
    t.resolution_mode = ResolutionMode::WEBPACK;

    // Shared with ProcessCommonJSModulesTest.
    let inputs = source_files(&[
        (
            "test.js",
            r#"(function(module) {
  console.log(module.id);
})(__webpack_require__(2)(module))
"#,
        ),
        (
            "/webpack/buildin/module.js",
            "module.exports = function(module) { return module; };",
        ),
    ]);
    let expecteds = source_files(&[
        ("test.js", "(function(){console.log('test.js')})()"),
        (
            "/webpack/buildin/module.js",
            r#"/** @const */ var module$webpack$buildin$module = {};
/** @const */ module$webpack$buildin$module.default =
    function(module) { return module; };
"#,
        ),
    ]);
    h.test(
        &mut t,
        vec![
            TestPart::Sources(CompilerTestCase::srcs_files(inputs)),
            TestPart::Expected(CompilerTestCase::expected_files(expecteds)),
        ],
    )
    .unwrap();
}

// port: ProcessCommonJSModulesTest#testWithoutExports
#[test]
fn test_without_exports() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "var name = require('./other'); name.call(null)",
        r#"var name = module$other.default;
module$other.default.call(null);
"#,
    );
    h.test(
        &mut t,
        vec![
            srcs(&[
                ("mod/name.js", "module.exports = {};"),
                (
                    "test/sub.js",
                    r#"var name = require('../mod/name');
(function() { let foo = name; foo(); })();
"#,
                ),
            ]),
            expected(&[
                (
                    "mod/name.js",
                    "/** @const */ var module$mod$name = {/** @const */ default: {}};",
                ),
                (
                    "test/sub.js",
                    r#"var name = module$mod$name.default;
(function() { let foo = module$mod$name.default; foo(); })();
"#,
                ),
            ]),
        ],
    )
    .unwrap();
}

// port: ProcessCommonJSModulesTest#testExports
#[test]
fn test_exports() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = require('./other');
exports.foo = 1;
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var name$$module$test = module$other.default;
module$test.default.foo = 1;
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = require('./other');
module.exports = function() {};
"#,
        r#"/** @const */ var module$test = {};
var name$$module$test = module$other.default;
/** @const */ module$test.default = function () {};
"#,
    );
}

// port: ProcessCommonJSModulesTest#testExportsInExpression
#[test]
fn test_exports_in_expression() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = require('./other');
var e;
e = module.exports = function() {};
"#,
        r#"/** @const */ var module$test = {};
var name$$module$test = module$other.default;
var e$$module$test;
e$$module$test = /** @const */ module$test.default = function () {};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = require('./other');
var e = module.exports = function() {};
"#,
        r#"/** @const */ var module$test = {};
var name$$module$test = module$other.default;
var e$$module$test = /** @const */ module$test.default = function () {};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = require('./other');
(module.exports = function() {})();
"#,
        r#"/** @const */ var module$test = {};
var name$$module$test = module$other.default;
(/** @const */ module$test.default = function () {})();
"#,
    );
}

// port: ProcessCommonJSModulesTest#testPropertyExports
#[test]
fn test_property_exports() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "exports.one = 1; module.exports.obj = {}; module.exports.obj.two = 2;",
        r#"/** @const */ var module$test = {default: {}};
module$test.default.one = 1;
module$test.default.obj = {};
module$test.default.obj.two = 2;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testModuleExportsWrittenWithExportsRefs
#[test]
fn test_module_exports_written_with_exports_refs() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"exports.one = 1;
module.exports = {};
"#,
        "/** @const */ var module$test = { default: {}}; module$test.default.one = 1;",
    );
}

// port: ProcessCommonJSModulesTest#testVarRenaming
#[test]
fn test_var_renaming() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"module.exports = {};
var a = 1, b = 2;
(function() { var a; b = 4})();
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var a$$module$test = 1;
var b$$module$test = 2;
(function() { var a; b$$module$test = 4})();
"#,
    );
}

// port: ProcessCommonJSModulesTest#testDash
#[test]
fn test_dash() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test-test.js",
        r#"var name = require('./other');
exports.foo = 1;
"#,
        r#"/** @const */ var module$test_test = {/** @const */ default: {}};
var name$$module$test_test=module$other.default
module$test_test.default.foo = 1;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testIndex
#[test]
fn test_index() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "foo/index.js",
        r#"var name = require('../other');
exports.bar = 1;
"#,
        r#"/** @const */ var module$foo$index = {/** @const */ default: {}};
var name$$module$foo$index = module$other.default;
module$foo$index.default.bar = 1;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testVarJsdocGoesOnAssignment
#[test]
fn test_var_jsdoc_goes_on_assignment() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "testcode.js",
        r#"/**
 * @const
 * @enum {number}
 */
var MyEnum = { ONE: 1, TWO: 2 };
module.exports = {MyEnum: MyEnum};
"#,
        r#"/** @const */ var module$testcode = {/** @const */ default: {}};
/**
 * @const
 * @enum {number}
 */
(module$testcode.default.MyEnum = {ONE:1, TWO:2});
"#,
    );
}

// port: ProcessCommonJSModulesTest#testModuleName
#[test]
fn test_module_name() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "foo/bar.js",
        r#"var name = require('../other');
module.exports = name;
"#,
        r#"/** @const */ var module$foo$bar = {};
var name$$module$foo$bar = module$other.default;
/** @const */ module$foo$bar.default = module$other.default;
"#,
    );
    h.test(
        &mut t,
        vec![
            srcs(&[
                ("foo/name.js", ""),
                (
                    "foo/bar.js",
                    r#"var name = require('./name');
module.exports = name;
"#,
                ),
            ]),
            expected(&[
                ("foo/name.js", ""),
                (
                    "foo/bar.js",
                    r#"/** @const */ var module$foo$bar = {};
var name$$module$foo$bar = module$foo$name.default;
/** @const */ module$foo$bar.default = module$foo$name.default;
"#,
                ),
            ]),
        ],
    )
    .unwrap();
}

// port: ProcessCommonJSModulesTest#testModuleExportsScope
#[test]
fn test_module_exports_scope() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foo = function (module) {
  module.exports = {};
};
module.exports = foo;
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = function (module) {
  module.exports={};
};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foo = function () {
  var module = {};
  module.exports = {};
};
module.exports = foo;
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = function() {
  var module={};
  module.exports={}
};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foo = function () {
  if (true) var module = {};
  module.exports = {};
};
module.exports = foo;
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = function() {
  if (true) var module={};
  module.exports={}
};
"#,
    );
}

// port: ProcessCommonJSModulesTest#testUMDPatternConversion
#[test]
fn test_umd_pattern_conversion() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof window.define === 'function' && window.define.amd) {
  window.define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof module === 'object' && module['exports']) {
  module['exports'] = foobar;
} else if (typeof define === 'function' && define['amd']) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else {
  this.foobar = foobar;
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof window.define === 'function' && window.define.amd) {
  window.define([], function() {return foobar;});
} else if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else {
  this.foobar = foobar;
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof define === 'function' && define['amd']) {
  define([], function() {return foobar;});
} else if (typeof module === 'object' && module['exports']) {
  module['exports'] = foobar;
} else {
  this.foobar = foobar;
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
}
if (typeof define === 'function' && define.amd) {
  define([], function () {return foobar;});
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
}
if (typeof window.define === 'function' && window.define.amd) {
  window.define([], function () {return foobar;});
}
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(&mut h, "test.js", r#"(function(global, factory) {
  true ? module.exports = factory(
             typeof angular === 'undefined' ? require('./other') :
                                              angular) :
         typeof define === 'function' && define.amd ?
         define('angular-cache', ['angular'], factory) :
         (global.angularCacheModuleName = factory(global.angular));
}(this, function(angular) {
  'use strict';
  console.log(angular);
  return angular;
}));
"#, r#"/** @const */ var module$test = {};
var global$$module$test = this;
var factory$$module$test = function(angular) {
  console.log(angular);
  return angular;
};
/** @const */
module$test.default = factory$$module$test(typeof angular === "undefined" ? module$other.default : angular);
"#);
    t.test_modules(&mut h, "test.js", r#"(function(global, factory) {
  true ? module.exports = factory(
             typeof angular === 'undefined' ? require('./other') :
                                              angular) :
         typeof window.define === 'function' && window.define.amd ?
         window.define('angular-cache', ['angular'], factory) :
         (global.angularCacheModuleName = factory(global.angular));
}(this, function(angular) {
  'use strict';
  console.log(angular);
  return angular;
}));
"#, r#"/** @const */ var module$test = {};
var global$$module$test = this;
var factory$$module$test = function(angular) {
  console.log(angular);
  return angular;
};
/** @const */
module$test.default = factory$$module$test(typeof angular === "undefined" ? module$other.default : angular);
"#);
}

// port: ProcessCommonJSModulesTest#testEs6ObjectShorthand
#[test]
fn test_es6_object_shorthand() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"function foo() {}
module.exports = {
  prop: 'value',
  foo
};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.foo = function () {};
module$test.default.prop = 'value';
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"module.exports = {
  prop: 'value',
  foo() {
    console.log('bar');
  }
};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.prop = 'value';
module$test.default.foo = function() {
  console.log('bar');
};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var a = require('./other');
module.exports = {a: a};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var a$$module$test = module$other.default;
module$test.default.a = module$other.default;
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var a = require('./other');
module.exports = {a};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var a$$module$test = module$other.default;
module$test.default.a = module$other.default;
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"var a = 4;
module.exports = {a};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.a = 4;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testKeywordsInExports
#[test]
fn test_keywords_in_exports() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "testcode.js",
        r#"var a = 4;
module.exports = { else: a };
"#,
        r#"/** @const */ var module$testcode = {/** @const */ default: {}};
module$testcode.default.else = 4;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testRequireResultUnused
#[test]
fn test_require_result_unused() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(&mut h, "test.js", "require('./other');", "");
}

// port: ProcessCommonJSModulesTest#testRequireEnsure
#[test]
fn test_require_ensure() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"require.ensure(['./other'], function(require) {
  var other = require('./other');
  var bar = other;
});
"#,
        r#"(function() {
  var other = module$other.default;
  var bar = module$other.default;
})()
"#,
    );
}

// port: ProcessCommonJSModulesTest#testFunctionRewriting
#[test]
fn test_function_rewriting() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"function foo() {}
foo.prototype = new Date();
module.exports = foo;
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = function() {};
module$test.default.prototype = new Date();
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"function foo() {}
foo.prototype = new Date();
module.exports = {foo: foo};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.foo = function () {};
module$test.default.foo.prototype = new Date();
"#,
    );
}

// port: ProcessCommonJSModulesTest#testFunctionHoisting
#[test]
fn test_function_hoisting() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"module.exports = foo;
function foo() {}
foo.prototype = new Date();
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = function() {};
module$test.default.prototype = new Date();
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"function foo() {}
Object.assign(foo, { bar: foobar });
function foobar() {}
module.exports = foo;
module.exports.bar = foobar;
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = function () {};
module$test.default.bar = function() {};
Object.assign(module$test.default, { bar: module$test.default.bar });
"#,
    );
}

// port: ProcessCommonJSModulesTest#testClassRewriting
#[test]
fn test_class_rewriting() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"class foo extends Array {}
module.exports = foo;
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = class extends Array {};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"class foo {}
module.exports = foo;
"#,
        "/** @const */ var module$test = {}; /** @const */ module$test.default = class {}",
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"class foo {}
module.exports.foo = foo;
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.foo = class {};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        "module.exports = class { bar() { return 'bar'; }};",
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = class {
  bar() { return 'bar'; }
};
"#,
    );
}

// port: ProcessCommonJSModulesTest#testDestructuringImports
#[test]
fn test_destructuring_imports() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"const {foo, bar} = require('./other');
var baz = foo + bar;
"#,
        r#"const {foo, bar} = module$other.default;
var baz = module$other.default.foo + module$other.default.bar;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testDestructuringImports2
#[test]
fn test_destructuring_imports2() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"const {foo, bar: {baz}} = require('./other');
module.exports = true;
"#,
        r#"/** @const */ var module$test = {};
const {foo: foo$$module$test, bar: {baz: baz$$module$test}} = module$other.default;
/** @const */ module$test.default = true;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testAnnotationsCopied
#[test]
fn test_annotations_copied() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"/** @interface */ var a;
/** @type {string} */ a.prototype.foo;
module.exports.a = a;
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
/** @interface */ module$test.default.a;
/** @type {string} */ module$test.default.a.prototype.foo;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testUMDRemoveIIFE
#[test]
fn test_umd_remove_iife() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}})()
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"!function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}}()
"#,
        "/** @const */ var module$test = {}; /** @const */ module$test.default = {foo: 'bar'};",
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"!function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}}()
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#";;;(function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}})()
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}}.call(this))
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {foo: 'bar'};
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#";;;(function(global){
var foobar = {foo: 'bar'};
global.foobar = foobar;
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  global.foobar = foobar;
}})(this)
"#,
        r#"/** @const */ var module$test = { default: {}};
module$test.default = {foo: 'bar'};
module$test.default.foobar = module$test.default;
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function(global){
var foobar = {foo: 'bar'};
global.foobar = foobar;
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  global.foobar = foobar;
}}.call(this, this))
"#,
        r#"/** @const */ var module$test = { default: {}};
module$test.default = {foo: 'bar'};
module$test.default.foobar = module$test.default;
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}}.call(window))
"#,
        r#"/** @const */ var module$test = {};
(function(){
  var foobar = {foo: 'bar'};
  /** @const */ module$test.default=foobar;
}).call(window);
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}})();
alert('foo');
"#,
        r#"/** @const */ var module$test = {};
(function(){
  var foobar = {foo: 'bar'};
  /** @const */ module$test.default = foobar;
})();
alert('foo');
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"alert('foo');
(function(){
var foobar = {foo: 'bar'};
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  this.foobar = foobar;
}})();
"#,
        r#"/** @const */ var module$test = {};
alert('foo');
(function(){
  var foobar={foo:"bar"};
  /** @const */ module$test.default=foobar;
})();
"#,
    );
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function(global){
/** @param {...*} var_args */
function log(var_args) {}
var foobar = {foo: 'bar', log: function() { log.apply(null, arguments); } };
global.foobar = foobar;
if (typeof module === 'object' && module.exports) {
  module.exports = foobar;
} else if (typeof define === 'function' && define.amd) {
  define([], function() {return foobar;});
} else {
  global.foobar = foobar;
}}.call(this, this))
"#,
        r#"/** @const */ var module$test = { default: {}};
/** @param {...*} var_args */
function log$$module$test(var_args){}
module$test.default = {
  foo: 'bar',
  log: function() { log$$module$test.apply(null,arguments); }
};
module$test.default.foobar = module$test.default;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testParamShadow
#[test]
fn test_param_shadow() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"/** @constructor */ function Foo() {}
/** @constructor */ function Bar(Foo) { this.foo = new Foo(); }
Foo.prototype.test = new Bar(Foo);
module.exports = Foo;
"#,
        r#"/** @const */ var module$test = {};
/** @const @constructor */ module$test.default = function () {};
/** @constructor */ function Bar$$module$test(Foo) { this.foo = new Foo(); }
module$test.default.prototype.test = new Bar$$module$test(module$test.default);
"#,
    );
}

// port: ProcessCommonJSModulesTest#testIssue2308
#[test]
fn test_issue2308() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "exports.y = null; var x; x = exports.y;",
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.y = null;
var x$$module$test;
x$$module$test = module$test.default.y
"#,
    );
}

// port: ProcessCommonJSModulesTest#testAbsoluteImportsWithModuleRoots
#[test]
fn test_absolute_imports_with_module_roots() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.module_roots = Some(vec!["/base".to_string()]);
    h.test(
        &mut t,
        vec![
            srcs(&[
                ("base/mod/name.js", "module.exports = {}"),
                (
                    "base/test/sub.js",
                    r#"var name = require('/mod/name');
(function() { let foo = name; foo(); })();
"#,
                ),
            ]),
            expected(&[
                (
                    "base/mod/name.js",
                    "/** @const */ var module$mod$name = {/** @const */ default: {}};",
                ),
                (
                    "base/test/sub.js",
                    r#"var name = module$mod$name.default;
(function() { let foo = module$mod$name.default; foo(); })();
"#,
                ),
            ]),
        ],
    )
    .unwrap();
}

// port: ProcessCommonJSModulesTest#testIssue2510
#[test]
fn test_issue2510() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "module.exports = {a: 1, get b() { return 2; }};",
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = {
  get b() { return 2; }
};
module$test.default.a = 1;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testIssue2450
#[test]
fn test_issue2450() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var BCRYPT_BLOCKS = 8,
    BCRYPT_HASHSIZE = 32;

module.exports = {
  BLOCKS: BCRYPT_BLOCKS,
  HASHSIZE: BCRYPT_HASHSIZE,
};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.BLOCKS = 8;
module$test.default.HASHSIZE = 32;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testWebpackAmdPattern
#[test]
fn test_webpack_amd_pattern() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var __WEBPACK_AMD_DEFINE_ARRAY__, __WEBPACK_AMD_DEFINE_RESULT__;
!(__WEBPACK_AMD_DEFINE_ARRAY__ =
      [__webpack_require__(1), __webpack_require__(2)],
  __WEBPACK_AMD_DEFINE_RESULT__ =
      function(b, c) {
        console.log(b, c.exportA, c.exportB);
      }.apply(exports, __WEBPACK_AMD_DEFINE_ARRAY__),
  __WEBPACK_AMD_DEFINE_RESULT__ !== undefined &&
      (module.exports = __WEBPACK_AMD_DEFINE_RESULT__));
"#,
        r#"/** @const */ var module$test = { default: {}};
var __WEBPACK_AMD_DEFINE_ARRAY__$$module$test;
!(__WEBPACK_AMD_DEFINE_ARRAY__$$module$test =
    [__webpack_require__(1), __webpack_require__(2)],
    module$test.default = function(b,c){console.log(b,c.exportA,c.exportB)}
        .apply(module$test.default,__WEBPACK_AMD_DEFINE_ARRAY__$$module$test),
    module$test.default!==undefined && module$test.default)
"#,
    );
    t.test_modules(&mut h, "test.js", r#"/** @suppress {duplicate} */var __WEBPACK_AMD_DEFINE_RESULT__;(function() {
  var dialogPolyfill = {prop: 'DIALOG_POLYFILL'};

  if ('function' === 'function' && 'amd' in __webpack_require__(124)) {
    // AMD support
    !(__WEBPACK_AMD_DEFINE_RESULT__ = (function() { return dialogPolyfill; }).call(exports, __webpack_require__, exports, module),
        __WEBPACK_AMD_DEFINE_RESULT__ !== undefined && (module.exports = __WEBPACK_AMD_DEFINE_RESULT__));
  } else if (typeof module === 'object' && typeof module['exports'] === 'object') {
    // CommonJS support
    module['exports'] = dialogPolyfill;
  } else {
    // all others
    window['dialogPolyfill'] = dialogPolyfill;
  }
})();
"#, r#"/** @const */ var module$test = {default: {}};
/** @suppress {duplicate} */
var __WEBPACK_AMD_DEFINE_RESULT__$$module$test;
(function () {
  var dialogPolyfill = {prop: "DIALOG_POLYFILL"};
  !(__WEBPACK_AMD_DEFINE_RESULT__$$module$test = function () {
    return dialogPolyfill
  }.call(module$test.default, __webpack_require__, module$test.default, {}),
  __WEBPACK_AMD_DEFINE_RESULT__$$module$test !== undefined && (module$test.default = __WEBPACK_AMD_DEFINE_RESULT__$$module$test))
})()
"#);
    let webpack_modules_by_id = map(&[
        ("1", "other.js"),
        ("yet_another.js", "yet_another.js"),
        ("3", "test.js"),
    ]);
    h.set_webpack_modules_by_id(webpack_modules_by_id);
    t.resolution_mode = ResolutionMode::WEBPACK;
    t.test_modules(&mut h, "test.js", r#"/** @suppress {duplicate} */var __WEBPACK_AMD_DEFINE_ARRAY__, __WEBPACK_AMD_DEFINE_RESULT__;(function (root, factory) {
  if (true) {
    !(__WEBPACK_AMD_DEFINE_ARRAY__ = [__webpack_require__(1),__webpack_require__('yet_another.js')], __WEBPACK_AMD_DEFINE_RESULT__ = function (a0,b1) {
      return (factory(a0,b1));
    }.apply(exports, __WEBPACK_AMD_DEFINE_ARRAY__),
      __WEBPACK_AMD_DEFINE_RESULT__ !== undefined && (module.exports = __WEBPACK_AMD_DEFINE_RESULT__));
  } else if (typeof module === 'object' && module.exports) {
    module.exports = factory(require('angular'),require('tinymce'));
  } else {
    root['banno.wysiwyg'] = factory(root['angular'],root['tinymce']);
  }
}(this, function (angular, tinymce) {
  console.log(angular, tinymce);
}))
"#, r#"/** @const */ var module$test = {default: {}};
/** @suppress {duplicate} */
var __WEBPACK_AMD_DEFINE_ARRAY__$$module$test;
/** @suppress {duplicate} */
module$test.default;
var root$$module$test = this;
var factory$$module$test = function (angular, tinymce) {
  console.log(angular, tinymce)
};
!(__WEBPACK_AMD_DEFINE_ARRAY__$$module$test = [module$other.default,
  module$yet_another.default], module$test.default = function (a0, b1) {
  return factory$$module$test(a0, b1)
}.apply(module$test.default, __WEBPACK_AMD_DEFINE_ARRAY__$$module$test),
  module$test.default !== undefined && module$test.default)
"#);
}

// port: ProcessCommonJSModulesTest#testIssue2593
#[test]
fn test_issue2593() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var first = 1,
    second = 2,
    third = 3,
    fourth = 4,
    fifth = 5;

module.exports = {};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var first$$module$test=1;
var second$$module$test=2;
var third$$module$test=3;
var fourth$$module$test=4;
var fifth$$module$test=5;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testTernaryUMDWrapper
#[test]
fn test_ternary_umd_wrapper() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foobar = {foo: 'bar'};
typeof module === 'object' && module.exports ?
   module.exports = foobar :
   typeof define === 'function' && define.amd ?
     define([], function() {return foobar;}) :
     this.foobar = foobar;
"#,
        "/** @const */ var module$test = {}; /** @const */ module$test.default = {foo: 'bar'};",
    );
}

// port: ProcessCommonJSModulesTest#testLeafletUMDWrapper
#[test]
fn test_leaflet_umd_wrapper() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function (global, factory) {
  typeof exports === 'object' && typeof module !== 'undefined' ?
    factory(exports) :
    typeof define === 'function' && define.amd ?
      define(['exports'], factory) :
      (factory((global.L = {})));
}(this, (function (exports) {
  'use strict';
  var webkit = userAgentContains('webkit');
  function userAgentContains(str) {
    return navigator.userAgent.toLowerCase().indexOf(str) >= 0;
  }
  exports.webkit = webkit
})));
"#,
        r#"/** @const */ var module$test={/** @const */ default: {}};
var global$$module$test = this;
var factory$$module$test = function(exports) {
  var webkit = userAgentContains("webkit");
  function userAgentContains(str) {
    return navigator.userAgent.toLowerCase().indexOf(str) >= 0;
  }
  exports.webkit = webkit;
};
factory$$module$test(module$test.default);
"#,
    );
}

// port: ProcessCommonJSModulesTest#testBowserUMDWrapper
#[test]
fn test_bowser_umd_wrapper() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"!function (root, name, definition) {
  if (typeof module != 'undefined' && module.exports)
    module.exports = definition()
  else if (typeof define == 'function' && define.amd)
    define(name, definition)
  else root[name] = definition()
}(this, 'foobar', function () {
  return {foo: 'bar'};
});
"#,
        r#"/** @const */ var module$test={};
var root$$module$test = this;
var name$$module$test = "foobar";
var definition$$module$test = function() {
  return {foo: 'bar'};
};
/** @const */
module$test.default = definition$$module$test();
"#,
    );
}

// port: ProcessCommonJSModulesTest#testDontSplitVarsInFor
#[test]
fn test_dont_split_vars_in_for() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "for (var a, b, c; ;) {}",
        "for (var a, b, c; ;) {}",
    );
}

// port: ProcessCommonJSModulesTest#testIssue2918
#[test]
fn test_issue2918() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"for (var a, b; a < 4; a++) {};
module.exports = {}
"#,
        r#"/** @const */ var module$test = {/** @const */ default:{}};
for(var a$$module$test,b$$module$test;a$$module$test<4;a$$module$test++) {};
"#,
    );
}

// port: ProcessCommonJSModulesTest#testExportsDirectAssignment
#[test]
fn test_exports_direct_assignment() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "exports = module.exports = {};",
        "/** @const */ var module$test = {/** @const */ default: {}};",
    );
}

// port: ProcessCommonJSModulesTest#testExportsPropertyHoisting
#[test]
fn test_exports_property_hoisting() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"exports.Buffer = Buffer;
Buffer.TYPED_ARRAY_SUPPORT = {};
function Buffer() {}
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.Buffer = function() {};
module$test.default.Buffer.TYPED_ARRAY_SUPPORT = {};
"#,
    );
}

// port: ProcessCommonJSModulesTest#testExportNameInParamList
#[test]
fn test_export_name_in_param_list() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var tinymce = { foo: 'bar' };
function register(cb) { cb(tinymce); }
register(function(tinymce) { module.exports = tinymce; });
"#,
        r#"/** @const */ var module$test = {};
var tinymce$$module$test = { foo: 'bar' };
function register$$module$test(cb) { cb(tinymce$$module$test); }
register$$module$test(function(tinymce) {
  /** @const */ module$test.default = tinymce;
});
"#,
    );
}

// port: ProcessCommonJSModulesTest#testIssue2616
#[test]
fn test_issue2616() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"var foo = function foo() {
  return 1;
};
module.exports = {
  foo: foo,
};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.foo = function foo() {
  return 1;
};
"#,
    );
}

// port: ProcessCommonJSModulesTest#testFingerprintUmd
#[test]
fn test_fingerprint_umd() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"(function (name, context, definition) {
  'use strict';
  if (typeof define === 'function' && define.amd) {
    define(definition);
  } else if (typeof module !== 'undefined' && module.exports) {
    module.exports = definition();
  } else if (context.exports) {
    context.exports = definition();
  } else {
    context[name] = definition();
  }
})('Fingerprint2', this, function() {
  var Fingerprint2 = function() {
    if (!(this instanceof Fingerprint2)) { return new Fingerprint2(); }
  };
  return Fingerprint2;
})
"#,
        r#"/** @const */ var module$test = {};
var name$$module$test = "Fingerprint2";
var context$$module$test = this;
var definition$$module$test = function() {
  var Fingerprint2 = function() {
    if (!(this instanceof Fingerprint2)) {
      return new Fingerprint2();
    }
  };
  return Fingerprint2;
};
/** @const */
module$test.default = definition$$module$test();
"#,
    );
}

// port: ProcessCommonJSModulesTest#testTypeofModuleReference
#[test]
fn test_typeof_module_reference() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"module.exports = 'foo';
console.log(typeof module);
console.log(typeof exports);
"#,
        r#"/** @const */ var module$test={ default: {}};
module$test.default = 'foo';
console.log('object');
console.log('object');
"#,
    );
}

// port: ProcessCommonJSModulesTest#testUpdateGenericTypeReferences
#[test]
fn test_update_generic_type_references() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"const Foo = require('./other');
/** @type {!Array<!Foo>} */ const bar = [];
module.exports = bar;
"#,
        r#"/** @const */ var module$test={};
const Foo$$module$test = module$other.default;
/** @const  @type {!Array<!module$other.default>} */ module$test.default = [];
"#,
    );
}

// port: ProcessCommonJSModulesTest#testMissingRequire
#[test]
fn test_missing_require() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules_error(&mut h, "require('missing');", &module_loader::LOAD_WARNING);
    t.test_modules(
        &mut h,
        "test.js",
        r#"/**
 * @fileoverview
 * @suppress {moduleLoad}
 */
var foo = require('missing');
"#,
        r#"/**
 * @fileoverview
 * @suppress {moduleLoad}
 */
var foo = module$missing.default;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testExportsUsageInIf
#[test]
fn test_exports_usage_in_if() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"exports.merge = function(source) {
  return Object.keys(source).reduce(function (acc, key) {
    if (Object.prototype.hasOwnProperty.call(acc, key)) {
      acc[key] = exports.merge(acc[key], value, options);
    } else {
      acc[key] = value;
    }
    return acc;
  }, {});
};
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
module$test.default.merge = function(source) {
  return Object.keys(source).reduce(function(acc,key) {
    if (Object.prototype.hasOwnProperty.call(acc,key)) {
      acc[key] = module$test.default.merge(acc[key],value,options);
    } else {
      acc[key] = value;
    }
    return acc;
  }, {});
}
"#,
    );
}

// port: ProcessCommonJSModulesTest#testModuleId
#[test]
fn test_module_id() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "module.exports = module.id;",
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = 'test.js';
"#,
    );
}

// port: ProcessCommonJSModulesTest#testModuleIdAlias
#[test]
fn test_module_id_alias() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"module.exports = 'foo';
function foobar(module) { return module.id; }
"#,
        r#"/** @const */ var module$test = {};
/** @const */ module$test.default = 'foo';
function foobar$$module$test(module) { return module.id; }
"#,
    );
}

// port: ProcessCommonJSModulesTest#testWebpackRequire
#[test]
fn test_webpack_require() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    let webpack_modules_by_id =
        map(&[("1", "other.js"), ("2", "yet_another.js"), ("3", "test.js")]);
    h.set_webpack_modules_by_id(webpack_modules_by_id);
    t.resolution_mode = ResolutionMode::WEBPACK;
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = __webpack_require__(1);
exports.foo = 1;
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var name$$module$test = module$other.default;
module$test.default.foo = 1;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testWebpackRequireString
#[test]
fn test_webpack_require_string() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    let webpack_modules_by_id = map(&[
        ("1", "other.js"),
        ("yet_another.js", "yet_another.js"),
        ("3", "test.js"),
    ]);
    h.set_webpack_modules_by_id(webpack_modules_by_id);
    t.resolution_mode = ResolutionMode::WEBPACK;
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = __webpack_require__('yet_another.js');
exports.foo = 1;
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var name$$module$test = module$yet_another.default;
module$test.default.foo = 1;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testComplexExportAssignment
#[test]
fn test_complex_export_assignment() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "const width = 800; const vwidth = exports.vwidth = width;",
        r#"/** @const */ var module$test = { /** @const */ default: {}};
module$test.default.vwidth = 800;
const vwidth$$module$test = module$test.default.vwidth;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testUMDRequiresIfTest
#[test]
fn test_umd_requires_if_test() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "var foobar = {foo: 'bar'}; if (foobar) { module.exports = foobar; }",
        r#"/** @const */ var module$test = {};
var foobar$$module$test={foo:"bar"};
if(foobar$$module$test) {
  /** @const */ module$test.default = foobar$$module$test;
}
"#,
    );
}

// port: ProcessCommonJSModulesTest#testObjectSpreadExport
#[test]
fn test_object_spread_export() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "var g = {}; module.exports = { ...g };",
        r#"/** @const */ var module$test = {};
var g$$module$test = {};
/** @const */ module$test.default = {
  ...g$$module$test
};
"#,
    );
}

// port: ProcessCommonJSModulesTest#testBabelTranspiledESModules
#[test]
fn test_babel_transpiled_es_modules() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"'use strict';

Object.defineProperty(exports, '__esModule', {
  value: true
});
exports.default = toInteger;
function toInteger(dirtyNumber) { }
module.exports = exports['default'];
"#,
        r#"/** @const */ var module$test = {default: {}};
module$test.default.default = function(dirtyNumber) { };
Object.defineProperty(module$test.default, '__esModule',{value:true})
module$test.default = module$test.default.default;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testLodashModulesCheck
#[test]
fn test_lodash_modules_check() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"/* Detect free variable `exports`. */
const freeExports = typeof exports == 'object' && exports !== null
    && !exports.nodeType && exports
/* Detect free variable `module`. */
const freeModule = freeExports && typeof module == 'object' && module !== null
    && !module.nodeType && module
console.log(freeExports, freeModule);
module.exports = true;
"#,
        r#"/** @const */ var module$test = {default: {}};
const freeExports$$module$test = 'object' == 'object' && module$test.default !== null
    && !module$test.default.nodeType && module$test.default;
const freeModule$$module$test = freeExports$$module$test && 'object' == 'object'
    && {} !== null && !{}.nodeType && {};
console.log(freeExports$$module$test, freeModule$$module$test);
module$test.default = true;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testIssue3051
#[test]
fn test_issue3051() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        r#"class Base {}
exports.Base = Base;

class Impl extends exports.Base {
    getString() {
        return "test";
    }
}
exports.Impl = Impl;

const w = new exports.Impl("a")
console.log(w.getString());
"#,
        r#"/** @const */ var module$test = {
    /** @const */ default: {}
};
module$test.default.Base = class {};
module$test.default.Impl = class extends module$test.default.Base {
    getString() {
        return "test"
    }
};
const w$$module$test = new module$test.default.Impl("a");
console.log(w$$module$test.getString());
"#,
    );
}

// port: ProcessCommonJSModulesTest#testDestructuredImportExported
#[test]
fn test_destructured_import_exported() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "const {Foo} = require('./other.js'); Foo; exports.Foo = Foo;",
        r#"/** @const */ var module$test = {
    /** @const */ default: {}
};
const {Foo: Foo$$module$test} = module$other.default;
module$other.default.Foo;
module$test.default.Foo = module$other.default.Foo;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testDestructuredExports
#[test]
fn test_destructured_exports() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "const {b} = {b: 1}; module.exports = {b: b};",
        r#"/** @const */ var module$test = {
    /** @const */ default: {}
};
const {b: b$$module$test} = {b: 1};
module$test.default.b = b$$module$test;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testWebpackRequireNamespace
#[test]
fn test_webpack_require_namespace() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    let webpack_modules_by_id = map(&[
        ("1", "other.js"),
        ("yet_another.js", "yet_another.js"),
        ("3", "test.js"),
    ]);
    h.set_webpack_modules_by_id(webpack_modules_by_id);
    t.resolution_mode = ResolutionMode::WEBPACK;
    t.test_modules(
        &mut h,
        "test.js",
        r#"var name = __webpack_require__.t('yet_another.js');
exports.foo = 1;
"#,
        r#"/** @const */ var module$test = {/** @const */ default: {}};
var name$$module$test = module$yet_another;
module$test.default.foo = 1;
"#,
    );
}

// port: ProcessCommonJSModulesTest#testGoogModuleUnaffected
#[test]
fn test_goog_module_unaffected() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "goog.module('foo'); exports.y = 123;",
        "goog.module('foo'); exports.y = 123;",
    );
}

// port: ProcessCommonJSModulesTest#testGoogProvideUnaffected
#[test]
fn test_goog_provide_unaffected() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "goog.provide('foo'); foo = 123;",
        "goog.provide('foo'); foo = 123;",
    );
}

// port: ProcessCommonJSModulesTest#testTopModuleCallNotRewritten
#[test]
fn test_top_module_call_not_rewritten() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    t.test_modules(
        &mut h,
        "test.js",
        "module('foo.bar');",
        "module('foo.bar');",
    );
}

// port: ProcessCommonJSModulesTest#testEsModuleExportsNotRewritten
#[test]
fn test_es_module_exports_not_rewritten() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    // String code = "export var foo = 1, bar = 2;";
    t.test_modules(
        &mut h,
        "test.js",
        "export var foo = 1, bar = 2;",
        "export var foo = 1, bar = 2;",
    );
}

// port: ProcessCommonJSModulesTest#testRequireDot
#[test]
fn test_require_dot() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    h.test(
        &mut t,
        vec![
            srcs(&[
                ("mod/index.js", "module.exports = {};"),
                (
                    "mod/sub.js",
                    r#"var name = require('.');
(function() { let foo = name; foo(); })();
"#,
                ),
            ]),
            expected(&[
                (
                    "mod/index.js",
                    "/** @const */ var module$mod$index = {/** @const */ default: {}};",
                ),
                (
                    "mod/sub.js",
                    r#"var name = module$mod$index.default;
(function() { let foo = module$mod$index.default; foo(); })();
"#,
                ),
            ]),
        ],
    )
    .unwrap();
}

// port: ProcessCommonJSModulesTest#testRequireDotDot
#[test]
fn test_require_dot_dot() {
    let (mut h, mut t) = ProcessCommonJSModulesTest::set_up();
    h.test(
        &mut t,
        vec![
            srcs(&[
                ("mod/index.js", "module.exports = {};"),
                (
                    "mod/sub/sub.js",
                    r#"var name = require('..');
(function() { let foo = name; foo(); })();
"#,
                ),
            ]),
            expected(&[
                (
                    "mod/index.js",
                    "/** @const */ var module$mod$index = {/** @const */ default: {}};",
                ),
                (
                    "mod/sub/sub.js",
                    r#"var name = module$mod$index.default;
(function() { let foo = module$mod$index.default; foo(); })();
"#,
                ),
            ]),
        ],
    )
    .unwrap();
}
