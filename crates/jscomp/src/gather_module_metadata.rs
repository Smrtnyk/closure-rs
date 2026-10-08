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
//   src/com/google/javascript/jscomp/GatherModuleMetadata.java.

//! Gathers metadata around modules that is useful for checking imports / requires and creates a
//! `ModuleMetadataMap`.
use crate::{
    AbstractCompiler,
    closure_check_module::DECLARE_LEGACY_NAMESPACE_IN_NON_MODULE,
    closure_primitive_errors::{
        self, DUPLICATE_MODULE, DUPLICATE_NAMESPACE, DUPLICATE_NAMESPACE_AND_MODULE,
    },
    closure_rewrite_module,
    compiler_pass::CompilerPass,
    deps::module_loader::{ModulePath, ResolutionMode},
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    modules::module_metadata_map::{self, ModuleMetadata, ModuleMetadataMap, ModuleType},
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    process_common_js_modules::ProcessCommonJSModules,
    var::VarId,
};
use closure_parsing::parser::identifiers::Identifiers;
use closure_rhino::{
    check_not_null, check_state, js_string::JsString, node::NodeId, qualified_name::QualifiedName,
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::{Arc, LazyLock};

// port: GatherModuleMetadata#MIXED_MODULE_TYPE
pub static MIXED_MODULE_TYPE: DiagnosticType = DiagnosticType::error(
    "JSC_MIXED_MODULE_TYPE",
    "A file cannot be both {0} and {1}.",
);

// port: GatherModuleMetadata#INVALID_NAMESPACE_OR_MODULE_ID
pub static INVALID_NAMESPACE_OR_MODULE_ID: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_NAMESPACE_OR_MODULE_ID",
    "Namespace and module ID must be a dot-separated sequence of legal property identifiers and must only contain ASCII, 0-9, $, ., and _. Found ''{0}''",
);

// port: GatherModuleMetadata#INVALID_DECLARE_MODULE_ID_CALL
pub static INVALID_DECLARE_MODULE_ID_CALL: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DECLARE_NAMESPACE_CALL",
    "goog.declareModuleId parameter must be a string literal.",
);

// port: GatherModuleMetadata#DECLARE_MODULE_ID_OUTSIDE_ES6_MODULE
pub static DECLARE_MODULE_ID_OUTSIDE_ES6_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_DECLARE_MODULE_NAMESPACE_OUTSIDE_ES6_MODULE",
    "goog.declareModuleId can only be called within ES6 modules.",
);

// port: GatherModuleMetadata#MULTIPLE_DECLARE_MODULE_NAMESPACE
pub static MULTIPLE_DECLARE_MODULE_NAMESPACE: DiagnosticType = DiagnosticType::error(
    "JSC_MULTIPLE_DECLARE_MODULE_NAMESPACE",
    "goog.declareModuleId can only be called once per ES6 module.",
);

// port: GatherModuleMetadata#INVALID_REQUIRE_TYPE
pub static INVALID_REQUIRE_TYPE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_REQUIRE_TYPE",
    "Argument to goog.requireType must be a string.",
);

// port: GatherModuleMetadata#INVALID_REQUIRE_DYNAMIC
pub static INVALID_REQUIRE_DYNAMIC: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_REQUIRE_DYNAMIC",
    "Argument to goog.requireDynamic must be a string.",
);

// port: GatherModuleMetadata#INVALID_MAYBE_REQUIRE
pub static INVALID_MAYBE_REQUIRE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_MAYBE_REQUIRE",
    "Argument to goog.maybeRequireFrameworkInternalOnlyDoNotCallOrElse must be a string.",
);

// port: GatherModuleMetadata#INVALID_SET_TEST_ONLY
pub static INVALID_SET_TEST_ONLY: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_SET_TEST_ONLY",
    "Optional, single argument to goog.setTestOnly must be a string.",
);

// port: GatherModuleMetadata#INVALID_NESTED_LOAD_MODULE
pub static INVALID_NESTED_LOAD_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_NESTED_LOAD_MODULE",
    "goog.loadModule cannot be nested.",
);

