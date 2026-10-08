/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/modules/EsModuleProcessor.java.

//! Collects information related to and resolves ES imports and exports. Also performs several ES
//! module related checks.
use crate::{
    AbstractCompiler,
    deps::module_loader::ModulePath,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    modules::{
        binding::{Binding, CreatedBy},
        export::Export,
        export_trace::ExportTrace,
        goog_es_imports::GoogEsImports,
        import::Import,
        module::Module,
        module_map_creator::{DOES_NOT_HAVE_EXPORT, ModuleProcessor},
        module_metadata_map::ModuleMetadata,
        module_request_resolver::ModuleRequestResolver,
        resolve_export_result::ResolveExportResult,
        unresolved_module::{UnresolvedModule, UnresolvedModuleId},
    },
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_util,
};
use closure_rhino::{check_state, js_string::JsString, node::NodeId, token::Token};
use indexmap::{IndexMap, IndexSet};
use std::{collections::BTreeSet, sync::Arc};

/// Error occurs when there is an ambiguous export, which can happen if there are multiple
/// `export * from` statements. If two modules that were `export * from`'d have the same key as an
/// export that export is now ambiguous. It can be resolved by exporting that key explicitly
/// locally.
///
/// Note: This is purposefully a warning. The spec does not treat this as an error. It is only an
/// error if an import attempts to use an ambiguous name. But just having ambiguous names is not
/// itself an error.
// port: EsModuleProcessor#AMBIGUOUS_EXPORT_DEFINITION
pub static AMBIGUOUS_EXPORT_DEFINITION: DiagnosticType = DiagnosticType::warning(
    "JSC_AMBIGUOUS_EXPORT_DEFINITION",
    "The export \"{0}\" is ambiguous.",
);

// port: EsModuleProcessor#CYCLIC_EXPORT_DEFINITION
pub static CYCLIC_EXPORT_DEFINITION: DiagnosticType = DiagnosticType::error(
    "JSC_CYCLIC_EXPORT_DEFINITION",
    "Cyclic export detected while resolving name \"{0}\".",
);

// Note: We only check for duplicate exports, not imports. Imports cannot be shadowed (by any
// other binding, including other imports) and so we check that in VariableReferenceCheck.
// port: EsModuleProcessor#DUPLICATE_EXPORT
pub static DUPLICATE_EXPORT: DiagnosticType =
    DiagnosticType::error("JSC_DUPLICATE_EXPORT", "Duplicate export of \"{0}\".");

// port: EsModuleProcessor#IMPORTED_AMBIGUOUS_EXPORT
pub static IMPORTED_AMBIGUOUS_EXPORT: DiagnosticType = DiagnosticType::error(
    "JSC_IMPORTED_AMBIGUOUS_EXPORT",
    "The requested name \"{0}\" is ambiguous.",
);

// port: EsModuleProcessor#NAMESPACE_IMPORT_CANNOT_USE_STAR
pub static NAMESPACE_IMPORT_CANNOT_USE_STAR: DiagnosticType = DiagnosticType::error(
    "JSC_NAMESPACE_IMPORT_CANNOT_USE_STAR",
    concat!(
        "Namespace imports ('goog:some.Namespace') cannot use import * as. ",
        "Did you mean to import {0} from ''{1}'';?"
    ),
);

// port: EsModuleProcessor#CANNOT_PATH_IMPORT_CLOSURE_FILE
pub static CANNOT_PATH_IMPORT_CLOSURE_FILE: DiagnosticType = DiagnosticType::error(
    "JSC_CANNOT_PATH_IMPORT_CLOSURE_FILE",
    concat!(
        "Cannot import Closure files by path. Use either import 'goog:namespace' or",
        " goog.require('namespace')"
    ),
);

/// Marks all exports that are mutated in an inner scope as mutable.
///
/// Exports mutated at the module scope are not marked as mutable as they are effectively constant
/// after module evaluation.
// port: EsModuleProcessor.FindMutableExports
struct FindMutableExports<'a> {
    exports: &'a mut Vec<Export>,
    exports_by_local_name: IndexMap<JsString, Vec<Export>>,
}

impl<'a> FindMutableExports<'a> {
    // port: EsModuleProcessor.FindMutableExports#FindMutableExports
    fn new(exports: &'a mut Vec<Export>) -> Self {
        // There may be multiple exports with the same local name because you can export a local
        // variable with an alias many different times. Example:
        //
        // let x;
        // export {x as y, x as z};
        let mut exports_by_local_name: IndexMap<JsString, Vec<Export>> = IndexMap::new();
        for e in exports.iter() {
            exports_by_local_name
                .entry(e.local_name().unwrap().clone())
                .or_default()
                .push(e.clone());
        }
        Self {
            exports,
            exports_by_local_name,
        }
    }
}

