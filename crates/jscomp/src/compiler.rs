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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/Compiler.java.

#![allow(
    clippy::unnecessary_unwrap,
    clippy::collapsible_if,
    clippy::type_complexity
)] // Preserve Java statement order and borrowing branches.
use crate::{
    change_tracker::ChangeTracker,
    check_level::CheckLevel,
    closure_coding_convention::ClosureCodingConvention,
    coding_convention::CodingConvention,
    compiler_input::CompilerInput,
    compiler_options::{CompilerOptions, LanguageMode},
    error_manager::ErrorManager,
    js_error::JSError,
    scope::ScopeArena,
    sorting_error_manager::SortingErrorManager,
    source_file::SourceFile,
    thread_safe_delegating_error_manager::ThreadSafeDelegatingErrorManager,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    input_id::InputId,
    ir::IR,
    node::{Ast, NodeId},
    static_source_file::{SourceKind, StaticSourceFile},
};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

/// `Compiler#inputSourceMaps`: a `ConcurrentHashMap<String, SourceMapInput>`, iterated in Java's
/// order (docs/PORTING.md §8).
pub type InputSourceMaps = closure_rhino::java_lang::concurrent_hash_map::ConcurrentHashMap<
    String,
    Arc<crate::source_map_input::SourceMapInput>,
>;

pub type AstSupplier = Arc<dyn Fn(&mut Compiler) -> NodeId + Send + Sync>;
/// Java's LinkedHashSet and the anonymous AbstractSet selected by initOptions.
#[derive(Default)]
pub struct ForwardDeclaredTypes {
    values: closure_rhino::fast_hash::IndexSet<String>,
    all_types: bool,
}
impl ForwardDeclaredTypes {
    // port: Compiler.<anonymous>#contains
    pub fn contains(&self, name: &str) -> bool {
        self.all_types || self.values.contains(name)
    }
    // port: Compiler.<anonymous>#add
    pub fn add(&mut self, name: String) -> bool {
        if self.all_types {
            false
        } else {
            self.values.insert(name)
        }
    }
    // port: Compiler.<anonymous>#iterator
    pub fn iterator(&self) -> impl Iterator<Item = &String> {
        self.values.iter()
    }
    // port: Compiler.<anonymous>#size
    pub fn size(&self) -> usize {
        self.values.len()
    }
}
impl closure_jstype::js_type_registry::ForwardDeclaredTypeSet for ForwardDeclaredTypes {
    // port: Compiler.<anonymous>#contains (the Set the JSTypeRegistry shares)
    fn contains(&self, name: &closure_rhino::js_string::JsString) -> bool {
        ForwardDeclaredTypes::contains(self, &name.to_string())
    }
}
pub struct Compiler {
    typed_ast_filesystem: Option<IndexMap<usize, AstSupplier>>,
    /// Rust-only: inputs parsed ahead on worker threads (`parallel_parse`).
    pub(crate) preparser: Option<crate::parallel_parse::Preparser>,
    passes: Option<Box<dyn crate::pass_config::PassConfig>>,
    externs: Vec<CompilerInput>,
    chunk_graph: Option<Arc<crate::js_chunk_graph::JSChunkGraph>>,
    module_types_by_name: IndexMap<String, crate::compiler_input::ModuleType>,
    source_map: Option<Arc<std::sync::Mutex<crate::source_map::SourceMap>>>,
    tracker: Option<Arc<std::sync::Mutex<crate::performance_tracker::PerformanceTracker>>>,
    compiler_executor: crate::compiler_executor::CompilerExecutor,
    out_stream: Option<Arc<std::sync::Mutex<Box<dyn std::io::Write + Send>>>>,
    excerpt_provider: Arc<crate::compiler_source_excerpt_provider::CompilerSourceExcerptProvider>,
    variable_map: Option<Arc<crate::variable_map::VariableMap>>,
    property_map: Option<Arc<crate::variable_map::VariableMap>>,
    string_map: Option<Arc<crate::variable_map::VariableMap>>,
    instrumentation_mapping: Option<Arc<crate::variable_map::VariableMap>>,
    extern_exports: Option<String>,
    css_names: Option<closure_rhino::fast_hash::IndexSet<String>>,
    id_generator_map: Option<String>,
    transpiled_files: bool,
    /// Rust-only: classes of the unported pass factories that execution omitted
    /// (PassListBuilder#build filters them), in the order they were omitted.
    omitted_unported_passes: Vec<&'static str>,
    input_path_by_webpack_id: IndexMap<String, String>,
    exported_names: closure_rhino::fast_hash::IndexSet<String>,
    define_names: closure_rhino::fast_hash::IndexSet<String>,
    has_reg_exp_global_references: bool,
    run_j2cl_passes: bool,
    extern_properties: Option<closure_rhino::fast_hash::IndexSet<String>>,
    /// Rust-only: `extern_properties` as JS strings, made once per value (RemoveUnusedCode reads
    /// them on every run).
    extern_properties_js: std::sync::OnceLock<Vec<closure_rhino::js_string::JsString>>,
    /// Rust-only (DECISIONS.md D-025): whether the reference collection of the externs can be
    /// skipped, while the externs are unchanged.
    pub(crate) externs_reference_summary:
        Option<crate::reference_collector::ExternsReferenceSummary>,
    accessor_summary: Option<Arc<crate::accessor_summary::AccessorSummary>>,
    unique_name_id: Arc<std::sync::atomic::AtomicI32>,
    unique_id_supplier: crate::unique_id_supplier::UniqueIdSupplier,
    cross_chunk_id_generator: crate::id_generator::IdGenerator,
    index_providers_by_type:
        IndexMap<std::any::TypeId, Box<dyn crate::index_provider::ErasedIndexProvider>>,
    type_registry: Option<closure_jstype::JSTypeRegistry>,
    /// Rust-only: errors the type registry's reporter queued for `Compiler#report` (see
    /// `QueueingOldRhinoErrorReporter`).
    queued_type_registry_errors: Arc<crate::rhino_error_reporter::JSErrorQueue>,
    forward_declared_types: Arc<std::sync::Mutex<ForwardDeclaredTypes>>,
    type_checking_has_run: bool,
    color_registry: Option<Arc<closure_rhino::jscomp_colors::color_registry::ColorRegistry>>,
    abstract_interpreter:
        Option<std::sync::Arc<dyn crate::reverse_abstract_interpreter::ReverseAbstractInterpreter>>,
    type_validator: Option<std::sync::Arc<crate::type_validator::TypeValidator>>,
    typed_scope_creator: Option<crate::typed_scope_creator::TypedScopeCreator>,
    top_scope: Option<crate::typed_scope::TypedScope>,
    module_metadata_map: Option<Arc<crate::modules::module_metadata_map::ModuleMetadataMap>>,
    module_map: Option<Arc<crate::modules::module_map::ModuleMap>>,
    transpilation_namespace: Option<crate::global_namespace::GlobalNamespace>,
    runtime_js_lib_manager:
        Option<Arc<std::sync::Mutex<crate::js::runtime_js_lib_manager::RuntimeJsLibManager>>>,
    runtime_library_typed_asts:
        Option<IndexMap<String, Arc<dyn Fn(&mut Compiler) -> NodeId + Send + Sync>>>,
    synthetic_externs_file: Arc<SourceFile>,
    synthetic_type_summary_file: Arc<SourceFile>,
    synthetic_type_summary_input: Option<CompilerInput>,
    last_js_source: Option<closure_rhino::js_string::JsString>,
    debug_message: Option<String>,

    parser_config: Option<closure_parsing::config::Config>,
    externs_parser_config: Option<closure_parsing::config::Config>,
    comments_per_file: IndexMap<String, Vec<closure_parsing::parser::trees::comment::Comment>>,
    // Java's ConcurrentHashMap: inputSourceMaps.values() iterates in its hash-table order, which
    // decides which input map's sourcesContent is kept for a source that several maps name.
    input_source_maps: Arc<std::sync::Mutex<InputSourceMaps>>,
    script_node_by_filename: Arc<std::sync::Mutex<IndexMap<String, NodeId>>>,
    module_loader: crate::deps::module_loader::ModuleLoader,
    pending_module_errors: Arc<std::sync::Mutex<Vec<JSError>>>,
    prefer_regex_parser: bool,
    stage: crate::abstract_compiler::LifeCycleStage,
    pub(crate) options: Option<CompilerOptions>,
    pub(crate) phase_optimizer:
        Option<Arc<std::sync::Mutex<crate::phase_optimizer::ScopeChangeState>>>,
    current_pass_index: i32,
    last_pass_name: Option<String>,
    error_manager: Option<Arc<std::sync::Mutex<ThreadSafeDelegatingErrorManager>>>,
    /// Rust-only (D-025): the error manager's "no halting errors since last asked" flag
    /// (`ThreadSafeDelegatingErrorManager::no_halting_errors_flag`).
    no_halting_errors: Option<Arc<std::sync::atomic::AtomicBool>>,
    warnings_guard: Option<crate::compiler_warnings_guard::CompilerWarningsGuard>,
    externs_root: Option<NodeId>,
    js_root: Option<NodeId>,
    extern_and_js_root: Option<NodeId>,
    allowable_features: Option<FeatureSet>,
    inputs_by_id: closure_rhino::fast_hash::IndexMap<InputId, CompilerInput>,
    source_map_original_sources: Arc<std::sync::Mutex<IndexMap<String, Arc<SourceFile>>>>,
    default_coding_convention: ClosureCodingConvention,
    pub change_tracker: ChangeTracker,
    synthetic_externs_input: Option<CompilerInput>,
    pub ast: Ast,
    pub(crate) scope_arena: std::sync::Arc<std::sync::RwLock<ScopeArena>>,
    /// Rust-only: lock-free copies of the immutable fields of `scope_arena` (see `ScopeMirror`).
    pub(crate) scope_mirror: crate::scope::ScopeMirror,
    pub(crate) typed_scope_arena:
        std::sync::Arc<std::sync::RwLock<crate::typed_scope::TypedScopeArena>>,
    /// Rust-only: lock-free copies of immutable `typed_scope_arena` fields.
    pub(crate) typed_scope_mirror: Vec<crate::typed_scope::TypedScopeMeta>,
    /// Rust-only (D-025): syntactic scopes kept for reuse by later passes.
    pub(crate) syntactic_scope_cache: crate::syntactic_scope_cache::SyntacticScopeCache,
}

impl Compiler {
    // port: Compiler#Compiler()
    pub fn new() -> Self {
        let mut change_tracker = ChangeTracker::new();
        change_tracker.add_change_handler(change_tracker.get_recent_change());
        let excerpt_provider = Arc::new(
            crate::compiler_source_excerpt_provider::CompilerSourceExcerptProvider::default(),
        );
        let input_source_maps = excerpt_provider.input_source_maps.clone();
        let source_map_original_sources = excerpt_provider.original_sources.clone();
        Self {
            typed_ast_filesystem: None,
            preparser: None,
            passes: None,
            externs: Vec::new(),
            chunk_graph: None,
            module_types_by_name: IndexMap::<_, _>::default(),
            source_map: None,
            tracker: None,
            compiler_executor: crate::compiler_executor::CompilerExecutor::default(),
            out_stream: None,
            excerpt_provider,
            variable_map: None,
            property_map: None,
            string_map: None,
            instrumentation_mapping: None,
            extern_exports: None,
            css_names: None,
            id_generator_map: None,
            transpiled_files: false,
            omitted_unported_passes: Vec::new(),
            input_path_by_webpack_id: IndexMap::<_, _>::default(),
            exported_names: closure_rhino::fast_hash::IndexSet::<_>::default(),
            define_names: closure_rhino::fast_hash::IndexSet::<_>::default(),
            has_reg_exp_global_references: true,
            run_j2cl_passes: false,
            extern_properties: None,
            extern_properties_js: std::sync::OnceLock::new(),
            externs_reference_summary: None,
            accessor_summary: None,
            unique_name_id: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            unique_id_supplier: crate::unique_id_supplier::UniqueIdSupplier::default(),
            cross_chunk_id_generator: crate::id_generator::IdGenerator::default(),
            index_providers_by_type: IndexMap::<_, _>::default(),
            type_registry: None,
            queued_type_registry_errors: Arc::default(),
            forward_declared_types: Arc::new(
                std::sync::Mutex::new(ForwardDeclaredTypes::default()),
            ),
            type_checking_has_run: false,
            color_registry: None,
            abstract_interpreter: None,
            type_validator: None,
            typed_scope_creator: None,
            top_scope: None,
            module_metadata_map: None,
            module_map: None,
            transpilation_namespace: None,
            runtime_js_lib_manager: None,
            runtime_library_typed_asts: None,
            synthetic_externs_file: Arc::new(SourceFile::from_code_with_kind(
                " [synthetic:externs] ",
                "",
                SourceKind::EXTERN,
            )),
            synthetic_type_summary_file: Arc::new(SourceFile::from_code_with_kind(
                " [synthetic:typeSummary] ",
                "",
                SourceKind::EXTERN,
            )),
            synthetic_type_summary_input: None,
            last_js_source: None,
            debug_message: None,
            options: None,
            parser_config: None,
            externs_parser_config: None,
            comments_per_file: IndexMap::<_, _>::default(),
            input_source_maps,
            script_node_by_filename: Arc::new(std::sync::Mutex::new(IndexMap::<_, _>::default())),
            module_loader: crate::deps::module_loader::EMPTY.clone(),
            pending_module_errors: Arc::new(std::sync::Mutex::new(Vec::new())),
            prefer_regex_parser: false,
            stage: crate::abstract_compiler::LifeCycleStage::RAW,
            phase_optimizer: None,
            current_pass_index: -1,
            last_pass_name: None,
            error_manager: None,
            no_halting_errors: None,
            warnings_guard: None,
            externs_root: None,
            js_root: None,
            extern_and_js_root: None,
            allowable_features: None,
            inputs_by_id: Default::default(),
            source_map_original_sources,
            default_coding_convention: ClosureCodingConvention::new(),
            change_tracker,
            synthetic_externs_input: None,
            ast: Ast::new(),
            scope_arena: ScopeArena::shared(),
            scope_mirror: crate::scope::ScopeMirror::default(),
            typed_scope_arena: crate::typed_scope::TypedScopeArena::shared(),
            typed_scope_mirror: Vec::new(),
            syntactic_scope_cache: Default::default(),
        }
    }

    // port: Compiler#Compiler(ErrorManager)
    pub fn new_with_error_manager(error_manager: Box<dyn ErrorManager>) -> Self {
        let mut compiler = Self::new();
        compiler.set_error_manager(error_manager);
        compiler
    }

    // port: Compiler#setErrorManager
    pub fn set_error_manager(&mut self, error_manager: Box<dyn ErrorManager>) {
        let manager = ThreadSafeDelegatingErrorManager::new(error_manager);
        self.no_halting_errors = Some(manager.no_halting_errors_flag());
        self.error_manager = Some(Arc::new(std::sync::Mutex::new(manager)));
    }

    /// Rust-only: `getErrorManager()` as the shared handle, so another compiler can take it with
    /// `setErrorManager` (Java wraps the returned ThreadSafeDelegatingErrorManager in another one,
    /// which only delegates).
    pub fn get_error_manager_handle(
        &mut self,
    ) -> Arc<std::sync::Mutex<ThreadSafeDelegatingErrorManager>> {
        drop(self.get_error_manager());
        Arc::clone(self.error_manager.as_ref().unwrap())
    }

    /// Rust-only: `setErrorManager(other.getErrorManager())`, see `get_error_manager_handle`.
    pub fn set_error_manager_handle(
        &mut self,
        error_manager: Arc<std::sync::Mutex<ThreadSafeDelegatingErrorManager>>,
    ) {
        self.no_halting_errors = Some(error_manager.lock().unwrap().no_halting_errors_flag());
        self.error_manager = Some(error_manager);
    }

