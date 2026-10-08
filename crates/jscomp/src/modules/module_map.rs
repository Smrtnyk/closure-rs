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
//   src/com/google/javascript/jscomp/modules/ModuleMap.java.

//! A map containing information about all modules in the compilation.
use crate::{deps::module_loader::ModulePath, modules::module::Module};
use closure_rhino::js_string::JsString;
use indexmap::IndexMap;
use std::sync::Arc;

/// A map containing information about all modules in the compilation.
///
/// This is currently used for ES modules and other types of module are not processed in detail.
#[derive(Debug, Default)]
pub struct ModuleMap {
    resolved_modules: IndexMap<String, Arc<Module>>,
    resolved_closure_modules: IndexMap<JsString, Arc<Module>>,
}

impl ModuleMap {
    // port: ModuleMap#ModuleMap
    pub fn new(
        resolved_modules: IndexMap<String, Arc<Module>>,
        resolved_closure_modules: IndexMap<JsString, Arc<Module>>,
    ) -> Self {
        Self {
            resolved_modules,
            resolved_closure_modules,
        }
    }

    // port: ModuleMap#getModule(String)
    pub fn get_module(&self, module_name: &str) -> Option<&Arc<Module>> {
        self.resolved_modules.get(module_name)
    }

    // port: ModuleMap#getModule(ModulePath)
    pub fn get_module_by_path(&self, path: &ModulePath) -> Option<&Arc<Module>> {
        self.get_module(&path.to_module_name())
    }

    // port: ModuleMap#getModulesByPath
    pub fn get_modules_by_path(&self) -> &IndexMap<String, Arc<Module>> {
        &self.resolved_modules
    }

    // port: ModuleMap#getModulesByClosureNamespace
    pub fn get_modules_by_closure_namespace(&self) -> &IndexMap<JsString, Arc<Module>> {
        &self.resolved_closure_modules
    }

    // port: ModuleMap#getClosureModule
    pub fn get_closure_module(&self, namespace: &JsString) -> Option<&Arc<Module>> {
        self.resolved_closure_modules.get(namespace)
    }

    // port: ModuleMap#emptyForTesting
    pub fn empty_for_testing() -> Self {
        Self::new(IndexMap::new(), IndexMap::new())
    }
}