// port: GatherModuleMetadata#INVALID_TOGGLE_USAGE
pub static INVALID_TOGGLE_USAGE: DiagnosticType =
    DiagnosticType::error("JSC_INVALID_TOGGLE_USAGE", "Invalid toggle usage: {0}");

// A Java static Node has no compilation arena. QualifiedName preserves the exact
// Node#matchesQualifiedName(Node) matcher for these NAME/GETPROP chains.
// port: GatherModuleMetadata#GOOG_PROVIDE
static GOOG_PROVIDE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.provide"));
// port: GatherModuleMetadata#GOOG_MODULE
static GOOG_MODULE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.module"));
// port: GatherModuleMetadata#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));
// port: GatherModuleMetadata#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: GatherModuleMetadata#GOOG_REQUIRE_TYPE
static GOOG_REQUIRE_TYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
// port: GatherModuleMetadata#GOOG_MAYBE_REQUIRE
static GOOG_MAYBE_REQUIRE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.maybeRequireFrameworkInternalOnlyDoNotCallOrElse"));
// port: GatherModuleMetadata#GOOG_REQUIRE_DYNAMIC
static GOOG_REQUIRE_DYNAMIC: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireDynamic"));
// port: GatherModuleMetadata#GOOG_SET_TEST_ONLY
static GOOG_SET_TEST_ONLY: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.setTestOnly"));
// port: GatherModuleMetadata#GOOG_MODULE_DECLARELEGACYNAMESPACE
static GOOG_MODULE_DECLARELEGACYNAMESPACE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.declareLegacyNamespace"));
// port: GatherModuleMetadata#GOOG_DECLARE_MODULE_ID
static GOOG_DECLARE_MODULE_ID: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.declareModuleId"));

// TODO(johnplaisted): Remove once clients have migrated to declareModuleId
// port: GatherModuleMetadata#GOOG_MODULE_DECLARNAMESPACE
static GOOG_MODULE_DECLARNAMESPACE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.declareNamespace"));

// port: GatherModuleMetadata#TOGGLE_NAME_PREFIX
const TOGGLE_NAME_PREFIX: &str = "TOGGLE_";

// port: GatherModuleMetadata#GOOG_LOADMODULE
static GOOG_LOADMODULE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.loadModule"));

pub struct GatherModuleMetadata {
    /// Map from module path to module. These modules represent files and thus will contain all
    /// goog namespaces that are in the file. These are not the same modules in
    /// modulesByGoogNamespace.
    modules_by_path: IndexMap<String, Arc<ModuleMetadata>>,

    /// Map from Closure namespace to module. These modules represent just the single namespace
    /// and thus each module has only one goog namespace in its googNamespaces(). These are not
    /// the same modules in modulesByPath.
    modules_by_goog_namespace: IndexMap<JsString, Arc<ModuleMetadata>>,

    /// The current module being traversed.
    current_module: Option<ModuleMetadataBuilder>,

    /// The module currentModule is nested under, if any. Modules are expected to be at most two
    /// deep (a script and then a goog.loadModule call).
    parent_module: Option<ModuleMetadataBuilder>,

    /// The call to goog.loadModule we are traversing.
    load_module_call: Option<NodeId>,

    process_common_js_modules: bool,
    module_resolution_mode: ResolutionMode,
}

impl GatherModuleMetadata {
    // port: GatherModuleMetadata#GatherModuleMetadata
    pub fn new(process_common_js_modules: bool, module_resolution_mode: ResolutionMode) -> Self {
        Self {
            modules_by_path: IndexMap::new(),
            modules_by_goog_namespace: IndexMap::new(),
            current_module: None,
            parent_module: None,
            load_module_call: None,
            process_common_js_modules,
            module_resolution_mode,
        }
    }

    // Must match closure/base.js's goog.VALID_MODULE_RE_ & also validates that dotted segments
    // are non-empty.
    // port: GatherModuleMetadata#isValidModuleId
    fn is_valid_module_id(id: &JsString) -> bool {
        // DOT_SPLITTER.split(id): Splitter.on('.'), empty segments included.
        let units = id.as_units();
        for segment in units.split(|&c| c == u16::from(b'.')) {
            if segment.is_empty() {
                return false;
            }
            for &ch in segment {
                if !Identifiers::is_identifier_part(ch) {
                    return false;
                }
            }
        }
        Self::namespace_segment_regex_matches(units)
    }

