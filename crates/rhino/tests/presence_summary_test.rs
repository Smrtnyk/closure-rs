/*
 * Copyright 2026 The closure-rs Authors.
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

//! Rust-only: the node arena's presence summary (`Ast::has_seen_token`, `has_seen_name`,
//! `has_seen_jsdoc_bit`) records everything that was ever in the arena and never forgets.

use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::{Bit, JSDocInfo};
use closure_rhino::node::{Ast, WatchedName};
use closure_rhino::token::Token;
use std::hash::Hasher;

const ALL_NAMES: [WatchedName; 8] = [
    WatchedName::Goog,
    WatchedName::GoogDollarPrefix,
    WatchedName::GoogColonPrefix,
    WatchedName::ModuleContentsGoogPrefix,
    WatchedName::MsgPrefix,
    WatchedName::JscompScopePrefix,
    WatchedName::DeclareIcuTemplateSuffix,
    WatchedName::ClosureToggleOrdinals,
];

fn seen_names(ast: &Ast) -> Vec<WatchedName> {
    ALL_NAMES
        .into_iter()
        .filter(|&name| ast.has_seen_name(name))
        .collect()
}

/// The watched names a single string literal node `s` makes the summary report.
fn names_of(s: &str) -> Vec<WatchedName> {
    let mut ast = Ast::new();
    ast.new_string(s);
    seen_names(&ast)
}

#[test]
fn watched_names_are_matched_exactly() {
    use WatchedName::*;
    assert_eq!(names_of("goog"), [Goog]);
    assert_eq!(names_of("goog$object$create"), [GoogDollarPrefix]);
    assert_eq!(names_of("goog:foo.Bar"), [GoogColonPrefix]);
    assert_eq!(
        names_of("module$contents$goog$object_create"),
        [ModuleContentsGoogPrefix]
    );
    assert_eq!(names_of("MSG_HELLO"), [MsgPrefix]);
    assert_eq!(names_of("MSG_"), [MsgPrefix]);
    assert_eq!(names_of("$jscomp$scope$123$0$MSG_A"), [JscompScopePrefix]);
    assert_eq!(names_of("declareIcuTemplate"), [DeclareIcuTemplateSuffix]);
    assert_eq!(names_of("mydeclareIcuTemplate"), [DeclareIcuTemplateSuffix]);
    assert_eq!(names_of("CLOSURE_TOGGLE_ORDINALS"), [ClosureToggleOrdinals]);
    for other in [
        "",
        "g",
        "goo",
        "googx",
        "Goog",
        "xgoog",
        "goog.provide",
        "MSG",
        "msg_x",
        "module$contents$x",
        "$jscomp$scop",
        "declareIcuTemplateX",
        "eclareIcuTemplate",
        "CLOSURE_TOGGLE_ORDINALSX",
        "CLOSURE_TOGGLE_ORDINAL",
    ] {
        assert_eq!(names_of(other), [], "{other}");
    }
}

#[test]
fn tokens_are_recorded_by_creation_and_set_token_and_never_forgotten() {
    let mut ast = Ast::new();
    assert!(!ast.has_seen_token(Token::CAST));
    let n = ast.new_node(Token::CAST);
    assert!(ast.has_seen_token(Token::CAST));
    assert!(!ast.has_seen_token(Token::SUPER));
    n.set_token(&mut ast, Token::SUPER);
    n.set_token(&mut ast, Token::NAME);
    assert!(ast.has_seen_token(Token::CAST));
    assert!(ast.has_seen_token(Token::SUPER));
    assert!(ast.has_seen_token(Token::NAME));
    // The last token (the bitset's highest bit) works too.
    ast.new_node(Token::PLACEHOLDER3);
    assert!(ast.has_seen_token(Token::PLACEHOLDER3));
}

#[test]
fn every_string_write_is_recorded_and_never_forgotten() {
    // set_string
    let mut ast = Ast::new();
    let n = IR::name(&mut ast, "x");
    assert_eq!(seen_names(&ast), []);
    n.set_string(&mut ast, "goog");
    n.set_string(&mut ast, "y");
    assert!(ast.has_seen_name(WatchedName::Goog));

    // string literal, template literal strings (cooked and raw), original names
    let mut ast = Ast::new();
    IR::string(&mut ast, "MSG_A");
    assert_eq!(seen_names(&ast), [WatchedName::MsgPrefix]);
    let mut ast = Ast::new();
    ast.new_template_lit_string(Some(JsString::from("goog")), "x");
    assert_eq!(seen_names(&ast), [WatchedName::Goog]);
    let mut ast = Ast::new();
    ast.new_template_lit_string(None, "MSG_A");
    assert_eq!(seen_names(&ast), [WatchedName::MsgPrefix]);
    let mut ast = Ast::new();
    let n = IR::name(&mut ast, "x");
    n.set_original_name(&mut ast, Some(JsString::from("MSG_A")));
    n.set_original_name(&mut ast, None);
    assert_eq!(seen_names(&ast), [WatchedName::MsgPrefix]);

    // Clones copy strings that were already seen; detaching forgets nothing.
    let mut ast = Ast::new();
    let goog = IR::name(&mut ast, "goog");
    let prop = IR::getprop(&mut ast, goog, "module");
    let clone = prop.clone_tree(&mut ast);
    let script = IR::script(&mut ast);
    let statement = IR::expr_result(&mut ast, clone);
    script.add_child_to_back(&mut ast, statement);
    statement.detach(&mut ast);
    assert_eq!(seen_names(&ast), [WatchedName::Goog]);
}

#[test]
fn jsdoc_flags_are_recorded_and_never_forgotten() {
    let mut ast = Ast::new();
    let n = IR::name(&mut ast, "x");
    assert!(!ast.has_seen_jsdoc_bit(Bit::EXPORT));
    let mut builder = JSDocInfo::builder();
    builder.record_export();
    n.set_jsdoc_info(&mut ast, builder.build());
    assert!(ast.has_seen_jsdoc_bit(Bit::EXPORT));
    assert!(!ast.has_seen_jsdoc_bit(Bit::PROVIDE_GOOG));
    n.set_jsdoc_info(&mut ast, None);
    assert!(ast.has_seen_jsdoc_bit(Bit::EXPORT));
}

#[test]
fn append_preparsed_merges_the_preparse_summary() {
    let mut preparse = Ast::new_for_preparse();
    let script = IR::script(&mut preparse);
    let goog = IR::name(&mut preparse, "goog");
    let statement = IR::expr_result(&mut preparse, goog);
    script.add_child_to_back(&mut preparse, statement);
    preparse.new_node(Token::CAST);
    let mut builder = JSDocInfo::builder();
    builder.record_provide_goog();
    script.set_jsdoc_info(&mut preparse, builder.build());

    let mut ast = Ast::new();
    IR::name(&mut ast, "MSG_A");
    ast.append_preparsed(preparse);
    assert_eq!(
        seen_names(&ast),
        [WatchedName::Goog, WatchedName::MsgPrefix]
    );
    assert!(ast.has_seen_token(Token::CAST));
    assert!(ast.has_seen_jsdoc_bit(Bit::PROVIDE_GOOG));
}

#[test]
fn verification_restores_a_summary_and_hashes_tree_contents() {
    let mut ast = Ast::new();
    let summary = ast.presence_summary();
    IR::name(&mut ast, "goog");
    assert!(ast.has_seen_name(WatchedName::Goog));
    ast.restore_presence_summary_for_verification(summary);
    assert!(!ast.has_seen_name(WatchedName::Goog));

    let hash = |ast: &Ast, root| {
        let mut state = std::hash::DefaultHasher::new();
        ast.hash_tree_for_verification(root, &mut state);
        state.finish()
    };
    let x = IR::name(&mut ast, "x");
    let script = IR::script(&mut ast);
    let statement = IR::expr_result(&mut ast, x);
    script.add_child_to_back(&mut ast, statement);
    let before = hash(&ast, script);
    IR::name(&mut ast, "detached");
    assert_eq!(hash(&ast, script), before);
    x.set_string(&mut ast, "y");
    assert_ne!(hash(&ast, script), before);
}