impl Callback for FindMutableExports<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: EsModuleProcessor.FindMutableExports#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_name(t) {
            return;
        }

        let scope = t.get_scope();
        if !NodeUtil::is_l_value(t, n)
            || scope
                .get_closest_hoist_scope(t.get_compiler())
                .unwrap()
                .is_module_scope(t.get_compiler())
        {
            return;
        }

        let name = n.get_string(t);
        let exports: Vec<Export> = self
            .exports_by_local_name
            .get(&name)
            .cloned()
            .unwrap_or_default();
        if exports.is_empty() {
            return;
        }

        let var = scope.get_var(t.get_compiler(), name);

        // A var declared in the module scope with the same name as an export must be the
        // export. And we know we're setting it in a function scope, so this cannot be the
        // declaration itself. We must be mutating.
        match var {
            None => return,
            Some(var) => {
                let var_scope = var.get_scope(t.get_compiler());
                if !var_scope.is_module_scope(t.get_compiler()) {
                    return;
                }
            }
        }

        for e in exports {
            let i = self.exports.iter().position(|x| *x == e).unwrap();
            let mutated = e.mutated_copy();
            self.exports[i] = mutated.clone();
            let list = self
                .exports_by_local_name
                .get_mut(e.local_name().unwrap())
                .unwrap();
            if let Some(pos) = list.iter().position(|x| *x == e) {
                list.remove(pos);
            }
            self.exports_by_local_name
                .entry(e.local_name().unwrap().clone())
                .or_default()
                .push(mutated);
        }
    }
}

/// Collects all imports and exports of a module. Builds a [`UnresolvedEsModule`] by slotting the
/// exports into one of three categories:
///
/// - Local exports - exports whose original definition is in this file.
/// - Indirect exports - exports that come from another file. Either via `export {name} from` or
///   `import {name}; export {name};`
/// - Star exports - `export * from` exports
// port: EsModuleProcessor.UnresolvedModuleBuilder
struct UnresolvedModuleBuilder {
    path: ModulePath,
    root: NodeId,
    imports_by_local_name: IndexMap<JsString, Import>,
    exports: Vec<Export>,
    exported_names: IndexSet<JsString>,
}

impl UnresolvedModuleBuilder {
    // port: EsModuleProcessor.UnresolvedModuleBuilder#UnresolvedModuleBuilder
    fn new(path: ModulePath, root: NodeId) -> Self {
        Self {
            path,
            root,
            imports_by_local_name: IndexMap::new(),
            exports: Vec::new(),
            exported_names: IndexSet::new(),
        }
    }

    // port: EsModuleProcessor.UnresolvedModuleBuilder#add(Import)
    fn add_import(&mut self, i: Import) {
        self.imports_by_local_name.insert(i.local_name().clone(), i);
    }

    /// True if the export was successfully added or false if the exported name already exists.
    // port: EsModuleProcessor.UnresolvedModuleBuilder#add(Export)
    fn add_export(&mut self, e: Export) -> bool {
        match e.export_name() {
            None => {
                self.exports.push(e);
                true
            }
            Some(export_name) => {
                if self.exported_names.contains(export_name) {
                    return false;
                }
                let export_name = export_name.clone();
                self.exports.push(e);
                self.exported_names.insert(export_name);
                true
            }
        }
    }

    // port: EsModuleProcessor.UnresolvedModuleBuilder#build
    fn build(
        self,
        compiler: &mut AbstractCompiler,
        metadata: Arc<ModuleMetadata>,
    ) -> UnresolvedModule {
        let mut local_exports: Vec<Export> = Vec::new();
        let mut indirect_exports_builder: Vec<Export> = Vec::new();
        let mut star_exports_builder: Vec<Export> = Vec::new();

        for ee in &self.exports {
            if ee.module_request().is_none() {
                if ee
                    .local_name()
                    .is_some_and(|l| self.imports_by_local_name.contains_key(l))
                {
                    // import A from '';
                    // export { A };

                    // import * as ns from '';
                    // export { ns };
                    indirect_exports_builder.push(ee.clone());
                } else {
                    // var y;
                    // export { y };

                    // export var x;
                    local_exports.push(ee.clone());
                }
            } else if ee.import_name().is_some_and(|n| *n == "*") {
                // export * from '';
                star_exports_builder.push(ee.clone());
            } else {
                // export { A } from '';
                indirect_exports_builder.push(ee.clone());
            }
        }

        NodeTraversal::traverse(
            compiler,
            self.root,
            &mut FindMutableExports::new(&mut local_exports),
        );

        UnresolvedModule::Es(UnresolvedEsModule::new(
            metadata,
            self.path,
            self.imports_by_local_name,
            local_exports,
            indirect_exports_builder,
            star_exports_builder,
        ))
    }
}

/// The immutable fields of an [`UnresolvedEsModule`], shared so that its methods can read them
/// while other modules (or this one) are resolved through the arena.
struct EsModuleFields {
    metadata: Arc<ModuleMetadata>,
    path: ModulePath,
    imports_by_local_name: IndexMap<JsString, Import>,
    local_exports: Vec<Export>,
    indirect_exports: Vec<Export>,
    star_exports: Vec<Export>,
}

