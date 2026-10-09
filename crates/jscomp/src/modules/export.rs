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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/modules/Export.java.

//! An `export`ed name in a module.
use crate::{deps::module_loader::ModulePath, modules::module_metadata_map::ModuleMetadata};
use closure_rhino::{check_not_null, check_state, js_string::JsString, node::NodeId};
use std::sync::Arc;

/// An `export`ed name in a module.
///
/// See <https://www.ecma-international.org/ecma-262/9.0/index.html#exportentry-record>.
///
/// Java record: structural equality. `ModuleMetadata` and `Node` compare by identity;
/// `ModulePath` has identity equality in Java and every export of one module shares the module's
/// single path object, so comparing the path text is equivalent.
#[derive(Clone)]
pub struct Export {
    /// `exportName` from the ES spec: the name used to export this binding.
    export_name: Option<JsString>,
    /// `moduleRequest` from the ES spec: the module specifier of an `export from`.
    module_request: Option<JsString>,
    /// `importName` from the ES spec: the name being re-exported from `moduleRequest`.
    import_name: Option<JsString>,
    /// `localName` from the ES spec: the local name of the exported binding.
    local_name: Option<JsString>,
    /// The module that contains this export.
    module_path: Option<ModulePath>,
    /// Node that this export originated from.
    export_node: Option<NodeId>,
    /// Node that this export binds (a NAME node for local exports).
    name_node: Option<NodeId>,
    /// The metadata of the module that has this export.
    module_metadata: Arc<ModuleMetadata>,
    /// The Closure namespace of a goog module export (or a faux non-ES module export).
    closure_namespace: Option<JsString>,
    /// Whether the export is ever mutated (in an inner scope).
    mutated: bool,
}

impl PartialEq for Export {
    fn eq(&self, other: &Self) -> bool {
        self.export_name == other.export_name
            && self.module_request == other.module_request
            && self.import_name == other.import_name
            && self.local_name == other.local_name
            && self.module_path.as_ref().map(ToString::to_string)
                == other.module_path.as_ref().map(ToString::to_string)
            && self.export_node == other.export_node
            && self.name_node == other.name_node
            && Arc::ptr_eq(&self.module_metadata, &other.module_metadata)
            && self.closure_namespace == other.closure_namespace
            && self.mutated == other.mutated
    }
}

impl Export {
    /// The {@link #exportName} of anonymous ES module default exports, e.g. `export default 0`.
    // port: Export#DEFAULT_EXPORT_NAME
    pub const DEFAULT_EXPORT_NAME: &'static str = "*default*";

    /// The {@link #exportName} of default exports (named or anonymous).
    // port: Export#DEFAULT
    pub const DEFAULT: &'static str = "default";

    /// The {@link #exportName} of a goog.module that assigns to `exports`, which is not a named
    /// export.
    // port: Export#NAMESPACE
    pub const NAMESPACE: &'static str = "*exports*";

    // port: Export#exportName
    pub fn export_name(&self) -> Option<&JsString> {
        self.export_name.as_ref()
    }

    // port: Export#moduleRequest
    pub fn module_request(&self) -> Option<&JsString> {
        self.module_request.as_ref()
    }

    // port: Export#importName
    pub fn import_name(&self) -> Option<&JsString> {
        self.import_name.as_ref()
    }

    // port: Export#localName
    pub fn local_name(&self) -> Option<&JsString> {
        self.local_name.as_ref()
    }

    // port: Export#modulePath
    pub fn module_path(&self) -> Option<&ModulePath> {
        self.module_path.as_ref()
    }

    // port: Export#exportNode
    pub fn export_node(&self) -> Option<NodeId> {
        self.export_node
    }

    // port: Export#nameNode
    pub fn name_node(&self) -> Option<NodeId> {
        self.name_node
    }

    // port: Export#moduleMetadata
    pub fn module_metadata(&self) -> &Arc<ModuleMetadata> {
        &self.module_metadata
    }

    // port: Export#closureNamespace
    pub fn closure_namespace(&self) -> Option<&JsString> {
        self.closure_namespace.as_ref()
    }

    // port: Export#mutated
    pub fn mutated(&self) -> bool {
        self.mutated
    }

    // port: Export#builder
    pub fn builder() -> Builder {
        Builder::default().mutated(false)
    }

    // port: Export#toBuilder
    pub fn to_builder(&self) -> Builder {
        Builder {
            export_name: self.export_name.clone(),
            module_request: self.module_request.clone(),
            import_name: self.import_name.clone(),
            local_name: self.local_name.clone(),
            module_path: self.module_path.clone(),
            export_node: self.export_node,
            name_node: self.name_node,
            module_metadata: Some(self.module_metadata.clone()),
            closure_namespace: self.closure_namespace.clone(),
            mutated: Some(self.mutated),
        }
    }

    /// Returns a copy of this export that has the {@link #mutated()} bit set.
    // port: Export#mutatedCopy
    pub fn mutated_copy(&self) -> Export {
        self.to_builder().mutated(true).auto_build()
    }
}

