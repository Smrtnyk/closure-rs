/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckExtraRequiresTest.java.

use super::support::LintTestCase;
use closure_jscomp::{
    check_level::CheckLevel,
    diagnostic_groups,
    lint::check_extra_requires::{CheckExtraRequires, EXTRA_REQUIRE_WARNING},
};

// port: CheckExtraRequiresTest#CheckExtraRequiresTest
// port: CheckExtraRequiresTest#getOptions
// port: CheckExtraRequiresTest#getProcessor
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckExtraRequires::new(c, None))).with_options(|options| {
        options.set_warning_level(diagnostic_groups::EXTRA_REQUIRE.clone(), CheckLevel::ERROR);
        options.set_warning_level(diagnostic_groups::MODULE_LOAD.clone(), CheckLevel::OFF);
    })
}

// port: CheckExtraRequiresTest#testExtraRequire
#[test]
fn test_extra_require() {
    let t = case();
    t.test_error("goog.require('foo.Bar');", &EXTRA_REQUIRE_WARNING);
}

// port: CheckExtraRequiresTest#testExtraImport
#[test]
fn test_extra_import() {
    let t = case();
    t.test_error("import z from '/x.y';", &EXTRA_REQUIRE_WARNING);
}

// port: CheckExtraRequiresTest#testFailForwardDeclareInModule
#[test]
fn test_fail_forward_declare_in_module() {
    let t = case();
    t.test_error(
        r#"goog.module('example');

var Event = goog.forwardDeclare('goog.events.Event');
var Unused = goog.forwardDeclare('goog.events.Unused');

/**
 * @param {!Event} event
 */
function listener(event) {
  alert(event);
}

exports = listener;
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testShadowedUnusedImport
#[test]
fn test_shadowed_unused_import() {
    let t = case();
    // It would be nice to catch this, but currently the pass is name based and thus misses
    // the fact that the import is unused.
    t.test_same(
        r#"goog.module('example');

var Shadowed = goog.forwardDeclare('foo.Shadowed');

function f(Shadowed) {
  alert(Shadowed);
}
"#,
    );
}

// port: CheckExtraRequiresTest#testFailForwardDeclare
#[test]
fn test_fail_forward_declare() {
    let t = case();
    t.test_error(
        r#"goog.forwardDeclare('goog.events.Event');
goog.forwardDeclare('goog.events.Unused');

/**
 * @param {!goog.events.Event} event
 */
function listener(event) {
  alert(event);
}
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testNoWarning_require
#[test]
fn test_no_warning_require() {
    let t = case();
    t.test_same("goog.require('foo.Bar'); var x = new foo.Bar();");
    t.test_same("goog.require('foo.Bar'); let x = new foo.Bar();");
    t.test_same("goog.require('foo.Bar'); const x = new foo.Bar();");
    t.test_same("goog.require('foo.Bar'); /** @type {foo.Bar} */ var x;");
    t.test_same("goog.require('foo.Bar'); /** @type {Array<foo.Bar>} */ var x;");
    t.test_same("goog.require('foo.Bar'); var x = new foo.Bar.Baz();");
    t.test_same("goog.require('foo.bar'); var x = foo.bar();");
    t.test_same("goog.require('foo.bar'); var x = /** @type {foo.bar} */ (null);");
    t.test_same("goog.require('foo.bar'); function f(/** foo.bar */ x) {}");
    t.test_same("goog.require('foo.bar'); alert(foo.bar.baz);");
    t.test_same("/** @suppress {extraRequire} */ goog.require('foo.bar');");
    t.test_same(
        "goog.require('foo.bar'); goog.scope(function() { var bar = foo.bar; alert(bar); });",
    );
    t.test_same("goog.require('foo'); foo();");
    t.test_same("goog.require('foo'); new foo();");
    t.test_same("/** @suppress {extraRequire} */ var bar = goog.require('foo.bar');");
}

// port: CheckExtraRequiresTest#testNoWarning_requireType
#[test]
fn test_no_warning_require_type() {
    let t = case();
    t.test_same("goog.requireType('foo.Bar'); /** @type {foo.Bar} */ var x;");
    t.test_same("goog.requireType('foo.Bar'); /** @type {Array<foo.Bar>} */ var x;");
    t.test_same("goog.requireType('foo.bar'); function f(/** foo.bar */ x) {}");
    t.test_same("/** @suppress {extraRequire} */ goog.requireType('foo.bar');");
    t.test_same("/** @suppress {extraRequire} */ var bar = goog.requireType('foo.bar');");
}

// port: CheckExtraRequiresTest#testNoWarning_require_externsJsDoc
#[test]
fn test_no_warning_require_externs_js_doc() {
    let mut t = case();
    t.externs = "/** @const */ var ns;".to_string();
    t.test_same("goog.require('ns.Foo'); /** @type {ns.Foo} */ var f;");
}

// port: CheckExtraRequiresTest#testNoWarning_requireType_externsJsDoc
#[test]
fn test_no_warning_require_type_externs_js_doc() {
    let mut t = case();
    t.externs = "/** @const */ var ns;".to_string();
    t.test_same("goog.requireType('ns.Foo'); /** @type {ns.Foo} */ var f;");
}

// port: CheckExtraRequiresTest#testNoWarning_require_externsNew
#[test]
fn test_no_warning_require_externs_new() {
    let mut t = case();
    t.externs = "/** @const */ var ns;".to_string();
    t.test_same("goog.require('ns.Foo'); new ns.Foo();");
}

// port: CheckExtraRequiresTest#testNoWarning_requireType_externsNew
#[test]
fn test_no_warning_require_type_externs_new() {
    let mut t = case();
    t.externs = "/** @const */ var ns;".to_string();
    t.test_same("goog.requireType('ns.Foo'); new ns.Foo();");
}

// port: CheckExtraRequiresTest#testNoWarning_esImport_objlitShorthand
#[test]
fn test_no_warning_es_import_objlit_shorthand() {
    let t = case();
    t.test_same(
        r#"import '/example.module';

import X from '/example.X';
alert({X});
"#,
    );
}

// port: CheckExtraRequiresTest#testNoWarning_require_InnerClassInExtends
#[test]
fn test_no_warning_require_inner_class_in_extends() {
    let t = case();
    t.test_same(
        r#"var goog = {};
goog.require('goog.foo.Bar');

/** @constructor @extends {goog.foo.Bar.Inner} */
function SubClass() {}
"#,
    );
}

// port: CheckExtraRequiresTest#testNoWarning_requireType_InnerClassInExtends
#[test]
fn test_no_warning_require_type_inner_class_in_extends() {
    let t = case();
    t.test_same(
        r#"var goog = {};
goog.requireType('goog.foo.Bar');

/** @constructor @extends {goog.foo.Bar.Inner} */
function SubClass() {}
"#,
    );
}

// port: CheckExtraRequiresTest#testWarning_require
#[test]
fn test_warning_require() {
    let t = case();
    t.test_error("goog.require('foo.bar');", &EXTRA_REQUIRE_WARNING);
    t.test_error(
        r#"goog.require('Bar');
function func( {a} ){}
func( {a: 1} );
"#,
        &EXTRA_REQUIRE_WARNING,
    );
    t.test_error(
        r#"goog.require('Bar');
function func( a = 1 ){}
func(42);
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testWarning_requireType
#[test]
fn test_warning_require_type() {
    let t = case();
    t.test_error("goog.requireType('foo.bar');", &EXTRA_REQUIRE_WARNING);
    t.test_error(
        r#"goog.requireType('Bar');
/** @type {string} */ var x
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testNoWarningMultipleFiles
#[test]
fn test_no_warning_multiple_files() {
    let t = case();
    t.test_same_sources(&[
        "goog.require('Foo'); var foo = new Foo();",
        "goog.require('Bar'); var bar = new Bar();",
    ]);
}

// port: CheckExtraRequiresTest#testPassModule
#[test]
fn test_pass_module() {
    let t = case();
    t.test_same(
        r#"import {Foo} from '/bar';
new Foo();
"#,
    );
    t.test_same(
        r#"import Bar from '/bar';
new Bar();
"#,
    );
    t.test_same(
        r#"import {CoolFeature as Foo} from '/bar';
new Foo();
"#,
    );
    t.test_same(
        r#"import Bar, {CoolFeature as Foo, OtherThing as Baz} from '/bar';
new Foo(); new Bar(); new Baz();
"#,
    );
}

// port: CheckExtraRequiresTest#testFailModule
#[test]
fn test_fail_module() {
    let t = case();
    t.test_error("import {Foo} from '/bar';", &EXTRA_REQUIRE_WARNING);
    t.test_error("import {Foo as Foo} from '/bar';", &EXTRA_REQUIRE_WARNING);
    t.test_error("import {Foo as Bar} from '/bar';", &EXTRA_REQUIRE_WARNING);
    t.test_error(
        r#"import {Foo} from '/bar';
goog.require('example.ExtraRequire');
new Foo;
"#,
        &EXTRA_REQUIRE_WARNING,
    );
    t.test_error(
        r#"import {Foo} from '/bar';
goog.requireType('example.ExtraRequire');
new Foo;
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testPassForwardDeclareInModule
#[test]
fn test_pass_forward_declare_in_module() {
    let t = case();
    t.test_same(
        r#"goog.module('example');

var Event = goog.forwardDeclare('goog.events.Event');

/**
 * @param {!Event} event
 */
function listener(event) {
  alert(event);
}

exports = listener;
"#,
    );
}

// port: CheckExtraRequiresTest#testPassForwardDeclare
#[test]
fn test_pass_forward_declare() {
    let t = case();
    t.test_same(
        r#"goog.forwardDeclare('goog.events.Event');

/**
 * @param {!goog.events.Event} event
 */
function listener(event) {
  alert(event);
}
"#,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleGet
#[test]
fn test_goog_module_get() {
    let t = case();
    t.test_same(
        r#"goog.provide('x.y');
goog.require('foo.bar');

goog.scope(function() {
var bar = goog.module.get('foo.bar');
x.y = function() {};
});
"#,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithAliasedRequire
#[test]
fn test_goog_module_with_aliased_require() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('example');

const asserts = goog.require('goog.asserts');

exports = function() {
  asserts.assert(true);
};
"#,
    );
    t.test_error(
        r#"goog.module('example');

const asserts = goog.require('goog.asserts');

exports = function() {
  goog.asserts.assert(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithDestructuringRequire
#[test]
fn test_goog_module_with_destructuring_require() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('example');

const {assert} = goog.require('goog.asserts');

exports = function() {
  assert(true);
};
"#,
    );
    t.test_error(
        r#"goog.module('example');

const {assert} = goog.require('goog.asserts');

exports = function() {
  goog.asserts.assert(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithDestructuringShortnameRequire
#[test]
fn test_goog_module_with_destructuring_shortname_require() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('example');

const {assert: googAssert} = goog.require('goog.asserts');

exports = function() {
  googAssert(true);
};
"#,
    );
    t.test_error(
        r#"goog.module('example');

var {assert: googAssert} = goog.require('goog.asserts');

exports = function() {
  assert(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
    t.test_error(
        r#"goog.module('example');

const {assert: googAssert} = goog.require('goog.asserts');

exports = function() {
  goog.asserts.assert(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithPartiallyUnusedDestructuringRequire
#[test]
fn test_goog_module_with_partially_unused_destructuring_require() {
    let t = case();
    t.test_error(
        r#"goog.module('example');

const {assert, fail} = goog.require('goog.asserts');

exports = function() {
  assert(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithEmptyDestructuringRequire
#[test]
fn test_goog_module_with_empty_destructuring_require() {
    let t = case();
    t.test_error(
        r#"goog.module('example');

var {} = goog.require('goog.asserts');
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithAliasedRequireType
#[test]
fn test_goog_module_with_aliased_require_type() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('example');

const color = goog.requireType('goog.color');

exports = /** @param {color.Rgb} x */ function(x) { alert(x); };
"#,
    );
    t.test_error(
        r#"goog.module('example');

const color = goog.requireType('goog.color');

exports = /** @param {goog.color.Rgb} x */ function(x) { alert(x); };
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithDestructuringRequireType
#[test]
fn test_goog_module_with_destructuring_require_type() {
    let t = case();
    t.test_error(
        r#"goog.module('example');

const {Rgb} = goog.requireType('goog.color');

exports = /** @param {goog.color.Rgb} x */ function(x) { alert(x); };
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithDestructuringShortnameRequireType
#[test]
fn test_goog_module_with_destructuring_shortname_require_type() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('example');

const {Rgb: googColorRgb} = goog.requireType('goog.color');

exports = /** @param {googColorRgb} x */ function(x) { alert(x); };
"#,
    );
    t.test_error(
        r#"goog.module('example');

const {Rgb: googColorRgb} = goog.requireType('goog.color');

exports = /** @param {Rgb} x */ function(x) { alert(x); };
"#,
        &EXTRA_REQUIRE_WARNING,
    );
    t.test_error(
        r#"goog.module('example');

const {Rgb: googColorRgb} = goog.requireType('goog.color');

exports = /** @param {goog.color.Rgb} x */ function(x) { alert(x); };
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithPartiallyUnusedDestructuringRequireType
#[test]
fn test_goog_module_with_partially_unused_destructuring_require_type() {
    let t = case();
    t.test_error(
        r#"goog.module('example');

const {Rgb, Hsv} = goog.require('goog.color');

exports = /** @param {Rgb} x */ function(x) { alert(x); };
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testGoogModuleWithEmptyDestructuringRequireType
#[test]
fn test_goog_module_with_empty_destructuring_require_type() {
    let t = case();
    t.test_error(
        r#"goog.module('example');

var {} = goog.requireType('goog.color');
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}

// port: CheckExtraRequiresTest#testES6ModuleWithDestructuringRequire
#[test]
fn test_es6_module_with_destructuring_require() {
    let t = case();
    t.test_error(
        r#"import '/example';

import {assert, fail} from '/goog.asserts';

export default function() {
  assert(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
    t.test_error(
        r#"import '/example';

import {assert as assert, fail as fail} from '/goog.asserts';

export default function() {
  assert(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
    t.test_error(
        r#"import '/example';

import {assert as a, fail as f} from '/goog.asserts';

export default function() {
  a(true);
};
"#,
        &EXTRA_REQUIRE_WARNING,
    );
}
