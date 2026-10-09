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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/NodeUtilTest.java.

use super::*;

// port: NodeUtilTest.ParseHelper#parse
pub(super) fn parse(compiler: &mut Compiler, js: impl Into<JsString>) -> NodeId {
    let mut options = CompilerOptions::new();
    options.set_strict_mode_input(false);
    options.set_warning_level(
        Arc::clone(&closure_jscomp::diagnostic_groups::ES5_STRICT),
        closure_jscomp::check_level::CheckLevel::OFF,
    );
    options.set_language_in(closure_jscomp::compiler_options::LanguageMode::UNSUPPORTED);
    compiler.init_options(options);
    let n = compiler.parse_test_code(js);
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
    n
}
// port: NodeUtilTest.ParseHelper#parseFirst
pub(super) fn parse_first(
    compiler: &mut Compiler,
    token: Token,
    js: impl Into<JsString>,
) -> NodeId {
    let root = parse(compiler, js);
    assert!(root.is_script(compiler));
    if token == Token::SCRIPT {
        root
    } else {
        get_node(compiler, root, token)
    }
}
// port: NodeUtilTest.ParseHelper#parseSecond
pub(super) fn parse_second(
    compiler: &mut Compiler,
    token: Token,
    js: impl Into<JsString>,
) -> NodeId {
    let root = parse_first(compiler, token, js);
    get_node(compiler, root, token)
}
// port: NodeUtilTest#parseExpr
pub(super) fn parse_expr(compiler: &mut Compiler, js: impl Into<JsString>) -> NodeId {
    let js = JsString::from("(")
        .concat(&js.into())
        .concat(&JsString::from(");"));
    parse(compiler, js).get_first_first_child(compiler).unwrap()
}
// port: NodeUtilTest#getNode
pub(super) fn get_node(compiler: &Compiler, root: NodeId, token: Token) -> NodeId {
    get_node_or_null(compiler, root, token).unwrap_or_else(|| {
        panic!(
            "No {token} node found in:\n {}",
            root.to_string_tree(compiler)
        )
    })
}
// port: NodeUtilTest#getNodeOrNull
pub(super) fn get_node_or_null(compiler: &Compiler, root: NodeId, token: Token) -> Option<NodeId> {
    for n in root.children(compiler) {
        if n.get_token(compiler) == token {
            return Some(n);
        }
        if let Some(found) = get_node_or_null(compiler, n, token) {
            return Some(found);
        }
    }
    None
}
// port: NodeUtilTest#getStringNode
pub(super) fn get_string_node(
    compiler: &Compiler,
    n: NodeId,
    name: &str,
    node_type: Token,
) -> Option<NodeId> {
    if n.get_token(compiler) == node_type && n.get_string(compiler) == name {
        return Some(n);
    }
    for c in n.children(compiler) {
        if let Some(found) = get_string_node(compiler, c, name, node_type) {
            return Some(found);
        }
    }
    None
}
// port: NodeUtilTest#getNameNode
pub(super) fn get_name_node(compiler: &Compiler, n: NodeId, name: &str) -> NodeId {
    get_string_node(compiler, n, name, Token::NAME).unwrap()
}
// port: NodeUtilTest#getNameNodeFrom
pub(super) fn get_name_node_from(compiler: &mut Compiler, code: &str, name: &str) -> NodeId {
    let ast = parse(compiler, code);
    get_name_node(compiler, ast, name)
}
// port: NodeUtilTest#getStringKeyNodeFrom
pub(super) fn get_string_key_node_from(compiler: &mut Compiler, code: &str, name: &str) -> NodeId {
    let ast = parse(compiler, code);
    get_string_node(compiler, ast, name, Token::STRING_KEY).unwrap()
}
// port: NodeUtilTest#getStringLitNodeFrom
pub(super) fn get_string_lit_node_from(compiler: &mut Compiler, code: &str, name: &str) -> NodeId {
    let ast = parse(compiler, code);
    get_string_node(compiler, ast, name, Token::STRINGLIT).unwrap()
}
// port: NodeUtilTest#getPattern
pub(super) fn get_pattern(compiler: &Compiler, tree: NodeId) -> NodeId {
    fn find(compiler: &Compiler, tree: NodeId) -> Option<NodeId> {
        if tree.is_destructuring_pattern(compiler) {
            return Some(tree);
        }
        for c in tree.children(compiler) {
            if let Some(result) = find(compiler, c) {
                return Some(result);
            }
        }
        None
    }
    find(compiler, tree).unwrap()
}
// port: NodeUtilTest.AssortedTests#isLiteralValue
pub(super) fn is_literal_value(compiler: &mut Compiler, code: &str) -> bool {
    let n = parse_expr(compiler, code);
    NodeUtil::is_literal_value(compiler, n, true)
}
// port: NodeUtilTest.AssortedTests#isLiteralValueExcludingFunctions
pub(super) fn is_literal_value_excluding_functions(compiler: &mut Compiler, code: &str) -> bool {
    let n = parse_expr(compiler, code);
    NodeUtil::is_literal_value(compiler, n, false)
}
// port: NodeUtilTest.AssortedTests#assertLiteralAndImmutable
pub(super) fn assert_literal_and_immutable(compiler: &Compiler, n: NodeId) {
    assert!(NodeUtil::is_literal_value(compiler, n, true));
    assert!(NodeUtil::is_literal_value(compiler, n, false));
    assert!(NodeUtil::is_immutable_value(compiler, n));
}
// port: NodeUtilTest.AssortedTests#assertLiteralButNotImmutable
pub(super) fn assert_literal_but_not_immutable(compiler: &Compiler, n: NodeId) {
    assert!(NodeUtil::is_literal_value(compiler, n, true));
    assert!(NodeUtil::is_literal_value(compiler, n, false));
    assert!(!NodeUtil::is_immutable_value(compiler, n));
}
// port: NodeUtilTest.AssortedTests#assertNotLiteral
pub(super) fn assert_not_literal(compiler: &Compiler, n: NodeId) {
    assert!(!NodeUtil::is_literal_value(compiler, n, true));
    assert!(!NodeUtil::is_literal_value(compiler, n, false));
    assert!(!NodeUtil::is_immutable_value(compiler, n));
}
// port: NodeUtilTest.AssortedTests#assertMayBeObjectLitKey
pub(super) fn assert_may_be_object_lit_key(compiler: &Compiler, n: NodeId, expected: bool) {
    assert_eq!(NodeUtil::may_be_object_lit_key(compiler, n), expected);
}
// port: NodeUtilTest.AssortedTests#assertIsObjectLitKey
pub(super) fn assert_is_object_lit_key(compiler: &Compiler, n: NodeId, expected: bool) {
    assert_eq!(NodeUtil::is_object_lit_key(compiler, n), expected);
}
// port: NodeUtilTest.AssortedTests#assertGetNameResult
pub(super) fn assert_get_name_result(compiler: &Compiler, function: NodeId, name: &str) {
    assert!(function.is_function(compiler));
    assert_eq!(NodeUtil::get_name(compiler, function).unwrap(), name);
}
// port: NodeUtilTest.AssortedTests#findParentOfFuncOrClassDescendant
fn find_parent_of_func_or_class_descendant(
    compiler: &Compiler,
    n: NodeId,
    token: Token,
) -> Option<NodeId> {
    assert!(matches!(token, Token::CLASS | Token::FUNCTION));
    for c in n.children(compiler) {
        if c.get_token(compiler) == token {
            return Some(n);
        }
        if let Some(found) = find_parent_of_func_or_class_descendant(compiler, c, token) {
            return Some(found);
        }
    }
    None
}
// port: NodeUtilTest.AssortedTests#getFuncOrClassChild
fn get_func_or_class_child(compiler: &Compiler, n: NodeId, token: Token) -> NodeId {
    assert!(matches!(token, Token::CLASS | Token::FUNCTION));
    n.children(compiler)
        .find(|c| c.get_token(compiler) == token)
        .unwrap()
}
// port: NodeUtilTest.AssortedTests#assertContainsAnonFunc
pub(super) fn assert_contains_anon_func(compiler: &mut Compiler, expected: bool, js: &str) {
    let root = parse(compiler, js);
    let parent = find_parent_of_func_or_class_descendant(compiler, root, Token::FUNCTION).unwrap();
    let node = get_func_or_class_child(compiler, parent, Token::FUNCTION);
    assert_eq!(
        NodeUtil::is_function_expression(compiler, node),
        expected,
        "{js}"
    );
}
// port: NodeUtilTest.AssortedTests#assertContainsAnonClass
pub(super) fn assert_contains_anon_class(compiler: &mut Compiler, expected: bool, js: &str) {
    let root = parse(compiler, js);
    let parent = find_parent_of_func_or_class_descendant(compiler, root, Token::CLASS).unwrap();
    let node = get_func_or_class_child(compiler, parent, Token::CLASS);
    assert_eq!(
        NodeUtil::is_class_expression(compiler, node),
        expected,
        "{js}"
    );
}
// port: NodeUtilTest.AssortedTests#assertNodeNames
pub(super) fn assert_node_names(compiler: &Compiler, expected: Vec<&str>, nodes: Vec<NodeId>) {
    let actual: closure_rhino::fx_hash::IndexSet<_> =
        nodes.into_iter().map(|n| n.get_string(compiler)).collect();
    let expected: closure_rhino::fx_hash::IndexSet<_> =
        expected.into_iter().map(JsString::from).collect();
    assert_eq!(actual, expected);
}
// port: NodeUtilTest.AssortedTests#replaceDeclChild
pub(super) fn replace_decl_child(
    compiler: &mut Compiler,
    js: &str,
    declaration_child: i32,
    expected: &str,
) {
    let actual = parse(compiler, js);
    let name_node = actual
        .get_first_child(compiler)
        .unwrap()
        .get_child_at_index(compiler, declaration_child)
        .unwrap();
    let block = IR::block(compiler);
    NodeUtil::replace_declaration_child(compiler, name_node, block);
    let expected = parse(compiler, expected);
    assert!(expected.is_equivalent_to(compiler, actual));
}
// port: NodeUtilTest#executedOnceTestCase
pub(super) fn executed_once_test_case(compiler: &mut Compiler, js: &str) -> bool {
    let n = get_name_node_from(compiler, js, "x");
    NodeUtil::is_executed_exactly_once(compiler, n)
}
// port: NodeUtilTest.AssortedTests#assertLValueNamedX
pub(super) fn assert_l_value_named_x(compiler: &Compiler, n: NodeId) {
    assert_eq!(n.get_string(compiler), "x");
    assert!(NodeUtil::is_l_value(compiler, n));
}
// port: NodeUtilTest.AssortedTests#assertNotLValueNamedX
pub(super) fn assert_not_l_value_named_x(compiler: &Compiler, n: NodeId) {
    assert_eq!(n.get_string(compiler), "x");
    assert!(!NodeUtil::is_l_value(compiler, n));
}
// port: NodeUtilTest.AssortedTests#assertLhsByDestructuring
pub(super) fn assert_lhs_by_destructuring(compiler: &Compiler, n: NodeId) {
    assert!(NodeUtil::is_lhs_by_destructuring(compiler, n));
}
// port: NodeUtilTest.AssortedTests#assertNotLhsByDestructuring
pub(super) fn assert_not_lhs_by_destructuring(compiler: &Compiler, n: NodeId) {
    assert!(!NodeUtil::is_lhs_by_destructuring(compiler, n));
}
// port: NodeUtilTest#getFunctionLValue
pub(super) fn get_function_l_value(compiler: &mut Compiler, js: &str) -> Option<JsString> {
    let n = parse_first(compiler, Token::FUNCTION, js);
    NodeUtil::get_best_l_value(compiler, n).map(|n| n.get_string(compiler))
}
// port: NodeUtilTest#functionIsRValueOfAssign
pub(super) fn function_is_r_value_of_assign(compiler: &mut Compiler, js: &str) -> bool {
    let ast = parse(compiler, js);
    let name_node = get_name_node(compiler, ast, "x");
    let func_node = get_node(compiler, ast, Token::FUNCTION);
    Some(func_node) == NodeUtil::get_r_value_of_l_value(compiler, name_node)
}
// port: NodeUtilTest#testFunctionName
pub(super) fn test_function_name(compiler: &mut Compiler, js: &str, expected: Option<&str>) {
    let n = parse_first(compiler, Token::FUNCTION, js);
    assert_eq!(
        NodeUtil::get_nearest_function_name(compiler, n),
        expected.map(JsString::from)
    );
}
// port: NodeUtilTest#visitLhsNodesInNode
pub(super) fn visit_lhs_nodes_in_node(compiler: &mut Compiler, js: &str) -> Vec<NodeId> {
    let root = parse(compiler, js);
    assert!(root.is_script(compiler));
    let mut root = root.get_only_child(compiler);
    if root.is_expr_result(compiler) {
        root = root.get_only_child(compiler);
        assert!(NodeUtil::is_assignment_op(compiler, root));
    }
    let mut nodes = Vec::new();
    NodeUtil::visit_lhs_nodes_in_node(compiler, root, &mut |_, n| nodes.push(n));
    nodes
}
// port: NodeUtilTest.AssortedTests#assertIsConstantDeclaration
pub(super) fn assert_is_constant_declaration(compiler: &Compiler, expected: bool, n: NodeId) {
    let info = NodeUtil::get_best_jsdoc_info(compiler, n);
    assert_eq!(
        NodeUtil::is_constant_declaration(compiler, info.as_deref(), n),
        expected
    );
}
// port: NodeUtilTest.AssortedTests#constructInferredConstantDeclaration
pub(super) fn construct_inferred_constant_declaration(compiler: &mut Compiler) -> NodeId {
    let name = IR::name(compiler, "foo");
    name.set_inferred_constant_var(compiler, true);
    let number = IR::number(compiler, 1.0);
    let declaration = IR::let_with_value(compiler, name, number);
    IR::script_with_children(compiler, &[declaration]);
    name
}
pub(super) fn assert_contents(actual: Vec<JsString>, expected: Vec<JsString>) {
    assert_eq!(actual.len(), expected.len());
    let mut actual = actual;
    let mut expected = expected;
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
