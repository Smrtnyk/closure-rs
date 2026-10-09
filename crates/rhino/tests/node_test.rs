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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/rhino/NodeTest.java.

use closure_rhino::{
    ir::IR,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jscomp_colors::standard_colors as sc,
    jscomp_serialization::node_property::NodeProperty,
    jsdoc_info::JSDocInfo,
    node::{
        Ast, JsDocComparison, NodeId, RecursionMode, SideEffectComparison, SideEffectFlags,
        TypeComparison,
    },
    testing::assert_node,
    token::Token,
};
use num_bigint::BigInt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
// port: NodeTest#testValidatePropertiesForRoot
#[test]
fn test_validate_properties_for_root() {
    let mut ast = Ast::new();
    let v1 = IR::root(&mut ast, &[]);
    let n = v1;
    let v2 = get_messages_from_validate_properties(&ast, n);
    assert!(v2.is_empty());
    n.set_source_file_for_testing(&mut ast, "file.js");
    let v3 = get_messages_from_validate_properties(&ast, n);
    assert_contains_exactly(v3, vec![String::from("ROOT has properties")]);
}

// port: NodeTest#getMessagesFromValidateProperties
fn get_messages_from_validate_properties(ast: &Ast, n: NodeId) -> Vec<String> {
    let mut list_builder = Vec::new();
    n.validate_properties(ast, |message| list_builder.push(message));
    list_builder
}

// port: NodeTest#testSideEffectFlagsSerialization
#[test]
fn test_side_effect_flags_serialization() {
    check_side_effect_flags_round_trip(SideEffectFlags::MUTATES_GLOBAL_STATE);
    check_side_effect_flags_round_trip(SideEffectFlags::MUTATES_THIS);
    check_side_effect_flags_round_trip(SideEffectFlags::MUTATES_ARGUMENTS);
    check_side_effect_flags_round_trip(SideEffectFlags::THROWS);
    let v1 = SideEffectFlags::THROWS | SideEffectFlags::MUTATES_THIS;
    check_side_effect_flags_round_trip(v1);
}

// port: NodeTest#checkSideEffectFlagsRoundTrip
fn check_side_effect_flags_round_trip(test_flags: i32) {
    let mut ast = Ast::new();
    let f = IR::name(&mut ast, "f");
    let original = IR::call(&mut ast, f, &[]);
    original.set_source_file_for_testing(&mut ast, "sourcefile");
    let restored = original.clone_node(&mut ast);
    original.set_side_effect_flags(&mut ast, test_flags);
    let serialized_properties = original.serialize_properties(&ast);
    restored.deserialize_properties(&mut ast, serialized_properties, false);
    assert_eq!(restored.get_side_effect_flags(&ast), test_flags);
}

// port: NodeTest#testValidatePropertiesForIsParenthesized
#[test]
fn test_validate_properties_for_is_parenthesized() {
    let mut ast = Ast::new();
    let v1 = IR::string(&mut ast, "");
    let n = v1;
    n.set_source_file_for_testing(&mut ast, "file.js");
    n.set_is_parenthesized(&mut ast, true);
    n.set_token(&mut ast, Token::STRING_KEY);
    let v2 = get_messages_from_validate_properties(&ast, n);
    assert_contains_exactly(v2, vec![String::from("non-expression is parenthesized")]);
}

// port: NodeTest#testValidatePropertiesForFunctionProperties
#[test]
fn test_validate_properties_for_function_properties() {
    let mut ast = Ast::new();
    let v1 = IR::empty(&mut ast);
    let n = v1;
    n.set_source_file_for_testing(&mut ast, "file.js");
    n.set_token(&mut ast, Token::FUNCTION);
    n.set_is_arrow_function(&mut ast, true);
    n.set_is_async_function(&mut ast, true);
    let v2 = get_messages_from_validate_properties(&ast, n);
    assert!(v2.is_empty());
    n.set_token(&mut ast, Token::EMPTY);
    let v3 = get_messages_from_validate_properties(&ast, n);
    assert_contains_exactly(
        v3,
        vec![
            String::from("invalid ARROW_FN prop"),
            String::from("invalid ASYNC_FN prop"),
        ],
    );
}

// port: NodeTest#testValidatePropertiesForSyntheticProperty
#[test]
fn test_validate_properties_for_synthetic_property() {
    let mut ast = Ast::new();
    let v1 = IR::block(&mut ast);
    let n = v1;
    n.set_source_file_for_testing(&mut ast, "file.js");
    n.set_is_synthetic_block(&mut ast, true);
    let v2 = get_messages_from_validate_properties(&ast, n);
    assert!(v2.is_empty());
    n.set_token(&mut ast, Token::EMPTY);
    let v3 = get_messages_from_validate_properties(&ast, n);
    assert_contains_exactly(v3, vec![String::from("invalid SYNTHETIC prop")]);
}

// port: NodeTest#testValidatePropertiesForOptChain
#[test]
fn test_validate_properties_for_opt_chain() {
    let mut ast = Ast::new();
    let v1 = IR::empty(&mut ast);
    let n = v1;
    n.set_source_file_for_testing(&mut ast, "file.js");
    n.set_token(&mut ast, Token::OPTCHAIN_CALL);
    n.set_is_optional_chain_start(&mut ast, true);
    let v2 = get_messages_from_validate_properties(&ast, n);
    assert!(v2.is_empty());
    n.set_token(&mut ast, Token::OPTCHAIN_GETELEM);
    let v3 = get_messages_from_validate_properties(&ast, n);
    assert!(v3.is_empty());
    n.set_token(&mut ast, Token::OPTCHAIN_GETELEM);
    let v4 = get_messages_from_validate_properties(&ast, n);
    assert!(v4.is_empty());
    n.set_token(&mut ast, Token::EMPTY);
    let v5 = get_messages_from_validate_properties(&ast, n);
    assert_contains_exactly(
        v5,
        vec![String::from("START_OF_OPT_CHAIN on non-optional Node")],
    );
}

// port: NodeTest#testValidatePropertiesForConstVarFlags
#[test]
fn test_validate_properties_for_const_var_flags() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let n = v1;
    n.set_source_file_for_testing(&mut ast, "file.js");
    n.set_declared_constant_var(&mut ast, true);
    n.set_inferred_constant_var(&mut ast, true);
    let v2 = get_messages_from_validate_properties(&ast, n);
    assert!(v2.is_empty());
    n.set_token(&mut ast, Token::IMPORT_STAR);
    let v3 = get_messages_from_validate_properties(&ast, n);
    assert!(v3.is_empty());
    n.set_token(&mut ast, Token::STRINGLIT);
    let v4 = get_messages_from_validate_properties(&ast, n);
    assert_contains_exactly(v4, vec![String::from("invalid CONST_VAR_FLAGS")]);
}

// port: NodeTest#testLinenoCharnoNormal
#[test]
fn test_lineno_charno_normal() {
    assert_lineno_charno(5, 6, 5, 6);
    assert_lineno_charno(456, 3423, 456, 3423);
    assert_lineno_charno(0, 0, 0, 0);
}

// port: NodeTest#testLinenoCharnoErroneous
#[test]
fn test_lineno_charno_erroneous() {
    assert_lineno_charno(-5, 90, -1, -1);
    assert_lineno_charno(90, -1, -1, -1);
}

// port: NodeTest#testMergeOverflowGraciously
#[test]
fn test_merge_overflow_graciously() {
    assert_lineno_charno(89, 4096, 89, 4095);
}

// port: NodeTest#assertLinenoCharno
fn assert_lineno_charno(lineno_in: i32, charno_in: i32, lineno_out: i32, charno_out: i32) {
    let mut ast = Ast::new();
    let test = IR::block(&mut ast);
    test.set_lineno_charno(&mut ast, lineno_in, charno_in);
    assert_eq!(test.get_lineno(&ast), lineno_out);
    assert_eq!(test.get_charno(&ast), charno_out);
}

// port: NodeTest#isEquivalentToConsidersStartOfOptionalChainProperty
#[test]
fn is_equivalent_to_considers_start_of_optional_chain_property() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = IR::start_opt_chain_getprop(&mut ast, v1, "b");
    let v3 = IR::continue_opt_chain_getprop(&mut ast, v2, "c");
    let single_segment_opt_chain = v3;
    let v4 = assert_node(single_segment_opt_chain);
    let v5 = single_segment_opt_chain.clone_tree(&mut ast);
    v4.is_equivalent_to(&ast, v5);
    let v7 = single_segment_opt_chain.clone_tree(&mut ast);
    let two_segment_opt_chain = v7;
    two_segment_opt_chain.set_is_optional_chain_start(&mut ast, true);
    let v8 = assert_node(single_segment_opt_chain);
    v8.is_not_equivalent_to(&ast, two_segment_opt_chain);
}

