/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerInput.java.

#![allow(clippy::collapsible_if)] // Preserve Java statement order and borrowing branches.
use crate::lazy_parsed_dependency_info::LazyParsedDependencyInfo;
use crate::{
    abstract_compiler::{AbstractCompiler, ConfigContext, READ_ERROR},
    deps::{
        dependency_info::{DependencyInfo, Require},
        module_loader::ModulePath,
        simple_dependency_info::SimpleDependencyInfo,
    },
    js_chunk::{JSChunk, JSChunkData},
    js_error::JSError,
    source_file::SourceFile,
};
use closure_parsing::{parser::feature_set::FeatureSet, parser_runner::ParserRunner};
use closure_rhino::{
    input_id::InputId,
    ir::IR,
    java_lang::io_exception::IOException,
    js_string::JsString,
    node::{NodeId, ObjectProp, Prop},
    static_source_file::{SourceKind, StaticSourceFile},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, Weak},
};

/// Shared Java object identity; the source and InputId are immutable.
#[derive(Clone)]
pub struct CompilerInput {
    id: Arc<InputId>,
    source_file: Arc<SourceFile>,
    pub(crate) state: Arc<Mutex<CompilerInputData>>,
}
#[derive(Default)]
pub(crate) struct CompilerInputData {
    chunk: Option<Weak<Mutex<JSChunkData>>>,
    ast: JsAst,
    dependency_info: Option<SimpleDependencyInfo>,
    lazy_load_flags: Option<IndexMap<String, String>>,
    is_lazy_dependency_info: bool,
    extra_requires: Vec<Require>,
    extra_provides: Vec<String>,
    ordered_requires: Vec<Require>,
    dynamic_requires: Vec<String>,
    require_dynamic_imports: Vec<String>,
    has_full_parse_dependency_info: bool,
    js_module_type: ModuleType,
    typed_scope: Option<crate::typed_scope::TypedScope>,
    compiler: bool,
    module_path: Option<ModulePath>,
}
#[derive(Default)]
struct JsAst {
    root: Option<NodeId>,
    features: Option<FeatureSet>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModuleType {
    #[default]
    NONE,
    GOOG,
    ES6,
    COMMONJS,
    JSON,
    IMPORTED_SCRIPT,
}
impl CompilerInput {
    // port: CompilerInput#CompilerInput(SourceFile)
    pub fn new(file: impl Into<Arc<SourceFile>>) -> Self {
        Self::new_with_extern(file, false)
    }
    // port: CompilerInput#CompilerInput(SourceFile, boolean)
    pub fn new_with_extern(file: impl Into<Arc<SourceFile>>, is_extern: bool) -> Self {
        let file = file.into();
        let id = InputId::new(file.get_name());
        Self::new_with_id(file, id, is_extern)
    }
    // port: CompilerInput#CompilerInput(SourceFile, InputId, boolean)
    pub fn new_with_id(
        source_file: impl Into<Arc<SourceFile>>,
        id: InputId,
        is_extern: bool,
    ) -> Self {
        let input = Self {
            source_file: source_file.into(),
            id: Arc::new(id),
            state: Arc::new(Mutex::new(CompilerInputData::default())),
        };
        if is_extern {
            input.set_is_extern();
        }
        input
    }
    pub fn get_source_file_arc(&self) -> Arc<SourceFile> {
        self.source_file.clone()
    }
    // port: CompilerInput#getAstRoot
    pub fn get_ast_root(&self, compiler: &mut AbstractCompiler) -> NodeId {
        let root = self.get_js_ast_root(compiler);
        assert!(root.is_script(compiler));
        assert!(root.get_input_id(compiler).is_some());
        root
    }
    // port: CompilerInput.JsAst#getAstRoot
    fn get_js_ast_root(&self, compiler: &mut AbstractCompiler) -> NodeId {
        if self.is_parsed() {
            return self.state.lock().unwrap().ast.root.unwrap();
        }
        if let Some(ast_root_source) =
            compiler.get_typed_ast_deserializer(self.source_file.as_ref())
        {
            let root = ast_root_source(compiler);
            let features = crate::node_util::NodeUtil::get_feature_set_of_script(compiler, root);
            let mut state = self.state.lock().unwrap();
            state.ast.root = Some(root);
            state.ast.features = features;
        } else {
            self.parse(compiler);
        }
        let root = self.state.lock().unwrap().ast.root.unwrap();
        let source: Arc<dyn StaticSourceFile> = self.source_file.clone();
        assert!(Arc::ptr_eq(
            &root.get_static_source_file(compiler).unwrap(),
            &source
        ));
        root.set_input_id(compiler, Some(self.id.clone()));
        self.source_file.clear_cached_source();
        root
    }
    // port: CompilerInput.JsAst#isParsed
    fn is_parsed(&self) -> bool {
        self.state.lock().unwrap().ast.root.is_some()
    }
    // port: CompilerInput.JsAst#parse
    fn parse(&self, compiler: &mut AbstractCompiler) {
        match self.source_file.get_code() {
            Ok(code) => {
                let config = compiler.get_parser_config(if self.source_file.is_extern() {
                    ConfigContext::EXTERNS
                } else {
                    ConfigContext::DEFAULT
                });
                // Split the compiler's arena borrow from reporting, retaining diagnostic order.
                let mut errors = crate::rhino_error_reporter::RecordingErrorHandler::default();
                let mut reporter =
                    crate::rhino_error_reporter::RhinoErrorReporter::for_old_rhino(&mut errors);
                let result = ParserRunner::parse(
                    &mut compiler.ast,
                    self.source_file.clone(),
                    code,
                    &config,
                    &mut reporter,
                );
                for error in errors.errors {
                    compiler.report(error);
                }
                {
                    let mut state = self.state.lock().unwrap();
                    state.ast.root = result.ast;
                    state.ast.features = Some(result.features);
                }
                if compiler.get_options().preserves_detailed_source_info() {
                    compiler.add_comments(self.source_file.get_name(), result.comments);
                }
                if let Some(url) = result.source_map_url
                    && compiler.get_options().get_resolve_source_map_annotations()
                {
                    if let Some(file) =
                        crate::source_map_resolver::SourceMapResolver::extract_source_map(
                            &self.source_file,
                            &url.to_string_lossy(),
                            compiler.get_options().get_parse_inline_source_maps(),
                        )
                    {
                        compiler.add_input_source_map(
                            self.source_file.get_name(),
                            crate::source_map_input::SourceMapInput::new(Arc::new(file)),
                        );
                    }
                }
            }
            Err(error) => compiler.report(JSError::make_without_location(
                &READ_ERROR,
                &[self.source_file.get_name(), &error.to_string()],
            )),
        }
        let root = self
            .state
            .lock()
            .unwrap()
            .ast
            .root
            .unwrap_or_else(|| IR::script(compiler));
        self.state.lock().unwrap().ast.root = Some(root);
        root.set_static_source_file(compiler, Some(self.source_file.clone()));
    }
    // port: CompilerInput#getInputId
    pub fn get_input_id(&self) -> &InputId {
        &self.id
    }
    // port: CompilerInput#getName
    pub fn get_name(&self) -> &str {
        self.id.get_id_name()
    }
    // port: CompilerInput#getPathRelativeToClosureBase
    pub fn get_path_relative_to_closure_base(&self) -> String {
        panic!("UnsupportedOperationException")
    }
    // port: CompilerInput#getSourceFile
    pub fn get_source_file(&self) -> &SourceFile {
        &self.source_file
    }
    // port: CompilerInput#clearAst
    // port: CompilerInput.JsAst#clearAst
    pub fn clear_ast(&self) {
        self.state.lock().unwrap().ast.root = None;
        self.source_file.clear_cached_source();
    }
    // port: CompilerInput#initShadowAst
    pub fn init_shadow_ast(&self, compiler: &AbstractCompiler, shadow_script: NodeId) {
        assert!(shadow_script.is_script(compiler));
        assert!(shadow_script.get_is_in_closure_unaware_subtree(compiler));
        self.state.lock().unwrap().ast.root = Some(shadow_script);
    }
    // port: CompilerInput#setCompiler
    pub fn set_compiler(&self, _compiler: &AbstractCompiler) {
        self.state.lock().unwrap().compiler = true;
    }
    // port: CompilerInput#setTypedScope
    pub fn set_typed_scope(&self, scope: Option<crate::typed_scope::TypedScope>) {
        self.state.lock().unwrap().typed_scope = scope;
    }
    // port: CompilerInput#getTypedScope
    pub fn get_typed_scope(&self) -> Option<crate::typed_scope::TypedScope> {
        self.state.lock().unwrap().typed_scope
    }
    // port: CompilerInput#CompilerInput(SourceFile, InputId)
    pub fn new_with_input_id(file: impl Into<Arc<SourceFile>>, id: InputId) -> Self {
        Self::new_with_id(file, id, false)
    }
    // port: CompilerInput#CompilerInput(SourceFile, String, boolean)
    pub fn new_with_string_id(file: impl Into<Arc<SourceFile>>, id: &str, is_extern: bool) -> Self {
        Self::new_with_id(file, InputId::new(id), is_extern)
    }
    // port: CompilerInput#addProvide
    pub fn add_provide(&self, provide: impl Into<String>) {
        self.state
            .lock()
            .unwrap()
            .extra_provides
            .push(provide.into());
    }
    // port: CompilerInput#getDynamicRequires
    pub fn get_dynamic_requires(&self) -> Vec<String> {
        self.state.lock().unwrap().dynamic_requires.clone()
    }
    // port: CompilerInput#addDynamicRequire
    pub fn add_dynamic_require(&self, require: impl Into<String>) -> bool {
        let require = require.into();
        let mut data = self.state.lock().unwrap();
        if !data.dynamic_requires.contains(&require) {
            data.dynamic_requires.push(require);
            return true;
        }
        false
    }
    // port: CompilerInput#getRequireDynamicImports
    pub fn get_require_dynamic_imports(&self) -> Vec<String> {
        self.state.lock().unwrap().require_dynamic_imports.clone()
    }
    // port: CompilerInput#addRequireDynamicImports
    pub fn add_require_dynamic_imports(&self, require: impl Into<String>) {
        let require = require.into();
        let mut data = self.state.lock().unwrap();
        if !data.require_dynamic_imports.contains(&require) {
            data.require_dynamic_imports.push(require);
        }
    }
    // port: CompilerInput#setHasFullParseDependencyInfo
    pub fn set_has_full_parse_dependency_info(&self, value: bool) {
        self.state.lock().unwrap().has_full_parse_dependency_info = value;
    }
    // port: CompilerInput#getJsModuleType
    pub fn get_js_module_type(&self) -> ModuleType {
        self.state.lock().unwrap().js_module_type
    }
    // port: CompilerInput#setJsModuleType
    pub fn set_js_module_type(&self, value: ModuleType) {
        self.state.lock().unwrap().js_module_type = value;
    }
    // port: CompilerInput#getCode
    pub fn get_code(&self) -> Result<JsString, IOException> {
        self.source_file.get_code()
    }
    // port: CompilerInput#getChunk
    pub fn get_chunk(&self) -> Option<JSChunk> {
        self.state
            .lock()
            .unwrap()
            .chunk
            .as_ref()
            .and_then(Weak::upgrade)
            .map(JSChunk)
    }
    // port: CompilerInput#setChunk
    pub fn set_chunk(&self, chunk: Option<&JSChunk>) {
        let mut data = self.state.lock().unwrap();
        let previous = data.chunk.as_ref().and_then(Weak::upgrade);
        assert!(
            chunk.is_none()
                || previous.is_none()
                || Arc::ptr_eq(&chunk.unwrap().0, previous.as_ref().unwrap())
        );
        data.chunk = chunk.map(|c| Arc::downgrade(&c.0));
    }
    // port: CompilerInput#overrideChunk
    pub fn override_chunk(&self, chunk: &JSChunk) {
        self.state.lock().unwrap().chunk = Some(Arc::downgrade(&chunk.0));
    }
    // port: CompilerInput#isExtern
    pub fn is_extern(&self) -> bool {
        self.source_file.is_extern()
    }
    // port: CompilerInput#setIsExtern
    pub fn set_is_extern(&self) {
        self.source_file.set_kind(SourceKind::EXTERN);
    }
    // port: CompilerInput#getLineOffset
    pub fn get_line_offset(&self, lineno: i32) -> i32 {
        self.source_file.get_line_offset(lineno)
    }
}
impl PartialEq for CompilerInput {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }
}
impl Eq for CompilerInput {}
impl Hash for CompilerInput {
    fn hash<H: Hasher>(&self, h: &mut H) {
        Arc::as_ptr(&self.state).hash(h);
    }
}
impl fmt::Debug for CompilerInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl fmt::Display for CompilerInput {
    // port: CompilerInput#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.get_name())
    }
}
impl CompilerInput {
    // port: CompilerInput#getRequires
    pub fn get_requires(&self, compiler: &mut AbstractCompiler) -> Vec<Require> {
        if self.state.lock().unwrap().has_full_parse_dependency_info {
            return self.state.lock().unwrap().ordered_requires.clone();
        }
        self.get_dependency_info(compiler).get_requires().to_vec()
    }
    // port: CompilerInput#getTypeRequires
    pub fn get_type_requires(&self, compiler: &mut AbstractCompiler) -> Vec<String> {
        self.get_dependency_info(compiler)
            .get_type_requires()
            .to_vec()
    }
    // port: CompilerInput#getProvides
    pub fn get_provides(&self, compiler: &mut AbstractCompiler) -> Vec<String> {
        self.get_dependency_info(compiler).get_provides().to_vec()
    }
    // port: CompilerInput#getHasExternsAnnotation
    pub fn get_has_externs_annotation(&self, compiler: &mut AbstractCompiler) -> bool {
        self.get_dependency_info(compiler)
            .get_has_externs_annotation()
    }
    // port: CompilerInput#getHasNoCompileAnnotation
    pub fn get_has_no_compile_annotation(&self, compiler: &mut AbstractCompiler) -> bool {
        self.get_dependency_info(compiler)
            .get_has_no_compile_annotation()
    }
    // port: CompilerInput#getKnownRequires
    pub fn get_known_requires(&self) -> Vec<Require> {
        let state = self.state.lock().unwrap();
        Self::concat(
            state
                .dependency_info
                .as_ref()
                .map_or(&[][..], DependencyInfo::get_requires),
            &state.extra_requires,
        )
    }
    // port: CompilerInput#getKnownRequiredSymbols
    pub fn get_known_required_symbols(&self) -> Vec<String> {
        Require::as_symbol_list(&self.get_known_requires())
    }
    // port: CompilerInput#getKnownProvides
    pub fn get_known_provides(&self) -> Vec<String> {
        let state = self.state.lock().unwrap();
        Self::concat(
            state
                .dependency_info
                .as_ref()
                .map_or(&[][..], DependencyInfo::get_provides),
            &state.extra_provides,
        )
    }
    // port: CompilerInput#addOrderedRequire
    pub fn add_ordered_require(&self, require: Require) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state.ordered_requires.contains(&require) {
            state.ordered_requires.push(require);
            return true;
        }
        false
    }
    // port: CompilerInput#addRequire
    pub fn add_require(&self, require: Require) {
        self.state.lock().unwrap().extra_requires.push(require);
    }
    // port: CompilerInput#getDependencyInfo
    pub fn get_dependency_info(&self, compiler: &mut AbstractCompiler) -> SimpleDependencyInfo {
        if self.state.lock().unwrap().dependency_info.is_none() {
            let (info, lazy) = self.generate_dependency_info(compiler);
            let mut state = self.state.lock().unwrap();
            state.dependency_info = Some(info);
            state.is_lazy_dependency_info = lazy;
        }
        let has_extras = {
            let state = self.state.lock().unwrap();
            !state.extra_requires.is_empty() || !state.extra_provides.is_empty()
        };
        if has_extras {
            let load_flags = self.get_dependency_load_flags(compiler);
            let mut state = self.state.lock().unwrap();
            let info = state.dependency_info.as_ref().unwrap();
            let new_info = SimpleDependencyInfo::builder(self.get_name(), self.get_name())
                .set_provides(Self::concat(info.get_provides(), &state.extra_provides))
                .set_requires(Self::concat(info.get_requires(), &state.extra_requires))
                .set_type_requires(info.get_type_requires().iter().cloned())
                .set_load_flags(load_flags)
                .set_has_externs_annotation(info.get_has_externs_annotation())
                .set_has_no_compile_annotation(info.get_has_no_compile_annotation())
                .build();
            state.dependency_info = Some(new_info);
            state.is_lazy_dependency_info = false;
            state.lazy_load_flags = None;
            state.extra_requires.clear();
            state.extra_provides.clear();
        }
        self.state
            .lock()
            .unwrap()
            .dependency_info
            .as_ref()
            .unwrap()
            .clone()
    }
    // port: CompilerInput#generateDependencyInfo
    fn generate_dependency_info(
        &self,
        compiler: &mut AbstractCompiler,
    ) -> (SimpleDependencyInfo, bool) {
        assert!(
            self.state.lock().unwrap().compiler,
            "Expected setCompiler to be called first: {self}"
        );
        let _manager = compiler.get_shared_error_manager();
        if crate::deps::js_file_regex_parser::JsFileRegexParser::is_supported()
            && compiler.prefer_regex_parser()
        {
            match self.get_code() {
                Ok(code) => {
                    let mut parser = crate::deps::js_file_regex_parser::JsFileRegexParser::new(
                        compiler.get_shared_error_manager(),
                    );
                    let info = parser
                        .set_module_loader(compiler.get_module_loader().clone())
                        .set_include_goog_base(true)
                        .parse_file(self.get_name(), self.get_name(), code);
                    (info, true)
                }
                Err(error) => {
                    compiler.get_error_manager().report(
                        crate::check_level::CheckLevel::ERROR,
                        JSError::make_without_location(
                            &READ_ERROR,
                            &[self.get_name(), &error.to_string()],
                        ),
                    );
                    ((**SimpleDependencyInfo::EMPTY).clone(), false)
                }
            }
        } else {
            let path = self.get_path(compiler);
            let mut finder = DepsFinder::new(path);
            let root = self.get_ast_root(compiler);
            finder.visit_tree(compiler, root);
            let info = root.get_jsdoc_info(compiler);
            (
                SimpleDependencyInfo::builder("", "")
                    .set_provides(finder.provides)
                    .set_requires(finder.requires)
                    .set_type_requires(finder.type_requires)
                    .set_load_flags(sorted_flags(finder.load_flags))
                    .set_has_externs_annotation(info.as_ref().is_some_and(|i| i.is_externs()))
                    .set_has_no_compile_annotation(info.as_ref().is_some_and(|i| i.is_no_compile()))
                    .build(),
                false,
            )
        }
    }
    // port: CompilerInput#isEs6Module
    pub fn is_es6_module(&self, compiler: &mut AbstractCompiler) -> bool {
        self.get_dependency_info(compiler).is_es6_module()
    }
    // port: CompilerInput#isGoogModule
    pub fn is_goog_module(&self, compiler: &mut AbstractCompiler) -> bool {
        self.get_dependency_info(compiler).is_goog_module()
    }
    // port: CompilerInput#getLoadFlags
    pub fn get_load_flags(&self, compiler: &mut AbstractCompiler) -> IndexMap<String, String> {
        self.get_dependency_info(compiler);
        self.get_dependency_load_flags(compiler)
    }
    // The load flags of the DependencyInfo: LazyParsedDependencyInfo#getLoadFlags when lazy.
    fn get_dependency_load_flags(
        &self,
        compiler: &mut AbstractCompiler,
    ) -> IndexMap<String, String> {
        let (lazy, cached, info) = {
            let state = self.state.lock().unwrap();
            (
                state.is_lazy_dependency_info,
                state.lazy_load_flags.clone(),
                state.dependency_info.as_ref().unwrap().clone(),
            )
        };
        if !lazy {
            return info.get_load_flags().clone();
        }
        if let Some(flags) = cached {
            return flags;
        }
        // generateDependencyInfo's LazyParsedDependencyInfo(info, this, compiler).
        let load_flags = LazyParsedDependencyInfo::new(info, self.clone())
            .get_load_flags(compiler)
            .clone();
        self.state.lock().unwrap().lazy_load_flags = Some(load_flags.clone());
        load_flags
    }
    // port: CompilerInput#concat
    fn concat<T: Clone + Eq + Hash>(first: &[T], second: &[T]) -> Vec<T> {
        first
            .iter()
            .chain(second)
            .cloned()
            .collect::<IndexSet<_>>()
            .into_iter()
            .collect()
    }
    // port: CompilerInput#getPath
    pub fn get_path(&self, compiler: &AbstractCompiler) -> ModulePath {
        let mut state = self.state.lock().unwrap();
        if state.module_path.is_none() {
            state.module_path = Some(compiler.get_module_loader().resolve(self.get_name()));
        }
        state.module_path.as_ref().unwrap().clone()
    }
    // port: CompilerInput#getFeatures
    pub fn get_features(&self, compiler: &mut AbstractCompiler) -> Option<FeatureSet> {
        self.get_js_ast_root(compiler);
        self.state.lock().unwrap().ast.features
    }
    /// Immutable view for DependencyInfo's borrowed getters; object equality remains CompilerInput identity.
    pub fn dependency_snapshot(
        &self,
        compiler: &mut AbstractCompiler,
    ) -> CompilerInputDependencyInfo {
        let info = self.get_dependency_info(compiler);
        let requires = self.get_requires(compiler);
        CompilerInputDependencyInfo {
            input: self.clone(),
            info: crate::deps::simple_dependency_info::Builder::from(&info)
                .set_requires(requires)
                .set_name(self.get_name())
                .build(),
        }
    }
}
pub(crate) fn sorted_flags(flags: IndexMap<String, String>) -> IndexMap<String, String> {
    let mut entries: Vec<_> = flags.into_iter().collect();
    entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    entries.into_iter().collect()
}
struct DepsFinder {
    load_flags: IndexMap<String, String>,
    provides: Vec<String>,
    requires: Vec<Require>,
    type_requires: Vec<String>,
    module_path: ModulePath,
}
impl DepsFinder {
    // port: CompilerInput.DepsFinder#DepsFinder
    fn new(module_path: ModulePath) -> Self {
        Self {
            load_flags: IndexMap::new(),
            provides: vec![],
            requires: vec![],
            type_requires: vec![],
            module_path,
        }
    }
    // port: CompilerInput.DepsFinder#visitTree
    fn visit_tree(&mut self, compiler: &AbstractCompiler, n: NodeId) {
        self.visit_subtree(compiler, n, None);
        assert!(n.is_script(compiler));
        if let Some(ObjectProp::Opaque(features)) = n.get_prop(compiler, Prop::FEATURE_SET) {
            let features = features.as_any().downcast_ref::<FeatureSet>().unwrap();
            let version = features.version();
            if version != "es3" {
                self.load_flags.insert("lang".into(), version.into());
            }
        }
    }
    // port: CompilerInput.DepsFinder#visitSubtree
    fn visit_subtree(
        &mut self,
        compiler: &AbstractCompiler,
        mut n: NodeId,
        parent: Option<NodeId>,
    ) {
        match n.get_token(compiler) {
            Token::CALL => {
                if n.has_two_children(compiler)
                    && n.get_first_child(compiler).unwrap().is_get_prop(compiler)
                    && n.get_first_first_child(compiler)
                        .unwrap()
                        .matches_name(compiler, "goog")
                {
                    if !self.requires.contains(Require::BASE) {
                        self.requires.push((**Require::BASE).clone());
                    }
                    let callee = n.get_first_child(compiler).unwrap();
                    let argument = n.get_last_child(compiler).unwrap();
                    match callee.get_string(compiler).to_string_lossy().as_str() {
                        "module" | "provide" => {
                            if callee.get_string(compiler) == "module"
                                && parent.unwrap().is_expr_result(compiler)
                                && parent
                                    .unwrap()
                                    .get_parent(compiler)
                                    .unwrap()
                                    .is_module_body(compiler)
                            {
                                self.load_flags.insert("module".into(), "goog".into());
                            }
                            if !argument.is_string_lit(compiler) {
                                return;
                            }
                            self.provides
                                .push(argument.get_string(compiler).to_string_lossy());
                            return;
                        }
                        "require" => {
                            if !argument.is_string_lit(compiler) {
                                return;
                            }
                            self.requires.push(Require::goog_require_symbol(
                                &argument.get_string(compiler).to_string_lossy(),
                            ));
                            return;
                        }
                        "requireType" => {
                            if !argument.is_string_lit(compiler) {
                                return;
                            }
                            self.type_requires
                                .push(argument.get_string(compiler).to_string_lossy());
                            return;
                        }
                        "loadModule" => {
                            assert!(
                                !argument.is_string_lit(compiler),
                                "Unsupported parse of goog.loadModule with string literal"
                            );
                            n = argument.get_last_child(compiler).unwrap();
                        }
                        "declareModuleId" => {
                            if !argument.is_string_lit(compiler) {
                                return;
                            }
                            self.provides
                                .push(argument.get_string(compiler).to_string_lossy());
                        }
                        _ => return,
                    }
                }
            }
            Token::MODULE_BODY => {
                if !parent
                    .unwrap()
                    .get_boolean_prop(compiler, Prop::GOOG_MODULE)
                {
                    self.provides.push(self.module_path.to_module_name());
                    self.load_flags.insert("module".into(), "es6".into());
                }
            }
            Token::IMPORT => {
                self.visit_es6_module_name(compiler, n.get_last_child(compiler).unwrap(), n);
                return;
            }
            Token::EXPORT => {
                if crate::node_util::NodeUtil::is_export_from(compiler, n) {
                    self.visit_es6_module_name(compiler, n.get_last_child(compiler).unwrap(), n);
                }
                return;
            }
            Token::EXPR_RESULT
            | Token::CONST
            | Token::LET
            | Token::VAR
            | Token::BLOCK
            | Token::NAME
            | Token::DESTRUCTURING_LHS => {}
            Token::SCRIPT => {
                if n.get_jsdoc_info(compiler)
                    .is_some_and(|info| info.is_provide_goog())
                {
                    self.provides.push("goog".into());
                }
            }
            _ => return,
        }
        for child in n.children(compiler) {
            self.visit_subtree(compiler, child, Some(n));
        }
    }
    // port: CompilerInput.DepsFinder#visitEs6ModuleName
    fn visit_es6_module_name(&mut self, compiler: &AbstractCompiler, n: NodeId, parent: NodeId) {
        assert!(n.is_string_lit(compiler));
        assert!(parent.is_export(compiler) || parent.is_import(compiler));
        let module_name = n.get_string(compiler).to_string_lossy();
        if let Some(symbol) = module_name.strip_prefix("goog:") {
            self.requires.push(Require::goog_require_symbol(symbol));
            return;
        }
        let imported_module = self
            .module_path
            .resolve_js_module(
                &module_name,
                Some(&self.module_path.to_string()),
                n.get_lineno(compiler),
                n.get_charno(compiler),
            )
            .unwrap_or_else(|| self.module_path.resolve_module_as_path(&module_name));
        self.requires.push(Require::es6_import(
            &imported_module.to_module_name(),
            &module_name,
        ));
    }
}
#[derive(Clone, Debug)]
pub struct CompilerInputDependencyInfo {
    pub input: CompilerInput,
    info: SimpleDependencyInfo,
}
impl PartialEq for CompilerInputDependencyInfo {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input
    }
}
impl Eq for CompilerInputDependencyInfo {}
impl Hash for CompilerInputDependencyInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.input.hash(state);
    }
}
impl DependencyInfo for CompilerInputDependencyInfo {
    fn get_name(&self) -> &str {
        self.input.get_name()
    }
    fn get_path_relative_to_closure_base(&self) -> &str {
        panic!("UnsupportedOperationException")
    }
    fn get_provides(&self) -> &[String] {
        self.info.get_provides()
    }
    fn get_requires(&self) -> &[Require] {
        self.info.get_requires()
    }
    fn get_type_requires(&self) -> &[String] {
        self.info.get_type_requires()
    }
    fn get_load_flags(&self) -> &IndexMap<String, String> {
        self.info.get_load_flags()
    }
    fn get_has_externs_annotation(&self) -> bool {
        self.info.get_has_externs_annotation()
    }
    fn get_has_no_compile_annotation(&self) -> bool {
        self.info.get_has_no_compile_annotation()
    }
}
