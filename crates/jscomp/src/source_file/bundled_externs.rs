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

//! Built-in externs and their build-generated parser snapshots.
use super::*;

impl SourceFile {
    /// The source text and snapshot come from the same embedded resource. A user file with
    /// the same path never receives a snapshot.
    pub fn bundled_externs(env: closure_resources::compiler_options::Environment) -> Vec<Self> {
        use closure_resources::{
            abstract_command_line_runner::AbstractCommandLineRunner, compiler_options::Environment,
        };
        AbstractCommandLineRunner::get_builtin_externs(env)
            .into_iter()
            .map(|resource| {
                let mut file = Self::from_code(&resource.path, resource.get_code());
                file.bundled_browser_extern = env == Environment::BROWSER;
                file
            })
            .collect()
    }

    fn is_original_bundled_extern(&self, code: &JsString) -> bool {
        self.bundled_browser_extern
            && self.is_extern()
            && !self.is_closure_unaware_code()
            && matches!(&self.loader, CodeLoader::Preloaded { preloaded_code } if preloaded_code == code)
    }

    pub(crate) fn bundled_dependency_info(
        &self,
        code: &JsString,
    ) -> Option<crate::deps::simple_dependency_info::SimpleDependencyInfo> {
        if !self.is_original_bundled_extern(code) {
            return None;
        }
        let name = self.get_name();
        let (externs, no_compile) =
            include!(concat!(env!("OUT_DIR"), "/bundled_extern_dependencies.rs"))?;
        Some(
            crate::deps::simple_dependency_info::SimpleDependencyInfo::builder(name, name)
                .set_has_externs_annotation(externs)
                .set_has_no_compile_annotation(no_compile)
                .build(),
        )
    }

