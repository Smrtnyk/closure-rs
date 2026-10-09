/*
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/modules/ModuleMapCreator.java,
//   src/com/google/javascript/jscomp/modules/UnresolvedModule.java.

//! Creates a [`ModuleMap`].
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    deps::module_loader::ModulePath,
    diagnostic_type::DiagnosticType,
    modules::{
        binding::Binding,
        closure_module_processor::ClosureModuleProcessor,
        es_module_processor::EsModuleProcessor,
        export::Export,
        goog_es_imports::GoogEsImports,
        import::Import,
        module::Module,
        module_map::ModuleMap,
        module_metadata_map::{ModuleMetadata, ModuleMetadataMap, ModuleType},
        module_request_resolver::ModuleRequestResolver,
        non_es_module_processor::NonEsModuleProcessor,
        resolve_export_result::ResolveExportResult,
        unresolved_module::{UnresolvedModule, UnresolvedModuleId, UnresolvedModules},
    },
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};
use std::sync::Arc;

// port: ModuleMapCreator#DOES_NOT_HAVE_EXPORT
pub static DOES_NOT_HAVE_EXPORT: DiagnosticType = DiagnosticType::error(
    "JSC_DOES_NOT_HAVE_EXPORT",
    "Requested module does not have an export \"{0}\".",
);

// port: ModuleMapCreator#DOES_NOT_HAVE_EXPORT_WITH_DETAILS
pub static DOES_NOT_HAVE_EXPORT_WITH_DETAILS: DiagnosticType = DiagnosticType::error(
    "JSC_DOES_NOT_HAVE_EXPORT_WITH_DETAILS",
    "Requested module does not have an export \"{0}\".{1}",
);

/// The anonymous `UnresolvedModule` of
/// `ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule`.
pub struct MissingNonClosureModule {
    path: ModulePath,
    metadata: Arc<ModuleMetadata>,
}

impl MissingNonClosureModule {
    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule (UnresolvedModule#reset)
    pub(crate) fn reset(&mut self) {}

    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule (UnresolvedModule#resolve)
    pub(crate) fn resolve(&self, _module_specifier: Option<&JsString>) -> Arc<Module> {
        Arc::new(
            Module::builder()
                .bound_names(IndexMap::<_, _>::default())
                .namespace(IndexMap::<_, _>::default())
                .local_name_to_local_export(IndexMap::<_, _>::default())
                .path(Some(self.path.clone()))
                .metadata(self.metadata.clone())
                .build(),
        )
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule (UnresolvedModule#metadata)
    pub(crate) fn metadata(&self) -> Arc<ModuleMetadata> {
        self.metadata.clone()
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule (UnresolvedModule#getExportedNames(ModuleRequestResolver))
    pub(crate) fn get_exported_names(&self) -> Arc<IndexSet<JsString>> {
        Arc::new(IndexSet::<_>::default())
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule (UnresolvedModule#getExportedNames(ModuleRequestResolver,Set))
    pub(crate) fn get_exported_names_visited(&self) -> Arc<IndexSet<JsString>> {
        Arc::new(IndexSet::<_>::default())
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule (UnresolvedModule#resolveExport)
    pub(crate) fn resolve_export(&self, export_name: &JsString) -> ResolveExportResult {
        ResolveExportResult::of(Binding::from_export(
            Export::builder()
                .local_name(Some(export_name.clone()))
                .module_metadata(self.metadata.clone())
                .module_path(Some(self.path.clone()))
                .closure_namespace(None)
                .build(),
            /* sourceNode= */ None,
        ))
    }
}

// port: ModuleMapCreator.ModuleRequestResolverImpl
struct ModuleRequestResolverImpl<'a> {
    non_es_module_processor: &'a mut NonEsModuleProcessor,
    unresolved_modules: &'a mut IndexMap<String, UnresolvedModuleId>,
    unresolved_modules_by_closure_namespace: &'a mut IndexMap<JsString, UnresolvedModuleId>,
    modules: &'a mut UnresolvedModules,
}

