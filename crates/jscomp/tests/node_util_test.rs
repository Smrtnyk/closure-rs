/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/NodeUtilTest.java.

use closure_jscomp::{
    Compiler,
    compiler_options::CompilerOptions,
    google_coding_convention::GoogleCodingConvention,
    node_util::{NodeUtil, Visitor},
};
use closure_jstype::{JSTypeNative, JSTypeRegistry};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::node::SideEffectFlags;
use closure_rhino::{
    error_reporter::NullErrorReporter,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use num_bigint::BigInt;
use std::sync::Arc;

#[path = "node_util_test/helpers.rs"]
mod helpers;
use helpers::*;
#[path = "node_util_test/manual_tests.rs"]
mod manual_tests;
#[path = "node_util_test/parameter_tests.rs"]
mod parameter_tests;
#[path = "node_util_test/parsed_tests.rs"]
mod parsed_tests;

mod assorted_tests {
    use super::*;

    // port: NodeUtilTest.AssortedTests#testGetSourceName
    #[test]
    fn test_get_source_name() {
        let mut ast = Ast::new();
        let n = ast.new_node(Token::BLOCK);
        let parent = ast.new_node_with_child(Token::BLOCK, n);
        parent.set_source_file_for_testing(&mut ast, "foo");
        assert_eq!(NodeUtil::get_source_name(&ast, n), Some("foo".to_owned()));
    }

    // port: NodeUtilTest.AssortedTests#testMayBeStringWithTypes
    #[test]
    fn test_may_be_string_with_types() {
        let mut ast = Ast::new();
        let registry = JSTypeRegistry::new(&mut ast, Box::new(NullErrorReporter), Vec::new());
        let a = IR::name(&mut ast, "a").set_jstype(
            &mut ast,
            Some(registry.get_native_type(JSTypeNative::NUMBER_TYPE)),
        );
        assert!(!NodeUtil::may_be_string_with_type(
            &ast,
            a,
            true,
            Some(&registry)
        ));
        let a = IR::name(&mut ast, "a").set_jstype(
            &mut ast,
            Some(registry.get_native_type(JSTypeNative::BIGINT_TYPE)),
        );
        assert!(!NodeUtil::may_be_string_with_type(
            &ast,
            a,
            true,
            Some(&registry)
        ));
        let a = IR::name(&mut ast, "a").set_jstype(
            &mut ast,
            Some(registry.get_native_type(JSTypeNative::BOOLEAN_TYPE)),
        );
        assert!(!NodeUtil::may_be_string_with_type(
            &ast,
            a,
            true,
            Some(&registry)
        ));
        let a = IR::name(&mut ast, "a").set_jstype(
            &mut ast,
            Some(registry.get_native_type(JSTypeNative::STRING_TYPE)),
        );
        assert!(NodeUtil::may_be_string_with_type(
            &ast,
            a,
            true,
            Some(&registry)
        ));
    }

    // port: NodeUtilTest.AssortedTests#testValidNames
    #[test]
    fn test_valid_names() {
        assert!(NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("a")
        ));
        assert!(NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("a3")
        ));
        assert!(!NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("3a")
        ));
        assert!(!NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("a.")
        ));
        assert!(!NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from(".a")
        ));
        assert!(!NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("a.b")
        ));
        assert!(!NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("true")
        ));
        assert!(!NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("a.true")
        ));
        assert!(!NodeUtil::is_valid_property_name(
            FeatureSet::ES3,
            &JsString::from("a..b")
        ));

        assert!(NodeUtil::is_valid_simple_name(&JsString::from("a")));
        assert!(NodeUtil::is_valid_simple_name(&JsString::from("a3")));
        assert!(!NodeUtil::is_valid_simple_name(&JsString::from("3a")));
        assert!(!NodeUtil::is_valid_simple_name(&JsString::from("a.")));
        assert!(!NodeUtil::is_valid_simple_name(&JsString::from(".a")));
        assert!(!NodeUtil::is_valid_simple_name(&JsString::from("a.b")));
        assert!(!NodeUtil::is_valid_simple_name(&JsString::from("true")));
        assert!(!NodeUtil::is_valid_simple_name(&JsString::from("a.true")));
        assert!(!NodeUtil::is_valid_simple_name(&JsString::from("a..b")));

        assert!(NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("a")
        ));
        assert!(NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("a3")
        ));
        assert!(!NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("3a")
        ));
        assert!(!NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("a.")
        ));
        assert!(!NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from(".a")
        ));
        assert!(NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("a.b")
        ));
        assert!(!NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("true")
        ));
        assert!(!NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("a.true")
        ));
        assert!(!NodeUtil::is_valid_qualified_name_features(
            FeatureSet::ES3,
            &JsString::from("a..b")
        ));
    }

    // port: NodeUtilTest.AssortedTests#testNewQName1
    #[test]
    fn test_new_qname1() {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        compiler.init(&[], &[], options);
        let actual = NodeUtil::new_qname(&mut compiler, JsString::from("ns.prop"));
        let name = IR::name(&mut compiler, "ns");
        let expected = IR::getprop(&mut compiler, name, "prop");
        assert!(actual.is_equivalent_to(&compiler, expected));
    }

    // port: NodeUtilTest.AssortedTests#testNewQualifiedNameNode2
    #[test]
    fn test_new_qualified_name_node2() {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        compiler.init(&[], &[], options);
        let actual = NodeUtil::new_qname(&mut compiler, JsString::from("this.prop"));
        let this = IR::this_node(&mut compiler);
        let expected = IR::getprop(&mut compiler, this, "prop");
        assert!(actual.is_equivalent_to(&compiler, expected));
    }

    // port: NodeUtilTest.AssortedTests#testNewQNameDeclarationWithQualifiedName
    #[test]
    fn test_new_qname_declaration_with_qualified_name() {
        let mut compiler = Compiler::new();
        let value = IR::number(&mut compiler, 0.0);
        let actual = create_new_qname_declaration(&mut compiler, "ns.prop", value, Token::VAR);
        let name = IR::name(&mut compiler, "ns");
        let getprop = IR::getprop(&mut compiler, name, "prop");
        let zero = IR::number(&mut compiler, 0.0);
        let assign = IR::assign(&mut compiler, getprop, zero);
        let expected = IR::expr_result(&mut compiler, assign);
        assert!(actual.is_equivalent_to(&compiler, expected));
    }

    // port: NodeUtilTest.AssortedTests#testNewQNameDeclarationWithVar
    #[test]
    fn test_new_qname_declaration_with_var() {
        let mut compiler = Compiler::new();
        let value = IR::number(&mut compiler, 0.0);
        let actual = create_new_qname_declaration(&mut compiler, "x", value, Token::VAR);
        let name = IR::name(&mut compiler, "x");
        let zero = IR::number(&mut compiler, 0.0);
        let expected = IR::var_with_value(&mut compiler, name, zero);
        assert!(actual.is_equivalent_to(&compiler, expected));
    }

    // port: NodeUtilTest.AssortedTests#testNewQNameDeclarationWithLet
    #[test]
    fn test_new_qname_declaration_with_let() {
        let mut compiler = Compiler::new();
        let value = IR::number(&mut compiler, 0.0);
        let actual = create_new_qname_declaration(&mut compiler, "x", value, Token::LET);
        let name = IR::name(&mut compiler, "x");
        let zero = IR::number(&mut compiler, 0.0);
        let expected = IR::let_with_value(&mut compiler, name, zero);
        assert!(actual.is_equivalent_to(&compiler, expected));
    }

    // port: NodeUtilTest.AssortedTests#testNewQNameDeclarationWithConst
    #[test]
    fn test_new_qname_declaration_with_const() {
        let mut compiler = Compiler::new();
        let value = IR::number(&mut compiler, 0.0);
        let actual = create_new_qname_declaration(&mut compiler, "x", value, Token::CONST);
        let name = IR::name(&mut compiler, "x");
        let zero = IR::number(&mut compiler, 0.0);
        let expected = IR::const_node(&mut compiler, name, zero);
        assert!(actual.is_equivalent_to(&compiler, expected));
    }

    // port: NodeUtilTest.AssortedTests#createNewQNameDeclaration
    fn create_new_qname_declaration(
        compiler: &mut Compiler,
        name: &str,
        value: NodeId,
        token: Token,
    ) -> NodeId {
        let mut options = CompilerOptions::new();
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        compiler.init(&[], &[], options);
        NodeUtil::new_qname_declaration(compiler, JsString::from(name), Some(value), None, token)
    }

    // port: NodeUtilTest.AssortedTests#testIsBundledGoogModule_onlyIfInScript
    #[test]
    fn test_is_bundled_goog_module_only_if_in_script() {
        let mut ast = Ast::new();
        let goog = IR::name(&mut ast, "goog");
        let load_module = IR::getprop(&mut ast, goog, "loadModule");
        let text = IR::string(&mut ast, "imaginary module text here");
        let call_node = IR::call(&mut ast, load_module, &[text]);
        assert!(!NodeUtil::is_bundled_goog_module_call(&ast, call_node));
        let expr_result = IR::expr_result(&mut ast, call_node);
        assert!(!NodeUtil::is_bundled_goog_module_call(&ast, call_node));
        IR::script_with_children(&mut ast, &[expr_result]);
        assert!(NodeUtil::is_bundled_goog_module_call(&ast, call_node));
    }

    // port: NodeUtilTest.AssortedTests#testGetOriginalName_string
    #[test]
    fn test_get_original_name_string() {
        assert_eq!(NodeUtil::get_original_name(&JsString::from("foo")), "foo");
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("foo$$1")),
            "foo"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("foo$$123")),
            "foo"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("foo_bar$$1")),
            "foo_bar"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("foo$jscomp$1")),
            "foo"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("foo_bar$jscomp$123")),
            "foo_bar"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("module$exports$foo$Bar")),
            "Bar"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("module$exports$foo$bar$Baz")),
            "Baz"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from(
                "module$exports$foo$bar$PREVENT_CSN_PROPERTY"
            )),
            "PREVENT_CSN_PROPERTY"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("module$contents$foo$Bar_baz")),
            "baz"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("module$contents$foo$Bar_baz_qux")),
            "qux"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("module$exports$foo$Bar$$1")),
            "Bar"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("module$contents$foo$Bar_baz$$123")),
            "baz"
        );
        assert_eq!(
            NodeUtil::get_original_name(&JsString::from("module$exports$foo$Bar$jscomp$1$$1")),
            "Bar"
        );
    }
}