// port: NodeTest#isEquivalentToConsidersDirectEval
#[test]
fn is_equivalent_to_considers_direct_eval() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "eval");
    let v2 = IR::call(&mut ast, v1, &[]);
    let direct_eval_call = v2;
    let v3 = direct_eval_call.get_first_child(&ast);
    v3.unwrap()
        .put_boolean_prop(&mut ast, NodeId::DIRECT_EVAL, true);
    let v4 = IR::name(&mut ast, "eval");
    let v5 = IR::call(&mut ast, v4, &[]);
    let indirect_eval_call = v5;
    let v6 = indirect_eval_call.get_first_child(&ast);
    v6.unwrap()
        .put_boolean_prop(&mut ast, NodeId::DIRECT_EVAL, false);
    let v7 = assert_node(direct_eval_call);
    v7.is_not_equivalent_to(&ast, indirect_eval_call);
}

// port: NodeTest#isEquivalentToForFunctionsConsidersKindOfFunction
#[test]
fn is_equivalent_to_for_functions_considers_kind_of_function() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "");
    let v2 = IR::param_list(&mut ast, &[]);
    let v3 = IR::block(&mut ast);
    let v4 = IR::function(&mut ast, v1, v2, v3);
    let normal_function = v4;
    let v5 = assert_node(normal_function);
    let v6 = normal_function.clone_tree(&mut ast);
    v5.is_equivalent_to(&ast, v6);
    let v8 = normal_function.clone_tree(&mut ast);
    let async_function = v8;
    async_function.set_is_async_function(&mut ast, true);
    let v9 = assert_node(async_function);
    let v10 = async_function.clone_tree(&mut ast);
    let v11 = v9.is_equivalent_to(&ast, v10);
    v11.is_not_equivalent_to(&ast, normal_function);
    let v13 = normal_function.clone_tree(&mut ast);
    let arrow_function = v13;
    arrow_function.set_is_arrow_function(&mut ast, true);
    let v14 = assert_node(arrow_function);
    let v15 = arrow_function.clone_tree(&mut ast);
    let v16 = v14.is_equivalent_to(&ast, v15);
    v16.is_not_equivalent_to(&ast, normal_function);
    let v18 = arrow_function.clone_tree(&mut ast);
    let async_arrow_function = v18;
    async_arrow_function.set_is_async_function(&mut ast, true);
    let v19 = assert_node(async_arrow_function);
    let v20 = async_arrow_function.clone_tree(&mut ast);
    let v21 = v19.is_equivalent_to(&ast, v20);
    let v22 = v21.is_not_equivalent_to(&ast, arrow_function);
    v22.is_not_equivalent_to(&ast, async_function);
    let v24 = normal_function.clone_tree(&mut ast);
    let generator_function = v24;
    generator_function.set_is_generator_function(&mut ast, true);
    let v25 = assert_node(generator_function);
    let v26 = generator_function.clone_tree(&mut ast);
    let v27 = v25.is_equivalent_to(&ast, v26);
    v27.is_not_equivalent_to(&ast, normal_function);
    let v29 = generator_function.clone_tree(&mut ast);
    let async_generator_function = v29;
    async_generator_function.set_is_async_function(&mut ast, true);
    let v30 = assert_node(async_generator_function);
    let v31 = async_generator_function.clone_tree(&mut ast);
    let v32 = v30.is_equivalent_to(&ast, v31);
    let v33 = v32.is_not_equivalent_to(&ast, async_function);
    v33.is_not_equivalent_to(&ast, generator_function);
}

// port: NodeTest#testIsEquivalentTo_withBoolean_isSame
#[test]
fn test_is_equivalent_to_with_boolean_is_same() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::LET);
    let node1 = v1;
    let v2 = node1.is_equivalent_to(&ast, node1);
    assert!(v2);
}

// port: NodeTest#testIsEquivalentTo_withBoolean_isDifferent
#[test]
fn test_is_equivalent_to_with_boolean_is_different() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::LET);
    let node1 = v1;
    let v2 = ast.new_node(Token::VAR);
    let node2 = v2;
    let v3 = node1.is_equivalent_to(&ast, node2);
    assert!(!v3);
}

// port: NodeTest#testIsEquivalentTo_considersDifferentEsModuleExports
#[test]
fn test_is_equivalent_to_considers_different_es_module_exports() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::EXPORT);
    let export_all_from = v1;
    export_all_from.put_boolean_prop(&mut ast, NodeId::EXPORT_ALL_FROM, true);
    let v2 = ast.new_node(Token::EXPORT);
    let export_default = v2;
    export_default.put_boolean_prop(&mut ast, NodeId::EXPORT_DEFAULT, true);
    let v3 = ast.new_node(Token::EXPORT);
    let simple_export = v3;
    let v4 = export_all_from.is_equivalent_to(&ast, export_default);
    assert!(!v4);
    let v5 = export_all_from.is_equivalent_to(&ast, simple_export);
    assert!(!v5);
    let v6 = export_default.is_equivalent_to(&ast, simple_export);
    assert!(!v6);
}

// port: NodeTest#testIsEquivalentToNumber
#[test]
fn test_is_equivalent_to_number() {
    let mut ast = Ast::new();
    let v1 = ast.new_number(1.0);
    let v2 = ast.new_number(1.0);
    let v3 = v1.is_equivalent_to(&ast, v2);
    assert!(v3);
    let v4 = ast.new_number(1.0);
    let v5 = ast.new_number(2.0);
    let v6 = v4.is_equivalent_to(&ast, v5);
    assert!(!v6);
}

// port: NodeTest#isEquivalentToConsidersPresenceOfClosureUnawareShadow
#[test]
fn is_equivalent_to_considers_presence_of_closure_unaware_shadow() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "x");
    let name_shadow1 = v1;
    let v2 = IR::name(&mut ast, "x");
    let name_shadow2 = v2;
    let v3 = IR::name(&mut ast, "x");
    let name_no_shadow = v3;
    let v4 = IR::root(&mut ast, &[]);
    name_shadow1.set_closure_unaware_shadow(&mut ast, Some(v4));
    let v5 = IR::root(&mut ast, &[]);
    name_shadow2.set_closure_unaware_shadow(&mut ast, Some(v5));
    let v6 = name_shadow1.is_equivalent_to(&ast, name_shadow2);
    assert!(v6);
    let v7 = name_shadow1.is_equivalent_to(&ast, name_no_shadow);
    assert!(!v7);
    let v8 = name_no_shadow.is_equivalent_to(&ast, name_shadow1);
    assert!(!v8);
}

// port: NodeTest#isEquivalentToConsidersPresenceOfClosureUnawareShadow_recursively
#[test]
fn is_equivalent_to_considers_presence_of_closure_unaware_shadow_recursively() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "x");
    let name_shadow1a = v1;
    let v2 = IR::name(&mut ast, "x");
    let name_shadow1b = v2;
    let v3 = IR::name(&mut ast, "x");
    let name_shadow2 = v3;
    let v4 = IR::block(&mut ast);
    let v5 = IR::script_with_children(&mut ast, &[v4]);
    let v6 = IR::root(&mut ast, &[v5]);
    let root1_shared = v6;
    let v7 = IR::root(&mut ast, &[]);
    let root2 = v7;
    name_shadow1a.set_closure_unaware_shadow(&mut ast, Some(root1_shared));
    let v8 = root1_shared.clone_tree(&mut ast);
    name_shadow1b.set_closure_unaware_shadow(&mut ast, Some(v8));
    name_shadow2.set_closure_unaware_shadow(&mut ast, Some(root2));
    let v9 = name_shadow1a.is_equivalent_to(&ast, name_shadow1b);
    assert!(v9);
    let v10 = name_shadow1a.is_equivalent_to(&ast, name_shadow2);
    assert!(!v10);
}

// port: NodeTest#testIsEquivalentTo_doesNotConsiderPresenceOfClosureUnawareShadow_inDeepNoShadowMode
#[test]
fn test_is_equivalent_to_does_not_consider_presence_of_closure_unaware_shadow_in_deep_no_shadow_mode()
 {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "x");
    let name_shadow1 = v1;
    let v2 = IR::name(&mut ast, "x");
    let name_shadow2 = v2;
    let v3 = IR::block(&mut ast);
    let v4 = IR::script_with_children(&mut ast, &[v3]);
    let v5 = IR::root(&mut ast, &[v4]);
    let root1 = v5;
    let v6 = IR::root(&mut ast, &[]);
    let root2 = v6;
    name_shadow1.set_closure_unaware_shadow(&mut ast, Some(root1));
    name_shadow2.set_closure_unaware_shadow(&mut ast, Some(root2));
    let v7 = name_shadow1.is_equivalent_to_with_options(
        &ast,
        name_shadow2,
        RecursionMode::DEEP_NO_SHADOW,
        TypeComparison::IGNORE,
        JsDocComparison::IGNORE,
        SideEffectComparison::COMPARE,
    );
    assert!(v7);
}

