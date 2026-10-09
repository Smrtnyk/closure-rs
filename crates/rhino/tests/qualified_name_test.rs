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
//   test/com/google/javascript/rhino/QualifiedNameTest.java.

use closure_rhino::{
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
};
// port: QualifiedNameTest#qname
fn qname(ast: &mut Ast, root: NodeId, props: &[&str]) -> NodeId {
    let mut n = root;
    for p in props {
        n = IR::getprop(ast, n, *p);
    }
    n
}

// port: QualifiedNameTest#testJoin_fromString
#[test]
fn test_join_from_string() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo.bar.baz");
    let n = v1;
    let v2 = n.join(&ast);
    assert_eq!(v2, "foo.bar.baz");
}

// port: QualifiedNameTest#testJoin_onSeparator_fromString
#[test]
fn test_join_on_separator_from_string() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo.bar.baz");
    let n = v1;
    let v2 = n.join_with_separator(&ast, '$' as u16);
    assert_eq!(v2, "foo$bar$baz");
}

// port: QualifiedNameTest#testJoin_fromNode
#[test]
fn test_join_from_node() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "foo");
    let v2 = qname(&mut ast, v1, &["bar", "baz"]);
    let v3 = v2.get_qualified_name_object(&ast);
    let mut n = v3;
    let v4 = n.as_ref().unwrap().join(&ast);
    assert_eq!(v4, "foo.bar.baz");
    let v5 = IR::this_node(&mut ast);
    let v6 = qname(&mut ast, v5, &["bar", "baz"]);
    let v7 = v6.get_qualified_name_object(&ast);
    n = v7;
    let v8 = n.as_ref().unwrap().join(&ast);
    assert_eq!(v8, "this.bar.baz");
    let v9 = IR::super_node(&mut ast);
    let v10 = qname(&mut ast, v9, &["bar", "baz"]);
    let v11 = v10.get_qualified_name_object(&ast);
    n = v11;
    let v12 = n.as_ref().unwrap().join(&ast);
    assert_eq!(v12, "super.bar.baz");
}

// port: QualifiedNameTest#testJoin_onSeparator_fromNode
#[test]
fn test_join_on_separator_from_node() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "foo");
    let v2 = qname(&mut ast, v1, &["bar", "baz"]);
    let v3 = v2.get_qualified_name_object(&ast);
    let mut n = v3;
    let v4 = n.as_ref().unwrap().join_with_separator(&ast, '$' as u16);
    assert_eq!(v4, "foo$bar$baz");
    let v5 = IR::this_node(&mut ast);
    let v6 = qname(&mut ast, v5, &["bar", "baz"]);
    let v7 = v6.get_qualified_name_object(&ast);
    n = v7;
    let v8 = n.as_ref().unwrap().join_with_separator(&ast, '$' as u16);
    assert_eq!(v8, "this$bar$baz");
    let v9 = IR::super_node(&mut ast);
    let v10 = qname(&mut ast, v9, &["bar", "baz"]);
    let v11 = v10.get_qualified_name_object(&ast);
    n = v11;
    let v12 = n.as_ref().unwrap().join_with_separator(&ast, '$' as u16);
    assert_eq!(v12, "super$bar$baz");
}

// port: QualifiedNameTest#testJoin_fromGetprop
#[test]
fn test_join_from_getprop() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let v2 = v1.getprop("bar");
    let v3 = v2.getprop("baz");
    let n = v3;
    let v4 = n.join(&ast);
    assert_eq!(v4, "foo.bar.baz");
}

// port: QualifiedNameTest#testJoin_onSeparator_fromGetprop
#[test]
fn test_join_on_separator_from_getprop() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let v2 = v1.getprop("bar");
    let v3 = v2.getprop("baz");
    let n = v3;
    let v4 = n.join_with_separator(&ast, '$' as u16);
    assert_eq!(v4, "foo$bar$baz");
}

// port: QualifiedNameTest#testComponents_fromString
#[test]
fn test_components_from_string() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo.bar.baz");
    let n = v1;
    let v2 = n.components(&ast);
    assert_eq!(
        v2,
        vec![
            JsString::from("foo"),
            JsString::from("bar"),
            JsString::from("baz")
        ]
    );
}

// port: QualifiedNameTest#testComponents_fromNode
#[test]
fn test_components_from_node() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "foo");
    let v2 = qname(&mut ast, v1, &["bar", "baz"]);
    let v3 = v2.get_qualified_name_object(&ast);
    let n = v3;
    let v4 = n.as_ref().unwrap().components(&ast);
    assert_eq!(
        v4,
        vec![
            JsString::from("foo"),
            JsString::from("bar"),
            JsString::from("baz")
        ]
    );
}

