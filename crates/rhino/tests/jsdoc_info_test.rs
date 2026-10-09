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
//   test/com/google/javascript/rhino/JSDocInfoTest.java.

use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::{JSDocInfo, PerFileClosureUnawareMode, Visibility::*},
    node::Ast,
    token::Token,
};
use std::sync::Arc;
// port: JSDocInfoTest#fromString
fn from_string(ast: &mut Ast, s: &str) -> Option<Arc<JSTypeExpression>> {
    Some(Arc::new(JSTypeExpression::new(ast.new_string(s), "")))
}
// port: JSDocInfoTest#createSampleTypeExpression
fn create_sample_type_expression(ast: &mut Ast) -> Arc<JSTypeExpression> {
    let item = ast.new_string("Item");
    let root = ast.new_node_with_child(Token::BANG, item);
    let s = ast.new_string("string");
    let child1 = ast.new_node_with_child(Token::QMARK, s);
    let s = ast.new_string("boolean");
    let child2 = ast.new_node_with_child(Token::QMARK, s);
    let s = ast.new_string("AnotherItem");
    let child3 = ast.new_node_with_child(Token::QMARK, s);
    root.add_child_to_back(ast, child1);
    root.add_child_to_back(ast, child2);
    root.add_child_to_back(ast, child3);
    Arc::new(JSTypeExpression::new(root, ""))
}
// port: JSDocInfoTest#createSampleTypeExpression_rootReplacement
fn create_sample_type_expression_root_replacement(ast: &mut Ast) -> Arc<JSTypeExpression> {
    let root = ast.new_string("Item");
    let s = ast.new_string("string");
    let child1 = ast.new_node_with_child(Token::QMARK, s);
    let s = ast.new_string("boolean");
    let child2 = ast.new_node_with_child(Token::QMARK, s);
    let s = ast.new_string("AnotherItem");
    let child3 = ast.new_node_with_child(Token::QMARK, s);
    root.add_child_to_back(ast, child1);
    root.add_child_to_back(ast, child2);
    root.add_child_to_back(ast, child3);
    Arc::new(JSTypeExpression::new(root, ""))
}
// port: JSDocInfoTest#EMPTY
fn empty() -> Arc<JSDocInfo> {
    JSDocInfo::builder().build_with_always(true).unwrap()
}
// port: JSDocInfoTest#testVisibilityOrdinal
#[test]
fn test_visibility_ordinal() {
    assert_eq!(PRIVATE as i32, 0);
    assert_eq!(PACKAGE as i32, 1);
    assert_eq!(PROTECTED as i32, 2);
    assert_eq!(PUBLIC as i32, 3);
}
// port: JSDocInfoTest#testSetConstant
#[test]
fn test_set_constant() {
    let mut builder = JSDocInfo::builder();
    builder.record_constancy();
    let info = builder.build().unwrap();
    assert!(!info.has_type());
    assert!(info.is_constant());
    assert!(!info.is_constructor());
    assert!(!info.is_define());
}
// port: JSDocInfoTest#testSetConstructor
#[test]
fn test_set_constructor() {
    let mut builder = JSDocInfo::builder();
    builder.record_constructor();
    let info = builder.build().unwrap();
    assert!(!info.is_constant());
    assert!(info.is_constructor());
    assert!(!info.is_define());
}
// port: JSDocInfoTest#testSetDefine
#[test]
fn test_set_define() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_define_type(from_string(&mut ast, "string"));
    let info = builder.build().unwrap();
    assert!(info.is_constant());
    assert!(!info.is_constructor());
    assert!(info.is_define());
}
// port: JSDocInfoTest#testSetTypeSummary
#[test]
fn test_set_type_summary() {
    let mut builder = JSDocInfo::builder();
    builder.record_type_summary();
    let info = builder.build().unwrap();
    assert!(info.is_type_summary());
}
// port: JSDocInfoTest#testSetOverride
#[test]
fn test_set_override() {
    let mut builder = JSDocInfo::builder();
    builder.record_override();
    let info = builder.build().unwrap();
    assert!(!info.is_deprecated());
    assert!(info.is_override());
}
// port: JSDocInfoTest#testSetExport
#[test]
fn test_set_export() {
    let mut builder = JSDocInfo::builder();
    builder.record_export();
    let info = builder.build().unwrap();
    assert!(info.is_export());
}
// port: JSDocInfoTest#testSetPolymerBehavior
#[test]
fn test_set_polymer_behavior() {
    assert!(!empty().is_polymer_behavior());
    let mut builder = JSDocInfo::builder();
    builder.record_polymer_behavior();
    let info = builder.build().unwrap();
    assert!(info.is_polymer_behavior());
}
// port: JSDocInfoTest#testSetPolymer
#[test]
fn test_set_polymer() {
    assert!(!empty().is_polymer());
    let mut builder = JSDocInfo::builder();
    builder.record_polymer();
    let info = builder.build().unwrap();
    assert!(info.is_polymer());
}
// port: JSDocInfoTest#testSetCustomElement
#[test]
fn test_set_custom_element() {
    assert!(!empty().is_custom_element());
    let mut builder = JSDocInfo::builder();
    builder.record_custom_element();
    let info = builder.build().unwrap();
    assert!(info.is_custom_element());
}
// port: JSDocInfoTest#testSetMixinClass
#[test]
fn test_set_mixin_class() {
    assert!(!empty().is_mixin_class());
    let mut builder = JSDocInfo::builder();
    builder.record_mixin_class();
    let info = builder.build().unwrap();
    assert!(info.is_mixin_class());
}
// port: JSDocInfoTest#testSetMixinFunction
#[test]
fn test_set_mixin_function() {
    assert!(!empty().is_mixin_function());
    let mut builder = JSDocInfo::builder();
    builder.record_mixin_function();
    let info = builder.build().unwrap();
    assert!(info.is_mixin_function());
}
// port: JSDocInfoTest#testSetNoAlias
#[test]
fn test_set_no_alias() {
    assert!(!empty().is_deprecated());
    assert!(!empty().is_override());
}
// port: JSDocInfoTest#testSetDeprecated
#[test]
fn test_set_deprecated() {
    let mut builder = JSDocInfo::builder();
    builder.record_deprecated();
    let info = builder.build().unwrap();
    assert!(!info.is_override());
    assert!(info.is_deprecated());
}
// port: JSDocInfoTest#testMultipleSetFlags1
#[test]
fn test_multiple_set_flags1() {
    let mut builder = JSDocInfo::builder();
    builder.record_constancy();
    builder.record_constructor();
    builder.record_no_collapse();
    let mut info = builder.build().unwrap();
    assert!(!info.has_type());
    assert!(info.is_constant());
    assert!(info.is_constructor());
    assert!(!info.is_define());
    assert!(info.is_no_collapse());
    builder = info.to_builder();
    builder.record_mutable();
    info = builder.build().unwrap();
    assert!(!info.is_constant());
    assert!(!info.is_define());
    assert!(info.is_no_collapse());
}
// port: JSDocInfoTest#testDescriptionContainsAtSignCode
#[test]
fn test_description_contains_at_sign_code() {
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_original_comment_string("Blah blah {@code blah blah} blah blah.");
    let info = builder.build().unwrap();
    assert!(info.is_at_sign_code_present());
}
// port: JSDocInfoTest#testDescriptionDoesNotContainAtSignCode
#[test]
fn test_description_does_not_contain_at_sign_code() {
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_original_comment_string("Blah blah `blah blah` blah blah.");
    let info = builder.build().unwrap();
    assert!(!info.is_at_sign_code_present());
}
// port: JSDocInfoTest#testCloneTypeExpressions2
#[test]
fn test_clone_type_expressions2() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_parameter("a", None);
    let info = builder.build().unwrap();
    let cloned = info.clone_with_type_nodes(&mut ast, true);
    assert!(cloned.get_parameter_type("a").is_none());
}
// port: JSDocInfoTest#testSetFileOverviewWithDocumentationOff
#[test]
fn test_set_file_overview_with_documentation_off() {
    let mut builder = JSDocInfo::builder();
    builder.record_file_overview("hi bob");
    let info = builder.build_with_always(true).unwrap();
    assert!(info.get_file_overview().is_none());
}
// port: JSDocInfoTest#testSetFileOverviewWithDocumentationOn
#[test]
fn test_set_file_overview_with_documentation_on() {
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_file_overview("hi bob");
    let info = builder.build().unwrap();
    assert_eq!(info.get_file_overview(), Some(JsString::from("hi bob")));
}
// port: JSDocInfoTest#testSetSuppressions
#[test]
fn test_set_suppressions() {
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_suppressions(&IndexSet::<_>::from_iter([
        JsString::from("sam"),
        JsString::from("bob"),
    ]));
    builder.record_suppression("fred");
    let info = builder.build().unwrap();
    assert_eq!(
        info.get_suppressions(),
        IndexSet::<_>::from_iter([
            JsString::from("bob"),
            JsString::from("sam"),
            JsString::from("fred")
        ])
    );
}
// port: JSDocInfoTest#testSetModifies
#[test]
fn test_set_modifies() {
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_modifies(&IndexSet::<_>::from_iter([JsString::from("this")]));
    let mut info = builder.build().unwrap();
    assert_eq!(
        info.get_modifies(),
        IndexSet::<_>::from_iter([JsString::from("this")])
    );
    builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_modifies(&IndexSet::<_>::from_iter([JsString::from("arguments")]));
    info = builder.build().unwrap();
    assert_eq!(
        info.get_modifies(),
        IndexSet::<_>::from_iter([JsString::from("arguments")])
    );
}
// port: JSDocInfoTest#testAddSingleTemplateTypeName
#[test]
fn test_add_single_template_type_name() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    assert!(builder.record_template_type_name(&mut ast, "T"));
    let info = builder.build().unwrap();
    let type_names = vec![JsString::from("T")];
    assert_eq!(info.get_template_type_names(), type_names);
}
// port: JSDocInfoTest#testAddMultipleTemplateTypeName
#[test]
fn test_add_multiple_template_type_name() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    let type_names = vec![JsString::from("T"), JsString::from("R")];
    builder.record_template_type_name(&mut ast, "T");
    builder.record_template_type_name(&mut ast, "R");
    let info = builder.build().unwrap();
    assert_eq!(info.get_template_type_names(), type_names);
}
// port: JSDocInfoTest#testFailToAddTemplateTypeName
#[test]
fn test_fail_to_add_template_type_name() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_template_type_name(&mut ast, "T");
    assert!(!builder.record_template_type_name(&mut ast, "T"));
}
// port: JSDocInfoTest#testGetThrowsDescription
#[test]
fn test_get_throws_description() {
    let mut builder = JSDocInfo::builder();
    builder.parse_documentation();
    builder.record_description("Lorem");
    builder.record_throws_annotation("{Error} Because it does.");
    builder.record_throws_annotation("{not a type}");
    let info = builder.build().unwrap();
    assert_eq!(
        info.get_throws_annotations(),
        vec![
            JsString::from("{Error} Because it does."),
            JsString::from("{not a type}")
        ]
    );
}
// port: JSDocInfoTest#testGetTypeNodes_excludesNull
#[test]
fn test_get_type_nodes_excludes_null() {
    let ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    assert!(!builder.record_implemented_interface(&ast, None));
    let info = builder.build_with_always(true).unwrap();
    let nodes = info.get_type_nodes();
    assert!(nodes.is_empty());
}
// port: JSDocInfoTest#testContainsDeclaration_implements
#[test]
fn test_contains_declaration_implements() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_visibility(INHERITED);
    let interface = from_string(&mut ast, "MyInterface");
    builder.record_implemented_interface(&ast, interface);
    let info = builder.build().unwrap();
    assert_eq!(info.get_implemented_interface_count(), 1);
    assert!(info.contains_declaration());
}
// port: JSDocInfoTest#testContainsDeclaration_extends
#[test]
fn test_contains_declaration_extends() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_visibility(INHERITED);
    builder.record_base_type(from_string(&mut ast, "MyBaseClass"));
    let info = builder.build().unwrap();
    assert!(info.has_base_type());
    assert!(info.contains_declaration());
}
// port: JSDocInfoTest#testRecordClosureUnaware_perFileMode
#[test]
fn test_record_closure_unaware_per_file_mode() {
    let mut builder = JSDocInfo::builder();
    builder.record_closure_unaware_code_with_mode(PerFileClosureUnawareMode::WHITESPACE);
    let info = builder.build().unwrap();
    assert!(info.is_closure_unaware_code());
    assert_eq!(
        info.get_per_file_closure_unaware_mode(),
        Some(PerFileClosureUnawareMode::WHITESPACE)
    );
}
// port: JSDocInfoTest#testRecordClosureUnaware_perFileMode_duplicate
#[test]
fn test_record_closure_unaware_per_file_mode_duplicate() {
    let mut builder = JSDocInfo::builder();
    builder.record_closure_unaware_code_with_mode(PerFileClosureUnawareMode::WHITESPACE);
    builder.record_closure_unaware_code_with_mode(PerFileClosureUnawareMode::SIMPLE);
    let info = builder.build().unwrap();
    assert!(info.is_closure_unaware_code());
    assert_eq!(
        info.get_per_file_closure_unaware_mode(),
        Some(PerFileClosureUnawareMode::WHITESPACE)
    );
}
// port: JSDocInfoTest#testNoClosureUnawarePerFileMode_defaultsToUnspecified
#[test]
fn test_no_closure_unaware_per_file_mode_defaults_to_unspecified() {
    let mut builder = JSDocInfo::builder();
    builder.record_closure_unaware_code();
    let info = builder.build().unwrap();
    assert!(info.is_closure_unaware_code());
    assert_eq!(
        info.get_per_file_closure_unaware_mode(),
        Some(PerFileClosureUnawareMode::UNSPECIFIED)
    );
}
// port: JSDocInfoTest#testRemovesModuleLocalNames
#[test]
fn test_removes_module_local_names() {
    let mut ast = Ast::new();
    let expr = create_sample_type_expression(&mut ast);
    let names = IndexSet::<_>::from_iter([JsString::from("Item"), JsString::from("AnotherItem")]);
    let new_expr = expr.replace_names_with_unknown_type(&mut ast, &names);
    let replaced = new_expr.get_all_type_names(&ast);
    assert!(!replaced.contains(&JsString::from("Item")));
    assert!(replaced.contains(&JsString::from("string")));
    assert!(replaced.contains(&JsString::from("boolean")));
    assert!(!replaced.contains(&JsString::from("AnotherItem")));
}
// port: JSDocInfoTest#testJSDocInfoCloneAndReplaceNames_params
#[test]
fn test_jsdoc_info_clone_and_replace_names_params() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    let expr = create_sample_type_expression(&mut ast);
    builder.record_parameter("a", Some(expr.clone()));
    let info = builder.build().unwrap();
    let names = IndexSet::<_>::from_iter([JsString::from("Item"), JsString::from("AnotherItem")]);
    let cloned = info.clone_and_replace_type_names(&mut ast, &names);
    assert_eq!(cloned.get_parameter_count(), 1);
    assert_eq!(cloned.get_parameter_name_at(0), Some(JsString::from("a")));
    let t = cloned.get_parameter_type("a").unwrap();
    assert!(!Arc::ptr_eq(&t, &expr));
    assert!(!t.get_all_type_names(&ast).contains(&JsString::from("Item")));
    assert!(
        !t.get_all_type_names(&ast)
            .contains(&JsString::from("AnotherItem"))
    );
}
// port: JSDocInfoTest#testJSDocInfoCloneAndReplaceNames_Type
#[test]
fn test_jsdoc_info_clone_and_replace_names_type() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    let expr = create_sample_type_expression(&mut ast);
    builder.record_type(Some(expr.clone()));
    let info = builder.build().unwrap();
    let names = IndexSet::<_>::from_iter([JsString::from("Item"), JsString::from("AnotherItem")]);
    let cloned = info.clone_and_replace_type_names(&mut ast, &names);
    let t = cloned.get_type().unwrap();
    assert!(!Arc::ptr_eq(&t, &expr));
    assert!(!t.get_all_type_names(&ast).contains(&JsString::from("Item")));
    assert!(
        !t.get_all_type_names(&ast)
            .contains(&JsString::from("AnotherItem"))
    );
}
// port: JSDocInfoTest#testJSDocInfoCloneAndReplaceNames_Type_rootReplacement
#[test]
fn test_jsdoc_info_clone_and_replace_names_type_root_replacement() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    let expr = create_sample_type_expression_root_replacement(&mut ast);
    builder.record_type(Some(expr.clone()));
    let info = builder.build().unwrap();
    let names = IndexSet::<_>::from_iter([JsString::from("Item"), JsString::from("AnotherItem")]);
    let cloned = info.clone_and_replace_type_names(&mut ast, &names);
    let t = cloned.get_type().unwrap();
    assert!(!Arc::ptr_eq(&t, &expr));
    assert_eq!(t.get_root().get_token(&ast), Token::QMARK);
    assert!(!t.get_all_type_names(&ast).contains(&JsString::from("Item")));
    assert!(
        !t.get_all_type_names(&ast)
            .contains(&JsString::from("AnotherItem"))
    );
}
// port: JSDocInfoTest#testJSDocInfoCloneAndReplaceNames_Type_SingleNode
#[test]
fn test_jsdoc_info_clone_and_replace_names_type_single_node() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    let root = ast.new_string("Item");
    let expr = Arc::new(JSTypeExpression::new(root, ""));
    builder.record_type(Some(expr.clone()));
    let info = builder.build().unwrap();
    let names = IndexSet::<_>::from_iter([JsString::from("Item"), JsString::from("AnotherItem")]);
    let cloned = info.clone_and_replace_type_names(&mut ast, &names);
    let t = cloned.get_type().unwrap();
    assert!(!Arc::ptr_eq(&t, &expr));
    assert_eq!(t.get_root().get_token(&ast), Token::QMARK);
    assert!(!t.get_all_type_names(&ast).contains(&JsString::from("Item")));
    assert!(
        !t.get_all_type_names(&ast)
            .contains(&JsString::from("AnotherItem"))
    );
}
// port: JSDocInfoTest#testGetTypeNodes_includesTemplateTypeBounds
#[test]
fn test_get_type_nodes_includes_template_type_bounds() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_template_type_name_with_bound(&mut ast, "A", None);
    let foo = from_string(&mut ast, "Foo");
    builder.record_template_type_name_with_bound(&mut ast, "B", foo);
    let bar = from_string(&mut ast, "Bar");
    builder.record_template_type_name_with_bound(&mut ast, "C", bar);
    let info = builder.build().unwrap();
    let mut roots: Vec<_> = info
        .get_template_types()
        .values()
        .map(|t| t.as_ref().unwrap().get_root())
        .collect();
    let mut nodes = info.get_type_nodes();
    assert_eq!(nodes.len(), 3);
    roots.sort();
    nodes.sort();
    assert_eq!(nodes, roots);
}
// port: JSDocInfoTest#testClosurePrimitiveId_affectsEquality
#[test]
fn test_closure_primitive_id_affects_equality() {
    let ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_closure_primitive_id("asserts.fail");
    let info = builder.build().unwrap();
    assert!(!JSDocInfo::are_equivalent(
        &ast,
        Some(&info),
        Some(&empty())
    ));
    builder = JSDocInfo::builder();
    builder.record_closure_primitive_id("asserts.fail");
    let other = builder.build().unwrap();
    assert!(JSDocInfo::are_equivalent(&ast, Some(&info), Some(&other)));
}