mod convert_to_non_opt_chain_tests {
    use super::*;

    // port: NodeUtilTest.ConvertToNonOptChainTests#simpleChain
    #[test]
    fn simple_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let orig_chain = IR::start_opt_chain_getprop(&mut ast, expr, "prop");
        NodeUtil::convert_to_non_optional_chain_segment(&mut ast, orig_chain);
        assert!(is_chain_converted(&ast, Some(orig_chain)));
    }

    // port: NodeUtilTest.ConvertToNonOptChainTests#continuedChain
    #[test]
    fn continued_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let inner_get_prop = IR::start_opt_chain_getprop(&mut ast, expr, "pro1");
        let outter_get_prop = IR::continue_opt_chain_getprop(&mut ast, inner_get_prop, "prop2");
        NodeUtil::convert_to_non_optional_chain_segment(&mut ast, outter_get_prop);
        assert!(is_chain_converted(&ast, Some(outter_get_prop)));
    }

    // port: NodeUtilTest.ConvertToNonOptChainTests#nestedChain
    #[test]
    fn nested_chain() {
        let mut ast = Ast::new();
        let expr1 = IR::name(&mut ast, "expr1");
        let inner_get_prop = IR::start_opt_chain_getprop(&mut ast, expr1, "pro1");
        let expr2 = IR::name(&mut ast, "expr2");
        let get_elem = IR::getelem(&mut ast, expr2, inner_get_prop);
        let outter_get_prop = IR::start_opt_chain_getprop(&mut ast, get_elem, "prop2");
        NodeUtil::convert_to_non_optional_chain_segment(&mut ast, outter_get_prop);
        assert!(is_chain_converted(&ast, Some(outter_get_prop)));
        assert!(
            outter_get_prop
                .get_first_child(&ast)
                .unwrap()
                .get_second_child(&ast)
                .unwrap()
                .is_opt_chain_get_prop(&ast)
        );
    }

    // port: NodeUtilTest.ConvertToNonOptChainTests#convertSegmentDownToStopNode
    #[test]
    fn convert_segment_down_to_stop_node() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let inner_get_prop = IR::start_opt_chain_getprop(&mut ast, expr, "prop1");
        let middle_get_prop = IR::continue_opt_chain_getprop(&mut ast, inner_get_prop, "prop2");
        let outer_get_prop = IR::continue_opt_chain_getprop(&mut ast, middle_get_prop, "prop3");
        NodeUtil::convert_to_non_optional_chain_segment_down_to(
            &mut ast,
            outer_get_prop,
            inner_get_prop,
        );
        assert!(outer_get_prop.is_get_prop(&ast));
        assert!(middle_get_prop.is_get_prop(&ast));
        assert!(inner_get_prop.is_opt_chain_get_prop(&ast));
    }

    // port: NodeUtilTest.ConvertToNonOptChainTests#isChainConverted
    fn is_chain_converted(ast: &Ast, node: Option<NodeId>) -> bool {
        let Some(node) = node else {
            return true;
        };
        !NodeUtil::is_opt_chain_node(ast, node)
            && is_chain_converted(ast, node.get_first_child(ast))
    }
}