    // port: Compiler#initOptions
    pub fn init_options(&mut self, options: CompilerOptions) {
        self.allowable_features = Some(options.get_language_in().to_feature_set());
        self.options = Some(options);
        self.init_experimental_output_feature_set_options();
        if self.error_manager.is_none() {
            let formatter = self.create_message_formatter();
            if let Some(stream) = self.out_stream.clone() {
                let mut generators:Vec<Box<dyn crate::sorting_error_manager::ErrorReportGenerator>>=vec![Box::new(crate::print_stream_error_report_generator::PrintStreamErrorReportGenerator::new(formatter,SharedOutput(stream),self.get_options().get_summary_detail_level()))];
                for generator in self.get_options().get_extra_report_generators() {
                    generators.push(Box::new(SharedReportGenerator(generator.clone())));
                }
                let mut manager = SortingErrorManager::new(generators);
                manager.set_deferred_reports(self.get_deferred_reports());
                self.set_error_manager(Box::new(manager));
            } else {
                self.set_error_manager(Box::new(
                    crate::logger_error_manager::LoggerErrorManager::new(
                        formatter,
                        Box::new(CompilerLogger),
                    ),
                ));
            }
        }
        if self.get_options().get_skip_non_transpilation_passes() {
            self.get_options_mut().set_runtime_library_mode(
                crate::js::runtime_js_lib_manager::RuntimeLibraryMode::NO_OP,
            );
        }
        self.module_loader = crate::deps::module_loader::EMPTY.clone();
        self.reconcile_options_with_guards();
        let should_require_inlining =
            crate::inline_functions::InlineFunctions::check_optimization_level_supports_require_inlining(
                self.get_options(),
            );
        let should_validate = should_require_inlining.should_validate();
        let message = should_require_inlining.opt_message();
        match self.get_options().get_should_validate_required_inlinings() {
            closure_rhino::jscomp_base::Tri::TRUE => assert!(
                should_validate,
                "shouldValidateRequiredInlinings cannot be set unless CompilerOptions enables sufficient optimization, but found: {message}"
            ),
            closure_rhino::jscomp_base::Tri::FALSE => {}
            closure_rhino::jscomp_base::Tri::UNKNOWN => self
                .get_options_mut()
                .set_validate_required_inlinings(should_validate),
        }
        if !self.get_options().is_typechecking_enabled() {
            self.get_options_mut()
                .set_use_types_for_local_optimization(false);
            self.get_options_mut().set_use_types_for_optimization(false);
        }
        if self
            .get_options()
            .assume_forward_declared_for_missing_types()
        {
            self.forward_declared_types = Arc::new(std::sync::Mutex::new(ForwardDeclaredTypes {
                values: closure_rhino::fast_hash::IndexSet::<_>::default(),
                all_types: true,
            }));
        }
        self.init_warnings_guard(self.get_options().get_warnings_guard().clone());
        if self.is_debug_logging_enabled() {
            self.get_options_mut().set_print_config(true);
        }
        self.prefer_regex_parser |= self.get_options().get_dependency_options().should_prune();
        assert!(
            !(self.get_options().get_merged_precompiled_libraries()
                && self
                    .get_options()
                    .get_dependency_options()
                    .needs_management()),
            "Using precompiled libraries (i.e. TypedAST) is incompatible with flags that automatically order/prune dependencies: {}",
            self.get_options().get_dependency_options()
        );
        if self.get_options().get_merged_precompiled_libraries()
            && !self.get_options().get_conformance_configs().is_empty()
        {
            let mut conformance_config_files = closure_rhino::fast_hash::IndexSet::<_>::default();
            for config in self.get_options().get_conformance_configs() {
                for requirement in config.get_requirement_list() {
                    conformance_config_files
                        .extend(requirement.get_config_file_list().iter().cloned());
                }
            }
            panic!(
                "use_precompiled_libraries mode does not support passing JS conformance configs: this mode skips all check passes including conformance\nUnsupported conformance configs: [{}]",
                conformance_config_files
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if self.get_options().should_run_dead_assignment_elimination()
            && !(self.get_options().should_remove_unused_variables()
                || self.get_options().should_remove_unused_local_variables())
        {
            panic!(
                "Invalid flag combination: enabling dead assignment elimination also requires unused variable removal. Either disable dead assignment elimination or enable unused variable removal."
            );
        }
        if self
            .get_options()
            .should_run_dead_property_assignment_elimination()
            && self.get_options().is_polymer_pass_enabled()
        {
            panic!(
                "Invalid flag combination: dead assignment elimination is incompatible with setPolymerPass(true)."
            );
        }
    }

    // port: Compiler#init
    pub fn init(
        &mut self,
        externs: &[Arc<SourceFile>],
        sources: &[Arc<SourceFile>],
        options: CompilerOptions,
    ) {
        let chunk = crate::js_chunk::JSChunk::new(crate::js_chunk::STRONG_CHUNK_NAME);
        for source in sources {
            chunk.add(CompilerInput::new_with_extern(source.clone(), false));
        }
        self.init_chunks(externs, vec![chunk], options);
        self.add_files_to_source_map(sources);
    }

    // port: Compiler#getErrors
    pub fn get_errors(&self) -> Vec<JSError> {
        self.report_queued_type_registry_errors_to_manager();
        self.error_manager
            .as_ref()
            .map_or_else(Vec::new, |manager| manager.lock().unwrap().get_errors())
    }

    // port: Compiler#getWarnings
    pub fn get_warnings(&self) -> Vec<JSError> {
        self.report_queued_type_registry_errors_to_manager();
        self.error_manager
            .as_ref()
            .map_or_else(Vec::new, |manager| manager.lock().unwrap().get_warnings())
    }

    // port: Compiler#getRoot
    pub fn get_root(&self) -> Option<NodeId> {
        self.extern_and_js_root
    }

    // port: Compiler#getAllowableFeatures
    pub fn get_allowable_features(&self) -> FeatureSet {
        self.allowable_features.expect("")
    }

    // port: Compiler#setAllowableFeatures
    pub fn set_allowable_features(&mut self, allowable_features: FeatureSet) {
        self.allowable_features = Some(allowable_features);
    }

    // port: Compiler#markFeatureNotAllowed
    pub fn mark_feature_not_allowed(&mut self, feature: Feature) {
        self.allowable_features = Some(self.get_allowable_features().without(feature));
    }

    // port: Compiler#markFeatureSetNotAllowed
    pub fn mark_feature_set_not_allowed(&mut self, feature_set: FeatureSet) {
        self.allowable_features = Some(self.get_allowable_features().without(feature_set));
    }

    // port: Compiler#getInput
    pub fn get_input(&self, id: &InputId) -> Option<&CompilerInput> {
        self.inputs_by_id.get(id)
    }

    // port: Compiler#putCompilerInput
    pub fn put_compiler_input(&mut self, input: CompilerInput) -> Option<CompilerInput> {
        input.set_compiler(self);
        self.excerpt_provider
            .files
            .lock()
            .unwrap()
            .insert(input.get_name().into(), input.get_source_file_arc());
        self.inputs_by_id
            .insert(input.get_input_id().clone(), input)
    }

    // port: Compiler#newCompilerOptions
    pub fn new_compiler_options(&self) -> CompilerOptions {
        CompilerOptions::new()
    }

    // port: Compiler#initCompilerOptionsIfTesting
    pub fn init_compiler_options_if_testing(&mut self) {
        if self.options.is_none() {
            self.init_options(self.new_compiler_options());
            self.options
                .as_mut()
                .expect("")
                .set_language(LanguageMode::UNSUPPORTED);
        }
    }

    // port: Compiler#getChangeTracker
    pub fn get_change_tracker(&mut self) -> &mut ChangeTracker {
        &mut self.change_tracker
    }

    // port: Compiler#getExternsRoot
    pub fn get_externs_root(&self) -> Option<NodeId> {
        self.externs_root
    }

    // port: Compiler#getJsRoot
    pub fn get_js_root(&self) -> Option<NodeId> {
        self.js_root
    }

    // port: Compiler#hasScopeChanged
    pub fn has_scope_changed(&self, n: NodeId) -> bool {
        self.phase_optimizer.as_ref().is_none_or(|p| {
            let state = p.lock().unwrap();
            !state.in_loop
                || state.time_of_last_run == 0
                || n.get_change_time(self) > state.time_of_last_run
        })
    }

    // port: Compiler#reportChangeToChangeScope
    pub fn report_change_to_change_scope(&mut self, change_scope_root: NodeId) {
        self.change_tracker
            .report_change_to_change_scope(&mut self.ast, change_scope_root);
    }

    // port: Compiler#reportFunctionDeleted
    pub fn report_function_deleted(&mut self, n: NodeId) {
        self.change_tracker
            .report_function_deleted(&mut self.ast, n);
    }

    // port: Compiler#reportChangeToEnclosingScope
    pub fn report_change_to_enclosing_scope(&mut self, n: NodeId) {
        self.change_tracker
            .report_change_to_enclosing_scope(&mut self.ast, n);
    }

    // port: Compiler#getCodingConvention
    pub fn get_coding_convention(&self) -> &dyn CodingConvention {
        self.get_options()
            .get_coding_convention_ref()
            .unwrap_or(&self.default_coding_convention)
    }

    // port: Compiler#report(JSError)
    pub fn report(&mut self, error: JSError) {
        self.report_queued_type_registry_errors();
        self.report_now(error);
    }
    /// Rust-only: the body of `Compiler#report`.
    fn report_now(&mut self, error: JSError) {
        let level = self.get_error_level(&error);
        if level.is_on() {
            self.init_compiler_options_if_testing();
            // Shared mutable ErrorHandler integration is completed with the options adapter.
            if let Some(handler) = self.get_options().get_error_handler().clone() {
                handler.lock().unwrap().report(level, error.clone());
            }
            self.error_manager
                .as_ref()
                .expect("")
                .lock()
                .unwrap()
                .report(level, error);
        }
    }

    // port: Compiler#report(CheckLevel,JSError)
    pub fn report_with_level(&mut self, _ignored_level: CheckLevel, error: JSError) {
        self.report(error);
    }

    // port: Compiler#throwInternalError
    pub fn throw_internal_error(&self, message: &str, _cause: &str) -> ! {
        panic!("INTERNAL COMPILER ERROR.\nPlease report this problem.\n\n{message}");
    }

    /// Rust-only: reports, in order, the errors the type registry's reporter queued
    /// (`QueueingOldRhinoErrorReporter`), as Java's reporter would have through `Compiler#report`.
    pub fn report_queued_type_registry_errors(&mut self) {
        let queued = self.queued_type_registry_errors.take();
        for error in queued {
            self.report_now(error);
        }
    }
    /// Rust-only: the queue behind the registry's reporter (`QueueingOldRhinoErrorReporter`), for
    /// other `Compiler#report` calls made from registry callbacks, where no `&mut Compiler` is in
    /// reach (FunctionTypeBuilder's @extends/@implements validators). Queued errors are reported
    /// in order before the Compiler next reports or reads errors.
    pub(crate) fn type_registry_error_queue(
        &self,
    ) -> Arc<crate::rhino_error_reporter::JSErrorQueue> {
        Arc::clone(&self.queued_type_registry_errors)
    }
    /// Rust-only: `report_queued_type_registry_errors` for the `&self` readers of the error
    /// manager. Before the options exist there is no error manager to read, so the queue waits for
    /// the next `report_queued_type_registry_errors`.
    fn report_queued_type_registry_errors_to_manager(&self) {
        if self.options.is_none() || self.error_manager.is_none() {
            return;
        }
        let queued = self.queued_type_registry_errors.take();
        for error in queued {
            // Compiler#report
            let level = self.get_error_level(&error);
            if level.is_on() {
                if let Some(handler) = self.get_options().get_error_handler().clone() {
                    handler.lock().unwrap().report(level, error.clone());
                }
                self.error_manager
                    .as_ref()
                    .expect("")
                    .lock()
                    .unwrap()
                    .report(level, error);
            }
        }
    }
    // port: Compiler#getErrorCount
    pub fn get_error_count(&self) -> i32 {
        self.report_queued_type_registry_errors_to_manager();
        self.error_manager
            .as_ref()
            .expect("")
            .lock()
            .unwrap()
            .get_error_count()
    }

    // port: Compiler#getWarningCount
    pub fn get_warning_count(&self) -> i32 {
        self.report_queued_type_registry_errors_to_manager();
        self.error_manager
            .as_ref()
            .expect("")
            .lock()
            .unwrap()
            .get_warning_count()
    }

    // port: Compiler#getSourceFileByName
    pub fn get_source_file_by_name(&self, source_name: &str) -> Option<Arc<SourceFile>> {
        if let Some(input) = self.inputs_by_id.get(&InputId::new(source_name)) {
            return Some(input.get_source_file_arc());
        }
        self.source_map_original_sources
            .lock()
            .unwrap()
            .get(source_name)
            .cloned()
    }

    // port: Compiler#getSourceLine
    pub fn get_source_line(&self, source_name: &str, line_number: i32) -> Option<String> {
        if line_number < 1 {
            return None;
        }
        if let Some(input) = self.get_source_file_by_name(source_name)
            && !input.is_stub_source_file_for_already_provided_input()
        {
            return input.get_line(line_number);
        }
        None
    }

    // port: Compiler#getOptions
    pub fn get_options(&self) -> &CompilerOptions {
        self.options.as_ref().expect("")
    }

    /// Retains Java's nullable options lookup for traversal construction.
    pub fn get_options_opt(&self) -> Option<&CompilerOptions> {
        self.options.as_ref()
    }

    // port: Compiler#getSynthesizedExternsInput
    pub fn get_synthesized_externs_input(&mut self) -> &CompilerInput {
        if self.synthetic_externs_input.is_some() {
            return self.synthetic_externs_input.as_ref().unwrap();
        }
        let input = CompilerInput::new_with_extern(self.synthetic_externs_file.clone(), true);
        let root = input.get_ast_root(self);
        self.put_compiler_input(input.clone());
        self.synthetic_externs_input = Some(input.clone());
        self.externs_root.unwrap().add_child_to_front(self, root);
        self.externs.insert(0, input.clone());
        self.script_node_by_filename
            .lock()
            .unwrap()
            .insert(input.get_source_file().get_name().into(), root);
        self.synthetic_externs_input.as_ref().unwrap()
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}
impl Deref for Compiler {
    type Target = Ast;
    fn deref(&self) -> &Ast {
        &self.ast
    }
}
impl DerefMut for Compiler {
    fn deref_mut(&mut self) -> &mut Ast {
        &mut self.ast
    }
}

impl Compiler {
    pub fn get_change_tracker_ref(&self) -> &ChangeTracker {
        &self.change_tracker
    }
    pub fn get_options_mut(&mut self) -> &mut CompilerOptions {
        self.options.as_mut().unwrap()
    }
    // port: Compiler#getErrorManager
    pub fn get_error_manager(
        &mut self,
    ) -> std::sync::MutexGuard<'_, ThreadSafeDelegatingErrorManager> {
        if self.options.is_none() {
            self.init_options(CompilerOptions::new());
        }
        self.report_queued_type_registry_errors();
        self.error_manager.as_ref().unwrap().lock().unwrap()
    }
    // port: Compiler#hasHaltingErrors
    pub fn has_halting_errors(&self) -> bool {
        self.report_queued_type_registry_errors_to_manager();
        if self.get_options().can_continue_after_errors() {
            return false;
        }
        if self
            .no_halting_errors
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
        {
            return false;
        }
        self.error_manager
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .has_halting_errors()
    }
    // port: Compiler#hasErrors
    pub fn has_errors(&self) -> bool {
        self.has_halting_errors()
    }
    // port: Compiler#beforePass
    pub fn before_pass(&mut self, _pass_name: &str) {
        self.current_pass_index = self.current_pass_index.wrapping_add(1);
    }
    /// Rust-only: the number of the running pass (see `SyntacticScopeCache`).
    pub(crate) fn current_pass_index(&self) -> i32 {
        self.current_pass_index
    }
    // port: Compiler#afterPass
    pub fn after_pass(&mut self, pass_name: &str) {
        self.maybe_print_source_after_each_pass(pass_name);
    }
    // port: Compiler#getLastPassName
    pub fn get_last_pass_name(&self) -> Option<&str> {
        self.last_pass_name.as_deref()
    }
    pub fn to_source_for_node(&mut self, root: NodeId) -> String {
        self.to_source_for_node_utf16(root).to_string_lossy()
    }
}

impl Compiler {
    // port: Compiler#getErrorManager (the shared ThreadSafeDelegatingErrorManager object)
    pub fn get_shared_error_manager(
        &mut self,
    ) -> crate::deps::js_file_line_parser::SharedErrorManager {
        if self.options.is_none() {
            self.init_options(CompilerOptions::new());
        }
        self.report_queued_type_registry_errors();
        self.error_manager.as_ref().unwrap().clone()
    }
    // port: Compiler#getParserConfigLanguageMode
    fn get_parser_config_language_mode(
        &self,
        language_mode: LanguageMode,
    ) -> closure_parsing::config::LanguageMode {
        use closure_parsing::config::LanguageMode as Mode;
        match language_mode {
            LanguageMode::ECMASCRIPT3 => Mode::ECMASCRIPT3,
            LanguageMode::ECMASCRIPT5 | LanguageMode::ECMASCRIPT5_STRICT => Mode::ECMASCRIPT5,
            LanguageMode::ECMASCRIPT_2015 => Mode::ECMASCRIPT_2015,
            LanguageMode::ECMASCRIPT_2016 => Mode::ECMASCRIPT_2016,
            LanguageMode::ECMASCRIPT_2017 => Mode::ECMASCRIPT_2017,
            LanguageMode::ECMASCRIPT_2018 => Mode::ECMASCRIPT_2018,
            LanguageMode::ECMASCRIPT_2019 => Mode::ECMASCRIPT_2019,
            LanguageMode::ECMASCRIPT_2020 => Mode::ECMASCRIPT_2020,
            LanguageMode::ECMASCRIPT_2021 => Mode::ECMASCRIPT_2021,
            LanguageMode::ECMASCRIPT_NEXT => Mode::ES_NEXT,
            LanguageMode::UNSTABLE => Mode::UNSTABLE,
            LanguageMode::UNSUPPORTED => Mode::UNSUPPORTED,
            _ => panic!(
                "Unexpected language mode: {}",
                self.get_options().get_language_in()
            ),
        }
    }
    // port: Compiler#getParserConfig
    pub fn get_parser_config(
        &mut self,
        context: crate::abstract_compiler::ConfigContext,
    ) -> closure_parsing::config::Config {
        use closure_parsing::config::{LanguageMode as Mode, StrictMode};
        if self.parser_config.is_none() || self.externs_parser_config.is_none() {
            if self.parser_config.is_none() {
                let config_language_mode =
                    self.get_parser_config_language_mode(self.get_options().get_language_in());
                let strict_mode = if self.get_options().expect_strict_mode_input() {
                    StrictMode::STRICT
                } else {
                    StrictMode::SLOPPY
                };
                let parser_config = self.create_config(config_language_mode, strict_mode);
                self.externs_parser_config = Some(if config_language_mode == Mode::ECMASCRIPT3 {
                    self.create_config(Mode::ECMASCRIPT5, strict_mode)
                } else {
                    parser_config.clone()
                });
                self.parser_config = Some(parser_config);
            }
        }
        if context == crate::abstract_compiler::ConfigContext::EXTERNS {
            self.externs_parser_config.as_ref().unwrap().clone()
        } else {
            self.parser_config.as_ref().unwrap().clone()
        }
    }
    // port: Compiler#createConfig
    pub fn create_config(
        &self,
        mode: closure_parsing::config::LanguageMode,
        strict_mode: closure_parsing::config::StrictMode,
    ) -> closure_parsing::config::Config {
        use closure_parsing::{config::RunMode, parser_runner::ParserRunner};
        let annotations = self
            .get_options()
            .get_extra_annotation_names()
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str().into()).collect());
        ParserRunner::create_config_full(
            mode,
            self.get_options().is_parse_js_doc_documentation(),
            if self.get_options().can_continue_after_errors() {
                RunMode::KEEP_GOING
            } else {
                RunMode::STOP_AFTER_ERROR
            },
            annotations.as_ref(),
            self.get_options().get_parse_inline_source_maps(),
            strict_mode,
        )
    }
    // port: Compiler#getDiagnosticGroups
    pub fn get_diagnostic_groups(&self) -> crate::diagnostic_groups::DiagnosticGroups {
        crate::diagnostic_groups::DiagnosticGroups::new()
    }
    // port: Compiler#preferRegexParser
    pub fn prefer_regex_parser(&self) -> bool {
        self.prefer_regex_parser
    }
    // port: Compiler#setPreferRegexParser
    pub fn set_prefer_regex_parser(&mut self, value: bool) {
        self.prefer_regex_parser = value;
    }
    // port: Compiler#getModuleLoader
    pub fn get_module_loader(&self) -> &crate::deps::module_loader::ModuleLoader {
        &self.module_loader
    }
    // port: Compiler#addComments
    pub fn add_comments(
        &mut self,
        filename: &str,
        comments: Vec<closure_parsing::parser::trees::comment::Comment>,
    ) {
        assert!(
            self.get_options().preserves_detailed_source_info(),
            "addComments may only be called in IDE mode."
        );
        self.comments_per_file.insert(filename.into(), comments);
    }
    // port: Compiler#getComments
    pub fn get_comments(
        &self,
        filename: &str,
    ) -> Option<&[closure_parsing::parser::trees::comment::Comment]> {
        assert!(
            self.get_options().preserves_detailed_source_info(),
            "getComments may only be called in IDE mode."
        );
        self.comments_per_file.get(filename).map(Vec::as_slice)
    }
    // port: Compiler#addInputSourceMap
    pub fn add_input_source_map(
        &mut self,
        name: &str,
        source_map: crate::source_map_input::SourceMapInput,
    ) {
        let source_map = Arc::new(source_map);
        self.input_source_maps
            .lock()
            .unwrap()
            .put(name.into(), source_map.clone());
        if self.get_options().get_source_map_include_sources_content() && self.source_map.is_some()
        {
            self.add_source_map_source_files(&source_map);
        }
    }
    // port: Compiler#getLifeCycleStage
    pub fn get_life_cycle_stage(&self) -> crate::abstract_compiler::LifeCycleStage {
        self.stage
    }
    // port: Compiler#setLifeCycleStage
    pub fn set_life_cycle_stage(&mut self, stage: crate::abstract_compiler::LifeCycleStage) {
        self.stage = stage;
    }
    // port: Compiler#getScriptNode
    pub fn get_script_node(&self, filename: &str) -> Option<NodeId> {
        self.script_node_by_filename
            .lock()
            .unwrap()
            .get(filename)
            .copied()
    }
    // port: Compiler#parseCodeHelper(SourceFile)
    fn parse_code_helper(&mut self, src: Arc<SourceFile>) -> NodeId {
        let input = CompilerInput::new(src);
        self.put_compiler_input(input.clone());
        let root = input.get_ast_root(self);
        self.script_node_by_filename
            .lock()
            .unwrap()
            .insert(input.get_source_file().get_name().into(), root);
        root
    }
    // port: Compiler#parseCodeHelper(List<SourceFile>)
    fn parse_code_helper_many(&mut self, srcs: Vec<Arc<SourceFile>>) -> NodeId {
        let root = IR::root(self, &[]);
        for src in srcs {
            let script = self.parse_code_helper(src);
            root.add_child_to_back(self, script);
        }
        root
    }
    // port: Compiler#parseTestCode(String)
    pub fn parse_test_code(&mut self, js: impl Into<closure_rhino::js_string::JsString>) -> NodeId {
        self.init_compiler_options_if_testing();
        self.init_based_on_options();
        self.parse_code_helper(Arc::new(SourceFile::from_code("testcode", js)))
    }
    // port: Compiler#parseTestCode(ImmutableList<String>)
    pub fn parse_test_code_many(&mut self, js: Vec<closure_rhino::js_string::JsString>) -> NodeId {
        self.init_compiler_options_if_testing();
        self.init_based_on_options();
        self.parse_code_helper_many(
            js.into_iter()
                .enumerate()
                .map(|(index, value)| {
                    Arc::new(SourceFile::from_code(&format!("testcode{index}"), value))
                })
                .collect(),
        )
    }
    // port: Compiler#parseSyntheticCode
    pub fn parse_synthetic_code(
        &mut self,
        filename: &str,
        js: impl Into<closure_rhino::js_string::JsString>,
    ) -> NodeId {
        self.init_compiler_options_if_testing();
        let source = Arc::new(SourceFile::from_code(
            &format!("{SYNTHETIC_FILE_NAME_PREFIX}{filename}] "),
            js,
        ));
        self.add_files_to_source_map(std::slice::from_ref(&source));
        self.parse_code_helper(source)
    }
    // port: Compiler#getDefaultErrorReporter
    pub fn get_default_error_reporter(
        &mut self,
    ) -> crate::rhino_error_reporter::OldRhinoErrorReporter<'_> {
        crate::rhino_error_reporter::RhinoErrorReporter::for_old_rhino(self)
    }
}
impl crate::error_handler::ErrorHandler for Compiler {
    // port: Compiler#report(CheckLevel, JSError)
    fn report(&mut self, _ignored_level: CheckLevel, error: JSError) {
        Compiler::report(self, error);
    }
}

struct SharedOutput(Arc<std::sync::Mutex<Box<dyn std::io::Write + Send>>>);
impl std::io::Write for SharedOutput {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}
struct SharedReportGenerator(
    Arc<std::sync::Mutex<dyn crate::sorting_error_manager::ErrorReportGenerator + Send>>,
);
impl crate::sorting_error_manager::ErrorReportGenerator for SharedReportGenerator {
    fn generate_report(&mut self, manager: &mut SortingErrorManager, ast: &Ast) {
        self.0.lock().unwrap().generate_report(manager, ast);
    }
}
struct CompilerLogger;
impl CompilerLogger {
    // port: Logger#isLoggable
    fn is_loggable(level: crate::logger_error_manager::Level) -> bool {
        let level_value = LOGGING_LEVEL.load(std::sync::atomic::Ordering::Relaxed);
        !(level.int_value() < level_value
            || level_value == crate::logger_error_manager::Level::OFF.int_value())
    }
}
impl crate::logger_error_manager::Logger for CompilerLogger {
    fn severe(&mut self, message: &str) {
        self.log(crate::logger_error_manager::Level::SEVERE, message);
    }
    fn warning(&mut self, message: &str) {
        self.log(crate::logger_error_manager::Level::WARNING, message);
    }
    fn log(&mut self, level: crate::logger_error_manager::Level, message: &str) {
        // The default ConsoleHandler publishes records at INFO and above.
        if Self::is_loggable(level)
            && level.int_value() >= crate::logger_error_manager::Level::INFO.int_value()
        {
            eprintln!("{message}");
        }
    }
}
#[derive(Default)]
pub struct CodeBuilder {
    sb: Vec<u16>,
    line_count: i32,
    col_count: i32,
}
impl CodeBuilder {
    // port: Compiler.CodeBuilder#reset
    pub fn reset(&mut self) {
        self.sb.clear();
    }
    // port: Compiler.CodeBuilder#append
    pub fn append(&mut self, text: impl Into<closure_rhino::js_string::JsString>) -> &mut Self {
        let text = text.into();
        self.sb.extend_from_slice(text.as_units());
        let mut last_index = None;
        for (i, c) in text.as_units().iter().enumerate() {
            if *c == 10 {
                self.line_count = self.line_count.wrapping_add(1);
                last_index = Some(i);
            }
        }
        if let Some(index) = last_index {
            self.col_count = (text.length() - index - 1) as i32;
        } else {
            self.col_count = self.col_count.wrapping_add(text.length() as i32);
        }
        self
    }
    // port: Compiler.CodeBuilder#toString
    pub fn to_js_string(&self) -> closure_rhino::js_string::JsString {
        closure_rhino::js_string::JsString::from_units(self.sb.clone())
    }
    // port: Compiler.CodeBuilder#getLength
    pub fn get_length(&self) -> usize {
        self.sb.len()
    }
    // port: Compiler.CodeBuilder#getLineIndex
    pub fn get_line_index(&self) -> i32 {
        self.line_count
    }
    // port: Compiler.CodeBuilder#getColumnIndex
    pub fn get_column_index(&self) -> i32 {
        self.col_count
    }
    // port: Compiler.CodeBuilder#endsWith
    pub fn ends_with(&self, suffix: &str) -> bool {
        let suffix: Vec<_> = suffix.encode_utf16().collect();
        self.sb.len() > suffix.len() && self.sb.ends_with(&suffix)
    }
}

pub static CHUNK_DEPENDENCY_ERROR: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error(
        "JSC_CHUNK_DEPENDENCY_ERROR",
        "Bad dependency: {0} -> {1}. Chunks must be listed in dependency order.",
    );
pub static MISSING_ENTRY_ERROR: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error(
        "JSC_MISSING_ENTRY_ERROR",
        "required entry point \"{0}\" never provided",
    );
pub static MISSING_CHUNK_ERROR: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error(
        "JSC_MISSING_CHUNK_ERROR",
        "unknown chunk \"{0}\" specified in entry point spec",
    );
pub static EMPTY_CHUNK_LIST_ERROR: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error(
        "JSC_EMPTY_CHUNK_LIST_ERROR",
        "At least one chunk must be provided",
    );
pub static EMPTY_ROOT_CHUNK_ERROR: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error(
        "JSC_EMPTY_ROOT_CHUNK_ERROR",
        "Root chunk ''{0}'' must contain at least one source code input",
    );
pub static DUPLICATE_INPUT: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error("JSC_DUPLICATE_INPUT", "Duplicate input: {0}");
pub static DUPLICATE_EXTERN_INPUT: crate::diagnostic_type::DiagnosticType =
    crate::diagnostic_type::DiagnosticType::error(
        "JSC_DUPLICATE_EXTERN_INPUT",
        "Duplicate extern input: {0}",
    );