    /// `NAMESPACE_SEGMENT_REGEX.matcher(id).matches()` for the pattern
    /// `^[a-zA-Z_$][a-zA-Z0-9_$.]*$`.
    // port: GatherModuleMetadata#NAMESPACE_SEGMENT_REGEX
    fn namespace_segment_regex_matches(id: &[u16]) -> bool {
        fn is_first(c: u16) -> bool {
            matches!(c, 0x61..=0x7A | 0x41..=0x5A) || c == u16::from(b'_') || c == u16::from(b'$')
        }
        fn is_rest(c: u16) -> bool {
            is_first(c) || matches!(c, 0x30..=0x39) || c == u16::from(b'.')
        }
        match id.split_first() {
            Some((&first, rest)) => is_first(first) && rest.iter().all(|&c| is_rest(c)),
            None => false,
        }
    }
}

impl CompilerPass for GatherModuleMetadata {
    // port: GatherModuleMetadata#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, externs, &mut Finder::new(self));
        NodeTraversal::traverse(compiler, root, &mut Finder::new(self));
        compiler.set_module_metadata_map(Arc::new(ModuleMetadataMap::new(
            self.modules_by_path.clone(),
            self.modules_by_goog_namespace.clone(),
        )));
    }
}

struct ModuleMetadataBuilder {
    ambiguous: bool,
    has_module_body: bool,
    declared_module_id: Option<NodeId>,
    declares_legacy_namespace: Option<NodeId>,
    metadata_builder: module_metadata_map::Builder,
    /// LinkedHashMultiset<String>.
    goog_namespaces: module_metadata_map::Multiset<JsString>,
}

impl ModuleMetadataBuilder {
    // port: GatherModuleMetadata.ModuleMetadataBuilder#ModuleMetadataBuilder
    fn new(root_node: NodeId, path: Option<ModulePath>) -> Self {
        let mut metadata_builder = ModuleMetadata::builder();
        metadata_builder
            .path(path)
            .root_node(Some(root_node))
            .module_type(ModuleType::SCRIPT)
            .uses_closure(false)
            .is_test_only(false);
        Self {
            ambiguous: false,
            has_module_body: false,
            declared_module_id: None,
            declares_legacy_namespace: None,
            metadata_builder,
            goog_namespaces: module_metadata_map::Multiset::new(),
        }
    }

    // port: GatherModuleMetadata.ModuleMetadataBuilder#moduleType
    fn module_type(&mut self, type_: ModuleType, t: &mut NodeTraversal<'_>, n: NodeId) {
        if self.metadata_builder.get_module_type() == type_ {
            return;
        }

        if self.metadata_builder.get_module_type() == ModuleType::SCRIPT {
            self.metadata_builder.module_type(type_);
            return;
        }

        self.ambiguous = true;
        t.report(
            n,
            &MIXED_MODULE_TYPE,
            &[
                self.metadata_builder.get_module_type().description(),
                type_.description(),
            ],
        );
    }

    // port: GatherModuleMetadata.ModuleMetadataBuilder#recordDeclareModuleId
    fn record_declare_module_id(&mut self, declared_module_id: NodeId) {
        self.declared_module_id = Some(declared_module_id);
    }

    // port: GatherModuleMetadata.ModuleMetadataBuilder#recordDeclareLegacyNamespace
    fn record_declare_legacy_namespace(&mut self, declares_legacy_namespace: NodeId) {
        self.declares_legacy_namespace = Some(declares_legacy_namespace);
    }

    // port: GatherModuleMetadata.ModuleMetadataBuilder#isScript
    fn is_script(&self) -> bool {
        self.metadata_builder.get_module_type() == ModuleType::SCRIPT
    }

