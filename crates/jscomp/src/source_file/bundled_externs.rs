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

    /// Only consumed together with the watch registered when this exact embedded parse was loaded.
    pub(crate) fn bundled_script_summary(&self) -> Option<&'static bundled_script::BundledScript> {
        use bundled_script::BundledScript;
        let name = self.get_name();
        include!(concat!(env!("OUT_DIR"), "/bundled_script_summaries.rs"))
    }

    pub(crate) fn bundled_declarations(&self) -> Option<&'static [u8]> {
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
        assert!(lookalike.bundled_declarations().is_none());
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
    fn gathered_properties_follow_appended_statements_and_edits_to_detached_type_nodes() {
        use super::bundled_script::BundledScript;
        use crate::{
            compiler::Compiler,
            compiler_input::CompilerInput,
            compiler_options::{CompilerOptions, LanguageMode},
            compiler_pass::CompilerPass,
            gather_extern_properties::{GatherExternProperties, Mode},
        };
        use closure_rhino::{
            ir::IR,
            js_type_expression::JSTypeExpression,
            jsdoc_info::JSDocInfo,
            node::{NodeId, SnapshotUse},
        };

        fn load(file: SourceFile) -> (Compiler, NodeId) {
            let mut compiler = Compiler::new();
            let mut options = CompilerOptions::new();
            options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
            compiler.init_options(options);
            let input = CompilerInput::new_with_extern(file, true);
            let script = input.get_ast_root(&mut compiler);
            (compiler, script)
        }
        fn collect(pair: &mut (Compiler, NodeId), mode: Mode) -> Vec<String> {
            let (compiler, script) = pair;
            compiler.set_extern_properties(IndexSet::default());
            GatherExternProperties::new(compiler, mode).process(compiler, *script, *script);
            compiler
                .get_extern_properties()
                .unwrap()
                .iter()
                .cloned()
                .collect()
        }
        fn append(pair: &mut (Compiler, NodeId)) {
            let (compiler, script) = pair;
            let name = IR::name(compiler, "window");
            let prop = IR::getprop(compiler, name, "appendedProperty");
            let field = IR::string_key(compiler, "appendedField");
            let ty = compiler.new_node_with_child(Token::LB, field);
            let mut doc = JSDocInfo::builder();
            doc.record_type(Some(Arc::new(JSTypeExpression::new(ty, "extern.js"))));
            prop.set_jsdoc_info(compiler, doc.build());
            let stmt = IR::expr_result(compiler, prop);
            script.add_child_to_back(compiler, stmt);
        }
        fn change_old_jsdoc_field(pair: &mut (Compiler, NodeId)) {
            let (compiler, script) = pair;
            let mut nodes = vec![*script];
            while let Some(n) = nodes.pop() {
                if n.is_string_key(compiler) && n.get_string_ref(compiler) == "identifier" {
                    n.set_string(compiler, "changedTypeField");
                    return;
                }
                nodes.extend(n.children(compiler));
                if let Some(doc) = n.get_jsdoc_info_ref(compiler) {
                    nodes.extend(doc.get_type_nodes());
                }
            }
            panic!("fixture contains the identifier record field");
        }
        let bundled = SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .find(|f| f.get_name() == "externs.zip//w3c_touch_event.js")
            .unwrap();
        let ordinary = SourceFile::from_code(bundled.get_name(), bundled.get_code().unwrap());
        let mut oracle = load(ordinary);
        let mut target = load(bundled);
        assert!(
            target
                .0
                .parsed_script_record::<BundledScript>(target.1, SnapshotUse::SyntaxAndJsDoc)
                .is_some()
        );
        for mode in [Mode::CHECK, Mode::OPTIMIZE, Mode::CHECK_AND_OPTIMIZE] {
            assert_eq!(collect(&mut target, mode), collect(&mut oracle, mode));
        }
        append(&mut target);
        append(&mut oracle);
        assert!(
            target
                .0
                .parsed_script_record::<BundledScript>(target.1, SnapshotUse::Syntax)
                .unwrap()
                .1
                .is_some()
        );
        for (mode, last) in [
            (Mode::CHECK, "appendedField"),
            (Mode::OPTIMIZE, "appendedProperty"),
        ] {
            let got = collect(&mut target, mode);
            assert_eq!(got, collect(&mut oracle, mode));
            assert_eq!(got.last().unwrap(), last);
        }
        change_old_jsdoc_field(&mut target);
        change_old_jsdoc_field(&mut oracle);
        assert!(
            target
                .0
                .parsed_script_record::<BundledScript>(target.1, SnapshotUse::SyntaxAndJsDoc)
                .is_none()
        );
        let fields = collect(&mut target, Mode::CHECK);
        assert_eq!(fields, collect(&mut oracle, Mode::CHECK));
        assert!(fields.iter().any(|s| s == "changedTypeField"));
    }

    #[test]
    fn declarations_preserve_the_source_cache_and_respect_overrides() {
        let file = SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .find(|file| file.get_name() == "externs.zip//es6.js")
            .unwrap();
        file.set_kind(SourceKind::EXTERN);
        file.clear_cached_source();
        let records = file.bundled_declarations().unwrap();
        assert!(records.contains(&declarations::PROPERTY));
        assert!(file.state.lock().unwrap().code.is_none());
        file.set_code_deprecated("var replacement;");
        assert!(file.bundled_declarations().is_none());
        file.clear_cached_source();
        assert_eq!(file.bundled_declarations(), Some(records));
    }

    #[test]
    fn synthetic_block_markers_preserve_live_property_validation() {
        use crate::{
            compiler::Compiler,
            compiler_input::CompilerInput,
            compiler_options::{CompilerOptions, LanguageMode},
            compiler_pass::CompilerPass,
            gather_extern_properties::{GatherExternProperties, Mode},
        };
        use closure_rhino::node::{ObjectProp, Prop};
        use std::panic::{AssertUnwindSafe, catch_unwind};

        fn failure(file: SourceFile) -> String {
            let mut compiler = Compiler::new();
            let mut options = CompilerOptions::new();
            options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
            options.set_synthetic_block_start_marker("start".to_owned());
            compiler.init_options(options);
            let input = CompilerInput::new_with_extern(file, true);
            let script = input.get_ast_root(&mut compiler);
            let mut nodes = vec![script];
            let block = loop {
                let n = nodes.pop().unwrap();
                if n.is_block(&compiler) {
                    break n;
                }
                nodes.extend(n.children(&compiler));
            };
            block.put_prop(
                &mut compiler,
                Prop::SYNTHETIC,
                Some(ObjectProp::Node(block)),
            );
            let panic = catch_unwind(AssertUnwindSafe(|| {
                GatherExternProperties::new(&compiler, Mode::OPTIMIZE).process(
                    &mut compiler,
                    script,
                    script,
                );
            }))
            .unwrap_err();
            panic
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| panic.downcast_ref::<&str>().unwrap().to_string())
        }

        let file = SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .find(|f| f.get_name() == "externs.zip//w3c_batterystatus.js")
            .unwrap();
        let ordinary = SourceFile::from_code_with_kind(
            file.get_name(),
            file.get_code().unwrap(),
            SourceKind::EXTERN,
        );
        let expected = failure(ordinary);
        assert!(expected.contains("UnsupportedOperationException"));
        assert_eq!(failure(file), expected);
    }

    #[test]
    fn lazy_scope_creation_reports_the_original_property_root() {
        use crate::{
            compiler::Compiler,
            compiler_options::{CompilerOptions, LanguageMode},
            compiler_pass::CompilerPass,
            var_check::VarCheck,
        };
        use std::panic::{AssertUnwindSafe, catch_unwind};

        fn failure(battery: SourceFile) -> String {
            let mut options = CompilerOptions::new();
            options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
            let mut compiler = Compiler::new();
            let module = SourceFile::from_code_with_kind(
                "other.js",
                "goog.loadModule(function() {});",
                SourceKind::EXTERN,
            );
            compiler.init(
                &[Arc::new(battery), Arc::new(module)],
                &[Arc::new(SourceFile::from_code("app.js", ""))],
                options,
            );
            compiler.parse_inputs();
            let externs = compiler.get_externs_root().unwrap();
            let root = compiler.get_js_root().unwrap();
            let panic = catch_unwind(AssertUnwindSafe(|| {
                VarCheck::new(&compiler).process(&mut compiler, externs, root);
            }))
            .unwrap_err();
            panic
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| panic.downcast_ref::<&str>().unwrap().to_string())
        }

        let file = SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .find(|f| f.get_name() == "externs.zip//w3c_batterystatus.js")
            .unwrap();
        let ordinary = SourceFile::from_code_with_kind(
            file.get_name(),
            file.get_code().unwrap(),
            SourceKind::EXTERN,
        );
        let expected = failure(ordinary);
        assert!(
            expected.contains("Node(NAME BatteryManager): externs.zip//w3c_batterystatus.js:37:0"),
            "{expected}"
        );
        assert!(
            expected.contains("Parent(GETPROP prototype): externs.zip//w3c_batterystatus.js:37:15"),
            "{expected}"
        );
        assert_eq!(failure(file), expected);
    }

    #[test]
    fn missing_global_property_roots_preserve_original_extern_diagnostics() {
        use crate::{
            check_level::CheckLevel,
            compiler::Compiler,
            compiler_options::{CompilerOptions, LanguageMode},
            compiler_pass::CompilerPass,
            diagnostic_groups::EXTERNS_VALIDATION,
            var_check::VarCheck,
        };

        fn warnings(battery: SourceFile, extra: &str) -> Vec<String> {
            let mut options = CompilerOptions::new();
            options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
            options.set_warning_level(EXTERNS_VALIDATION.clone(), CheckLevel::WARNING);
            let mut compiler = Compiler::new();
            let extras = SourceFile::from_code_with_kind("extra.js", extra, SourceKind::EXTERN);
            compiler.init(
                &[Arc::new(extras), Arc::new(battery)],
                &[Arc::new(SourceFile::from_code("app.js", ""))],
                options,
            );
            compiler.parse_inputs();
            let externs = compiler.get_externs_root().unwrap();
            let root = compiler.get_js_root().unwrap();
            VarCheck::new(&compiler).process(&mut compiler, externs, root);
            compiler
                .get_warnings()
                .into_iter()
                .map(|w| w.to_string())
                .collect()
        }

        for (extra, missing) in [("", true), ("var Navigator;", false)] {
            let bundled = SourceFile::bundled_externs(Environment::BROWSER)
                .into_iter()
                .find(|f| f.get_name() == "externs.zip//w3c_batterystatus.js")
                .unwrap();
            let roots = bundled
                .bundled_script_summary()
                .unwrap()
                .global_property_roots
                .unwrap();
            assert!(roots.contains(&"Navigator"));
            let ordinary = SourceFile::from_code_with_kind(
                bundled.get_name(),
                bundled.get_code().unwrap(),
                SourceKind::EXTERN,
            );
            let expected = warnings(ordinary, extra);
            assert_eq!(expected.iter().any(|w| w.contains("Navigator")), missing);
            assert_eq!(warnings(bundled, extra), expected);
        }
    }

    /// The diagnostics of VariableReferenceCheck, every one enabled, after the passes before it
    /// that change the externs.
    fn variable_reference_diagnostics(
        extra_externs: &str,
        code: &str,
        skip: bool,
    ) -> (Vec<String>, usize) {
        use crate::{
            check_level::CheckLevel,
            compiler::Compiler,
            compiler_options::{CompilerOptions, LanguageMode},
            compiler_pass::CompilerPass,
            declared_global_externs_on_window::DeclaredGlobalExternsOnWindow,
            diagnostic_group::DiagnosticGroup,
            var_check::VAR_MULTIPLY_DECLARED_ERROR,
            variable_reference_check::*,
        };
        let mut options = CompilerOptions::new();
        options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
        for diagnostic in [
            &EARLY_REFERENCE,
            &EARLY_EXPORTS_REFERENCE,
            &REDECLARED_VARIABLE,
            &EARLY_REFERENCE_ERROR,
            &REASSIGNED_CONSTANT,
            &REDECLARED_VARIABLE_ERROR,
            &DECLARATION_NOT_DIRECTLY_IN_BLOCK,
            &UNUSED_LOCAL_ASSIGNMENT,
            &VAR_MULTIPLY_DECLARED_ERROR,
        ] {
            options.set_warning_level(DiagnosticGroup::for_type(diagnostic), CheckLevel::WARNING);
        }
        let mut externs: Vec<Arc<SourceFile>> = SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .map(Arc::new)
            .collect();
        if !extra_externs.is_empty() {
            externs.push(Arc::new(SourceFile::from_code_with_kind(
                "extra.js",
                extra_externs,
                SourceKind::EXTERN,
            )));
        }
        let mut compiler = Compiler::new();
        compiler.init(
            &externs,
            &[Arc::new(SourceFile::from_code("app.js", code))],
            options,
        );
        compiler.parse_inputs();
        let externs = compiler.get_externs_root().unwrap();
        let root = compiler.get_js_root().unwrap();
        DeclaredGlobalExternsOnWindow::new(&compiler).process(&mut compiler, externs, root);
        let mut check = VariableReferenceCheck::new(&compiler);
        check.set_skip_bundled_externs(skip);
        check.process(&mut compiler, externs, root);
        let diagnostics = compiler
            .get_errors()
            .into_iter()
            .chain(compiler.get_warnings())
            .map(|e| e.to_string())
            .collect();
        (diagnostics, check.skipped_bundled_scripts())
    }

    /// `VariableReferenceCheck` leaves out the references in the bundled externs because they
    /// report nothing for the bundled externs alone, with every diagnostic of the pass enabled.
    /// An externs update that breaks this fails here.
    #[test]
    fn variable_reference_check_reports_nothing_for_the_bundled_externs() {
        // A report filtered by an @suppress tag still sets `hasErrors`, which is read only for a
        // const var: the bundled externs declare none.
        for file in SourceFile::bundled_externs(Environment::BROWSER) {
            file.set_kind(SourceKind::EXTERN);
            let file = Arc::new(file);
            let code = file.read_source().unwrap();
            let mut ast = Ast::new();
            let root = ParserRunner::parse(
                &mut ast,
                file.clone(),
                code.into_text(),
                &extern_snapshot::CONFIG,
                &mut AlwaysThrowsErrorReporter,
            )
            .ast
            .unwrap();
            assert!(
                root.children(&ast).all(|statement| !matches!(
                    statement.get_token(&ast),
                    Token::LET | Token::CONST | Token::CLASS
                )),
                "{}",
                file.get_name()
            );
        }
        let (diagnostics, skipped) = variable_reference_diagnostics("", "", false);
        assert_eq!(skipped, 0);
        assert_eq!(diagnostics, Vec::<String>::new());
        let (diagnostics, skipped) = variable_reference_diagnostics("", "", true);
        assert_eq!(
            skipped,
            SourceFile::bundled_externs(Environment::BROWSER).len()
        );
        assert_eq!(diagnostics, Vec::<String>::new());
    }

    #[test]
    fn variable_reference_check_skips_bundled_prefixes_without_changing_diagnostics() {
        let mut reported = 0;
        for (extra, code, skips) in [
            (
                "",
                "window['x'] = 1; Infinity = 2; Symbol.iterator; Array.prototype.y = 1;",
                true,
            ),
            (
                "",
                "let a = b; let b = 1; const c = 1; c = 2; d = 1; var d;",
                true,
            ),
            ("", "Array = 1; Array; function f() { return Array; }", true),
            (
                "var extra; Array.extra; extra = Symbol;",
                "extra = 1; Symbol = extra;",
                true,
            ),
            ("", "export var window = 1; window = 2;", true),
            ("", "var performance = 1; performance = 2;", false),
            ("", "Symbol(); function Symbol() {}", false),
            ("", "{ let Array = 1; } try {} catch (Map) {}", false),
            ("var Array;", "Array = 2;", false),
            ("function Map() {}", "", false),
        ] {
            let (expected, skipped) = variable_reference_diagnostics(extra, code, false);
            assert_eq!(skipped, 0);
            let (actual, skipped) = variable_reference_diagnostics(extra, code, true);
            assert_eq!(skipped > 0, skips, "{extra} / {code}");
            assert_eq!(actual, expected, "{extra} / {code}");
            reported += expected.len();
        }
        assert!(reported >= 5, "{reported}");
    }

    /// AnalyzePrototypeProperties marks the names of the GETPROPs and class members of the
    /// externs; for a `normalize_clean` script that is its `properties` summary. StrictModeCheck
    /// finds nothing to report in such a script.
    #[test]
    fn clean_scripts_summarize_the_marked_extern_properties() {
        use closure_rhino::node::NodeId;
        fn marked(ast: &Ast, n: NodeId, names: &mut IndexSet<String>) {
            for c in n.children(ast) {
                marked(ast, c, names);
            }
            if matches!(
                n.get_token(ast),
                Token::GETPROP | Token::MEMBER_FUNCTION_DEF | Token::GETTER_DEF | Token::SETTER_DEF
            ) {
                names.insert(n.get_string_ref(ast).to_string_lossy());
            }
        }
        let mut checked = 0;
        for file in SourceFile::bundled_externs(Environment::BROWSER) {
            file.set_kind(SourceKind::EXTERN);
            let Some(summary) = file.bundled_script_summary() else {
                continue;
            };
            if !summary.normalize_clean {
                continue;
            }
            let file = Arc::new(file);
            let code = file.read_source().unwrap();
            let mut ast = Ast::new();
            let root = ParserRunner::parse(
                &mut ast,
                file.clone(),
                code.into_text(),
                &extern_snapshot::CONFIG,
                &mut AlwaysThrowsErrorReporter,
            )
            .ast
            .unwrap();
            let mut names = IndexSet::default();
            marked(&ast, root, &mut names);
            assert!(
                names.iter().eq(summary.properties.iter()),
                "{}",
                file.get_name()
            );
            // StrictModeCheck reports nothing for such a script.
            let mut nodes = vec![root];
            while let Some(n) = nodes.pop() {
                assert!(
                    !matches!(
                        n.get_token(&ast),
                        Token::DELPROP | Token::WITH | Token::CLASS
                    ) && !(n.is_object_lit(&ast) && n.has_children(&ast))
                        && !(n.is_assign(&ast)
                            && n.get_first_child(&ast).is_some_and(|lhs| {
                                lhs.is_name(&ast)
                                    && (lhs.get_string_ref(&ast) == "arguments"
                                        || lhs.get_string_ref(&ast) == "eval")
                            })),
                    "{}",
                    file.get_name()
                );
                nodes.extend(n.children(&ast));
            }
            checked += 1;
        }
        assert!(checked > 100, "{checked}");
    }

    #[test]
    fn declarations_require_the_snapshot_loaded_for_this_script() {
        use crate::{
            compiler::Compiler,
            compiler_input::CompilerInput,
            compiler_options::{CompilerOptions, LanguageMode},
        };

        let file = Arc::new(
            SourceFile::bundled_externs(Environment::BROWSER)
                .into_iter()
                .find(|file| file.get_name() == "externs.zip//es6.js")
                .unwrap(),
        );
        let input = CompilerInput::new_with_extern(file.clone(), true);
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
        compiler.init_options(options);
        let original = input.get_ast_root(&mut compiler);
        assert!(input.bundled_declarations(original).is_some());

        input.clear_ast();
        file.set_code_deprecated("var replacement;");
        let replacement = input.get_ast_root(&mut compiler);
        assert!(!file.has_source_in_memory());
        assert!(file.bundled_declarations().is_some());
        assert!(input.bundled_declarations(replacement).is_none());
        assert!(input.bundled_declarations(original).is_none());

        input.clear_ast();
        let reloaded = input.get_ast_root(&mut compiler);
        assert!(input.bundled_declarations(reloaded).is_some());
        assert!(input.bundled_declarations(replacement).is_none());
        let shadow = closure_rhino::ir::IR::script(&mut compiler);
        shadow.set_static_source_file(&mut compiler, Some(file));
        shadow.set_is_in_closure_unaware_subtree(&mut compiler, true);
        input.init_shadow_ast(&compiler, shadow);
        assert!(input.bundled_declarations(shadow).is_none());
        assert!(input.bundled_declarations(reloaded).is_none());
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
