/*
 * Copyright 2026 The closure-rs Authors.
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

// STAND-IN: SymbolTable is API-only (no CLI path); not ported
use crate::{compiler::Compiler, typed_scope::TypedScope};
use closure_jstype::JSTypeRegistry;
use closure_rhino::node::NodeId;
pub struct SymbolTable;
impl SymbolTable {
    pub fn new(_compiler: &Compiler, _registry: &JSTypeRegistry) -> Self {
        Self
    }
    pub fn add_scopes(&mut self, _scopes: Vec<TypedScope>) {}
    pub fn add_symbols_from<T>(&mut self, _symbols: &T) {}
    pub fn find_scopes(&mut self, _externs: NodeId, _root: NodeId) {
        panic!("SymbolTable#findScopes is not ported");
    }
    pub fn flatten_goog_module_exports(&mut self) {}
    pub fn fill_namespace_references(&mut self) {}
    pub fn fill_property_scopes(&mut self) {}
    pub fn fill_this_references(&mut self, _externs: NodeId, _root: NodeId) {}
    pub fn fill_property_symbols(&mut self, _externs: NodeId, _root: NodeId) {}
    pub fn fill_super_references(&mut self, _externs: NodeId, _root: NodeId) {}
    pub fn fill_js_doc_info(&mut self, _externs: NodeId, _root: NodeId) {}
    pub fn fill_symbol_visibility(&mut self, _externs: NodeId, _root: NodeId) {}
    pub fn fill_goog_provide_module_requires(&mut self, _externs: NodeId, _root: NodeId) {}
    pub fn remove_generated_symbols(&mut self) {}
}
