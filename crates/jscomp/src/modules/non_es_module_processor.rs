/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/modules/NonEsModuleProcessor.java.

//! Catch all module processor for non-ES and non-goog modules that doesn't do any scanning of
//! exports but instead will always treat every name as `munged.name`.
use crate::{
    AbstractCompiler,
    deps::module_loader::ModulePath,
    modules::{
        binding::Binding, export::Export, goog_es_imports::GoogEsImports, module::Module,
        module_map_creator::ModuleProcessor, module_metadata_map::ModuleMetadata,
        resolve_export_result::ResolveExportResult, unresolved_module::UnresolvedModule,
    },
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};
use std::sync::Arc;

/// Catch all module processor for non-ES and non-goog modules that doesn't do any scanning of
/// exports but instead will always treat every name as `munged.name`.
///
/// This includes goog.provide'd names and CommonJS modules.
#[derive(Default)]
pub struct NonEsModuleProcessor;

// port: NonEsModuleProcessor.NonEsModule
pub struct NonEsModule {
    metadata: Arc<ModuleMetadata>,
    path: Option<ModulePath>,
    script_node: Option<NodeId>,
}

impl NonEsModule {
    // port: NonEsModuleProcessor.NonEsModule#NonEsModule
    fn new(
        metadata: Arc<ModuleMetadata>,
        path: Option<ModulePath>,
        script_node: Option<NodeId>,
    ) -> Self {
        Self {
            metadata,
            path,
            script_node,
        }
    }

    // port: NonEsModuleProcessor.NonEsModule#resolveExport(ModuleRequestResolver,String)
    pub(crate) fn resolve_export(&self) -> ResolveExportResult {
        panic!("java.lang.UnsupportedOperationException");
    }

    // port: NonEsModuleProcessor.NonEsModule#resolveExport(ModuleRequestResolver,String,String,Set,Set)
    pub(crate) fn resolve_export_with_specifier(
        &self,
        module_specifier: Option<&JsString>,
        export_name: &JsString,
    ) -> ResolveExportResult {
        let mut namespace: Option<JsString> = None;
        if module_specifier.is_some_and(GoogEsImports::is_goog_import_specifier) {
            namespace = Some(GoogEsImports::get_closure_id_from_goog_import_specifier(
                module_specifier.unwrap(),
            ));
        } else if self.metadata.is_goog_provide() {
            namespace = module_specifier.cloned();
        }

        if self.metadata.is_common_js() {
            // Currently we don't scan require()s, so this only gets hit if an ES module imports a
            // CJS module. If so then this module should only have a default export.
            if *export_name != "default" {
                return ResolveExportResult::not_found();
            }
        }

        ResolveExportResult::of(Binding::from_export(
            Export::builder()
                .export_name(Some(export_name.clone()))
                .module_metadata(self.metadata.clone())
                .module_path(self.path.clone())
                .closure_namespace(namespace)
                .build(),
            self.script_node,
        ))
    }

    // port: NonEsModuleProcessor.NonEsModule#reset
    pub(crate) fn reset(&mut self) {}

    // port: NonEsModuleProcessor.NonEsModule#resolve
    pub(crate) fn resolve(&self, module_specifier: Option<&JsString>) -> Arc<Module> {
        let mut namespace: Option<JsString> = None;
        if module_specifier.is_some_and(GoogEsImports::is_goog_import_specifier) {
            namespace = Some(GoogEsImports::get_closure_id_from_goog_import_specifier(
                module_specifier.unwrap(),
            ));
        }
        Arc::new(
            Module::builder()
                .path(self.path.clone())
                .metadata(self.metadata.clone())
                .namespace(IndexMap::<_, _>::default())
                .bound_names(IndexMap::<_, _>::default())
                .local_name_to_local_export(IndexMap::<_, _>::default())
                .closure_namespace(namespace)
                .build(),
        )
    }

    // port: NonEsModuleProcessor.NonEsModule#metadata
    pub(crate) fn metadata(&self) -> Arc<ModuleMetadata> {
        self.metadata.clone()
    }

    // port: NonEsModuleProcessor.NonEsModule#getExportedNames(ModuleRequestResolver)
    pub(crate) fn get_exported_names(&self) -> Arc<IndexSet<JsString>> {
        panic!("java.lang.UnsupportedOperationException");
    }

    // port: NonEsModuleProcessor.NonEsModule#getExportedNames(ModuleRequestResolver,Set)
    pub(crate) fn get_exported_names_visited(&self) -> Arc<IndexSet<JsString>> {
        panic!("java.lang.UnsupportedOperationException");
    }
}

impl NonEsModuleProcessor {
    // port: NonEsModuleProcessor#process
    pub fn process_module(
        &mut self,
        metadata: Arc<ModuleMetadata>,
        path: Option<ModulePath>,
        script: Option<NodeId>,
    ) -> UnresolvedModule {
        UnresolvedModule::NonEs(NonEsModule::new(metadata, path, script))
    }
}

impl ModuleProcessor for NonEsModuleProcessor {
    // port: NonEsModuleProcessor#process (the processor needs no compiler)
    fn process(
        &mut self,
        _compiler: &mut AbstractCompiler,
        metadata: Arc<ModuleMetadata>,
        path: Option<ModulePath>,
        script: Option<NodeId>,
    ) -> UnresolvedModule {
        self.process_module(metadata, path, script)
    }
}