mod get_start_of_opt_chain_tests {
    use super::*;

    // port: NodeUtilTest.GetStartOfOptChainTests#isStartOfChain
    #[test]
    fn is_start_of_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let opt_chain_get = IR::start_opt_chain_getprop(&mut ast, expr, "prop");
        assert_eq!(
            NodeUtil::get_start_of_opt_chain_segment(&ast, opt_chain_get),
            opt_chain_get
        );
    }

    // port: NodeUtilTest.GetStartOfOptChainTests#shortChain
    #[test]
    fn short_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let inner_get_prop = IR::start_opt_chain_getprop(&mut ast, expr, "pro1");
        let outter_get_prop = IR::continue_opt_chain_getprop(&mut ast, inner_get_prop, "prop2");
        assert_eq!(
            NodeUtil::get_start_of_opt_chain_segment(&ast, outter_get_prop),
            inner_get_prop
        );
    }

    // port: NodeUtilTest.GetStartOfOptChainTests#mixedChain
    #[test]
    fn mixed_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let call = IR::call(&mut ast, expr, &[]);
        let get_prop = IR::getprop(&mut ast, call, "prop1");
        let opt_chain_get_prop = IR::start_opt_chain_getprop(&mut ast, get_prop, "prop2");
        let opt_chain_call = IR::continue_opt_chain_call(&mut ast, opt_chain_get_prop, &[]);
        let prop3 = IR::name(&mut ast, "prop3");
        let opt_chain_get_elem = IR::continue_opt_chain_getelem(&mut ast, opt_chain_call, prop3);
        assert_eq!(
            NodeUtil::get_start_of_opt_chain_segment(&ast, opt_chain_get_elem),
            opt_chain_get_prop
        );
        assert_eq!(
            NodeUtil::get_start_of_opt_chain_segment(&ast, opt_chain_call),
            opt_chain_get_prop
        );
    }
}

