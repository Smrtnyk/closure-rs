/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/BrowserModuleResolver.java,
//   src/com/google/javascript/jscomp/deps/ModuleResolver.java.

use super::{
    module_loader::{
        INVALID_MODULE_PATH, LOAD_WARNING, ModuleLoader, ModuleResolverFactory, PathEscaper,
        SharedErrorHandler,
    },
    module_resolver::{ModuleResolver, ModuleResolverBase},
};
use crate::check_level::CheckLevel;
use closure_rhino::fx_hash::IndexSet;
use std::sync::{Arc, LazyLock};
pub struct BrowserModuleResolver {
    base: ModuleResolverBase,
}
impl BrowserModuleResolver {
    pub const FACTORY: &'static LazyLock<Arc<dyn ModuleResolverFactory>> = &FACTORY;
    // port: BrowserModuleResolver#BrowserModuleResolver
    pub fn new(
        module_paths: IndexSet<String>,
        module_root_paths: Vec<String>,
        error_handler: SharedErrorHandler,
        path_escaper: PathEscaper,
    ) -> Self {
        Self {
            base: ModuleResolverBase::new(
                module_paths,
                module_root_paths,
                error_handler,
                path_escaper,
            ),
        }
    }
}
struct Factory;
impl ModuleResolverFactory for Factory {
    // port: BrowserModuleResolver#FACTORY
    fn create(
        &self,
        paths: IndexSet<String>,
        roots: Vec<String>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
    ) -> Arc<dyn ModuleResolver> {
        Arc::new(BrowserModuleResolver::new(paths, roots, handler, escaper))
    }
}
pub static FACTORY: LazyLock<Arc<dyn ModuleResolverFactory>> = LazyLock::new(|| Arc::new(Factory));
impl ModuleResolver for BrowserModuleResolver {
    // port: ModuleResolver#ModuleResolver (base fields)
    fn base(&self) -> &ModuleResolverBase {
        &self.base
    }
    // port: BrowserModuleResolver#resolveJsModule
    fn resolve_js_module(
        &self,
        script_address: &str,
        module_address: &str,
        sourcename: Option<&str>,
        lineno: i32,
        colno: i32,
    ) -> Option<String> {
        if ModuleLoader::is_ambiguous_identifier(module_address) {
            self.base.report(
                CheckLevel::WARNING,
                super::module_resolver::make_js_error(
                    sourcename,
                    lineno,
                    colno,
                    &INVALID_MODULE_PATH,
                    &[module_address, "BROWSER"],
                ),
            );
            return None;
        }
        let load_address = self.base.locate(script_address, module_address);
        if load_address.is_none() {
            self.base.report(
                CheckLevel::WARNING,
                super::module_resolver::make_js_error(
                    sourcename,
                    lineno,
                    colno,
                    &LOAD_WARNING,
                    &[module_address],
                ),
            );
        }
        load_address
    }
    // port: BrowserModuleResolver#resolveJsModuleSilently
    fn resolve_js_module_silently(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        if ModuleLoader::is_ambiguous_identifier(module_address) {
            None
        } else {
            self.base.locate(script_address, module_address)
        }
    }
}