impl ModuleRequestResolverImpl<'_> {
    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingNonClosureModule
    fn get_fallback_for_missing_non_closure_module(
        &mut self,
        path: ModulePath,
    ) -> UnresolvedModuleId {
        let metadata = ModuleMetadata::builder()
            .root_node(None)
            .path(Some(path.clone()))
            .module_type(ModuleType::ES6_MODULE)
            .is_test_only(false)
            .uses_closure(false)
            .build();
        self.modules.add(UnresolvedModule::MissingNonClosure(
            MissingNonClosureModule { path, metadata },
        ))
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#getFallbackForMissingClosureModule
    fn get_fallback_for_missing_closure_module(
        &mut self,
        namespace: &JsString,
    ) -> UnresolvedModuleId {
        let metadata = ModuleMetadata::builder()
            .add_goog_namespace(namespace.clone())
            .is_test_only(false)
            .module_type(ModuleType::GOOG_PROVIDE)
            .path(None)
            .root_node(None)
            .uses_closure(true)
            .build();
        let module = self
            .non_es_module_processor
            .process_module(metadata, /* path= */ None, /* script= */ None);
        self.modules.add(module)
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#resolveForClosure
    fn resolve_for_closure(&mut self, namespace: &JsString) -> Option<UnresolvedModuleId> {
        if let Some(module) = self.unresolved_modules_by_closure_namespace.get(namespace) {
            return Some(*module);
        }
        let module = self.get_fallback_for_missing_closure_module(namespace);
        self.unresolved_modules_by_closure_namespace
            .insert(namespace.clone(), module);
        Some(module)
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#resolve(String,ModulePath,Node)
    fn resolve_request(
        &mut self,
        compiler: &mut AbstractCompiler,
        module_request: &JsString,
        module_path: &ModulePath,
        for_line_info: Option<NodeId>,
    ) -> Option<UnresolvedModuleId> {
        if GoogEsImports::is_goog_import_specifier(module_request) {
            let namespace =
                GoogEsImports::get_closure_id_from_goog_import_specifier(module_request);
            return self.resolve_for_closure(&namespace);
        }

        let for_line_info = for_line_info.unwrap();
        let mut requested_path = module_path.resolve_js_module(
            &module_request.to_string(),
            for_line_info.get_source_file_name(compiler).as_deref(),
            for_line_info.get_lineno(compiler),
            for_line_info.get_charno(compiler),
        );
        // The module loader reported to the compiler (its ErrorHandler) during the resolution.
        compiler.flush_module_errors();
        if requested_path.is_none() {
            let path = module_path.resolve_module_as_path(&module_request.to_string());
            if !self.unresolved_modules.contains_key(&path.to_module_name()) {
                let module_name = path.to_module_name();
                let module = self.get_fallback_for_missing_non_closure_module(path);
                self.unresolved_modules.insert(module_name, module);
                return Some(module);
            }
            requested_path = Some(path);
        }
        self.unresolved_modules
            .get(&requested_path.unwrap().to_module_name())
            .copied()
    }
}

impl ModuleRequestResolver for ModuleRequestResolverImpl<'_> {
    // port: ModuleMapCreator.ModuleRequestResolverImpl#resolve(Import)
    fn resolve_import(
        &mut self,
        compiler: &mut AbstractCompiler,
        i: &Import,
    ) -> Option<UnresolvedModuleId> {
        match i.module_path() {
            None => self.resolve_for_closure(i.module_request()),
            Some(module_path) => self.resolve_request(
                compiler,
                i.module_request(),
                module_path,
                Some(i.import_node()),
            ),
        }
    }

    // port: ModuleMapCreator.ModuleRequestResolverImpl#resolve(Export)
    fn resolve_export(
        &mut self,
        compiler: &mut AbstractCompiler,
        e: &Export,
    ) -> Option<UnresolvedModuleId> {
        self.resolve_request(
            compiler,
            e.module_request().unwrap(),
            e.module_path().unwrap(),
            e.export_node(),
        )
    }

    fn modules(&mut self) -> &mut UnresolvedModules {
        self.modules
    }
}

/// A basic interface that can scan and return information about a module.
// port: ModuleMapCreator.ModuleProcessor
pub trait ModuleProcessor {
    // port: ModuleMapCreator.ModuleProcessor#process
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        metadata: Arc<ModuleMetadata>,
        path: Option<ModulePath>,
        script: Option<NodeId>,
    ) -> UnresolvedModule;
}

/// Creates a [`ModuleMap`].
pub struct ModuleMapCreator {
    es_module_processor: EsModuleProcessor,
    closure_module_processor: ClosureModuleProcessor,
    non_es_module_processor: NonEsModuleProcessor,
    unresolved_modules: IndexMap<String, UnresolvedModuleId>,
    unresolved_modules_by_closure_namespace: IndexMap<JsString, UnresolvedModuleId>,
    module_metadata_map: Arc<ModuleMetadataMap>,
    /// Owns the `UnresolvedModule` objects the two maps above refer to.
    modules: UnresolvedModules,
}

impl ModuleMapCreator {
    // port: ModuleMapCreator#ModuleMapCreator
    pub fn new(module_metadata_map: Arc<ModuleMetadataMap>) -> Self {
        Self {
            module_metadata_map,
            es_module_processor: EsModuleProcessor::new(),
            closure_module_processor: ClosureModuleProcessor::new(),
            non_es_module_processor: NonEsModuleProcessor,
            unresolved_modules: IndexMap::<_, _>::default(),
            unresolved_modules_by_closure_namespace: IndexMap::<_, _>::default(),
            modules: UnresolvedModules::new(),
        }
    }