// port: QualifiedNameTest#testComponents_fromGetprop
#[test]
fn test_components_from_getprop() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let v2 = v1.getprop("bar");
    let v3 = v2.getprop("baz");
    let n = v3;
    let v4 = n.components(&ast);
    assert_eq!(
        v4,
        vec![
            JsString::from("foo"),
            JsString::from("bar"),
            JsString::from("baz")
        ]
    );
}

// port: QualifiedNameTest#testGetOwner_fromString
#[test]
fn test_get_owner_from_string() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo.bar.baz");
    let v2 = v1.get_owner(&ast);
    let v3 = v2.as_ref().unwrap().join(&ast);
    assert_eq!(v3, "foo.bar");
    let v4 = QualifiedName::of("foo");
    let v5 = v4.get_owner(&ast);
    assert!(v5.is_none());
}

// port: QualifiedNameTest#testGetOwner_fromNode
#[test]
fn test_get_owner_from_node() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "foo");
    let v2 = qname(&mut ast, v1, &["bar", "baz"]);
    let v3 = v2.get_qualified_name_object(&ast);
    let n = v3;
    let v4 = n.as_ref().unwrap().get_owner(&ast);
    let v5 = v4.as_ref().unwrap().join(&ast);
    assert_eq!(v5, "foo.bar");
    let v6 = IR::name(&mut ast, "foo");
    let v7 = v6.get_qualified_name_object(&ast);
    let v8 = v7.as_ref().unwrap().get_owner(&ast);
    assert!(v8.is_none());
    let v9 = IR::this_node(&mut ast);
    let v10 = v9.get_qualified_name_object(&ast);
    let v11 = v10.as_ref().unwrap().get_owner(&ast);
    assert!(v11.is_none());
    let v12 = IR::super_node(&mut ast);
    let v13 = v12.get_qualified_name_object(&ast);
    let v14 = v13.as_ref().unwrap().get_owner(&ast);
    assert!(v14.is_none());
}

// port: QualifiedNameTest#testGetOwner_fromGetprop
#[test]
fn test_get_owner_from_getprop() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let foo = v1;
    let v2 = foo.getprop("bar");
    let foo_bar = v2;
    let v3 = foo_bar.get_owner(&ast);
    assert_eq!(v3, Some(foo));
    let v4 = foo_bar.getprop("baz");
    let v5 = v4.get_owner(&ast);
    assert_eq!(v5, Some(foo_bar));
}

// port: QualifiedNameTest#testGetComponent_fromString
#[test]
fn test_get_component_from_string() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo.bar.baz");
    let v2 = v1.get_component(&ast);
    assert_eq!(v2, "baz");
    let v3 = QualifiedName::of("foo");
    let v4 = v3.get_component(&ast);
    assert_eq!(v4, "foo");
}

// port: QualifiedNameTest#testGetComponent_fromNode
#[test]
fn test_get_component_from_node() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "foo");
    let v2 = qname(&mut ast, v1, &["bar", "baz"]);
    let v3 = v2.get_qualified_name_object(&ast);
    let n = v3;
    let v4 = n.as_ref().unwrap().get_component(&ast);
    assert_eq!(v4, "baz");
    let v5 = IR::name(&mut ast, "foo");
    let v6 = v5.get_qualified_name_object(&ast);
    let v7 = v6.as_ref().unwrap().get_component(&ast);
    assert_eq!(v7, "foo");
    let v8 = IR::this_node(&mut ast);
    let v9 = v8.get_qualified_name_object(&ast);
    let v10 = v9.as_ref().unwrap().get_component(&ast);
    assert_eq!(v10, "this");
    let v11 = IR::super_node(&mut ast);
    let v12 = v11.get_qualified_name_object(&ast);
    let v13 = v12.as_ref().unwrap().get_component(&ast);
    assert_eq!(v13, "super");
}

// port: QualifiedNameTest#testGetComponent_fromGetprop
#[test]
fn test_get_component_from_getprop() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let foo = v1;
    let v2 = foo.getprop("bar");
    let foo_bar = v2;
    let v3 = foo_bar.get_component(&ast);
    assert_eq!(v3, "bar");
    let v4 = foo_bar.getprop("baz");
    let v5 = v4.get_component(&ast);
    assert_eq!(v5, "baz");
}

