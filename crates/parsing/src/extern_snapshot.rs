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

//! Parser snapshots embedded for the bundled browser externs. The host build and the target
//! compiler use this same schema and parser config. A file with diagnostics, source maps or
//! unsupported properties uses the ordinary parser.

use crate::{
    config::{Config, LanguageMode, StrictMode},
    parser::feature_set::FeatureSet,
    parser_runner::{ParseResult, ParserRunner},
};
use closure_rhino::{
    error_reporter::ErrorReporter,
    js_string::JsString,
    node::{
        Ast, NodeId,
        snapshot::{AstSnapshot, BundledAstSnapshot},
    },
    static_source_file::StaticSourceFile,
};
use std::sync::{Arc, LazyLock};

pub static CONFIG: LazyLock<Config> =
    LazyLock::new(|| ParserRunner::create_config(LanguageMode::ES_NEXT, None, StrictMode::STRICT));

#[derive(serde::Serialize, serde::Deserialize)]
struct Script<A = AstSnapshot<FeatureSet>> {
    arena: A,
    features: FeatureSet,
}

pub fn generate(file: Arc<dyn StaticSourceFile>, code: JsString) -> Option<Vec<u8>> {
    generate_with_ast(file, code, |_, _| ()).map(|(bytes, ())| bytes)
}

/// Derive additional build-time data from the very same successful parse as the snapshot.
pub fn generate_with_ast<T>(
    file: Arc<dyn StaticSourceFile>,
    code: JsString,
    inspect: impl FnOnce(&Ast, NodeId) -> T,
) -> Option<(Vec<u8>, T)> {
    let (script, data) = capture(file, code, inspect)?;
    Some((
        postcard::to_allocvec(&script).expect("serialize extern"),
        data,
    ))
}

/// The embedded copy can use borrowed fixed-width rows. Keep generating and accepting the
/// original serde format for standalone snapshots.
pub fn generate_bundled_with_ast<T>(
    file: Arc<dyn StaticSourceFile>,
    code: JsString,
    inspect: impl FnOnce(&Ast, NodeId) -> T,
) -> Option<(Vec<u8>, T)> {
    let (script, data) = capture(file, code, inspect)?;
    let script = Script {
        arena: script.arena.for_bundle(),
        features: script.features,
    };
    // A legacy snapshot starts with its nonzero root id, never a zero byte.
    let bytes = postcard::to_allocvec(&(0u8, script));
    Some((bytes.expect("serialize bundled extern"), data))
}

fn capture<T>(
    file: Arc<dyn StaticSourceFile>,
    code: JsString,
    inspect: impl FnOnce(&Ast, NodeId) -> T,
) -> Option<(Script, T)> {
    let mut ast = Ast::new_for_preparse();
    let mut diagnostics = Diagnostics::default();
    let result =
        ParserRunner::try_parse(&mut ast, file.clone(), code, &CONFIG, &mut diagnostics).ok()?;
    if diagnostics.0
        || !result.comments.is_empty()
        || !result.top_level_statement_ranges.is_empty()
        || result.source_map_url.is_some()
    {
        return None;
    }
    let root = result.ast?;
    let arena = AstSnapshot::capture(&ast, root, &file)?;
    Some((
        Script {
            arena,
            features: result.features,
        },
        inspect(&ast, root),
    ))
}

pub fn load(bytes: &[u8], ast: &mut Ast, file: Arc<dyn StaticSourceFile>) -> ParseResult {
    let (root, features) = if let Some(packed) = bytes.strip_prefix(&[0]) {
        let script: Script<BundledAstSnapshot<FeatureSet, &[u8]>> =
            postcard::from_bytes(packed).expect("build-generated packed extern");
        (script.arena.append(ast, file), script.features)
    } else {
        let script: Script = postcard::from_bytes(bytes).expect("build-generated extern snapshot");
        (script.arena.append(ast, file), script.features)
    };
    ParseResult::new(Some(root), Vec::new(), features, None)
}