impl Compiler {
    // port: Compiler#Compiler(PrintStream)
    pub fn new_with_output_stream(out: Option<Box<dyn std::io::Write + Send>>) -> Self {
        let mut compiler = Self::new();
        compiler.out_stream = out.map(|out| Arc::new(std::sync::Mutex::new(out)));
        compiler
    }
    /// Rust adapter for Java callers that pass the Compiler as its SourceExcerptProvider.
    pub fn get_source_excerpt_provider(
        &self,
    ) -> Arc<dyn crate::source_excerpt_provider::SourceExcerptProvider> {
        self.excerpt_provider.clone()
    }
    /// Rust adapter: the queue this Compiler, as the formatters' and report generators'
    /// SourceExcerptProvider, reports into (see `sorting_error_manager::DeferredReports`).
    pub fn get_deferred_reports(&self) -> crate::sorting_error_manager::DeferredReports {
        self.excerpt_provider.pending_errors.clone()
    }
    /// Rust adapter: a Send writer sharing Compiler#outStream (the runner's error print stream).
    pub fn get_out_stream_writer(&self) -> Option<Box<dyn std::io::Write + Send>> {
        self.out_stream
            .clone()
            .map(|stream| Box::new(SharedOutput(stream)) as Box<dyn std::io::Write + Send>)
    }
    // port: Compiler#createMessageFormatter
    fn create_message_formatter(&self) -> Box<dyn crate::message_formatter::MessageFormatter> {
        self.get_options().get_error_format().to_formatter(
            Some(self.excerpt_provider.clone()),
            self.get_options().should_colorize_error_output(),
        )
    }
    // port: Compiler#initExperimentalOutputFeatureSetOptions
    fn init_experimental_output_feature_set_options(&mut self) {
        use crate::compiler_options::ExperimentalOutputFeatureSet;
        let set = self.get_options().get_output_feature_set();
        if set == FeatureSet::ES5 || set == FeatureSet::ES3 {
            return;
        }
        if let Some(set) = self.get_options().get_experimental_output_feature_set() {
            let features=match set {
                ExperimentalOutputFeatureSet::ES5_WITH_SOME_PERFORMANT_ES2015_AND_ASYNC_FUNCTIONS=>FeatureSet::ES5.with(Feature::ASYNC_FUNCTIONS).with(Feature::BLOCK_SCOPED_FUNCTION_DECLARATION).with(Feature::CONST_DECLARATIONS).with(Feature::GENERATORS).with(Feature::LET_DECLARATIONS),
                ExperimentalOutputFeatureSet::BROWSER_2019_WITHOUT_CLASSES_AND_SPREAD=>FeatureSet::BROWSER_2019.without(Feature::CLASSES).without(Feature::CLASS_GETTER_SETTER).without(Feature::NEW_TARGET).without(Feature::SUPER).without(Feature::SPREAD_EXPRESSIONS),
            };
            self.get_options_mut().set_output_feature_set(features);
        }
    }
    // port: Compiler#initWarningsGuard
    fn init_warnings_guard(
        &mut self,
        guard: Arc<crate::compose_warnings_guard::ComposeWarningsGuard>,
    ) {
        self.warnings_guard = Some(crate::compiler_warnings_guard::CompilerWarningsGuard::new(
            guard,
        ));
    }
    // port: Compiler#reconcileOptionsWithGuards
    pub fn reconcile_options_with_guards(&mut self) {
        use crate::diagnostic_groups as groups;
        use closure_rhino::jscomp_base::Tri;
        let options = self.get_options_mut();
        if options.enables(&groups::CHECK_TYPES) {
            options.set_check_types(true);
        } else if options.disables(&groups::CHECK_TYPES) {
            options.set_check_types(false);
        } else if !options.get_check_types() {
            options.set_warning_level(
                crate::diagnostic_group::DiagnosticGroup::for_type(
                    &crate::rhino_error_reporter::TYPE_PARSE_ERROR,
                ),
                CheckLevel::OFF,
            );
        }
        if !options.get_check_types() {
            options.set_warning_level(groups::BOUNDED_GENERICS.clone(), CheckLevel::OFF);
        }
        if !options.get_check_symbols() && !options.enables(&groups::CHECK_VARIABLES) {
            options.set_warning_level(groups::CHECK_VARIABLES.clone(), CheckLevel::OFF);
        }
        if options.get_skip_non_transpilation_passes() && !options.enables(&groups::CHECK_VARIABLES)
        {
            options.set_warning_level(groups::CHECK_VARIABLES.clone(), CheckLevel::OFF);
        }
        if options.get_skip_non_transpilation_passes() && !options.enables(&groups::MISSING_PROVIDE)
        {
            options.set_warning_level(groups::MISSING_PROVIDE.clone(), CheckLevel::OFF);
        }
        if options.should_disambiguate_properties()
            && crate::warnings_guard::WarningsGuard::must_run_checks(
                options.get_warnings_guard().as_ref(),
                &groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION,
            ) == Tri::UNKNOWN
        {
            options.set_warning_level(
                groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION.clone(),
                CheckLevel::ERROR,
            );
        }
    }
    // port: Compiler#getErrorLevel
    pub fn get_error_level(&self, error: &JSError) -> CheckLevel {
        self.warnings_guard
            .as_ref()
            .and_then(|guard| guard.level(self, error))
            .unwrap_or(error.default_level)
    }
    // port: Compiler#initChunks
    pub fn init_chunks(
        &mut self,
        externs: &[Arc<SourceFile>],
        chunks: Vec<crate::js_chunk::JSChunk>,
        options: CompilerOptions,
    ) {
        self.init_options(options);
        self.check_first_chunk(&chunks);
        self.externs.clear();
        for file in externs {
            self.externs
                .push(CompilerInput::new_with_extern(file.clone(), true));
        }
        match crate::js_chunk_graph::JSChunkGraph::new(chunks) {
            Ok(graph) => self.chunk_graph = Some(Arc::new(graph)),
            Err(error) => {
                self.report(JSError::make_without_location(
                    &CHUNK_DEPENDENCY_ERROR,
                    &[
                        &error.get_chunk().get_name(),
                        &error.get_dependent_chunk().get_name(),
                    ],
                ));
                return;
            }
        }
        self.fill_empty_chunks();
        self.comments_per_file.clear();
        self.init_based_on_options();
        self.init_inputs_by_id_map();
        self.init_ast();
        if self.is_debug_logging_enabled() || self.get_options().should_print_config() {
            self.print_config();
        }
        let mut chunk_graph_log = self.create_or_reopen_log("Compiler", "chunk_graph.dot", &[]);
        chunk_graph_log.log(&mut || {
            crate::dot_formatter::DotFormatter::to_dot_graph(
                &self.chunk_graph.as_ref().unwrap().to_graphviz_graph(),
            )
        });
        self.runtime_js_lib_manager = Some(Arc::new(std::sync::Mutex::new(
            crate::js::runtime_js_lib_manager::RuntimeJsLibManager::create(
                self.get_options().get_runtime_library_mode(),
                Box::new(|compiler: &mut Compiler, resource_name: &str, path: &str| {
                    Some(compiler.load_resource_contents(resource_name, path))
                }),
                Self::get_change_tracker_and_ast,
                if self.get_options().get_runtime_library_mode()
                    == crate::js::runtime_js_lib_manager::RuntimeLibraryMode::EXTERN_FIELD_NAMES
                {
                    Box::new(Self::get_synthesized_externs_root)
                } else {
                    Box::new(Self::get_node_for_runtime_code_insertion)
                },
            ),
        )));
    }
    // port: Compiler#checkFirstChunk
    fn check_first_chunk(&mut self, chunks: &[crate::js_chunk::JSChunk]) {
        if chunks.is_empty() {
            self.report(JSError::make_without_location(&EMPTY_CHUNK_LIST_ERROR, &[]));
        } else if chunks[0].get_inputs().is_empty() && chunks.len() > 1 {
            self.report(JSError::make_without_location(
                &EMPTY_ROOT_CHUNK_ERROR,
                &[&chunks[0].get_name()],
            ));
        }
    }
    // port: Compiler#joinPathParts
    pub fn join_path_parts(parts: &[&str]) -> String {
        parts.join(crate::platform::Platform::get_file_seperator())
    }
    // port: Compiler#fillEmptyChunks
    fn fill_empty_chunks(&mut self) {
        for chunk in self.get_chunks().unwrap() {
            if !chunk.is_weak() && chunk.get_inputs().is_empty() {
                let input = CompilerInput::new(SourceFile::from_code(
                    &Self::create_fill_file_name(&chunk.get_name()),
                    "",
                ));
                input.set_compiler(self);
                chunk.add(input);
            }
        }
    }
    // port: Compiler#rebuildInputsFromChunks
    pub fn rebuild_inputs_from_chunks(&mut self) {
        self.init_inputs_by_id_map();
    }
    // port: Compiler#initInputsByIdMap
    fn init_inputs_by_id_map(&mut self) {
        self.inputs_by_id.clear();
        for input in self.externs.clone() {
            if self.put_compiler_input(input.clone()).is_some() {
                self.report(JSError::make_without_location(
                    &DUPLICATE_EXTERN_INPUT,
                    &[input.get_name()],
                ));
            }
        }
        for input in self.get_inputs_in_order() {
            if self.put_compiler_input(input.clone()).is_some() {
                self.report(JSError::make_without_location(
                    &DUPLICATE_INPUT,
                    &[input.get_name()],
                ));
            }
        }
    }
    // port: Compiler#initAST
    fn init_ast(&mut self) {
        let js = IR::root(self, &[]);
        let externs = IR::root(self, &[]);
        self.js_root = Some(js);
        self.externs_root = Some(externs);
        self.change_tracker.set_externs_root(externs);
        self.externs_reference_summary = None;
        self.extern_and_js_root = Some(IR::root(self, &[externs, js]));
    }
    // port: Compiler#getChunkGraph
    pub fn get_chunk_graph(&self) -> Option<&crate::js_chunk_graph::JSChunkGraph> {
        self.chunk_graph.as_deref()
    }
    // port: Compiler#getChunks
    pub fn get_chunks(&self) -> Option<Vec<crate::js_chunk::JSChunk>> {
        self.chunk_graph
            .as_ref()
            .map(|graph| graph.get_all_chunks().to_vec())
    }
    // port: Compiler#getInputsInOrder
    pub fn get_inputs_in_order(&self) -> Vec<CompilerInput> {
        self.chunk_graph
            .as_ref()
            .map_or_else(Vec::new, |graph| graph.get_all_inputs())
    }
    // port: Compiler#getNumberOfInputs
    pub fn get_number_of_inputs(&self) -> usize {
        self.chunk_graph.as_ref().unwrap().get_input_count()
    }
    // port: Compiler#getInputsById
    pub fn get_inputs_by_id(&self) -> &closure_rhino::fast_hash::IndexMap<InputId, CompilerInput> {
        &self.inputs_by_id
    }
    // port: Compiler#getExternsInOrder
    pub fn get_externs_in_order(&self) -> &[CompilerInput] {
        &self.externs
    }
    // port: Compiler#getInputsForTesting
    pub fn get_inputs_for_testing(&self) -> Option<Vec<CompilerInput>> {
        self.chunk_graph
            .as_ref()
            .map(|graph| graph.get_all_inputs())
    }
    // port: Compiler#getExternsForTesting
    pub fn get_externs_for_testing(&self) -> &[CompilerInput] {
        &self.externs
    }
    // port: Compiler#initBasedOnOptions
    pub fn init_based_on_options(&mut self) {
        self.input_source_maps
            .lock()
            .unwrap()
            .put_all(self.get_options().get_input_source_maps().clone());
        if self.get_options().should_gather_source_map_info() {
            let mut source_map = self.get_options().get_source_map_format().get_instance();
            source_map.set_prefix_mappings(
                self.get_options()
                    .get_source_map_location_mappings()
                    .iter()
                    .map(|mapping| {
                        Box::new(SharedLocationMapping(mapping.clone()))
                            as Box<dyn crate::source_map::LocationMapping>
                    })
                    .collect(),
            );
            if self.get_options().get_apply_input_source_maps() {
                source_map.set_source_file_mapping(Some(Box::new(self.excerpt_provider.clone())));
            }
            self.source_map = Some(Arc::new(std::sync::Mutex::new(source_map)));
            if self.get_options().get_apply_input_source_maps()
                && self.get_options().get_source_map_include_sources_content()
            {
                let maps: Vec<_> = self
                    .input_source_maps
                    .lock()
                    .unwrap()
                    .values()
                    .cloned()
                    .collect();
                for map in maps {
                    self.add_source_map_source_files(&map);
                }
            }
        }
    }
    // port: Compiler#addFilesToSourceMap
    fn add_files_to_source_map(&self, files: &[Arc<SourceFile>]) {
        if self.get_options().get_source_map_include_sources_content()
            && let Some(map) = &self.source_map
        {
            let mut map = map.lock().unwrap();
            for file in files {
                let source = file.get_code().unwrap_or_else(|error| panic!("{error}"));
                map.add_source_file(&file.get_name().into(), Some(&source));
            }
        }
    }
    // port: Compiler#compile(SourceFile, SourceFile, CompilerOptions)
    pub fn compile_single(
        &mut self,
        extern_file: Arc<SourceFile>,
        input: Arc<SourceFile>,
        options: CompilerOptions,
    ) -> crate::result::Result {
        self.compile(&[extern_file], &[input], options)
    }
    // port: Compiler#compile(List, List, CompilerOptions)
    pub fn compile(
        &mut self,
        externs: &[Arc<SourceFile>],
        inputs: &[Arc<SourceFile>],
        options: CompilerOptions,
    ) -> crate::result::Result {
        assert!(self.js_root.is_none());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.init(externs, inputs, options);
            self.compile_initialized();
        }));
        self.generate_report();
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
        self.get_result()
    }
    // port: Compiler#compileChunks
    pub fn compile_chunks(
        &mut self,
        externs: &[Arc<SourceFile>],
        chunks: Vec<crate::js_chunk::JSChunk>,
        options: CompilerOptions,
    ) -> crate::result::Result {
        assert!(self.js_root.is_none());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.init_chunks(externs, chunks, options);
            self.compile_initialized();
        }));
        self.generate_report();
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
        self.get_result()
    }
    // The two Java convenience methods share this identical statement sequence.
    fn compile_initialized(&mut self) {
        if !self.has_errors() {
            self.parse_for_compilation();
        }
        if !self.has_errors() {
            if self.get_options().get_instrument_for_coverage_only() {
                self.instrument_for_coverage();
            } else {
                self.stage1_passes();
                if !self.has_errors() {
                    self.stage2_passes(
                        crate::compiler_options::SegmentOfCompilationToRun::OPTIMIZATIONS,
                    );
                    if !self.has_errors() {
                        self.stage3_passes();
                    }
                }
            }
            self.perform_post_compilation_tasks();
        }
    }
    // port: Compiler#generateReport
    pub fn generate_report(&mut self) {
        let tracer = self.new_tracer("generateReport");
        self.report_queued_type_registry_errors();
        // Reports queued outside a report generator: Java made them when they happened.
        self.flush_source_map_errors();
        self.error_manager
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .generate_report(&self.ast);
        self.flush_source_map_errors();
        self.stop_tracer(tracer, "generateReport");
    }
    // port: Compiler#disableThreads
    pub fn disable_threads(&mut self) {
        self.compiler_executor.disable_threads();
    }
    // port: Compiler#setTimeout
    pub fn set_timeout(&mut self, timeout: i32) {
        self.compiler_executor.set_timeout(timeout);
    }
    // port: Compiler#runInCompilerThread
    pub fn run_in_compiler_thread<T: Send>(
        &mut self,
        callable: impl FnOnce(&mut Compiler) -> T + Send,
    ) -> T {
        let executor = self.compiler_executor.clone();
        let dump = self
            .options
            .as_ref()
            .is_some_and(|options| options.get_tracer_mode().is_on());
        executor.run_in_compiler_thread(|| callable(self), dump)
    }
    // port: Compiler#stage1Passes
    pub fn stage1_passes(&mut self) {
        assert!(
            self.chunk_graph.is_some(),
            "No inputs. Did you call init() or initChunks()?"
        );
        assert!(!self.has_errors());
        assert!(!self.get_options().get_instrument_for_coverage_only());
        self.run_in_compiler_thread(Self::perform_checks);
    }
    // port: Compiler#stage2Passes
    pub fn stage2_passes(&mut self, segment: crate::compiler_options::SegmentOfCompilationToRun) {
        use crate::compiler_options::SegmentOfCompilationToRun::*;
        assert!(
            matches!(
                segment,
                OPTIMIZATIONS | OPTIMIZATIONS_FIRST_HALF | OPTIMIZATIONS_SECOND_HALF
            ),
            "Unsupported segment of compilation to run in stage 2: {segment}"
        );
        assert!(
            self.chunk_graph.is_some(),
            "No inputs. Did you call init() or initChunks()?"
        );
        assert!(!self.has_errors());
        assert!(!self.get_options().get_instrument_for_coverage_only());
        if let Some(weak) = self
            .chunk_graph
            .as_ref()
            .unwrap()
            .get_chunk_by_name(crate::js_chunk::WEAK_CHUNK_NAME)
        {
            for input in self.get_inputs_in_order() {
                if input.get_source_file().is_weak() {
                    assert!(
                        input.get_chunk().as_ref() == Some(&weak),
                        "Expected all weak files to be in the weak chunk."
                    );
                }
            }
        }
        self.run_in_compiler_thread(|compiler| {
            if compiler.get_options().should_optimize() {
                compiler.perform_transpilation_and_optimizations(segment);
            }
        });
    }
    // port: Compiler#stage3Passes
    pub fn stage3_passes(&mut self) {
        assert!(
            self.chunk_graph.is_some(),
            "No inputs. Did you call init() or initChunks()?"
        );
        assert!(
            !self.has_errors(),
            "Cannot run stage3Passes with errors present: {:?}",
            self.get_errors()
        );
        assert!(!self.get_options().get_instrument_for_coverage_only());
        self.run_in_compiler_thread(|compiler| {
            if compiler.get_options().should_optimize() {
                compiler.perform_finalizations();
            }
        });
    }
    // port: Compiler#performChecks
    fn perform_checks(&mut self) {
        if self.get_options().get_skip_non_transpilation_passes() {
            self.whitespace_only_passes();
            if self
                .get_options()
                .needs_transpilation_from(self.get_options().get_language_in().to_feature_set())
            {
                self.transpile_and_dont_check();
            }
        } else {
            self.check();
        }
    }
    // port: Compiler#parseForCompilation
    pub fn parse_for_compilation(&mut self) {
        closure_rhino::check_state!(
            self.typed_ast_filesystem.is_none(),
            "Unnecessary if initWithTypedAstFilesystem was called"
        );
        self.run_in_compiler_thread(Self::parse_for_compilation_internal);
    }
    // port: Compiler#parseForCompilationInternal
    fn parse_for_compilation_internal(&mut self) {
        // Java throws the unchecked InvalidOptionsException to compile()'s caller, which no
        // compiler code catches: unwind with the typed exception (not a String panic) so callers
        // (IntegrationTestCase's assertThrows) see its class and message.
        if let Err(e) =
            crate::compiler_options_preprocessor::CompilerOptionsPreprocessor::preprocess(
                self.get_options(),
            )
        {
            std::panic::panic_any(e);
        }
        self.maybe_set_tracker();
        self.parse_inputs();
    }
    // port: Compiler#parse()
    pub fn parse(&mut self) {
        self.parse_inputs();
    }
    // port: Compiler#parse(SourceFile)
    pub fn parse_file(&mut self, file: Arc<SourceFile>) -> NodeId {
        self.init_compiler_options_if_testing();
        CompilerInput::new(file).get_ast_root(self)
    }
    // port: Compiler#getPassConfig
    pub fn get_pass_config(&mut self) -> &dyn crate::pass_config::PassConfig {
        if self.passes.is_none() {
            self.passes = Some(self.create_pass_config_internal());
        }
        self.passes.as_deref().unwrap()
    }
    // port: Compiler#createPassConfigInternal
    fn create_pass_config_internal(&self) -> Box<dyn crate::pass_config::PassConfig> {
        Box::new(crate::default_pass_config::DefaultPassConfig::new(
            self.get_options().clone(),
        ))
    }
    // port: Compiler#setPassConfig
    pub fn set_pass_config(&mut self, passes: Box<dyn crate::pass_config::PassConfig>) {
        assert!(self.passes.is_none(), "setPassConfig was already called");
        self.passes = Some(passes);
    }
    // port: Compiler#whitespaceOnlyPasses
    pub fn whitespace_only_passes(&mut self) {
        self.run_custom_passes(
            crate::custom_pass_execution_time::CustomPassExecutionTime::BEFORE_CHECKS,
        );
        let tracer = self.new_tracer("runWhitespaceOnlyPasses");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let list = self.get_pass_config().get_whitespace_only_passes();
            let passes = self.build_pass_list(&list);
            for factory in passes {
                let mut pass = factory.create(self);
                self.process(pass.as_mut());
            }
        }));
        self.stop_tracer(tracer, "runWhitespaceOnlyPasses");
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
    // port: Compiler#markTranspiledFiles
    fn mark_transpiled_files(&mut self) {
        for script in self.js_root.unwrap().children(self) {
            assert!(script.is_script(self));
            let features =
                crate::node_util::NodeUtil::get_feature_set_of_script(self, script).unwrap();
            if !self
                .get_options()
                .get_output_feature_set()
                .contains(features)
            {
                self.transpiled_files = true;
                return;
            }
        }
    }
    // port: Compiler#transpileAndDontCheck
    pub fn transpile_and_dont_check(&mut self) {
        let tracer = self.new_tracer("runTranspileOnlyPasses");
        self.mark_transpiled_files();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let list = self.get_pass_config().get_transpile_only_passes();
            let factories = self.build_pass_list(&list);
            for factory in factories {
                if self.has_errors() {
                    return;
                }
                self.before_pass(factory.get_name());
                let mut pass = factory.create(self);
                self.process(pass.as_mut());
                self.after_pass(factory.get_name());
            }
        }));
        self.stop_tracer(tracer, "runTranspileOnlyPasses");
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
    // port: Compiler#createPhaseOptimizer
    fn create_phase_optimizer(&mut self) -> crate::phase_optimizer::PhaseOptimizer {
        let mut optimizer = crate::phase_optimizer::PhaseOptimizer::new(self, self.tracker.clone());
        if self.get_options().get_dev_mode() == crate::compiler_options::DevMode::EVERY_PASS {
            optimizer.set_validity_check(self, self.validity_check_factory());
        }
        if self.get_options().get_check_determinism() {
            optimizer.set_print_ast_hashcodes(true);
        }
        optimizer
    }
    fn validity_check_factory(&self) -> crate::pass_factory::PassFactory {
        crate::pass_factory::PassFactory::builder()
            .set_name("validityCheck")
            .set_run_in_fixed_point_loop(true)
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(crate::validity_check::ValidityCheck::new(compiler))
            }))
            // TODO(b/280906684): Cannot use a lambda nor a simple anonymous function (which might
            // be converted to lambdas) directly due to this inference problem. Needs the cast to
            // make jscompiler happy.
            .set_condition(Arc::new(|_| panic!("Unexpected")))
            .build()
    }
    // port: Compiler#check
    pub fn check(&mut self) {
        self.run_custom_passes(
            crate::custom_pass_execution_time::CustomPassExecutionTime::BEFORE_CHECKS,
        );
        let list = self.get_pass_config().get_checks();
        let factories = self.build_pass_list(&list);
        let mut optimizer = self.create_phase_optimizer();
        optimizer.consume(factories);
        crate::compiler_pass::CompilerPass::process(
            &mut optimizer,
            self,
            self.externs_root.unwrap(),
            self.js_root.unwrap(),
        );
        if self.has_errors() {
            return;
        }
        self.run_custom_passes(
            crate::custom_pass_execution_time::CustomPassExecutionTime::BEFORE_OPTIMIZATIONS,
        );
        self.phase_optimizer = None;
    }
    // port: Compiler#process
    pub fn process(&mut self, pass: &mut dyn crate::compiler_pass::CompilerPass) {
        pass.process(self, self.externs_root.unwrap(), self.js_root.unwrap());
    }
    // port: Compiler#runValidityCheck
    fn run_validity_check(&mut self) {
        let mut pass = self.validity_check_factory().create(self);
        self.process(pass.as_mut());
    }
    /// Rust-only: PassListBuilder#build, recording the unported factories it omits so the
    /// unit-record harness can attribute a differing result to the first omitted pass.
    fn build_pass_list(
        &mut self,
        list: &crate::pass_list_builder::PassListBuilder,
    ) -> Vec<crate::pass_factory::PassFactory> {
        self.omitted_unported_passes.extend(
            list.build_including_unported()
                .iter()
                .filter_map(crate::pass_factory::PassFactory::get_unported_class),
        );
        list.build()
    }
    /// Rust-only: the classes of the unported pass factories this compiler's execution omitted
    /// so far, in order (Java would have run them).
    pub fn get_omitted_unported_passes(&self) -> &[&'static str] {
        &self.omitted_unported_passes
    }
    /// Rust-only: records unported pass factories a nested (shadow) compiler of this compiler
    /// omitted (NestedCompilerRunner), so the unit-record harness attributes them as well.
    pub(crate) fn add_omitted_unported_passes(&mut self, classes: &[&'static str]) {
        self.omitted_unported_passes.extend_from_slice(classes);
    }
    // port: Compiler#runCustomPasses
    fn run_custom_passes(
        &mut self,
        time: crate::custom_pass_execution_time::CustomPassExecutionTime,
    ) {
        let passes = self.get_options().get_custom_passes_at(time);
        let tracer = self.new_tracer("runCustomPasses");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for pass in passes {
                self.process(&mut *pass.lock().unwrap());
            }
        }));
        self.stop_tracer(tracer, "runCustomPasses");
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
    // port: Compiler#performTranspilationAndOptimizations
    pub fn perform_transpilation_and_optimizations(
        &mut self,
        segment: crate::compiler_options::SegmentOfCompilationToRun,
    ) {
        assert!(self.get_options().should_optimize());
        let list = self.get_pass_config().get_optimizations();
        // Rust-only: record the unported factories of this segment that execution omits.
        let including_unported = list.build_including_unported();
        if !including_unported.is_empty() {
            let omitted = self
                .get_optimization_passes_to_run_in_current_segment(segment, including_unported)
                .iter()
                .filter_map(crate::pass_factory::PassFactory::get_unported_class)
                .collect::<Vec<_>>();
            self.omitted_unported_passes.extend(omitted);
        }
        let factories = list.build();
        if factories.is_empty() {
            return;
        }
        let factories = self.get_optimization_passes_to_run_in_current_segment(segment, factories);
        self.mark_transpiled_files();
        let mut optimizer = self.create_phase_optimizer();
        optimizer.consume(factories);
        crate::compiler_pass::CompilerPass::process(
            &mut optimizer,
            self,
            self.externs_root.unwrap(),
            self.js_root.unwrap(),
        );
        self.phase_optimizer = None;
    }
    // port: Compiler#performFinalizations
    pub fn perform_finalizations(&mut self) {
        let list = self.get_pass_config().get_finalizations();
        let factories = self.build_pass_list(&list);
        if factories.is_empty() {
            return;
        }
        let mut optimizer = self.create_phase_optimizer();
        optimizer.consume(factories);
        crate::compiler_pass::CompilerPass::process(
            &mut optimizer,
            self,
            self.externs_root.unwrap(),
            self.js_root.unwrap(),
        );
        self.phase_optimizer = None;
    }
    // port: Compiler#getOptimizationPassesToRunInCurrentSegment
    fn get_optimization_passes_to_run_in_current_segment(
        &self,
        segment: crate::compiler_options::SegmentOfCompilationToRun,
        passes: Vec<crate::pass_factory::PassFactory>,
    ) -> Vec<crate::pass_factory::PassFactory> {
        use crate::compiler_options::SegmentOfCompilationToRun::*;
        match segment {
            OPTIMIZATIONS => passes,
            OPTIMIZATIONS_FIRST_HALF => {
                Self::pass_list_until(passes, crate::pass_names::OPTIMIZATIONS_HALFWAY_POINT)
            }
            OPTIMIZATIONS_SECOND_HALF => {
                Self::pass_list_after(passes, crate::pass_names::OPTIMIZATIONS_HALFWAY_POINT)
            }
            _ => panic!("Unsupported segment of optimizations to run: {segment}"),
        }
    }
    // port: Compiler#passListUntil
    fn pass_list_until(
        passes: Vec<crate::pass_factory::PassFactory>,
        name: &str,
    ) -> Vec<crate::pass_factory::PassFactory> {
        let mut list = Vec::new();
        for pass in passes {
            if pass.get_name() == name {
                return list;
            }
            list.push(pass);
        }
        list
    }
    // port: Compiler#passListAfter
    fn pass_list_after(
        passes: Vec<crate::pass_factory::PassFactory>,
        name: &str,
    ) -> Vec<crate::pass_factory::PassFactory> {
        let mut list = Vec::new();
        for pass in passes {
            if pass.get_name() == name {
                list.clear();
                continue;
            }
            list.push(pass);
        }
        list
    }
    // port: Compiler#maybeSetTracker
    pub fn maybe_set_tracker(&mut self) {
        if !self.get_options().get_tracer_mode().is_on() || self.tracker.is_some() {
            return;
        }
        let tracker = crate::performance_tracker::PerformanceTracker::new(
            self.externs_root.unwrap(),
            self.js_root.unwrap(),
            self.get_options().get_tracer_mode(),
        );
        self.change_tracker
            .add_change_handler(tracker.get_code_change_handler());
        self.tracker = Some(Arc::new(std::sync::Mutex::new(tracker)));
    }
    // port: Compiler#newTracer
    fn new_tracer(&self, pass_name: &str) -> crate::tracer::Tracer {
        let comment = format!(
            "{pass_name}{}",
            if self
                .change_tracker
                .get_recent_change()
                .lock()
                .unwrap()
                .has_code_changed()
            {
                " on recently changed AST"
            } else {
                ""
            }
        );
        if self.get_options().get_tracer_mode().is_on()
            && let Some(tracker) = &self.tracker
        {
            tracker.lock().unwrap().record_pass_start(pass_name, true);
        }
        crate::tracer::Tracer::new(Some("Compiler"), Some(&comment))
    }
    // port: Compiler#stopTracer
    fn stop_tracer(&self, mut tracer: crate::tracer::Tracer, pass_name: &str) {
        let result = tracer.stop();
        if self.get_options().get_tracer_mode().is_on()
            && let Some(tracker) = &self.tracker
        {
            tracker
                .lock()
                .unwrap()
                .record_pass_stop(self, pass_name, result);
        }
    }
    // port: Compiler#getResult
    pub fn get_result(&self) -> crate::result::Result {
        crate::result::Result::new(
            self.get_errors(),
            self.get_warnings(),
            self.variable_map.clone(),
            self.property_map.clone(),
            None,
            self.string_map.clone(),
            self.instrumentation_mapping.clone(),
            self.source_map.clone(),
            self.extern_exports.clone(),
            self.css_names.clone(),
            self.id_generator_map.clone(),
            self.transpiled_files,
        )
    }
    pub(crate) fn get_shared_scripts(&self) -> Arc<std::sync::Mutex<IndexMap<String, NodeId>>> {
        self.script_node_by_filename.clone()
    }
}
struct SharedLocationMapping(Arc<dyn crate::source_map::LocationMapping + Send + Sync>);
impl crate::source_map::LocationMapping for SharedLocationMapping {
    fn map(
        &self,
        location: &closure_rhino::js_string::JsString,
    ) -> Option<closure_rhino::js_string::JsString> {
        self.0.map(location)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self.0.as_any()
    }
}
impl std::fmt::Display for SharedLocationMapping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Compiler {
    // port: Compiler#isDebugLoggingEnabled
    pub fn is_debug_logging_enabled(&self) -> bool {
        self.get_options().get_debug_log_directory().is_some()
    }
    // port: Compiler#getDebugLogFilterList
    pub fn get_debug_log_filter_list(&self) -> Vec<String> {
        self.get_options()
            .get_debug_log_filter()
            .map_or_else(Vec::new, |filter| {
                filter
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
    }
    // port: Compiler#createOrReopenLog
    pub fn create_or_reopen_log(
        &self,
        owner: &str,
        first_name_part: &str,
        rest_name_parts: &[&str],
    ) -> Box<dyn crate::diagnostic::log_file::LogFile> {
        if !self.is_debug_logging_enabled() {
            return crate::diagnostic::log_file::create_no_op();
        }
        let mut file = self
            .get_options()
            .get_debug_log_directory()
            .unwrap()
            .join(owner)
            .join(first_name_part);
        for part in rest_name_parts {
            file = file.join(part);
        }
        let filters = self.get_debug_log_filter_list();
        if filters.is_empty()
            || filters
                .iter()
                .any(|filter| file.to_string_lossy().contains(filter))
        {
            crate::diagnostic::log_file::create_or_reopen(&file)
        } else {
            crate::diagnostic::log_file::create_no_op()
        }
    }
    // port: Compiler#createOrReopenIndexedLog
    pub fn create_or_reopen_indexed_log(
        &self,
        owner: &str,
        first_name_part: &str,
        rest_name_parts: &[&str],
    ) -> Box<dyn crate::diagnostic::log_file::LogFile> {
        assert!(self.current_pass_index >= 0, "{}", self.current_pass_index);
        let index = format!("{:03}", self.current_pass_index);
        if rest_name_parts.is_empty() {
            self.create_or_reopen_log(owner, &format!("{index}_{first_name_part}"), &[])
        } else {
            let mut parts: Vec<String> = rest_name_parts.iter().map(|s| (*s).into()).collect();
            let last = parts.len() - 1;
            parts[last] = format!("{index}_{}", parts[last]);
            self.create_or_reopen_log(
                owner,
                first_name_part,
                &parts.iter().map(String::as_str).collect::<Vec<_>>(),
            )
        }
    }
    // port: Compiler#logToFile
    fn log_to_file(&self, name: &str, contents: impl FnOnce() -> String) {
        let mut contents = Some(contents);
        self.create_or_reopen_log("Compiler", name, &[])
            .log(&mut || (contents.take().unwrap())());
    }
    // port: Compiler#printConfig
    pub fn print_config(&self) {
        let externs = format!(
            "[{}]",
            self.externs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let inputs =
            serde_json::to_string_pretty(&self.get_chunk_graph().unwrap().to_json()).unwrap();
        let options = self.get_options().to_string();
        let warnings = self.get_options().get_warnings_guard().to_string();
        if self.is_debug_logging_enabled() {
            self.log_to_file("externs.log", || externs);
            self.log_to_file("inputs.json", || inputs);
            self.log_to_file("options.log", || options);
            self.log_to_file("warningsGuard.log", || warnings);
        } else {
            eprintln!(
                "==== Externs ====\n{externs}\n==== Inputs ====\n{inputs}\n==== CompilerOptions ====\n{options}\n==== WarningsGuard ====\n{warnings}"
            );
        }
    }
    // port: Compiler#maybeLogPrunedInputs
    fn maybe_log_pruned_inputs(&self) {
        if self.is_debug_logging_enabled() {
            self.log_to_file("inputs_pruned.json", || {
                serde_json::to_string_pretty(&self.get_chunk_graph().unwrap().to_json()).unwrap()
            });
        }
    }
    // port: Compiler#performPostCompilationTasks
    pub fn perform_post_compilation_tasks(&mut self) {
        self.run_in_compiler_thread(Self::perform_post_compilation_tasks_internal);
    }
    // port: Compiler#performPostCompilationTasksInternal
    fn perform_post_compilation_tasks_internal(&mut self) {
        if self.get_options().get_dev_mode() == crate::compiler_options::DevMode::START_AND_END {
            self.run_validity_check();
        }
        self.process(&mut crate::manage_closure_unaware_code::ManageClosureUnawareCode::unwrap());
        if let Some(tracker) = &self.tracker {
            if let Some(path) = self.get_options().get_tracer_output() {
                let mut output = std::fs::File::create(path).unwrap();
                tracker
                    .lock()
                    .unwrap()
                    .output_tracer_report(&mut output)
                    .unwrap();
            } else {
                let mut output = SharedOutput(self.out_stream.clone().expect("null tracer output"));
                tracker
                    .lock()
                    .unwrap()
                    .output_tracer_report(&mut output)
                    .unwrap();
            }
        }
    }
    // port: Compiler#instrumentForCoverage
    pub fn instrument_for_coverage(&mut self) {
        assert!(
            self.chunk_graph.is_some(),
            "No inputs. Did you call init() or initChunks()?"
        );
        assert!(!self.has_errors());
        self.run_in_compiler_thread(|compiler| {
            assert!(compiler.get_options().get_instrument_for_coverage_only());
            assert!(!compiler.has_errors());
            let option = compiler.get_options().get_instrument_for_coverage_option();
            if option != crate::compiler_options::InstrumentOption::NONE {
                compiler.instrument_for_coverage_internal(option);
            }
        });
    }
    // port: Compiler#instrumentForCoverageInternal
    fn instrument_for_coverage_internal(
        &mut self,
        option: crate::compiler_options::InstrumentOption,
    ) {
        let tracer = self.new_tracer("instrumentationPass");
        let mut pass = crate::coverage_instrumentation_pass::CoverageInstrumentationPass::new(
            option,
            self.get_options()
                .get_production_instrumentation_array_name(),
        );
        self.process(&mut pass);
        self.stop_tracer(tracer, "instrumentationPass");
    }
    // port: Compiler#toSource()
    pub fn to_source(&mut self) -> closure_rhino::js_string::JsString {
        self.run_in_compiler_thread(|compiler| {
            let tracer = compiler.new_tracer("toSource");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut cb = CodeBuilder::default();
                let mut tracker =
                    crate::compiler_license_tracker::SingleBinaryLicenseTracker::new(compiler);
                if let Some(js_root) = compiler.js_root {
                    let mut i = 0;
                    if compiler.get_options().should_print_externs() {
                        for script in compiler
                            .externs_root
                            .unwrap()
                            .children(compiler)
                            .collect::<Vec<_>>()
                        {
                            compiler.to_source_with_builder(&mut cb, &mut tracker, i, script);
                            i += 1;
                        }
                    }
                    for script in js_root.children(compiler).collect::<Vec<_>>() {
                        compiler.to_source_with_builder(&mut cb, &mut tracker, i, script);
                        i += 1;
                    }
                }
                cb.to_js_string()
            }));
            compiler.stop_tracer(tracer, "toSource");
            result.unwrap_or_else(|error| std::panic::resume_unwind(error))
        })
    }
    // port: Compiler#toSource(JSChunk)
    pub fn to_source_for_chunk(
        &mut self,
        chunk: &crate::js_chunk::JSChunk,
    ) -> closure_rhino::js_string::JsString {
        let mut tracker = crate::compiler_license_tracker::ScriptNodeLicensesOnlyTracker::new(self);
        self.to_source_for_chunk_with_tracker(&mut tracker, chunk)
    }
    // port: Compiler#toSource(LicenseTracker, JSChunk)
    pub fn to_source_for_chunk_with_tracker(
        &mut self,
        tracker: &mut (dyn crate::code_printer::LicenseTracker + Send),
        chunk: &crate::js_chunk::JSChunk,
    ) -> closure_rhino::js_string::JsString {
        self.run_in_compiler_thread(|compiler| {
            let inputs = chunk.get_inputs();
            if inputs.is_empty() {
                return closure_rhino::js_string::JsString::default();
            }
            let mut cb = CodeBuilder::default();
            for (i, input) in inputs.iter().enumerate() {
                let root = input.get_ast_root(compiler);
                compiler.to_source_with_builder(&mut cb, tracker, i as i32, root);
            }
            cb.to_js_string()
        })
    }
    // port: Compiler#toSource(CodeBuilder, LicenseTracker, int, Node)
    pub fn to_source_with_builder(
        &mut self,
        cb: &mut CodeBuilder,
        tracker: &mut (dyn crate::code_printer::LicenseTracker + Send),
        input_seq_num: i32,
        root: NodeId,
    ) {
        self.run_in_compiler_thread(|compiler| {
            if compiler.get_options().should_print_input_delimiter() {
                if cb.get_length() > 0 && !cb.ends_with("\n") {
                    cb.append("\n");
                }
                assert!(root.is_script(compiler));
                let input_id = root.get_input_id(compiler).unwrap();
                let input_name = input_id.get_id_name();
                let source_name = root.get_source_file_name(compiler).unwrap();
                assert!(!source_name.is_empty());
                let delimiter = compiler
                    .get_options()
                    .get_input_delimiter()
                    .replace("%name%", input_name)
                    .replace("%num%", &input_seq_num.to_string())
                    .replace("%n%", "\n");
                cb.append(delimiter.as_str()).append("\n");
            }
            let source_and_mappings =
                compiler.to_source_and_mappings(root, input_seq_num == 0, tracker);
            let code = source_and_mappings.source;
            for license in tracker.emit_licenses() {
                cb.append("/*\n").append(license).append("*/\n");
            }
            if code.is_empty() {
                return;
            }
            if compiler.get_options().should_gather_source_map_info() {
                compiler
                    .source_map
                    .as_ref()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .set_starting_position(cb.get_line_index(), cb.get_column_index());
            }
            let chars = code.as_units();
            let length = chars.len();
            let last_char = chars[length - 1];
            let second_last_char = if length >= 2 { chars[length - 2] } else { 0 };
            let has_semicolon = last_char == 59 || (last_char == 10 && second_last_char == 59);
            cb.append(code);
            if !has_semicolon {
                cb.append(";");
            }
            if compiler.get_options().should_gather_source_map_info() {
                let mut map = compiler.source_map.as_ref().unwrap().lock().unwrap();
                for mapping in source_and_mappings.mappings.unwrap() {
                    map.add_mapping(compiler, &mapping);
                }
            }
        });
    }
    // port: Compiler#toSource(Node)
    pub fn to_source_for_node_utf16(&mut self, root: NodeId) -> closure_rhino::js_string::JsString {
        self.init_compiler_options_if_testing();
        let mut tracker = crate::compiler_license_tracker::ScriptNodeLicensesOnlyTracker::new(self);
        let code = self
            .to_source_and_mappings(root, false, &mut tracker)
            .source;
        let mut sb = CodeBuilder::default();
        for license in crate::code_printer::LicenseTracker::emit_licenses(&tracker) {
            sb.append("/*\n").append(license).append("*/\n");
        }
        sb.append(code);
        sb.to_js_string()
    }
    // port: Compiler#toSourceAndMappings
    fn to_source_and_mappings(
        &self,
        n: NodeId,
        first_output: bool,
        tracker: &mut dyn crate::code_printer::LicenseTracker,
    ) -> crate::code_printer::SourceAndMappings {
        crate::code_printer::Builder::new(n)
            .set_compiler_options(self.get_options())
            .set_tag_as_type_summary(self.get_options().should_generate_typed_externs())
            .set_tag_as_strict(first_output && self.get_options().should_emit_use_strict())
            .set_license_tracker(Some(tracker))
            .build_with_source_mappings(self)
    }
    // port: Compiler#getLicenseForFile
    pub fn get_license_for_file(
        compiler: &Compiler,
        file_name: Option<&str>,
    ) -> Option<closure_rhino::js_string::JsString> {
        compiler
            .get_script_node(file_name?)?
            .get_jsdoc_info(compiler)?
            .get_license()
    }
    // port: Compiler#toSourceArray
    pub fn to_source_array(
        &mut self,
        tracker: &mut (dyn crate::code_printer::LicenseTracker + Send),
        chunk: &crate::js_chunk::JSChunk,
    ) -> Vec<closure_rhino::js_string::JsString> {
        self.run_in_compiler_thread(|compiler| {
            let inputs = chunk.get_inputs();
            if inputs.is_empty() {
                return Vec::new();
            }
            let mut sources = Vec::with_capacity(inputs.len());
            let mut cb = CodeBuilder::default();
            for (i, input) in inputs.iter().enumerate() {
                let root = input.get_ast_root(compiler);
                cb.reset();
                compiler.to_source_with_builder(&mut cb, tracker, i as i32, root);
                sources.push(cb.to_js_string());
            }
            sources
        })
    }
}