// port: NodeTest#testNumberRejects_isNaN
#[test]
fn test_number_rejects_is_nan() {
    assert_number_node_rejects(f64::NAN);
    assert_number_node_rejects(-f64::NAN);
}

// port: NodeTest#testNumberRejects_negativeValues
#[test]
fn test_number_rejects_negative_values() {
    assert_number_node_rejects(-1394793.114);
    assert_number_node_rejects(-1.0);
    assert_number_node_rejects(-0.0);
    assert_number_node_rejects(f64::NEG_INFINITY);
}

// port: NodeTest#assertNumberNodeRejects
fn assert_number_node_rejects(d: f64) {
    assert!(catch_unwind(|| Ast::new().new_number(d)).is_err());
    assert!(
        catch_unwind(|| {
            let mut ast = Ast::new();
            let number = ast.new_number(0.0);
            number.set_double(&mut ast, d);
        })
        .is_err()
    );
}

// port: NodeTest#testBigintRejects_negativeValues
#[test]
fn test_bigint_rejects_negative_values() {
    let mut ast = Ast::new();
    let v1 = "-1394793".parse::<BigInt>().unwrap();
    assert_big_int_node_rejects(v1);
    let v2 = "-1".parse::<BigInt>().unwrap();
    assert_big_int_node_rejects(v2);
    let v3 = "-0".parse::<BigInt>().unwrap();
    let v4 = ast.new_big_int(v3);
    assert!(Some(v4).is_some());
}

// port: NodeTest#assertBigIntNodeRejects
fn assert_big_int_node_rejects(x: BigInt) {
    assert!(catch_unwind(AssertUnwindSafe(|| Ast::new().new_big_int(x.clone()))).is_err());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut ast = Ast::new();
            let number = ast.new_big_int(BigInt::from(0));
            number.set_big_int(&mut ast, x);
        }))
        .is_err()
    );
}

// port: NodeTest#testIsEquivalentToBigInt
#[test]
fn test_is_equivalent_to_big_int() {
    let mut ast = Ast::new();
    let v1 = ast.new_big_int(BigInt::from(1));
    let v2 = ast.new_big_int(BigInt::from(1));
    let v3 = v1.is_equivalent_to(&ast, v2);
    assert!(v3);
    let v4 = ast.new_big_int(BigInt::from(1));
    let v5 = ast.new_big_int(BigInt::from(10));
    let v6 = v4.is_equivalent_to(&ast, v5);
    assert!(!v6);
}

// port: NodeTest#testIsEquivalentToString
#[test]
fn test_is_equivalent_to_string() {
    let mut ast = Ast::new();
    let v1 = ast.new_string("1");
    let v2 = ast.new_string("1");
    let v3 = v1.is_equivalent_to(&ast, v2);
    assert!(v3);
    let v4 = ast.new_string("1");
    let v5 = ast.new_string("2");
    let v6 = v4.is_equivalent_to(&ast, v5);
    assert!(!v6);
}

// port: NodeTest#testCheckTreeTypeAwareEqualsSameNull
#[test]
fn test_check_tree_type_aware_equals_same_null() {
    let mut ast = Ast::new();
    let v1 = ast.new_string_with_token(Token::NAME, "f");
    let node1 = v1;
    let v2 = ast.new_string_with_token(Token::NAME, "f");
    let node2 = v2;
    let v3 = node1.is_equivalent_to_typed(&ast, node2);
    assert!(v3);
}

// port: NodeTest#testCheckTreeTypeAwareEqualsColorsSameNull
#[test]
fn test_check_tree_type_aware_equals_colors_same_null() {
    let mut ast = Ast::new();
    let v1 = ast.new_string_with_token(Token::NAME, "f");
    let node1 = v1;
    let v2 = ast.new_string_with_token(Token::NAME, "f");
    let node2 = v2;
    let v3 = node1.is_equivalent_to_typed(&ast, node2);
    assert!(v3);
}

// port: NodeTest#testIsQualifiedName
#[test]
fn test_is_qualified_name() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = v1.is_qualified_name(&ast);
    assert!(v2);
    let v3 = IR::name(&mut ast, "$");
    let v4 = v3.is_qualified_name(&ast);
    assert!(v4);
    let v5 = IR::name(&mut ast, "_");
    let v6 = v5.is_qualified_name(&ast);
    assert!(v6);
    let v7 = IR::name(&mut ast, "a");
    let v8 = IR::getprop(&mut ast, v7, "b");
    let v9 = v8.is_qualified_name(&ast);
    assert!(v9);
    let v10 = IR::this_node(&mut ast);
    let v11 = IR::getprop(&mut ast, v10, "b");
    let v12 = v11.is_qualified_name(&ast);
    assert!(v12);
    let v13 = IR::number(&mut ast, 0.0);
    let v14 = v13.is_qualified_name(&ast);
    assert!(!v14);
    let v15 = IR::arraylit(&mut ast, &[]);
    let v16 = v15.is_qualified_name(&ast);
    assert!(!v16);
    let v17 = IR::objectlit(&mut ast, &[]);
    let v18 = v17.is_qualified_name(&ast);
    assert!(!v18);
    let v19 = IR::string(&mut ast, "");
    let v20 = v19.is_qualified_name(&ast);
    assert!(!v20);
    let v21 = IR::name(&mut ast, "a");
    let v22 = IR::string(&mut ast, "b");
    let v23 = IR::getelem(&mut ast, v21, v22);
    let v24 = v23.is_qualified_name(&ast);
    assert!(!v24);
    let v25 = IR::name(&mut ast, "a");
    let v26 = IR::string(&mut ast, "b");
    let v27 = IR::getelem(&mut ast, v25, v26);
    let v28 = IR::getprop(&mut ast, v27, "c");
    let v29 = v28.is_qualified_name(&ast);
    assert!(!v29);
    let v30 = IR::name(&mut ast, "a");
    let v31 = IR::getprop(&mut ast, v30, "b");
    let v32 = IR::string(&mut ast, "c");
    let v33 = IR::getelem(&mut ast, v31, v32);
    let v34 = v33.is_qualified_name(&ast);
    assert!(!v34);
    let v35 = IR::name(&mut ast, "a");
    let v36 = IR::call(&mut ast, v35, &[]);
    let v37 = v36.is_qualified_name(&ast);
    assert!(!v37);
    let v38 = IR::name(&mut ast, "a");
    let v39 = IR::call(&mut ast, v38, &[]);
    let v40 = IR::getprop(&mut ast, v39, "b");
    let v41 = v40.is_qualified_name(&ast);
    assert!(!v41);
    let v42 = IR::name(&mut ast, "a");
    let v43 = IR::getprop(&mut ast, v42, "b");
    let v44 = IR::call(&mut ast, v43, &[]);
    let v45 = v44.is_qualified_name(&ast);
    assert!(!v45);
    let v46 = IR::string(&mut ast, "a");
    let v47 = v46.is_qualified_name(&ast);
    assert!(!v47);
    let v48 = IR::string(&mut ast, "x");
    let v49 = IR::regexp(&mut ast, v48);
    let v50 = v49.is_qualified_name(&ast);
    assert!(!v50);
    let v51 = IR::name(&mut ast, "x");
    let v52 = ast.new_node_with_child(Token::INC, v51);
    let v53 = v52.is_qualified_name(&ast);
    assert!(!v53);
}