/// A module that has had its exports and imports parsed and categorized but has yet to
/// transitively resolve them.
// port: EsModuleProcessor.UnresolvedEsModule
pub struct UnresolvedEsModule {
    fields: Arc<EsModuleFields>,
    exported_names: Option<Arc<IndexSet<JsString>>>,
    resolved_imports: IndexMap<JsString, ResolveExportResult>,
    resolved_exports: IndexMap<JsString, ResolveExportResult>,
    resolved: Option<Arc<Module>>,
}

impl UnresolvedEsModule {
    // port: EsModuleProcessor.UnresolvedEsModule#UnresolvedEsModule
    fn new(
        metadata: Arc<ModuleMetadata>,
        path: ModulePath,
        imports_by_local_name: IndexMap<JsString, Import>,
        local_exports: Vec<Export>,
        indirect_exports: Vec<Export>,
        star_exports: Vec<Export>,
    ) -> Self {
        Self {
            fields: Arc::new(EsModuleFields {
                metadata,
                path,
                imports_by_local_name,
                local_exports,
                indirect_exports,
                star_exports,
            }),
            exported_names: None,
            resolved_imports: IndexMap::new(),
            resolved_exports: IndexMap::new(),
            resolved: None,
        }
    }

    /// The module behind `this` (Java's `this` inside the module's methods).
    fn this(resolver: &mut dyn ModuleRequestResolver, this: UnresolvedModuleId) -> &mut Self {
        match resolver.modules().get_mut(this) {
            UnresolvedModule::Es(m) => m,
            _ => unreachable!("not an UnresolvedEsModule"),
        }
    }

    fn fields(
        resolver: &mut dyn ModuleRequestResolver,
        this: UnresolvedModuleId,
    ) -> Arc<EsModuleFields> {
        Self::this(resolver, this).fields.clone()
    }

    // port: EsModuleProcessor.UnresolvedEsModule#reset
    pub(crate) fn reset(&mut self) {
        self.resolved = None;
        self.exported_names = None;
        self.resolved_imports.clear();
        self.resolved_exports.clear();
    }

    // port: EsModuleProcessor.UnresolvedEsModule#resolve
    pub(crate) fn resolve(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        _module_specifier: Option<&JsString>,
    ) -> Arc<Module> {
        if Self::this(module_request_resolver, this).resolved.is_none() {
            let f = Self::fields(module_request_resolver, this);
            // Every import creates a locally bound name.
            let mut bound_names: IndexMap<JsString, Binding> =
                Self::get_all_resolved_imports(this, compiler, module_request_resolver);

            let mut local_name_to_local_export: IndexMap<JsString, Export> = IndexMap::new();

            // Only local exports that are not an anonymous default export create local bindings.
            for e in &f.local_exports {
                local_name_to_local_export.insert(e.local_name().unwrap().clone(), e.clone());

                if *e.local_name().unwrap() != Export::DEFAULT_EXPORT_NAME {
                    let b = this.resolve_export(
                        compiler,
                        module_request_resolver,
                        e.export_name().unwrap(),
                    );
                    check_state!(b.resolved(), "Cannot have invalid missing own export!");
                    if !b.is_ambiguous() {
                        bound_names.insert(
                            e.local_name().unwrap().clone(),
                            b.get_binding().unwrap().clone(),
                        );
                    }
                }
            }

            // getAllResolvedExports is required to make a module but also performs cycle
            // checking.
            let namespace = Self::get_all_resolved_exports(this, compiler, module_request_resolver);
            Self::this(module_request_resolver, this).resolved = Some(Arc::new(
                Module::builder()
                    .bound_names(bound_names)
                    .namespace(namespace)
                    .metadata(f.metadata.clone())
                    .path(Some(f.path.clone()))
                    .local_name_to_local_export(local_name_to_local_export)
                    .build(),
            ));
        }
        Self::this(module_request_resolver, this)
            .resolved
            .clone()
            .unwrap()
    }

    // port: EsModuleProcessor.UnresolvedEsModule#metadata
    pub(crate) fn metadata(&self) -> Arc<ModuleMetadata> {
        self.fields.metadata.clone()
    }

    /// A map from import bound name to binding.
    // port: EsModuleProcessor.UnresolvedEsModule#getAllResolvedImports
    fn get_all_resolved_imports(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
    ) -> IndexMap<JsString, Binding> {
        let f = Self::fields(module_request_resolver, this);
        let mut imports = IndexMap::new();
        for name in f.imports_by_local_name.keys() {
            let b = Self::resolve_import(this, compiler, module_request_resolver, name);
            if b.resolved() {
                imports.insert(name.clone(), b.get_binding().unwrap().clone());
            }
        }
        imports
    }