    pub(crate) fn extern_snapshot(
        &self,
        config: &closure_parsing::config::Config,
        code: &JsString,
    ) -> Option<&'static [u8]> {
        if !self.is_original_bundled_extern(code)
            || config != &*closure_parsing::extern_snapshot::CONFIG
        {
            return None;
        }
        let name = self.get_name();
        include!(concat!(env!("OUT_DIR"), "/bundled_externs.rs"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use closure_parsing::{
        config::{JsDocParsing, StrictMode},
        extern_snapshot,
        parser_runner::ParserRunner,
    };
    use closure_resources::compiler_options::Environment;
    use closure_rhino::{
        error_reporter::AlwaysThrowsErrorReporter,
        fast_hash::IndexSet,
        node::{Ast, JsDocComparison, RecursionMode, SideEffectComparison, TypeComparison},
        token::Token,
    };

    #[test]
    fn bundled_dependencies_match_the_dependency_parser() {
        use crate::{
            black_hole_error_manager::BlackHoleErrorManager,
            deps::js_file_regex_parser::JsFileRegexParser, error_manager::ErrorManager,
        };
        let errors = Arc::new(Mutex::new(BlackHoleErrorManager::new()));
        let mut checked_es6 = false;
        for file in SourceFile::bundled_externs(Environment::BROWSER) {
            file.set_kind(SourceKind::EXTERN);
            let code = file.get_code().unwrap();
            if let Some(info) = file.bundled_dependency_info(&code) {
                let name = file.get_name();
                let expected = JsFileRegexParser::new(errors.clone())
                    .set_include_goog_base(true)
                    .parse_file(name, name, code);
                assert_eq!(info, expected, "{name}");
                checked_es6 |= name == "externs.zip//es6.js";
            }
        }
        assert!(checked_es6);
        let errors = errors.lock().unwrap();
        assert_eq!(
            (errors.get_error_count(), errors.get_warning_count()),
            (0, 0)
        );
    }

    #[test]
    fn snapshots_match_fresh_parses_of_bundled_externs() {
        let mut parsed = Ast::new();
        let mut loaded = Ast::new();
        // All 128 files append consecutively after a pre-existing node. This exercises local
        // ids, the first implicit template bound, and subsequent references to the singleton.
        parsed.new_node(Token::EMPTY);
        loaded.new_node(Token::EMPTY);
        let mut tested_type_transformations = false;
        for file in SourceFile::bundled_externs(Environment::BROWSER) {
            file.set_kind(SourceKind::EXTERN);
            let file = Arc::new(file);
            let code = file.get_code().unwrap();
            let Some(bytes) = file.extern_snapshot(&extern_snapshot::CONFIG, &code) else {
                // A future bundled file can require diagnostics or a source-only property.
                assert!(extern_snapshot::generate(file.clone(), code).is_none());
                continue;
            };
            tested_type_transformations |= file.get_name() == "externs.zip//es6.js";
            let expected = ParserRunner::parse(
                &mut parsed,
                file.clone(),
                code,
                &extern_snapshot::CONFIG,
                &mut AlwaysThrowsErrorReporter,
            );
            let actual = extern_snapshot::load(bytes, &mut loaded, file.clone());
            let root = expected.ast.unwrap();
            assert_eq!(Some(root), actual.ast);
            assert_eq!(expected.features, actual.features);
            assert_eq!(parsed.node_count(), loaded.node_count());
            assert_eq!(parsed.presence_summary(), loaded.presence_summary());
            assert!(
                root.is_equivalent_to_across_with_options(
                    &parsed,
                    &loaded,
                    root,
                    RecursionMode::DEEP,
                    TypeComparison::COMPARE,
                    JsDocComparison::COMPARE,
                    SideEffectComparison::COMPARE
                ),
                "{}",
                file.get_name()
            );
            let mut seen = IndexSet::<_>::default();
            let mut nodes = vec![root];
            let source: Arc<dyn StaticSourceFile> = file;
            while let Some(n) = nodes.pop() {
                if !seen.insert(n) {
                    continue;
                }
                assert_eq!(
                    (
                        n.get_lineno(&parsed),
                        n.get_charno(&parsed),
                        n.get_length(&parsed)
                    ),
                    (
                        n.get_lineno(&loaded),
                        n.get_charno(&loaded),
                        n.get_length(&loaded)
                    )
                );
                if n.get_static_source_file(&parsed)
                    .is_some_and(|s| Arc::ptr_eq(&s, &source))
                {
                    assert!(Arc::ptr_eq(
                        &n.get_static_source_file(&loaded).unwrap(),
                        &source
                    ));
                }
                nodes.extend(n.children(&parsed));
                if let Some(doc) = n.get_jsdoc_info(&parsed) {
                    nodes.extend(doc.get_type_nodes());
                }
            }
        }
        assert!(
            tested_type_transformations,
            "the es6 externs exercise nested feature sets"
        );
    }

    #[test]
    fn only_matching_resource_text_kind_and_config_use_the_snapshot() {
        let file = SourceFile::bundled_externs(Environment::BROWSER).remove(0);
        let code = file.get_code().unwrap();
        let config = &*closure_parsing::extern_snapshot::CONFIG;
        assert!(file.extern_snapshot(config, &code).is_none());
        file.set_kind(SourceKind::EXTERN);
        assert!(file.extern_snapshot(config, &code).is_some());
        for other in [
            config
                .to_builder()
                .set_strict_mode(StrictMode::SLOPPY)
                .build(),
            config
                .to_builder()
                .set_js_doc_parsing_mode(JsDocParsing::INCLUDE_ALL_COMMENTS)
                .build(),
            config
                .to_builder()
                .set_parse_inline_source_maps(false)
                .build(),
        ] {
            assert!(file.extern_snapshot(&other, &code).is_none());
        }
        let lookalike =
            SourceFile::from_code_with_kind(file.get_name(), code.clone(), SourceKind::EXTERN);
        assert!(lookalike.extern_snapshot(config, &code).is_none());
        assert!(lookalike.bundled_dependency_info(&code).is_none());
        file.set_code_deprecated("var replacement;");
        assert!(
            file.bundled_dependency_info(&file.get_code().unwrap())
                .is_none()
        );
        assert!(
            file.extern_snapshot(config, &file.get_code().unwrap())
                .is_none()
        );
        file.clear_cached_source();
        assert!(
            file.extern_snapshot(config, &file.get_code().unwrap())
                .is_some()
        );
        file.mark_as_closure_unaware_code();
        assert!(file.extern_snapshot(config, &code).is_none());
    }
    #[test]
    fn files_requiring_parse_diagnostics_use_source() {
        let file = Arc::new(SourceFile::from_code_with_kind(
            "extern.js",
            "var = ;",
            SourceKind::EXTERN,
        ));
        let code = file.get_code().unwrap();
        assert!(extern_snapshot::generate(file, code).is_none());
    }
}
