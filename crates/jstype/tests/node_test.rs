/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Nick Santos
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/rhino/NodeTest.java.

//! The `NodeTest.java` cases that need a `JSTypeRegistry` (closure-rhino cannot depend on
//! closure-jstype; the other NodeTest cases are in crates/rhino/tests/node_test.rs).

use closure_jstype::{
    JSTypeRegistry, js_type_native::JSTypeNative,
    testing::base_js_type_test_case::SharedTestErrorReporter,
};
use closure_rhino::{
    jscomp_serialization::node_property::NodeProperty,
    node::{Ast, NodeId},
    testing::test_error_reporter::TestErrorReporter,
    token::Token,
};
use std::sync::{Arc, Mutex};

/// `new JSTypeRegistry(testErrorReporter)`, keeping a handle on the reporter.
fn new_registry(ast: &mut Ast) -> (JSTypeRegistry, Arc<Mutex<TestErrorReporter>>) {
    let test_error_reporter = Arc::new(Mutex::new(TestErrorReporter::new()));
    let registry = JSTypeRegistry::new(
        ast,
        Box::new(SharedTestErrorReporter(test_error_reporter.clone())),
        Vec::new(),
    );
    (registry, test_error_reporter)
}

// port: NodeTest#testCheckTreeTypeAwareEqualsSame
#[test]
fn test_check_tree_type_aware_equals_same() {
    let mut ast = Ast::new();
    let (registry, test_error_reporter) = new_registry(&mut ast);
    let node1 = ast.new_string_with_token(Token::NAME, "f");
    node1.set_jstype(
        &mut ast,
        Some(registry.get_native_type(JSTypeNative::NUMBER_TYPE)),
    );
    let node2 = ast.new_string_with_token(Token::NAME, "f");
    node2.set_jstype(
        &mut ast,
        Some(registry.get_native_type(JSTypeNative::NUMBER_TYPE)),
    );
    assert!(node1.is_equivalent_to_typed(&ast, node2));
    test_error_reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

// port: NodeTest#testCheckTreeTypeAwareEqualsDifferent
#[test]
fn test_check_tree_type_aware_equals_different() {
    let mut ast = Ast::new();
    let (registry, test_error_reporter) = new_registry(&mut ast);
    let node1 = ast.new_string_with_token(Token::NAME, "f");
    node1.set_jstype(
        &mut ast,
        Some(registry.get_native_type(JSTypeNative::NUMBER_TYPE)),
    );
    let node2 = ast.new_string_with_token(Token::NAME, "f");
    node2.set_jstype(
        &mut ast,
        Some(registry.get_native_type(JSTypeNative::STRING_TYPE)),
    );
    assert!(!node1.is_equivalent_to_typed(&ast, node2));
    test_error_reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

// port: NodeTest#testCheckTreeTypeAwareEqualsDifferentNull
#[test]
fn test_check_tree_type_aware_equals_different_null() {
    let mut ast = Ast::new();
    let (registry, test_error_reporter) = new_registry(&mut ast);
    let node1 = ast.new_string_with_token(Token::NAME, "f");
    node1.set_jstype(
        &mut ast,
        Some(registry.get_native_type(JSTypeNative::NUMBER_TYPE)),
    );
    let node2 = ast.new_string_with_token(Token::NAME, "f");
    assert!(!node1.is_equivalent_to_typed(&ast, node2));
    test_error_reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

// port: NodeTest#bitsetFromNodeProperties
fn bitset_from_node_properties(props: &[NodeProperty]) -> i64 {
    let mut bitset = 0;
    for prop in props {
        bitset = NodeId::set_node_property_bit(bitset, *prop);
    }
    bitset
}

// port: NodeTest#testSerializeProperties_typeBeforeCast
#[test]
fn test_serialize_properties_type_before_cast() {
    let mut ast = Ast::new();
    let (registry, _test_error_reporter) = new_registry(&mut ast);
    let node = ast.new_string_with_token(Token::NAME, "f");
    node.set_jstype_before_cast(
        &mut ast,
        Some(registry.get_native_type(JSTypeNative::NUMBER_TYPE)),
    );
    let result = node.serialize_properties(&ast);
    // Special case: Rhino node prop TYPE_BEFORE_CAST is converted to NodeProperty.COLOR_FROM_CAST
    assert_eq!(
        result,
        bitset_from_node_properties(&[NodeProperty::COLOR_FROM_CAST])
    );
}