#[derive(Default)]
struct Diagnostics(bool);
impl ErrorReporter for Diagnostics {
    fn error(&mut self, _: &str, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
    fn warning(&mut self, _: &str, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
    fn error_js_string(&mut self, _: &JsString, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
    fn warning_js_string(&mut self, _: &JsString, _: &str, _: i32, _: i32) {
        self.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use closure_rhino::{
        node::{JsDocComparison, RecursionMode, SideEffectComparison, TypeComparison},
        simple_source_file::SimpleSourceFile,
        static_source_file::SourceKind,
    };

    #[test]
    fn legacy_packed_snapshots_recover_flags_from_a_transplanted_node_payload() {
        use closure_rhino::{
            jsdoc_info::{Bit, JSDocInfo},
            token::Token,
        };

        let file: Arc<dyn StaticSourceFile> =
            Arc::new(SimpleSourceFile::new("replacement.js", SourceKind::EXTERN));
        let mut worker = Ast::new_for_preparse();
        let root = worker.new_node(Token::EMPTY);
        let mut other = Ast::new();
        let annotated = other.new_node(Token::EMPTY);
        let mut builder = JSDocInfo::builder();
        builder.record_constructor();
        annotated.set_jsdoc_info(&mut other, builder.build());
        std::mem::swap(&mut worker[root], &mut other[annotated]);
        assert!(!worker.has_seen_jsdoc_bit(Bit::CONSTRUCTOR));
        let arena = AstSnapshot::<FeatureSet>::capture(&worker, root, &file).unwrap();
        let legacy = postcard::to_allocvec(&(
            0u8,
            Script {
                arena: arena.for_bundle(),
                features: FeatureSet::BARE_MINIMUM,
            },
        ))
        .unwrap();
        let mut restored = Ast::new();
        let node = load(&legacy, &mut restored, file).ast.unwrap();
        assert!(restored.has_seen_jsdoc_bit(Bit::CONSTRUCTOR));
        assert!(node.get_jsdoc_info_ref(&restored).unwrap().is_constructor());
    }

    #[test]
    fn relocated_packed_properties_keep_their_identity_and_can_be_edited() {
        use closure_rhino::{js_type_expression::JSTypeExpression, node::Prop, token::Token};

        let file: Arc<dyn StaticSourceFile> =
            Arc::new(SimpleSourceFile::new("docs.js", SourceKind::EXTERN));
        let code = JsString::from(
            "/** @template T */ function Box() {}\n/** @type {number} */ var value;",
        );
        let ordinary = generate(file.clone(), code.clone()).unwrap();
        let (packed, ()) = generate_bundled_with_ast(file.clone(), code, |_, _| ()).unwrap();
        for preexisting_bound in [false, true] {
            let mut a = Ast::new();
            let mut b = Ast::new();
            for arena in [&mut a, &mut b] {
                arena.new_node(Token::EMPTY);
                if preexisting_bound {
                    JSTypeExpression::implicit_template_bound(arena);
                }
            }
            let expected = load(&ordinary, &mut a, file.clone()).ast.unwrap();
            let root = load(&packed, &mut b, file.clone()).ast.unwrap();
            assert_eq!(expected, root);
            assert_eq!(a.node_count(), b.node_count());
            let var = root.get_last_child(&b).unwrap();
            let annotated = [var, var.get_only_child(&b)]
                .into_iter()
                .find(|n| n.get_jsdoc_info_ref(&b).is_some())
                .unwrap();
            let original_doc = annotated.get_jsdoc_info(&b).unwrap();
            let ty = original_doc.get_type().unwrap().get_root();
            assert!(ty.get_parent(&b).is_none());
            assert_eq!(
                ty,
                annotated
                    .get_jsdoc_info(&a)
                    .unwrap()
                    .get_type()
                    .unwrap()
                    .get_root()
            );
            assert!(Arc::ptr_eq(
                annotated.get_static_source_file_ref(&b).unwrap(),
                &file
            ));
            let copy = annotated.clone_node(&mut b);
            let head = annotated.get_prop_list_head_for_testing(&b).unwrap();
            assert!(Arc::ptr_eq(
                &copy.get_prop_list_head_for_testing(&b).unwrap(),
                &head
            ));
            assert!(Arc::ptr_eq(
                copy.get_jsdoc_info_ref(&b).unwrap(),
                &original_doc
            ));

            annotated.remove_prop(&mut b, Prop::JSDOC_INFO);
            annotated.set_static_source_file(&mut b, None);
            assert!(annotated.get_jsdoc_info_ref(&b).is_none());
            assert!(annotated.get_static_source_file_ref(&b).is_none());
            assert!(Arc::ptr_eq(
                copy.get_static_source_file_ref(&b).unwrap(),
                &file
            ));
            assert!(Arc::ptr_eq(
                copy.get_jsdoc_info_ref(&b).unwrap(),
                &original_doc
            ));
        }
    }

    #[test]
    fn standalone_and_packed_snapshots_keep_literals_and_unpaired_surrogates() {
        let file: Arc<dyn StaticSourceFile> =
            Arc::new(SimpleSourceFile::new("test.js", SourceKind::EXTERN));
        let mut units: Vec<_> = "var values = [17.25, 12345678901234567890n, `a\u{1f642}`, '"
            .encode_utf16()
            .collect();
        units.extend([0x00ff, 0xdfff, 0x0101]);
        units.extend("'];".encode_utf16());
        let code = JsString::from_units(units);
        let ordinary = generate(file.clone(), code.clone()).unwrap();
        let (packed, ()) = generate_bundled_with_ast(file.clone(), code, |_, _| ()).unwrap();
        assert_ne!(ordinary[0], 0);
        assert_eq!(packed[0], 0);
        let mut a = Ast::new();
        let mut b = Ast::new();
        let expected = load(&ordinary, &mut a, file.clone());
        let actual = load(&packed, &mut b, file);
        assert_eq!(expected.features, actual.features);
        assert_eq!(expected.ast, actual.ast);
        assert!(expected.ast.unwrap().is_equivalent_to_across_with_options(
            &a,
            &b,
            actual.ast.unwrap(),
            RecursionMode::DEEP,
            TypeComparison::COMPARE,
            JsDocComparison::COMPARE,
            SideEffectComparison::COMPARE,
        ));
    }
}