struct ModuleErrorHandler(Arc<std::sync::Mutex<Vec<JSError>>>);
impl crate::error_handler::ErrorHandler for ModuleErrorHandler {
    fn report(&mut self, _level: CheckLevel, error: JSError) {
        self.0.lock().unwrap().push(error);
    }
}
impl Compiler {
    // port: Compiler#initializeModuleLoader
    pub fn initialize_module_loader(&mut self) {
        use crate::deps::module_loader::{
            ModuleLoader, ModuleResolverFactory, PathResolver, ResolutionMode,
        };
        let factory: Arc<dyn ModuleResolverFactory> =
            match self.get_options().get_module_resolution_mode() {
                ResolutionMode::BROWSER => {
                    Arc::clone(crate::deps::browser_module_resolver::BrowserModuleResolver::FACTORY)
                }
                ResolutionMode::NODE => {
                    let inputs = self.get_inputs_in_order();
                    Arc::new(
                        crate::deps::node_module_resolver::Factory::with_package_json_main_entries(
                            Some(self.process_json_inputs(&inputs)),
                        ),
                    )
                }
                ResolutionMode::WEBPACK => {
                    Arc::new(crate::deps::webpack_module_resolver::Factory::new(
                        self.input_path_by_webpack_id.clone(),
                    ))
                }
                ResolutionMode::BROWSER_WITH_TRANSFORMED_PREFIXES => Arc::new(
                    crate::deps::browser_with_transformed_prefixes_module_resolver::Factory::new(
                        self.get_options()
                            .get_browser_resolver_prefix_replacements()
                            .clone(),
                    ),
                ),
            };
        // ModuleLoader only reads DependencyInfo.getName while constructing this file table.
        let inputs = self
            .get_inputs_in_order()
            .iter()
            .map(|input| {
                crate::deps::simple_dependency_info::SimpleDependencyInfo::builder(
                    input.get_name(),
                    input.get_name(),
                )
                .build()
            })
            .collect::<Vec<_>>();
        self.module_loader = ModuleLoader::builder()
            .set_module_roots(self.get_options().get_module_roots().clone())
            .set_inputs(inputs)
            .set_factory(factory)
            .set_path_resolver(PathResolver::RELATIVE)
            .set_path_escaper(self.get_options().get_path_escaper())
            .build();
    }
    // port: Compiler#parseInputs
    pub fn parse_inputs(&mut self) -> Option<NodeId> {
        let dev_mode = self.get_options().get_dev_mode() != crate::compiler_options::DevMode::OFF;
        self.externs_root.unwrap().detach_children(self);
        self.js_root.unwrap().detach_children(self);
        self.script_node_by_filename.lock().unwrap().clear();
        let tracer = self.new_tracer(crate::pass_names::PARSE_INPUTS);
        self.before_pass(crate::pass_names::PARSE_INPUTS);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Rust-only (D-025): parse the inputs ahead on worker threads.
            let all_inputs: Vec<CompilerInput> = self
                .externs
                .iter()
                .cloned()
                .chain(self.get_inputs_in_order())
                .collect();
            crate::parallel_parse::start(self, &all_inputs);
            if self.get_options().get_num_parallel_threads() > 1 {
                crate::prebuild_ast::PrebuildAst::new(
                    self.get_options().get_num_parallel_threads(),
                )
                .prebuild(self, &self.externs.clone());
            }
            for input in self.externs.clone() {
                let n = input.get_ast_root(self);
                if self.has_errors() {
                    return None;
                }
                self.externs_root.unwrap().add_child_to_back(self, n);
                self.script_node_by_filename
                    .lock()
                    .unwrap()
                    .insert(input.get_source_file().get_name().into(), n);
            }
            if self
                .get_options()
                .get_language_in()
                .to_feature_set()
                .has(Feature::MODULES)
                || self.get_options().get_process_common_js_modules()
            {
                self.initialize_module_loader();
            } else {
                self.module_loader = crate::deps::module_loader::EMPTY.clone();
            }
            if self
                .get_options()
                .get_dependency_options()
                .needs_management()
            {
                self.find_modules_from_entry_points(
                    self.get_options()
                        .get_language_in()
                        .to_feature_set()
                        .has(Feature::MODULES),
                    self.get_options().get_process_common_js_modules(),
                );
            } else if self
                .get_options()
                .needs_transpilation_from(FeatureSet::ES2015_MODULES)
                || self.get_options().get_process_common_js_modules()
            {
                if self
                    .get_options()
                    .get_language_in()
                    .to_feature_set()
                    .has(Feature::MODULES)
                {
                    self.parse_potential_modules(&self.get_inputs_in_order());
                }
                let mut input_module_identifiers = IndexMap::<_, _>::default();
                for input in self.get_inputs_in_order() {
                    if input.get_known_provides().is_empty() {
                        let path = self
                            .module_loader
                            .resolve(input.get_source_file().get_name());
                        input_module_identifiers.insert(path.to_module_name(), input);
                    }
                }
                let mut inputs_to_rewrite = IndexMap::<_, _>::default();
                for input in self.get_inputs_in_order() {
                    for require in input.get_known_required_symbols() {
                        if let Some(required) = input_module_identifiers.get(&require)
                            && !inputs_to_rewrite.contains_key(&require)
                        {
                            inputs_to_rewrite.insert(require, required.clone());
                        }
                    }
                }
                for input in inputs_to_rewrite.values() {
                    input.set_js_module_type(crate::compiler_input::ModuleType::IMPORTED_SCRIPT);
                }
            }
            self.module_loader
                .set_error_handler(Some(Arc::new(std::sync::Mutex::new(ModuleErrorHandler(
                    self.pending_module_errors.clone(),
                )))));
            self.order_inputs();
            self.mark_closure_unaware_code();
            self.flush_module_errors();
            if self.has_errors() {
                return None;
            }
            if self.get_options().get_num_parallel_threads() > 1 {
                crate::prebuild_ast::PrebuildAst::new(
                    self.get_options().get_num_parallel_threads(),
                )
                .prebuild(self, &self.get_inputs_in_order());
            }
            for input in self.get_inputs_in_order() {
                let n = input.get_ast_root(self);
                if dev_mode {
                    self.run_validity_check();
                    if self.has_errors() {
                        return None;
                    }
                }
                if self.get_options().should_gather_source_map_info()
                    || self.get_options().get_extern_exports_path().is_some()
                    || !self
                        .get_options()
                        .get_replace_strings_function_descriptions()
                        .is_empty()
                {
                    let mut annotator = if self.get_options().get_dev_mode()
                        == crate::compiler_options::DevMode::OFF
                    {
                        crate::source_information_annotator::SourceInformationAnnotator::create()
                    } else {
                        crate::source_information_annotator::SourceInformationAnnotator::create_with_annotation_checks(input.get_name())
                    };
                    crate::node_traversal::NodeTraversal::traverse(self, n, &mut annotator);
                }
                if crate::node_util::NodeUtil::is_from_type_summary(self, n) {
                    input.set_is_extern();
                    self.externs_root.unwrap().add_child_to_back(self, n);
                } else {
                    self.js_root.unwrap().add_child_to_back(self, n);
                }
                self.script_node_by_filename
                    .lock()
                    .unwrap()
                    .insert(input.get_source_file().get_name().into(), n);
            }
            self.flush_module_errors();
            if self.has_errors() {
                return None;
            }
            // Save on memory. Any future calls to "createInputConsideringTypedAstFilesystem" will
            // throw.
            // TODO(lharker): do we actually need the synthetic externs file in stage 2?
            if let Some(typed_ast_filesystem) = &self.typed_ast_filesystem {
                // If anything (besides the synthetic externs) is in the typedAstFilesystem, that
                // indicates it never should have been added in the first place: every
                // CompilerInput should now be initialized, a process which removes it from the
                // typedAstFilesystem
                closure_rhino::check_state!(
                    typed_ast_filesystem.len() == 1 || typed_ast_filesystem.is_empty(),
                    "%s",
                    typed_ast_filesystem.len()
                );
            }
            self.extern_and_js_root
        }));
        // Rust-only: inputs preparsed but never asked for (pruned, or after an error).
        crate::parallel_parse::finish(self);
        self.after_pass(crate::pass_names::PARSE_INPUTS);
        self.stop_tracer(tracer, crate::pass_names::PARSE_INPUTS);
        result.unwrap_or_else(|error| std::panic::resume_unwind(error))
    }
    // Borrowed compiler callbacks are replayed immediately after dependency traversal.
    // Rust-only: the module loader's ErrorHandler (Java: the Compiler itself) cannot hold the
    // compiler, so every caller that resolves module paths (ModuleMapCreator, TypeInference,
    // ProcessCommonJSModules, ...) replays its reports right after the resolution, where Java
    // reported them.
    pub fn flush_module_errors(&mut self) {
        let errors = std::mem::take(&mut *self.pending_module_errors.lock().unwrap());
        for error in errors {
            self.report(error);
        }
    }
    // port: Compiler#orderInputsWithLargeStack
    pub fn order_inputs_with_large_stack(&mut self) {
        self.run_in_compiler_thread(|compiler| {
            let tracer = compiler.new_tracer("orderInputsWithLargeStack");
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compiler.order_inputs()));
            compiler.stop_tracer(tracer, "orderInputsWithLargeStack");
            if let Err(error) = result {
                std::panic::resume_unwind(error);
            }
        });
    }
    // port: Compiler#orderInputs
    pub fn order_inputs(&mut self) {
        self.maybe_do_threaded_parsing();
        let original_inputs = self.get_inputs_in_order();
        if let Some(tracker) = &self.tracker {
            tracker
                .lock()
                .unwrap()
                .record_pre_pruning_input_count(&original_inputs, &self.externs);
        }
        self.mark_externs(&original_inputs);
        let mut stale_inputs = false;
        if self
            .get_options()
            .get_dependency_options()
            .needs_management()
        {
            let graph = self.chunk_graph.as_ref().unwrap().clone();
            let options = self.get_options().get_dependency_options().clone();
            match graph.manage_dependencies(self, &options) {
                Ok(result) => {
                    stale_inputs = true;
                    self.maybe_log_pruning_analysis(&result);
                }
                Err(crate::js_chunk_graph::DependencyManagementError::MissingProvide(error)) => {
                    self.report(JSError::make_without_location(
                        &MISSING_ENTRY_ERROR,
                        &[&error.to_string()],
                    ))
                }
                Err(crate::js_chunk_graph::DependencyManagementError::MissingChunk(error)) => self
                    .report(JSError::make_without_location(
                        &MISSING_CHUNK_ERROR,
                        &[&error.to_string()],
                    )),
            }
        }
        self.hoist_externs(&original_inputs);
        self.fill_empty_chunks();
        self.hoist_no_compile_files();
        if stale_inputs {
            self.repartition_inputs();
        }
        self.maybe_log_pruned_inputs();
        self.flush_module_errors();
    }
    // port: Compiler#findModulesFromEntryPoints
    #[allow(clippy::mutable_key_type)]
    fn find_modules_from_entry_points(
        &mut self,
        support_es6_modules: bool,
        support_common_js_modules: bool,
    ) {
        self.maybe_do_threaded_parsing();
        let mut entry_points = Vec::new();
        let mut inputs_by_provide = IndexMap::<_, _>::default();
        let mut inputs_by_identifier = IndexMap::<_, _>::default();
        for input in self.get_inputs_in_order() {
            let provides: Vec<_> = input
                .get_provides(self)
                .into_iter()
                .filter(|p| !p.starts_with("module$"))
                .collect();
            if !self
                .get_options()
                .get_dependency_options()
                .should_drop_moochers()
                && provides.is_empty()
            {
                entry_points.push(input.clone());
            }
            inputs_by_identifier.insert(
                crate::module_identifier::ModuleIdentifier::for_file(
                    &input.get_path(self).to_string(),
                )
                .to_string(),
                input.clone(),
            );
            for provide in provides {
                inputs_by_provide.insert(provide, input.clone());
            }
        }
        for identifier in self.get_options().get_dependency_options().entry_points() {
            let name = identifier.to_string();
            if let Some(input) = inputs_by_provide
                .get(&name)
                .or_else(|| inputs_by_identifier.get(&name))
            {
                entry_points.push(input.clone());
            }
        }
        let mut working_input_set: closure_rhino::fast_hash::IndexSet<CompilerInput> =
            self.get_inputs_in_order().into_iter().collect();
        for entry_point in entry_points {
            self.find_modules_from_input(
                &entry_point,
                false,
                &mut working_input_set,
                &inputs_by_identifier,
                &inputs_by_provide,
                support_es6_modules,
                support_common_js_modules,
            );
        }
    }
    // port: Compiler#findModulesFromInput
    #[allow(clippy::too_many_arguments, clippy::mutable_key_type)]
    fn find_modules_from_input(
        &mut self,
        input: &CompilerInput,
        was_imported_by_module: bool,
        inputs: &mut closure_rhino::fast_hash::IndexSet<CompilerInput>,
        inputs_by_identifier: &IndexMap<String, CompilerInput>,
        inputs_by_provide: &IndexMap<String, CompilerInput>,
        support_es6_modules: bool,
        support_common_js_modules: bool,
    ) {
        use crate::compiler_input::ModuleType;
        if !inputs.shift_remove(input) {
            if was_imported_by_module && input.get_js_module_type() == ModuleType::NONE {
                input.set_js_module_type(ModuleType::IMPORTED_SCRIPT);
            }
            return;
        }
        let mut find_deps = crate::find_module_dependencies::FindModuleDependencies::new(
            support_es6_modules,
            support_common_js_modules,
            &self.input_path_by_webpack_id,
        );
        let root = input.get_ast_root(self);
        find_deps.process(self, root);
        if was_imported_by_module && input.get_js_module_type() == ModuleType::NONE {
            input.set_js_module_type(ModuleType::IMPORTED_SCRIPT);
        }
        let path = input.get_path(self);
        self.module_types_by_name
            .insert(path.to_module_name(), input.get_js_module_type());
        let mut all_deps: Vec<_> = input
            .get_requires(self)
            .iter()
            .map(|r| r.get_symbol().to_owned())
            .collect();
        all_deps.extend(input.get_dynamic_requires());
        all_deps.extend(input.get_type_requires(self));
        for required_namespace in all_deps {
            let (required_input, required_by_module_import) =
                if let Some(input) = inputs_by_provide.get(&required_namespace) {
                    (Some(input), false)
                } else {
                    (inputs_by_identifier.get(&required_namespace), true)
                };
            if let Some(required_input) = required_input {
                self.find_modules_from_input(
                    required_input,
                    required_by_module_import,
                    inputs,
                    inputs_by_identifier,
                    inputs_by_provide,
                    support_es6_modules,
                    support_common_js_modules,
                );
            }
        }
    }
    // port: Compiler#hoistExterns
    pub fn hoist_externs(&mut self, original_inputs: &[CompilerInput]) {
        let mut stale_inputs = false;
        for input in original_inputs {
            if self.hoist_if_extern(input) {
                stale_inputs = true;
            }
        }
        if stale_inputs {
            self.repartition_inputs();
        }
    }
    // port: Compiler#hoistIfExtern
    fn hoist_if_extern(&mut self, input: &CompilerInput) -> bool {
        if input.get_has_externs_annotation(self) {
            let root = input.get_ast_root(self);
            self.externs_root.unwrap().add_child_to_back(self, root);
            self.script_node_by_filename
                .lock()
                .unwrap()
                .insert(input.get_source_file().get_name().into(), root);
            if let Some(chunk) = input.get_chunk() {
                chunk.remove(input);
            }
            self.externs.push(input.clone());
            true
        } else {
            false
        }
    }
    // port: Compiler#markClosureUnawareCode
    fn mark_closure_unaware_code(&mut self) {
        for input in self.get_inputs_in_order() {
            let root = input.get_ast_root(self);
            if root
                .get_jsdoc_info(self)
                .is_some_and(|info| info.is_closure_unaware_code())
            {
                input.get_source_file().mark_as_closure_unaware_code();
            }
        }
    }
    // port: Compiler#markExterns
    fn mark_externs(&mut self, original_inputs: &[CompilerInput]) {
        for input in original_inputs {
            if input.get_has_externs_annotation(self) {
                input.set_is_extern();
            }
        }
    }
    // port: Compiler#hoistNoCompileFiles
    pub fn hoist_no_compile_files(&mut self) {
        let mut stale_inputs = false;
        self.maybe_do_threaded_parsing();
        for input in self.get_inputs_in_order() {
            if input.get_has_no_compile_annotation(self) {
                input.get_chunk().unwrap().remove(&input);
                stale_inputs = true;
            }
        }
        if stale_inputs {
            self.repartition_inputs();
        }
    }
    // port: Compiler#maybeDoThreadedParsing
    fn maybe_do_threaded_parsing(&mut self) {
        if self.get_options().get_num_parallel_threads() > 1 {
            crate::prebuild_dependency_info::PrebuildDependencyInfo::new(
                self.get_options().get_num_parallel_threads(),
            )
            .prebuild(self, &self.get_inputs_in_order());
        }
    }
    // port: Compiler#repartitionInputs
    fn repartition_inputs(&mut self) {
        self.fill_empty_chunks();
        self.rebuild_inputs_from_chunks();
    }
    // port: Compiler#processJsonInputs
    pub fn process_json_inputs(
        &mut self,
        inputs_to_process: &[CompilerInput],
    ) -> IndexMap<String, String> {
        let mut rewrite_json = crate::rewrite_json_to_module::RewriteJsonToModule::new();
        for input in inputs_to_process {
            if !input.get_source_file().get_name().ends_with(".json") {
                continue;
            }
            input.set_compiler(self);
            let Ok(code) = input.get_source_file().get_code() else {
                continue;
            };
            let mut wrapped = CodeBuilder::default();
            wrapped.append("(").append(code).append(")");
            input
                .get_source_file()
                .set_code_deprecated(wrapped.to_js_string());
            let root = input.get_ast_root(self);
            input.set_js_module_type(crate::compiler_input::ModuleType::JSON);
            crate::compiler_pass::CompilerPass::process(
                &mut rewrite_json,
                self,
                self.externs_root.unwrap(),
                root,
            );
        }
        rewrite_json.get_package_json_main_entries()
    }
    // port: Compiler#parsePotentialModules
    fn parse_potential_modules(&mut self, inputs_to_process: &[CompilerInput]) {
        let mut filtered_inputs = Vec::new();
        for input in inputs_to_process {
            if !self.get_options().get_dependency_options().should_prune()
                || !crate::deps::js_file_regex_parser::JsFileRegexParser::is_supported()
                || input.is_es6_module(self)
            {
                filtered_inputs.push(input.clone());
            }
        }
        if self.get_options().get_num_parallel_threads() > 1 {
            crate::prebuild_ast::PrebuildAst::new(self.get_options().get_num_parallel_threads())
                .prebuild(self, &filtered_inputs);
        }
        for input in filtered_inputs {
            input.set_compiler(self);
            input.get_requires(self);
            input.set_js_module_type(crate::compiler_input::ModuleType::ES6);
        }
    }
    // port: Compiler#maybeLogPruningAnalysis
    fn maybe_log_pruning_analysis(
        &self,
        result: &crate::js_chunk_graph::DependencyManagementResult,
    ) {
        if self.tracker.is_none() || !self.get_options().get_tracer_mode().do_pruning_analysis() {
            return;
        }
        let entry_points = result.entry_point_inputs.iter().map(|input| {
            result
                .sorter
                .get_sorted_list()
                .iter()
                .find(|view| view.input == *input)
                .expect("entry point inputs are sorter inputs")
                .clone()
        });
        let analysis =
            crate::pruning_analysis::PruningAnalysis::create(&result.sorter, entry_points);
        let pruning_result = analysis.analyze();
        self.tracker.as_ref().unwrap().lock().unwrap().set_pruning_analysis_summary(format!("\nTransitive dependency count per entry point:\n{}\n\nTop bottleneck dependencies:\n{}",pruning_result.entry_point_dependency_count,pruning_result.bottleneck_blame));
    }
}

