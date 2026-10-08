/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/modules/ClosureModuleProcessor.java.

//! Processor for goog.module
use crate::{
    AbstractCompiler,
    deps::module_loader::ModulePath,
    js_error::JSError,
    modules::{
        binding::Binding,
        closure_require_processor::{ClosureRequireProcessor, Require},
        export::Export,
        module::Module,
        module_map_creator::{DOES_NOT_HAVE_EXPORT_WITH_DETAILS, ModuleProcessor},
        module_metadata_map::ModuleMetadata,
        module_request_resolver::ModuleRequestResolver,
        resolve_export_result::ResolveExportResult,
        unresolved_module::{UnresolvedModule, UnresolvedModuleId},
    },
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_state, js_string::JsString, jscomp_base::guava_format, node::Ast,
    node::NodeId, token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

/// Processor for goog.module
///
/// The namespace of a goog.module contains all named exports, e.g. `exports.x = 0`, and any
/// 'default export's that assign directly to the `exports` object, e.g. `exports = class {}`).
///
/// The bound names include any names imported through a goog.require(Type)/forwardDeclare.
#[derive(Default)]
pub struct ClosureModuleProcessor;

impl ClosureModuleProcessor {
    // port: ClosureModuleProcessor#ClosureModuleProcessor (the compiler field is dropped, DESIGN §6)
    pub fn new() -> Self {
        Self
    }
}

// port: ClosureModuleProcessor.UnresolvedGoogModule
pub struct UnresolvedGoogModule {
    metadata: Arc<ModuleMetadata>,
    src_file_name: Option<String>,
    path: Option<ModulePath>,
    namespace: Arc<IndexMap<JsString, Binding>>,
    requires_by_local_name: Arc<IndexMap<JsString, Require>>,
    resolved: Option<Arc<Module>>,
}

impl UnresolvedGoogModule {
    // port: ClosureModuleProcessor.UnresolvedGoogModule#UnresolvedGoogModule
    fn new(
        metadata: Arc<ModuleMetadata>,
        src_file_name: Option<String>,
        path: Option<ModulePath>,
        namespace: IndexMap<JsString, Binding>,
        requires: IndexMap<JsString, Require>,
    ) -> Self {
        Self {
            metadata,
            src_file_name,
            path,
            namespace: Arc::new(namespace),
            requires_by_local_name: Arc::new(requires),
            resolved: None,
        }
    }

    /// The module behind `this` (Java's `this` inside the module's methods).
    fn this(resolver: &mut dyn ModuleRequestResolver, this: UnresolvedModuleId) -> &mut Self {
        match resolver.modules().get_mut(this) {
            UnresolvedModule::Goog(m) => m,
            _ => unreachable!("not an UnresolvedGoogModule"),
        }
    }

    // port: ClosureModuleProcessor.UnresolvedGoogModule#resolveExport(ModuleRequestResolver,String)
    pub(crate) fn resolve_export(&self, export_name: &JsString) -> ResolveExportResult {
        if self.namespace.contains_key(export_name) {
            return ResolveExportResult::of(self.namespace.get(export_name).unwrap().clone());
        }
        ResolveExportResult::not_found()
    }

    // port: ClosureModuleProcessor.UnresolvedGoogModule#resolve
    pub(crate) fn resolve(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        _module_specifier: Option<&JsString>,
    ) -> Arc<Module> {
        if Self::this(module_request_resolver, this).resolved.is_none() {
            // Every import creates a locally bound name.
            let bound_names: IndexMap<JsString, Binding> =
                Self::get_all_resolved_imports(this, compiler, module_request_resolver);

            let m = Self::this(module_request_resolver, this);
            m.resolved = Some(Arc::new(
                Module::builder()
                    .path(m.path.clone())
                    .metadata(m.metadata.clone())
                    .namespace((*m.namespace).clone())
                    .bound_names(bound_names)
                    .local_name_to_local_export(IndexMap::new())
                    .closure_namespace(Some(get_only_element(m.metadata.goog_namespaces().iter())))
                    .build(),
            ));
        }
        Self::this(module_request_resolver, this)
            .resolved
            .clone()
            .unwrap()
    }

    /// A map from import bound name to binding.
    // port: ClosureModuleProcessor.UnresolvedGoogModule#getAllResolvedImports
    fn get_all_resolved_imports(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
    ) -> IndexMap<JsString, Binding> {
        let mut imports = IndexMap::new();
        let requires_by_local_name = Self::this(module_request_resolver, this)
            .requires_by_local_name
            .clone();
        for name in requires_by_local_name.keys() {
            let b = Self::resolve_import(this, compiler, module_request_resolver, name);
            if b.resolved() {
                imports.insert(name.clone(), b.get_binding().unwrap().clone());
            }
        }
        imports
    }

    // port: ClosureModuleProcessor.UnresolvedGoogModule#resolveImport
    fn resolve_import(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        name: &JsString,
    ) -> ResolveExportResult {
        let require = Self::this(module_request_resolver, this)
            .requires_by_local_name
            .get(name)
            .unwrap()
            .clone();
        let import_record = require.import_record();
        let requested = module_request_resolver.resolve_import(compiler, import_record);
        match requested {
            None => ResolveExportResult::error(),
            Some(requested) if *import_record.import_name() == Export::NAMESPACE => {
                // Return a binding based on the other module's metadata.
                ResolveExportResult::of(Binding::from_closure_namespace(
                    requested.metadata(module_request_resolver),
                    import_record.name_node(),
                    import_record.module_request().clone(),
                    require.created_by(),
                ))
            }
            Some(requested) => {
                let result = requested.resolve_export_with_sets(
                    compiler,
                    module_request_resolver,
                    Some(import_record.module_request()),
                    import_record.import_name(),
                    &mut IndexSet::new(),
                    &mut IndexSet::new(),
                );
                if !result.found() && !result.had_error() {
                    Self::report_invalid_destructuring_require(
                        this,
                        compiler,
                        module_request_resolver,
                        requested,
                        import_record,
                    );
                    return ResolveExportResult::error();
                }
                // Java: importRecord.nameNode() == null ? importNode() : nameNode(); an Import's
                // nameNode is never null.
                let for_source_info = import_record.name_node();
                result.copy(Some(for_source_info), require.created_by())
            }
        }
    }

    // port: ClosureModuleProcessor.UnresolvedGoogModule#metadata
    pub(crate) fn metadata(&self) -> Arc<ModuleMetadata> {
        self.metadata.clone()
    }

    // port: ClosureModuleProcessor.UnresolvedGoogModule#getExportedNames(ModuleRequestResolver)
    pub(crate) fn get_exported_names(&self) -> Arc<IndexSet<JsString>> {
        // Unsupported until such time as it becomes useful
        panic!("java.lang.UnsupportedOperationException");
    }

    // port: ClosureModuleProcessor.UnresolvedGoogModule#getExportedNames(ModuleRequestResolver,Set)
    pub(crate) fn get_exported_names_visited(&self) -> Arc<IndexSet<JsString>> {
        panic!("java.lang.UnsupportedOperationException");
    }

    // port: ClosureModuleProcessor.UnresolvedGoogModule#reset
    pub(crate) fn reset(&mut self) {
        self.resolved = None;
    }

    /// Reports an error given an invalid destructuring require.
    // port: ClosureModuleProcessor.UnresolvedGoogModule#reportInvalidDestructuringRequire
    fn report_invalid_destructuring_require(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        requested: UnresolvedModuleId,
        import_record: &crate::modules::import::Import,
    ) {
        let mut additional_info = String::new();
        if let UnresolvedModule::Goog(unresolved_goog_module) =
            module_request_resolver.modules().get(requested)
        {
            // Detect some edge cases and given more helpful error messages.
            let exports = unresolved_goog_module.namespace.clone();
            if exports.contains_key(&JsString::from(Export::NAMESPACE)) {
                // Can't use destructuring imports on a goog.module with a default export like
                //   exports = class {
                // (even if there is an assignment like `exports.Bar = 0;` later)
                additional_info = guava_format(
                    // Use Strings.lenientFormat for GWT/J2CL compatability
                    concat!(
                        "\n",
                        "The goog.module \"%s\" cannot be destructured as it contains a default",
                        " export, not named exports. See %s."
                    ),
                    &[
                        import_record.module_request().to_string(),
                        "https://github.com/google/closure-library/wiki/goog.module%3A-an-ES6-module-like-alternative-to-goog.provide#destructuring-imports".to_string(),
                    ],
                );
                if may_be_accidental_default_export(compiler, import_record.import_name(), &exports)
                {
                    // Give the user a more detailed error message, since this is a tricky edge
                    // case.
                    additional_info += &guava_format(
                        concat!(
                            "\n",
                            "Either use a non-destructuring require or rewrite the goog.module",
                            " \"%s\" to support destructuring requires. For example, consider",
                            " replacing\n",
                            "  exports = {%s: <value>[, ...]};\n",
                            "with individual named export assignments like\n",
                            "  exports.%s = <value>;\n"
                        ),
                        &[
                            import_record.module_request().to_string(),
                            import_record.import_name().to_string(),
                            import_record.import_name().to_string(),
                        ],
                    );
                }
            }
        }

        let src_file_name = Self::this(module_request_resolver, this)
            .src_file_name
            .clone();
        let import_node = import_record.import_node();
        let error = JSError::builder(
            &DOES_NOT_HAVE_EXPORT_WITH_DETAILS,
            &[&import_record.import_name().to_string(), &additional_info],
        );
        // JSError.make(String sourceName, ...) with a null source name leaves it unset.
        let error = match src_file_name {
            Some(src_file_name) => error.set_source_location(
                &src_file_name,
                import_node.get_lineno(compiler),
                import_node.get_charno(compiler),
            ),
            None => error,
        };
        compiler.report(error.build());
    }
}

/// Guava's `Iterables.getOnlyElement`.
fn get_only_element<'a>(mut iter: impl Iterator<Item = &'a JsString>) -> JsString {
    let first = iter.next().expect("java.util.NoSuchElementException");
    check_argument!(iter.next().is_none(), "expected one element");
    first.clone()
}