// port: NodeTest#testMatchesQualifiedName1
#[test]
fn test_matches_qualified_name1() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = v1.matches_qualified_name(&ast, "a");
    assert!(v2);
    let v3 = IR::name(&mut ast, "a");
    let v4 = v3.matches_qualified_name(&ast, "ab");
    assert!(!v4);
    let v5 = IR::name(&mut ast, "a");
    let v6 = v5.matches_qualified_name(&ast, "a.b");
    assert!(!v6);
    let v7 = IR::name(&mut ast, "a");
    let v8 = v7.matches_qualified_name(&ast, ".b");
    assert!(!v8);
    let v9 = IR::name(&mut ast, "a");
    let v10 = v9.matches_qualified_name(&ast, "a.");
    assert!(!v10);
    let v11 = qname(&mut ast, "a.b");
    let v12 = v11.matches_qualified_name(&ast, "a");
    assert!(!v12);
    let v13 = qname(&mut ast, "a.b");
    let v14 = v13.matches_qualified_name(&ast, "a.b");
    assert!(v14);
    let v15 = qname(&mut ast, "a.b");
    let v16 = v15.matches_qualified_name(&ast, "a.bc");
    assert!(!v16);
    let v17 = qname(&mut ast, "a.b");
    let v18 = v17.matches_qualified_name(&ast, ".b");
    assert!(!v18);
    let v19 = qname(&mut ast, "a.b");
    let v20 = v19.matches_qualified_name(&ast, "this.b");
    assert!(!v20);
    let v21 = qname(&mut ast, "this");
    let v22 = v21.matches_qualified_name(&ast, "this");
    assert!(v22);
    let v23 = qname(&mut ast, "this");
    let v24 = v23.matches_qualified_name(&ast, "thisx");
    assert!(!v24);
    let v25 = qname(&mut ast, "this.b");
    let v26 = v25.matches_qualified_name(&ast, "a");
    assert!(!v26);
    let v27 = qname(&mut ast, "this.b");
    let v28 = v27.matches_qualified_name(&ast, "a.b");
    assert!(!v28);
    let v29 = qname(&mut ast, "this.b");
    let v30 = v29.matches_qualified_name(&ast, ".b");
    assert!(!v30);
    let v31 = qname(&mut ast, "this.b");
    let v32 = v31.matches_qualified_name(&ast, "a.");
    assert!(!v32);
    let v33 = qname(&mut ast, "this.b");
    let v34 = v33.matches_qualified_name(&ast, "super.b");
    assert!(!v34);
    let v35 = qname(&mut ast, "this.b");
    let v36 = v35.matches_qualified_name(&ast, "this.b");
    assert!(v36);
    let v37 = qname(&mut ast, "super");
    let v38 = v37.matches_qualified_name(&ast, "super");
    assert!(v38);
    let v39 = qname(&mut ast, "super");
    let v40 = v39.matches_qualified_name(&ast, "superx");
    assert!(!v40);
    let v41 = qname(&mut ast, "super.b");
    let v42 = v41.matches_qualified_name(&ast, "a");
    assert!(!v42);
    let v43 = qname(&mut ast, "super.b");
    let v44 = v43.matches_qualified_name(&ast, "a.b");
    assert!(!v44);
    let v45 = qname(&mut ast, "super.b");
    let v46 = v45.matches_qualified_name(&ast, ".b");
    assert!(!v46);
    let v47 = qname(&mut ast, "super.b");
    let v48 = v47.matches_qualified_name(&ast, "a.");
    assert!(!v48);
    let v49 = qname(&mut ast, "super.b");
    let v50 = v49.matches_qualified_name(&ast, "this.b");
    assert!(!v50);
    let v51 = qname(&mut ast, "super.b");
    let v52 = v51.matches_qualified_name(&ast, "super.b");
    assert!(v52);
    let v53 = qname(&mut ast, "a.b.c");
    let v54 = v53.matches_qualified_name(&ast, "a.b.c");
    assert!(v54);
    let v55 = qname(&mut ast, "a.b.c");
    let v56 = v55.matches_qualified_name(&ast, "a.b.c");
    assert!(v56);
    let v57 = IR::import_star(&mut ast, "a");
    let v58 = v57.matches_qualified_name(&ast, "a");
    assert!(v58);
    let v59 = IR::import_star(&mut ast, "a");
    let v60 = v59.matches_qualified_name(&ast, "b");
    assert!(!v60);
    let v61 = IR::number(&mut ast, 0.0);
    let v62 = v61.matches_qualified_name(&ast, "a.b");
    assert!(!v62);
    let v63 = IR::arraylit(&mut ast, &[]);
    let v64 = v63.matches_qualified_name(&ast, "a.b");
    assert!(!v64);
    let v65 = IR::objectlit(&mut ast, &[]);
    let v66 = v65.matches_qualified_name(&ast, "a.b");
    assert!(!v66);
    let v67 = IR::string(&mut ast, "");
    let v68 = v67.matches_qualified_name(&ast, "a.b");
    assert!(!v68);
    let v69 = IR::name(&mut ast, "a");
    let v70 = IR::string(&mut ast, "b");
    let v71 = IR::getelem(&mut ast, v69, v70);
    let v72 = v71.matches_qualified_name(&ast, "a.b");
    assert!(!v72);
    let v73 = IR::name(&mut ast, "a");
    let v74 = IR::string(&mut ast, "b");
    let v75 = IR::getelem(&mut ast, v73, v74);
    let v76 = IR::getprop(&mut ast, v75, "c");
    let v77 = v76.matches_qualified_name(&ast, "a.b.c");
    assert!(!v77);
    let v78 = IR::name(&mut ast, "a");
    let v79 = IR::getprop(&mut ast, v78, "b");
    let v80 = IR::string(&mut ast, "c");
    let v81 = IR::getelem(&mut ast, v79, v80);
    let v82 = v81.matches_qualified_name(&ast, "a.b.c");
    assert!(!v82);
    let v83 = IR::name(&mut ast, "a");
    let v84 = IR::call(&mut ast, v83, &[]);
    let v85 = v84.matches_qualified_name(&ast, "a");
    assert!(!v85);
    let v86 = IR::name(&mut ast, "a");
    let v87 = IR::call(&mut ast, v86, &[]);
    let v88 = IR::getprop(&mut ast, v87, "b");
    let v89 = v88.matches_qualified_name(&ast, "a.b");
    assert!(!v89);
    let v90 = IR::name(&mut ast, "a");
    let v91 = IR::getprop(&mut ast, v90, "b");
    let v92 = IR::call(&mut ast, v91, &[]);
    let v93 = v92.matches_qualified_name(&ast, "a.b");
    assert!(!v93);
    let v94 = IR::string(&mut ast, "a");
    let v95 = v94.matches_qualified_name(&ast, "a");
    assert!(!v95);
    let v96 = IR::string(&mut ast, "x");
    let v97 = IR::regexp(&mut ast, v96);
    let v98 = v97.matches_qualified_name(&ast, "x");
    assert!(!v98);
    let v99 = IR::name(&mut ast, "x");
    let v100 = ast.new_node_with_child(Token::INC, v99);
    let v101 = v100.matches_qualified_name(&ast, "x");
    assert!(!v101);
}

