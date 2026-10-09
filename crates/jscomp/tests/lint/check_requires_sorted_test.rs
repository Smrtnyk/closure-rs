/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   test/com/google/javascript/jscomp/lint/CheckRequiresSortedTest.java.

use super::support::LintTestCase;
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    lint::check_requires_sorted::{CheckRequiresSorted, Mode, REQUIRES_NOT_SORTED},
    node_traversal::NodeTraversal,
};
use closure_parsing::config::JsDocParsing;
use closure_rhino::node::NodeId;

struct RequiresSortedPass(CheckRequiresSorted);

impl CompilerPass for RequiresSortedPass {
    // port: CheckRequiresSortedTest#getProcessor (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, &mut self.0);
    }
}

// port: CheckRequiresSortedTest#getProcessor
// port: CheckRequiresSortedTest#getOptions
fn case() -> LintTestCase {
    let mut t = LintTestCase::new(|_| {
        Box::new(RequiresSortedPass(CheckRequiresSorted::new(
            Mode::COLLECT_AND_REPORT,
        )))
    })
    .with_options(|options| {
        options.set_parse_js_doc_documentation(JsDocParsing::INCLUDE_DESCRIPTIONS_WITH_WHITESPACE);
        options.set_preserve_detailed_source_info(true);
        options.set_pretty_print(true);
        options.set_preserve_type_annotations(true);
        options.set_prefer_single_quotes(true);
        options.set_emit_use_strict(false);
    });
    t.parse_js_doc_documentation = JsDocParsing::INCLUDE_DESCRIPTIONS_WITH_WHITESPACE;
    t
}

// port: CheckRequiresSortedTest#testNoWarning
#[test]
fn test_no_warning() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('x');

const c = goog.require('c');
const {b} = goog.require('b');
goog.require('a');
"#,
    );
}

// port: CheckRequiresSortedTest#testNoWarning_noRequires
#[test]
fn test_no_warning_no_requires() {
    let t = case();
    t.test_no_warning(
        r#"goog.module('x');

alert(1);
"#,
    );
}

// port: CheckRequiresSortedTest#testWarning
#[test]
fn test_warning() {
    let t = case();
    t.test_warning_containing(
        &[r#"goog.module('x');

const b = goog.require('b');
const a = goog.require('a')

alert(1);
"#],
        &REQUIRES_NOT_SORTED,
        r#"The correct order is:

const a = goog.require('a');
const b = goog.require('b');
"#,
    );
}

// port: CheckRequiresSortedTest#testWarning_withJsDoc
#[test]
fn test_warning_with_js_doc() {
    let t = case();
    t.test_warning_containing(
        &[r#"goog.module('x');

/**
 * @suppress {extraRequire}
 */
goog.require('b');
goog.require('a');

alert(1);
"#],
        &REQUIRES_NOT_SORTED,
        r#"The correct order is:

goog.require('a');
/**
 * @suppress {extraRequire}
 */
goog.require('b');
"#,
    );
}

// port: CheckRequiresSortedTest#testWarning_destructuringWithNoShorthandProperties
#[test]
fn test_warning_destructuring_with_no_shorthand_properties() {
    let t = case();
    t.test_warning_containing(
        &[r#"goog.module('x');

const {a: a} = goog.require('a');

alert(1);
"#],
        &REQUIRES_NOT_SORTED,
        r#"The correct order is:

const {a} = goog.require('a');
"#,
    );
}
