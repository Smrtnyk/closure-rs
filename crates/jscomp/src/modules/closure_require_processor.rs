/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/modules/ClosureRequireProcessor.java.

//! Handles creating an [`Import`] from goog.require(Type) or goog.forwardDeclare.
use crate::{
    modules::{binding::CreatedBy, export::Export, import::Import},
    node_util::NodeUtil,
};
use closure_rhino::{check_argument, js_string::JsString, node::Ast, node::NodeId};

/// Handles creating an [`Import`] from goog.require(Type) or goog.forwardDeclare.
///
/// This logic can be used by both goog.modules and ES modules
pub struct ClosureRequireProcessor {
    name_declaration: NodeId,
    require_kind: CreatedBy,
}

/// Represents a goog.require(Type) or goog.forwardDeclare
///
/// - `local_name`: the name local to the module with the require; e.g. `b` in
///   `const b = goog.require('a');`
/// - `import_record`: an [`Import`] containing all metadata about this require
/// - `created_by`: whether this is a goog.require, goog.requireType, or goog.forwardDeclare
// port: ClosureRequireProcessor.Require
#[derive(Clone, Debug, PartialEq)]
pub struct Require {
    local_name: JsString,
    import_record: Import,
    created_by: CreatedBy,
}

impl Require {
    // port: ClosureRequireProcessor.Require#Require
    fn new(local_name: JsString, import_record: Import, created_by: CreatedBy) -> Self {
        check_argument!(created_by.is_closure_import());
        Self {
            local_name,
            import_record,
            created_by,
        }
    }

    // port: ClosureRequireProcessor.Require#create
    fn create(local_name: JsString, import_record: Import, created_by: CreatedBy) -> Self {
        Self::new(local_name, import_record, created_by)
    }

    // port: ClosureRequireProcessor.Require#localName
    pub fn local_name(&self) -> &JsString {
        &self.local_name
    }

    // port: ClosureRequireProcessor.Require#importRecord
    pub fn import_record(&self) -> &Import {
        &self.import_record
    }

    // port: ClosureRequireProcessor.Require#createdBy
    pub fn created_by(&self) -> CreatedBy {
        self.created_by
    }
}

// port: ClosureRequireProcessor#GOOG_DEPENDENCY_CALLS
const GOOG_DEPENDENCY_CALLS: [(&str, CreatedBy); 3] = [
    ("require", CreatedBy::GOOG_REQUIRE),
    ("requireType", CreatedBy::GOOG_REQUIRE_TYPE),
    ("forwardDeclare", CreatedBy::GOOG_FORWARD_DECLARE),
];

impl ClosureRequireProcessor {
    // port: ClosureRequireProcessor#ClosureRequireProcessor
    fn new(ast: &Ast, name_declaration: NodeId, require_kind: CreatedBy) -> Self {
        check_argument!(NodeUtil::is_name_declaration(ast, Some(name_declaration)));
        Self {
            name_declaration,
            require_kind,
        }
    }

    /// Returns all Require built from the given statement, or null if it is not a require
    ///
    /// `name_declaration` is a VAR, LET, or CONST; returns all Requires contained in this
    /// declaration
    // port: ClosureRequireProcessor#getAllRequires
    pub fn get_all_requires(ast: &Ast, name_declaration: NodeId) -> Vec<Require> {
        let first = name_declaration.get_first_child(ast).unwrap();
        let rhs = if first.is_destructuring_lhs(ast) {
            first.get_second_child(ast)
        } else {
            name_declaration.get_first_first_child(ast)
        };
        // This may be a require, requireType, or forwardDeclare.
        let Some(require_kind) = Self::get_module_dependency_type_from_rhs(ast, rhs) else {
            return Vec::new();
        };

        Self::new(ast, name_declaration, require_kind).get_all_requires_in_declaration(ast)
    }

    /// Checks if the given rvalue is a goog.require(Type) or goog.forwardDeclare call, and if so
    /// returns which one.
    ///
    /// Returns a Closure require (where [`CreatedBy::is_closure_import`] is true) or null.
    // port: ClosureRequireProcessor#getModuleDependencyTypeFromRhs
    fn get_module_dependency_type_from_rhs(ast: &Ast, value: Option<NodeId>) -> Option<CreatedBy> {
        let value = value?;
        if !value.is_call(ast)
            || !value.has_two_children(ast)
            || !value.get_second_child(ast).unwrap().is_string_lit(ast)
        {
            return None;
        }
        let callee = value.get_first_child(ast).unwrap();
        if !callee.is_get_prop(ast) {
            return None;
        }
        let owner = callee.get_first_child(ast).unwrap();
        if !owner.is_name(ast) || owner.get_string_ref(ast) != "goog" {
            return None;
        }

        let callee_name = callee.get_string(ast);
        GOOG_DEPENDENCY_CALLS
            .iter()
            .find(|(name, _)| callee_name == *name)
            .map(|(_, created_by)| *created_by)
    }

    /// Returns a new list of all required names in [`Self::name_declaration`]
    // port: ClosureRequireProcessor#getAllRequiresInDeclaration
    fn get_all_requires_in_declaration(&self, ast: &Ast) -> Vec<Require> {
        let first = self.name_declaration.get_first_child(ast).unwrap();
        let rhs = if first.is_destructuring_lhs(ast) {
            first.get_second_child(ast).unwrap()
        } else {
            self.name_declaration.get_first_first_child(ast).unwrap()
        };
        let namespace = rhs.get_second_child(ast).unwrap().get_string(ast);

        if first.is_name(ast) {
            // const modA = goog.require('modA');
            let lhs = first;
            vec![Require::create(
                lhs.get_string(ast),
                Import::builder()
                    .module_request(namespace)
                    .local_name(lhs.get_string(ast))
                    .import_name(Export::NAMESPACE)
                    .import_node(self.name_declaration)
                    .name_node(lhs)
                    .build(),
                self.require_kind,
            )]
        } else {
            // const {x, y} = goog.require('modA');
            let object_pattern = self.name_declaration.get_first_first_child(ast).unwrap();
            if !object_pattern.is_object_pattern(ast) {
                // bad JS, ignore
                return Vec::new();
            }
            self.get_all_requires_from_destructuring(ast, object_pattern, namespace)
        }
    }

    /// Returns all requires from destructruring, like `const {x, y, z} = goog.require('a');`
    // port: ClosureRequireProcessor#getAllRequiresFromDestructuring
    fn get_all_requires_from_destructuring(
        &self,
        ast: &Ast,
        object_pattern: NodeId,
        namespace: JsString,
    ) -> Vec<Require> {
        let mut require_builder = Vec::new();
        let mut key = object_pattern.get_first_child(ast);
        while let Some(k) = key {
            if !k.is_string_key(ast) {
                // Bad code, just ignore. We warn elsewhere.
                key = k.get_next(ast);
                continue;
            }
            let lhs = k.get_only_child(ast);
            if !lhs.is_name(ast) {
                // Bad code ( e.g. `const {a = 0} = goog.require(...)`). We warn elsewhere.
                key = k.get_next(ast);
                continue;
            }
            require_builder.push(Require::create(
                lhs.get_string(ast),
                Import::builder()
                    .module_request(namespace.clone())
                    .local_name(lhs.get_string(ast))
                    .import_name(k.get_string(ast))
                    .import_node(self.name_declaration)
                    .name_node(lhs)
                    .build(),
                self.require_kind,
            ));
            key = k.get_next(ast);
        }
        require_builder
    }
}