impl Compiler {
    // port: Compiler#resolveSibling
    pub fn resolve_sibling(from_path: &str, to_path: &str) -> String {
        if to_path.starts_with('/') {
            return to_path.into();
        }
        let mut from_parts = closure_rhino::java_lang::string::split(from_path, "/");
        let mut to_parts = closure_rhino::java_lang::string::split(to_path, "/");
        if !from_parts.is_empty() {
            from_parts.pop();
        }
        while !from_parts.is_empty() && !to_parts.is_empty() {
            if to_parts[0] == "." {
                to_parts.remove(0);
            } else if to_parts[0] == ".." {
                to_parts.remove(0);
                from_parts.pop();
            } else {
                break;
            }
        }
        from_parts.extend(to_parts);
        from_parts.join("/")
    }
    // port: Compiler#addSourceMapSourceFiles
    fn add_source_map_source_files(
        &self,
        input_source_map: &crate::source_map_input::SourceMapInput,
    ) {
        let Some(consumer) = self.excerpt_provider.get_consumer(input_source_map) else {
            self.flush_source_map_errors();
            return;
        };
        self.flush_source_map_errors();
        let Some(content) = consumer.get_original_sources_content() else {
            return;
        };
        let sources = consumer.get_original_sources();
        for (source, code) in sources.iter().zip(content) {
            let source = crate::source_map_resolver::SourceMapResolver::get_relative_path(
                input_source_map.get_original_path(),
                &source
                    .as_ref()
                    .expect("null original source")
                    .to_string_lossy(),
            );
            self.source_map
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .add_source_file(
                    &source.get_name().into(),
                    // Java passes a null "sourcesContent" entry on as a null content.
                    code.as_ref(),
                );
        }
        assert_eq!(
            sources.len(),
            content.len(),
            "Source map's \"sources\" and \"sourcesContent\" lengths do not match."
        );
    }
    fn flush_source_map_errors(&self) {
        let errors = std::mem::take(&mut *self.excerpt_provider.pending_errors.lock().unwrap());
        if !errors.is_empty() {
            let manager = self.error_manager.as_ref().unwrap().lock().unwrap();
            for (level, error) in errors {
                manager.report(level, error);
            }
        }
    }
    // port: Compiler#getSourceMapping
    pub fn get_source_mapping(
        &self,
        source_name: Option<&str>,
        line_number: i32,
        column_number: i32,
    ) -> Option<closure_sourcemap::proto::mapping::OriginalMapping> {
        let mapping = crate::source_excerpt_provider::SourceExcerptProvider::get_source_mapping(
            self.excerpt_provider.as_ref(),
            source_name,
            line_number,
            column_number,
        );
        self.flush_source_map_errors();
        mapping
    }
    // port: Compiler#getBase64SourceMapContents
    pub fn get_base64_source_map_contents(&self, source_name: &str) -> Option<String> {
        use base64::Engine;
        let map = self
            .input_source_maps
            .lock()
            .unwrap()
            .get(source_name)
            .cloned()?;
        if map.get_original_path().ends_with(".inline.map") {
            let unencoded = map
                .get_raw_source_map_contents()
                .expect("null raw source map contents");
            Some(
                base64::engine::general_purpose::STANDARD
                    .encode(unencoded.to_string_lossy().as_bytes()),
            )
        } else {
            None
        }
    }
    // port: Compiler#getSourceFileContentByName
    pub fn get_source_file_content_by_name(
        &self,
        source_name: &str,
    ) -> Option<closure_rhino::js_string::JsString> {
        let file = self
            .get_source_file_by_name(source_name)
            .expect("null source file");
        if file.is_stub_source_file_for_already_provided_input() {
            return None;
        }
        file.get_code().ok()
    }
    // port: Compiler#getSourceLines
    pub fn get_source_lines(
        &self,
        source_name: &str,
        line_number: i32,
        length: i32,
    ) -> Option<crate::simple_region::SimpleRegion> {
        if line_number < 1 {
            return None;
        }
        let file = self.get_source_file_by_name(source_name)?;
        if file.is_stub_source_file_for_already_provided_input() {
            return None;
        }
        file.get_lines(line_number, length)
    }
    // port: Compiler#getSourceRegion
    pub fn get_source_region(
        &self,
        source_name: &str,
        line_number: i32,
    ) -> Option<crate::simple_region::SimpleRegion> {
        if line_number < 1 {
            return None;
        }
        let file = self.get_source_file_by_name(source_name)?;
        if file.is_stub_source_file_for_already_provided_input() {
            return None;
        }
        file.get_region(line_number)
    }
    // port: Compiler#getSourceMap
    pub fn get_source_map(&self) -> Option<Arc<std::sync::Mutex<crate::source_map::SourceMap>>> {
        self.source_map.clone()
    }
    pub fn get_input_source_maps(&self) -> InputSourceMaps {
        self.input_source_maps.lock().unwrap().clone()
    }
    // port: Compiler#resetAndIntitializeSourceMap
    pub fn reset_and_intitialize_source_map(&self) {
        let Some(map) = &self.source_map else {
            return;
        };
        map.lock().unwrap().reset();
        if self.get_options().get_source_map_include_sources_content() {
            if self.get_options().get_apply_input_source_maps() {
                let maps: Vec<_> = self
                    .input_source_maps
                    .lock()
                    .unwrap()
                    .values()
                    .cloned()
                    .collect();
                for map in maps {
                    self.add_source_map_source_files(&map);
                }
            }
            if let Some(chunks) = self.get_chunks() {
                let files: Vec<_> = chunks
                    .into_iter()
                    .flat_map(|chunk| chunk.get_inputs())
                    .filter(|input| {
                        !input
                            .get_source_file()
                            .is_stub_source_file_for_already_provided_input()
                    })
                    .map(|input| input.get_source_file_arc())
                    .collect();
                self.add_files_to_source_map(&files);
            }
        }
    }
}
impl crate::source_file_mapping::SourceFileMapping for Compiler {
    fn get_source_mapping(
        &self,
        name: &closure_rhino::js_string::JsString,
        line: i32,
        column: i32,
    ) -> Option<closure_sourcemap::proto::mapping::OriginalMapping> {
        Compiler::get_source_mapping(self, Some(&name.to_string_lossy()), line, column)
    }
}