/// Returns whether the user appears to have confused a default export of an object literal with
/// the named export object literal shorthand.
///
/// Basically, `exports = {foo: 0};`, does /not/ create a 'named export' of 'foo' because 0 is not
/// a name. So users cannot destructuring-require `const {foo} = goog.require('the.module');`.
/// However, if instead of `0` the user exported a name like `bar`, the user could use a
/// destructuring require.
// port: ClosureModuleProcessor#mayBeAccidentalDefaultExport
fn may_be_accidental_default_export(
    ast: &Ast,
    import_name: &JsString,
    exports: &IndexMap<JsString, Binding>,
) -> bool {
    let default_export = exports
        .get(&JsString::from(Export::NAMESPACE))
        .unwrap()
        .originating_export()
        .unwrap()
        .export_node()
        .unwrap();
    check_state!(
        default_export.matches_name(ast, "exports")
            && default_export.get_parent(ast).unwrap().is_assign(ast),
        "%s",
        default_export.to_string(ast)
    );
    let exported_value = default_export.get_next(ast).unwrap();
    if !exported_value.is_object_lit(ast) {
        return false;
    }
    // Look for `importName` in the exported object literal.
    let mut key = exported_value.get_first_child(ast);
    while let Some(k) = key {
        if k.is_string_key(ast) && k.get_string(ast) == *import_name {
            return true;
        }
        key = k.get_next(ast);
    }
    false
}

