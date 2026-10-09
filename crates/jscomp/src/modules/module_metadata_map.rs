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
//   src/com/google/javascript/jscomp/modules/ModuleMetadataMap.java.

//! Contains metadata around modules (or scripts) that is useful for checking imports / requires.
use crate::deps::module_loader::ModulePath;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::{fmt, hash::Hash, sync::Arc};

/// Guava's `ImmutableMultiset` / `LinkedHashMultiset`: distinct elements in first-insertion
/// order, each iterated as many times as it was added. Equality ignores order, as Guava's
/// `Multiset#equals` does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Multiset<E: Hash + Eq> {
    counts: IndexMap<E, i32>,
}

impl<E: Hash + Eq> Default for Multiset<E> {
    fn default() -> Self {
        Self {
            counts: IndexMap::<_, _>::default(),
        }
    }
}

impl<E: Hash + Eq + Clone> Multiset<E> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Multiset#add(E)` / `ImmutableMultiset.Builder#add(E)`.
    pub fn add(&mut self, element: E) {
        *self.counts.entry(element).or_insert(0) += 1;
    }

    /// `ImmutableMultiset.Builder#addAll(Iterable)`.
    pub fn add_all(&mut self, elements: &Multiset<E>) {
        for e in elements.iter() {
            self.add(e.clone());
        }
    }

    /// `Multiset#contains(Object)`.
    pub fn contains(&self, element: &E) -> bool {
        self.counts.contains_key(element)
    }

    /// `Multiset#count(Object)`.
    pub fn count(&self, element: &E) -> i32 {
        self.counts.get(element).copied().unwrap_or(0)
    }

    /// `Collection#size()`: the number of occurrences.
    pub fn size(&self) -> i32 {
        self.counts.values().sum()
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// `Multiset#elementSet()`.
    pub fn element_set(&self) -> impl Iterator<Item = &E> {
        self.counts.keys()
    }

    /// Iterates every occurrence, grouped by element in first-insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &E> {
        self.counts
            .iter()
            .flat_map(|(e, &c)| std::iter::repeat_n(e, c as usize))
    }
}

/// Describes what type of module this is.
// port: ModuleMetadataMap.ModuleType
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleType {
    ES6_MODULE,
    GOOG_MODULE,
    LEGACY_GOOG_MODULE,
    COMMON_JS,
    // The following two cases are not actually modules, but are useful to include in the map.
    /// A file that has at least one goog.provide but is not a goog.module.
    GOOG_PROVIDE,
    /// A file that is not a module and has no goog.provides.
    SCRIPT,
}

impl ModuleType {
    // port: ModuleMetadataMap.ModuleType#description
    pub fn description(self) -> &'static str {
        match self {
            ModuleType::ES6_MODULE => "an ES6 module",
            ModuleType::GOOG_MODULE => "a goog.module",
            ModuleType::LEGACY_GOOG_MODULE => "a goog.module",
            ModuleType::COMMON_JS => "a CommonJS module",
            ModuleType::GOOG_PROVIDE => "a script file that contains at least one goog.provide",
            ModuleType::SCRIPT => "a script file that does not contain a goog.provide",
        }
    }

    /// Java's `Enum#name()`.
    pub fn name(self) -> &'static str {
        match self {
            ModuleType::ES6_MODULE => "ES6_MODULE",
            ModuleType::GOOG_MODULE => "GOOG_MODULE",
            ModuleType::LEGACY_GOOG_MODULE => "LEGACY_GOOG_MODULE",
            ModuleType::COMMON_JS => "COMMON_JS",
            ModuleType::GOOG_PROVIDE => "GOOG_PROVIDE",
            ModuleType::SCRIPT => "SCRIPT",
        }
    }
}

/// Contains metadata around modules (or scripts) that is useful for checking imports / requires.
pub struct ModuleMetadataMap {
    /// Map from module path to module. These modules represent files and thus will contain all
    /// goog namespaces that are in the file. These are not the same modules in
    /// modulesByGoogNamespace.
    modules_by_path: IndexMap<String, Arc<ModuleMetadata>>,
    /// Map from Closure namespace to module. These modules represent just the single namespace
    /// and thus each module has only one goog namespace in its googNamespaces(). These are not
    /// the same modules in modulesByPath.
    modules_by_goog_namespace: IndexMap<JsString, Arc<ModuleMetadata>>,
    /// Every module metadata instance, in insertion order and deduplicated by identity.
    module_metadata: Vec<Arc<ModuleMetadata>>,
}