impl Compiler {
    // port: Compiler#setExternExports
    pub fn set_extern_exports(&mut self, exports: String) {
        self.extern_exports = Some(exports);
    }
    // port: Compiler#setVariableMap
    pub fn set_variable_map(&mut self, map: Arc<crate::variable_map::VariableMap>) {
        self.variable_map = Some(map);
    }
    // port: Compiler#getVariableMap
    pub fn get_variable_map(&self) -> Option<&Arc<crate::variable_map::VariableMap>> {
        self.variable_map.as_ref()
    }
    // port: Compiler#setPropertyMap
    pub fn set_property_map(&mut self, map: Arc<crate::variable_map::VariableMap>) {
        self.property_map = Some(map);
    }
    // port: Compiler#getPropertyMap
    pub fn get_property_map(&self) -> Option<&Arc<crate::variable_map::VariableMap>> {
        self.property_map.as_ref()
    }
    // port: Compiler#setStringMap
    pub fn set_string_map(&mut self, map: Arc<crate::variable_map::VariableMap>) {
        self.string_map = Some(map);
    }
    // port: Compiler#getStringMap
    pub fn get_string_map(&self) -> Option<&Arc<crate::variable_map::VariableMap>> {
        self.string_map.as_ref()
    }
    // port: Compiler#setInstrumentationMapping
    pub fn set_instrumentation_mapping(&mut self, map: Arc<crate::variable_map::VariableMap>) {
        self.instrumentation_mapping = Some(map);
    }
    // port: Compiler#getInstrumentationMapping
    pub fn get_instrumentation_mapping(&self) -> Option<&Arc<crate::variable_map::VariableMap>> {
        self.instrumentation_mapping.as_ref()
    }
    // port: Compiler#setCssNames
    pub fn set_css_names(&mut self, names: Option<closure_rhino::fast_hash::IndexSet<String>>) {
        self.css_names = names;
    }
    // port: Compiler#setIdGeneratorMap
    pub fn set_id_generator_map(&mut self, map: String) {
        self.id_generator_map = Some(map);
    }
    // port: Compiler#getTranspiledFiles
    pub fn get_transpiled_files(&self) -> bool {
        self.transpiled_files
    }
    // port: Compiler#setTranspiledFiles
    pub fn set_transpiled_files(&mut self, value: bool) {
        self.transpiled_files = value;
    }
    // port: Compiler#getCrossChunkIdGenerator
    pub fn get_cross_chunk_id_generator(&mut self) -> &mut crate::id_generator::IdGenerator {
        &mut self.cross_chunk_id_generator
    }
    // port: Compiler#setAnonymousFunctionNameMap
    pub fn set_anonymous_function_name_map(&mut self, _map: Arc<crate::variable_map::VariableMap>) {
    }
    // port: Compiler#addExportedNames
    pub fn add_exported_names(&mut self, names: impl IntoIterator<Item = String>) {
        self.exported_names.extend(names);
    }
    // port: Compiler#getExportedNames
    pub fn get_exported_names(&self) -> &closure_rhino::fast_hash::IndexSet<String> {
        &self.exported_names
    }
    // port: Compiler#setDefineNames
    pub fn set_define_names(&mut self, names: impl IntoIterator<Item = String>) {
        self.define_names = names.into_iter().collect();
    }
    // port: Compiler#getDefineNames
    pub fn get_define_names(&self) -> &closure_rhino::fast_hash::IndexSet<String> {
        &self.define_names
    }
    // port: Compiler#hasRegExpGlobalReferences
    pub fn has_reg_exp_global_references(&self) -> bool {
        self.has_reg_exp_global_references
    }
    // port: Compiler#setHasRegExpGlobalReferences
    pub fn set_has_reg_exp_global_references(&mut self, references: bool) {
        self.has_reg_exp_global_references = references;
    }
    // port: Compiler#setRunJ2clPasses
    pub fn set_run_j2cl_passes(&mut self, value: bool) {
        self.run_j2cl_passes = value;
    }
    // port: Compiler#runJ2clPasses
    pub fn run_j2cl_passes(&self) -> bool {
        self.run_j2cl_passes
    }
    // port: Compiler#setExternProperties
    pub fn set_extern_properties(
        &mut self,
        properties: closure_rhino::fast_hash::IndexSet<String>,
    ) {
        self.extern_properties = Some(properties);
        self.extern_properties_js = std::sync::OnceLock::new();
    }
    // port: Compiler#getExternProperties
    pub fn get_extern_properties(&self) -> Option<&closure_rhino::fast_hash::IndexSet<String>> {
        self.extern_properties.as_ref()
    }
    /// Rust-only: `get_extern_properties` as JS strings, interned so that copying one needs no
    /// reference count (RemoveUnusedCode and OptimizeCalls copy them all on every run).
    pub fn get_extern_properties_js(&self) -> Option<&[closure_rhino::js_string::JsString]> {
        let properties = self.extern_properties.as_ref()?;
        Some(self.extern_properties_js.get_or_init(|| {
            properties
                .iter()
                .map(|s| closure_rhino::rhino_string_pool::RhinoStringPool::add_or_get(s.as_str()))
                .collect()
        }))
    }
    // port: Compiler#getAccessorSummary
    pub fn get_accessor_summary(&self) -> Option<&Arc<crate::accessor_summary::AccessorSummary>> {
        self.accessor_summary.as_ref()
    }
    // port: Compiler#setAccessorSummary
    pub fn set_accessor_summary(&mut self, summary: Arc<crate::accessor_summary::AccessorSummary>) {
        self.accessor_summary = Some(summary);
    }
    // port: Compiler#getUniqueIdSupplier
    pub fn get_unique_id_supplier(&mut self) -> &mut crate::unique_id_supplier::UniqueIdSupplier {
        &mut self.unique_id_supplier
    }
    // port: Compiler#nextUniqueNameId
    pub fn next_unique_name_id(&self) -> i32 {
        self.unique_name_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
    // port: Compiler#getUniqueNameIdSupplier
    pub fn get_unique_name_id_supplier(&self) -> Arc<dyn Fn() -> String + Send + Sync> {
        let id = self.unique_name_id.clone();
        Arc::new(move || {
            id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                .to_string()
        })
    }
    // port: Compiler#areNodesEqualForInlining
    pub fn are_nodes_equal_for_inlining(&self, n1: NodeId, n2: NodeId) -> bool {
        if self.get_options().should_ambiguate_properties()
            || self.get_options().should_disambiguate_properties()
        {
            n1.is_equivalent_to_typed(self, n2)
        } else {
            n1.is_equivalent_to(self, n2)
        }
    }
    // port: Compiler#addIndexProvider
    pub fn add_index_provider<
        T: std::any::Any + Send,
        P: crate::index_provider::IndexProvider<T> + 'static,
    >(
        &mut self,
        provider: P,
    ) {
        let type_ = provider.get_type();
        let previous = self.index_providers_by_type.insert(
            type_,
            Box::new(crate::index_provider::Provider(
                provider,
                std::marker::PhantomData::<T>,
            )),
        );
        assert!(
            previous.is_none(),
            "A provider is already registered for index of type {}",
            std::any::type_name::<T>().rsplit("::").next().unwrap()
        );
    }
    // port: Compiler#getIndex
    pub fn get_index<T: std::any::Any + Send>(&mut self) -> Option<T> {
        self.index_providers_by_type
            .get_mut(&std::any::TypeId::of::<T>())
            .map(|provider| *provider.get().downcast::<T>().unwrap())
    }
    // port: Compiler#clearJSTypeRegistry
    pub fn clear_js_type_registry(&mut self) {
        self.type_registry = None;
        self.type_validator = None;
        self.abstract_interpreter = None;
    }
    // port: Compiler#isTypeRegistryCleared
    pub fn is_type_registry_cleared(&self) -> bool {
        self.type_checking_has_run && self.type_registry.is_none()
    }
    // port: Compiler#getTypeRegistry
    pub fn get_type_registry(&mut self) -> &mut closure_jstype::JSTypeRegistry {
        self.get_type_registry_and_ast().0
    }
    /// Rust-only: `Compiler#getTypeRegistry` together with the arena its methods read, split-borrowed.
    pub fn get_type_registry_and_ast(&mut self) -> (&mut closure_jstype::JSTypeRegistry, &mut Ast) {
        // Errors the registry queued since the last call reach the error manager first.
        self.report_queued_type_registry_errors();
        if self.type_registry.is_none() {
            assert!(
                !self.has_type_checking_run(),
                "Attempted to re-initialize JSTypeRegistry after it had been cleared"
            );
            // oldErrorReporter = RhinoErrorReporter.forOldRhino(this)
            let old_error_reporter =
                crate::rhino_error_reporter::QueueingOldRhinoErrorReporter::new(Arc::clone(
                    &self.queued_type_registry_errors,
                ));
            self.type_registry = Some(
                closure_jstype::JSTypeRegistry::new_with_shared_forward_declared_types(
                    &mut self.ast,
                    Box::new(old_error_reporter),
                    self.forward_declared_types.clone(),
                ),
            );
        }
        (self.type_registry.as_mut().unwrap(), &mut self.ast)
    }
    /// Rust-only: the `typeRegistry` field (as `Compiler#getAstAnalyzer` hands it to
    /// `new AstAnalyzer(..)`, possibly null) together with the arena, split-borrowed.
    pub fn get_type_registry_field_and_ast(
        &mut self,
    ) -> (Option<&mut closure_jstype::JSTypeRegistry>, &Ast) {
        (self.type_registry.as_mut(), &self.ast)
    }
    /// Rust-only: `Compiler#getCodingConvention` together with `Compiler#getTypeRegistry` and the
    /// arena, split-borrowed (`CodingConvention` methods that read the registry need all three).
    pub fn get_coding_convention_type_registry_and_ast(
        &mut self,
    ) -> (&dyn CodingConvention, &closure_jstype::JSTypeRegistry, &Ast) {
        self.get_type_registry();
        let convention = self
            .get_options()
            .get_coding_convention_ref()
            .unwrap_or(&self.default_coding_convention);
        (convention, self.type_registry.as_ref().unwrap(), &self.ast)
    }
    /// Rust-only: the `typeRegistry` field (as the AstFactories this compiler creates hold it)
    /// together with the arena, split-borrowed mutably.
    pub fn get_type_registry_field_and_ast_mut(
        &mut self,
    ) -> (Option<&mut closure_jstype::JSTypeRegistry>, &mut Ast) {
        (self.type_registry.as_mut(), &mut self.ast)
    }
    // port: Compiler#getColorRegistry
    pub fn get_color_registry(
        &self,
    ) -> &Arc<closure_rhino::jscomp_colors::color_registry::ColorRegistry> {
        self.color_registry
            .as_ref()
            .expect("Color registry has not been initialized yet")
    }
    // port: Compiler#setColorRegistry
    pub fn set_color_registry(
        &mut self,
        registry: Arc<closure_rhino::jscomp_colors::color_registry::ColorRegistry>,
    ) {
        assert!(self.runtime_library_typed_asts.is_some());
        self.color_registry = Some(registry);
    }
    // port: Compiler#forwardDeclareType
    pub fn forward_declare_type(&mut self, name: String) {
        self.forward_declared_types.lock().unwrap().add(name);
    }
    // port: Compiler#setTypeCheckingHasRun
    pub fn set_type_checking_has_run(&mut self, value: bool) {
        self.type_checking_has_run = value;
    }
    // port: Compiler#hasTypeCheckingRun
    pub fn has_type_checking_run(&self) -> bool {
        self.type_checking_has_run
    }
    // port: Compiler#hasOptimizationColors
    pub fn has_optimization_colors(&self) -> bool {
        self.color_registry.is_some()
    }
    // port: Compiler#getTypedScopeCreator
    pub fn get_typed_scope_creator(
        &mut self,
    ) -> &mut crate::typed_scope_creator::TypedScopeCreator {
        if self.typed_scope_creator.is_none() {
            assert!(
                !self.has_type_checking_run(),
                "Attempted to re-initialize TypedScopeCreator after it had been cleared"
            );
            self.typed_scope_creator =
                Some(crate::typed_scope_creator::TypedScopeCreator::new(self));
        }
        self.typed_scope_creator.as_mut().unwrap()
    }
    // port: Compiler#clearTypedScopeCreator
    pub fn clear_typed_scope_creator(&mut self) {
        self.typed_scope_creator = None;
    }
    /// Rust-only: moves the typed scope creator out of the compiler (creating it like
    /// `getTypedScopeCreator`), for a pass that holds it while it also borrows the compiler
    /// (TypeInferencePass). `restore_typed_scope_creator` puts it back.
    pub fn take_typed_scope_creator(&mut self) -> crate::typed_scope_creator::TypedScopeCreator {
        self.get_typed_scope_creator();
        self.typed_scope_creator.take().unwrap()
    }
    /// Rust-only: puts back the creator `take_typed_scope_creator` moved out.
    pub fn restore_typed_scope_creator(
        &mut self,
        creator: crate::typed_scope_creator::TypedScopeCreator,
    ) {
        self.typed_scope_creator = Some(creator);
    }
    // port: Compiler#getTopScope
    pub fn get_top_scope(&self) -> Option<crate::typed_scope::TypedScope> {
        self.top_scope
    }
    // port: Compiler#setTopScope
    pub fn set_top_scope(&mut self, x: Option<crate::typed_scope::TypedScope>) {
        closure_rhino::check_state!(
            x.is_none_or(|x| x.get_parent(self).is_none()),
            &x.map_or_else(|| "null".to_owned(), |x| x.to_string(self))
        );
        self.top_scope = x;
    }
    // port: Compiler#getReverseAbstractInterpreter
    /// The interpreter is a shared object (Java returns the same instance); callers hold the `Arc`
    /// while they pass this compiler to it.
    pub fn get_reverse_abstract_interpreter(
        &mut self,
    ) -> std::sync::Arc<dyn crate::reverse_abstract_interpreter::ReverseAbstractInterpreter> {
        use crate::chainable_reverse_abstract_interpreter::ChainableReverseAbstractInterpreter;
        if self.abstract_interpreter.is_none() {
            let mut interpreter: std::sync::Arc<dyn ChainableReverseAbstractInterpreter> =
                crate::semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter::new(
                    self.get_type_registry(),
                );
            if self.get_options().get_closure_pass() {
                interpreter =
                    crate::closure_reverse_abstract_interpreter::ClosureReverseAbstractInterpreter::new(
                        self.get_type_registry(),
                    )
                    .append(interpreter)
                    .get_first();
            }
            self.abstract_interpreter = Some(interpreter);
        }
        self.abstract_interpreter.clone().unwrap()
    }
    // port: Compiler#getTypeValidator
    pub fn get_type_validator(&mut self) -> std::sync::Arc<crate::type_validator::TypeValidator> {
        if self.type_validator.is_none() {
            assert!(
                !self.has_type_checking_run(),
                "Attempted to re-initialize TypeValidator after it had been cleared"
            );
            self.type_validator = Some(std::sync::Arc::new(
                crate::type_validator::TypeValidator::new(self),
            ));
        }
        std::sync::Arc::clone(self.type_validator.as_ref().unwrap())
    }
    // port: Compiler#getTypeMismatches
    pub fn get_type_mismatches(&mut self) -> Vec<crate::type_mismatch::TypeMismatch> {
        if self.type_checking_has_run {
            self.get_type_validator().get_mismatches()
        } else {
            panic!("Can't ask for type mismatches before type checking.");
        }
    }
    // port: Compiler#getTranspilationNamespace
    pub fn get_transpilation_namespace(&mut self) -> &mut crate::global_namespace::GlobalNamespace {
        if self.transpilation_namespace.is_none() {
            let js = self.js_root.unwrap();
            let mut gn =
                crate::global_namespace::GlobalNamespace::new(self, self.externs_root.unwrap(), js);
            gn.set_should_traverse_script_predicate(Arc::new(|compiler, script| {
                script.is_from_externs(compiler)
                    || script.is_first_child_of(compiler, compiler.get_js_root())
            }));
            gn.get_name_forest(self);
            self.transpilation_namespace = Some(gn);
        }
        self.transpilation_namespace.as_mut().unwrap()
    }
    /// Rust-only: lends out the transpilation namespace (built as `getTranspilationNamespace`
    /// builds it) so its lazy lookups can take the compiler; give it back with
    /// `restore_transpilation_namespace`.
    pub(crate) fn take_transpilation_namespace(
        &mut self,
    ) -> crate::global_namespace::GlobalNamespace {
        self.get_transpilation_namespace();
        self.transpilation_namespace.take().unwrap()
    }
    /// Rust-only: returns the namespace lent by `take_transpilation_namespace`.
    pub(crate) fn restore_transpilation_namespace(
        &mut self,
        namespace: crate::global_namespace::GlobalNamespace,
    ) {
        self.transpilation_namespace = Some(namespace);
    }

    /// Rust-only: `GlobalNamespace` queries take the compiler, so the lazily created transpilation
    /// namespace (`getTranspilationNamespace`) is lent out of the compiler for the duration of `f`.
    pub fn with_transpilation_namespace<R>(
        &mut self,
        f: impl FnOnce(&mut crate::global_namespace::GlobalNamespace, &mut Compiler) -> R,
    ) -> R {
        self.get_transpilation_namespace();
        let mut namespace = self.transpilation_namespace.take().unwrap();
        let result = f(&mut namespace, self);
        self.transpilation_namespace = Some(namespace);
        result
    }
    // port: Compiler#getModuleMetadataMap
    pub fn get_module_metadata_map(
        &self,
    ) -> Option<&Arc<crate::modules::module_metadata_map::ModuleMetadataMap>> {
        self.module_metadata_map.as_ref()
    }
    // port: Compiler#setModuleMetadataMap
    pub fn set_module_metadata_map(
        &mut self,
        map: Arc<crate::modules::module_metadata_map::ModuleMetadataMap>,
    ) {
        self.module_metadata_map = Some(map);
    }
    // port: Compiler#getModuleMap
    pub fn get_module_map(&self) -> Option<&Arc<crate::modules::module_map::ModuleMap>> {
        self.module_map.as_ref()
    }
    // port: Compiler#setModuleMap
    pub fn set_module_map(&mut self, map: Arc<crate::modules::module_map::ModuleMap>) {
        self.module_map = Some(map);
    }
    // port: Compiler#reportDisambiguatePropertiesSummary
    pub fn report_disambiguate_properties_summary(&self, summary: impl FnOnce() -> String) {
        if let Some(tracker) = &self.tracker {
            tracker
                .lock()
                .unwrap()
                .set_disambiguate_properties_summary(summary());
        }
    }
    // port: Compiler#reportAmbiguatePropertiesSummary
    pub fn report_ambiguate_properties_summary(&self, summary: impl FnOnce() -> String) {
        if let Some(tracker) = &self.tracker {
            tracker
                .lock()
                .unwrap()
                .set_ambiguate_properties_summary(summary());
        }
    }
    // port: Compiler#getRuntimeJsLibManager
    ///
    /// Java shares the manager object between the compiler and the AstFactories it creates; Rust
    /// shares it as `Arc<Mutex<..>>`. Methods that reach the compiler take it as an argument:
    /// `compiler.get_runtime_js_lib_manager().lock().unwrap().ensure_library_injected(compiler, ..)`.
    pub fn get_runtime_js_lib_manager(
        &self,
    ) -> Arc<std::sync::Mutex<crate::js::runtime_js_lib_manager::RuntimeJsLibManager>> {
        Arc::clone(
            self.runtime_js_lib_manager
                .as_ref()
                .expect("null RuntimeJsLibManager"),
        )
    }
    /// Java's `getRuntimeJsLibManager()` returns null before `init`; this is that nullable view
    /// (`get_runtime_js_lib_manager` panics instead).
    // port: Compiler#getRuntimeJsLibManager (nullable)
    pub fn get_runtime_js_lib_manager_or_null(
        &self,
    ) -> Option<&Arc<std::sync::Mutex<crate::js::runtime_js_lib_manager::RuntimeJsLibManager>>>
    {
        self.runtime_js_lib_manager.as_ref()
    }
    /// Rust-only: the `ChangeTracker` the Java manager holds, split-borrowed with the arena.
    pub fn get_change_tracker_and_ast(&mut self) -> (&mut ChangeTracker, &mut Ast) {
        (&mut self.change_tracker, &mut self.ast)
    }
    // port: Compiler#createAstFactory
    pub fn create_ast_factory(&mut self) -> crate::ast_factory::AstFactory {
        let stage = self.stage;
        let runtime_js_lib_manager = self.runtime_js_lib_manager.clone();
        if self.has_type_checking_run() {
            if self.has_optimization_colors() {
                crate::ast_factory::AstFactory::create_factory_with_colors(
                    stage,
                    Arc::clone(self.get_color_registry()),
                    runtime_js_lib_manager,
                )
            } else {
                crate::ast_factory::AstFactory::create_factory_with_types(
                    stage,
                    self.get_type_registry(),
                    runtime_js_lib_manager,
                )
            }
        } else {
            crate::ast_factory::AstFactory::create_factory_without_types(
                stage,
                runtime_js_lib_manager,
            )
        }
    }
    // port: Compiler#createAstFactoryWithoutTypes
    pub fn create_ast_factory_without_types(&self) -> crate::ast_factory::AstFactory {
        crate::ast_factory::AstFactory::create_factory_without_types(
            self.stage,
            self.runtime_js_lib_manager.clone(),
        )
    }
    // port: Compiler#createDefaultExpressionDecomposer
    pub fn create_default_expression_decomposer(
        &mut self,
    ) -> crate::expression_decomposer::ExpressionDecomposer {
        let root = self.ast.new_node(closure_rhino::token::Token::SCRIPT);
        let scope = crate::scope::Scope::create_global_scope(self, root);
        let unique_name_id_supplier = self.get_unique_name_id_supplier();
        self.create_expression_decomposer(
            unique_name_id_supplier,
            closure_rhino::fast_hash::IndexSet::<_>::default(),
            scope,
        )
    }
    // port: Compiler#createExpressionDecomposer
    pub fn create_expression_decomposer(
        &mut self,
        unique_name_id_supplier: Arc<dyn Fn() -> String + Send + Sync>,
        known_constant_functions: closure_rhino::fast_hash::IndexSet<
            closure_rhino::js_string::JsString,
        >,
        scope: crate::scope::Scope,
    ) -> crate::expression_decomposer::ExpressionDecomposer {
        // If the output is ES5, then it may end up running on IE11, so enable a workaround
        // for one of its bugs.
        let enabled_workarounds =
            if FeatureSet::ES5.contains(self.get_options().get_output_feature_set()) {
                closure_rhino::fast_hash::IndexSet::<_>::from_iter([
                    crate::expression_decomposer::Workaround::BROKEN_IE11_LOCATION_ASSIGN,
                ])
            } else {
                closure_rhino::fast_hash::IndexSet::<_>::default()
            };
        crate::expression_decomposer::ExpressionDecomposer::new(
            self,
            unique_name_id_supplier,
            known_constant_functions,
            scope,
            enabled_workarounds,
        )
    }
}

pub const SYNTHETIC_FILE_NAME_PREFIX: &str = " [synthetic:";
pub static SYNTHETIC_CODE_INPUT_ID: std::sync::LazyLock<InputId> =
    std::sync::LazyLock::new(|| InputId::new(" [synthetic:input] "));
impl Compiler {
    // port: Compiler#getSynthesizedExternsRoot
    pub fn get_synthesized_externs_root(&mut self) -> NodeId {
        let input = self.get_synthesized_externs_input().clone();
        input.get_ast_root(self)
    }
    // port: Compiler#getSynthesizedTypeSummaryInput
    pub fn get_synthesized_type_summary_input(&mut self) -> &CompilerInput {
        if self.synthetic_type_summary_input.is_some() {
            return self.synthetic_type_summary_input.as_ref().unwrap();
        }
        let input = CompilerInput::new_with_extern(self.synthetic_type_summary_file.clone(), true);
        let root = input.get_ast_root(self);
        self.put_compiler_input(input.clone());
        self.synthetic_type_summary_input = Some(input.clone());
        self.externs_root.unwrap().add_child_to_front(self, root);
        self.externs.insert(0, input.clone());
        self.script_node_by_filename
            .lock()
            .unwrap()
            .insert(input.get_source_file().get_name().into(), root);
        let mut builder = closure_rhino::jsdoc_info::JSDocInfo::builder();
        builder.record_type_summary();
        root.set_jsdoc_info(self, builder.build());
        self.synthetic_type_summary_input.as_ref().unwrap()
    }
    // port: Compiler#getSyntheticCodeInputId
    pub fn get_synthetic_code_input_id(&self) -> &InputId {
        &SYNTHETIC_CODE_INPUT_ID
    }
    // port: Compiler#initializeSyntheticCodeInput
    pub fn initialize_synthetic_code_input(&mut self) {
        assert!(
            !self.inputs_by_id.contains_key(&*SYNTHETIC_CODE_INPUT_ID),
            "Already initialized synthetic input"
        );
        let input = CompilerInput::new(SourceFile::from_code(
            SYNTHETIC_CODE_INPUT_ID.get_id_name(),
            "",
        ));
        assert!(
            !self.inputs_by_id.contains_key(input.get_input_id()),
            "Conflicting synthetic id name"
        );
        let root = input.get_ast_root(self);
        self.js_root.unwrap().add_child_to_front(self, root);
        let first_chunk = self.get_chunks().unwrap().first().unwrap().clone();
        if first_chunk.get_name() == crate::js_chunk::STRONG_CHUNK_NAME {
            first_chunk.add(input.clone());
        }
        input.set_chunk(Some(&first_chunk));
        self.put_compiler_input(input.clone());
        self.comments_per_file
            .insert(SYNTHETIC_CODE_INPUT_ID.get_id_name().into(), Vec::new());
        let ast_root = input.get_ast_root(self);
        self.report_change_to_change_scope(ast_root);
    }
    // port: Compiler#removeSyntheticCodeInput()
    pub fn remove_synthetic_code_input(&mut self) {
        self.remove_synthetic_code_input_internal(false);
    }
    // port: Compiler#mergeSyntheticCodeInput
    pub fn merge_synthetic_code_input(&mut self) {
        self.remove_synthetic_code_input_internal(true);
    }
    // port: Compiler#removeSyntheticCodeInput(boolean)
    fn remove_synthetic_code_input_internal(&mut self, merge_content_into_first_input: bool) {
        assert!(
            self.inputs_by_id.contains_key(&*SYNTHETIC_CODE_INPUT_ID),
            "Never initialized the synthetic input"
        );
        let input = self
            .inputs_by_id
            .get(&*SYNTHETIC_CODE_INPUT_ID)
            .unwrap()
            .clone();
        let ast_root = input.get_ast_root(self);
        assert!(ast_root.is_first_child_of(self, self.js_root));
        assert_eq!(*SYNTHETIC_CODE_INPUT_ID, *input.get_input_id());
        if merge_content_into_first_input && ast_root.has_children(self) {
            let next = ast_root
                .get_next(self)
                .expect("Must provide at least one source");
            Self::check_not_module(
                self,
                next,
                "Cannot remove synthetic code input until modules are rewritten",
            );
            let children: Vec<_> = ast_root.children(self).collect();
            for child in children.into_iter().rev() {
                child.detach(self);
                next.add_child_to_front(self, child);
            }
            self.report_change_to_change_scope(next);
        }
        ast_root.detach(self);
        self.report_change_to_change_scope(ast_root);
        ast_root.set_deleted(self, true);
        crate::node_util::NodeUtil::mark_functions_deleted(self, ast_root);
        input.get_chunk().unwrap().remove(&input);
        self.inputs_by_id.shift_remove(input.get_input_id());
    }
    // port: Compiler#checkNotModule
    fn check_not_module(ast: &Ast, script: NodeId, message: &str) -> NodeId {
        assert!(script.is_script(ast));
        if script.has_children(ast) {
            assert!(
                !script.get_first_child(ast).unwrap().is_module_body(ast),
                "{message}"
            );
        }
        script
    }
    // port: Compiler#getNodeForCodeInsertion
    pub fn get_node_for_code_insertion(
        &mut self,
        chunk: Option<&crate::js_chunk::JSChunk>,
    ) -> NodeId {
        if let Some(input) = self.inputs_by_id.get(&*SYNTHETIC_CODE_INPUT_ID).cloned() {
            return input.get_ast_root(self);
        }
        if let Some(chunk) = chunk {
            let inputs = chunk.get_inputs();
            assert!(!inputs.is_empty(), "Root chunk has no inputs");
            let root = inputs[0].get_ast_root(self);
            Self::check_not_module(self, root, "Cannot insert code into a module")
        } else {
            let inputs = self.get_inputs_in_order();
            assert!(
                self.chunk_graph.is_some() && !inputs.is_empty(),
                "No inputs"
            );
            let root = inputs[0].get_ast_root(self);
            Self::check_not_module(self, root, "Cannot insert code into a module")
        }
    }
    // port: Compiler#getNodeForRuntimeCodeInsertion
    pub fn get_node_for_runtime_code_insertion(&mut self) -> NodeId {
        if self.get_options().get_typed_ast_output_file().is_none() {
            self.get_node_for_code_insertion(None)
        } else {
            let input = self.get_synthesized_type_summary_input().clone();
            input.get_ast_root(self)
        }
    }
    // port: Compiler#initWebpackMap
    pub fn init_webpack_map(&mut self, map: IndexMap<String, String>) {
        self.input_path_by_webpack_id = map;
    }
    // port: Compiler#setDebugMessage
    pub fn set_debug_message(&mut self, message: String) {
        self.compiler_executor.set_debug_message(message.clone());
        self.debug_message = Some(message);
    }
    // port: Compiler#createCompilerExecutor
    pub fn create_compiler_executor() -> crate::compiler_executor::CompilerExecutor {
        crate::compiler_executor::CompilerExecutor::default()
    }
    // port: Compiler#getCompilerExecutor
    pub fn get_compiler_executor(&self) -> &crate::compiler_executor::CompilerExecutor {
        &self.compiler_executor
    }
}

impl Compiler {
    // port: Compiler#maybePrintSourceAfterEachPass
    pub fn maybe_print_source_after_each_pass(&mut self, pass_name: &str) {
        if !self.get_options().should_print_source_after_each_pass() {
            return;
        }
        let current_js_source = self.get_current_js_source();
        if self.last_js_source.as_ref() == Some(&current_js_source) {
            return;
        }
        if self.is_debug_logging_enabled() {
            self.create_or_reopen_indexed_log("Compiler", "source_after_pass", &[pass_name])
                .log(&mut || current_js_source.to_string_lossy());
        } else {
            eprintln!();
            if let Some(message) = &self.debug_message {
                eprintln!("// DEBUG: {message}");
            }
            eprintln!(
                "// {pass_name} yields:\n// ************************************\n{current_js_source}\n// ************************************"
            );
        }
        self.last_js_source = Some(current_js_source);
    }
    // port: Compiler#getCurrentJsSource
    pub fn get_current_js_source(&mut self) -> closure_rhino::js_string::JsString {
        self.reset_and_intitialize_source_map();
        let files = self
            .get_options()
            .get_files_to_print_after_each_pass_regex_list()
            .clone();
        let chunks = self
            .get_options()
            .get_chunks_to_print_after_each_pass_regex_list()
            .clone();
        let qnames: closure_rhino::fast_hash::IndexSet<String> = self
            .get_options()
            .get_qname_uses_to_print_after_each_pass_list()
            .iter()
            .cloned()
            .collect();
        let mut builder = CodeBuilder::default();
        if files.is_empty() && chunks.is_empty() && qnames.is_empty() {
            return self.to_source();
        }
        if !files.is_empty() {
            for root in [self.externs_root.unwrap(), self.js_root.unwrap()] {
                for file_node in root.children(self).collect::<Vec<_>>() {
                    let file_name = file_node.get_source_file_name(self).unwrap();
                    for regex in &files {
                        if java_matches(&file_name, regex) {
                            builder
                                .append(format!("// {file_name}\n"))
                                .append(self.to_source_for_node_utf16(file_node));
                            break;
                        }
                    }
                }
            }
            if builder.get_length() == 0 {
                builder.append(format!(
                    "// No files matched any of: [{}]",
                    files.join(", ")
                ));
            }
        }
        if !chunks.is_empty() {
            let all_chunks = self.get_chunks().unwrap();
            let mut unmatched_chunk_names = Vec::new();
            let mut tracker =
                crate::compiler_license_tracker::ChunkGraphAwareLicenseTracker::new(self);
            for chunk in all_chunks {
                let name = chunk.get_name();
                tracker.set_current_chunk_context(chunk.clone());
                for regex in &chunks {
                    if java_matches(&name, regex) {
                        builder
                            .append(format!("// module '{name}'\n"))
                            .append(self.to_source_for_chunk_with_tracker(&mut tracker, &chunk));
                        break;
                    }
                }
                unmatched_chunk_names.push(name);
            }
            if builder.get_length() == 0 {
                builder.append("// No chunks were matched:\n");
                for name in unmatched_chunk_names {
                    builder.append(format!("// {name}\n"));
                }
            }
        }
        if !qnames.is_empty() {
            let mut original_to_new_qname_map: IndexMap<
                String,
                closure_rhino::fast_hash::IndexSet<String>,
            > = IndexMap::<_, _>::default();
            builder.append("//\n// closure-compiler: Printing all of the top-level statements\n// that contain references to these qualified names.\n//\n");
            // Java's filtered Stream has not been consumed yet; the multimap is still empty here.
            for qname in &qnames {
                builder.append(format!("// '{qname}'\n"));
                if let Some(names) = original_to_new_qname_map.get(qname) {
                    for name in names {
                        builder.append(format!("// '{name}' (originally '{qname}')\n"));
                    }
                }
            }
            builder.append("//\n");
            for statement in self.get_top_level_statements(self.js_root.unwrap()) {
                let mut found = false;
                let mut stack = vec![statement];
                while let Some(n) = stack.pop() {
                    if n.is_qualified_name(self) {
                        let qname = n.get_qualified_name(self).unwrap().to_string_lossy();
                        if qnames.contains(&qname) {
                            found = true;
                            break;
                        }
                        if let Some(original) = n.get_original_qualified_name(self) {
                            let original = original.to_string_lossy();
                            if qnames.contains(&original) {
                                original_to_new_qname_map
                                    .entry(original)
                                    .or_default()
                                    .insert(qname);
                                found = true;
                                break;
                            }
                        }
                    }
                    let children: Vec<_> = n.children(self).collect();
                    stack.extend(children.into_iter().rev());
                }
                if found {
                    builder
                        .append(format!("// {}\n", statement.get_location(self)))
                        .append(self.to_source_for_node_utf16(statement))
                        .append("\n");
                }
            }
        }
        builder.to_js_string()
    }
    // port: Compiler#getTopLevelStatements
    fn get_top_level_statements(&mut self, root: NodeId) -> Vec<NodeId> {
        struct Collector(Vec<NodeId>);
        impl crate::node_traversal::Callback for Collector {
            // port: Compiler#anonymous.AbstractPreOrderCallback.shouldTraverse
            // port: Compiler.<anonymous>#shouldTraverse
            fn should_traverse(
                &mut self,
                t: &mut crate::node_traversal::NodeTraversal<'_>,
                n: NodeId,
                _parent: Option<NodeId>,
            ) -> bool {
                if crate::node_util::NodeUtil::is_statement(t.get_compiler(), n) {
                    self.0.push(n);
                    false
                } else {
                    true
                }
            }
            fn visit(
                &mut self,
                _t: &mut crate::node_traversal::NodeTraversal<'_>,
                _n: NodeId,
                _parent: Option<NodeId>,
            ) {
            }
        }
        let mut collector = Collector(Vec::new());
        crate::node_traversal::NodeTraversal::traverse(self, root, &mut collector);
        collector.0
    }
}
fn java_matches(value: &str, pattern: &str) -> bool {
    closure_rhino::java_lang::pattern::Pattern::compile(pattern)
        .matcher(value)
        .matches()
}

#[derive(Default)]
pub struct ExternalSourceLoader;
impl ExternalSourceLoader {
    // port: Compiler.ExternalSourceLoader#loadSource
    pub fn load_source(&self, _filename: &str) -> SourceFile {
        panic!("Cannot load without a valid loader.");
    }
}
static LOGGING_LEVEL: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(crate::logger_error_manager::Level::INFO.int_value());
impl Compiler {
    // port: Compiler#getModuleTypeByName
    pub fn get_module_type_by_name(
        &self,
        module_name: &str,
    ) -> Option<crate::compiler_input::ModuleType> {
        self.module_types_by_name.get(module_name).copied()
    }
    // port: Compiler#getAstAnalyzer
    pub fn get_ast_analyzer(&self) -> crate::ast_analyzer::AstAnalyzer {
        let options = self.get_options();
        let analysis_settings = crate::ast_analyzer::Options::builder()
            .set_use_types_for_local_optimization(options.should_use_types_for_local_optimization())
            .set_assume_getters_are_pure(options.get_assume_getters_are_pure())
            .set_has_regexp_global_references(self.has_reg_exp_global_references())
            .set_assume_known_builtins_are_pure(
                options.get_assume_properties_are_statically_analyzable(),
            )
            .build();
        crate::ast_analyzer::AstAnalyzer::new(
            analysis_settings,
            self.get_accessor_summary().cloned(),
        )
    }
    // port: Compiler#ensureDefaultPassConfig
    pub fn ensure_default_pass_config(&mut self) -> &crate::default_pass_config::DefaultPassConfig {
        self.get_pass_config()
            .get_base_pass_config()
            .as_default_pass_config()
            .expect("PassConfigs must eventually delegate to the DefaultPassConfig")
    }
    // port: Compiler#buildKnownSymbolTable
    pub fn build_known_symbol_table(&mut self) -> crate::symbol_table::SymbolTable {
        self.get_type_registry();
        let mut symbol_table =
            crate::symbol_table::SymbolTable::new(self, self.type_registry.as_ref().unwrap());
        let externs = self.externs_root.unwrap();
        let js = self.js_root.unwrap();
        if let Some(creator) = &self.typed_scope_creator {
            symbol_table.add_scopes(creator.get_all_memoized_scopes());
            symbol_table.add_symbols_from(creator);
        } else {
            symbol_table.find_scopes(externs, js);
        }
        let namespace = crate::global_namespace::GlobalNamespace::new(self, externs, js);
        symbol_table.add_symbols_from(&namespace);
        let mut creator = crate::syntactic_scope_creator::SyntacticScopeCreator::new();
        let mut refs = crate::reference_collector::ReferenceCollector::new(
            self,
            crate::reference_collector::ReferenceCollector::DO_NOTHING_BEHAVIOR,
            &mut creator,
        );
        let root = self.get_root().unwrap();
        refs.process(self, root);
        symbol_table.add_symbols_from(&refs);
        if let Some(preprocessor) = self
            .ensure_default_pass_config()
            .get_preprocessor_symbol_table()
        {
            symbol_table.add_symbols_from(&preprocessor);
        }
        symbol_table.flatten_goog_module_exports();
        symbol_table.fill_namespace_references();
        symbol_table.fill_property_scopes();
        symbol_table.fill_this_references(externs, js);
        symbol_table.fill_property_symbols(externs, js);
        symbol_table.fill_super_references(externs, js);
        symbol_table.fill_js_doc_info(externs, js);
        symbol_table.fill_symbol_visibility(externs, js);
        symbol_table.fill_goog_provide_module_requires(externs, js);
        symbol_table.remove_generated_symbols();
        symbol_table
    }
    // port: Compiler#computeCFG
    pub fn compute_cfg(&mut self) -> crate::control_flow_graph::ControlFlowGraph<NodeId> {
        let tracer = self.new_tracer("computeCFG");
        let root = self.js_root.unwrap();
        let cfg = crate::control_flow_analysis::ControlFlowAnalysis::builder()
            .set_compiler(self)
            .set_cfg_root(root)
            .set_traverse_functions(true)
            .compute_cfg(self);
        self.stop_tracer(tracer, "computeCFG");
        cfg
    }
    // port: Compiler#getAstDotGraph
    pub fn get_ast_dot_graph(&mut self) -> String {
        if let Some(root) = self.js_root {
            let cfg = crate::control_flow_analysis::ControlFlowAnalysis::builder()
                .set_compiler(self)
                .set_cfg_root(root)
                .set_traverse_functions(true)
                .compute_cfg(self);
            crate::dot_formatter::DotFormatter::to_dot_with_cfg(self, root, Some(&cfg))
        } else {
            String::new()
        }
    }
    // port: Compiler#setLoggingLevel
    pub fn set_logging_level(level: crate::logger_error_manager::Level) {
        LOGGING_LEVEL.store(level.int_value(), std::sync::atomic::Ordering::Relaxed);
    }
    // port: Compiler#initRuntimeLibraryTypedAsts
    pub fn init_runtime_library_typed_asts(
        &mut self,
        color_pool_builder: Option<&mut crate::serialization::color_pool::Builder>,
    ) {
        closure_rhino::check_state!(self.runtime_library_typed_asts.is_none());

        let path = ["", "/runtime_libs.typedast"].join("");

        let synthetic_externs_file = self.synthetic_externs_file.clone();
        let resolve_source_map_annotations =
            self.get_options().get_resolve_source_map_annotations();
        let parse_inline_source_maps = self.get_options().get_parse_inline_source_maps();
        let stream = closure_resources::jar::get_resource_as_stream(
            "com.google.javascript.jscomp.Compiler",
            &path,
        )
        .expect("runtime_libs.typedast")
        .read_all_bytes();
        let ast_data =
            crate::serialization::typed_ast_deserializer::TypedAstDeserializer::deserialize_runtime_libraries(
                self,
                synthetic_externs_file,
                color_pool_builder,
                stream,
                resolve_source_map_annotations,
                parse_inline_source_maps,
            );

        // Re-index the runtime libraries by file name rather than SourceFile object
        let mut runtime_library_typed_asts: IndexMap<String, AstSupplier> =
            IndexMap::<_, _>::default();
        for (file, supplier) in ast_data.get_filesystem().values() {
            runtime_library_typed_asts
                .entry(file.get_name().to_string())
                .or_insert_with(|| supplier.clone());
        }

        self.runtime_library_typed_asts = Some(runtime_library_typed_asts);
    }
    // port: Compiler#loadResourceContents
    pub fn load_resource_contents(&mut self, resource_name: &str, path: &str) -> NodeId {
        if self.get_life_cycle_stage().has_color_and_simplified_jsdoc() {
            let supplier=self.runtime_library_typed_asts.as_ref().expect("Must call initRuntimeLibraryTypedAsts before calling ensureLibraryInjected during optimizations").get(path).unwrap_or_else(||panic!("Missing precompiled .typedast for '{path}'. If this file is newly added, you may need to regenerate runtime_libs.typedast.textproto")).clone();
            return supplier(self);
        }
        assert!(
            !self.has_type_checking_run(),
            "runtime library injected after type checking but before optimization colors"
        );
        let original_code =
            closure_resources::resources::resource_loader::ResourceLoader::load_text_resource(
                "com.google.javascript.jscomp.Compiler",
                &format!("js/{resource_name}.js"),
            );
        let source = Arc::new(SourceFile::from_code(path, original_code));
        self.add_files_to_source_map(std::slice::from_ref(&source));
        self.parse_code_helper(source)
    }
}

impl Compiler {
    // port: Compiler#getTypedAstDeserializer
    /// Rust-only: whether ASTs come from a TypedAST filesystem (`get_typed_ast_deserializer`).
    pub(crate) fn has_typed_ast_filesystem(&self) -> bool {
        self.typed_ast_filesystem.is_some()
    }
    pub fn get_typed_ast_deserializer(&mut self, file: &SourceFile) -> Option<AstSupplier> {
        let filesystem = self.typed_ast_filesystem.as_mut()?;
        let ast = filesystem.shift_remove(&(std::ptr::from_ref(file) as usize));
        assert!(
            ast.is_some()
                || file.get_name().starts_with(SYNTHETIC_FILE_NAME_PREFIX)
                || Self::is_fill_file_name(file.get_name()),
            "TypedAST filesystem initialized, but missing requested file: {}",
            file.get_name()
        );
        ast
    }