    // port: GatherModuleMetadata.ModuleMetadataBuilder#build
    fn build(&mut self, compiler: &mut AbstractCompiler) -> Arc<ModuleMetadata> {
        self.metadata_builder
            .goog_namespaces_builder()
            .add_all(&self.goog_namespaces);
        if !self.ambiguous {
            if self.has_module_body && self.metadata_builder.get_module_type() == ModuleType::SCRIPT
            {
                // A script with no imports or exports, but has a module body, must be an ES
                // module.
                self.metadata_builder.module_type(ModuleType::ES6_MODULE);
            }

            if let Some(declared_module_id) = self.declared_module_id
                && self.metadata_builder.get_module_type() != ModuleType::ES6_MODULE
            {
                compiler.report(JSError::make(
                    compiler,
                    declared_module_id,
                    &DECLARE_MODULE_ID_OUTSIDE_ES6_MODULE,
                    &[],
                ));
            }

            if let Some(declares_legacy_namespace) = self.declares_legacy_namespace {
                if self.metadata_builder.get_module_type() == ModuleType::GOOG_MODULE {
                    self.metadata_builder
                        .module_type(ModuleType::LEGACY_GOOG_MODULE);
                } else {
                    compiler.report(JSError::make(
                        compiler,
                        declares_legacy_namespace,
                        &DECLARE_LEGACY_NAMESPACE_IN_NON_MODULE,
                        &[],
                    ));
                }
            }
        }

        self.metadata_builder.build()
    }
}

/// Traverses the AST and build a sets of `ModuleMetadata`s.
struct Finder<'a> {
    outer: &'a mut GatherModuleMetadata,
    // Store both names and vars. Strings alone is insufficient to determine whether a name is
    // actually a toggle module (since it could have been shadowed, or may have been defined in a
    // different file), but looking up by only vars is much slower. This way we can do a fast
    // name lookup, followed by a slower var lookup only if the name is known to be a toggle
    // module name.
    toggle_module_names: IndexSet<JsString>,
    /// `Set<Var>` (a LinkedHashSet, null allowed) under Var's ScopedName equality.
    toggle_modules: Vec<Option<VarId>>,
}

impl<'a> Finder<'a> {
    fn new(outer: &'a mut GatherModuleMetadata) -> Self {
        Self {
            outer,
            toggle_module_names: IndexSet::new(),
            toggle_modules: Vec::new(),
        }
    }

    fn current_module(&mut self) -> &mut ModuleMetadataBuilder {
        check_not_null!(self.outer.current_module.as_mut())
    }

    // port: GatherModuleMetadata.Finder#visitImportOrExport
    fn visit_import_or_export(&mut self, t: &mut NodeTraversal<'_>, import_or_export: NodeId) {
        check_not_null!(self.outer.current_module.as_ref());
        self.current_module()
            .module_type(ModuleType::ES6_MODULE, t, import_or_export);
        if import_or_export.is_import(t)
            // export from
            || (import_or_export.has_two_children(t)
                && import_or_export.get_last_child(t).unwrap().is_string_lit(t))
        {
            let specifier = import_or_export.get_last_child(t).unwrap().get_string(t);
            self.current_module()
                .metadata_builder
                .es6_import_specifiers_builder()
                .add(specifier);
        }
    }

    // port: GatherModuleMetadata.Finder#visitDynamicImport
    fn visit_dynamic_import(&mut self, t: &mut NodeTraversal<'_>, dynamic_import: NodeId) {
        if dynamic_import.get_first_child(t).unwrap().is_string_lit(t) {
            let specifier = dynamic_import.get_first_child(t).unwrap().get_string(t);
            self.current_module()
                .metadata_builder
                .es6_import_specifiers_builder()
                .add(specifier);
        }
    }

    // port: GatherModuleMetadata.Finder#enterModule
    fn enter_module(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, path: Option<ModulePath>) {
        let new_module = ModuleMetadataBuilder::new(n, path);
        if self.outer.current_module.is_some() {
            if self.outer.parent_module.is_some() {
                t.report(n, &INVALID_NESTED_LOAD_MODULE, &[]);
            }
            self.outer.parent_module = self.outer.current_module.take();
        }
        self.outer.current_module = Some(new_module);
    }