impl ModuleMetadataMap {
    // port: ModuleMetadataMap#ModuleMetadataMap
    pub fn new(
        modules_by_path: IndexMap<String, Arc<ModuleMetadata>>,
        modules_by_goog_namespace: IndexMap<JsString, Arc<ModuleMetadata>>,
    ) -> Self {
        // ImmutableSet.builder().addAll(modulesByPath.values()).addAll(...).build(), with
        // ModuleMetadata's reference equality.
        let mut module_metadata: Vec<Arc<ModuleMetadata>> = Vec::new();
        for m in modules_by_path
            .values()
            .chain(modules_by_goog_namespace.values())
        {
            if !module_metadata.iter().any(|o| Arc::ptr_eq(o, m)) {
                module_metadata.push(m.clone());
            }
        }
        Self {
            modules_by_path,
            modules_by_goog_namespace,
            module_metadata,
        }
    }

    // port: ModuleMetadataMap#getModulesByPath
    pub fn get_modules_by_path(&self) -> &IndexMap<String, Arc<ModuleMetadata>> {
        &self.modules_by_path
    }

    // port: ModuleMetadataMap#getModulesByGoogNamespace
    pub fn get_modules_by_goog_namespace(&self) -> &IndexMap<JsString, Arc<ModuleMetadata>> {
        &self.modules_by_goog_namespace
    }

    // port: ModuleMetadataMap#getAllModuleMetadata
    pub fn get_all_module_metadata(&self) -> &[Arc<ModuleMetadata>] {
        &self.module_metadata
    }

    // port: ModuleMetadataMap#emptyForTesting
    pub fn empty_for_testing() -> Self {
        Self::new(IndexMap::<_, _>::default(), IndexMap::<_, _>::default())
    }
}

impl Default for ModuleMetadataMap {
    fn default() -> Self {
        Self::empty_for_testing()
    }
}

/// Struct containing basic information about a module including its type and goog namespaces.
///
/// Java uses reference equality for this class; compare `Arc<ModuleMetadata>` handles with
/// `Arc::ptr_eq`.
pub struct ModuleMetadata {
    module_type: ModuleType,
    root_node: Option<NodeId>,
    uses_closure: bool,
    is_test_only: bool,
    goog_namespaces: Multiset<JsString>,
    strongly_required_goog_namespaces: Multiset<JsString>,
    dynamically_required_goog_namespaces: Multiset<JsString>,
    maybe_required_goog_namespaces: Multiset<JsString>,
    weakly_required_goog_namespaces: Multiset<JsString>,
    es6_import_specifiers: Multiset<JsString>,
    nested_modules: Vec<Arc<ModuleMetadata>>,
    read_toggles: Multiset<JsString>,
    path: Option<ModulePath>,
}

impl fmt::Debug for ModuleMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleMetadata")
            .field("module_type", &self.module_type)
            .field("root_node", &self.root_node)
            .field("path", &self.path.as_ref().map(ToString::to_string))
            .finish_non_exhaustive()
    }
}

/// Guava `ImmutableMultiset#toString`: the entry set, `e` for a count of 1, else `e x n`.
fn multiset_to_string(multiset: &Multiset<JsString>) -> String {
    let entries = multiset
        .element_set()
        .map(|e| {
            let count = multiset.count(e);
            if count == 1 {
                e.to_string()
            } else {
                format!("{e} x {count}")
            }
        })
        .collect::<Vec<_>>();
    format!("[{}]", entries.join(", "))
}

