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
//   src/com/google/javascript/jscomp/modules/Module.java.

//! Information for modules, particularly ES modules, that is useful for rewriting.
use crate::{
    deps::module_loader::ModulePath,
    modules::{binding::Binding, export::Export, module_metadata_map::ModuleMetadata},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use std::sync::Arc;

/// Information for modules, particularly ES modules, that is useful for rewriting. The primary
/// pieces of information are what variables are exported (transitive or local), and what names
/// are imported.
///
/// - `path`: path of this module. Null if this module is a nested `goog.loadModule`.
/// - `namespace`: map of exported identifiers to originating binding (keys are the exported
///   names).
/// - `bound_names`: map of local identifiers to originating binding (imports and exported names
///   that originate in this module).
/// - `local_name_to_local_export`: map of local identifier name to local export definition.
/// - `closure_namespace`: the specific Closure namespace this module represents, if any.
#[derive(Clone)]
pub struct Module {
    metadata: Arc<ModuleMetadata>,
    path: Option<ModulePath>,
    namespace: IndexMap<JsString, Binding>,
    bound_names: IndexMap<JsString, Binding>,
    local_name_to_local_export: IndexMap<JsString, Export>,
    closure_namespace: Option<JsString>,
}

impl Module {
    // port: Module#metadata
    pub fn metadata(&self) -> &Arc<ModuleMetadata> {
        &self.metadata
    }

    // port: Module#path
    pub fn path(&self) -> Option<&ModulePath> {
        self.path.as_ref()
    }

    // port: Module#namespace
    pub fn namespace(&self) -> &IndexMap<JsString, Binding> {
        &self.namespace
    }

    // port: Module#boundNames
    pub fn bound_names(&self) -> &IndexMap<JsString, Binding> {
        &self.bound_names
    }

    // port: Module#localNameToLocalExport
    pub fn local_name_to_local_export(&self) -> &IndexMap<JsString, Export> {
        &self.local_name_to_local_export
    }

    // port: Module#closureNamespace
    pub fn closure_namespace(&self) -> Option<&JsString> {
        self.closure_namespace.as_ref()
    }

    /// Creates a new builder.
    // port: Module#builder
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Returns this module in builder form.
    // port: Module#toBuilder
    pub fn to_builder(&self) -> Builder {
        Builder {
            metadata: Some(self.metadata.clone()),
            path: self.path.clone(),
            namespace: Some(self.namespace.clone()),
            bound_names: Some(self.bound_names.clone()),
            local_name_to_local_export: Some(self.local_name_to_local_export.clone()),
            closure_namespace: self.closure_namespace.clone(),
        }
    }
}

/// Builder for [`Module`] (Java `@AutoBuilder`).
#[derive(Default)]
pub struct Builder {
    metadata: Option<Arc<ModuleMetadata>>,
    path: Option<ModulePath>,
    namespace: Option<IndexMap<JsString, Binding>>,
    bound_names: Option<IndexMap<JsString, Binding>>,
    local_name_to_local_export: Option<IndexMap<JsString, Export>>,
    closure_namespace: Option<JsString>,
}

impl Builder {
    // port: Module.Builder#metadata
    pub fn metadata(mut self, value: Arc<ModuleMetadata>) -> Self {
        self.metadata = Some(value);
        self
    }

    // port: Module.Builder#path
    pub fn path(mut self, value: Option<ModulePath>) -> Self {
        self.path = value;
        self
    }

    // port: Module.Builder#namespace
    pub fn namespace(mut self, value: IndexMap<JsString, Binding>) -> Self {
        self.namespace = Some(value);
        self
    }

    // port: Module.Builder#boundNames
    pub fn bound_names(mut self, value: IndexMap<JsString, Binding>) -> Self {
        self.bound_names = Some(value);
        self
    }

    // port: Module.Builder#localNameToLocalExport
    pub fn local_name_to_local_export(mut self, value: IndexMap<JsString, Export>) -> Self {
        self.local_name_to_local_export = Some(value);
        self
    }

    // port: Module.Builder#closureNamespace
    pub fn closure_namespace(mut self, value: Option<JsString>) -> Self {
        self.closure_namespace = value;
        self
    }

    // port: Module.Builder#build
    pub fn build(self) -> Module {
        // port: Module#Module (the record's requireNonNull checks)
        Module {
            metadata: self.metadata.expect("metadata"),
            path: self.path,
            namespace: self.namespace.expect("namespace"),
            bound_names: self.bound_names.expect("boundNames"),
            local_name_to_local_export: self
                .local_name_to_local_export
                .expect("localNameToLocalExport"),
            closure_namespace: self.closure_namespace,
        }
    }
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module")
            .field("metadata", &self.metadata)
            .field("path", &self.path.as_ref().map(ToString::to_string))
            .field("namespace", &self.namespace)
            .field("bound_names", &self.bound_names)
            .field(
                "local_name_to_local_export",
                &self.local_name_to_local_export,
            )
            .field("closure_namespace", &self.closure_namespace)
            .finish()
    }
}
