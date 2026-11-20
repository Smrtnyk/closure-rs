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
use std::sync::OnceLock;

#[derive(Debug)]
pub(crate) struct BundledSource {
    bytes: &'static [u8],
    num_units: i32,
    num_lines: i32,
    text: OnceLock<JsString>,
}

impl BundledSource {
    pub(super) fn text(&self) -> &JsString {
        self.text
            .get_or_init(|| closure_resources::jar::decode_utf8(self.bytes).into())
    }

    pub(super) fn size(&self) -> (i32, i32) {
        (self.num_units, self.num_lines)
    }
}

impl SourceCode {
    fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bundled(a), Self::Bundled(b)) if Arc::ptr_eq(a, b) => true,
            _ => self.as_text() == other.as_text(),
        }
    }
}

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
                let name = resource.get_name();
                let sizes = if env == Environment::BROWSER {
                    include!(concat!(env!("OUT_DIR"), "/bundled_extern_sizes.rs"))
                } else {
                    None
                };
                let mut file = if let Some((num_units, num_lines)) = sizes {
                    let preloaded_code = SourceCode::Bundled(Arc::new(BundledSource {
                        bytes: resource.content,
                        num_units,
                        num_lines,
                        text: OnceLock::new(),
                    }));
                    Self::new(
                        CodeLoader::Preloaded { preloaded_code },
                        resource.path,
                        SourceKind::STRONG,
                    )
                } else {
                    Self::from_code(name, resource.get_code())
                };
                file.bundled_browser_extern = env == Environment::BROWSER;
                file
            })
            .collect()
    }

    fn is_original_bundled_extern(&self, code: Option<&SourceCode>) -> bool {
        if !self.bundled_browser_extern || !self.is_extern() {
            return false;
        }
        let state = self.state.lock().unwrap();
        let code = code.or(state.code.as_ref());
        !state.is_closure_unaware_code
            && matches!(&self.loader, CodeLoader::Preloaded { preloaded_code } if code.is_none_or(|code| preloaded_code.matches(code)))
    }

    pub(crate) fn bundled_dependency_info(
        &self,
        code: &SourceCode,
    ) -> Option<crate::deps::simple_dependency_info::SimpleDependencyInfo> {
        if !self.is_original_bundled_extern(Some(code)) {
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
        code: &SourceCode,
    ) -> Option<&'static [u8]> {
        if !self.is_original_bundled_extern(Some(code))
            || config != &*closure_parsing::extern_snapshot::CONFIG
        {
            return None;
        }
        let name = self.get_name();
        include!(concat!(env!("OUT_DIR"), "/bundled_externs.rs"))
    }

    pub(crate) fn bundled_property_declarations(&self) -> Option<&'static [u8]> {
        // When the parser has released the cached source, the next read would retrieve
        // the original embedded resource. Avoid loading it just to make this check.
        if !self.is_original_bundled_extern(None) {
            return None;
        }
        let name = self.get_name();
        include!(concat!(env!("OUT_DIR"), "/bundled_extern_declarations.rs"))
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
            let code = file.read_source().unwrap();
            if let Some(info) = file.bundled_dependency_info(&code) {
                let name = file.get_name();
                let expected = JsFileRegexParser::new(errors.clone())
                    .set_include_goog_base(true)
                    .parse_file(name, name, code.into_text());
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
            let code = file.read_source().unwrap();
            let Some(bytes) = file.extern_snapshot(&extern_snapshot::CONFIG, &code) else {
                // A future bundled file can require diagnostics or a source-only property.
                assert!(extern_snapshot::generate(file.clone(), code.into_text()).is_none());
                continue;
            };
            tested_type_transformations |= file.get_name() == "externs.zip//es6.js";
            let expected = ParserRunner::parse(
                &mut parsed,
                file.clone(),
                code.into_text(),
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
        let code = file.read_source().unwrap();
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
            SourceFile::from_code_with_kind(file.get_name(), code.as_text(), SourceKind::EXTERN);
        assert!(lookalike.extern_snapshot(config, &code).is_none());
        assert!(lookalike.bundled_dependency_info(&code).is_none());
        assert!(lookalike.bundled_property_declarations().is_none());
        file.set_code_deprecated("var replacement;");
        assert!(
            file.bundled_dependency_info(&file.read_source().unwrap())
                .is_none()
        );
        assert!(
            file.extern_snapshot(config, &file.read_source().unwrap())
                .is_none()
        );
        file.clear_cached_source();
        assert!(
            file.extern_snapshot(config, &file.read_source().unwrap())
                .is_some()
        );
        file.mark_as_closure_unaware_code();
        assert!(file.extern_snapshot(config, &code).is_none());
    }

    #[test]
    fn declarations_preserve_the_source_cache_and_respect_overrides() {
        let file = SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .find(|file| file.get_name() == "externs.zip//es6.js")
            .unwrap();
        file.set_kind(SourceKind::EXTERN);
        file.clear_cached_source();
        let records = file.bundled_property_declarations().unwrap();
        assert!(records.contains(&1));
        assert!(file.state.lock().unwrap().code.is_none());
        file.set_code_deprecated("var replacement;");
        assert!(file.bundled_property_declarations().is_none());
        file.clear_cached_source();
        assert_eq!(file.bundled_property_declarations(), Some(records));
    }

    #[test]
    fn cached_sizes_and_overrides_preserve_utf16_source_reads() {
        use closure_resources::abstract_command_line_runner::AbstractCommandLineRunner;
        let resources = AbstractCommandLineRunner::get_builtin_externs(Environment::BROWSER);
        for (file, resource) in SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .zip(resources)
        {
            let expected = SourceFile::from_code(&resource.path, resource.get_code());
            assert!(!file.has_source_in_memory());
            assert_eq!(file.get_num_lines(), expected.get_num_lines());
            assert_eq!(file.get_num_bytes(), expected.get_num_bytes());
            assert!(file.has_source_in_memory());
            if let CodeLoader::Preloaded {
                preloaded_code: SourceCode::Bundled(source),
            } = &file.loader
            {
                assert!(source.text.get().is_none());
            }
            let original = file.get_code().unwrap();
            assert_eq!(original, expected.get_code().unwrap());
            assert_eq!(file.get_proto(), expected.get_proto());
            file.set_code_deprecated("\u{feff}x\u{1f642}\r\n\u{2028}\u{2029}");
            assert_eq!((file.get_num_bytes(), file.get_num_lines()), (7, 4));
            file.clear_cached_source();
            assert!(!file.has_source_in_memory());
            assert!(file.load_code_uncached().unwrap().ptr_eq(&original));
            assert!(!file.has_source_in_memory());
            assert!(file.get_code().unwrap().ptr_eq(&original));
            assert_eq!(file.get_num_lines(), expected.get_num_lines());
            assert_eq!(file.get_num_bytes(), expected.get_num_bytes());
        }
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
