/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PolyfillUsageFinder.java.

//! Detects all potential usages of polyfilled classes or methods. Port of
//! `PolyfillUsageFinder.java`.
use crate::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    compiler_options::LanguageMode,
    guarded_callback::{GuardedCallback, GuardedCallbackSubclass},
    node_traversal::NodeTraversal,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, java_lang::pattern::Pattern, js_string::JsString, node::NodeId, token::Token,
};
use std::{collections::VecDeque, sync::Arc};

/// Represents a single polyfill: specifically, for a native symbol, a set of native and polyfill
/// versions, and a library to ensure is injected if the output version is less than the native
/// version.
// port: PolyfillUsageFinder.Polyfill
#[derive(Debug)]
pub struct Polyfill {
    /// The full name of the polyfill, e.g `Map` or `String.prototype.includes`
    pub native_symbol: JsString,

    /// The language version at (or above) which the native symbol is available and sufficient. If
    /// the language out flag is at least as high as {@code nativeVersion} then the polyfill is not
    /// needed. This string should be one of those returned by FeatureSet.version().
    pub native_version: String,

    /// The required language version for the polyfill to work. This should not be higher than {@code
    /// nativeVersion}, but may be the same in cases where there is no polyfill provided. This is
    /// used to emit a warning if the language out flag is too low. This string should be one of
    /// those returned by FeatureSet.version().
    pub polyfill_version: String,

    /// Runtime library to inject for the polyfill, e.g. "es6/map".
    pub library: String,

    pub kind: Kind,
}

// port: PolyfillUsageFinder.Polyfill.Kind
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    STATIC, // Map or Array.of
    METHOD, // String.prototype.includes
}

impl Polyfill {
    // port: PolyfillUsageFinder.Polyfill#Polyfill
    pub fn new(
        native_symbol: JsString,
        native_version: String,
        polyfill_version: String,
        library: String,
        kind: Kind,
    ) -> Self {
        Self {
            native_symbol,
            native_version,
            polyfill_version,
            library,
            kind,
        }
    }
}

/// Maps from polyfill names to the actual Polyfill object.
// port: PolyfillUsageFinder.Polyfills
#[derive(Debug)]
pub struct Polyfills {
    // Map of method polyfills, keyed by native method name.
    methods: IndexMap<JsString, Vec<Arc<Polyfill>>>,
    // Map of static polyfills, keyed by fully-qualified native name.
    statics: IndexMap<JsString, Arc<Polyfill>>,
    // Set of suffixes of qualified names.
    suffixes: IndexSet<JsString>,
    // Map of all polyfills, keyed by their native version (the first ECMAScript spec version in
    // which they are defined)
    by_native_version: IndexMap<String, Vec<Arc<Polyfill>>>,
}

impl Polyfills {
    // port: PolyfillUsageFinder.Polyfills#Polyfills
    fn new(
        methods: IndexMap<JsString, Vec<Arc<Polyfill>>>,
        statics: IndexMap<JsString, Arc<Polyfill>>,
        by_native_version: IndexMap<String, Vec<Arc<Polyfill>>>,
    ) -> Self {
        let suffixes = statics
            .keys()
            .map(|arg| arg.substring_from((arg.last_index_of_char(u16::from(b'.')) + 1) as usize))
            .collect::<IndexSet<_>>();
        Self {
            methods,
            statics,
            suffixes,
            by_native_version,
        }
    }