impl ModuleProcessor for ClosureModuleProcessor {
    // port: ClosureModuleProcessor#process
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        metadata: Arc<ModuleMetadata>,
        path: Option<ModulePath>,
        script: Option<NodeId>,
    ) -> UnresolvedModule {
        let script = script.unwrap();
        check_argument!(
            script.is_script(compiler) || script.is_call(compiler),
            "Unexpected module root %s",
            script.to_string(compiler)
        );
        check_argument!(
            script.is_call(compiler) || path.is_some(),
            "Non goog.loadModules must have a path"
        );

        let mut module_processing_callback = ModuleProcessingCallback::new(metadata.clone());
        NodeTraversal::traverse(compiler, script, &mut module_processing_callback);

        UnresolvedModule::Goog(UnresolvedGoogModule::new(
            metadata,
            script.get_source_file_name(compiler),
            path,
            module_processing_callback.namespace,
            module_processing_callback.requires_by_local_name,
        ))
    }
}

/// Traverses a subtree rooted at a module, gathering all exports and requires
// port: ClosureModuleProcessor.ModuleProcessingCallback
struct ModuleProcessingCallback {
    metadata: Arc<ModuleMetadata>,
    /// The Closure namespace 'a.b.c' from the `goog.module('a.b.c');` statement
    closure_namespace: JsString,
    // Note: the following two maps are mutable because in some cases, we need to check if a key
    // has already been added before trying to add a second.
    /// All named exports and explicit assignments of the `exports` object
    namespace: IndexMap<JsString, Binding>,
    /// All required/forwardDeclared local names
    requires_by_local_name: IndexMap<JsString, Require>,
    /// Whether we've come across an "exports = ..." assignment
    seen_exports_assignment: bool,
}

impl ModuleProcessingCallback {
    // port: ClosureModuleProcessor.ModuleProcessingCallback#ModuleProcessingCallback
    fn new(metadata: Arc<ModuleMetadata>) -> Self {
        let closure_namespace = get_only_element(metadata.goog_namespaces().iter());
        Self {
            metadata,
            namespace: IndexMap::new(),
            requires_by_local_name: IndexMap::new(),
            closure_namespace,
            seen_exports_assignment: false,
        }
    }

    /// If an assignment is to 'exports', adds it to the list of Exports
    // port: ClosureModuleProcessor.ModuleProcessingCallback#maybeInitializeExports
    fn maybe_initialize_exports(&mut self, ast: &Ast, assignment: NodeId) {
        let lhs = assignment.get_first_child(ast).unwrap();
        let rhs = assignment.get_second_child(ast).unwrap();

        if lhs.is_name(ast) && lhs.get_string(ast) == "exports" {
            // This may be a 'named exports' or may be a default export.
            // It is a 'named export' if and only if it is assigned an object literal w/ string
            // keys, whose values are all names.
            if NodeUtil::is_named_exports_literal(ast, rhs) {
                self.initialize_named_exports_literal(ast, rhs);
            } else {
                self.seen_exports_assignment = true;
                self.mark_exports_assignment_in_namespace(lhs);
            }
        } else if lhs.is_get_prop(ast)
            && lhs.get_first_child(ast).unwrap().is_name(ast)
            && lhs.get_first_child(ast).unwrap().get_string(ast) == "exports"
        {
            let exported_id = lhs.get_string(ast);
            self.add_property_export(exported_id, lhs);
        }
    }

