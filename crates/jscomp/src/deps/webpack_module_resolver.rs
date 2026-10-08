/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/ModuleResolver.java,
//   src/com/google/javascript/jscomp/deps/NodeModuleResolver.java,
//   src/com/google/javascript/jscomp/deps/WebpackModuleResolver.java.

use super::{
    module_loader::{ModuleLoader, ModuleResolverFactory, PathEscaper, SharedErrorHandler},
    module_resolver::{ModuleResolver, ModuleResolverBase},
    node_module_resolver::NodeModuleResolver,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;
pub struct Factory {
    lookup_map: IndexMap<String, String>,
}
impl Factory {
    // port: WebpackModuleResolver.Factory#Factory
    pub fn new(lookup_map: IndexMap<String, String>) -> Self {
        Self { lookup_map }
    }
}
impl ModuleResolverFactory for Factory {
    // port: WebpackModuleResolver.Factory#create
    fn create(
        &self,
        paths: IndexSet<String>,
        roots: Vec<String>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
    ) -> Arc<dyn ModuleResolver> {
        let mut normalized = IndexMap::new();
        for (id, path) in &self.lookup_map {
            let mut path = ModuleLoader::normalize(&escaper.escape(path), &roots);
            if ModuleLoader::is_ambiguous_identifier(&path) {
                path = format!("/{path}");
            }
            normalized.insert(id.clone(), path);
        }
        Arc::new(WebpackModuleResolver::new(
            paths, roots, normalized, handler, escaper,
        ))
    }
}
pub struct WebpackModuleResolver {
    node: NodeModuleResolver,
    modules_by_id: IndexMap<String, String>,
}
impl WebpackModuleResolver {
    // port: WebpackModuleResolver#WebpackModuleResolver
    pub fn new(
        paths: IndexSet<String>,
        roots: Vec<String>,
        modules_by_id: IndexMap<String, String>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
    ) -> Self {
        Self {
            node: NodeModuleResolver::new(paths, roots, None, handler, escaper),
            modules_by_id,
        }
    }
}
impl ModuleResolver for WebpackModuleResolver {
    // port: ModuleResolver#ModuleResolver (base fields)
    fn base(&self) -> &ModuleResolverBase {
        self.node.base()
    }
    // port: NodeModuleResolver#getPackageJsonMainEntries
    fn get_package_json_main_entries(&self) -> IndexMap<String, String> {
        self.node.get_package_json_main_entries()
    }
    // port: WebpackModuleResolver#resolveJsModule
    fn resolve_js_module(
        &self,
        script_address: &str,
        module_address: &str,
        sourcename: Option<&str>,
        lineno: i32,
        colno: i32,
    ) -> Option<String> {
        self.modules_by_id.get(module_address).cloned().or_else(|| {
            self.node
                .resolve_js_module(script_address, module_address, sourcename, lineno, colno)
        })
    }
    // port: WebpackModuleResolver#resolveJsModuleSilently
    fn resolve_js_module_silently(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        self.modules_by_id.get(module_address).cloned().or_else(|| {
            self.node
                .resolve_js_module_silently(script_address, module_address)
        })
    }
}