// port: NodeTest#testMatchesQualifiedName2
#[test]
fn test_matches_qualified_name2() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = qname(&mut ast, "a");
    let v3 = v1.matches_qualified_name_node(&ast, v2);
    assert!(v3);
    let v4 = IR::name(&mut ast, "a");
    let v5 = qname(&mut ast, "a.b");
    let v6 = v4.matches_qualified_name_node(&ast, v5);
    assert!(!v6);
    let v7 = qname(&mut ast, "a.b");
    let v8 = qname(&mut ast, "a");
    let v9 = v7.matches_qualified_name_node(&ast, v8);
    assert!(!v9);
    let v10 = qname(&mut ast, "a.b");
    let v11 = qname(&mut ast, "a.b");
    let v12 = v10.matches_qualified_name_node(&ast, v11);
    assert!(v12);
    let v13 = qname(&mut ast, "a.b");
    let v14 = qname(&mut ast, ".b");
    let v15 = v13.matches_qualified_name_node(&ast, v14);
    assert!(!v15);
    let v16 = qname(&mut ast, "a.b");
    let v17 = qname(&mut ast, "this.b");
    let v18 = v16.matches_qualified_name_node(&ast, v17);
    assert!(!v18);
    let v19 = qname(&mut ast, "this.b");
    let v20 = qname(&mut ast, "a");
    let v21 = v19.matches_qualified_name_node(&ast, v20);
    assert!(!v21);
    let v22 = qname(&mut ast, "this.b");
    let v23 = qname(&mut ast, "a.b");
    let v24 = v22.matches_qualified_name_node(&ast, v23);
    assert!(!v24);
    let v25 = qname(&mut ast, "this.b");
    let v26 = qname(&mut ast, "super.b");
    let v27 = v25.matches_qualified_name_node(&ast, v26);
    assert!(!v27);
    let v28 = qname(&mut ast, "this.b");
    let v29 = qname(&mut ast, "this.b");
    let v30 = v28.matches_qualified_name_node(&ast, v29);
    assert!(v30);
    let v31 = qname(&mut ast, "super.b");
    let v32 = qname(&mut ast, "a");
    let v33 = v31.matches_qualified_name_node(&ast, v32);
    assert!(!v33);
    let v34 = qname(&mut ast, "super.b");
    let v35 = qname(&mut ast, "a.b");
    let v36 = v34.matches_qualified_name_node(&ast, v35);
    assert!(!v36);
    let v37 = qname(&mut ast, "super.b");
    let v38 = qname(&mut ast, "this.b");
    let v39 = v37.matches_qualified_name_node(&ast, v38);
    assert!(!v39);
    let v40 = qname(&mut ast, "super.b");
    let v41 = qname(&mut ast, "super.b");
    let v42 = v40.matches_qualified_name_node(&ast, v41);
    assert!(v42);
    let v43 = qname(&mut ast, "a.b.c");
    let v44 = qname(&mut ast, "a.b.c");
    let v45 = v43.matches_qualified_name_node(&ast, v44);
    assert!(v45);
    let v46 = qname(&mut ast, "a.b.c");
    let v47 = qname(&mut ast, "a.b.c");
    let v48 = v46.matches_qualified_name_node(&ast, v47);
    assert!(v48);
    let v49 = IR::number(&mut ast, 0.0);
    let v50 = qname(&mut ast, "a.b");
    let v51 = v49.matches_qualified_name_node(&ast, v50);
    assert!(!v51);
    let v52 = IR::arraylit(&mut ast, &[]);
    let v53 = qname(&mut ast, "a.b");
    let v54 = v52.matches_qualified_name_node(&ast, v53);
    assert!(!v54);
    let v55 = IR::objectlit(&mut ast, &[]);
    let v56 = qname(&mut ast, "a.b");
    let v57 = v55.matches_qualified_name_node(&ast, v56);
    assert!(!v57);
    let v58 = IR::string(&mut ast, "");
    let v59 = qname(&mut ast, "a.b");
    let v60 = v58.matches_qualified_name_node(&ast, v59);
    assert!(!v60);
    let v61 = IR::name(&mut ast, "a");
    let v62 = IR::string(&mut ast, "b");
    let v63 = IR::getelem(&mut ast, v61, v62);
    let v64 = qname(&mut ast, "a.b");
    let v65 = v63.matches_qualified_name_node(&ast, v64);
    assert!(!v65);
    let v66 = IR::name(&mut ast, "a");
    let v67 = IR::string(&mut ast, "b");
    let v68 = IR::getelem(&mut ast, v66, v67);
    let v69 = IR::getprop(&mut ast, v68, "c");
    let v70 = qname(&mut ast, "a.b.c");
    let v71 = v69.matches_qualified_name_node(&ast, v70);
    assert!(!v71);
    let v72 = IR::name(&mut ast, "a");
    let v73 = IR::getprop(&mut ast, v72, "b");
    let v74 = IR::string(&mut ast, "c");
    let v75 = IR::getelem(&mut ast, v73, v74);
    let v76 = v75.matches_qualified_name(&ast, "a.b.c");
    assert!(!v76);
    let v77 = IR::name(&mut ast, "a");
    let v78 = IR::call(&mut ast, v77, &[]);
    let v79 = qname(&mut ast, "a");
    let v80 = v78.matches_qualified_name_node(&ast, v79);
    assert!(!v80);
    let v81 = IR::name(&mut ast, "a");
    let v82 = IR::call(&mut ast, v81, &[]);
    let v83 = IR::getprop(&mut ast, v82, "b");
    let v84 = qname(&mut ast, "a.b");
    let v85 = v83.matches_qualified_name_node(&ast, v84);
    assert!(!v85);
    let v86 = IR::name(&mut ast, "a");
    let v87 = IR::getprop(&mut ast, v86, "b");
    let v88 = IR::call(&mut ast, v87, &[]);
    let v89 = qname(&mut ast, "a.b");
    let v90 = v88.matches_qualified_name_node(&ast, v89);
    assert!(!v90);
    let v91 = IR::string(&mut ast, "a");
    let v92 = qname(&mut ast, "a");
    let v93 = v91.matches_qualified_name_node(&ast, v92);
    assert!(!v93);
    let v94 = IR::string(&mut ast, "x");
    let v95 = IR::regexp(&mut ast, v94);
    let v96 = qname(&mut ast, "x");
    let v97 = v95.matches_qualified_name_node(&ast, v96);
    assert!(!v97);
    let v98 = IR::name(&mut ast, "x");
    let v99 = ast.new_node_with_child(Token::INC, v98);
    let v100 = qname(&mut ast, "x");
    let v101 = v99.matches_qualified_name_node(&ast, v100);
    assert!(!v101);
}

// port: NodeTest#testMatchesName
#[test]
fn test_matches_name() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "");
    let v2 = v1.matches_name(&ast, "");
    assert!(!v2);
    let v3 = IR::name(&mut ast, "a");
    let v4 = v3.matches_name(&ast, "a");
    assert!(v4);
    let v5 = IR::name(&mut ast, "a");
    let v6 = v5.matches_name(&ast, "a.b");
    assert!(!v6);
    let v7 = IR::name(&mut ast, "a");
    let v8 = v7.matches_name(&ast, "");
    assert!(!v8);
    let v9 = IR::this_node(&mut ast);
    let v10 = v9.matches_name(&ast, "this");
    assert!(!v10);
    let v11 = IR::super_node(&mut ast);
    let v12 = v11.matches_name(&ast, "super");
    assert!(!v12);
}

// port: NodeTest#testMatchesNameNodes
#[test]
fn test_matches_name_nodes() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "a");
    let v2 = qname(&mut ast, "a");
    let v3 = v1.matches_name_node(&ast, v2);
    assert!(v3);
    let v4 = IR::name(&mut ast, "a");
    let v5 = qname(&mut ast, "a.b");
    let v6 = v4.matches_name_node(&ast, v5);
    assert!(!v6);
    let v7 = IR::this_node(&mut ast);
    let v8 = qname(&mut ast, "this");
    let v9 = v7.matches_name_node(&ast, v8);
    assert!(!v9);
    let v10 = IR::super_node(&mut ast);
    let v11 = qname(&mut ast, "super");
    let v12 = v10.matches_name_node(&ast, v11);
    assert!(!v12);
}

// port: NodeTest#qname
fn qname(ast: &mut Ast, name: &str) -> NodeId {
    let name = JsString::from(name);
    let mut end_pos = name.index_of(".");
    if end_pos == -1 {
        return IR::name(ast, name);
    }
    let node_name = name.substring(0, end_pos as usize);
    let mut node = if node_name == "this" {
        IR::this_node(ast)
    } else if node_name == "super" {
        IR::super_node(ast)
    } else {
        IR::name(ast, node_name)
    };
    loop {
        let start_pos = end_pos + 1;
        end_pos = name.index_of_from(".", start_pos);
        let part = if end_pos == -1 {
            name.substring_from(start_pos as usize)
        } else {
            name.substring(start_pos as usize, end_pos as usize)
        };
        node = IR::getprop(ast, node, part);
        if end_pos == -1 {
            break;
        }
    }
    node
}

// port: NodeTest#testCloneAnnontations
#[test]
fn test_clone_annontations() {
    let mut ast = Ast::new();
    let v1 = get_var_ref(&mut ast, "a");
    let n = v1;
    n.set_length(&mut ast, 1);
    let v2 = n.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(!v2);
    n.put_boolean_prop(&mut ast, NodeId::IS_CONSTANT_NAME, true);
    let v3 = n.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v3);
    let v4 = n.clone_node(&mut ast);
    let node_clone = v4;
    let v5 = node_clone.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v5);
    let v6 = node_clone.get_length(&ast);
    assert_eq!(v6, 1);
}

// port: NodeTest#testCloneValues
#[test]
fn test_clone_values() {
    let mut ast = Ast::new();
    let v1 = ast.new_number(100.0);
    let number = v1;
    let v2 = number.clone_node(&mut ast);
    let v3 = v2.get_double(&ast);
    assert_eq!(v3, 100.0);
    let v4 = JsString::from("a");
    let v5 = ast.new_string(v4);
    let string = v5;
    let v6 = string.clone_node(&mut ast);
    let v7 = v6.get_string(&ast);
    let v8 = string.get_string(&ast);
    assert!(v7.ptr_eq(&v8));
    let v9 = JsString::from("a");
    let v10 = JsString::from("b");
    let v11 = ast.new_template_lit_string(Some(v9), v10);
    let template = v11;
    let v12 = template.clone_node(&mut ast);
    let v13 = v12.get_cooked_string(&ast);
    let v14 = template.get_cooked_string(&ast);
    assert!(v13.as_ref().unwrap().ptr_eq(v14.as_ref().unwrap()));
    let v15 = template.clone_node(&mut ast);
    let v16 = v15.get_raw_string(&ast);
    let v17 = template.get_raw_string(&ast);
    assert!(v16.ptr_eq(&v17));
    let v18 = "100".parse::<BigInt>().unwrap();
    let v19 = ast.new_big_int(v18);
    let bigint = v19;
    let v20 = bigint.clone_node(&mut ast);
    let v21 = v20.get_big_int(&ast);
    let v22 = bigint.get_big_int(&ast);
    assert!(Arc::ptr_eq(&v21, &v22));
}