    /// Builds a Polyfills instance from a polyfill table, which is a simple
    /// text file with lines containing space-separated tokens:
    ///   [NATIVE_SYMBOL] [NATIVE_VERSION] [POLYFILL_VERSION] [LIBRARY]
    /// For example,
    ///   Array.prototype.fill es6 es3 es6/array/fill
    ///   Map es6 es3 es6/map
    ///   WeakMap es6 es6
    /// The last line, WeakMap, does not have a polyfill available, so the
    /// library token is empty.
    // port: PolyfillUsageFinder.Polyfills#fromTable
    pub fn from_table(table: &str) -> Polyfills {
        let mut methods: IndexMap<JsString, Vec<Arc<Polyfill>>> = IndexMap::<_, _>::default();
        let mut statics: IndexMap<JsString, Arc<Polyfill>> = IndexMap::<_, _>::default();
        let mut by_native_version: IndexMap<String, Vec<Arc<Polyfill>>> =
            IndexMap::<_, _>::default();
        for line in table.split('\n').filter(|s| !s.is_empty()) {
            let tokens = java_trim(line)
                .split(' ')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if tokens.len() == 1 && tokens[0].is_empty() {
                continue;
            } else if tokens.len() < 3 {
                panic!("Invalid table: too few tokens on line: {line}");
            }
            let symbol = tokens[0];
            let is_prototype_method = symbol.contains(".prototype.");
            let native_version_str = tokens[1];
            let polyfill_version_str = tokens[2];
            let polyfill = Arc::new(Polyfill::new(
                JsString::from(symbol),
                native_version_str.to_string(),
                polyfill_version_str.to_string(),
                if tokens.len() > 3 {
                    tokens[3].to_string()
                } else {
                    String::new()
                },
                if is_prototype_method {
                    Kind::METHOD
                } else {
                    Kind::STATIC
                },
            ));
            if is_prototype_method {
                methods
                    .entry(JsString::from(
                        PROTOTYPE_PREFIX.with(|p| p.matcher(symbol).replace_all("")),
                    ))
                    .or_default()
                    .push(polyfill.clone());
            } else {
                // ImmutableMap.Builder#buildOrThrow rejects duplicate keys.
                if statics.contains_key(&JsString::from(symbol)) {
                    panic!("Multiple entries with same key: {symbol}");
                }
                statics.insert(JsString::from(symbol), polyfill.clone());
            }
            by_native_version
                .entry(native_version_str.to_string())
                .or_default()
                .push(polyfill);
        }
        Polyfills::new(methods, statics, by_native_version)
    }

    // port: PolyfillUsageFinder.Polyfills#getPolyfillsNewerThan
    pub fn get_polyfills_newer_than(&self, language_mode: LanguageMode) -> Vec<Arc<Polyfill>> {
        let feature_set = language_mode.to_feature_set();
        let mut result = Vec::new();

        for (native_version_str, polyfills) in &self.by_native_version {
            let polyfill_native_feature_set =
                get_polyfill_supported_feature_set(native_version_str);
            if !feature_set.contains(polyfill_native_feature_set) {
                result.extend(polyfills.iter().cloned());
            }
        }
        result
    }
}

thread_local! {
    // The regex of `symbol.replaceAll(".*\\.prototype\\.", "")` in Polyfills#fromTable.
    static PROTOTYPE_PREFIX: Pattern = Pattern::compile(".*\\.prototype\\.");
}

// Java String#trim: removes leading and trailing characters <= ' '.
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// Converts a polyfill native version string as passed to $jscomp.polyfill, e.g. "es6", to a
/// FeatureSet.
// port: PolyfillUsageFinder#getPolyfillSupportedFeatureSet
pub fn get_polyfill_supported_feature_set(native_version_str: &str) -> FeatureSet {
    // When calculating whether a polyfill should be included for a given language_out associated
    // with a BFSY, we want to also compare using a BFSY FeatureSet. Needed because BFSY FeatureSets
    // often exclude features but the ES20XX FeatureSets do not meaning we could unnecessarily
    // include polyfills that, in reality, could be excluded.
    FeatureSet::browser_feature_set_value_of(native_version_str)
        .unwrap_or_else(|e| panic!("{e}"))
        .without(Feature::MODULES)
}

// port: PolyfillUsageFinder.PolyfillUsage
#[derive(Clone, Debug)]
pub struct PolyfillUsage {
    polyfill: Arc<Polyfill>,
    node: NodeId,
    name: JsString,
    is_explicit_global: bool,
}

impl PolyfillUsage {
    // port: PolyfillUsageFinder.PolyfillUsage#PolyfillUsage
    pub fn new(
        polyfill: Arc<Polyfill>,
        node: NodeId,
        name: JsString,
        is_explicit_global: bool,
    ) -> Self {
        Self {
            polyfill,
            node,
            name,
            is_explicit_global,
        }
    }

    // port: PolyfillUsageFinder.PolyfillUsage#createExplicit
    fn create_explicit(polyfill: Arc<Polyfill>, node: NodeId, name: JsString) -> Self {
        Self::new(polyfill, node, name, /* isExplicitGlobal= */ true)
    }