    // port: GatherModuleMetadata.Finder#leaveModule
    fn leave_module(&mut self, compiler: &mut AbstractCompiler) {
        check_not_null!(self.outer.current_module.as_ref());
        let module = self.current_module().build(compiler);
        if let Some(path) = module.path() {
            self.outer
                .modules_by_path
                .insert(path.to_string(), module.clone());
        }
        for namespace in module.goog_namespaces().iter() {
            self.outer
                .modules_by_goog_namespace
                .insert(namespace.clone(), module.clone());
        }
        if let Some(parent_module) = self.outer.parent_module.as_mut() {
            parent_module
                .metadata_builder
                .nested_modules_builder()
                .push(module);
        }
        self.outer.current_module = self.outer.parent_module.take();
    }

    // port: GatherModuleMetadata.Finder#isFromGoogImport
    fn is_from_goog_import(compiler: &AbstractCompiler, goog: VarId) -> bool {
        let name_node = goog.get_name_node(compiler);

        // Because other tools are regex based we force importing this file as "import * as
        // goog".
        name_node.is_some_and(|name_node| {
            name_node.is_import_star(compiler)
                && name_node.get_string(compiler) == "goog"
                && name_node
                    .get_parent(compiler)
                    .unwrap()
                    .get_first_child(compiler)
                    .unwrap()
                    .is_empty(compiler)
                && name_node
                    .get_parent(compiler)
                    .unwrap()
                    .get_last_child(compiler)
                    .unwrap()
                    .get_string(compiler)
                    .ends_with(&"/goog.js".into())
        })
    }

    fn toggle_modules_contains(&self, compiler: &AbstractCompiler, var: Option<VarId>) -> bool {
        self.toggle_modules.iter().any(|v| match (v, var) {
            (None, None) => true,
            (Some(a), Some(b)) => a.equals(compiler, b),
            _ => false,
        })
    }

    // port: GatherModuleMetadata.Finder#visitName
    fn visit_name(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let name = n.get_string(t);
        if self.toggle_module_names.contains(&name) {
            let scope = t.get_scope();
            let name_var = scope.get_var(t.get_compiler(), name.clone());
            if self.toggle_modules_contains(t.get_compiler(), name_var) {
                let parent = n.get_parent(t).unwrap();
                if parent.is_get_prop(t) {
                    let prop = parent.get_string(t);
                    self.add_toggle(t, n, &prop);
                } else if !NodeUtil::is_name_declaration(t, Some(parent)) {
                    t.report(
                        n,
                        &INVALID_TOGGLE_USAGE,
                        &["toggle modules may not be used other than looking up properties"],
                    );
                }
            }
        }
        if name != "goog" {
            return;
        }

        let scope = t.get_scope();
        let root = scope.get_var(t.get_compiler(), "goog");
        if let Some(root) = root
            && !Self::is_from_goog_import(t.get_compiler(), root)
        {
            return;
        }

        self.current_module().metadata_builder.uses_closure(true);
    }