    // port: EsModuleProcessor.UnresolvedEsModule#resolveImport(ModuleRequestResolver,String,Set,Set)
    fn resolve_import_with_sets(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        name: &JsString,
        resolve_set: &mut IndexSet<ExportTrace>,
        export_star_set: &mut IndexSet<UnresolvedModuleId>,
    ) -> ResolveExportResult {
        if let Some(b) = Self::this(module_request_resolver, this)
            .resolved_imports
            .get(name)
        {
            return b.clone();
        }
        let b = Self::resolve_import_impl(
            this,
            compiler,
            module_request_resolver,
            name,
            resolve_set,
            export_star_set,
        );
        Self::this(module_request_resolver, this)
            .resolved_imports
            .insert(name.clone(), b.clone());
        b
    }

    // port: EsModuleProcessor.UnresolvedEsModule#resolveImport(ModuleRequestResolver,String)
    fn resolve_import(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        name: &JsString,
    ) -> ResolveExportResult {
        Self::resolve_import_with_sets(
            this,
            compiler,
            module_request_resolver,
            name,
            &mut IndexSet::new(),
            &mut IndexSet::new(),
        )
    }

    // port: EsModuleProcessor.UnresolvedEsModule#resolveImportImpl
    fn resolve_import_impl(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        name: &JsString,
        resolve_set: &mut IndexSet<ExportTrace>,
        export_star_set: &mut IndexSet<UnresolvedModuleId>,
    ) -> ResolveExportResult {
        let f = Self::fields(module_request_resolver, this);
        let i = f.imports_by_local_name.get(name).unwrap();
        let Some(requested) = module_request_resolver.resolve_import(compiler, i) else {
            return ResolveExportResult::error();
        };
        let import_star = *i.import_name() == "*";
        let requested_metadata = requested.metadata(module_request_resolver);

        if import_star
            || (*i.import_name() == Export::DEFAULT
                && (requested_metadata.is_goog_provide() || requested_metadata.is_goog_module()))
        {
            if !GoogEsImports::is_goog_import_specifier(i.module_request())
                && (requested_metadata.is_goog_module() || requested_metadata.is_goog_provide())
            {
                let import_node = i.import_node();
                compiler.report(JSError::make_with_source_location(
                    &f.path.to_string(),
                    import_node.get_lineno(compiler),
                    import_node.get_charno(compiler),
                    &CANNOT_PATH_IMPORT_CLOSURE_FILE,
                    &[&i.local_name().to_string(), &i.module_request().to_string()],
                ));
                return ResolveExportResult::error();
            }

            if import_star && GoogEsImports::is_goog_import_specifier(i.module_request()) {
                let import_node = i.import_node();
                compiler.report(JSError::make_with_source_location(
                    &f.path.to_string(),
                    import_node.get_lineno(compiler),
                    import_node.get_charno(compiler),
                    &NAMESPACE_IMPORT_CANNOT_USE_STAR,
                    &[&i.local_name().to_string(), &i.module_request().to_string()],
                ));
                return ResolveExportResult::error();
            }

            let closure_namespace = if GoogEsImports::is_goog_import_specifier(i.module_request()) {
                Some(GoogEsImports::get_closure_id_from_goog_import_specifier(
                    i.module_request(),
                ))
            } else {
                None
            };
            ResolveExportResult::of(Binding::from_es_import_star(
                requested_metadata,
                closure_namespace,
                i.name_node(),
            ))
        } else {
            let result = requested.resolve_export_with_sets(
                compiler,
                module_request_resolver,
                Some(i.module_request()),
                i.import_name(),
                resolve_set,
                export_star_set,
            );
            if !result.found() && !result.had_error() {
                let import_node = i.import_node();
                compiler.report(JSError::make_with_source_location(
                    &f.path.to_string(),
                    import_node.get_lineno(compiler),
                    import_node.get_charno(compiler),
                    &DOES_NOT_HAVE_EXPORT,
                    &[&i.import_name().to_string()],
                ));
                return ResolveExportResult::error();
            } else if result.is_ambiguous() {
                let import_node = i.import_node();
                compiler.report(JSError::make_with_source_location(
                    &f.path.to_string(),
                    import_node.get_lineno(compiler),
                    import_node.get_charno(compiler),
                    &IMPORTED_AMBIGUOUS_EXPORT,
                    &[&i.import_name().to_string()],
                ));
                return ResolveExportResult::error();
            }
            // Java: i.nameNode() == null ? i.importNode() : i.nameNode(); an Import's nameNode
            // is never null.
            let for_source_info = i.name_node();
            result.copy(Some(for_source_info), CreatedBy::IMPORT)
        }
    }

    // port: EsModuleProcessor.UnresolvedEsModule#getExportedNames(ModuleRequestResolver)
    pub(crate) fn get_exported_names(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
    ) -> Arc<IndexSet<JsString>> {
        if Self::this(module_request_resolver, this)
            .exported_names
            .is_none()
        {
            let exported_names = Self::get_exported_names_visited(
                this,
                compiler,
                module_request_resolver,
                &mut IndexSet::new(),
            );
            Self::this(module_request_resolver, this).exported_names = Some(exported_names);
        }
        Self::this(module_request_resolver, this)
            .exported_names
            .clone()
            .unwrap()
    }