    // port: PolyfillUsageFinder.PolyfillUsage#createNonExplicit
    fn create_non_explicit(polyfill: Arc<Polyfill>, node: NodeId, name: JsString) -> Self {
        Self::new(polyfill, node, name, /* isExplicitGlobal= */ false)
    }

    // port: PolyfillUsageFinder.PolyfillUsage#polyfill
    pub fn polyfill(&self) -> &Arc<Polyfill> {
        &self.polyfill
    }

    // port: PolyfillUsageFinder.PolyfillUsage#node
    pub fn node(&self) -> NodeId {
        self.node
    }

    // port: PolyfillUsageFinder.PolyfillUsage#name
    pub fn name(&self) -> &JsString {
        &self.name
    }

    // port: PolyfillUsageFinder.PolyfillUsage#isExplicitGlobal
    pub fn is_explicit_global(&self) -> bool {
        self.is_explicit_global
    }
}

/// The `Consumer<PolyfillUsage>` of the traversal methods. It receives the compiler because Rust
/// consumers cannot capture it (DESIGN §6).
pub type PolyfillConsumer<'a> = dyn FnMut(&mut AbstractCompiler, PolyfillUsage) + 'a;

pub struct PolyfillUsageFinder {
    polyfills: Arc<Polyfills>,
}

impl PolyfillUsageFinder {
    // port: PolyfillUsageFinder#PolyfillUsageFinder
    pub fn new(polyfills: Arc<Polyfills>) -> Self {
        Self { polyfills }
    }