    // port: GatherModuleMetadata.Finder#visitGoogCall
    #[allow(clippy::collapsible_else_if)] // Retain Java control flow.
    fn visit_goog_call(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !n.has_children(t)
            || !n.get_first_child(t).unwrap().is_get_prop(t)
            || !n.get_first_child(t).unwrap().is_qualified_name(t)
        {
            return;
        }

        let getprop = n.get_first_child(t).unwrap();

        let mut first_prop = n.get_first_child(t).unwrap();

        while first_prop.is_get_prop(t) {
            first_prop = first_prop.get_first_child(t).unwrap();
        }

        if !first_prop.is_name(t) || first_prop.get_string(t) != "goog" {
            return;
        }

        let scope = t.get_scope();
        let root = scope.get_var(t.get_compiler(), "goog");

        // If this is a locally defined variable it can't be the global "goog", so exit early.
        if let Some(root) = root
            && root.is_local(t.get_compiler())
            && !root
                .get_scope(t.get_compiler())
                .is_module_scope(t.get_compiler())
        {
            return;
        }

        // If this is a module-level variable but wasn't created by importing goog.js, return.
        if let Some(root) = root
            && root
                .get_scope(t.get_compiler())
                .is_module_scope(t.get_compiler())
            && !Self::is_from_goog_import(t.get_compiler(), root)
        {
            return;
        }

        // If goog is defined in this script then it does not use Closure. If this is a bundle
        // with base.js in it, then it doesn't need base.js again.
        if root.is_none_or(|root| {
            let compiler = t.get_compiler();
            root.get_name_node(compiler)
                .and_then(|name_node| NodeUtil::get_enclosing_script(compiler, name_node))
                != NodeUtil::get_enclosing_script(compiler, n)
        }) {
            self.current_module().metadata_builder.uses_closure(true);
        }

        if GOOG_PROVIDE.matches(t, getprop) {
            self.current_module()
                .module_type(ModuleType::GOOG_PROVIDE, t, n);
            if n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t) {
                let namespace = n.get_last_child(t).unwrap().get_string(t);
                self.add_namespace(ModuleType::GOOG_PROVIDE, namespace, t, n);
            } else {
                t.report(n, &closure_rewrite_module::INVALID_PROVIDE_NAMESPACE, &[]);
                self.current_module().metadata_builder.uses_closure(false);
            }
        } else if GOOG_MODULE.matches(t, getprop) {
            self.current_module()
                .module_type(ModuleType::GOOG_MODULE, t, n);
            if n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t) {
                let namespace = n.get_last_child(t).unwrap().get_string(t);
                self.add_namespace(ModuleType::GOOG_MODULE, namespace, t, n);
            } else {
                t.report(n, &closure_rewrite_module::INVALID_MODULE_ID_ARG, &[]);
                self.current_module().metadata_builder.uses_closure(false);
            }
        } else if GOOG_MODULE_GET.matches(t, getprop) {
            // Look for a module named ???$2etoggles and a getprop of .TOGGLE_??? on the call's
            // return
            if !n.has_two_children(t)
                || !n.get_last_child(t).unwrap().is_string_lit(t)
                || !n
                    .get_last_child(t)
                    .unwrap()
                    .get_string(t)
                    .ends_with(&"$2etoggles".into())
            {
                return; // only do anything with toggle namespaces
            }
            let parent = n.get_parent(t).unwrap();
            if !parent.is_get_prop(t) {
                t.report(
                    n,
                    &INVALID_TOGGLE_USAGE,
                    &["goog.module.get of toggles module must immediately look up a single toggle"],
                );
                return;
            }
            let prop = parent.get_string(t);
            self.add_toggle(t, parent, &prop);
        } else if GOOG_MODULE_DECLARELEGACYNAMESPACE.matches(t, getprop) {
            self.current_module().record_declare_legacy_namespace(n);
        } else if GOOG_DECLARE_MODULE_ID.matches(t, getprop)
            || GOOG_MODULE_DECLARNAMESPACE.matches(t, getprop)
        {
            if self.current_module().declared_module_id.is_some() {
                t.report(n, &MULTIPLE_DECLARE_MODULE_NAMESPACE, &[]);
            }
            if n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t) {
                self.current_module().record_declare_module_id(n);
                let namespace = n.get_last_child(t).unwrap().get_string(t);
                self.add_namespace(ModuleType::GOOG_MODULE, namespace, t, n);
            } else {
                t.report(n, &INVALID_DECLARE_MODULE_ID_CALL, &[]);
            }
        } else if GOOG_REQUIRE.matches(t, getprop) {
            if n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t) {
                let namespace = n.get_last_child(t).unwrap().get_string(t);
                self.current_module()
                    .metadata_builder
                    .strongly_required_goog_namespaces_builder()
                    .add(namespace.clone());
                if namespace.ends_with(&"$2etoggles".into()) {
                    // Track imports of *.toggles.ts, which are rewritten to $2etoggles.
                    let call_parent = n.get_parent(t).unwrap();
                    let lhs = call_parent.get_first_child(t);
                    if call_parent.is_destructuring_lhs(t) {
                        // const {TOGGLE_foo} = goog.require('foo$2etoggles');
                        let keys: Vec<NodeId> = lhs.unwrap().children(t).collect();
                        for key in keys {
                            if key.is_string_key(t) {
                                let key_string = key.get_string(t);
                                self.add_toggle(t, n, &key_string);
                            } else {
                                t.report(
                                    n,
                                    &INVALID_TOGGLE_USAGE,
                                    &["must be destructured with string keys"],
                                );
                            }
                        }
                    } else if call_parent.is_name(t) {
                        // const fooToggles = goog.require('foo$2etoggles');
                        let name = call_parent.get_string(t);
                        let scope = t.get_scope();
                        let name_var = scope.get_var(t.get_compiler(), name.clone());
                        if !self.toggle_modules_contains(t.get_compiler(), name_var) {
                            self.toggle_modules.push(name_var);
                        }
                        self.toggle_module_names.insert(name);
                    } else if self.current_module().metadata_builder.get_module_type()
                        != ModuleType::GOOG_PROVIDE
                    {
                        // Side-effect toggle-module imports are not allowed, since they don't
                        // actually do anything.  We allow it in `goog.provide()` files because
                        // there's no other way to import, and since toggle modules don't declare
                        // a legacy namespace, it's unusable without a `goog.module.get()` (so we
                        // can catch the toggle use there, instead).
                        t.report(n, &INVALID_TOGGLE_USAGE, &["import must be assigned"]);
                    }
                }
            } else {
                t.report(n, &closure_primitive_errors::INVALID_REQUIRE_NAMESPACE, &[]);
            }
        } else if GOOG_REQUIRE_TYPE.matches(t, getprop) {
            if n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t) {
                let namespace = n.get_last_child(t).unwrap().get_string(t);
                self.current_module()
                    .metadata_builder
                    .weakly_required_goog_namespaces_builder()
                    .add(namespace);
            } else {
                t.report(n, &INVALID_REQUIRE_TYPE, &[]);
            }
        } else if GOOG_MAYBE_REQUIRE.matches(t, getprop) {
            if n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t) {
                let namespace = n.get_last_child(t).unwrap().get_string(t);
                self.current_module()
                    .metadata_builder
                    .maybe_required_goog_namespaces_builder()
                    .add(namespace);
            } else {
                t.report(n, &INVALID_MAYBE_REQUIRE, &[]);
            }
        } else if GOOG_SET_TEST_ONLY.matches(t, getprop) {
            if n.has_one_child(t)
                || (n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t))
            {
                self.current_module().metadata_builder.is_test_only(true);
            } else {
                t.report(n, &INVALID_SET_TEST_ONLY, &[]);
            }
        } else if GOOG_REQUIRE_DYNAMIC.matches(t, getprop) {
            if n.has_two_children(t) && n.get_last_child(t).unwrap().is_string_lit(t) {
                let namespace = n.get_last_child(t).unwrap().get_string(t);
                self.current_module()
                    .metadata_builder
                    .dynamically_required_goog_namespaces_builder()
                    .add(namespace);
            } else {
                t.report(n, &INVALID_REQUIRE_DYNAMIC, &[]);
            }
        }
    }

    /// Record a toggle usage (either a destructured import or a property lookup on a module).
    // port: GatherModuleMetadata.Finder#addToggle
    fn add_toggle(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, name: &JsString) {
        let prefix = JsString::from(TOGGLE_NAME_PREFIX);
        if name.starts_with(&prefix) {
            let toggle_name = name.substring_from(prefix.length());
            self.current_module()
                .metadata_builder
                .read_toggles_builder()
                .add(toggle_name);
        } else {
            t.report(
                n,
                &INVALID_TOGGLE_USAGE,
                &["all toggle names must start with `TOGGLE_`"],
            );
        }
    }

    /// Adds the namespaces to the module and checks if the given Closure namespace is a
    /// duplicate or not.
    // port: GatherModuleMetadata.Finder#addNamespace
    fn add_namespace(
        &mut self,
        module_type: ModuleType,
        namespace: JsString,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
    ) {
        let namespace_str = namespace.to_string_lossy();
        if module_type == ModuleType::GOOG_PROVIDE || module_type == ModuleType::LEGACY_GOOG_MODULE
        {
            let compiler = t.get_compiler();
            if !NodeUtil::is_valid_qualified_name_features(
                compiler.get_options().get_language_in().to_feature_set(),
                &namespace,
            ) {
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_NAMESPACE_OR_MODULE_ID,
                    &[&namespace_str],
                ));
            }
        }
        if module_type == ModuleType::GOOG_MODULE || module_type == ModuleType::LEGACY_GOOG_MODULE {
            // non-legacy goog.modules don't technically need to be valid qualified names
            if !GatherModuleMetadata::is_valid_module_id(&namespace) {
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_NAMESPACE_OR_MODULE_ID,
                    &[&namespace_str],
                ));
            }
        }

        let mut existing_type: Option<ModuleType> = None;
        let mut existing_file_source: Option<String> = None;
        if self.current_module().goog_namespaces.contains(&namespace) {
            existing_type = Some(self.current_module().metadata_builder.get_module_type());
            existing_file_source = t.get_source_name();
        } else {
            let existing_module = self
                .outer
                .modules_by_goog_namespace
                .get(&namespace)
                .cloned();
            if let Some(existing_module) = existing_module {
                existing_type = Some(existing_module.module_type());
                existing_file_source = existing_module.root_node().unwrap().get_source_file_name(t);
            }
        }
        self.current_module().goog_namespaces.add(namespace);
        if let Some(existing_type) = existing_type {
            let existing_file_source = existing_file_source.as_deref().unwrap_or("null");
            match existing_type {
                ModuleType::ES6_MODULE
                | ModuleType::GOOG_MODULE
                | ModuleType::LEGACY_GOOG_MODULE => {
                    let diagnostic = if module_type == ModuleType::GOOG_PROVIDE {
                        &DUPLICATE_NAMESPACE_AND_MODULE
                    } else {
                        &DUPLICATE_MODULE
                    };
                    t.report(n, diagnostic, &[&namespace_str, existing_file_source]);
                    return;
                }
                ModuleType::GOOG_PROVIDE => {
                    let diagnostic = if module_type == ModuleType::GOOG_PROVIDE {
                        &DUPLICATE_NAMESPACE
                    } else {
                        &DUPLICATE_NAMESPACE_AND_MODULE
                    };
                    t.report(n, diagnostic, &[&namespace_str, existing_file_source]);
                    return;
                }
                ModuleType::COMMON_JS | ModuleType::SCRIPT => {}
            }
            check_state!(false, "Unexpected module type: %s", existing_type.name());
        }
    }
}

