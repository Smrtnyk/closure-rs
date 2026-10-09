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
//   src/com/google/javascript/jscomp/deps/ModuleResolver.java,
//   src/com/google/javascript/jscomp/deps/NodeModuleResolver.java.

use super::{
    module_loader::{
        LOAD_WARNING, ModuleLoader, ModuleResolverFactory, PathEscaper, SharedErrorHandler,
        best_match_path_ordering,
    },
    module_resolver::{ModuleResolver, ModuleResolverBase},
};
use crate::check_level::CheckLevel;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::java_lang::string::split;
use std::sync::Arc;
pub struct NodeModuleResolver {
    pub(crate) base: ModuleResolverBase,
    package_json_main_entries: IndexMap<String, String>,
    node_modules_folders: Vec<String>,
}
impl NodeModuleResolver {
    const FILE_EXTENSIONS_TO_SEARCH: [&'static str; 5] = ["", ".js", ".json", ".i.js", ".js.i.js"];
    const FILES_TO_SEARCH: [&'static str; 3] = ["/package.json", "/index.js", "/index.json"];
    // port: NodeModuleResolver#buildNodeModulesFoldersRegistry
    fn build_node_modules_folders_registry(
        paths: &IndexSet<String>,
        roots: &[String],
    ) -> Vec<String> {
        let mut registry = IndexSet::<_>::default();
        for module_path in paths {
            let mut path = module_path.as_str();
            for root in roots {
                if let Some(tail) = path.strip_prefix(root) {
                    path = tail;
                    break;
                }
            }
            let dirs = split(path, "/node_modules/");
            let mut parent_path = String::new();
            for dir in dirs.iter().take(dirs.len().saturating_sub(1)) {
                parent_path.push_str(dir);
                parent_path.push('/');
                registry.insert(parent_path.clone());
                parent_path.push_str("node_modules/");
            }
        }
        let mut registry: Vec<_> = registry.into_iter().collect();
        registry.sort_by(|a, b| best_match_path_ordering(a, b));
        registry
    }
    // port: NodeModuleResolver#NodeModuleResolver
    pub fn new(
        paths: IndexSet<String>,
        roots: Vec<String>,
        entries: Option<IndexMap<String, String>>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
    ) -> Self {
        let node_modules_folders = Self::build_node_modules_folders_registry(&paths, &roots);
        Self {
            base: ModuleResolverBase::new(paths, roots, handler, escaper),
            node_modules_folders,
            package_json_main_entries: entries
                .map(Self::build_package_json_main_entries)
                .unwrap_or_default(),
        }
    }
    // port: NodeModuleResolver#buildPackageJsonMainEntries
    fn build_package_json_main_entries(
        entries: IndexMap<String, String>,
    ) -> IndexMap<String, String> {
        let mut builder = IndexMap::<_, _>::default();
        for (k, v) in entries {
            let key = if ModuleLoader::is_ambiguous_identifier(&k) {
                format!("/{k}")
            } else {
                k
            };
            if let Some(old) = builder.insert(key.clone(), v.clone()) {
                panic!("Multiple entries with same key: {key}={v} and {key}={old}");
            }
        }
        builder
    }
    // port: NodeModuleResolver#internalResolveJsModule
    fn internal_resolve_js_module(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        if ModuleLoader::is_absolute_identifier(module_address)
            || ModuleLoader::is_relative_identifier(module_address)
        {
            self.resolve_js_module_node_file_or_directory(script_address, module_address)
        } else {
            self.resolve_js_module_from_registry(script_address, module_address)
        }
    }
    // port: NodeModuleResolver#resolveJsModuleFile
    pub fn resolve_js_module_file(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        for extension in Self::FILE_EXTENSIONS_TO_SEARCH {
            let mut candidate = format!("{module_address}{extension}");
            let canonical = self.base.canonicalize_path(script_address, &candidate);
            if let Some(mapping) = self.package_json_main_entries.get(&canonical) {
                candidate = mapping.clone();
                if candidate == ModuleLoader::JSC_BROWSER_SKIPLISTED_MARKER {
                    return None;
                }
            }
            if let Some(address) = self.base.locate(script_address, &candidate) {
                return Some(address);
            }
        }
        None
    }
    // port: NodeModuleResolver#resolveJsModuleNodeFileOrDirectory
    fn resolve_js_module_node_file_or_directory(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        self.resolve_js_module_file(script_address, module_address)
            .or_else(|| self.resolve_js_module_node_directory(script_address, module_address))
    }
    // port: NodeModuleResolver#resolveJsModuleNodeDirectory
    fn resolve_js_module_node_directory(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        let address = module_address.strip_suffix('/').unwrap_or(module_address);
        for file in Self::FILES_TO_SEARCH {
            if let Some(load_address) = self
                .base
                .locate(script_address, &format!("{address}{file}"))
            {
                if file == "/package.json" {
                    if let Some(entry) = self.package_json_main_entries.get(&load_address) {
                        return self.resolve_js_module_file(script_address, entry);
                    }
                } else {
                    return Some(load_address);
                }
            }
        }
        None
    }
    // port: NodeModuleResolver#resolveJsModuleFromRegistry
    fn resolve_js_module_from_registry(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        let script = if ModuleLoader::is_ambiguous_identifier(script_address) {
            format!("/{script_address}")
        } else {
            script_address.into()
        };
        for folder in &self.node_modules_folders {
            if !script.starts_with(folder) {
                continue;
            }
            let full = format!("{folder}node_modules/{module_address}");
            if let Some(load_address) = self
                .resolve_js_module_file(script_address, &full)
                .or_else(|| self.resolve_js_module_node_directory(script_address, &full))
            {
                return Some(load_address);
            }
        }
        None
    }
}
impl ModuleResolver for NodeModuleResolver {
    // port: ModuleResolver#ModuleResolver (base fields)
    fn base(&self) -> &ModuleResolverBase {
        &self.base
    }
    // port: NodeModuleResolver#getPackageJsonMainEntries
    fn get_package_json_main_entries(&self) -> IndexMap<String, String> {
        self.package_json_main_entries.clone()
    }
    // port: NodeModuleResolver#resolveJsModule
    fn resolve_js_module(
        &self,
        script_address: &str,
        module_address: &str,
        sourcename: Option<&str>,
        lineno: i32,
        colno: i32,
    ) -> Option<String> {
        let address = self.internal_resolve_js_module(script_address, module_address);
        if address.is_none() {
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
        address
    }
    // port: NodeModuleResolver#resolveJsModuleSilently
    fn resolve_js_module_silently(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        self.internal_resolve_js_module(script_address, module_address)
    }
}
#[derive(Default)]
pub struct Factory {
    package_json_main_entries: Option<IndexMap<String, String>>,
}
impl Factory {
    // port: NodeModuleResolver.Factory#Factory()
    pub fn new() -> Self {
        Self::with_package_json_main_entries(None)
    }
    // port: NodeModuleResolver.Factory#Factory(Map)
    pub fn with_package_json_main_entries(entries: Option<IndexMap<String, String>>) -> Self {
        Self {
            package_json_main_entries: entries,
        }
    }
}
impl ModuleResolverFactory for Factory {
    // port: NodeModuleResolver.Factory#create
    fn create(
        &self,
        paths: IndexSet<String>,
        roots: Vec<String>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
    ) -> Arc<dyn ModuleResolver> {
        Arc::new(NodeModuleResolver::new(
            paths,
            roots,
            self.package_json_main_entries.clone(),
            handler,
            escaper,
        ))
    }
}