    /// Passes all polyfill usages found, in postorder, to the given polyfillConsumer
    ///
    /// <p>Excludes polyfill usages behind a guard, like {@code if (Promise) return
    /// Promise.resolve('ok');}
    // port: PolyfillUsageFinder#traverseExcludingGuarded
    pub fn traverse_excluding_guarded(
        &self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        polyfill_consumer: &mut PolyfillConsumer<'_>,
    ) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut Traverser::new(self, polyfill_consumer, Guard::ONLY_UNGUARDED),
        );
    }

    /// Passes all polyfill usages found, in postorder, to the given polyfillConsumer
    ///
    /// <p>Includes polyfill usages that are behind a guard, like {@code if (Promise) return
    /// Promise.resolve('ok');}
    // port: PolyfillUsageFinder#traverseIncludingGuarded
    pub fn traverse_including_guarded(
        &self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        polyfill_consumer: &mut PolyfillConsumer<'_>,
    ) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut Traverser::new(self, polyfill_consumer, Guard::ALL),
        );
    }

    /// Passes all polyfill usages found, in postorder, to the given polyfillConsumer
    ///
    /// <p>Only includes polyfill usages that are behind a guard, like {@code if (Promise) return
    /// Promise.resolve('ok');}
    // port: PolyfillUsageFinder#traverseOnlyGuarded
    pub fn traverse_only_guarded(
        &self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        polyfill_consumer: &mut PolyfillConsumer<'_>,
    ) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut Traverser::new(self, polyfill_consumer, Guard::ONLY_GUARDED),
        );
    }

    // port: PolyfillUsageFinder#maybeCreateStaticPolyfillUsageForGetPropChain
    fn maybe_create_static_polyfill_usage_for_get_prop_chain(
        &self,
        traversal: &mut NodeTraversal<'_>,
        get_prop_node: NodeId,
    ) -> Option<PolyfillUsage> {
        check_argument!(
            get_prop_node.is_get_prop(traversal) || get_prop_node.is_opt_chain_get_prop(traversal),
            &get_prop_node.to_string(traversal)
        );
        if !self
            .polyfills
            .suffixes
            .contains(get_prop_node.get_string_ref(traversal))
        {
            // Save execution time by bailing out early if the property name at the end of the chain
            // doesn't match any of the known polyfills.
            return None;
        }
        let last_component = get_prop_node.get_string(traversal);
        // NOTE: We are not using isQualifiedName() and getQualifiedName() here, because we want to
        // locate the owner node and also have this code work for optional chains.
        let mut components: VecDeque<JsString> = VecDeque::new();
        components.push_front(last_component);
        let mut owner_node = get_prop_node
            .get_first_child(traversal)
            .expect("getprop owner");
        while owner_node.is_get_prop(traversal) || owner_node.is_opt_chain_get_prop(traversal) {
            components.push_front(owner_node.get_string(traversal));
            owner_node = owner_node
                .get_first_child(traversal)
                .expect("getprop owner");
        }
        if !owner_node.is_name(traversal) {
            // Static polyfills are always fully qualified names beginning with a NAME node.
            // e.g. `Array.from` or `globalThis.Promise`
            return None;
        }
        let root_name = owner_node.get_string(traversal);
        components.push_front(root_name.clone());
        let full_name = join_dot(&components);
        let global_prefix = find_global_prefix(&full_name);
        if let Some(global_prefix) = global_prefix {
            // The full name starts with a known global value, like `goog.global.` or `globalThis.`.
            // We must strip that off before matching with the known polyfill names.
            // (Note that the connecting '.' is included and will also be stripped.)
            // Also, the presence of the explicit global value name means we don't have to check the
            // scope for a shadowing variable as we do below.
            let polyfill = self
                .polyfills
                .statics
                .get(&full_name.substring_from(global_prefix.length()));
            if let Some(polyfill) = polyfill {
                return Some(PolyfillUsage::create_explicit(
                    polyfill.clone(),
                    get_prop_node,
                    polyfill.native_symbol.clone(),
                ));
            }
        } else {
            let polyfill = self.polyfills.statics.get(&full_name);
            if let Some(polyfill) = polyfill {
                // If we see a declaration of the name, then it is defined by the source code,
                // and we won't count it as a reference to our polyfill.
                // Checking the scope is relatively expensive, so we don't want to do it until
                // we've confirmed that this node looks like it could be a polyfill reference.
                if self.is_polyfill_visible_in_scope(traversal, &root_name) {
                    return Some(PolyfillUsage::create_non_explicit(
                        polyfill.clone(),
                        get_prop_node,
                        polyfill.native_symbol.clone(),
                    ));
                }
            }
        }
        None
    }

    /// @return Whether the Polyfill name is visible in the current scope.
    // port: PolyfillUsageFinder#isPolyfillVisibleInScope
    fn is_polyfill_visible_in_scope(
        &self,
        t: &mut NodeTraversal<'_>,
        global_name: &JsString,
    ) -> bool {
        // This class is only supposed to traverse over actual sources, not externs,
        // so it shouldn't see the declarations of the things being polyfilled.

        // If the AST is normalized we can avoid building the scope (which is relatively expensive)
        // and rely on the normalization guarantee that every name in AST is unique.

        // NOTE: there is an edge case that we are purposely ignoring here.  This pass expects
        // the traverse the AST without externs, this creates a distinction between undeclared variables
        // (externs) and symbols in the global scope (internal globals).  So then against the
        // un-normalized AST for detecting whether to inject Polyfills, it won't inject,  if there is a
        // global declaration of the same name.  But when removing Polyfills (which happens when the
        // AST is normalized), it won't try to remove these same Polyfills if they were present,
        // because the scope distinction isn't used.  This doesn't change behavior because they
        // wouldn't have been injected in the first place.

        if t.get_compiler().get_life_cycle_stage() == LifeCycleStage::NORMALIZED {
            return true;
        }
        let scope = t.get_scope();
        scope.get_var(t.get_compiler(), global_name).is_none()
    }
}

// `String.join(".", components)`
fn join_dot(components: &VecDeque<JsString>) -> JsString {
    let mut units: Vec<u16> = Vec::new();
    for (i, component) in components.iter().enumerate() {
        if i > 0 {
            units.push(u16::from(b'.'));
        }
        units.extend_from_slice(component.as_units());
    }
    JsString::from_units(units)
}

// port: PolyfillUsageFinder.Guard
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Guard {
    ONLY_GUARDED,
    ONLY_UNGUARDED,
    ALL,
}

impl Guard {
    // port: PolyfillUsageFinder.Guard#shouldInclude
    fn should_include(self, is_guarded: bool) -> bool {
        match self {
            Guard::ALL => true,
            Guard::ONLY_GUARDED => is_guarded,
            Guard::ONLY_UNGUARDED => !is_guarded,
        }
    }
}

// port: PolyfillUsageFinder.Traverser
struct Traverser<'a, 'c> {
    base: GuardedCallback<JsString>,
    outer: &'a PolyfillUsageFinder,
    polyfill_consumer: &'a mut PolyfillConsumer<'c>,
    // Whether to emit usages like Promise in `if (Promise) return Promise.resolve('ok');}`
    include_guarded_usages: Guard,
}