// port: NodeTest#testSharedProps1
#[test]
fn test_shared_props1() {
    let mut ast = Ast::new();
    let v1 = get_call(&mut ast, "A");
    let n = v1;
    n.set_side_effect_flags(&mut ast, 5);
    let v2 = ast.new_node(Token::TRUE);
    let m = v2;
    m.clone_props_from(&mut ast, n);
    let v4 = n.get_prop_list_head_for_testing(&ast);
    let v5 = m.get_prop_list_head_for_testing(&ast);
    assert!(same_arc(&v4, &v5));
    let v6 = n.get_side_effect_flags(&ast);
    assert_eq!(v6, 5);
    let v7 = m.get_side_effect_flags(&ast);
    assert_eq!(v7, 5);
}

// port: NodeTest#testSharedProps2
#[test]
fn test_shared_props2() {
    let mut ast = Ast::new();
    let v1 = get_call(&mut ast, "A");
    let n = v1;
    n.set_side_effect_flags(&mut ast, 5);
    let v2 = get_call(&mut ast, "B");
    let m = v2;
    m.clone_props_from(&mut ast, n);
    n.set_side_effect_flags(&mut ast, 6);
    let v4 = n.get_side_effect_flags(&ast);
    assert_eq!(v4, 6);
    let v5 = m.get_side_effect_flags(&ast);
    assert_eq!(v5, 5);
    let v6 = m.get_prop_list_head_for_testing(&ast);
    let v7 = n.get_prop_list_head_for_testing(&ast);
    let v8 = same_arc(&v6, &v7);
    assert!(!v8);
    m.set_side_effect_flags(&mut ast, 7);
    let v9 = n.get_side_effect_flags(&ast);
    assert_eq!(v9, 6);
    let v10 = m.get_side_effect_flags(&ast);
    assert_eq!(v10, 7);
}

// port: NodeTest#testSharedProps3
#[test]
fn test_shared_props3() {
    let mut ast = Ast::new();
    let v1 = get_call(&mut ast, "A");
    let n = v1;
    n.set_side_effect_flags(&mut ast, 2);
    n.put_boolean_prop(&mut ast, NodeId::INCRDECR_PROP, true);
    let v2 = ast.new_node(Token::TRUE);
    let m = v2;
    m.clone_props_from(&mut ast, n);
    n.set_side_effect_flags(&mut ast, 4);
    let v4 = n.get_side_effect_flags(&ast);
    assert_eq!(v4, 4);
    let v5 = m.get_side_effect_flags(&ast);
    assert_eq!(v5, 2);
}

// port: NodeTest#testBooleanProp
#[test]
fn test_boolean_prop() {
    let mut ast = Ast::new();
    let v1 = get_var_ref(&mut ast, "a");
    let n = v1;
    n.put_boolean_prop(&mut ast, NodeId::IS_CONSTANT_NAME, false);
    let v2 = n.lookup_property(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v2.is_none());
    let v3 = n.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(!v3);
    n.put_boolean_prop(&mut ast, NodeId::IS_CONSTANT_NAME, true);
    let v4 = n.lookup_property(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v4.is_some());
    let v5 = n.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v5);
    n.put_boolean_prop(&mut ast, NodeId::IS_CONSTANT_NAME, false);
    let v6 = n.lookup_property(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v6.is_none());
    let v7 = n.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(!v7);
}

// port: NodeTest#testCloneAnnontations2
#[test]
fn test_clone_annontations2() {
    let mut ast = Ast::new();
    let v1 = get_var_ref(&mut ast, "a");
    let n = v1;
    n.put_boolean_prop(&mut ast, NodeId::IS_CONSTANT_NAME, true);
    let v2 = n.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v2);
    let v3 = n.clone_node(&mut ast);
    let node_clone = v3;
    let v4 = node_clone.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v4);
    let v5 = n.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v5);
    let v6 = node_clone.get_boolean_prop(&ast, NodeId::IS_CONSTANT_NAME);
    assert!(v6);
}

// port: NodeTest#bitsetFromNodeProperties
fn bitset_from_node_properties(props: &[NodeProperty]) -> i64 {
    let mut bitset = 0;
    for prop in props {
        bitset = NodeId::set_node_property_bit(bitset, *prop);
    }
    bitset
}

// port: NodeTest#testSerializeProperties
#[test]
fn test_serialize_properties() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "");
    let v2 = IR::param_list(&mut ast, &[]);
    let v3 = IR::block(&mut ast);
    let v4 = IR::function(&mut ast, v1, v2, v3);
    let node = v4;
    node.set_is_async_function(&mut ast, true);
    node.set_is_generator_function(&mut ast, true);
    let v5 = node.serialize_properties(&ast);
    let result = v5;
    let v6 = vec![NodeProperty::GENERATOR_FN, NodeProperty::ASYNC_FN];
    let v7 = bitset_from_node_properties(&v6);
    assert_eq!(result, v7);
}

// port: NodeTest#testSerializeProperties_isDeclaredConstant
#[test]
fn test_serialize_properties_is_declared_constant() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::NAME);
    let node = v1;
    node.set_declared_constant_var(&mut ast, true);
    let v2 = node.serialize_properties(&ast);
    let result = v2;
    let v3 = vec![NodeProperty::IS_DECLARED_CONSTANT];
    let v4 = bitset_from_node_properties(&v3);
    assert_eq!(result, v4);
}

// port: NodeTest#testSerializeProperties_isInferredConstant
#[test]
fn test_serialize_properties_is_inferred_constant() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::NAME);
    let node = v1;
    node.set_inferred_constant_var(&mut ast, true);
    let v2 = node.serialize_properties(&ast);
    let result = v2;
    let v3 = vec![NodeProperty::IS_INFERRED_CONSTANT];
    let v4 = bitset_from_node_properties(&v3);
    assert_eq!(result, v4);
}

// port: NodeTest#testSerializeProperties_untranslatableRhinoProp
#[test]
fn test_serialize_properties_untranslatable_rhino_prop() {
    let mut ast = Ast::new();
    let v1 = get_call(&mut ast, "A");
    let node = v1;
    node.set_use_strict(&mut ast, true);
    let v2 = node.serialize_properties(&ast);
    let result = v2;
    let v3 = node.is_use_strict(&ast);
    assert!(v3);
    assert_eq!(result, 0);
}

// port: NodeTest#testSerializeProperties_privateIdentifier
#[test]
fn test_serialize_properties_private_identifier() {
    let mut ast = Ast::new();
    let v1 = ast.new_string_with_token(Token::NAME, "#field");
    let node = v1;
    node.set_private_identifier(&mut ast);
    let v2 = node.serialize_properties(&ast);
    let result = v2;
    let v3 = vec![NodeProperty::PRIVATE_IDENTIFIER];
    let v4 = bitset_from_node_properties(&v3);
    assert_eq!(result, v4);
}

// port: NodeTest#testSerializeProperties_privateIdentifierRoundTrip
#[test]
fn test_serialize_properties_private_identifier_round_trip() {
    let mut ast = Ast::new();
    let v1 = ast.new_string_with_token(Token::NAME, "#field");
    let original = v1;
    original.set_source_file_for_testing(&mut ast, "sourcefile");
    let v2 = original.clone_node(&mut ast);
    let restored = v2;
    let v3 = restored.is_private_identifier(&ast);
    assert!(!v3);
    original.set_private_identifier(&mut ast);
    let v4 = original.serialize_properties(&ast);
    restored.deserialize_properties(&mut ast, v4, false);
    let v5 = restored.is_private_identifier(&ast);
    assert!(v5);
}