// port: QualifiedNameTest#testMatch_fromString
#[test]
fn test_match_from_string() {
    let mut ast = Ast::new();
    let v1 = QualifiedName::of("foo.bar.baz");
    let mut n = v1;
    let v2 = IR::name(&mut ast, "foo");
    let v3 = qname(&mut ast, v2, &["bar", "baz"]);
    let v4 = n.matches(&ast, v3);
    assert!(v4);
    let v5 = IR::this_node(&mut ast);
    let v6 = qname(&mut ast, v5, &["bar", "baz"]);
    let v7 = n.matches(&ast, v6);
    assert!(!v7);
    let v8 = IR::super_node(&mut ast);
    let v9 = qname(&mut ast, v8, &["bar", "baz"]);
    let v10 = n.matches(&ast, v9);
    assert!(!v10);
    let v11 = IR::name(&mut ast, "foo");
    let v12 = qname(&mut ast, v11, &["baz", "bar"]);
    let v13 = n.matches(&ast, v12);
    assert!(!v13);
    let v14 = QualifiedName::of("this.qux");
    n = v14;
    let v15 = IR::this_node(&mut ast);
    let v16 = qname(&mut ast, v15, &["qux"]);
    let v17 = n.matches(&ast, v16);
    assert!(v17);
    let v18 = IR::name(&mut ast, "x");
    let v19 = qname(&mut ast, v18, &["qux"]);
    let v20 = n.matches(&ast, v19);
    assert!(!v20);
    let v21 = QualifiedName::of("super.qux");
    n = v21;
    let v22 = IR::super_node(&mut ast);
    let v23 = qname(&mut ast, v22, &["qux"]);
    let v24 = n.matches(&ast, v23);
    assert!(v24);
    let v25 = IR::name(&mut ast, "x");
    let v26 = qname(&mut ast, v25, &["qux"]);
    let v27 = n.matches(&ast, v26);
    assert!(!v27);
}

// port: QualifiedNameTest#testMatch_fromNode
#[test]
fn test_match_from_node() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "foo");
    let v2 = qname(&mut ast, v1, &["bar", "baz"]);
    let v3 = v2.get_qualified_name_object(&ast);
    let mut n = v3;
    let v4 = IR::name(&mut ast, "foo");
    let v5 = qname(&mut ast, v4, &["bar", "baz"]);
    let v6 = n.as_ref().unwrap().matches(&ast, v5);
    assert!(v6);
    let v7 = IR::this_node(&mut ast);
    let v8 = qname(&mut ast, v7, &["bar", "baz"]);
    let v9 = n.as_ref().unwrap().matches(&ast, v8);
    assert!(!v9);
    let v10 = IR::super_node(&mut ast);
    let v11 = qname(&mut ast, v10, &["bar", "baz"]);
    let v12 = n.as_ref().unwrap().matches(&ast, v11);
    assert!(!v12);
    let v13 = IR::name(&mut ast, "foo");
    let v14 = qname(&mut ast, v13, &["baz", "bar"]);
    let v15 = n.as_ref().unwrap().matches(&ast, v14);
    assert!(!v15);
    let v16 = IR::this_node(&mut ast);
    let v17 = qname(&mut ast, v16, &["qux"]);
    let v18 = v17.get_qualified_name_object(&ast);
    n = v18;
    let v19 = IR::this_node(&mut ast);
    let v20 = qname(&mut ast, v19, &["qux"]);
    let v21 = n.as_ref().unwrap().matches(&ast, v20);
    assert!(v21);
    let v22 = IR::name(&mut ast, "x");
    let v23 = qname(&mut ast, v22, &["qux"]);
    let v24 = n.as_ref().unwrap().matches(&ast, v23);
    assert!(!v24);
    let v25 = IR::super_node(&mut ast);
    let v26 = qname(&mut ast, v25, &["qux"]);
    let v27 = v26.get_qualified_name_object(&ast);
    n = v27;
    let v28 = IR::super_node(&mut ast);
    let v29 = qname(&mut ast, v28, &["qux"]);
    let v30 = n.as_ref().unwrap().matches(&ast, v29);
    assert!(v30);
    let v31 = IR::name(&mut ast, "x");
    let v32 = qname(&mut ast, v31, &["qux"]);
    let v33 = n.as_ref().unwrap().matches(&ast, v32);
    assert!(!v33);
}