impl ModuleMetadata {
    /// The AutoValue-generated `toString`. It needs the arena for `rootNode`, so it takes the
    /// `Ast`.
    // port: ModuleMetadataMap.ModuleMetadata#toString (AutoValue)
    pub fn to_string(&self, ast: &Ast) -> String {
        let nested = self
            .nested_modules()
            .iter()
            .map(|n| n.to_string(ast))
            .collect::<Vec<_>>();
        format!(
            "ModuleMetadata{{moduleType={}, rootNode={}, usesClosure={}, isTestOnly={}, googNamespaces={}, stronglyRequiredGoogNamespaces={}, dynamicallyRequiredGoogNamespaces={}, maybeRequiredGoogNamespaces={}, weaklyRequiredGoogNamespaces={}, es6ImportSpecifiers={}, nestedModules=[{}], readToggles={}, path={}}}",
            self.module_type().name(),
            self.root_node()
                .map_or_else(|| "null".to_string(), |n| n.to_string(ast)),
            self.uses_closure(),
            self.is_test_only(),
            multiset_to_string(self.goog_namespaces()),
            multiset_to_string(self.strongly_required_goog_namespaces()),
            multiset_to_string(self.dynamically_required_goog_namespaces()),
            multiset_to_string(self.maybe_required_goog_namespaces()),
            multiset_to_string(self.weakly_required_goog_namespaces()),
            multiset_to_string(self.es6_import_specifiers()),
            nested.join(", "),
            multiset_to_string(self.read_toggles()),
            self.path()
                .map_or_else(|| "null".to_string(), |p| p.to_string()),
        )
    }
}

impl ModuleMetadata {
    // port: ModuleMetadataMap.ModuleMetadata#moduleType
    pub fn module_type(&self) -> ModuleType {
        self.module_type
    }

    // port: ModuleMetadataMap.ModuleMetadata#isEs6Module
    pub fn is_es6_module(&self) -> bool {
        self.module_type() == ModuleType::ES6_MODULE
    }

    // port: ModuleMetadataMap.ModuleMetadata#isGoogModule
    pub fn is_goog_module(&self) -> bool {
        self.is_non_legacy_goog_module() || self.is_legacy_goog_module()
    }

    // port: ModuleMetadataMap.ModuleMetadata#isNonLegacyGoogModule
    pub fn is_non_legacy_goog_module(&self) -> bool {
        self.module_type() == ModuleType::GOOG_MODULE
    }

    // port: ModuleMetadataMap.ModuleMetadata#isLegacyGoogModule
    pub fn is_legacy_goog_module(&self) -> bool {
        self.module_type() == ModuleType::LEGACY_GOOG_MODULE
    }

    // port: ModuleMetadataMap.ModuleMetadata#isGoogProvide
    pub fn is_goog_provide(&self) -> bool {
        self.module_type() == ModuleType::GOOG_PROVIDE
    }

    // port: ModuleMetadataMap.ModuleMetadata#hasLegacyGoogNamespaces
    pub fn has_legacy_goog_namespaces(&self) -> bool {
        self.is_goog_provide() || self.is_legacy_goog_module()
    }

    // port: ModuleMetadataMap.ModuleMetadata#isCommonJs
    pub fn is_common_js(&self) -> bool {
        self.module_type() == ModuleType::COMMON_JS
    }

    // port: ModuleMetadataMap.ModuleMetadata#isNonProvideScript
    pub fn is_non_provide_script(&self) -> bool {
        self.module_type() == ModuleType::SCRIPT
    }

    // port: ModuleMetadataMap.ModuleMetadata#isModule
    pub fn is_module(&self) -> bool {
        match self.module_type() {
            ModuleType::GOOG_PROVIDE | ModuleType::SCRIPT => false,
            ModuleType::COMMON_JS
            | ModuleType::ES6_MODULE
            | ModuleType::GOOG_MODULE
            | ModuleType::LEGACY_GOOG_MODULE => true,
        }
    }

    /// AST node that represents the root of this module.
    ///
    /// Null for synthetic modules for these goog.provide namespaces that are implicitly created,
    /// e.g. `goog.provide('a.b.c')` implicitly provides the namespaces 'a' and 'a.b'.
    // port: ModuleMetadataMap.ModuleMetadata#rootNode
    pub fn root_node(&self) -> Option<NodeId> {
        self.root_node
    }

    /// Whether this file uses Closure Library at all. Note that a file could use Closure Library
    /// even without calling goog.provide/module/require - there are some primitives in base.js
    /// that can be used without being required like goog.isArray.
    // port: ModuleMetadataMap.ModuleMetadata#usesClosure
    pub fn uses_closure(&self) -> bool {
        self.uses_closure
    }

