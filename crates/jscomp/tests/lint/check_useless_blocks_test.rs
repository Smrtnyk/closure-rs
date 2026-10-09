/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckUselessBlocksTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_useless_blocks::{CheckUselessBlocks, USELESS_BLOCK};

// port: CheckUselessBlocksTest#getProcessor
// port: CheckUselessBlocksTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckUselessBlocks::new(c))).with_lint_checks_warning()
}

// port: CheckUselessBlocksTest#testCheckUselessBlocks_noWarning
#[test]
fn test_check_useless_blocks_no_warning() {
    let t = case();
    t.test_same("while (foo) { bar(); }");
    t.test_same("if (true) { var x = 1; }");
    t.test_same("if (foo) { if (bar) { baz(); } }");
    t.test_same("function bar() { baz(); }");
    t.test_same("function f() { switch (x) { case 1: { return 5; } } }");
    t.test_same("blah: { break blah; }");
    // TODO(moz): For block-scoped function declaration, we should technically
    // warn if we are in non-strict mode and the language mode is ES5 or below.
    t.test_same("{ function foo() {} }");
    t.test_same("let x = 1;");
    t.test_same("{ let x = 1; }");
    t.test_same("if (true) { let x = 1; }");
    t.test_same("{ const y = 1; }");
    t.test_same("{ class Foo {} }");
}

// port: CheckUselessBlocksTest#testCheckUselessBlocks_withES6Modules_noWarning
#[test]
fn test_check_useless_blocks_with_es6_modules_no_warning() {
    let t = case();
    t.test_same("export function f() { switch (x) { case 1: { return 5; } } }");
}

// port: CheckUselessBlocksTest#testCheckUselessBlocks_warning
#[test]
fn test_check_useless_blocks_warning() {
    let t = case();
    t.test_warning("{}", &USELESS_BLOCK);
    t.test_warning("{ var f = function() {}; }", &USELESS_BLOCK);
    t.test_warning("{ var x = 1; }", &USELESS_BLOCK);
    t.test_warning(
        r#"function f() {
  return
    {foo: 'bar'};
}
"#,
        &USELESS_BLOCK,
    );
    t.test_warning(
        r#"if (foo) {
  bar();
  {
    baz();
  }
}
"#,
        &USELESS_BLOCK,
    );
    t.test_warning(
        r#"if (foo) {
  bar();
} {
  baz();
}
"#,
        &USELESS_BLOCK,
    );
    t.test_warning("function bar() { { baz(); } }", &USELESS_BLOCK);
    t.test_warning("{ let x = function() {}; {} }", &USELESS_BLOCK);
    t.test_warning("{ let x = function() { {} }; }", &USELESS_BLOCK);
    t.test_warning("{ var f = class {}; }", &USELESS_BLOCK);
}

// port: CheckUselessBlocksTest#testCheckUselessBlocks_withES6Modules_warning
#[test]
fn test_check_useless_blocks_with_es6_modules_warning() {
    let t = case();
    t.test_warning("export function bar() { { baz(); } }", &USELESS_BLOCK);
}
