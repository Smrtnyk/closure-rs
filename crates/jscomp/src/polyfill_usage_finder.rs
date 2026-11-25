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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/PolyfillUsageFinder.java.

//! Detects all potential usages of polyfilled classes or methods. Port of
//! `PolyfillUsageFinder.java`.
use crate::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    colors::{Color, ColorId, color_registry::ColorRegistry, standard_colors},
    compiler_options::LanguageMode,
    guarded_callback::{GuardedCallback, GuardedCallbackSubclass},
    node_traversal::NodeTraversal,
    syntactic_scope_cache::SyntacticScopeCache,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_resources::resources::resource_loader::ResourceLoader;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument,
    java_lang::pattern::Pattern,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{Arc, LazyLock},
};

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

#[derive(Debug)]
struct MethodPolyfills {
    guard_name: JsString,
    definitions: Vec<Arc<Polyfill>>,
}

/// Maps from polyfill names to the actual Polyfill object.
// port: PolyfillUsageFinder.Polyfills
#[derive(Debug)]
pub struct Polyfills {
    // Map of method polyfills, keyed by native method name.
    methods: IndexMap<JsString, MethodPolyfills>,
    // Map of static polyfills, keyed by fully-qualified native name.
    statics: IndexMap<JsString, Arc<Polyfill>>,
    // Set of suffixes of qualified names.
    suffixes: IndexSet<JsString>,
    // Possible first components of static polyfills and explicit global prefixes.
    roots: IndexSet<JsString>,
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
        let roots = statics
            .keys()
            .map(
                |name| match name.as_units().iter().position(|&c| c == u16::from(b'.')) {
                    None => name.clone(),
                    Some(end) => name.substring(0, end),
                },
            )
            .chain(
                GLOBAL_NAMES
                    .into_iter()
                    .map(|prefix| JsString::from(prefix.split_once('.').unwrap().0)),
            )
            .collect();
        let methods = methods
            .into_iter()
            .map(|(name, definitions)| {
                let guard_name = JsString::from(".").concat(&name);
                (
                    name,
                    MethodPolyfills {
                        guard_name,
                        definitions,
                    },
                )
            })
            .collect();
        Self {
            methods,
            statics,
            suffixes,
            roots,
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

    /// Rust-only (performance divergence): the jar's polyfill table `js/polyfills.txt` through
    /// [`Polyfills::from_table`], parsed once per process. The RewritePolyfills, IsolatePolyfills
    /// and RemoveUnusedCode constructors each load and parse it in Java (the three classes are in
    /// one package, so it is the same resource); the table never changes and `Polyfills` is not
    /// modified after it is built.
    pub fn from_jar_table() -> Arc<Polyfills> {
        static TABLE: LazyLock<Arc<Polyfills>> = LazyLock::new(|| {
            Arc::new(Polyfills::from_table(&ResourceLoader::load_text_resource(
                "com.google.javascript.jscomp.RewritePolyfills",
                "js/polyfills.txt",
            )))
        });
        Arc::clone(&TABLE)
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

/// Rust-only (performance divergence): what `traverse_only_guarded_cached` found in the functions
/// it traversed, kept by the caller between its traversals of a normalized tree.
#[derive(Default)]
pub(crate) struct GuardedPolyfillUsageCache {
    /// `disambiguate_properties` and whether colors exist: both select the usages passed on.
    environment: Option<(bool, bool)>,
    functions: IndexMap<NodeId, GuardedFunctionUsages>,
}

/// The guarded usages in one function that the traversal reached with nothing guarded and no
/// conditional linked (`GuardedCallbackSubclass::enter_function`), so that they depend on its
/// subtree alone.
struct GuardedFunctionUsages {
    /// The function's change stamp when it was traversed.
    stamp: u32,
    /// Every function below it then, with its change stamp. A function is a change scope of its
    /// own, and one added later changes the scope it is added to, which is one of these.
    nested: Arc<[(NodeId, u32)]>,
    /// The usages passed on, in order.
    usages: Arc<[PolyfillUsage]>,
}

/// Where the caller keeps its `GuardedPolyfillUsageCache`.
pub(crate) type GuardedPolyfillUsageCacheOf =
    fn(&mut AbstractCompiler) -> &mut GuardedPolyfillUsageCache;

pub struct PolyfillUsageFinder {
    polyfills: Arc<Polyfills>,
    disambiguate_properties: bool,
    registry: Option<Arc<ColorRegistry>>,

    // Safe as an instance field because PolyfillUsageFinder is single-threaded,
    // hasInSupertypesList only re-enters via direct recursion, and every added element is removed
    // in the finally block before returning, leaving the set empty between top-level calls
    // (avoiding per-call allocation). The calls nest, so the set is kept as a stack.
    has_in_supertypes_list_seen_set: RefCell<Vec<Color>>,
}

impl PolyfillUsageFinder {
    // port: PolyfillUsageFinder#PolyfillUsageFinder
    pub fn new(
        compiler: &AbstractCompiler,
        polyfills: Arc<Polyfills>,
        disambiguate_properties: bool,
    ) -> Self {
        Self {
            polyfills,
            disambiguate_properties,
            registry: if compiler.has_optimization_colors() {
                Some(Arc::clone(compiler.get_color_registry()))
            } else {
                None
            },
            has_in_supertypes_list_seen_set: RefCell::new(Vec::new()),
        }
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
        let mut traverser = Traverser::new(self, polyfill_consumer, Guard::ONLY_GUARDED);
        // In a normalized tree, checking a polyfill name never creates a scope.
        traverser.skip_unguardable_nodes =
            compiler.get_life_cycle_stage() == LifeCycleStage::NORMALIZED;
        NodeTraversal::traverse(compiler, root, &mut traverser);
    }

    /// Rust-only (performance divergence): `traverse_only_guarded` for a normalized tree that is
    /// traversed again and again (`RemoveUnusedCode`). A function whose subtree has not changed
    /// since an earlier traversal, and that is reached with nothing guarded and no conditional
    /// linked, is not traversed again: its usages are passed on from `cache`.
    pub(crate) fn traverse_only_guarded_cached(
        &self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        cache: GuardedPolyfillUsageCacheOf,
        polyfill_consumer: &mut PolyfillConsumer<'_>,
    ) {
        if compiler.get_life_cycle_stage() != LifeCycleStage::NORMALIZED {
            self.traverse_only_guarded(compiler, root, polyfill_consumer);
            return;
        }
        let environment = (self.disambiguate_properties, self.registry.is_some());
        let kept = cache(compiler);
        if kept.environment != Some(environment) {
            kept.functions.clear();
            kept.environment = Some(environment);
        }
        let mut traverser = Traverser::new(self, polyfill_consumer, Guard::ONLY_GUARDED);
        traverser.skip_unguardable_nodes = true;
        traverser.function_cache = Some(cache);
        NodeTraversal::traverse(compiler, root, &mut traverser);
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
        // Find the receiver before building a qualified name. Most matching suffixes occur
        // on ordinary expressions or on names that cannot begin a static polyfill.
        let mut owner_node = get_prop_node
            .get_first_child(traversal)
            .expect("getprop owner");
        while owner_node.is_get_prop(traversal) || owner_node.is_opt_chain_get_prop(traversal) {
            owner_node = owner_node
                .get_first_child(traversal)
                .expect("getprop owner");
        }
        if !owner_node.is_name(traversal) {
            return None;
        }
        let root_name = owner_node.get_string_ref(traversal);
        // A synthetic NAME can itself contain dots. The complete textual lookup below
        // handles that case, as well as optional chains.
        if !self.polyfills.roots.contains(root_name)
            && !root_name.as_units().contains(&u16::from(b'.'))
        {
            return None;
        }
        let root_name = root_name.clone();
        let mut components = VecDeque::new();
        let mut part = get_prop_node;
        while part != owner_node {
            components.push_front(part.get_string(traversal));
            part = part.get_first_child(traversal).expect("getprop owner");
        }
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
                .get(&full_name.substring_from(global_prefix.len()));
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
    skip_unguardable_nodes: bool,
    /// Rust-only (performance divergence): set by `traverse_only_guarded_cached`.
    function_cache: Option<GuardedPolyfillUsageCacheOf>,
    /// The isolated functions being traversed, outermost first, each with the lengths of
    /// `usage_log` and `function_log` when it was entered.
    recording: Vec<(NodeId, usize, usize)>,
    /// The usages passed on since the outermost function of `recording` was entered.
    usage_log: Vec<PolyfillUsage>,
    /// The functions reached since then, with their change stamps.
    function_log: Vec<(NodeId, u32)>,
    /// How many of the functions of `recording`, counted from the outermost, contain a usage
    /// that was decided by the color of a node. A color is not part of a function's change
    /// stamp, so what was found in these functions is not kept.
    color_dependent: usize,
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
            skip_unguardable_nodes: false,
            function_cache: None,
            recording: Vec::new(),
            usage_log: Vec::new(),
            function_log: Vec::new(),
            color_dependent: 0,
        }
    }

    /// Passes a usage on to the consumer (Java calls `polyfillConsumer.accept`).
    fn accept(&mut self, compiler: &mut AbstractCompiler, usage: PolyfillUsage) {
        if !self.recording.is_empty() {
            self.usage_log.push(usage.clone());
        }
        (self.polyfill_consumer)(compiler, usage);
    }

    // port: PolyfillUsageFinder.Traverser#visitName
    fn visit_name(&mut self, traversal: &mut NodeTraversal<'_>, name_node: NodeId) {
        // Rust-only: the name is looked up in place and copied only for a polyfill
        // (performance divergence).
        let outer = self.outer;
        let Some(polyfill) = outer
            .polyfills
            .statics
            .get(name_node.get_string_ref(traversal))
        else {
            // no polyfill exists for this name
            return;
        };
        let name = name_node.get_string(traversal);

        if outer.is_polyfill_visible_in_scope(traversal, &name)
            && self
                .include_guarded_usages
                .should_include(self.base.is_guarded(name.clone()))
        {
            self.accept(
                traversal.get_compiler(),
                PolyfillUsage::create_non_explicit(polyfill.clone(), name_node, name),
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
                self.accept(traversal.get_compiler(), static_polyfill_usage);
            }
        } else {
            // We don't have a static polyfill usage, but this could still be a reference to one of
            // several possible method polyfills.
            // e.g. `obj.includes(x)` could be a usage of `Array.prototype.includes` or
            // `String.prototype.includes`.
            let outer = self.outer;
            let Some(methods) = outer
                .polyfills
                .methods
                .get(get_prop_node.get_string_ref(traversal))
            else {
                return;
            };
            if self
                .include_guarded_usages
                .should_include(self.base.is_guarded(methods.guard_name.clone()))
            {
                let property_name = get_prop_node.get_string(traversal);
                for polyfill in &methods.definitions {
                    // The iterator test only applies to the exact owner "Iterator".
                    if outer.disambiguate_properties
                        && polyfill.native_symbol.starts_with("Iterator.prototype.")
                    {
                        self.color_dependent = self.recording.len();
                        if outer.is_disjoint_from_iterator(
                            traversal,
                            get_prop_node,
                            Some("Iterator"),
                        ) {
                            continue;
                        }
                    }
                    self.accept(
                        traversal.get_compiler(),
                        PolyfillUsage::create_non_explicit(
                            polyfill.clone(),
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
        if self.skip_unguardable_nodes && !self.base.can_guard_any_resource() {
            return;
        }
        match node.get_token(traversal) {
            Token::NAME => self.visit_name(traversal, node),
            Token::GETPROP | Token::OPTCHAIN_GETPROP => self.visit_get_prop_chain(traversal, node),
            _ => {
                // nothing to do
            }
        }
    }

    fn enter_function(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        function: NodeId,
        isolated: bool,
    ) -> bool {
        let Some(cache) = self.function_cache else {
            return false;
        };
        let compiler = traversal.get_compiler();
        let stamp = SyntacticScopeCache::subtree_stamp(compiler, function);
        if !self.recording.is_empty() {
            self.function_log.push((function, stamp));
        }
        if !isolated {
            return false;
        }
        let kept = cache(compiler)
            .functions
            .get(&function)
            .filter(|kept| kept.stamp == stamp)
            .map(|kept| (kept.nested.clone(), kept.usages.clone()));
        if let Some((nested, usages)) = kept
            && nested.iter().all(|&(nested_function, nested_stamp)| {
                SyntacticScopeCache::cached_subtree_stamp(compiler, nested_function) == nested_stamp
            })
        {
            if !self.recording.is_empty() {
                self.function_log.extend_from_slice(&nested);
            }
            for usage in usages.iter() {
                self.accept(compiler, usage.clone());
            }
            return true;
        }
        self.recording
            .push((function, self.usage_log.len(), self.function_log.len()));
        false
    }

    fn exit_function(&mut self, traversal: &mut NodeTraversal<'_>, function: NodeId) {
        let Some(cache) = self.function_cache else {
            return;
        };
        if self.recording.last().is_none_or(|&(f, _, _)| f != function) {
            return;
        }
        let (_, usage_start, function_start) = self.recording.pop().unwrap();
        let compiler = traversal.get_compiler();
        if self.color_dependent > self.recording.len() {
            // This function and the ones around it stay out of the cache.
            self.color_dependent = self.recording.len();
            cache(compiler).functions.swap_remove(&function);
        } else {
            let kept = GuardedFunctionUsages {
                stamp: SyntacticScopeCache::cached_subtree_stamp(compiler, function),
                nested: self.function_log[function_start..].into(),
                usages: self.usage_log[usage_start..].into(),
            };
            cache(compiler).functions.insert(function, kept);
        }
        if self.recording.is_empty() {
            self.usage_log.clear();
            self.function_log.clear();
        }
    }
}

// port: PolyfillUsageFinder#findGlobalPrefix
fn find_global_prefix(qualified_name: &JsString) -> Option<&'static str> {
    GLOBAL_NAMES
        .into_iter()
        .find(|&global| qualified_name.starts_with(global))
}

// port: PolyfillUsageFinder#GLOBAL_NAMES
const GLOBAL_NAMES: [&str; 4] = ["goog.global.", "window.", "goog$global.", "globalThis."];

// port: PolyfillUsageFinder#STANDARD_COLLECTION_IDS
const STANDARD_COLLECTION_IDS: [ColorId; 14] = [
    standard_colors::ARRAY_ID,
    standard_colors::ARRAY_PROTOTYPE_ID,
    standard_colors::READONLY_ARRAY_ID,
    standard_colors::READONLY_ARRAY_PROTOTYPE_ID,
    standard_colors::I_TEMPLATE_ARRAY_ID,
    standard_colors::I_TEMPLATE_ARRAY_PROTOTYPE_ID,
    standard_colors::MAP_ID,
    standard_colors::MAP_PROTOTYPE_ID,
    standard_colors::READONLY_MAP_ID,
    standard_colors::READONLY_MAP_PROTOTYPE_ID,
    standard_colors::SET_ID,
    standard_colors::SET_PROTOTYPE_ID,
    standard_colors::READONLY_SET_ID,
    standard_colors::READONLY_SET_PROTOTYPE_ID,
];

impl PolyfillUsageFinder {
    // port: PolyfillUsageFinder#isDisjointFromIterator(Node,String)
    pub fn is_disjoint_from_iterator(
        &self,
        ast: &Ast,
        get_prop_node: NodeId,
        owner_name: Option<&str>,
    ) -> bool {
        // Currently only checks "Iterator" because ES2025 Iterator helper methods (forEach, reduce,
        // toArray) collide with very common Array/Collection method names, causing significant
        // binary size regressions when polyfills cannot be pruned.
        if owner_name != Some("Iterator") {
            return false;
        }
        // Registry can be null in three supported scenarios:
        // 1. Transpile only builds.
        // 2. Monolithic builds without type checking.
        // 3. Precompiled .typedast (stage 2) builds with type-based optimizations disabled.
        if self.registry.is_none() {
            return false;
        }
        let Some(receiver_color) = get_prop_node.get_first_child(ast).unwrap().get_color(ast)
        else {
            return false;
        };
        self.is_disjoint_from_iterator_color(&receiver_color)
    }

    // port: PolyfillUsageFinder#isDisjointFromIterator(Color)
    pub fn is_disjoint_from_iterator_color(&self, receiver: &Color) -> bool {
        let iterator_color = self
            .registry
            .as_ref()
            .unwrap()
            .get(standard_colors::ITERATOR_ID);
        if *receiver == iterator_color {
            return false;
        }

        if receiver.is_union() {
            for alt in receiver.get_union_elements() {
                if !self.is_disjoint_from_iterator_color(alt) {
                    return false;
                }
            }
            return true;
        }

        if receiver.is_primitive() {
            return true;
        }
        // Back off on invalidating colors that cannot be proven disjoint from Iterator via nominal
        // supertype edges in ColorRegistry:
        // - UNKNOWN (?/*) and TOP_OBJECT (Object), which can hold any runtime object (including an
        //   Iterator) without explicit supertype edges in ColorRegistry.
        // - Structural interfaces (@record) and record types ({...}) (per b/571244650): because
        //   Iterator.prototype helper methods are declared on Iterator in the externs, a real
        //   runtime Iterator (e.g. from `arr.values()` or `Iterator.from(x)`, which inherits from
        //   Iterator.prototype) structurally satisfies a @record such as
        //   `{forEach: function(...)}`. When passed to a function parameter typed as that @record,
        //   the `.forEach()` call site only sees the @record receiver color (which has no nominal
        //   edge from Iterator in ColorRegistry), and `Iterator.from` itself does not inject the
        //   separate per-method helper polyfills.
        // - Nominal types invalidated by type mismatches elsewhere in the program.
        //
        // However, in large Google3 builds, native collection types (Array, Map, Set) and their
        // prototypes are frequently marked isInvalidating due to dynamic property access or type
        // mismatches elsewhere in the codebase. Because their nominal inheritance hierarchy is
        // sound and provably disjoint from Iterator, we exempt them from isInvalidating backoff so
        // that calls on Array/Map/Set do not inject polyfills.
        if receiver.is_invalidating() && !STANDARD_COLLECTION_IDS.contains(&receiver.get_id()) {
            return false;
        }

        // Two types are non-disjoint if either is a supertype of the other:
        // 1. hasInSupertypesList(receiver, iteratorColor): receiver is a subtype of Iterator
        //    (e.g. `class MyIter extends Iterator`), which inherits Iterator.prototype methods.
        // 2. hasInSupertypesList(iteratorColor, receiver): receiver is a supertype of Iterator
        //    (e.g. `IteratorIterable` from `arr.values()`, or `Iterable`), which can hold an
        //    Iterator instance at runtime.
        !self.has_in_supertypes_list(receiver, &iterator_color)
            && !self.has_in_supertypes_list(&iterator_color, receiver)
    }

    // port: PolyfillUsageFinder#hasInSupertypesList
    fn has_in_supertypes_list(&self, sub_ctor: &Color, super_ctor: &Color) -> bool {
        if self
            .has_in_supertypes_list_seen_set
            .borrow()
            .contains(sub_ctor)
        {
            return false;
        }
        self.has_in_supertypes_list_seen_set
            .borrow_mut()
            .push(sub_ctor.clone());
        let result = (|| {
            if sub_ctor == super_ctor {
                return true;
            }
            let registry = self.registry.as_ref().unwrap();
            for immediate_supertype in registry.get_disambiguation_supertypes(sub_ctor) {
                if immediate_supertype.is_union() {
                    for alt in immediate_supertype.get_union_elements() {
                        if self.has_in_supertypes_list(alt, super_ctor) {
                            return true;
                        }
                    }
                } else if self.has_in_supertypes_list(immediate_supertype, super_ctor) {
                    return true;
                }
            }
            false
        })();
        // finally
        self.has_in_supertypes_list_seen_set.borrow_mut().pop();
        result
    }
}