// port: NodeTest#testGetIndexOfChild
#[test]
fn test_get_index_of_child() {
    let mut ast = Ast::new();
    let v1 = get_assign_expr(&mut ast, "b", "c");
    let assign = v1;
    let v2 = assign.get_child_count(&ast);
    assert_eq!(v2, 2);
    let v3 = assign.get_first_child(&ast);
    let first_child = v3;
    let v4 = first_child.unwrap().get_next(&ast);
    let second_child = v4;
    assert!(second_child.is_some());
    let v5 = assign.get_index_of_child(&ast, first_child.unwrap());
    assert_eq!(v5, 0);
    let v6 = assign.get_index_of_child(&ast, second_child.unwrap());
    assert_eq!(v6, 1);
    let v7 = assign.get_index_of_child(&ast, assign);
    assert_eq!(v7, -1);
}

// port: NodeTest#testSrcrefIfMissing
#[test]
fn test_srcref_if_missing() {
    let mut ast = Ast::new();
    let v1 = get_assign_expr(&mut ast, "b", "c");
    let assign = v1;
    assign.set_lineno_charno(&mut ast, 99, 0);
    assign.set_source_file_for_testing(&mut ast, "foo.js");
    let v2 = assign.get_first_child(&ast);
    let lhs = v2;
    lhs.unwrap().srcref_if_missing(&mut ast, assign);
    let v4 = assert_node(lhs.unwrap());
    v4.has_lineno(&ast, 99);
    let v6 = lhs.unwrap().get_source_file_name(&ast);
    assert_eq!(v6.as_ref().unwrap(), "foo.js");
    assign.set_lineno_charno(&mut ast, 101, 0);
    assign.set_source_file_for_testing(&mut ast, "bar.js");
    lhs.unwrap().srcref_if_missing(&mut ast, assign);
    let v8 = assert_node(lhs.unwrap());
    v8.has_lineno(&ast, 99);
    let v10 = lhs.unwrap().get_source_file_name(&ast);
    assert_eq!(v10.as_ref().unwrap(), "foo.js");
}

// port: NodeTest#testSrcref
#[test]
fn test_srcref() {
    let mut ast = Ast::new();
    let v1 = get_assign_expr(&mut ast, "b", "c");
    let assign = v1;
    assign.set_lineno_charno(&mut ast, 99, 0);
    assign.set_source_file_for_testing(&mut ast, "foo.js");
    let v2 = assign.get_first_child(&ast);
    let lhs = v2;
    lhs.unwrap().srcref(&mut ast, assign);
    let v4 = assert_node(lhs.unwrap());
    v4.has_lineno(&ast, 99);
    let v6 = lhs.unwrap().get_source_file_name(&ast);
    assert_eq!(v6.as_ref().unwrap(), "foo.js");
    assign.set_lineno_charno(&mut ast, 101, 0);
    assign.set_source_file_for_testing(&mut ast, "bar.js");
    lhs.unwrap().srcref(&mut ast, assign);
    let v8 = assert_node(lhs.unwrap());
    v8.has_lineno(&ast, 101);
    let v10 = lhs.unwrap().get_source_file_name(&ast);
    assert_eq!(v10.as_ref().unwrap(), "bar.js");
}

// port: NodeTest#testInvalidSourceOffset
#[test]
fn test_invalid_source_offset() {
    let mut ast = Ast::new();
    let v1 = ast.new_string("a");
    let string = v1;
    string.set_lineno_charno(&mut ast, -1, -1);
    let v2 = string.get_source_offset(&ast);
    assert!(v2 < 0);
    string.set_source_file_for_testing(&mut ast, "foo.js");
    let v3 = string.get_source_offset(&ast);
    assert!(v3 < 0);
}

// port: NodeTest#testQualifiedName
#[test]
fn test_qualified_name() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "");
    let v2 = v1.get_qualified_name(&ast);
    assert!(v2.is_none());
    let v3 = IR::name(&mut ast, "a");
    let v4 = v3.get_qualified_name(&ast);
    assert_eq!(v4.as_ref().unwrap(), "a");
    let v5 = IR::this_node(&mut ast);
    let v6 = v5.get_qualified_name(&ast);
    assert_eq!(v6.as_ref().unwrap(), "this");
    let v7 = IR::super_node(&mut ast);
    let v8 = v7.get_qualified_name(&ast);
    assert_eq!(v8.as_ref().unwrap(), "super");
    let v9 = IR::name(&mut ast, "a");
    let v10 = IR::getprop(&mut ast, v9, "b");
    let v11 = v10.get_qualified_name(&ast);
    assert_eq!(v11.as_ref().unwrap(), "a.b");
    let v12 = IR::this_node(&mut ast);
    let v13 = IR::getprop(&mut ast, v12, "b");
    let v14 = v13.get_qualified_name(&ast);
    assert_eq!(v14.as_ref().unwrap(), "this.b");
    let v15 = IR::super_node(&mut ast);
    let v16 = IR::getprop(&mut ast, v15, "b");
    let v17 = v16.get_qualified_name(&ast);
    assert_eq!(v17.as_ref().unwrap(), "super.b");
    let v18 = IR::name(&mut ast, "a");
    let v19 = IR::call(&mut ast, v18, &[]);
    let v20 = IR::getprop(&mut ast, v19, "b");
    let v21 = v20.get_qualified_name(&ast);
    assert!(v21.is_none());
}

// port: NodeTest#testJSDocInfoClone
#[test]
fn test_jsdoc_info_clone() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "varName");
    let v2 = IR::var(&mut ast, v1);
    let original = v2;
    let v3 = JSDocInfo::builder();
    let mut builder = v3;
    let v4 = IR::name(&mut ast, "TypeName");
    let v5 = Arc::new(JSTypeExpression::new(v4, "blah"));
    builder.record_type(Some(v5));
    let v7 = builder.build();
    let info = v7;
    let v8 = original.get_first_child(&ast);
    v8.unwrap().set_jsdoc_info(&mut ast, info);
    let v9 = original.clone_tree(&mut ast);
    let mut clone = v9;
    let v10 = clone.get_first_child(&ast);
    let v11 = v10.unwrap().get_jsdoc_info(&ast);
    let v12 = original.get_first_child(&ast);
    let v13 = v12.unwrap().get_jsdoc_info(&ast);
    assert!(Arc::ptr_eq(v11.as_ref().unwrap(), v13.as_ref().unwrap()));
    let v14 = clone.get_first_child(&ast);
    let v15 = v14.unwrap().get_jsdoc_info(&ast);
    let v16 = v15.as_ref().unwrap().get_type();
    let v17 = original.get_first_child(&ast);
    let v18 = v17.unwrap().get_jsdoc_info(&ast);
    let v19 = v18.as_ref().unwrap().get_type();
    assert!(Arc::ptr_eq(v16.as_ref().unwrap(), v19.as_ref().unwrap()));
    let v20 = clone.get_first_child(&ast);
    let v21 = v20.unwrap().get_jsdoc_info(&ast);
    let v22 = v21.as_ref().unwrap().get_type();
    let v23 = v22.as_ref().unwrap().get_root();
    let v24 = original.get_first_child(&ast);
    let v25 = v24.unwrap().get_jsdoc_info(&ast);
    let v26 = v25.as_ref().unwrap().get_type();
    let v27 = v26.as_ref().unwrap().get_root();
    assert!(v23 == v27);
    let v28 = original.clone_tree_with_type_exprs(&mut ast, true);
    clone = v28;
    let v29 = clone.get_first_child(&ast);
    let v30 = v29.unwrap().get_jsdoc_info(&ast);
    let v31 = original.get_first_child(&ast);
    let v32 = v31.unwrap().get_jsdoc_info(&ast);
    assert!(!(Arc::ptr_eq(v30.as_ref().unwrap(), v32.as_ref().unwrap())));
    let v33 = clone.get_first_child(&ast);
    let v34 = v33.unwrap().get_jsdoc_info(&ast);
    let v35 = v34.as_ref().unwrap().get_type();
    let v36 = original.get_first_child(&ast);
    let v37 = v36.unwrap().get_jsdoc_info(&ast);
    let v38 = v37.as_ref().unwrap().get_type();
    assert!(!(Arc::ptr_eq(v35.as_ref().unwrap(), v38.as_ref().unwrap())));
    let v39 = clone.get_first_child(&ast);
    let v40 = v39.unwrap().get_jsdoc_info(&ast);
    let v41 = v40.as_ref().unwrap().get_type();
    let v42 = v41.as_ref().unwrap().get_root();
    let v43 = original.get_first_child(&ast);
    let v44 = v43.unwrap().get_jsdoc_info(&ast);
    let v45 = v44.as_ref().unwrap().get_type();
    let v46 = v45.as_ref().unwrap().get_root();
    assert_ne!(v42, v46);
}