    /// Whether goog.setTestOnly was called.
    // port: ModuleMetadataMap.ModuleMetadata#isTestOnly
    pub fn is_test_only(&self) -> bool {
        self.is_test_only
    }

    /// Closure namespaces that this file is associated with. Created by goog.provide,
    /// goog.module, and goog.declareModuleId.
    // port: ModuleMetadataMap.ModuleMetadata#googNamespaces
    pub fn goog_namespaces(&self) -> &Multiset<JsString> {
        &self.goog_namespaces
    }

    /// Closure namespaces this file strongly requires, i.e., arguments to goog.require calls.
    // port: ModuleMetadataMap.ModuleMetadata#stronglyRequiredGoogNamespaces
    pub fn strongly_required_goog_namespaces(&self) -> &Multiset<JsString> {
        &self.strongly_required_goog_namespaces
    }

    /// Closure namespaces this file requires dynamically, i.e., arguments to
    /// goog.requireDynamic calls.
    // port: ModuleMetadataMap.ModuleMetadata#dynamicallyRequiredGoogNamespaces
    pub fn dynamically_required_goog_namespaces(&self) -> &Multiset<JsString> {
        &self.dynamically_required_goog_namespaces
    }

    /// Closure namespaces this file conditionally requires, i.e., arguments to
    /// goog.maybeRequireFrameworkInternalOnlyDoNotCallOrElse calls.
    // port: ModuleMetadataMap.ModuleMetadata#maybeRequiredGoogNamespaces
    pub fn maybe_required_goog_namespaces(&self) -> &Multiset<JsString> {
        &self.maybe_required_goog_namespaces
    }

    /// Closure namespaces this file weakly requires, i.e., arguments to goog.requireType calls.
    // port: ModuleMetadataMap.ModuleMetadata#weaklyRequiredGoogNamespaces
    pub fn weakly_required_goog_namespaces(&self) -> &Multiset<JsString> {
        &self.weakly_required_goog_namespaces
    }

    /// Raw text of all ES6 import specifiers (includes "export from" as well).
    // port: ModuleMetadataMap.ModuleMetadata#es6ImportSpecifiers
    pub fn es6_import_specifiers(&self) -> &Multiset<JsString> {
        &self.es6_import_specifiers
    }

    // port: ModuleMetadataMap.ModuleMetadata#nestedModules
    pub fn nested_modules(&self) -> &[Arc<ModuleMetadata>] {
        &self.nested_modules
    }

    /// Readable names for all toggles that this module reads (excluding the `TOGGLE_` prefix).
    // port: ModuleMetadataMap.ModuleMetadata#readToggles
    pub fn read_toggles(&self) -> &Multiset<JsString> {
        &self.read_toggles
    }

    // port: ModuleMetadataMap.ModuleMetadata#path
    pub fn path(&self) -> Option<&ModulePath> {
        self.path.as_ref()
    }

    // port: ModuleMetadataMap.ModuleMetadata#builder
    pub fn builder() -> Builder {
        Builder::default()
    }
}

/// The AutoValue builder of `ModuleMetadata`.
#[derive(Default)]
pub struct Builder {
    module_type: Option<ModuleType>,
    root_node: Option<NodeId>,
    uses_closure: Option<bool>,
    is_test_only: Option<bool>,
    goog_namespaces_builder: Multiset<JsString>,
    strongly_required_goog_namespaces_builder: Multiset<JsString>,
    dynamically_required_goog_namespaces_builder: Multiset<JsString>,
    maybe_required_goog_namespaces_builder: Multiset<JsString>,
    weakly_required_goog_namespaces_builder: Multiset<JsString>,
    es6_import_specifiers_builder: Multiset<JsString>,
    nested_modules_builder: Vec<Arc<ModuleMetadata>>,
    read_toggles_builder: Multiset<JsString>,
    path: Option<ModulePath>,
}

