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
 *   Bob Jervis
 *   Google Inc.
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
 *   Norris Boyd
 *   Roger Lawrence
 *   Mike McCabe
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
//   src/com/google/javascript/rhino/Node.java,
//   src/com/google/javascript/rhino/testing/NodeSubject.java.

//! Node assertions shared by Rhino and compiler tests.
use crate::node::{
    Ast, JsDocComparison, NodeId, RecursionMode, SideEffectComparison, TypeComparison,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeMismatch {
    pub actual: NodeId,
    pub expected: NodeId,
}

// port: NodeSubject#findFirstMismatch
pub fn find_first_mismatch(
    ast: &Ast,
    actual: NodeId,
    expected: NodeId,
    js_doc: bool,
) -> Option<NodeMismatch> {
    find_first_mismatch_with_options(ast, actual, expected, js_doc, false)
}

// port: NodeSubject#findFirstMismatch (Node comparison flags)
pub fn find_first_mismatch_with_options(
    ast: &Ast,
    actual: NodeId,
    expected: NodeId,
    js_doc: bool,
    side_effect: bool,
) -> Option<NodeMismatch> {
    find_first_mismatch_across_with_options(ast, actual, ast, expected, js_doc, side_effect)
}
// port: NodeSubject#findFirstMismatch (separate owning arenas)
pub fn find_first_mismatch_across(
    ast: &Ast,
    actual: NodeId,
    expected_ast: &Ast,
    expected: NodeId,
    js_doc: bool,
) -> Option<NodeMismatch> {
    find_first_mismatch_across_with_options(ast, actual, expected_ast, expected, js_doc, false)
}
// port: NodeSubject#findFirstMismatch (separate owning arenas and comparison flags)
pub fn find_first_mismatch_across_with_options(
    ast: &Ast,
    actual: NodeId,
    expected_ast: &Ast,
    expected: NodeId,
    js_doc: bool,
    side_effect: bool,
) -> Option<NodeMismatch> {
    if !actual.is_equivalent_to_across_with_options(
        ast,
        expected_ast,
        expected,
        RecursionMode::SHALLOW,
        TypeComparison::IGNORE,
        if js_doc {
            JsDocComparison::COMPARE
        } else {
            JsDocComparison::IGNORE
        },
        if side_effect {
            SideEffectComparison::COMPARE
        } else {
            SideEffectComparison::IGNORE
        },
    ) {
        return Some(NodeMismatch { actual, expected });
    }
    for (a, e) in actual.children(ast).zip(expected.children(expected_ast)) {
        if let Some(mismatch) =
            find_first_mismatch_across_with_options(ast, a, expected_ast, e, js_doc, side_effect)
        {
            return Some(mismatch);
        }
    }
    if let (Some(a), Some(e)) = (
        actual.get_closure_unaware_shadow(ast),
        expected.get_closure_unaware_shadow(expected_ast),
    ) {
        return find_first_mismatch_across_with_options(
            ast,
            a,
            expected_ast,
            e,
            js_doc,
            side_effect,
        );
    }
    None
}
type Serializer = Box<dyn Fn(&Ast, NodeId) -> String>;
pub struct NodeSubject {
    actual: NodeId,
    serializer: Option<Serializer>,
    generic_name_replacements: indexmap::IndexMap<String, String>,
}
// port: NodeSubject#assertNode
pub fn assert_node(node: NodeId) -> NodeSubject {
    NodeSubject::new(node)
}
impl NodeSubject {
    // port: NodeSubject#NodeSubject
    fn new(node: NodeId) -> Self {
        Self {
            actual: node,
            serializer: None,
            generic_name_replacements: indexmap::IndexMap::new(),
        }
    }
    // port: NodeSubject#isEqualTo(Node)
    pub fn is_equal_to(&self, ast: &Ast, expected: NodeId) -> &Self {
        self.is_equal_to_internal(ast, expected, false)
    }
    // port: NodeSubject#isEqualIncludingJsDocTo
    pub fn is_equal_including_js_doc_to(&self, ast: &Ast, expected: NodeId) -> &Self {
        self.is_equal_to_internal(ast, expected, true)
    }
    // port: Subject#isNotEqualTo (NodeSubject inherited assertion)
    pub fn is_not_equal_to(&self, _ast: &Ast, expected: NodeId) -> &Self {
        assert_ne!(self.actual, expected, "expected not to be equal to");
        self
    }
    // port: NodeSubject#isEqualToInternal
    pub fn is_equal_to_internal(&self, ast: &Ast, expected: NodeId, check_jsdoc: bool) -> &Self {
        if let Err(message) = self.check_equal_to(ast, expected, check_jsdoc) {
            panic!("{message}");
        }
        self
    }
    // port: NodeSubject#isEqualToInternal (Result assertion for harness callers)
    pub fn check_equal_to(
        &self,
        ast: &Ast,
        expected: NodeId,
        check_jsdoc: bool,
    ) -> Result<(), String> {
        self.check_equal_to_across(ast, ast, expected, check_jsdoc)
    }
    // port: NodeSubject#isEqualToInternal (separate owning arenas)
    pub fn check_equal_to_across(
        &self,
        ast: &Ast,
        expected_ast: &Ast,
        expected: NodeId,
        check_jsdoc: bool,
    ) -> Result<(), String> {
        if find_first_mismatch_across(ast, self.actual, expected_ast, expected, check_jsdoc)
            .is_none()
        {
            return Ok(());
        }
        let mut message = String::from("Node tree inequality");
        let pairs = self.maybe_extract_scripts_separately(ast, expected_ast, expected, self.actual);
        for (index, (expected, actual)) in pairs.iter().copied().enumerate() {
            let Some(mismatch) =
                find_first_mismatch_across(ast, actual, expected_ast, expected, check_jsdoc)
            else {
                continue;
            };
            let prefix = maybe_get_error_prefix(ast, pairs.len(), index, actual);
            let expected_js = self.serialize_node(expected_ast, expected);
            let actual_js = self.serialize_node(ast, actual);
            if expected_js == actual_js {
                message.push_str(&format!(
                    "\n{prefix}AST diff\nexpected:\n{}\nactual:\n{}",
                    mismatch.expected.to_string_tree(expected_ast),
                    mismatch.actual.to_string_tree(ast)
                ));
            } else if std::env::var_os("NodeSubject.PARSEABLE_ACTUAL").is_some() {
                message.push_str(&format!(
                    "\n{prefix}Actual JS (parseable): <<<{}>>>",
                    self.apply_generic_names(&actual_js)
                ));
            } else if expected_js.trim_matches(|c: char| c <= ' ').is_empty() {
                message.push_str(&format!(
                    "\n{prefix}Empty expected JS. Actual JS is:\n    {}",
                    self.apply_generic_names(&actual_js).replace('\n', "\n    ")
                ));
            } else {
                let mut expected_normalized = self.apply_generic_names(&expected_js);
                let mut actual_normalized = self.apply_generic_names(&actual_js);
                if expected_normalized == actual_normalized {
                    expected_normalized = expected_js;
                    actual_normalized = actual_js;
                }
                message.push_str(&format!("\n{prefix}JS diff\nexpected:\n{expected_normalized}\nactual:\n{actual_normalized}"));
            }
            if check_jsdoc {
                let expected_doc = jsdoc_to_string_nullsafe(expected_ast, mismatch.expected);
                let actual_doc = jsdoc_to_string_nullsafe(ast, mismatch.actual);
                if expected_doc != actual_doc {
                    message.push_str(&format!(
                        "\n{prefix}JSDoc diff\nexpected:\n{expected_doc}\nactual:\n{actual_doc}"
                    ));
                }
            }
        }
        Err(message)
    }
    // port: NodeSubject#usingSerializer
    pub fn using_serializer(
        mut self,
        serializer: impl Fn(&Ast, NodeId) -> String + 'static,
    ) -> Self {
        self.serializer = Some(Box::new(serializer));
        self
    }
    // port: NodeSubject#withGenericNameReplacements
    pub fn with_generic_name_replacements(
        mut self,
        replacements: indexmap::IndexMap<String, String>,
    ) -> Self {
        self.generic_name_replacements = replacements;
        self
    }
    // port: NodeSubject#applyGenericNames
    fn apply_generic_names(&self, input: &str) -> String {
        let mut input = input.to_string();
        for (from, to) in &self.generic_name_replacements {
            input = input.replace(from, to);
        }
        input
    }
    // port: NodeSubject#serializeNode
    fn serialize_node(&self, ast: &Ast, node: NodeId) -> String {
        self.serializer.as_ref().map_or_else(
            || node.to_string_tree(ast),
            |s| normalize_serialized(&s(ast, node)),
        )
    }
    // port: NodeSubject#maybeExtractScriptsSeparately
    fn maybe_extract_scripts_separately(
        &self,
        ast: &Ast,
        expected_ast: &Ast,
        expected: NodeId,
        actual: NodeId,
    ) -> Vec<(NodeId, NodeId)> {
        if is_script_source_root(expected_ast, expected)
            && is_script_source_root(ast, actual)
            && expected.get_child_count(expected_ast) == actual.get_child_count(ast)
        {
            expected
                .children(expected_ast)
                .zip(actual.children(ast))
                .collect()
        } else {
            vec![(expected, actual)]
        }
    }
    // port: NodeSubject#isEquivalentTo
    pub fn is_equivalent_to(&self, ast: &Ast, other: NodeId) -> &Self {
        assert!(
            self.actual.is_equivalent_to(ast, other),
            "isEquivalentTo({})",
            other.to_string(ast)
        );
        self
    }
    // port: NodeSubject#isNotEquivalentTo
    pub fn is_not_equivalent_to(&self, ast: &Ast, other: NodeId) -> &Self {
        assert!(
            !self.actual.is_equivalent_to(ast, other),
            "isEquivalentTo({})",
            other.to_string(ast)
        );
        self
    }
    // port: NodeSubject#hasTrailingComma
    pub fn has_trailing_comma(&self, ast: &Ast) -> &Self {
        assert!(self.actual.has_trailing_comma(ast));
        self
    }
    // port: NodeSubject#hasLineno
    pub fn has_lineno(&self, ast: &Ast, lineno: i32) -> &Self {
        assert_eq!(self.actual.get_lineno(ast), lineno);
        self
    }
}

// port: NodeSubject#isScriptSourceRoot
fn is_script_source_root(ast: &Ast, node: NodeId) -> bool {
    node.is_root(ast)
        && node
            .get_parent(ast)
            .is_some_and(|p| p.is_root(ast) && p.get_last_child(ast) == Some(node))
}
// port: NodeSubject#maybeGetErrorPrefix
fn maybe_get_error_prefix(ast: &Ast, count: usize, index: usize, actual: NodeId) -> String {
    if count <= 1 {
        String::new()
    } else if let Some(name) = actual.get_source_file_name(ast).filter(|s| !s.is_empty()) {
        format!("For expected {index}, script \"{name}\": ")
    } else {
        format!("For expected {index}: ")
    }
}
// port: NodeSubject#jsdocToStringNullsafe
fn jsdoc_to_string_nullsafe(ast: &Ast, node: NodeId) -> String {
    node.get_jsdoc_info(ast)
        .map_or_else(|| "(null)".into(), |d| d.to_string_verbose(ast))
}
// port: NodeSubject#serializeNode (Java replaceAlls)
fn normalize_serialized(input: &str) -> String {
    let b = input.as_bytes();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'{' && b.get(i + 1) == Some(&b'\n') {
            let mut j = i + 2;
            while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
                j += 1;
            }
            if b.get(j) == Some(&b'}') {
                blocks.extend_from_slice(b"{}");
                i = j + 1;
                continue;
            }
        }
        blocks.push(b[i]);
        i += 1;
    }
    let mut out = Vec::new();
    for c in blocks {
        if c == b'\n' {
            while out.last() == Some(&b' ') {
                out.pop();
            }
        }
        out.push(c);
    }
    while out.last() == Some(&b' ') {
        out.pop();
    }
    String::from_utf8(out).expect("ASCII replacements keep UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ir::IR, jsdoc_info::JSDocInfo, node::Prop, token::Token};
    // port: NodeSubject#findFirstMismatch (HARNESS.md ast properties)
    #[test]
    fn find_first_mismatch_compares_only_semantic_properties() {
        let mut ast = Ast::new();
        let compared = [
            Prop::ARROW_FN,
            Prop::ASYNC_FN,
            Prop::GENERATOR_FN,
            Prop::START_OF_OPT_CHAIN,
            Prop::STATIC_MEMBER,
            Prop::YIELD_ALL,
            Prop::EXPORT_DEFAULT,
            Prop::EXPORT_ALL_FROM,
            Prop::INCRDECR,
            Prop::QUOTED,
            Prop::FREE_CALL,
            Prop::DIRECT_EVAL,
            Prop::COMPUTED_PROP_METHOD,
            Prop::COMPUTED_PROP_GETTER,
            Prop::COMPUTED_PROP_SETTER,
        ];
        for prop in
            compared
                .into_iter()
                .chain([Prop::IS_PARENTHESIZED, Prop::SYNTHETIC, Prop::USE_STRICT])
        {
            let a = ast.new_node(Token::FUNCTION);
            let e = ast.new_node(Token::FUNCTION);
            a.put_boolean_prop(&mut ast, prop, true);
            assert_eq!(
                find_first_mismatch(&ast, a, e, false).is_some(),
                compared.contains(&prop),
                "{prop:?}"
            );
        }
    }
    // port: NodeSubject#findFirstMismatch (pre-order, count, strings, numbers and shadows)
    #[test]
    fn find_first_mismatch_walks_children_and_shadow_roots() {
        let mut ast = Ast::new();
        let a = IR::name(&mut ast, "a");
        let b = IR::name(&mut ast, "b");
        let r = ast.new_node_with_child(Token::ROOT, a);
        let s = ast.new_node_with_child(Token::ROOT, b);
        assert_eq!(
            find_first_mismatch(&ast, r, s, false),
            Some(NodeMismatch {
                actual: a,
                expected: b
            })
        );
        let extra = IR::name(&mut ast, "a");
        s.add_child_to_back(&mut ast, extra);
        assert_eq!(
            find_first_mismatch(&ast, r, s, false),
            Some(NodeMismatch {
                actual: r,
                expected: s
            })
        );
        let z = ast.new_number(0.0);
        let nz = ast.new_number(0.0);
        assert_node(z).is_equal_to(&ast, nz);
        let one = ast.new_number(1.0);
        assert!(find_first_mismatch(&ast, one, z, false).is_some());
        let a = IR::name(&mut ast, "x");
        let b = IR::name(&mut ast, "x");
        let x = IR::name(&mut ast, "shadowA");
        let y = IR::name(&mut ast, "shadowB");
        a.set_closure_unaware_shadow(&mut ast, Some(x));
        b.set_closure_unaware_shadow(&mut ast, Some(y));
        assert_eq!(
            find_first_mismatch(&ast, a, b, false),
            Some(NodeMismatch {
                actual: x,
                expected: y
            })
        );
        let a = ast.new_template_lit_string(Some("c".into()), "r");
        let b = ast.new_template_lit_string(Some("c".into()), "other");
        assert!(find_first_mismatch(&ast, a, b, false).is_some());
    }
    // port: NodeSubject#isEqualIncludingJsDocTo
    #[test]
    fn jsdoc_and_side_effect_flags_are_optional() {
        let mut ast = Ast::new();
        let a = IR::name(&mut ast, "x");
        let b = IR::name(&mut ast, "x");
        let mut builder = JSDocInfo::builder();
        builder.record_constancy();
        a.set_jsdoc_info(&mut ast, builder.build());
        assert_node(a).is_equal_to(&ast, b);
        assert!(
            assert_node(a)
                .check_equal_to(&ast, b, true)
                .unwrap_err()
                .starts_with("Node tree inequality")
        );
        let a = IR::name(&mut ast, "x");
        a.set_unused_parameter(&mut ast, true);
        assert_node(a).is_equal_to(&ast, b);
        assert!(find_first_mismatch_with_options(&ast, a, b, false, true).is_some());
    }
    // port: NodeSubject#findFirstMismatch (subclass payload and declared type expression)
    #[test]
    fn payloads_declared_types_and_shadow_presence_are_compared() {
        let mut ast = Ast::new();
        let a = IR::name(&mut ast, "x");
        let b = IR::name(&mut ast, "x");
        a.set_length(&mut ast, 99);
        a.set_lineno_charno(&mut ast, 7, 2);
        a.set_original_name(&mut ast, Some("before".into()));
        assert_node(a).is_equal_to(&ast, b).is_not_equal_to(&ast, b);
        let expression = IR::name(&mut ast, "Foo");
        a.set_declared_type_expression(&mut ast, Some(expression));
        assert!(find_first_mismatch(&ast, a, b, false).is_some());
        let expression = IR::name(&mut ast, "Foo");
        b.set_declared_type_expression(&mut ast, Some(expression));
        assert_node(a).is_equal_to(&ast, b);
        let shadow = IR::name(&mut ast, "shadow");
        a.set_closure_unaware_shadow(&mut ast, Some(shadow));
        assert!(find_first_mismatch(&ast, a, b, false).is_some());
        let a = ast.new_big_int(num_bigint::BigInt::from(123));
        let b = ast.new_big_int(num_bigint::BigInt::from(124));
        assert!(find_first_mismatch(&ast, a, b, false).is_some());
        let a = ast.new_template_lit_string(None, "raw");
        let b = ast.new_template_lit_string(Some("cooked".into()), "raw");
        assert!(find_first_mismatch(&ast, a, b, false).is_some());
        let a = IR::name(&mut ast, "x");
        let b = IR::name(&mut ast, "x");
        let mut builder = JSDocInfo::builder();
        builder.record_constancy();
        a.set_jsdoc_info(&mut ast, builder.build());
        let mut builder = JSDocInfo::builder();
        builder.record_constancy();
        b.set_jsdoc_info(&mut ast, builder.build());
        assert_node(a).is_equal_including_js_doc_to(&ast, b);
    }
    // port: NodeSubject#isEqualToInternal / serializeNode / maybeExtractScriptsSeparately
    #[test]
    fn failure_facts_use_serialization_and_script_prefixes() {
        assert_eq!(normalize_serialized("{\n \t}  \nx  "), "{}\nx");
        let mut ast = Ast::new();
        let a = IR::name(&mut ast, "generated");
        let b = IR::name(&mut ast, "expected");
        let message = assert_node(a)
            .using_serializer(|_, _| "same".into())
            .check_equal_to(&ast, b, false)
            .unwrap_err();
        assert!(message.contains("Node tree inequality\nAST diff"));
        let message = assert_node(a)
            .using_serializer(move |_, n| {
                if n == b {
                    "".into()
                } else {
                    "generated\n".into()
                }
            })
            .with_generic_name_replacements(indexmap::IndexMap::from([(
                "generated".into(),
                "short".into(),
            )]))
            .check_equal_to(&ast, b, false)
            .unwrap_err();
        assert!(message.contains("Empty expected JS. Actual JS is:\n    short"));
        let a_script = ast.new_node_with_child(Token::SCRIPT, a);
        let b_script = ast.new_node_with_child(Token::SCRIPT, b);
        let a_root = ast.new_node_with_child(Token::ROOT, a_script);
        let b_root = ast.new_node_with_child(Token::ROOT, b_script);
        let extra = ast.new_node(Token::SCRIPT);
        a_root.add_child_to_back(&mut ast, extra);
        let extra = ast.new_node(Token::SCRIPT);
        b_root.add_child_to_back(&mut ast, extra);
        ast.new_node_with_child(Token::ROOT, a_root);
        ast.new_node_with_child(Token::ROOT, b_root);
        assert!(
            assert_node(a_root)
                .check_equal_to(&ast, b_root, false)
                .unwrap_err()
                .contains("For expected 0:")
        );
    }
    // port: NodeSubject#findFirstMismatch / Node#isEquivalentTo (separate compiler arenas)
    #[test]
    fn cross_arena_comparison_resolves_colliding_ids_in_each_owner() {
        let mut a = Ast::new();
        let mut b = Ast::new();
        let x = IR::name(&mut a, "x");
        let y = IR::name(&mut b, "y");
        assert_eq!(x, y);
        assert!(find_first_mismatch_across(&a, x, &b, y, false).is_some());
        y.set_string(&mut b, "x");
        assert!(x.is_equivalent_to_across(&a, &b, y));
        assert_node(x)
            .check_equal_to_across(&a, &b, y, true)
            .unwrap();
        let dte_a = a.new_string("number");
        let dte_b = b.new_string("string");
        assert_eq!(dte_a, dte_b);
        x.set_declared_type_expression(&mut a, Some(dte_a));
        y.set_declared_type_expression(&mut b, Some(dte_b));
        assert!(!x.is_equivalent_to_across(&a, &b, y));
        dte_b.set_string(&mut b, "number");
        assert!(x.is_equivalent_to_across(&a, &b, y));
        let mut doc_a = JSDocInfo::builder();
        doc_a.record_type(Some(std::sync::Arc::new(
            crate::js_type_expression::JSTypeExpression::new(dte_a, "a"),
        )));
        let mut doc_b = JSDocInfo::builder();
        doc_b.record_type(Some(std::sync::Arc::new(
            crate::js_type_expression::JSTypeExpression::new(dte_b, "b"),
        )));
        x.set_jsdoc_info(&mut a, doc_a.build());
        y.set_jsdoc_info(&mut b, doc_b.build());
        assert!(find_first_mismatch_across(&a, x, &b, y, true).is_none());
        dte_b.set_string(&mut b, "string");
        assert!(!crate::jsdoc_info::JSDocInfo::are_equivalent_across(
            &a,
            &b,
            x.get_jsdoc_info(&a).as_deref(),
            y.get_jsdoc_info(&b).as_deref()
        ));
        x.set_declared_type_expression(&mut a, None);
        y.set_declared_type_expression(&mut b, None);
        assert!(find_first_mismatch_across(&a, x, &b, y, false).is_none());
        assert!(find_first_mismatch_across(&a, x, &b, y, true).is_some());
        let shadow_a = IR::name(&mut a, "shadow");
        let shadow_b = IR::name(&mut b, "other");
        x.set_closure_unaware_shadow(&mut a, Some(shadow_a));
        y.set_closure_unaware_shadow(&mut b, Some(shadow_b));
        assert_eq!(
            find_first_mismatch_across(&a, x, &b, y, false),
            Some(NodeMismatch {
                actual: shadow_a,
                expected: shadow_b
            })
        );
        let error = assert_node(x)
            .using_serializer(|ast, n| n.to_string_tree(ast))
            .check_equal_to_across(&a, &b, y, false)
            .unwrap_err();
        assert!(error.contains("shadow"));
        assert!(error.contains("other"));
    }
}