// port: NodeTest#testAddChildToFrontWithSingleNode
#[test]
fn test_add_child_to_front_with_single_node() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::SCRIPT);
    let root = v1;
    let v2 = ast.new_node(Token::SCRIPT);
    let node_to_add = v2;
    root.add_child_to_front(&mut ast, node_to_add);
    let v3 = node_to_add.get_parent(&ast);
    assert_eq!(v3, Some(root));
    let v4 = root.get_first_child(&ast);
    assert_eq!(Some(node_to_add), v4);
    let v5 = root.get_last_child(&ast);
    assert_eq!(Some(node_to_add), v5);
    let v6 = node_to_add.get_next(&ast);
    assert!(v6.is_none());
}

// port: NodeTest#testAddChildToFrontWithLargerTree
#[test]
fn test_add_child_to_front_with_larger_tree() {
    let mut ast = Ast::new();
    let v1 = ast.new_string("left");
    let left = v1;
    let v2 = ast.new_string("mid");
    let mid = v2;
    let v3 = ast.new_string("right");
    let right = v3;
    let v4 = ast.new_node_with_children3(Token::SCRIPT, left, mid, right);
    let root = v4;
    let v5 = ast.new_node(Token::SCRIPT);
    let node_to_add = v5;
    root.add_child_to_front(&mut ast, node_to_add);
    let v6 = node_to_add.get_parent(&ast);
    assert_eq!(v6, Some(root));
    let v7 = root.get_first_child(&ast);
    assert_eq!(Some(node_to_add), v7);
    let v8 = node_to_add.get_previous(&ast);
    assert!(v8.is_none());
    let v9 = node_to_add.get_next(&ast);
    assert_eq!(v9, Some(left));
    let v10 = left.get_previous(&ast);
    assert_eq!(v10, Some(node_to_add));
}

// port: NodeTest#testDetach1
#[test]
fn test_detach1() {
    let mut ast = Ast::new();
    let v1 = ast.new_string("left");
    let left = v1;
    let v2 = ast.new_string("mid");
    let mid = v2;
    let v3 = ast.new_string("right");
    let right = v3;
    let v4 = ast.new_node_with_children3(Token::SCRIPT, left, mid, right);
    let root = v4;
    let v5 = mid.get_parent(&ast);
    assert_eq!(v5, Some(root));
    let v6 = mid.get_previous(&ast);
    assert_eq!(v6, Some(left));
    let v7 = mid.get_next(&ast);
    assert_eq!(v7, Some(right));
    mid.detach(&mut ast);
    let v9 = mid.get_parent(&ast);
    assert!(v9.is_none());
    let v10 = mid.get_next(&ast);
    assert!(v10.is_none());
    let v11 = right.get_previous(&ast);
    assert_eq!(v11, Some(left));
    let v12 = left.get_next(&ast);
    assert_eq!(v12, Some(right));
}

// port: NodeTest#testGetAncestors
#[test]
fn test_get_ancestors() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::ROOT);
    let grandparent = v1;
    let v2 = ast.new_node(Token::PLACEHOLDER1);
    let parent = v2;
    let v3 = ast.new_node(Token::PLACEHOLDER2);
    let node = v3;
    grandparent.add_child_to_front(&mut ast, parent);
    parent.add_child_to_front(&mut ast, node);
    let v4 = node.get_ancestors(&ast);
    assert_contains_exactly(v4.collect::<Vec<_>>(), vec![parent, grandparent]);
}

// port: NodeTest#testGetAncestors_empty
#[test]
fn test_get_ancestors_empty() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::ROOT);
    let node = v1;
    let v2 = node.get_ancestors(&ast);
    assert!(v2.count() == 0);
}

// port: NodeTest#testTrailingComma
#[test]
fn test_trailing_comma() {
    let mut ast = Ast::new();
    let v1 = ast.new_node(Token::ARRAYLIT);
    let list = v1;
    list.set_trailing_comma(&mut ast, true);
    let v2 = assert_node(list);
    v2.has_trailing_comma(&ast);
}

// port: NodeTest#getVarRef
fn get_var_ref(ast: &mut Ast, name: &str) -> NodeId {
    ast.new_string_with_token(Token::NAME, name)
}
// port: NodeTest#getAssignExpr
fn get_assign_expr(ast: &mut Ast, name1: &str, name2: &str) -> NodeId {
    let lhs = get_var_ref(ast, name1);
    let rhs = get_var_ref(ast, name2);
    ast.new_node_with_children2(Token::ASSIGN, lhs, rhs)
}
// port: NodeTest#getCall
fn get_call(ast: &mut Ast, name1: &str) -> NodeId {
    let callee = get_var_ref(ast, name1);
    ast.new_node_with_child(Token::CALL, callee)
}

// port: NodeTest#testValidateMemoryGuarantees_checksForDifferentSourceFileInstances
#[test]
fn test_validate_memory_guarantees_checks_for_different_source_file_instances() {
    let mut ast = Ast::new();
    let v1 = IR::block(&mut ast);
    let b1 = v1;
    let v2 = IR::block(&mut ast);
    let b2 = v2;
    let v3 = IR::block(&mut ast);
    let other = v3;
    b1.set_source_file_for_testing(&mut ast, "file.js");
    b2.set_static_source_file_from(&mut ast, b1);
    let v4 = b1.get_static_source_file(&ast);
    other.set_static_source_file(&mut ast, v4);
    let v5 = NodeId::validate_memory_sensitive_property_guarantees(&ast, b1, false, b2, false);
    assert!(v5);
    let v6 = NodeId::validate_memory_sensitive_property_guarantees(&ast, b1, false, other, false);
    assert!(!v6);
}

fn same_arc<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

// Truth's containsExactly checks values and multiplicities without ordering.
fn assert_contains_exactly<T: PartialEq + std::fmt::Debug>(mut actual: Vec<T>, expected: Vec<T>) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for value in expected {
        let position = actual
            .iter()
            .position(|item| item == &value)
            .unwrap_or_else(|| panic!("missing {value:?} in {actual:?}"));
        actual.remove(position);
    }
}

// port: NodeTest#testValidatePropertiesForColorFromCast
#[test]
fn test_validate_properties_for_color_from_cast() {
    let mut ast = Ast::new();
    let n = IR::name(&mut ast, "a");
    n.set_source_file_for_testing(&mut ast, "file.js");
    n.set_color(&mut ast, Some(sc::NUMBER.clone()));
    n.set_color_from_type_cast(&mut ast);
    assert!(get_messages_from_validate_properties(&ast, n).is_empty());
    n.set_color(&mut ast, None);
    assert_contains_exactly(
        get_messages_from_validate_properties(&ast, n),
        vec!["COLOR_FROM_CAST with no Color".into()],
    );
}
// port: NodeTest#testCheckTreeTypeAwareEqualsColorsSame
#[test]
fn test_check_tree_type_aware_equals_colors_same() {
    let mut ast = Ast::new();
    let node1 = ast.new_string_with_token(Token::NAME, "f");
    node1.set_color(&mut ast, Some(sc::NUMBER.clone()));
    let node2 = ast.new_string_with_token(Token::NAME, "f");
    node2.set_color(&mut ast, Some(sc::NUMBER.clone()));
    assert!(node1.is_equivalent_to_typed(&ast, node2));
}
// port: NodeTest#testCheckTreeTypeAwareEqualsColorsDifferent
#[test]
fn test_check_tree_type_aware_equals_colors_different() {
    let mut ast = Ast::new();
    let node1 = ast.new_string_with_token(Token::NAME, "f");
    node1.set_color(&mut ast, Some(sc::NUMBER.clone()));
    let node2 = ast.new_string_with_token(Token::NAME, "f");
    node2.set_color(&mut ast, Some(sc::STRING.clone()));
    assert!(!node1.is_equivalent_to_typed(&ast, node2));
}
// port: NodeTest#testCheckTreeTypeAwareEqualsColorsDifferentNull
#[test]
fn test_check_tree_type_aware_equals_colors_different_null() {
    let mut ast = Ast::new();
    let node1 = ast.new_string_with_token(Token::NAME, "f");
    node1.set_color(&mut ast, Some(sc::NUMBER.clone()));
    let node2 = ast.new_string_with_token(Token::NAME, "f");
    assert!(!node1.is_equivalent_to_typed(&ast, node2));
}
// port: NodeTest#testIsPos
#[test]
fn test_is_pos() {
    let mut ast = Ast::new();
    let pos = ast.new_node(Token::POS);
    assert!(pos.is_pos(&ast));
    let neg = ast.new_node(Token::NEG);
    assert!(!neg.is_pos(&ast));
    let number = ast.new_number(1.0);
    assert!(!number.is_pos(&ast));
}