    // port: EsModuleProcessor.UnresolvedEsModule#getExportedNames(ModuleRequestResolver,Set)
    pub(crate) fn get_exported_names_visited(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        visited: &mut IndexSet<UnresolvedModuleId>,
    ) -> Arc<IndexSet<JsString>> {
        if visited.contains(&this) {
            // import * cycle
            return Arc::new(IndexSet::new());
        }

        visited.insert(this);
        let f = Self::fields(module_request_resolver, this);

        // exports are in Array.prototype.sort() order.
        let mut exported_names: BTreeSet<JsString> = BTreeSet::new();

        for e in &f.local_exports {
            exported_names.insert(e.export_name().unwrap().clone());
        }

        for e in &f.indirect_exports {
            exported_names.insert(e.export_name().unwrap().clone());
        }

        for e in &f.star_exports {
            let requested = module_request_resolver.resolve_export(compiler, e);
            if let Some(requested) = requested {
                if requested.metadata(module_request_resolver).is_es6_module() {
                    for n in requested
                        .get_exported_names_visited(compiler, module_request_resolver, visited)
                        .iter()
                    {
                        // Default exports are not exported with export *.
                        if *n != Export::DEFAULT && !exported_names.contains(n) {
                            exported_names.insert(n.clone());
                        }
                    }
                } else {
                    let error = JSError::make(
                        compiler,
                        e.export_node().unwrap(),
                        &transpilation_util::CANNOT_CONVERT_YET,
                        &["Wildcard export for non-ES module"],
                    );
                    compiler.report(error);
                }
            }
        }

        Arc::new(exported_names.into_iter().collect())
    }

    /// Map of exported name to binding.
    // port: EsModuleProcessor.UnresolvedEsModule#getAllResolvedExports
    fn get_all_resolved_exports(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
    ) -> IndexMap<JsString, Binding> {
        let mut exports = IndexMap::new();
        let names = Self::get_exported_names(this, compiler, module_request_resolver);
        for name in names.iter() {
            let b = this.resolve_export(compiler, module_request_resolver, name);
            check_state!(b.found(), "Cannot have invalid own export.");
            if b.resolved() {
                exports.insert(name.clone(), b.get_binding().unwrap().clone());
            } else if b.is_ambiguous() {
                let path = Self::fields(module_request_resolver, this).path.to_string();
                compiler.report(JSError::make_with_source_location(
                    &path,
                    -1,
                    -1,
                    &AMBIGUOUS_EXPORT_DEFINITION,
                    &[&name.to_string()],
                ));
            }
        }
        exports
    }

    // port: EsModuleProcessor.UnresolvedEsModule#resolveExport(ModuleRequestResolver,String,Set,Set)
    fn resolve_export_impl(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        export_name: &JsString,
        resolve_set: &mut IndexSet<ExportTrace>,
        export_star_set: &mut IndexSet<UnresolvedModuleId>,
    ) -> ResolveExportResult {
        if !Self::get_exported_names(this, compiler, module_request_resolver).contains(export_name)
        {
            return ResolveExportResult::not_found();
        }

        let f = Self::fields(module_request_resolver, this);

        if !resolve_set.insert(ExportTrace::create(this, export_name.clone())) {
            // Cycle!
            compiler.report(JSError::make_with_source_location(
                &f.path.to_string(),
                0,
                0,
                &CYCLIC_EXPORT_DEFINITION,
                &[&export_name.to_string()],
            ));
            return ResolveExportResult::error();
        }

        for e in &f.local_exports {
            if e.export_name() == Some(export_name) {
                let for_source_info = if e.name_node().is_some() {
                    e.name_node()
                } else {
                    e.export_node()
                };
                return ResolveExportResult::of(Binding::from_export(e.clone(), for_source_info));
            }
        }

        for e in &f.indirect_exports {
            if e.export_name() == Some(export_name) {
                if e.local_name()
                    .is_some_and(|l| f.imports_by_local_name.contains_key(l))
                {
                    // import whatever from 'mod';
                    // export { whatever };
                    return Self::resolve_import_with_sets(
                        this,
                        compiler,
                        module_request_resolver,
                        e.local_name().unwrap(),
                        resolve_set,
                        export_star_set,
                    )
                    .copy(e.name_node(), CreatedBy::EXPORT);
                } else {
                    let requested = module_request_resolver.resolve_export(compiler, e);
                    let Some(requested) = requested else {
                        return ResolveExportResult::error();
                    };
                    // export { whatever } from 'mod';
                    let result = requested.resolve_export_with_sets(
                        compiler,
                        module_request_resolver,
                        e.module_request(),
                        e.import_name().unwrap(),
                        resolve_set,
                        export_star_set,
                    );
                    if !result.found() && !result.had_error() {
                        let export_node = e.export_node().unwrap();
                        compiler.report(JSError::make_with_source_location(
                            &f.path.to_string(),
                            export_node.get_lineno(compiler),
                            export_node.get_charno(compiler),
                            &DOES_NOT_HAVE_EXPORT,
                            &[&e.import_name().unwrap().to_string()],
                        ));
                        return ResolveExportResult::error();
                    } else if result.is_ambiguous() {
                        let export_node = e.export_node().unwrap();
                        compiler.report(JSError::make_with_source_location(
                            &f.path.to_string(),
                            export_node.get_lineno(compiler),
                            export_node.get_charno(compiler),
                            &IMPORTED_AMBIGUOUS_EXPORT,
                            &[&e.import_name().unwrap().to_string()],
                        ));
                    }
                    return result.copy(e.name_node(), CreatedBy::EXPORT);
                }
            }
        }

        check_state!(
            *export_name != Export::DEFAULT,
            "Default export cannot come from export *."
        );

        if export_star_set.contains(&this) {
            // Cycle!
            compiler.report(JSError::make_with_source_location(
                &f.path.to_string(),
                -1,
                -1,
                &CYCLIC_EXPORT_DEFINITION,
                &[&export_name.to_string()],
            ));
            return ResolveExportResult::error();
        }

        export_star_set.insert(this);

        let mut star_resolution: Option<ResolveExportResult> = None;

        for e in &f.star_exports {
            let requested = module_request_resolver.resolve_export(compiler, e);
            let Some(requested) = requested else {
                return ResolveExportResult::error();
            };
            if requested
                .get_exported_names(compiler, module_request_resolver)
                .contains(export_name)
            {
                let resolution = requested.resolve_export_with_sets(
                    compiler,
                    module_request_resolver,
                    e.module_request(),
                    export_name,
                    resolve_set,
                    export_star_set,
                );
                if resolution.had_error() {
                    // Recursive case; error was reported on base case.
                    return resolution;
                } else if resolution.is_ambiguous() {
                    return resolution;
                } else {
                    match &star_resolution {
                        None => {
                            // First time finding something, not ambiguous.
                            star_resolution =
                                Some(resolution.copy(e.export_node(), CreatedBy::EXPORT));
                        }
                        Some(star_resolution) => {
                            // Second time finding something, might be ambiguous!
                            // Not ambiguous if it is the same export (same module and export
                            // name).
                            if !star_resolution.ptr_eq(&resolution) {
                                return ResolveExportResult::ambiguous();
                            }
                        }
                    }
                }
            }
        }

        match star_resolution {
            None => ResolveExportResult::error(),
            Some(star_resolution) => star_resolution,
        }
    }

