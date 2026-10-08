/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ModuleIdentifier.java.

use crate::deps::module_names::ModuleNames;
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleIdentifier {
    name: String,
    closure_namespace: String,
    module_name: String,
}
impl ModuleIdentifier {
    // port: ModuleIdentifier#ModuleIdentifier
    pub fn new(name: String, closure_namespace: String, module_name: String) -> Self {
        Self {
            name,
            closure_namespace,
            module_name,
        }
    }
    // port: ModuleIdentifier#name
    pub fn name(&self) -> &str {
        &self.name
    }
    // port: ModuleIdentifier#closureNamespace
    pub fn closure_namespace(&self) -> &str {
        &self.closure_namespace
    }
    // port: ModuleIdentifier#moduleName
    pub fn module_name(&self) -> &str {
        &self.module_name
    }
    // port: ModuleIdentifier#getName
    pub fn get_name(&self) -> &str {
        self.name()
    }
    // port: ModuleIdentifier#getClosureNamespace
    pub fn get_closure_namespace(&self) -> &str {
        self.closure_namespace()
    }
    // port: ModuleIdentifier#getModuleName
    pub fn get_module_name(&self) -> &str {
        self.module_name()
    }
    // port: ModuleIdentifier#forClosure
    pub fn for_closure(name: &str) -> Self {
        let normalized_name = name.strip_prefix("goog:").unwrap_or(name);
        let mut namespace = normalized_name;
        let mut module_name = normalized_name;
        if let Some(split_point) = normalized_name.find(':') {
            module_name = &normalized_name[..split_point];
            // Java min(splitPoint + 1, length - 1) preserves a trailing colon.
            namespace = &normalized_name[(split_point + 1).min(normalized_name.len() - 1)..];
        }
        Self::new(normalized_name.into(), namespace.into(), module_name.into())
    }
    // port: ModuleIdentifier#forFile
    pub fn for_file(filepath: &str) -> Self {
        let normalized_name = ModuleNames::file_to_module_name(filepath);
        Self::new(filepath.into(), normalized_name.clone(), normalized_name)
    }
    // port: ModuleIdentifier#forFlagValue
    pub fn for_flag_value(flag_value: &str) -> Self {
        if flag_value.starts_with("goog:") {
            Self::for_closure(flag_value)
        } else {
            Self::for_file(flag_value)
        }
    }
}
impl std::fmt::Display for ModuleIdentifier {
    // port: ModuleIdentifier#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.closure_namespace() == self.module_name() {
            f.write_str(self.closure_namespace())
        } else {
            write!(f, "{}:{}", self.module_name(), self.closure_namespace())
        }
    }
}
