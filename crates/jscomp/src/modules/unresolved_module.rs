/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/modules/UnresolvedModule.java.

//! A module which has had some of its imports and exports statements scanned but has yet to
//! resolve anything transitively.
use crate::{
    AbstractCompiler,
    modules::{
        closure_module_processor::UnresolvedGoogModule, es_module_processor::UnresolvedEsModule,
        export_trace::ExportTrace, module::Module, module_map_creator::MissingNonClosureModule,
        module_metadata_map::ModuleMetadata, module_request_resolver::ModuleRequestResolver,
        non_es_module_processor::NonEsModule, resolve_export_result::ResolveExportResult,
    },
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;
use std::sync::Arc;

/// Handle of a Java `UnresolvedModule` object in the [`UnresolvedModules`] arena. Java uses
/// reference equality for these objects (`equals`/`hashCode` are final), which is handle equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnresolvedModuleId(usize);

/// The concrete Java subclasses of the abstract `UnresolvedModule`.
pub enum UnresolvedModule {
    /// The anonymous class of `ModuleMapCreator.ModuleRequestResolverImpl
    /// #getFallbackForMissingNonClosureModule`.
    MissingNonClosure(MissingNonClosureModule),
    /// `EsModuleProcessor.UnresolvedEsModule`.
    Es(UnresolvedEsModule),
    /// `ClosureModuleProcessor.UnresolvedGoogModule`.
    Goog(UnresolvedGoogModule),
    /// `NonEsModuleProcessor.NonEsModule`.
    NonEs(NonEsModule),
}

/// Owns every `UnresolvedModule` of one module map creation (Java's object heap for them).
#[derive(Default)]
pub struct UnresolvedModules {
    modules: Vec<UnresolvedModule>,
}

impl UnresolvedModules {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a module (Java's `new`) and returns its handle.
    pub fn add(&mut self, module: UnresolvedModule) -> UnresolvedModuleId {
        self.modules.push(module);
        UnresolvedModuleId(self.modules.len() - 1)
    }

    pub fn get(&self, id: UnresolvedModuleId) -> &UnresolvedModule {
        &self.modules[id.0]
    }

    pub fn get_mut(&mut self, id: UnresolvedModuleId) -> &mut UnresolvedModule {
        &mut self.modules[id.0]
    }
}

impl UnresolvedModuleId {
    /// Clears any caching so that `resolve` will create a new Module.
    // port: UnresolvedModule#reset
    pub fn reset(self, resolver: &mut dyn ModuleRequestResolver) {
        match resolver.modules().get_mut(self) {
            UnresolvedModule::MissingNonClosure(m) => m.reset(),
            UnresolvedModule::Es(m) => m.reset(),
            UnresolvedModule::Goog(m) => m.reset(),
            UnresolvedModule::NonEs(m) => m.reset(),
        }
    }

    // port: UnresolvedModule#resolve(ModuleRequestResolver)
    pub fn resolve(
        self,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
    ) -> Arc<Module> {
        self.resolve_with_specifier(
            compiler,
            module_request_resolver,
            /* moduleSpecifier= */ None,
        )
    }

    /// Resolves all imports and exports and returns a resolved module.
    ///
    /// `module_specifier` is the module specifier that was used to import this module, if
    /// resolving an import.
    // port: UnresolvedModule#resolve(ModuleRequestResolver,String)
    pub fn resolve_with_specifier(
        self,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        module_specifier: Option<&JsString>,
    ) -> Arc<Module> {
        match module_request_resolver.modules().get(self) {
            UnresolvedModule::MissingNonClosure(m) => m.resolve(module_specifier),
            UnresolvedModule::Es(_) => UnresolvedEsModule::resolve(
                self,
                compiler,
                module_request_resolver,
                module_specifier,
            ),
            UnresolvedModule::Goog(_) => UnresolvedGoogModule::resolve(
                self,
                compiler,
                module_request_resolver,
                module_specifier,
            ),
            UnresolvedModule::NonEs(m) => m.resolve(module_specifier),
        }
    }

    /// Returns the metadata corresponding to this module
    // port: UnresolvedModule#metadata
    pub fn metadata(self, resolver: &mut dyn ModuleRequestResolver) -> Arc<ModuleMetadata> {
        match resolver.modules().get(self) {
            UnresolvedModule::MissingNonClosure(m) => m.metadata(),
            UnresolvedModule::Es(m) => m.metadata(),
            UnresolvedModule::Goog(m) => m.metadata(),
            UnresolvedModule::NonEs(m) => m.metadata(),
        }
    }

    /// Returns all names in this module's namespace. Names are sorted per Java's string ordering,
    /// which should be the same as JavaScript's Array.protype.sort, which is how the spec says
    /// these keys should be ordered in the ES module object.
    // port: UnresolvedModule#getExportedNames(ModuleRequestResolver)
    pub fn get_exported_names(
        self,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
    ) -> Arc<IndexSet<JsString>> {
        match module_request_resolver.modules().get(self) {
            UnresolvedModule::MissingNonClosure(m) => m.get_exported_names(),
            UnresolvedModule::Es(_) => {
                UnresolvedEsModule::get_exported_names(self, compiler, module_request_resolver)
            }
            UnresolvedModule::Goog(m) => m.get_exported_names(),
            UnresolvedModule::NonEs(m) => m.get_exported_names(),
        }
    }

    /// Returns all names in this module's namespace, sorted per Java's string ordering.
    ///
    /// `visited` is used to detect `export *` cycles.
    // port: UnresolvedModule#getExportedNames(ModuleRequestResolver,Set)
    pub fn get_exported_names_visited(
        self,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        visited: &mut IndexSet<UnresolvedModuleId>,
    ) -> Arc<IndexSet<JsString>> {
        match module_request_resolver.modules().get(self) {
            UnresolvedModule::MissingNonClosure(m) => m.get_exported_names_visited(),
            UnresolvedModule::Es(_) => UnresolvedEsModule::get_exported_names_visited(
                self,
                compiler,
                module_request_resolver,
                visited,
            ),
            UnresolvedModule::Goog(m) => m.get_exported_names_visited(),
            UnresolvedModule::NonEs(m) => m.get_exported_names_visited(),
        }
    }

    /// Resolves `export_name`: the binding if found, or a result saying that the export is
    /// ambiguous, that the module has no such export, or that some other error happened
    /// (a cycle, or a module transitively returned that there was no such export).
    // port: UnresolvedModule#resolveExport(ModuleRequestResolver,String)
    pub fn resolve_export(
        self,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        export_name: &JsString,
    ) -> ResolveExportResult {
        match module_request_resolver.modules().get(self) {
            // Overridden by the subclasses that cannot resolve without a specifier.
            UnresolvedModule::Goog(m) => m.resolve_export(export_name),
            UnresolvedModule::NonEs(m) => m.resolve_export(),
            UnresolvedModule::MissingNonClosure(_) | UnresolvedModule::Es(_) => self
                .resolve_export_with_sets(
                    compiler,
                    module_request_resolver,
                    /* moduleSpecifier= */ None,
                    export_name,
                    &mut IndexSet::<_>::default(),
                    &mut IndexSet::<_>::default(),
                ),
        }
    }

    /// Resolves `export_name`.
    ///
    /// `module_specifier` is the specifier used to reference this module, if this trace is from
    /// an import; `resolve_set` detects invalid cycles (it is invalid to reach the same exact
    /// export, same module with the same export name, in a given cycle); `export_star_set` is
    /// used for cycle checking with `export *` statements.
    // port: UnresolvedModule#resolveExport(ModuleRequestResolver,String,String,Set,Set)
    pub fn resolve_export_with_sets(
        self,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        module_specifier: Option<&JsString>,
        export_name: &JsString,
        resolve_set: &mut IndexSet<ExportTrace>,
        export_star_set: &mut IndexSet<UnresolvedModuleId>,
    ) -> ResolveExportResult {
        match module_request_resolver.modules().get(self) {
            UnresolvedModule::MissingNonClosure(m) => m.resolve_export(export_name),
            UnresolvedModule::Es(_) => UnresolvedEsModule::resolve_export_cached(
                self,
                compiler,
                module_request_resolver,
                module_specifier,
                export_name,
                resolve_set,
                export_star_set,
            ),
            UnresolvedModule::Goog(m) => m.resolve_export(export_name),
            UnresolvedModule::NonEs(m) => {
                m.resolve_export_with_specifier(module_specifier, export_name)
            }
        }
    }
}