    // port: EsModuleProcessor.UnresolvedEsModule#resolveExport(ModuleRequestResolver,String,String,Set,Set)
    pub(crate) fn resolve_export_cached(
        this: UnresolvedModuleId,
        compiler: &mut AbstractCompiler,
        module_request_resolver: &mut dyn ModuleRequestResolver,
        _module_specifier: Option<&JsString>,
        export_name: &JsString,
        resolve_set: &mut IndexSet<ExportTrace>,
        export_star_set: &mut IndexSet<UnresolvedModuleId>,
    ) -> ResolveExportResult {
        // Explicit containsKey check here since values can be null!
        if let Some(b) = Self::this(module_request_resolver, this)
            .resolved_exports
            .get(export_name)
        {
            return b.clone();
        }
        let b = Self::resolve_export_impl(
            this,
            compiler,
            module_request_resolver,
            export_name,
            resolve_set,
            export_star_set,
        );
        Self::this(module_request_resolver, this)
            .resolved_exports
            .insert(export_name.clone(), b.clone());
        b
    }
}

/// Collects information related to and resolves ES imports and exports.
#[derive(Default)]
pub struct EsModuleProcessor {
    current_module_builder: Option<UnresolvedModuleBuilder>,
    metadata: Option<Arc<ModuleMetadata>>,
}

impl ModuleProcessor for EsModuleProcessor {
    // port: EsModuleProcessor#process
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        metadata: Arc<ModuleMetadata>,
        path: Option<ModulePath>,
        script: Option<NodeId>,
    ) -> UnresolvedModule {
        let script = script.unwrap();
        self.metadata = Some(metadata.clone());
        self.current_module_builder = Some(UnresolvedModuleBuilder::new(path.unwrap(), script));
        NodeTraversal::traverse(compiler, script, self);
        let m = self
            .current_module_builder
            .take()
            .unwrap()
            .build(compiler, metadata);
        self.metadata = None;
        self.current_module_builder = None;
        m
    }
}

impl EsModuleProcessor {
    // port: EsModuleProcessor#EsModuleProcessor
    pub fn new() -> Self {
        Self::default()
    }

    fn builder(&mut self) -> &mut UnresolvedModuleBuilder {
        self.current_module_builder.as_mut().unwrap()
    }

    fn metadata(&self) -> Arc<ModuleMetadata> {
        self.metadata.clone().unwrap()
    }

    /// `t.getInput().getPath()`.
    fn input_path(t: &mut NodeTraversal<'_>) -> ModulePath {
        let input = t.get_input().unwrap().clone();
        input.get_path(t.get_compiler())
    }

