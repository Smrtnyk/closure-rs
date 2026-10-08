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
//   src/com/google/javascript/jscomp/modules/ModuleRequestResolver.java.

//! Resolves requests for other modules.
use crate::{
    AbstractCompiler,
    modules::{
        export::Export,
        import::Import,
        unresolved_module::{UnresolvedModuleId, UnresolvedModules},
    },
};

/// Resolves requests for other modules.
///
/// The compiler argument gives access to the AST for the line information of the request (Java
/// reads it from the nodes directly) and receives the module loader's reports. Java returns `UnresolvedModule` references; the Rust port returns handles into the
/// [`UnresolvedModules`] arena the resolver exposes through [`ModuleRequestResolver::modules`].
pub trait ModuleRequestResolver {
    /// Returns the module that this import references, if it exists in the compilation.
    // port: ModuleRequestResolver#resolve(Import)
    fn resolve_import(
        &mut self,
        compiler: &mut AbstractCompiler,
        i: &Import,
    ) -> Option<UnresolvedModuleId>;

    /// Returns the module that this export references, if it exists in the compilation.
    // port: ModuleRequestResolver#resolve(Export)
    fn resolve_export(
        &mut self,
        compiler: &mut AbstractCompiler,
        e: &Export,
    ) -> Option<UnresolvedModuleId>;

    /// The arena that owns every module handle this resolver returns (Rust only: Java holds the
    /// `UnresolvedModule` objects directly).
    fn modules(&mut self) -> &mut UnresolvedModules;
}
