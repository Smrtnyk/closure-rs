/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/SourceInformationAnnotatorTest.java.

//! Port of SourceInformationAnnotatorTest.java.
use closure_jscomp::{
    Compiler, node_traversal::NodeTraversal,
    source_information_annotator::SourceInformationAnnotator,
};
use closure_rhino::{ir::IR, js_string::JsString, token::Token};

#[test]
// port: SourceInformationAnnotatorTest#testPreserveAnnotatedName
fn test_preserve_annotated_name() {
    let mut compiler = Compiler::new();
    let root = compiler.new_node(Token::SCRIPT);
    let name = compiler.new_string("foo");
    name.set_original_name(&mut compiler, Some(JsString::from("bar")));
    root.add_child_to_back(&mut compiler, name);

    NodeTraversal::traverse(
        &mut compiler,
        root,
        &mut SourceInformationAnnotator::create(),
    );
    assert_eq!(
        name.get_original_name(&compiler),
        Some(JsString::from("bar"))
    );
}

#[test]
// port: SourceInformationAnnotatorTest#testSetOriginalGetpropNames
fn test_set_original_getprop_names() {
    let mut compiler = Compiler::new();
    let root = compiler.new_node(Token::SCRIPT);
    let x = IR::name(&mut compiler, "x");
    let getprop = IR::getprop(&mut compiler, x, "y");
    let expr = IR::expr_result(&mut compiler, getprop);
    root.add_child_to_back(&mut compiler, expr);

    NodeTraversal::traverse(
        &mut compiler,
        root,
        &mut SourceInformationAnnotator::create(),
    );

    assert_eq!(
        getprop.get_original_name(&compiler),
        Some(JsString::from("y"))
    );
}

#[test]
// port: SourceInformationAnnotatorTest#testSetOriginalOptChainGetpropNames
fn test_set_original_opt_chain_getprop_names() {
    let mut compiler = Compiler::new();
    let root = compiler.new_node(Token::SCRIPT);
    let x = IR::name(&mut compiler, "x");
    let getprop = IR::start_opt_chain_getprop(&mut compiler, x, "y");
    let expr = IR::expr_result(&mut compiler, getprop);
    root.add_child_to_back(&mut compiler, expr);

    NodeTraversal::traverse(
        &mut compiler,
        root,
        &mut SourceInformationAnnotator::create(),
    );

    assert_eq!(
        getprop.get_original_name(&compiler),
        Some(JsString::from("y"))
    );
}

#[test]
// port: SourceInformationAnnotatorTest#doesNotSetOriginalStringName
fn does_not_set_original_string_name() {
    let mut compiler = Compiler::new();
    let root = compiler.new_node(Token::SCRIPT);
    let string = IR::string(&mut compiler, "x");
    let expr = IR::expr_result(&mut compiler, string);
    root.add_child_to_back(&mut compiler, expr);

    NodeTraversal::traverse(
        &mut compiler,
        root,
        &mut SourceInformationAnnotator::create(),
    );

    // No need for the original name because strings are almost never mangled by JSCompiler and
    // source information mapping "identifier"s don't care about raw strings.
    assert_eq!(string.get_original_name(&compiler), None);
}