    /// Initializes a compiler with deserialized state from the given TypedAst.List
    ///
    /// This method initializes all the state needed to run `.stage2Passes()` or do any similar
    /// sort of optimization work.
    ///
    /// `typed_ast_list_stream`: a binary-serialized TypedAst.List proto
    /// Rust-only: Java's public final field `Compiler.SYNTHETIC_EXTERNS_FILE`.
    pub fn synthetic_externs_file(&self) -> Arc<SourceFile> {
        self.synthetic_externs_file.clone()
    }

    // port: Compiler#initWithTypedAstFilesystem
    pub fn init_with_typed_ast_filesystem(
        &mut self,
        externs: &[Arc<SourceFile>],
        sources: &[Arc<SourceFile>],
        mut options: CompilerOptions,
        typed_ast_list_stream: &mut dyn std::io::Read,
    ) {
        let mut files: Vec<Arc<SourceFile>> = Vec::new();
        for file in externs.iter().chain(sources) {
            if !files.iter().any(|f| Arc::ptr_eq(f, file)) {
                files.push(file.clone());
            }
        }

        options.set_merged_precompiled_libraries(true);

        self.init_options(options.clone());
        self.init(externs, sources, options);
        self.merge_and_deserialize_typed_asts(&files, typed_ast_list_stream);
    }

    /// Initializes a compiler with deserialized state from the given TypedAst.List
    ///
    /// This method initializes all the state needed to run `.stage2Passes()` or do any similar
    /// sort of optimization work.
    ///
    /// `typed_ast_list_stream`: a binary-serialized TypedAst.List proto
    // port: Compiler#initChunksWithTypedAstFilesystem
    pub fn init_chunks_with_typed_ast_filesystem(
        &mut self,
        externs: &[Arc<SourceFile>],
        chunks: Vec<crate::js_chunk::JSChunk>,
        mut options: CompilerOptions,
        typed_ast_list_stream: &mut dyn std::io::Read,
    ) {
        let mut files: Vec<Arc<SourceFile>> = Vec::new();
        let mut add = |file: Arc<SourceFile>| {
            if !files.iter().any(|f| Arc::ptr_eq(f, &file)) {
                files.push(file);
            }
        };
        for file in externs {
            add(file.clone());
        }
        for chunk in &chunks {
            for input in chunk.get_inputs() {
                add(input.get_source_file_arc());
            }
        }

        options.set_merged_precompiled_libraries(true);

        self.init_options(options.clone());
        self.init_chunks(externs, chunks, options);
        self.merge_and_deserialize_typed_asts(&files, typed_ast_list_stream);
    }

    // port: Compiler#mergeAndDeserializeTypedAsts
    fn merge_and_deserialize_typed_asts(
        &mut self,
        required_input_files: &[Arc<SourceFile>],
        typed_ast_list_stream: &mut dyn std::io::Read,
    ) {
        closure_rhino::check_state!(self.typed_ast_filesystem.is_none());
        self.maybe_set_tracker();

        self.set_life_cycle_stage(
            crate::abstract_compiler::LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC,
        );
        // To speed up builds that don't run type-based optimizations, skip type deserialization
        let deserialize_types = self.get_options().requires_types_for_optimization();
        let resolve_source_map_annotations =
            self.get_options().get_resolve_source_map_annotations();
        let parse_inline_source_maps = self.get_options().get_parse_inline_source_maps();

        // Rust-only: the deserializer reads a byte slice; Java hands it the InputStream, whose
        // IOException it rethrows unchecked.
        let mut typed_ast_bytes = Vec::new();
        if let Err(e) = typed_ast_list_stream.read_to_end(&mut typed_ast_bytes) {
            panic!("{e}");
        }
        let synthetic_externs_file = self.synthetic_externs_file.clone();
        let mut ast_data = self.run_in_compiler_thread(|compiler| {
            let tracer = compiler.new_tracer("deserializeTypedAst");
            let result = crate::serialization::typed_ast_deserializer::TypedAstDeserializer::deserialize_full_ast(
                compiler,
                synthetic_externs_file,
                required_input_files,
                &typed_ast_bytes,
                deserialize_types,
                resolve_source_map_annotations,
                parse_inline_source_maps,
            );
            compiler.stop_tracer(tracer, "deserializeTypedAst");
            result
        });

        self.typed_ast_filesystem = Some(
            ast_data
                .take_filesystem()
                .into_iter()
                .map(|(key, (_, supplier))| (key, supplier))
                .collect(),
        );
        self.extern_properties_js = std::sync::OnceLock::new();
        self.extern_properties = Some(
            ast_data
                .get_extern_properties()
                .iter()
                .map(|p| p.to_string())
                .collect(),
        );
        self.color_registry = ast_data.get_color_registry().cloned();
        self.set_type_checking_has_run(deserialize_types);

        for library in ast_data.get_runtime_libraries().clone() {
            let runtime_js_lib_manager = self.get_runtime_js_lib_manager();
            runtime_js_lib_manager
                .lock()
                .unwrap()
                .ensure_library_injected(self, &library, false);
        }

        self.get_synthesized_externs_input(); // Force lazy creation.

        // TODO(lharker): refactor things to avoid this misleading 'parse' call. It's not actually
        // parsing JS source code, just doing more deserialization & building the Rhino AST.
        self.run_in_compiler_thread(Self::parse_for_compilation_internal);
    }
}

/// Rust-only: Java's `NonClosingOutputStream` over the caller's stream. The state compression
/// wrapper owns the stream it wraps (`Box<dyn Write + Send>`), so the bytes go to a shared buffer
/// that `saveState` copies to the caller's stream once the compressor is closed.
struct NonClosingOutputStream(Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for NonClosingOutputStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Rust-only: the compression stream `saveState` writes to. Java closes it to "flush uncompressed
/// bytes and close the compressor"; Rust finishes the gzip stream explicitly and drops a custom
/// wrapper's stream after flushing it.
enum StateCompressionStream {
    Gzip(
        crate::serialization::fast_gzip_output_stream::FastGzipOutputStream<NonClosingOutputStream>,
    ),
    Wrapped(Box<dyn std::io::Write + Send>),
}
impl std::io::Write for StateCompressionStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Gzip(out) => out.write(buf),
            Self::Wrapped(out) => out.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Gzip(out) => out.flush(),
            Self::Wrapped(out) => out.flush(),
        }
    }
}
impl StateCompressionStream {
    // port: OutputStream#close
    fn close(self) -> std::io::Result<()> {
        match self {
            Self::Gzip(out) => out.finish().map(|_| ()),
            Self::Wrapped(mut out) => out.flush(),
        }
    }
}

use crate::compiler_state_proto::{
    ChunkProto, FeatureProto, JSCompilerStateProto, LifeCycleStageProto, VariableMapEntryProto,
};
use crate::serialization::protobuf::Message as _;