    // port: EsModuleProcessor#visitExportAllFrom
    fn visit_export_all_from(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        // export * from '';
        let module_request = export.get_second_child(t).unwrap().get_string(t);
        let module_path = Self::input_path(t);
        let metadata = self.metadata();
        self.builder().add_export(
            Export::builder()
                .export_name(None)
                .module_request(Some(module_request))
                .import_name(Some(JsString::from("*")))
                .local_name(None)
                .module_path(Some(module_path))
                .export_node(Some(export))
                .module_metadata(metadata)
                .build(),
        );
    }

    // port: EsModuleProcessor#visitExportDefault
    fn visit_export_default(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        // export default <expression>;
        let child = export.get_first_child(t).unwrap();
        let mut name = JsString::from(Export::DEFAULT_EXPORT_NAME);

        if child.is_function(t) || child.is_class(t) {
            let maybe_name = NodeUtil::get_name(t, child);
            if let Some(maybe_name) = maybe_name.filter(|n| !n.is_empty()) {
                name = maybe_name;
            }
        }

        let module_path = Self::input_path(t);
        let metadata = self.metadata();
        if !self.builder().add_export(
            Export::builder()
                .export_name(Some(JsString::from(Export::DEFAULT)))
                .module_request(None)
                .import_name(None)
                .local_name(Some(name))
                .module_path(Some(module_path))
                .export_node(Some(export))
                .module_metadata(metadata)
                .build(),
        ) {
            t.report(export, &DUPLICATE_EXPORT, &[Export::DEFAULT]);
        }
    }

    // port: EsModuleProcessor#visitExportFrom
    fn visit_export_from(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        // export { Foo, Bar as Rab } from '';
        let mut child = export.get_first_first_child(t);
        while let Some(c) = child {
            let import_name = c.get_first_child(t).unwrap().get_string(t);
            let exported_name = c.get_last_child(t).unwrap().get_string(t);
            let module_request = export.get_second_child(t).unwrap().get_string(t);
            let module_path = Self::input_path(t);
            let name_node = c.get_first_child(t);
            let metadata = self.metadata();
            if !self.builder().add_export(
                Export::builder()
                    .export_name(Some(exported_name.clone()))
                    .module_request(Some(module_request))
                    .import_name(Some(import_name))
                    .local_name(None)
                    .module_path(Some(module_path))
                    .export_node(Some(export))
                    .name_node(name_node)
                    .module_metadata(metadata)
                    .build(),
            ) {
                t.report(export, &DUPLICATE_EXPORT, &[&exported_name.to_string()]);
            }
            child = c.get_next(t);
        }
    }

    // port: EsModuleProcessor#visitExportSpecs
    fn visit_export_specs(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        // export { Foo, Bar as Rab };
        let mut child = export.get_first_first_child(t);
        while let Some(c) = child {
            let local_name = c.get_first_child(t).unwrap().get_string(t);
            let exported_name = c.get_last_child(t).unwrap().get_string(t);
            let module_path = Self::input_path(t);
            let name_node = c.get_first_child(t);
            let metadata = self.metadata();
            if !self.builder().add_export(
                Export::builder()
                    .export_name(Some(exported_name.clone()))
                    .module_request(None)
                    .import_name(None)
                    .local_name(Some(local_name))
                    .module_path(Some(module_path))
                    .export_node(Some(export))
                    .name_node(name_node)
                    .module_metadata(metadata)
                    .build(),
            ) {
                t.report(export, &DUPLICATE_EXPORT, &[&exported_name.to_string()]);
            }
            child = c.get_next(t);
        }
    }

