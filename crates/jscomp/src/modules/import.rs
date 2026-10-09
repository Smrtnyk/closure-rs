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
//   src/com/google/javascript/jscomp/modules/Import.java.

//! An `import`ed name in a module.
use crate::deps::module_loader::ModulePath;
use closure_rhino::{js_string::JsString, node::NodeId};

/// An `import`ed name in a module.
///
/// See <https://www.ecma-international.org/ecma-262/9.0/index.html#importentry-record>.
///
/// Java record: structural equality (`ModulePath` has identity equality in Java; every import of
/// one module shares the module's single path object, so comparing the path text is equivalent).
#[derive(Clone)]
pub struct Import {
    /// `moduleRequest` from the ES spec: the module specifier the import was made with.
    module_request: JsString,
    /// `importName` from the ES spec: the name of the export being imported, `*` for a namespace.
    import_name: JsString,
    /// `localName` from the ES spec: the name of the import in the importing module.
    local_name: JsString,
    /// The module path of the module making the import (`null` for Closure requires).
    module_path: Option<ModulePath>,
    /// Node that this import originated from (the IMPORT or the name declaration).
    import_node: NodeId,
    /// Node that this import binds (a NAME node).
    name_node: NodeId,
}

impl PartialEq for Import {
    fn eq(&self, other: &Self) -> bool {
        self.module_request == other.module_request
            && self.import_name == other.import_name
            && self.local_name == other.local_name
            && self.module_path.as_ref().map(ToString::to_string)
                == other.module_path.as_ref().map(ToString::to_string)
            && self.import_node == other.import_node
            && self.name_node == other.name_node
    }
}

impl Import {
    // port: Import#moduleRequest
    pub fn module_request(&self) -> &JsString {
        &self.module_request
    }

    // port: Import#importName
    pub fn import_name(&self) -> &JsString {
        &self.import_name
    }

    // port: Import#localName
    pub fn local_name(&self) -> &JsString {
        &self.local_name
    }

    // port: Import#modulePath
    pub fn module_path(&self) -> Option<&ModulePath> {
        self.module_path.as_ref()
    }

    // port: Import#importNode
    pub fn import_node(&self) -> NodeId {
        self.import_node
    }

    // port: Import#nameNode
    pub fn name_node(&self) -> NodeId {
        self.name_node
    }

    // port: Import#builder
    pub fn builder() -> Builder {
        Builder::default()
    }
}

/// Builder for [`Import`] (Java `@AutoBuilder`).
#[derive(Default)]
pub struct Builder {
    module_request: Option<JsString>,
    import_name: Option<JsString>,
    local_name: Option<JsString>,
    module_path: Option<ModulePath>,
    import_node: Option<NodeId>,
    name_node: Option<NodeId>,
}

impl Builder {
    // port: Import.Builder#moduleRequest
    pub fn module_request(mut self, value: impl Into<JsString>) -> Self {
        self.module_request = Some(value.into());
        self
    }

    // port: Import.Builder#importName
    pub fn import_name(mut self, value: impl Into<JsString>) -> Self {
        self.import_name = Some(value.into());
        self
    }

    // port: Import.Builder#localName
    pub fn local_name(mut self, value: impl Into<JsString>) -> Self {
        self.local_name = Some(value.into());
        self
    }

    // port: Import.Builder#modulePath
    pub fn module_path(mut self, value: ModulePath) -> Self {
        self.module_path = Some(value);
        self
    }

    // port: Import.Builder#importNode
    pub fn import_node(mut self, value: NodeId) -> Self {
        self.import_node = Some(value);
        self
    }

    // port: Import.Builder#nameNode
    pub fn name_node(mut self, value: NodeId) -> Self {
        self.name_node = Some(value);
        self
    }

    // port: Import.Builder#build
    pub fn build(self) -> Import {
        // port: Import#Import (the record's requireNonNull checks)
        Import {
            module_request: self.module_request.expect("moduleRequest"),
            import_name: self.import_name.expect("importName"),
            local_name: self.local_name.expect("localName"),
            module_path: self.module_path,
            import_node: self.import_node.expect("importNode"),
            name_node: self.name_node.expect("nameNode"),
        }
    }
}

impl std::fmt::Debug for Import {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Import")
            .field("module_request", &self.module_request)
            .field("import_name", &self.import_name)
            .field("local_name", &self.local_name)
            .field(
                "module_path",
                &self.module_path.as_ref().map(ToString::to_string),
            )
            .field("import_node", &self.import_node)
            .field("name_node", &self.name_node)
            .finish()
    }
}