impl Callback for Finder<'_> {
    // port: GatherModuleMetadata.Finder#shouldTraverse
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::SCRIPT => {
                let input = check_not_null!(t.get_input()).clone();
                let path = input.get_path(t.get_compiler());
                self.enter_module(t, n, Some(path));
            }
            Token::IMPORT | Token::EXPORT => self.visit_import_or_export(t, n),
            Token::CALL => {
                if n.is_call(t) && GOOG_LOADMODULE.matches(t, n.get_first_child(t).unwrap()) {
                    self.outer.load_module_call = Some(n);
                    self.enter_module(t, n, None);
                }
            }
            Token::MODULE_BODY => self.current_module().has_module_body = true,
            Token::DYNAMIC_IMPORT => self.visit_dynamic_import(t, n),
            _ => {}
        }

        true
    }

    // port: GatherModuleMetadata.Finder#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if self.outer.process_common_js_modules
            && self
                .outer
                .current_module
                .as_ref()
                .is_some_and(|m| m.is_script())
        {
            // A common JS import (call to "require") does not force a module to be rewritten as
            // commonJS. Only an export statement.
            let resolution_mode = self.outer.module_resolution_mode;
            if ProcessCommonJSModules::is_common_js_export(t, n, resolution_mode) {
                self.current_module()
                    .module_type(ModuleType::COMMON_JS, t, n);
                return;
            }
        }

        match n.get_token(t) {
            Token::SCRIPT => self.leave_module(t.get_compiler()),
            Token::NAME => self.visit_name(t, n),
            Token::CALL => {
                if self.outer.load_module_call == Some(n) {
                    self.leave_module(t.get_compiler());
                    self.outer.load_module_call = None;
                } else {
                    self.visit_goog_call(t, n);
                }
            }
            _ => {}
        }
    }
}