    /// Adds stub export declarations `exports.Foo;` to the list of Exports
    // port: ClosureModuleProcessor.ModuleProcessingCallback#maybeInitializeExportsStub
    fn maybe_initialize_exports_stub(&mut self, ast: &Ast, qname: NodeId) {
        let owner = qname.get_first_child(ast).unwrap();
        if owner.is_name(ast) && owner.get_string(ast) == "exports" {
            self.add_property_export(qname.get_string(ast), qname);
        }
    }

    /// Adds an explicit namespace export.
    ///
    /// Note that all goog.modules create an 'exports' object, but this object is only added to
    /// the Module namespace if there is an explicit' exports = ...' assignment
    // port: ClosureModuleProcessor.ModuleProcessingCallback#markExportsAssignmentInNamespace
    fn mark_exports_assignment_in_namespace(&mut self, exports_node: NodeId) {
        self.namespace.insert(
            JsString::from(Export::NAMESPACE),
            Binding::from_export(
                Export::builder()
                    .export_name(Some(JsString::from(Export::NAMESPACE)))
                    .export_node(Some(exports_node))
                    .module_metadata(self.metadata.clone())
                    .closure_namespace(Some(self.closure_namespace.clone()))
                    .module_path(self.metadata.path().cloned())
                    .build(),
                Some(exports_node),
            ),
        );
    }

    // port: ClosureModuleProcessor.ModuleProcessingCallback#initializeNamedExportsLiteral
    fn initialize_named_exports_literal(&mut self, ast: &Ast, object_lit: NodeId) {
        let mut key = object_lit.get_first_child(ast);
        while let Some(k) = key {
            self.add_property_export(k.get_string(ast), k);
            key = k.get_next(ast);
        }
    }

    /// Adds a named export to the list of Exports
    // port: ClosureModuleProcessor.ModuleProcessingCallback#addPropertyExport
    fn add_property_export(&mut self, exported_id: JsString, prop_node: NodeId) {
        if self.seen_exports_assignment {
            // We've seen an assignment "exports = ...", so this is not a named export.
            return;
        } else if self.namespace.contains_key(&exported_id) {
            // Ignore duplicate exports - this is an error but checked elsewhere.
            return;
        }
        self.namespace.insert(
            exported_id.clone(),
            Binding::from_export(
                Export::builder()
                    .export_name(Some(exported_id))
                    .export_node(Some(prop_node))
                    .module_metadata(self.metadata.clone())
                    .closure_namespace(Some(self.closure_namespace.clone()))
                    .module_path(self.metadata.path().cloned())
                    .build(),
                Some(prop_node),
            ),
        );
    }

    /// Adds a goog.require(Type) or forwardDeclare to the list of `requires_by_local_name`
    // port: ClosureModuleProcessor.ModuleProcessingCallback#maybeInitializeRequire
    fn maybe_initialize_require(&mut self, ast: &Ast, name_declaration: NodeId) {
        for require in ClosureRequireProcessor::get_all_requires(ast, name_declaration) {
            self.requires_by_local_name
                .entry(require.local_name().clone())
                .or_insert(require);
        }
    }
}

impl Callback for ModuleProcessingCallback {
    // port: ClosureModuleProcessor.ModuleProcessingCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::MODULE_BODY | Token::SCRIPT | Token::CALL /* Traverse into goog.loadModule calls. */ | Token::BLOCK => true,
            Token::FUNCTION => {
                // Only traverse into functions that are the argument of a goog.loadModule call,
                // which is the module root. Avoid traversing function declarations like:
                //     goog.module('a.b'); function (exports) { exports.x = 0; }
                let parent = parent.unwrap();
                parent.is_call(t) && Some(parent) == self.metadata.root_node()
            }
            Token::EXPR_RESULT => {
                let expr = n.get_first_child(t).unwrap();
                if expr.is_assign(t) {
                    self.maybe_initialize_exports(t, expr);
                } else if expr.is_get_prop(t) {
                    self.maybe_initialize_exports_stub(t, expr);
                }
                false
            }
            Token::CONST | Token::VAR | Token::LET => {
                // Note that `let` is valid only for `goog.forwardDeclare`.
                self.maybe_initialize_require(t, n);
                false
            }
            _ => false,
        }
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}