mod get_end_of_opt_chain_tests {
    use super::*;

    // port: NodeUtilTest.GetEndOfOptChainTests#isEndOfChain
    #[test]
    fn is_end_of_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let opt_chain_get = IR::start_opt_chain_getprop(&mut ast, expr, "prop");
        assert_eq!(
            NodeUtil::get_end_of_opt_chain_segment(&ast, opt_chain_get),
            opt_chain_get
        );
    }

    // port: NodeUtilTest.GetEndOfOptChainTests#shortChain
    #[test]
    fn short_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let inner_get_prop = IR::start_opt_chain_getprop(&mut ast, expr, "pro1");
        let outer_get_prop = IR::continue_opt_chain_getprop(&mut ast, inner_get_prop, "prop2");
        assert_eq!(
            NodeUtil::get_end_of_opt_chain_segment(&ast, inner_get_prop),
            outer_get_prop
        );
    }

    // port: NodeUtilTest.GetEndOfOptChainTests#twoChains
    #[test]
    fn two_chains() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let call = IR::call(&mut ast, expr, &[]);
        let start_opt_get_prop = IR::start_opt_chain_getprop(&mut ast, call, "prop1");
        let opt_chain_get_prop =
            IR::continue_opt_chain_getprop(&mut ast, start_opt_get_prop, "prop2");
        let opt_chain_call = IR::continue_opt_chain_call(&mut ast, opt_chain_get_prop, &[]);
        let prop3 = IR::name(&mut ast, "prop3");
        IR::start_opt_chain_getelem(&mut ast, opt_chain_call, prop3);
        assert_eq!(
            NodeUtil::get_end_of_opt_chain_segment(&ast, start_opt_get_prop),
            opt_chain_call
        );
    }

    // port: NodeUtilTest.GetEndOfOptChainTests#breakingOutOfOptChain
    #[test]
    fn breaking_out_of_opt_chain() {
        let mut ast = Ast::new();
        let expr = IR::name(&mut ast, "expr");
        let start_opt_get_prop = IR::start_opt_chain_getprop(&mut ast, expr, "prop1");
        let opt_chain_get_prop =
            IR::continue_opt_chain_getprop(&mut ast, start_opt_get_prop, "prop2");
        IR::getprop(&mut ast, opt_chain_get_prop, "prop3");
        assert_eq!(
            NodeUtil::get_end_of_opt_chain_segment(&ast, start_opt_get_prop),
            opt_chain_get_prop
        );
    }
}

