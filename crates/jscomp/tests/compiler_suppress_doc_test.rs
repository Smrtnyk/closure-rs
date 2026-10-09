/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/WarningsGuardTest.java.

use closure_jscomp::{
    Compiler, check_level::CheckLevel, diagnostic_group::DiagnosticGroup,
    diagnostic_type::DiagnosticType, js_error::JSError,
    suppress_doc_warnings_guard::SuppressDocWarningsGuard,
};
use closure_rhino::{node::NodeId, token::Token};
use std::sync::Arc;
static BAR_WARNING: DiagnosticType = DiagnosticType::warning("BAR", "Bar description");
fn guard() -> SuppressDocWarningsGuard {
    SuppressDocWarningsGuard::new(closure_rhino::fast_hash::IndexMap::<_, _>::from_iter([(
        "deprecated".into(),
        Arc::new(DiagnosticGroup::new(&[&BAR_WARNING])),
    )]))
}
// port: WarningsGuardTest#findNameNode
fn find_name_node(c: &Compiler, root: NodeId, name: &str) -> Option<NodeId> {
    if root.is_name(c) && root.get_string(c) == name {
        return Some(root);
    }
    root.children(c).find_map(|n| find_name_node(c, n, name))
}
fn level(
    c: &Compiler,
    g: &SuppressDocWarningsGuard,
    root: NodeId,
    name: &str,
) -> Option<CheckLevel> {
    g.level(
        c,
        &JSError::make(c, find_name_node(c, root, name).unwrap(), &BAR_WARNING, &[]),
    )
}
// port: WarningsGuardTest#testSuppressGuard1
#[test]
fn test_suppress_guard1() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code(
        "/** @suppress {deprecated} */ function f() { a; }\nfunction g() { b; }\n",
    );
    assert_eq!(
        g.level(&c, &JSError::make(&c, code, &BAR_WARNING, &[])),
        None
    );
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
    assert_eq!(level(&c, &g, code, "b"), None);
}
// port: WarningsGuardTest#testSuppressGuard2
#[test]
fn test_suppress_guard2() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code("/** @fileoverview \n * @suppress {deprecated} */ function f() { a; }\nfunction g() { b; }\n");
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
    assert_eq!(level(&c, &g, code, "b"), Some(CheckLevel::OFF));
}
// port: WarningsGuardTest#testSuppressGuard3
#[test]
fn test_suppress_guard3() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code("/** @suppress {deprecated} */ var f = function() { a; }\n");
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
}
// port: WarningsGuardTest#testSuppressGuard4
#[test]
fn test_suppress_guard4() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code(
        "var goog = {};\n/** @suppress {deprecated} */ goog.f = function() { a; }\n",
    );
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
}
// port: WarningsGuardTest#testSuppressGuard5
#[test]
fn test_suppress_guard5() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code(
        "var goog = {};\ngoog.f = function() { /** @suppress {deprecated} */ (a); }\n",
    );
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
}
// port: WarningsGuardTest#testSuppressGuard6
#[test]
fn test_suppress_guard6() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code("/** @fileoverview @suppress {deprecated} */\n console.log(a);");
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
}
// port: WarningsGuardTest#testSuppressGuard7
#[test]
fn test_suppress_guard7() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code("console.log(/** @suppress {deprecated} */ (a));");
    assert_eq!(level(&c, &g, code, "a"), None);
}
// port: WarningsGuardTest#testSuppressDocGuard_appliesSuppressionsOnComputedPropMethod_toPropNameExpression
#[test]
fn test_suppress_doc_guard_applies_suppressions_on_computed_prop_method_to_prop_name_expression() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code("class Foo {\n  /** @suppress {deprecated} */\n  [a]() { }\n}\n");
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
}
// port: WarningsGuardTest#testSuppressGuard_onCompoundAssignment
#[test]
fn test_suppress_guard_on_compound_assignment() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code("var goog = {};\n/** @suppress {deprecated} */ goog.f += a\n");
    assert_eq!(level(&c, &g, code, "a"), Some(CheckLevel::OFF));
}
// port: WarningsGuardTest#testSuppressGuard_onDetachedNode
#[test]
fn test_suppress_guard_on_detached_node() {
    let mut c = Compiler::new();
    let g = guard();
    let code = c.parse_test_code("/** @fileoverview @suppress {deprecated} */\n\nvar x;");
    let n = c.new_node(Token::EMPTY);
    let source_name = code.get_source_file_name(&c).unwrap();
    n.set_source_file_for_testing(&mut c, source_name);
    let error = JSError::make(&c, n, &BAR_WARNING, &[]);
    assert_eq!(g.level(&c, &error), Some(CheckLevel::OFF));
}

// port: WarningsGuardTest#findGetPropNode
fn find_get_prop_node(c: &Compiler, root: NodeId, name: &str) -> Option<NodeId> {
    if root.is_get_prop(c) && root.get_string(c) == name {
        return Some(root);
    }
    root.children(c)
        .find_map(|n| find_get_prop_node(c, n, name))
}
// port: WarningsGuardTest#testSuppressGuard_strictMissingPropertyOnUnionTypes
#[test]
fn test_suppress_guard_strict_missing_property_on_union_types() {
    use closure_jscomp::{
        diagnostic_groups::DiagnosticGroups, type_check::ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE,
    };
    let mut c = Compiler::new();
    let g = SuppressDocWarningsGuard::new(DiagnosticGroups::get_registered_groups());
    let code = c.parse_test_code("class C {}\nclass D{}\n/** @type {(C|D)} */\nlet obj;\n/** @suppress {strictMissingProperties} */\nobj.prop\n");
    assert_eq!(
        g.level(
            &c,
            &JSError::make(
                &c,
                find_get_prop_node(&c, code, "prop").unwrap(),
                &ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE,
                &[]
            )
        ),
        Some(CheckLevel::OFF)
    );
}
