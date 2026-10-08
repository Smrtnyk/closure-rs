/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ErrorHandler.java,
//   src/com/google/javascript/jscomp/JSError.java,
//   src/com/google/javascript/jscomp/deps/ModuleResolver.java.

use super::{
    module_loader::{ModuleLoader, PathEscaper, SharedErrorHandler},
    module_names::ModuleNames,
};
use crate::{check_level::CheckLevel, js_error::JSError};
use indexmap::{IndexMap, IndexSet};
use std::sync::Mutex;
pub struct ModuleResolverBase {
    pub module_paths: IndexSet<String>,
    pub module_root_paths: Vec<String>,
    pub error_handler: Mutex<SharedErrorHandler>,
    pub path_escaper: PathEscaper,
}
impl ModuleResolverBase {
    // port: ModuleResolver#ModuleResolver
    pub fn new(
        module_paths: IndexSet<String>,
        module_root_paths: Vec<String>,
        error_handler: SharedErrorHandler,
        path_escaper: PathEscaper,
    ) -> Self {
        Self {
            module_paths,
            module_root_paths,
            error_handler: Mutex::new(error_handler),
            path_escaper,
        }
    }
    // port: ModuleResolver#getPackageJsonMainEntries
    pub fn get_package_json_main_entries(&self) -> IndexMap<String, String> {
        IndexMap::new()
    }
    // port: ModuleResolver#resolveModuleAsPath
    pub fn resolve_module_as_path(&self, script_address: &str, module_address: &str) -> String {
        let module_address = if module_address.ends_with(".js") {
            module_address.to_owned()
        } else {
            format!("{module_address}.js")
        };
        let mut path = self.path_escaper.escape(&module_address);
        if ModuleLoader::is_relative_identifier(&module_address) {
            let last_index = script_address.rfind('/').map_or(0, |i| i + 1);
            path =
                ModuleNames::canonicalize_path(&format!("{}{path}", &script_address[..last_index]));
        }
        ModuleLoader::normalize(&path, &self.module_root_paths)
    }
    // port: ModuleResolver#locate
    pub fn locate(&self, script_address: &str, name: &str) -> Option<String> {
        let canonicalized_path = self.canonicalize_path(script_address, name);
        let normalized_path = if ModuleLoader::is_ambiguous_identifier(&canonicalized_path) {
            format!("/{canonicalized_path}")
        } else {
            canonicalized_path.clone()
        };
        if self.module_paths.contains(&normalized_path) {
            return Some(canonicalized_path);
        }
        for root_path in &self.module_root_paths {
            if self
                .module_paths
                .contains(&format!("{root_path}{normalized_path}"))
            {
                return Some(canonicalized_path);
            }
        }
        None
    }
    // port: ModuleResolver#canonicalizePath
    pub fn canonicalize_path(&self, script_address: &str, module_address: &str) -> String {
        let mut path = self.path_escaper.escape(module_address);
        if ModuleLoader::is_relative_identifier(module_address) {
            let last_index = script_address.rfind('/').map_or(0, |i| i + 1);
            path =
                ModuleNames::canonicalize_path(&format!("{}{path}", &script_address[..last_index]));
        }
        path
    }
    // port: ModuleResolver#setErrorHandler
    pub fn set_error_handler(&self, error_handler: SharedErrorHandler) {
        *self.error_handler.lock().unwrap() = error_handler;
    }
    // port: ErrorHandler#report (dispatch through Java's shared handler)
    pub fn report(&self, level: CheckLevel, error: JSError) {
        let handler = self.error_handler.lock().unwrap().clone();
        handler.lock().unwrap().report(level, error);
    }
}
pub trait ModuleResolver: Send + Sync {
    // port: ModuleResolver#ModuleResolver (base fields)
    fn base(&self) -> &ModuleResolverBase;
    // port: ModuleResolver#resolveJsModule
    fn resolve_js_module(
        &self,
        script_address: &str,
        module_address: &str,
        sourcename: Option<&str>,
        lineno: i32,
        colno: i32,
    ) -> Option<String>;
    // port: ModuleResolver#resolveJsModuleSilently
    fn resolve_js_module_silently(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String>;
    // port: ModuleResolver#resolveModuleAsPath
    fn resolve_module_as_path(&self, script_address: &str, module_address: &str) -> String {
        self.base()
            .resolve_module_as_path(script_address, module_address)
    }
    // port: ModuleResolver#getPackageJsonMainEntries
    fn get_package_json_main_entries(&self) -> IndexMap<String, String> {
        self.base().get_package_json_main_entries()
    }
    // port: ModuleResolver#setErrorHandler
    fn set_error_handler(&self, error_handler: SharedErrorHandler) {
        self.base().set_error_handler(error_handler);
    }
}

// port: JSError#make(String,int,int,DiagnosticType,String...) (nullable source name)
// Missing diagnostics API: make_with_source_location/set_source_location only accept &str;
// Java permits a null source name. Keep the workaround here in deps, using the public record field.
pub(crate) fn make_js_error(
    source_name: Option<&str>,
    lineno: i32,
    charno: i32,
    type_: &'static crate::diagnostic_type::DiagnosticType,
    arguments: &[&str],
) -> JSError {
    let mut error = JSError::make_with_source_location(
        source_name.unwrap_or(""),
        lineno,
        charno,
        type_,
        arguments,
    );
    if source_name.is_none() {
        error.source_name = None;
    }
    error
}
