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
//   src/com/google/javascript/jscomp/modules/Binding.java.

//! A binding in a module: a name and the module or export it originates from.
use crate::modules::{export::Export, module_metadata_map::ModuleMetadata};
use closure_rhino::{
    check_argument, check_not_null, check_state, js_string::JsString, node::NodeId,
};
use std::{fmt, sync::Arc};

/// Represents a variable binding: either an exported name or an entire module namespace object.
///
/// See <https://www.ecma-international.org/ecma-262/9.0/index.html#resolvedbinding-record>.
///
/// Java record: structural equality (`ModuleMetadata` and `Node` compare by identity).
#[derive(Clone, Debug)]
pub struct Binding {
    /// The metadata of the module this binding came from.
    metadata: Arc<ModuleMetadata>,
    /// The AST node to use for source information; `null` for bindings of missing modules.
    source_node: Option<NodeId>,
    /// The export this binding originated from, or `null` for a module namespace.
    originating_export: Option<Export>,
    /// Whether this binding is the entire module namespace object.
    is_module_namespace: bool,
    /// The Closure namespace of the bound module, if it is a Closure module.
    closure_namespace: Option<JsString>,
    /// How this binding was created.
    created_by: CreatedBy,
}

impl PartialEq for Binding {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.metadata, &other.metadata)
            && self.source_node == other.source_node
            && self.originating_export == other.originating_export
            && self.is_module_namespace == other.is_module_namespace
            && self.closure_namespace == other.closure_namespace
            && self.created_by == other.created_by
    }
}

/// Different ways that Bindings can be created.
// port: Binding.CreatedBy
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CreatedBy {
    /// A binding created by an export in an ES module.
    EXPORT,
    /// A binding created by an ES import.
    IMPORT,
    /// A binding created by a goog.require statement.
    GOOG_REQUIRE,
    /// A binding created by a goog.requireType statement.
    GOOG_REQUIRE_TYPE,
    /// A binding created by a goog.forwardDeclare statement.
    GOOG_FORWARD_DECLARE,
}

impl CreatedBy {
    /// Whether this is some goog.* dependency import
    // port: Binding.CreatedBy#isClosureImport
    pub fn is_closure_import(self) -> bool {
        self == CreatedBy::GOOG_REQUIRE
            || self == CreatedBy::GOOG_REQUIRE_TYPE
            || self == CreatedBy::GOOG_FORWARD_DECLARE
    }
}

impl fmt::Display for CreatedBy {
    // port: Enum#toString (Binding.CreatedBy)
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Binding {
    // port: Binding#Binding
    fn new(
        metadata: Arc<ModuleMetadata>,
        source_node: Option<NodeId>,
        originating_export: Option<Export>,
        is_module_namespace: bool,
        closure_namespace: Option<JsString>,
        created_by: CreatedBy,
    ) -> Self {
        Self {
            metadata,
            source_node,
            originating_export,
            is_module_namespace,
            closure_namespace,
            created_by,
        }
    }

    // port: Binding#metadata
    pub fn metadata(&self) -> &Arc<ModuleMetadata> {
        &self.metadata
    }

    // port: Binding#sourceNode
    pub fn source_node(&self) -> Option<NodeId> {
        self.source_node
    }

    // port: Binding#originatingExport
    pub fn originating_export(&self) -> Option<&Export> {
        self.originating_export.as_ref()
    }

    // port: Binding#isModuleNamespace
    pub fn is_module_namespace(&self) -> bool {
        self.is_module_namespace
    }

    // port: Binding#closureNamespace
    pub fn closure_namespace(&self) -> Option<&JsString> {
        self.closure_namespace.as_ref()
    }

    // port: Binding#createdBy
    pub fn created_by(&self) -> CreatedBy {
        self.created_by
    }

    /// Binding for an exported value that is not a module namespace object.
    // port: Binding#from(Export,Node)
    pub fn from_export(bound_export: Export, source_node: Option<NodeId>) -> Self {
        Self::new(
            bound_export.module_metadata().clone(),
            source_node,
            Some(bound_export.clone()),
            /* isModuleNamespace= */ false,
            /* closureNamespace= */ bound_export.closure_namespace().cloned(),
            CreatedBy::EXPORT,
        )
    }

    /// Binding for an entire module namespace created by const x = goog.require(Type)('...')
    // port: Binding#from(ModuleMetadata,Node,String,CreatedBy)
    pub fn from_closure_namespace(
        metadata: Arc<ModuleMetadata>,
        source_node: NodeId,
        closure_namespace: JsString,
        created_by: CreatedBy,
    ) -> Self {
        check_argument!(
            created_by.is_closure_import(),
            "Expected goog.require(Type) or goog.forwardDeclare, got %s",
            created_by
        );
        Self::new(
            metadata,
            Some(source_node),
            /* originatingExport= */ None,
            /* isModuleNamespace= */ true,
            Some(closure_namespace),
            created_by,
        )
    }

    /// Binding for an entire module namespace created by an `import *`.
    // port: Binding#from(ModuleMetadata,String,Node)
    pub fn from_es_import_star(
        metadata_of_bound_module: Arc<ModuleMetadata>,
        closure_namespace: Option<JsString>,
        source_node: NodeId,
    ) -> Self {
        Self::new(
            metadata_of_bound_module,
            Some(source_node),
            /* originatingExport= */ None,
            /* isModuleNamespace= */ true,
            closure_namespace,
            CreatedBy::IMPORT,
        )
    }

    /// Copies the binding with a new source node and CreatedBy binding.
    // port: Binding#copy
    pub fn copy(&self, source_node: Option<NodeId>, created_by: CreatedBy) -> Self {
        let source_node = check_not_null!(source_node);
        Self::new(
            self.metadata().clone(),
            Some(source_node),
            self.originating_export().cloned(),
            self.is_module_namespace(),
            self.closure_namespace().cloned(),
            created_by,
        )
    }

    /// Returns the name the export is bound to, assuming it is not a module namespace object.
    // port: Binding#boundName
    pub fn bound_name(&self) -> Option<&JsString> {
        check_state!(!self.is_module_namespace());
        self.originating_export().unwrap().local_name()
    }

    /// Returns whether or not this Binding is mutated in an inner scope.
    // port: Binding#isMutated
    pub fn is_mutated(&self) -> bool {
        // Module namespaces can never be mutated. They are always imported, and import bound names
        // are const.
        !self.is_module_namespace() && self.originating_export().unwrap().mutated()
    }

    /// Returns whether this Binding originated from an ES import.
    // port: Binding#isCreatedByEsImport
    pub fn is_created_by_es_import(&self) -> bool {
        self.created_by() == CreatedBy::IMPORT
    }

    /// Returns whether this Binding originated from an ES export.
    // port: Binding#isCreatedByEsExport
    pub fn is_created_by_es_export(&self) -> bool {
        self.created_by() == CreatedBy::EXPORT
    }

    /// Returns whether this Binding originated from an ES import or goog.require
    // port: Binding#isSomeImport
    pub fn is_some_import(&self) -> bool {
        self.created_by() != CreatedBy::EXPORT
    }
}
