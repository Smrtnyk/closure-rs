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
//   test/com/google/javascript/jscomp/lint/CheckProvidesSortedTest.java.

use super::support::LintTestCase;
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    lint::check_provides_sorted::{CheckProvidesSorted, Mode, PROVIDES_NOT_SORTED},
    node_traversal::NodeTraversal,
};
use closure_parsing::config::JsDocParsing;
use closure_rhino::node::NodeId;

struct ProvidesSortedPass(CheckProvidesSorted);

impl CompilerPass for ProvidesSortedPass {
    // port: CheckProvidesSortedTest#getProcessor (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, &mut self.0);
    }
}

// port: CheckProvidesSortedTest#getProcessor
// port: CheckProvidesSortedTest#getOptions
fn case() -> LintTestCase {
    let mut t = LintTestCase::new(|_| {
        Box::new(ProvidesSortedPass(CheckProvidesSorted::new(
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

// port: CheckProvidesSortedTest#testNoWarning
#[test]
fn test_no_warning() {
    let t = case();
    t.test_no_warning(
        r#"/** @fileoverview foo */

goog.provide('a');
goog.provide('b');
goog.provide('c');

alert(1);
"#,
    );
}

// port: CheckProvidesSortedTest#testNoWarning_noProvides
#[test]
fn test_no_warning_no_provides() {
    let t = case();
    t.test_no_warning(
        r#"/** @fileoverview foo */

goog.module('m');

goog.require('x');
"#,
    );
}

// port: CheckProvidesSortedTest#testWarning
#[test]
fn test_warning() {
    let t = case();
    t.test_warning_containing(
        &[r#"/** @fileoverview foo */

goog.provide('b');
goog.provide('a');
goog.provide('c');

alert(1);
"#],
        &PROVIDES_NOT_SORTED,
        r#"The correct order is:

goog.provide('a');
goog.provide('b');
goog.provide('c');
"#,
    );
}