    // port: EsModuleProcessor#visitExportNameDeclaration
    fn visit_export_name_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        export: NodeId,
        declaration: NodeId,
    ) {
        //    export var Foo;
        //    export let {a, b:[c,d]} = {};
        // The consumer does not change the AST, so the LHS nodes are collected first and then
        // passed to addExportNameDeclaration in visiting order.
        let mut lhs_nodes: Vec<NodeId> = Vec::new();
        NodeUtil::visit_lhs_nodes_in_node(t.get_compiler(), declaration, &mut |_, lhs| {
            lhs_nodes.push(lhs)
        });
        for lhs in lhs_nodes {
            self.add_export_name_declaration(t, lhs, export);
        }
    }

    // port: EsModuleProcessor#addExportNameDeclaration
    fn add_export_name_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        lhs: NodeId,
        export: NodeId,
    ) {
        check_state!(lhs.is_name(t));
        let name = lhs.get_string(t);
        let module_path = Self::input_path(t);
        let metadata = self.metadata();
        if !self.builder().add_export(
            Export::builder()
                .export_name(Some(name.clone()))
                .module_request(None)
                .import_name(None)
                .local_name(Some(name.clone()))
                .module_path(Some(module_path))
                .export_node(Some(export))
                .name_node(Some(lhs))
                .module_metadata(metadata)
                .build(),
        ) {
            t.report(export, &DUPLICATE_EXPORT, &[&name.to_string()]);
        }
    }

    // port: EsModuleProcessor#visitExportFunctionOrClass
    fn visit_export_function_or_class(
        &mut self,
        t: &mut NodeTraversal<'_>,
        export: NodeId,
        declaration: NodeId,
    ) {
        // export function foo() {}
        // export class Foo {}
        check_state!(declaration.is_function(t) || declaration.is_class(t));
        let name_node = declaration.get_first_child(t).unwrap();
        let name = name_node.get_string(t);
        let module_path = Self::input_path(t);
        let metadata = self.metadata();
        if !self.builder().add_export(
            Export::builder()
                .export_name(Some(name.clone()))
                .module_request(None)
                .import_name(None)
                .local_name(Some(name.clone()))
                .module_path(Some(module_path))
                .export_node(Some(export))
                .name_node(Some(name_node))
                .module_metadata(metadata)
                .build(),
        ) {
            t.report(export, &DUPLICATE_EXPORT, &[&name.to_string()]);
        }
    }

    // port: EsModuleProcessor#visitExport
    fn visit_export(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        if export.get_boolean_prop(t, closure_rhino::node::Prop::EXPORT_ALL_FROM) {
            self.visit_export_all_from(t, export);
        } else if export.get_boolean_prop(t, closure_rhino::node::Prop::EXPORT_DEFAULT) {
            self.visit_export_default(t, export);
        } else if export.has_two_children(t) {
            self.visit_export_from(t, export);
        } else if export.get_first_child(t).unwrap().is_export_specs(t) {
            self.visit_export_specs(t, export);
        } else {
            let declaration = export.get_first_child(t).unwrap();
            if NodeUtil::is_name_declaration(t, Some(declaration)) {
                self.visit_export_name_declaration(t, export, declaration);
            } else {
                self.visit_export_function_or_class(t, export, declaration);
            }
        }
    }

    // port: EsModuleProcessor#visitImportDefault
    fn visit_import_default(
        &mut self,
        t: &mut NodeTraversal<'_>,
        import_node: NodeId,
        module_request: &JsString,
    ) {
        // import Default from '';
        let first = import_node.get_first_child(t).unwrap();
        let local_name = first.get_string(t);
        let module_path = Self::input_path(t);
        self.builder().add_import(
            Import::builder()
                .module_request(module_request)
                .import_name(Export::DEFAULT)
                .local_name(local_name)
                .module_path(module_path)
                .import_node(import_node)
                .name_node(first)
                .build(),
        );
    }

    // port: EsModuleProcessor#visitImportSpecs
    fn visit_import_specs(
        &mut self,
        t: &mut NodeTraversal<'_>,
        import_node: NodeId,
        module_request: &JsString,
    ) {
        // import { A, b as B } from '';
        let mut child = import_node.get_second_child(t).unwrap().get_first_child(t);
        while let Some(c) = child {
            let import_name = c.get_first_child(t).unwrap().get_string(t);
            let local_name = c.get_last_child(t).unwrap().get_string(t);
            let module_path = Self::input_path(t);
            let name_node = c.get_second_child(t).unwrap();
            self.builder().add_import(
                Import::builder()
                    .module_request(module_request)
                    .import_name(import_name)
                    .local_name(local_name)
                    .module_path(module_path)
                    .import_node(import_node)
                    .name_node(name_node)
                    .build(),
            );
            child = c.get_next(t);
        }
    }

    // port: EsModuleProcessor#visitImportStar
    fn visit_import_star(
        &mut self,
        t: &mut NodeTraversal<'_>,
        import_node: NodeId,
        module_request: &JsString,
    ) {
        // import * as ns from '';
        let second = import_node.get_second_child(t).unwrap();
        let local_name = second.get_string(t);
        let module_path = Self::input_path(t);
        self.builder().add_import(
            Import::builder()
                .module_request(module_request)
                .import_name("*")
                .local_name(local_name)
                .import_node(import_node)
                .module_path(module_path)
                .name_node(second)
                .build(),
        );
    }

    // port: EsModuleProcessor#visitImport
    fn visit_import(&mut self, t: &mut NodeTraversal<'_>, import_node: NodeId) {
        let module_request = import_node.get_last_child(t).unwrap().get_string(t);

        if import_node.get_first_child(t).unwrap().is_name(t) {
            self.visit_import_default(t, import_node, &module_request);
        }

        let second = import_node.get_second_child(t).unwrap();
        if second.is_import_specs(t) {
            self.visit_import_specs(t, import_node, &module_request);
        } else if second.is_import_star(t) {
            self.visit_import_star(t, import_node, &module_request);
        }
        // no entry for import '';
    }
}

impl Callback for EsModuleProcessor {
    // port: EsModuleProcessor#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        matches!(
            n.get_token(t),
            Token::ROOT | Token::SCRIPT | Token::MODULE_BODY | Token::EXPORT | Token::IMPORT
        )
    }

    // port: EsModuleProcessor#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::EXPORT => self.visit_export(t, n),
            Token::IMPORT => self.visit_import(t, n),
            _ => {}
        }
    }
}