    // port: ModuleMapCreator#create
    fn create(&mut self, compiler: &mut AbstractCompiler) -> ModuleMap {
        self.unresolved_modules.clear();
        self.unresolved_modules_by_closure_namespace.clear();

        // There are modules that aren't associated with scripts - nested goog.modules in
        // goog.loadModule calls.
        let module_metadata_map = self.module_metadata_map.clone();
        for module_metadata in module_metadata_map.get_all_module_metadata() {
            self.process_metadata(compiler, module_metadata.clone());
        }

        self.resolve(compiler)
    }

    // port: ModuleMapCreator#resolve
    fn resolve(&mut self, compiler: &mut AbstractCompiler) -> ModuleMap {
        let mut request_resolver = ModuleRequestResolverImpl {
            non_es_module_processor: &mut self.non_es_module_processor,
            unresolved_modules: &mut self.unresolved_modules,
            unresolved_modules_by_closure_namespace: &mut self
                .unresolved_modules_by_closure_namespace,
            modules: &mut self.modules,
        };
        let mut resolved_modules: IndexMap<String, Arc<Module>> = IndexMap::<_, _>::default();
        let mut resolved_closure_modules: IndexMap<JsString, Arc<Module>> =
            IndexMap::<_, _>::default();

        // We need to resolve in a loop as any missing reference will add a fake to the
        // unresolvedModules map (see getFallback* methods above). This would cause a concurrent
        // modification exception if we just iterated over unresolvedModules. So the first loop
        // through should resolve any "known" modules, and the second any "unrecognized" modules.
        loop {
            let to_resolve: Vec<String> = request_resolver
                .unresolved_modules
                .keys()
                .filter(|k| !resolved_modules.contains_key(*k))
                .cloned()
                .collect();
            for key in to_resolve {
                let module = *request_resolver.unresolved_modules.get(&key).unwrap();
                let resolved = module.resolve(compiler, &mut request_resolver);
                resolved_modules.insert(key, resolved.clone());
                for namespace in resolved.metadata().goog_namespaces().iter() {
                    resolved_closure_modules.insert(namespace.clone(), resolved.clone());
                }
            }
            if request_resolver
                .unresolved_modules
                .keys()
                .all(|k| resolved_modules.contains_key(k))
            {
                break;
            }
        }

        loop {
            let to_resolve: Vec<JsString> = request_resolver
                .unresolved_modules_by_closure_namespace
                .keys()
                .filter(|k| !resolved_closure_modules.contains_key(*k))
                .cloned()
                .collect();
            for namespace in to_resolve {
                let module = *request_resolver
                    .unresolved_modules_by_closure_namespace
                    .get(&namespace)
                    .unwrap();
                let resolved = module.resolve(compiler, &mut request_resolver);
                resolved_closure_modules.insert(namespace, resolved);
            }
            if request_resolver
                .unresolved_modules_by_closure_namespace
                .keys()
                .all(|k| resolved_closure_modules.contains_key(k))
            {
                break;
            }
        }

        self.unresolved_modules.clear();
        self.unresolved_modules_by_closure_namespace.clear();

        ModuleMap::new(resolved_modules, resolved_closure_modules)
    }

    // port: ModuleMapCreator#process(ModuleMetadata)
    fn process_metadata(
        &mut self,
        compiler: &mut AbstractCompiler,
        module_metadata: Arc<ModuleMetadata>,
    ) {
        let processor: &mut dyn ModuleProcessor = match module_metadata.module_type() {
            ModuleType::ES6_MODULE => &mut self.es_module_processor,
            ModuleType::GOOG_MODULE | ModuleType::LEGACY_GOOG_MODULE => {
                if module_metadata.goog_namespaces().size() == 1 {
                    &mut self.closure_module_processor
                } else {
                    // this indicates some malformed Closure module. We should already have
                    // reported an error.
                    &mut self.non_es_module_processor
                }
            }
            _ => &mut self.non_es_module_processor,
        };

        let module = processor.process(
            compiler,
            module_metadata.clone(),
            module_metadata.path().cloned(),
            module_metadata.root_node(),
        );
        let module = self.modules.add(module);

        // Have to use module names as keys because path "resolution" (ModuleLoader) "respects"
        // leading slashes. Meaning that if you look up "file.js" and "/file.js" you'll get
        // different paths back. But they'll have the same module name.
        if let Some(path) = module_metadata.path() {
            self.unresolved_modules
                .insert(path.to_module_name(), module);
        }

        for namespace in module_metadata.goog_namespaces().iter() {
            self.unresolved_modules_by_closure_namespace
                .insert(namespace.clone(), module);
        }
    }
}

impl CompilerPass for ModuleMapCreator {
    // port: ModuleMapCreator#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {
        let module_map = self.create(compiler);
        compiler.set_module_map(Arc::new(module_map));
    }
}