mod node_traversal_tests {
    use super::*;

    #[derive(Default)]
    struct RecordingVisitor {
        nodes: Vec<NodeId>,
    }
    impl Visitor for RecordingVisitor {
        fn visit(&mut self, _ast: &mut Ast, node: NodeId) {
            self.nodes.push(node);
        }
    }

    // port: NodeUtilTest.NodeTraversalTests#testVisitPreOrder
    #[test]
    fn test_visit_pre_order() {
        let mut ast = Ast::new();
        let mut visitor = RecordingVisitor::default();
        let root = build_test_tree(&mut ast);
        NodeUtil::visit_pre_order(&mut ast, root, &mut visitor);
        let node_strings = get_node_strings_from_mock_visitor(&ast, &visitor);
        assert_eq!(node_strings, ["A", "B", "C", "D", "E", "F", "G"]);
    }

    // port: NodeUtilTest.NodeTraversalTests#testVisitPostOrder
    #[test]
    fn test_visit_post_order() {
        let mut ast = Ast::new();
        let mut visitor = RecordingVisitor::default();
        let root = build_test_tree(&mut ast);
        NodeUtil::visit_post_order(&mut ast, root, &mut visitor);
        let node_strings = get_node_strings_from_mock_visitor(&ast, &visitor);
        assert_eq!(node_strings, ["C", "D", "B", "F", "G", "E", "A"]);
    }

    // port: NodeUtilTest.NodeTraversalTests#testIteratePreOrder
    #[test]
    fn test_iterate_pre_order() {
        let mut ast = Ast::new();
        let root = build_test_tree(&mut ast);
        let node_names: Vec<_> = NodeUtil::pre_order_iterable(&ast, root)
            .into_iter()
            .map(|node| node.get_string(&ast))
            .collect();
        assert_eq!(node_names, ["A", "B", "C", "D", "E", "F", "G"]);
    }

    // port: NodeUtilTest.NodeTraversalTests#testIteratePreOrderWithPredicate
    #[test]
    fn test_iterate_pre_order_with_predicate() {
        let mut ast = Ast::new();
        let root = build_test_tree(&mut ast);
        let is_not_e = |ast: &Ast, n: NodeId| n.get_string(ast) != "E";
        let node_names: Vec<_> = NodeUtil::pre_order_iterable_with_predicate(&ast, root, &is_not_e)
            .into_iter()
            .map(|node| node.get_string(&ast))
            .collect();
        assert_eq!(node_names, ["A", "B", "C", "D", "E"]);
    }

    // port: NodeUtilTest.NodeTraversalTests#buildTestTree
    fn build_test_tree(ast: &mut Ast) -> NodeId {
        let a = ast.new_string("A");
        let b = ast.new_string("B");
        let c = ast.new_string("C");
        b.add_child_to_back(ast, c);
        let d = ast.new_string("D");
        b.add_child_to_back(ast, d);
        a.add_child_to_back(ast, b);
        let e = ast.new_string("E");
        let f = ast.new_string("F");
        e.add_child_to_back(ast, f);
        let g = ast.new_string("G");
        e.add_child_to_back(ast, g);
        a.add_child_to_back(ast, e);
        a
    }

    // port: NodeUtilTest.NodeTraversalTests#getNodeStringsFromMockVisitor
    fn get_node_strings_from_mock_visitor(ast: &Ast, visitor: &RecordingVisitor) -> Vec<JsString> {
        visitor
            .nodes
            .iter()
            .map(|node| node.get_string(ast))
            .collect()
    }
}
