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
//   src/com/google/javascript/jscomp/ijs/FileInfo.java.

//! Port of `com.google.javascript.jscomp.ijs.FileInfo`: keeps track of what has been seen so far
//! in a given file.

use crate::ijs::potential_declaration::PotentialDeclaration;
use closure_rhino::{
    check_not_null,
    js_string::JsString,
    node::{Ast, NodeId},
};
use indexmap::{IndexMap, IndexSet};

/// Class to keep track of what has been seen so far in a given file.
pub struct FileInfo {
    provided_namespaces: IndexSet<JsString>,
    required_local_names: IndexSet<JsString>,
    /// Java `MultimapBuilder.linkedHashKeys().arrayListValues()`.
    declarations: IndexMap<JsString, Vec<PotentialDeclaration>>,
    is_from_type_script: bool,
}

impl FileInfo {
    // port: FileInfo#FileInfo
    pub fn new(source_file_name: &str) -> Self {
        Self {
            provided_namespaces: IndexSet::new(),
            required_local_names: IndexSet::new(),
            declarations: IndexMap::new(),
            is_from_type_script: source_file_name.ends_with(".closure.js"),
        }
    }

    // port: FileInfo#isFromTypeScript
    pub fn is_from_type_script(&self) -> bool {
        self.is_from_type_script
    }

    // port: FileInfo#recordNameDeclaration
    pub fn record_name_declaration(&mut self, ast: &Ast, qualified_name_node: NodeId) {
        self.record_declaration(PotentialDeclaration::from_name(ast, qualified_name_node));
    }

    // port: FileInfo#recordMemberFieldDef
    pub fn record_member_field_def(&mut self, ast: &Ast, field_node: NodeId) {
        self.record_declaration(PotentialDeclaration::from_member_field_def(ast, field_node));
    }

    // port: FileInfo#recordMethod
    pub fn record_method(&mut self, ast: &Ast, function_node: NodeId) {
        self.record_declaration(PotentialDeclaration::from_method(ast, function_node));
    }

    // port: FileInfo#recordStringKeyDeclaration
    pub fn record_string_key_declaration(&mut self, ast: &Ast, string_key_node: NodeId) {
        self.record_declaration(PotentialDeclaration::from_string_key(ast, string_key_node));
    }

    // port: FileInfo#recordDefine
    pub fn record_define(&mut self, ast: &Ast, call_node: NodeId) {
        self.record_declaration(PotentialDeclaration::from_define(ast, call_node));
    }

    // port: FileInfo#recordAliasDeclaration
    pub fn record_alias_declaration(&mut self, ast: &Ast, name_node: NodeId) {
        self.record_declaration(PotentialDeclaration::from_alias(ast, name_node));
    }

    // port: FileInfo#getDeclarations
    pub fn get_declarations(&mut self) -> &mut IndexMap<JsString, Vec<PotentialDeclaration>> {
        &mut self.declarations
    }

    // port: FileInfo#recordDeclaration
    pub fn record_declaration(&mut self, decl: PotentialDeclaration) {
        self.declarations
            .entry(decl.get_fully_qualified_name().clone())
            .or_default()
            .push(decl);
    }

    // port: FileInfo#recordImport
    pub fn record_import(&mut self, local_name: JsString) {
        self.required_local_names.insert(local_name);
    }

    // port: FileInfo#isNameDeclared
    pub fn is_name_declared(&self, fully_qualified_name: &JsString) -> bool {
        self.declarations.contains_key(fully_qualified_name)
    }

    // port: FileInfo#containsPrefix
    fn contains_prefix<'a>(
        fully_qualified_name: &JsString,
        prefix_namespaces: impl IntoIterator<Item = &'a JsString>,
    ) -> bool {
        for prefix in prefix_namespaces {
            if fully_qualified_name == prefix
                || fully_qualified_name.starts_with(prefix.concat(&JsString::from(".")))
            {
                return true;
            }
        }
        false
    }

    // port: FileInfo#isPrefixProvided
    pub fn is_prefix_provided(&self, fully_qualified_name: &JsString) -> bool {
        Self::contains_prefix(fully_qualified_name, &self.provided_namespaces)
    }

    // port: FileInfo#isPrefixRequired
    pub fn is_prefix_required(&self, fully_qualified_name: &JsString) -> bool {
        Self::contains_prefix(fully_qualified_name, &self.required_local_names)
    }

    // port: FileInfo#isStrictPrefixDeclared
    pub fn is_strict_prefix_declared(&self, fully_qualified_name: &JsString) -> bool {
        for prefix in self.declarations.keys() {
            if fully_qualified_name.starts_with(prefix.concat(&JsString::from("."))) {
                return true;
            }
        }
        false
    }

    // port: FileInfo#markProvided
    pub fn mark_provided(&mut self, provided_name: Option<JsString>) {
        let provided_name = check_not_null!(provided_name);
        self.provided_namespaces.insert(provided_name);
    }
}