/// Builder for [`Export`] (Java `@AutoBuilder`).
#[derive(Default)]
pub struct Builder {
    export_name: Option<JsString>,
    module_request: Option<JsString>,
    import_name: Option<JsString>,
    local_name: Option<JsString>,
    module_path: Option<ModulePath>,
    export_node: Option<NodeId>,
    name_node: Option<NodeId>,
    module_metadata: Option<Arc<ModuleMetadata>>,
    closure_namespace: Option<JsString>,
    mutated: Option<bool>,
}

impl Builder {
    // port: Export.Builder#exportName
    pub fn export_name(mut self, value: Option<JsString>) -> Self {
        self.export_name = value;
        self
    }

    // port: Export.Builder#moduleRequest
    pub fn module_request(mut self, value: Option<JsString>) -> Self {
        self.module_request = value;
        self
    }

    // port: Export.Builder#importName
    pub fn import_name(mut self, value: Option<JsString>) -> Self {
        self.import_name = value;
        self
    }

    // port: Export.Builder#localName
    pub fn local_name(mut self, value: Option<JsString>) -> Self {
        self.local_name = value;
        self
    }

    // port: Export.Builder#modulePath
    pub fn module_path(mut self, value: Option<ModulePath>) -> Self {
        self.module_path = value;
        self
    }

    // port: Export.Builder#exportNode
    pub fn export_node(mut self, value: Option<NodeId>) -> Self {
        self.export_node = value;
        self
    }

    // port: Export.Builder#nameNode
    pub fn name_node(mut self, value: Option<NodeId>) -> Self {
        self.name_node = value;
        self
    }

    // port: Export.Builder#moduleMetadata
    pub fn module_metadata(mut self, value: Arc<ModuleMetadata>) -> Self {
        self.module_metadata = Some(value);
        self
    }

    // port: Export.Builder#closureNamespace
    pub fn closure_namespace(mut self, value: Option<JsString>) -> Self {
        self.closure_namespace = value;
        self
    }

    // port: Export.Builder#mutated
    pub fn mutated(mut self, value: bool) -> Self {
        self.mutated = Some(value);
        self
    }

    // port: Export.Builder#autoBuild
    fn auto_build(self) -> Export {
        // port: Export#Export (the record's requireNonNull check)
        Export {
            export_name: self.export_name,
            module_request: self.module_request,
            import_name: self.import_name,
            local_name: self.local_name,
            module_path: self.module_path,
            export_node: self.export_node,
            name_node: self.name_node,
            module_metadata: self.module_metadata.expect("moduleMetadata"),
            closure_namespace: self.closure_namespace,
            mutated: self.mutated.expect("Missing required properties: mutated"),
        }
    }

    // port: Export.Builder#build
    pub fn build(self) -> Export {
        let e = self.auto_build();
        if e.module_metadata().is_es6_module() {
            Self::validate_es_module(&e);
        } else if e.module_metadata().is_goog_module() {
            Self::validate_goog_module(&e);
        } else {
            Self::validate_other_module(&e);
        }
        e
    }

    /// Export from an ES module.
    // port: Export.Builder#validateEsModule
    fn validate_es_module(e: &Export) {
        check_state!(e.closure_namespace().is_none());
        check_state!(
            e.import_name().is_none_or(|n| *n != "*")
                || (e.module_request().is_some()
                    && e.export_name().is_none()
                    && e.local_name().is_none()),
            "Star exports should not have exported / local names."
        );
        check_state!(
            e.local_name().is_none() || e.module_request().is_none(),
            "Local exports should not have module requests."
        );
        check_state!(
            e.module_request().is_none() || e.local_name().is_none(),
            "Reexports should not have local names."
        );
        check_state!(
            e.module_request().is_none() || e.import_name().is_some(),
            "Reexports should have import names."
        );
        check_state!(
            e.import_name().is_none() || e.module_request().is_some(),
            "Exports with an import name should be a reexport."
        );
    }

    /// Some export from a goog module.
    // port: Export.Builder#validateGoogModule
    fn validate_goog_module(e: &Export) {
        check_state!(
            e.closure_namespace().is_some(),
            "Exports should be associated with a namespace"
        );
        check_state!(e.export_name().is_some(), "Exports should be named");
        check_state!(e.export_node().is_some(), "Exports should have a node");
        check_state!(
            e.local_name().is_none(),
            "goog.module Exports don't set a localName"
        );
        check_state!(
            e.module_request().is_none(),
            "goog modules cannot export from other modules"
        );
    }

    /// Some faux export from a non-ES module.
    // port: Export.Builder#validateOtherModule
    fn validate_other_module(e: &Export) {
        check_not_null!(e.export_name());
        // Fields ignored for these fake exports. Should not set these.
        check_state!(e.export_node().is_none());
        check_state!(e.local_name().is_none());
        check_state!(e.module_request().is_none());
        check_state!(e.import_name().is_none());
        check_state!(e.name_node().is_none());
    }
}

impl std::fmt::Debug for Export {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Export")
            .field("export_name", &self.export_name)
            .field("module_request", &self.module_request)
            .field("import_name", &self.import_name)
            .field("local_name", &self.local_name)
            .field(
                "module_path",
                &self.module_path.as_ref().map(ToString::to_string),
            )
            .field("export_node", &self.export_node)
            .field("name_node", &self.name_node)
            .field("module_metadata", &self.module_metadata)
            .field("closure_namespace", &self.closure_namespace)
            .field("mutated", &self.mutated)
            .finish()
    }
}