// port: QualifiedNameTest#testMatch_fromGetprop
#[test]
fn test_match_from_getprop() {
    let mut ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let v2 = v1.getprop("bar");
    let v3 = v2.getprop("baz");
    let mut n = v3;
    let v4 = IR::name(&mut ast, "foo");
    let v5 = qname(&mut ast, v4, &["bar", "baz"]);
    let v6 = n.matches(&ast, v5);
    assert!(v6);
    let v7 = IR::this_node(&mut ast);
    let v8 = qname(&mut ast, v7, &["bar", "baz"]);
    let v9 = n.matches(&ast, v8);
    assert!(!v9);
    let v10 = IR::super_node(&mut ast);
    let v11 = qname(&mut ast, v10, &["bar", "baz"]);
    let v12 = n.matches(&ast, v11);
    assert!(!v12);
    let v13 = IR::name(&mut ast, "foo");
    let v14 = qname(&mut ast, v13, &["baz", "bar"]);
    let v15 = n.matches(&ast, v14);
    assert!(!v15);
    let v16 = QualifiedName::of("this");
    let v17 = v16.getprop("qux");
    n = v17;
    let v18 = IR::this_node(&mut ast);
    let v19 = qname(&mut ast, v18, &["qux"]);
    let v20 = n.matches(&ast, v19);
    assert!(v20);
    let v21 = IR::name(&mut ast, "x");
    let v22 = qname(&mut ast, v21, &["qux"]);
    let v23 = n.matches(&ast, v22);
    assert!(!v23);
    let v24 = QualifiedName::of("super");
    let v25 = v24.getprop("qux");
    n = v25;
    let v26 = IR::super_node(&mut ast);
    let v27 = qname(&mut ast, v26, &["qux"]);
    let v28 = n.matches(&ast, v27);
    assert!(v28);
    let v29 = IR::name(&mut ast, "x");
    let v30 = qname(&mut ast, v29, &["qux"]);
    let v31 = n.matches(&ast, v30);
    assert!(!v31);
}

// port: QualifiedNameTest#testEdgeCases_fromString
#[test]
fn test_edge_cases_from_string() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("");
    let v2 = v1.components(&ast);
    assert_contains_exactly(v2, vec![JsString::from("")]);
    let v3 = QualifiedName::of(".");
    let v4 = v3.components(&ast);
    assert_contains_exactly(v4, vec![JsString::from(""), JsString::from("")]);
    let v5 = QualifiedName::of(".foo.bar");
    let v6 = v5.components(&ast);
    assert_contains_exactly(
        v6,
        vec![
            JsString::from(""),
            JsString::from("foo"),
            JsString::from("bar"),
        ],
    );
    let v7 = QualifiedName::of("foo.bar.");
    let v8 = v7.components(&ast);
    assert_contains_exactly(
        v8,
        vec![
            JsString::from("foo"),
            JsString::from("bar"),
            JsString::from(""),
        ],
    );
    let v9 = QualifiedName::of("foo..bar");
    let v10 = v9.components(&ast);
    assert_contains_exactly(
        v10,
        vec![
            JsString::from("foo"),
            JsString::from(""),
            JsString::from("bar"),
        ],
    );
}

// port: QualifiedNameTest#testGetComponentCount_fromString
#[test]
fn test_get_component_count_from_string() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let v2 = v1.get_component_count(&ast);
    assert_eq!(v2, 1);
    let v3 = QualifiedName::of("foo.bar");
    let v4 = v3.get_component_count(&ast);
    assert_eq!(v4, 2);
    let v5 = QualifiedName::of("foo.bar.baz");
    let v6 = v5.get_component_count(&ast);
    assert_eq!(v6, 3);
}

// port: QualifiedNameTest#testGetComponentCount_fromNode
#[test]
fn test_get_component_count_from_node() {
    let mut ast = Ast::new();
    let v1 = IR::name(&mut ast, "foo");
    let v2 = v1.get_qualified_name_object(&ast);
    let v3 = v2.as_ref().unwrap().get_component_count(&ast);
    assert_eq!(v3, 1);
    let v4 = IR::name(&mut ast, "foo");
    let v5 = qname(&mut ast, v4, &["bar"]);
    let v6 = v5.get_qualified_name_object(&ast);
    let v7 = v6.as_ref().unwrap().get_component_count(&ast);
    assert_eq!(v7, 2);
    let v8 = IR::name(&mut ast, "foo");
    let v9 = qname(&mut ast, v8, &["bar", "baz"]);
    let v10 = v9.get_qualified_name_object(&ast);
    let v11 = v10.as_ref().unwrap().get_component_count(&ast);
    assert_eq!(v11, 3);
}

// port: QualifiedNameTest#testGetComponentCount_fromGetprop
#[test]
fn test_get_component_count_from_getprop() {
    let ast = Ast::new();
    let v1 = QualifiedName::of("foo");
    let v2 = v1.getprop("bar");
    let v3 = v2.get_component_count(&ast);
    assert_eq!(v3, 2);
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