impl Builder {
    // port: ModuleMetadataMap.ModuleMetadata.Builder#build
    pub fn build(&mut self) -> Arc<ModuleMetadata> {
        // AutoValue_ModuleMetadataMap_ModuleMetadata.Builder#build
        let mut missing = String::new();
        if self.module_type.is_none() {
            missing.push_str(" moduleType");
        }
        if self.uses_closure.is_none() {
            missing.push_str(" usesClosure");
        }
        if self.is_test_only.is_none() {
            missing.push_str(" isTestOnly");
        }
        check_state!(
            missing.is_empty(),
            "Missing required properties:%s",
            missing
        );
        Arc::new(ModuleMetadata {
            module_type: self.module_type.unwrap(),
            root_node: self.root_node,
            uses_closure: self.uses_closure.unwrap(),
            is_test_only: self.is_test_only.unwrap(),
            goog_namespaces: self.goog_namespaces_builder.clone(),
            strongly_required_goog_namespaces: self
                .strongly_required_goog_namespaces_builder
                .clone(),
            dynamically_required_goog_namespaces: self
                .dynamically_required_goog_namespaces_builder
                .clone(),
            maybe_required_goog_namespaces: self.maybe_required_goog_namespaces_builder.clone(),
            weakly_required_goog_namespaces: self.weakly_required_goog_namespaces_builder.clone(),
            es6_import_specifiers: self.es6_import_specifiers_builder.clone(),
            nested_modules: self.nested_modules_builder.clone(),
            read_toggles: self.read_toggles_builder.clone(),
            path: self.path.clone(),
        })
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#googNamespacesBuilder
    pub fn goog_namespaces_builder(&mut self) -> &mut Multiset<JsString> {
        &mut self.goog_namespaces_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#addGoogNamespace
    pub fn add_goog_namespace(&mut self, namespace: impl Into<JsString>) -> &mut Self {
        self.goog_namespaces_builder().add(namespace.into());
        self
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#stronglyRequiredGoogNamespacesBuilder
    pub fn strongly_required_goog_namespaces_builder(&mut self) -> &mut Multiset<JsString> {
        &mut self.strongly_required_goog_namespaces_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#dynamicallyRequiredGoogNamespacesBuilder
    pub fn dynamically_required_goog_namespaces_builder(&mut self) -> &mut Multiset<JsString> {
        &mut self.dynamically_required_goog_namespaces_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#maybeRequiredGoogNamespacesBuilder
    pub fn maybe_required_goog_namespaces_builder(&mut self) -> &mut Multiset<JsString> {
        &mut self.maybe_required_goog_namespaces_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#weaklyRequiredGoogNamespacesBuilder
    pub fn weakly_required_goog_namespaces_builder(&mut self) -> &mut Multiset<JsString> {
        &mut self.weakly_required_goog_namespaces_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#es6ImportSpecifiersBuilder
    pub fn es6_import_specifiers_builder(&mut self) -> &mut Multiset<JsString> {
        &mut self.es6_import_specifiers_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#nestedModulesBuilder
    pub fn nested_modules_builder(&mut self) -> &mut Vec<Arc<ModuleMetadata>> {
        &mut self.nested_modules_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#readTogglesBuilder
    pub fn read_toggles_builder(&mut self) -> &mut Multiset<JsString> {
        &mut self.read_toggles_builder
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#path
    pub fn path(&mut self, value: Option<ModulePath>) -> &mut Self {
        self.path = value;
        self
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#usesClosure
    pub fn uses_closure(&mut self, value: bool) -> &mut Self {
        self.uses_closure = Some(value);
        self
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#isTestOnly
    pub fn is_test_only(&mut self, value: bool) -> &mut Self {
        self.is_test_only = Some(value);
        self
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#moduleType()
    pub fn get_module_type(&self) -> ModuleType {
        // AutoValue: the getter throws when the property has not been set.
        check_state!(
            self.module_type.is_some(),
            "Property \"moduleType\" has not been set"
        );
        self.module_type.unwrap()
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#moduleType(ModuleType)
    pub fn module_type(&mut self, value: ModuleType) -> &mut Self {
        self.module_type = Some(value);
        self
    }

    // port: ModuleMetadataMap.ModuleMetadata.Builder#rootNode
    pub fn root_node(&mut self, root: Option<NodeId>) -> &mut Self {
        self.root_node = root;
        self
    }
}