impl<'a, 'c> Traverser<'a, 'c> {
    // port: PolyfillUsageFinder.Traverser#Traverser
    fn new(
        outer: &'a PolyfillUsageFinder,
        polyfill_consumer: &'a mut PolyfillConsumer<'c>,
        include_guarded_usages: Guard,
    ) -> Self {
        Self {
            base: GuardedCallback::new(),
            outer,
            polyfill_consumer,
            include_guarded_usages,
        }
    }

    // port: PolyfillUsageFinder.Traverser#visitName
    fn visit_name(&mut self, traversal: &mut NodeTraversal<'_>, name_node: NodeId) {
        // Rust-only: the name is looked up in place and copied only for a polyfill (D-025).
        let Some(polyfill) = self
            .outer
            .polyfills
            .statics
            .get(name_node.get_string_ref(traversal))
            .cloned()
        else {
            // no polyfill exists for this name
            return;
        };
        let name = name_node.get_string(traversal);

        if self.outer.is_polyfill_visible_in_scope(traversal, &name)
            && self
                .include_guarded_usages
                .should_include(self.is_guarded(name.clone()))
        {
            (self.polyfill_consumer)(
                traversal.get_compiler(),
                PolyfillUsage::create_non_explicit(polyfill, name_node, name),
            );
        }
    }

    // port: PolyfillUsageFinder.Traverser#visitGetPropChain
    fn visit_get_prop_chain(&mut self, traversal: &mut NodeTraversal<'_>, get_prop_node: NodeId) {
        // First see if we have a usage that matches a full static polyfill name.
        // e.g. `Array.from` or `globalThis.Promise.allSettled`
        let static_polyfill_usage = self
            .outer
            .maybe_create_static_polyfill_usage_for_get_prop_chain(traversal, get_prop_node);
        if let Some(static_polyfill_usage) = static_polyfill_usage {
            if self
                .include_guarded_usages
                .should_include(self.is_guarded(static_polyfill_usage.name().clone()))
            {
                (self.polyfill_consumer)(traversal.get_compiler(), static_polyfill_usage);
            }
        } else {
            // We don't have a static polyfill usage, but this could still be a reference to one of
            // several possible method polyfills.
            // e.g. `obj.includes(x)` could be a usage of `Array.prototype.includes` or
            // `String.prototype.includes`.
            let property_name = get_prop_node.get_string(traversal);
            let method_polyfills = self
                .outer
                .polyfills
                .methods
                .get(&property_name)
                .cloned()
                .unwrap_or_default();
            // Note that we use ".foo" as the guard check for methods to keep them distinct in case
            // there is also a static "foo" polyfill.
            if !method_polyfills.is_empty()
                && self
                    .include_guarded_usages
                    .should_include(self.is_guarded(JsString::from(".").concat(&property_name)))
            {
                for polyfill in method_polyfills {
                    (self.polyfill_consumer)(
                        traversal.get_compiler(),
                        PolyfillUsage::create_non_explicit(
                            polyfill,
                            get_prop_node,
                            property_name.clone(),
                        ),
                    );
                }
            }
        }
    }
}

impl GuardedCallbackSubclass for Traverser<'_, '_> {
    type Resource = JsString;

    fn guarded_callback(&mut self) -> &mut GuardedCallback<JsString> {
        &mut self.base
    }

    // port: PolyfillUsageFinder.Traverser#visitGuarded
    fn visit_guarded(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
        _parent: Option<NodeId>,
    ) {
        match node.get_token(traversal) {
            Token::NAME => self.visit_name(traversal, node),
            Token::GETPROP | Token::OPTCHAIN_GETPROP => self.visit_get_prop_chain(traversal, node),
            _ => {
                // nothing to do
            }
        }
    }
}

// port: PolyfillUsageFinder#findGlobalPrefix
fn find_global_prefix(qualified_name: &JsString) -> Option<JsString> {
    for global in GLOBAL_NAMES {
        let global = JsString::from(global);
        if qualified_name.starts_with(&global) {
            return Some(global);
        }
    }
    None
}

// port: PolyfillUsageFinder#GLOBAL_NAMES
const GLOBAL_NAMES: [&str; 4] = ["goog.global.", "window.", "goog$global.", "globalThis."];
