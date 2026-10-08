/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PreprocessorSymbolTable.java.

use crate::abstract_compiler::AbstractCompiler;
use closure_jstype::{
    JSTypeRegistry, TypeId, simple_reference::SimpleReference, simple_slot::SimpleSlot,
    static_typed_scope::StaticTypedScope, static_typed_slot::StaticTypedSlot,
};
use closure_rhino::{
    check_not_null,
    js_string::JsString,
    node::{Ast, NodeId},
};
use indexmap::IndexMap;
use std::sync::{Arc, Mutex};

/// A symbol table for references that are removed by preprocessor passes (like
/// `ProcessClosurePrimitives`).
///
/// Java implements `StaticTypedScope` and `StaticSymbolTable<SimpleSlot, Reference>`. The jstype
/// `SimpleSlot` is a `StaticTypedSlot`, not a Rhino `StaticSlot`, so the `StaticSymbolTable`
/// methods are inherent methods with the Java names.
pub struct PreprocessorSymbolTable {
    /// All preprocessor symbols are globals.
    symbols: IndexMap<JsString, SimpleSlot>,

    /// Java's `ArrayListMultimap`; only read through `get(name)`, so its key order is unobserved.
    refs: IndexMap<JsString, Vec<Reference>>,

    root: Option<NodeId>,
}

impl PreprocessorSymbolTable {
    // port: PreprocessorSymbolTable#PreprocessorSymbolTable
    pub fn new(root: Option<NodeId>) -> Self {
        Self {
            symbols: IndexMap::new(),
            refs: IndexMap::new(),
            root,
        }
    }

    // port: PreprocessorSymbolTable#getReferences
    pub fn get_references(&self, symbol: &SimpleSlot) -> &[Reference] {
        self.refs.get(&symbol.name).map_or(&[], Vec::as_slice)
    }

    // port: PreprocessorSymbolTable#getAllSymbols
    pub fn get_all_symbols(&self) -> impl Iterator<Item = &SimpleSlot> {
        self.symbols.values()
    }

    // port: PreprocessorSymbolTable#getScope
    pub fn get_scope(&self, _slot: &SimpleSlot) -> &dyn StaticTypedScope {
        self
    }

    // port: PreprocessorSymbolTable#addReference(Node)
    pub fn add_reference(&mut self, ast: &Ast, node: NodeId) {
        let name = self.get_qualified_name(ast, node);
        self.add_reference_with_name(node, name);
    }

    // port: PreprocessorSymbolTable#addReference(Node, String)
    pub fn add_reference_with_name(&mut self, node: NodeId, name: Option<JsString>) {
        let name = check_not_null!(name);

        self.symbols
            .entry(name.clone())
            .or_insert_with(|| SimpleSlot::new(name.clone(), None, true));

        let symbol = self.symbols.get(&name).unwrap().clone();
        self.refs
            .entry(name)
            .or_default()
            .push(Reference::new(symbol, node));
    }

    /// This variant of Node#getQualifiedName is adds special support STRING nodes which
    /// represent module names.
    // port: PreprocessorSymbolTable#getQualifiedName
    pub fn get_qualified_name(&self, ast: &Ast, n: NodeId) -> Option<JsString> {
        if n.is_string_lit(ast) {
            Some(n.get_string(ast))
        } else {
            n.get_qualified_name(ast)
        }
    }
}

impl StaticTypedScope for PreprocessorSymbolTable {
    // port: PreprocessorSymbolTable#getRootNode
    fn get_root_node(&self) -> Option<NodeId> {
        self.root
    }

    // port: PreprocessorSymbolTable#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        None
    }

    // port: PreprocessorSymbolTable#getSlot
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.symbols.get(name).map(|s| s as &dyn StaticTypedSlot)
    }

    // port: PreprocessorSymbolTable#getOwnSlot
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.get_slot(name)
    }

    // port: PreprocessorSymbolTable#getTypeOfThis
    fn get_type_of_this(&self, _reg: &JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        None
    }
}

// port: PreprocessorSymbolTable.Reference
pub struct Reference(SimpleReference<SimpleSlot>);

impl Reference {
    // port: PreprocessorSymbolTable.Reference#Reference
    pub fn new(symbol: SimpleSlot, node: NodeId) -> Self {
        Self(SimpleReference::new(symbol, node))
    }
}

impl std::ops::Deref for Reference {
    type Target = SimpleReference<SimpleSlot>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Object that maybe contains instance of the table. This object is needed because
/// PreprocessorSymbolTable is used by multiple passes in different parts of code which
/// initialized at different times (some even before compiler object is created). Instead
/// instance of factory is passed around. Each pass that uses PreprocessorSymbolTable has to call
/// maybeInitialize() before getting instance.
///
/// Java passes the one factory object around by reference; a clone shares the same cached
/// instance, and the instance is shared with the passes that add references to it.
#[derive(Clone, Default)]
pub struct CachedInstanceFactory {
    instance: Arc<Mutex<Option<Arc<Mutex<PreprocessorSymbolTable>>>>>,
}

impl CachedInstanceFactory {
    // port: PreprocessorSymbolTable.CachedInstanceFactory#maybeInitialize
    pub fn maybe_initialize(&self, compiler: &AbstractCompiler) {
        if compiler.get_options().preserves_detailed_source_info() {
            let root = compiler.get_root();
            let mut instance = self.instance.lock().unwrap();
            if instance
                .as_ref()
                .is_none_or(|i| i.lock().unwrap().get_root_node() != root)
            {
                *instance = Some(Arc::new(Mutex::new(PreprocessorSymbolTable::new(root))));
            }
        }
    }

    // port: PreprocessorSymbolTable.CachedInstanceFactory#getInstanceOrNull
    pub fn get_instance_or_null(&self) -> Option<Arc<Mutex<PreprocessorSymbolTable>>> {
        self.instance.lock().unwrap().clone()
    }
}