impl Compiler {
    /// Serializable state of the compiler specific to multistage binary builds
    ///
    /// Only contains state that does not make sense in 'multilevel' binary builds (where
    /// library-level TypedASTs are the input). Such state belongs in the jscomp.TypedAst proto.
    // port: Compiler#toProto(Feature)
    fn feature_to_proto(feature: Feature) -> FeatureProto {
        // Java's Enum#name is the constant's name, which Rust's Debug prints.
        match FeatureProto::value_of(&["FEATURE_", &format!("{feature:?}")].join("")) {
            Some(proto) => proto,
            None => FeatureProto::FEATURE_UNKNOWN,
        }
    }

    // port: Compiler#toProto(LifeCycleStage)
    fn life_cycle_stage_to_proto(
        stage: crate::abstract_compiler::LifeCycleStage,
    ) -> LifeCycleStageProto {
        use crate::abstract_compiler::LifeCycleStage;
        match stage {
            LifeCycleStage::RAW => LifeCycleStageProto::LIFE_CYCLE_STAGE_RAW,
            LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC => {
                LifeCycleStageProto::LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC
            }
            LifeCycleStage::NORMALIZED => LifeCycleStageProto::LIFE_CYCLE_STAGE_NORMALIZED,
            LifeCycleStage::NORMALIZED_OBFUSCATED => {
                LifeCycleStageProto::LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED
            }
        }
    }

    // port: Compiler#toVariableMapEntries
    fn to_variable_map_entries(
        map: Option<&crate::variable_map::VariableMap>,
    ) -> Vec<VariableMapEntryProto> {
        let mut entries = Vec::new();
        if let Some(map) = map {
            for (key, value) in map.get_original_name_to_new_name_map() {
                entries.push(
                    VariableMapEntryProto::new_builder()
                        .set_original_name(key.to_string())
                        .set_new_name(value.to_string())
                        .build(),
                );
            }
        }
        entries
    }

    // port: Compiler#fromProto(List<FeatureProto>)
    fn feature_set_from_proto(protos: &[FeatureProto]) -> FeatureSet {
        let mut features: closure_rhino::fast_hash::IndexSet<Feature> =
            closure_rhino::fast_hash::IndexSet::<_>::default();
        for p in protos {
            if *p != FeatureProto::FEATURE_UNKNOWN {
                // Java's Feature#valueOf throws IllegalArgumentException for an unknown name.
                let name = &p.name()["FEATURE_".len()..];
                if let Some(feature) = Feature::ALL
                    .iter()
                    .copied()
                    .find(|feature| format!("{feature:?}") == name)
                {
                    features.insert(feature);
                } else {
                    // ignore
                }
            }
        }
        FeatureSet::BARE_MINIMUM.with_set(&features.into_iter().collect::<Vec<_>>())
    }

    // port: Compiler#fromProto(LifeCycleStageProto)
    fn life_cycle_stage_from_proto(
        stage: LifeCycleStageProto,
    ) -> crate::abstract_compiler::LifeCycleStage {
        use crate::abstract_compiler::LifeCycleStage;
        match stage {
            LifeCycleStageProto::LIFE_CYCLE_STAGE_RAW => LifeCycleStage::RAW,
            LifeCycleStageProto::LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC => {
                LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC
            }
            LifeCycleStageProto::LIFE_CYCLE_STAGE_NORMALIZED => LifeCycleStage::NORMALIZED,
            LifeCycleStageProto::LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED => {
                LifeCycleStage::NORMALIZED_OBFUSCATED
            }
            #[allow(unreachable_patterns)]
            _ => LifeCycleStage::RAW,
        }
    }

    // port: Compiler#fromVariableMapEntries
    fn from_variable_map_entries(
        entries: &[VariableMapEntryProto],
    ) -> Option<crate::variable_map::VariableMap> {
        let mut map: IndexMap<
            closure_rhino::js_string::JsString,
            closure_rhino::js_string::JsString,
        > = IndexMap::<_, _>::default();
        for entry in entries {
            map.insert(
                closure_rhino::js_string::JsString::from(entry.get_original_name()),
                closure_rhino::js_string::JsString::from(entry.get_new_name()),
            );
        }
        Some(crate::variable_map::VariableMap::new(&map))
    }

    /// Returns a builder for the serializable state of the compiler.
    // port: Compiler#getCompilerStateProtoBuilder
    pub fn get_compiler_state_proto_builder(&mut self) -> JSCompilerStateProto {
        let mut builder = JSCompilerStateProto::new_builder();

        for feature in self.get_allowable_features().get_features() {
            builder = builder.add_allowable_features(Self::feature_to_proto(feature));
        }
        builder = builder
            .set_type_checking_has_run(self.type_checking_has_run)
            .set_has_reg_exp_global_references(self.has_reg_exp_global_references)
            .set_life_cycle_stage(Self::life_cycle_stage_to_proto(self.get_life_cycle_stage()))
            .set_merged_precompiled_libraries(
                self.get_options().get_merged_precompiled_libraries(),
            );

        let chunk_list = self
            .chunk_graph
            .as_ref()
            .expect("chunkGraph")
            .get_integral_chunk_array_for_serialization()
            .to_vec();
        let mut chunk_name_map: IndexMap<String, i32> = IndexMap::<_, _>::default();
        for (i, chunk) in chunk_list.iter().enumerate() {
            chunk_name_map.insert(chunk.get_name(), i as i32);
        }

        for chunk in &chunk_list {
            let mut chunk_builder = ChunkProto::new_builder();
            chunk_builder = chunk_builder.set_name(chunk.get_name());
            for dep in chunk.get_dependencies() {
                let dep_idx = chunk_name_map.get(&dep.get_name()).copied();
                closure_rhino::check_state!(
                    dep_idx.is_some(),
                    "Missing dependency %s for chunk %s",
                    dep.get_name(),
                    chunk.get_name()
                );
                chunk_builder = chunk_builder.add_dependencies(dep_idx.unwrap());
            }
            for input in chunk.get_inputs() {
                chunk_builder = chunk_builder.add_input_ids(input.get_input_id().get_id_name());
            }
            builder = builder.add_chunks(chunk_builder.build());
        }

        builder = builder
            .set_unique_name_id(
                self.unique_name_id
                    .load(std::sync::atomic::Ordering::SeqCst),
            )
            .add_all_unique_id_supplier(self.unique_id_supplier.to_proto())
            .add_all_exported_names(self.exported_names.iter().cloned());

        builder = builder.set_has_css_names(self.css_names.is_some());
        if let Some(css_names) = &self.css_names {
            builder = builder.add_all_css_names(css_names.iter().cloned());
        }

        if let Some(id_generator_map) = &self.id_generator_map {
            builder = builder.set_id_generator_map(id_generator_map.clone());
        }

        builder = builder
            .set_transpiled_files(self.transpiled_files)
            .set_id_generator_current_id(self.cross_chunk_id_generator.get_current_id())
            .set_run_j2cl_passes(self.run_j2cl_passes);

        for extern_input in &self.externs {
            builder = builder.add_externs(extern_input.get_input_id().get_id_name());
        }

        let runtime_js_lib_manager = self.get_runtime_js_lib_manager();
        let runtime_js_lib_manager = runtime_js_lib_manager.lock().unwrap();
        builder =
            builder.add_all_injected_libraries(runtime_js_lib_manager.get_injected_libraries());
        let last_injected_library = runtime_js_lib_manager.get_last_injected_library();
        drop(runtime_js_lib_manager);
        let last_injected_library_index_in_first_script =
            match (last_injected_library, self.js_root) {
                (Some(last_injected_library), Some(js_root)) if js_root.has_children(&self.ast) => {
                    js_root
                        .get_first_child(&self.ast)
                        .unwrap()
                        .get_index_of_child(&self.ast, last_injected_library)
                }
                _ => -1,
            };
        builder = builder.set_last_injected_library_index_in_first_script(
            last_injected_library_index_in_first_script,
        );

        if let Some(accessor_summary) = &self.accessor_summary {
            builder = builder.set_accessor_summary(accessor_summary.to_proto());
        }

        builder = builder.set_has_string_map(self.string_map.is_some());
        if self.string_map.is_some() {
            builder = builder
                .add_all_string_map(Self::to_variable_map_entries(self.string_map.as_deref()));
        }

        builder = builder.set_has_instrumentation_mapping(self.instrumentation_mapping.is_some());
        if self.instrumentation_mapping.is_some() {
            builder = builder.add_all_instrumentation_mapping(Self::to_variable_map_entries(
                self.instrumentation_mapping.as_deref(),
            ));
        }

        builder
    }

    /// Restore the portions of the compiler state that don't require access to the serialized AST.
    // port: Compiler#restoreFromState
    pub fn restore_from_state(&mut self, compiler_state: &JSCompilerStateProto) {
        self.allowable_features = Some(Self::feature_set_from_proto(
            compiler_state.get_allowable_features_list(),
        ));
        self.type_checking_has_run = compiler_state.get_type_checking_has_run();
        self.script_node_by_filename.lock().unwrap().clear();
        self.has_reg_exp_global_references = compiler_state.get_has_reg_exp_global_references();

        for library in compiler_state.get_injected_libraries_list() {
            self.get_runtime_js_lib_manager()
                .lock()
                .unwrap()
                .record_library_injected(library);
        }

        let stage =
            if compiler_state.get_life_cycle_stage() == LifeCycleStageProto::LIFE_CYCLE_STAGE_RAW {
                crate::abstract_compiler::LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC
            } else {
                Self::life_cycle_stage_from_proto(compiler_state.get_life_cycle_stage())
            };
        self.set_life_cycle_stage(stage);
        self.get_options_mut()
            .set_merged_precompiled_libraries(compiler_state.get_merged_precompiled_libraries());

        self.unique_name_id.store(
            compiler_state.get_unique_name_id(),
            std::sync::atomic::Ordering::SeqCst,
        );
        self.unique_id_supplier = crate::unique_id_supplier::UniqueIdSupplier::from_proto(
            compiler_state.get_unique_id_supplier_list(),
        );

        self.exported_names.clear();
        self.exported_names
            .extend(compiler_state.get_exported_names_list().iter().cloned());

        if compiler_state.get_has_css_names() {
            self.css_names = Some(
                compiler_state
                    .get_css_names_list()
                    .iter()
                    .cloned()
                    .collect(),
            );
        } else {
            self.css_names = None;
        }

        self.variable_map = None;
        self.property_map = None;

        if compiler_state.has_id_generator_map() {
            self.id_generator_map = Some(compiler_state.get_id_generator_map().to_string());
        } else {
            self.id_generator_map = None;
        }
        self.transpiled_files = compiler_state.get_transpiled_files();

        self.cross_chunk_id_generator = crate::id_generator::IdGenerator::default();
        self.cross_chunk_id_generator
            .set_current_id(compiler_state.get_id_generator_current_id());

        self.run_j2cl_passes = compiler_state.get_run_j2cl_passes();

        if compiler_state.has_accessor_summary() {
            self.accessor_summary = Some(Arc::new(
                crate::accessor_summary::AccessorSummary::from_proto(
                    compiler_state.get_accessor_summary(),
                ),
            ));
        } else {
            self.accessor_summary = None;
        }

        if compiler_state.get_has_string_map() {
            self.string_map =
                Self::from_variable_map_entries(compiler_state.get_string_map_list()).map(Arc::new);
        } else {
            self.string_map = None;
        }

        if compiler_state.get_has_instrumentation_mapping() {
            self.instrumentation_mapping =
                Self::from_variable_map_entries(compiler_state.get_instrumentation_mapping_list())
                    .map(Arc::new);
        } else {
            self.instrumentation_mapping = None;
        }

        // We don't save changeStamp, because its value turned out to be non-deterministic
        // (Reasons for this are unknown.), and there's no benefit to saving it anyway.
        // We don't save the change stamps that are stored on the AST nodes,
        // so all the AST Nodes we read in effectively have a change stamp of 0,
        // and we can just start the compiler's counter over at 1.
        self.change_tracker.reset_change_stamp();
    }

    // port: Compiler#saveState
    pub fn save_state(
        &mut self,
        output_stream: &mut (dyn std::io::Write + Send),
    ) -> std::io::Result<()> {
        // Do not close the outputstream, caller is responsible for closing it.
        let buffer: Arc<std::sync::Mutex<Vec<u8>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let non_closing_out = NonClosingOutputStream(Arc::clone(&buffer));
        self.run_in_compiler_thread(move |compiler: &mut Compiler| -> std::io::Result<()> {
            let tracer = compiler.new_tracer("serializeCompilerState");
            let compression_stream = match compiler.get_options().get_state_compression_wrapper() {
                Some(wrapper) => StateCompressionStream::Wrapped(wrapper(Box::new(non_closing_out))),
                None => StateCompressionStream::Gzip(
                    crate::serialization::fast_gzip_output_stream::FastGzipOutputStream::new(
                        non_closing_out,
                    )?,
                ),
            };

            let mut buffered_compression_stream =
                std::io::BufWriter::with_capacity(64 * 1024, compression_stream);

            let builder = compiler.get_compiler_state_proto_builder();
            let mut delimited = Vec::new();
            builder.build().write_delimited_to(&mut delimited);
            std::io::Write::write_all(&mut buffered_compression_stream, &delimited)?;
            compiler.stop_tracer(tracer, "serializeCompilerState");
            let tracer = compiler.new_tracer("serializeTypedAst");
            let include_debug_info = compiler.get_options().should_serialize_extra_debug_info();
            let externs_root = compiler.externs_root.unwrap();
            let js_root = compiler.js_root.unwrap();
            crate::compiler_pass::CompilerPass::process(
                &mut crate::serialization::serialize_typed_ast_pass::SerializeTypedAstPass::create_from_output_stream(
                    &mut buffered_compression_stream,
                    crate::serialization::serialization_options::SerializationOptions::builder()
                        .set_runtime_libraries(Vec::new())
                        .set_include_debug_info(include_debug_info)
                        .build(),
                ),
                compiler,
                externs_root,
                js_root,
            );
            compiler.stop_tracer(tracer, "serializeTypedAst");
            // Close the buffered stream to flush uncompressed bytes and close the compressor
            buffered_compression_stream
                .into_inner()
                .map_err(std::io::IntoInnerError::into_error)?
                .close()
        })?;
        let bytes = std::mem::take(&mut *buffer.lock().unwrap());
        output_stream.write_all(&bytes)
    }

    // port: Compiler#restoreState
    pub fn restore_state(
        &mut self,
        input_stream: &mut (dyn std::io::Read + Send),
    ) -> std::io::Result<()> {
        self.init_warnings_guard(self.get_options().get_warnings_guard().clone());
        self.maybe_set_tracker();

        // Rust-only: Java's NonClosingInputStream hands the caller's stream to the decompression
        // wrapper, which owns its stream here (`Box<dyn Read + Send>`): read the caller's bytes first.
        let mut input = Vec::new();
        input_stream.read_to_end(&mut input)?;
        self.run_in_compiler_thread(move |compiler: &mut Compiler| -> std::io::Result<()> {
            let tracer = compiler.new_tracer(crate::pass_names::DESERIALIZE_COMPILER_STATE);
            Self::log_fine("Deserializing the CompilerState");
            let non_closing_in: Box<dyn std::io::Read + Send> =
                Box::new(std::io::Cursor::new(input));
            let decompress_stream: Box<dyn std::io::Read + Send> =
                match compiler.get_options().get_state_decompression_wrapper() {
                    Some(wrapper) => wrapper(non_closing_in),
                    None => Box::new(flate2::read::MultiGzDecoder::new(non_closing_in)),
                };
            let mut buffered_decompress_stream =
                std::io::BufReader::with_capacity(64 * 1024, decompress_stream);
            let result = compiler.deserialize_compiler_state(&mut buffered_decompress_stream);
            drop(buffered_decompress_stream);
            Self::log_fine("Finished deserializing CompilerState");
            compiler.stop_tracer(tracer, crate::pass_names::DESERIALIZE_COMPILER_STATE);
            result
        })?;

        if let Some(tracker) = &self.tracker {
            tracker
                .lock()
                .unwrap()
                .update_after_deserialize(&self.ast, self.js_root.unwrap());
        }
        Ok(())
    }

    /// Rust-only: `logger.fine(..)` on the compiler's logger.
    fn log_fine(message: &str) {
        crate::logger_error_manager::Logger::log(
            &mut CompilerLogger,
            crate::logger_error_manager::Level::FINE,
            message,
        );
    }

    // this method must be called from within a "compiler thread" with a larger stack
    // port: Compiler#deserializeCompilerState
    fn deserialize_compiler_state(
        &mut self,
        input_stream: &mut dyn std::io::Read,
    ) -> std::io::Result<()> {
        // Do not close the input stream, caller is responsible for closing it.
        let mut bytes = Vec::new();
        input_stream.read_to_end(&mut bytes)?;
        let mut coded_input =
            crate::serialization::protobuf::CodedInputStream::new_instance(&bytes);
        let state_proto = JSCompilerStateProto::parse_delimited_from(&mut coded_input)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
            .unwrap_or_default();
        let typed_ast_stream = &bytes[coded_input.get_total_bytes_read()..];

        let _ = closure_rhino::check_not_null!(
            self.chunk_graph.as_ref(),
            "Did you forget to call .init or .initChunks before restoreState?"
        );
        let mut extern_files_builder: IndexMap<String, Arc<SourceFile>> =
            IndexMap::<_, _>::default();
        let mut code_files_builder: IndexMap<String, Arc<SourceFile>> = IndexMap::<_, _>::default();
        let mut all_input_files: Vec<Arc<SourceFile>> = Vec::new();
        let mut all_input_file_keys: closure_rhino::fast_hash::IndexSet<usize> =
            closure_rhino::fast_hash::IndexSet::<_>::default();
        for input in self.chunk_graph.as_ref().unwrap().get_all_inputs() {
            let file = input.get_source_file_arc();
            if all_input_file_keys.insert(Arc::as_ptr(&file) as usize) {
                all_input_files.push(file.clone());
            }
            Self::immutable_map_put(
                &mut code_files_builder,
                input.get_input_id().get_id_name(),
                file,
            );
        }
        for extern_input in &self.externs {
            let file = extern_input.get_source_file_arc();
            if all_input_file_keys.insert(Arc::as_ptr(&file) as usize) {
                all_input_files.push(file.clone());
            }
            Self::immutable_map_put(
                &mut extern_files_builder,
                extern_input.get_input_id().get_id_name(),
                file,
            );
        }

        let synthetic_externs_file = self.synthetic_externs_file.clone();
        let resolve_source_map_annotations =
            self.get_options().get_resolve_source_map_annotations();
        let parse_inline_source_maps = self.get_options().get_parse_inline_source_maps();
        let mut deserialized_ast =
            crate::serialization::typed_ast_deserializer::TypedAstDeserializer::deserialize_full_ast(
                self,
                synthetic_externs_file,
                &all_input_files,
                typed_ast_stream,
                state_proto.get_type_checking_has_run(),
                resolve_source_map_annotations,
                parse_inline_source_maps,
            );

        self.restore_from_state(&state_proto);

        // Restore the chunk graph
        let mut chunks: Vec<crate::js_chunk::JSChunk> = Vec::new();
        for chunk_proto in state_proto.get_chunks_list() {
            let chunk = crate::js_chunk::JSChunk::new(chunk_proto.get_name());
            chunks.push(chunk);
        }
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_proto = state_proto.get_chunks(i as i32);
            for &dep_idx in chunk_proto.get_dependencies_list() {
                chunk.add_dependency(&chunks[dep_idx as usize]);
            }
        }
        self.chunk_graph = Some(Arc::new(
            crate::js_chunk_graph::JSChunkGraph::new(chunks.clone())
                .unwrap_or_else(|e| panic!("{e}")),
        ));

        // Restore TypedAST and related fields
        self.extern_properties_js = std::sync::OnceLock::new();
        self.extern_properties = Some(
            deserialized_ast
                .get_extern_properties()
                .iter()
                .map(|p| p.to_string())
                .collect(),
        );
        let externs_root = IR::root(self, &[]);
        let js_root = IR::root(self, &[]);
        self.extern_and_js_root = Some(IR::root(self, &[externs_root, js_root]));
        self.externs_root = Some(externs_root);
        self.change_tracker.set_externs_root(externs_root);
        self.externs_reference_summary = None;
        self.js_root = Some(js_root);
        self.inputs_by_id.clear();
        self.externs.clear();

        self.color_registry = deserialized_ast.get_color_registry().cloned();

        // Tells CompilerInput::getRoot to deserialize an AST rather than re-parsing the file
        self.typed_ast_filesystem = Some(
            deserialized_ast
                .take_filesystem()
                .into_iter()
                .map(|(key, (_, supplier))| (key, supplier))
                .collect(),
        );

        // overwrite any existing CompilerInput instances. Reuse the SourceFiles created by
        // TypedAstDeserializer; otherwise lookups in this.typedAstFilesystem will fail.
        let extern_files = extern_files_builder;
        let code_files = code_files_builder;
        for extern_name in state_proto.get_externs_list() {
            if extern_name == self.synthetic_externs_file.get_name() {
                self.get_synthesized_externs_input();
                continue;
            }
            let extern_file = match extern_files.get(extern_name) {
                Some(file) => file.clone(),
                None => {
                    // Extern files may passed in with regular source files, and later 'hoisted' to externs
                    // because they are annotated `@externs`.
                    closure_rhino::check_not_null!(
                        code_files.get(extern_name),
                        "Missing %s",
                        extern_name
                    )
                    .clone()
                }
            };
            let input =
                CompilerInput::new_with_extern(extern_file.clone(), /* isExtern= */ true);
            let script = input.get_ast_root(self); // accesses this.typedAstFilesystem

            externs_root.add_child_to_back(self, script);
            self.put_compiler_input(input.clone());
            self.script_node_by_filename
                .lock()
                .unwrap()
                .insert(extern_file.get_name().into(), script);
            self.externs.push(input);
        }

        // Restore the inputs for each chunk.
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_proto = state_proto.get_chunks(i as i32);
            for input_id in chunk_proto.get_input_ids_list() {
                let Some(src) = code_files.get(input_id) else {
                    // The auto-generated empty fill files used to facilitate CCCM for
                    // empty chunks may not have gotten serialized, but all the others
                    // should have.
                    closure_rhino::check_state!(
                        Self::is_fill_file_name(input_id),
                        "Missing %s",
                        input_id
                    );
                    continue;
                };
                let input = CompilerInput::new(src.clone());
                let script = input.get_ast_root(self); // accesses this.typedAstFilesystem

                js_root.add_child_to_back(self, script);
                self.script_node_by_filename
                    .lock()
                    .unwrap()
                    .insert(src.get_name().into(), script);

                self.put_compiler_input(input.clone()); // overwrite the old input
                chunk.add(input);
            }
        }

        for library in deserialized_ast.get_runtime_libraries().clone() {
            let runtime_js_lib_manager = self.get_runtime_js_lib_manager();
            runtime_js_lib_manager
                .lock()
                .unwrap()
                .ensure_library_injected(self, &library, false);
        }

        self.typed_ast_filesystem = None; // allow garbage collection

        if state_proto.get_last_injected_library_index_in_first_script() != -1 {
            let last_injected_library = js_root
                .get_first_child(&self.ast)
                .unwrap()
                .get_child_at_index(
                    &self.ast,
                    state_proto.get_last_injected_library_index_in_first_script(),
                );
            self.get_runtime_js_lib_manager()
                .lock()
                .unwrap()
                .set_last_injected_library(last_injected_library);
        }
        Ok(())
    }

    /// Rust-only: `ImmutableMap.Builder#put` + `buildOrThrow`, which throws on a duplicate key.
    fn immutable_map_put(
        map: &mut IndexMap<String, Arc<SourceFile>>,
        key: &str,
        file: Arc<SourceFile>,
    ) {
        let previous = map.insert(key.to_string(), file);
        closure_rhino::check_argument!(
            previous.is_none(),
            "Multiple entries with same key: %s",
            key
        );
    }
}
