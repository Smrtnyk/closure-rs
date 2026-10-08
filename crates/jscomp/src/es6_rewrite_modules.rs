/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Es6RewriteModules.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Rewrites a ES6 module into a form that can be safely concatenated. Note that we treat a file as
//! an ES6 module if it has at least one import or export statement.
//!
//! Also rewrites any goog.{require,requireType,forwardDeclare,goog.module.get} calls that are
//! either in an ES module or of an ES module using goog.declareModuleId.
#![allow(clippy::needless_return, clippy::needless_late_init)] // Preserve Java statement order.
use crate::{
    abstract_compiler::AbstractCompiler,
    ast_factory::AstFactory,
    closure_primitive_errors::{
        INVALID_CLOSURE_CALL_SCOPE_ERROR, INVALID_DESTRUCTURING_FORWARD_DECLARE,
        INVALID_GET_NAMESPACE, INVALID_REQUIRE_NAMESPACE, MISSING_MODULE_OR_PROVIDE,
        MODULE_USES_GOOG_MODULE_GET,
    },
    closure_rewrite_module::{ClosureRewriteModule, ILLEGAL_MODULE_RENAMING_CONFLICT},
    compiler_options::ChunkOutputType,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    es6_rename_type_references::{Es6RenameTypeReferences, RenameTable},
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    js_error::JSError,
    module_renaming::{GlobalTypedScope, GlobalizedModuleName, ModuleRenaming},
    modules::{
        binding::Binding,
        module::Module,
        module_map::ModuleMap,
        module_metadata_map::{ModuleMetadata, ModuleMetadataMap, ModuleType},
    },
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    preprocessor_symbol_table::PreprocessorSymbolTable,
    transpilation_util,
    typed_scope::TypedScope,
};
use closure_jstype::{TypeId, js_type_native::JSTypeNative, static_typed_scope::StaticTypedScope};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::{Arc, LazyLock, Mutex};

// port: Es6RewriteModules#LHS_OF_GOOG_REQUIRE_MUST_BE_CONST
pub static LHS_OF_GOOG_REQUIRE_MUST_BE_CONST: DiagnosticType = DiagnosticType::error(
    "JSC_LHS_OF_GOOG_REQUIRE_MUST_BE_CONST",
    "The left side of a goog.require() or goog.requireType() must use ''const'' (not ''let'' or ''var'')",
);

// port: Es6RewriteModules#REQUIRE_TYPE_FOR_ES6_SHOULD_BE_CONST
pub static REQUIRE_TYPE_FOR_ES6_SHOULD_BE_CONST: DiagnosticType = DiagnosticType::error(
    "JSC_REQUIRE_TYPE_FOR_ES6_SHOULD_BE_CONST",
    "goog.requireType alias for ES6 module should be const.",
);

// port: Es6RewriteModules#FORWARD_DECLARE_FOR_ES6_SHOULD_BE_CONST
pub static FORWARD_DECLARE_FOR_ES6_SHOULD_BE_CONST: DiagnosticType = DiagnosticType::error(
    "JSC_FORWARD_DECLARE_FOR_ES6_SHOULD_BE_CONST",
    "goog.forwardDeclare alias for ES6 module should be const.",
);

// port: Es6RewriteModules#SHOULD_IMPORT_ES6_MODULE
pub static SHOULD_IMPORT_ES6_MODULE: DiagnosticType = DiagnosticType::warning(
    "JSC_SHOULD_IMPORT_ES6_MODULE",
    "ES6 modules should import other ES6 modules rather than goog.require them.",
);

// port: Es6RewriteModules#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: Es6RewriteModules#GOOG_REQUIRETYPE
static GOOG_REQUIRETYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
// port: Es6RewriteModules#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));
// port: Es6RewriteModules#GOOG_FORWARDDECLARE
static GOOG_FORWARDDECLARE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.forwardDeclare"));
// port: Es6RewriteModules#GOOG_DECLAREMODULEID
static GOOG_DECLAREMODULEID: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.declareModuleId"));

/// Rust-only: Guava `Splitter.on(".")` (`DOT_SPLITTER`), with `limit(n)` when `limit > 0`, on
/// UTF-16 code units.
fn dot_splitter_split_to_list(name: &JsString, limit: usize) -> Vec<JsString> {
    let mut result = Vec::new();
    let mut start = 0usize;
    loop {
        if limit > 0 && result.len() + 1 == limit {
            result.push(name.substring_from(start));
            return result;
        }
        let index = name.index_of_from(&JsString::from("."), start as i32);
        if index < 0 {
            result.push(name.substring_from(start));
            return result;
        }
        result.push(name.substring(start, index as usize));
        start = index as usize + 1;
    }
}

/// Rust-only: Java's `"" + s` for a nullable string.
fn string_value_of(s: Option<&str>) -> &str {
    s.unwrap_or("null")
}

/// Rewrites a ES6 module into a form that can be safely concatenated.
// port: Es6RewriteModules
pub struct Es6RewriteModules {
    ast_factory: AstFactory,
    unknown_type: TypeId,

    preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,

    /// Local variable names that were goog.require'd to qualified name we need to line.
    ///
    /// We need to inline all required names since there are certain well-known Closure symbols
    /// (like goog.asserts) that later stages of the compiler check for and cannot handle aliases.
    // TODO(johnplaisted): This is actually incorrect if the require'd thing is mutated. But we need
    // it so that things like goog.asserts work. Mutated closure symbols are a lot rarer than
    // needing to use asserts and the like. Until there's a better solution to finding aliases of
    // well known symbols we have to inline anything that is require'd.
    names_to_inline_by_alias: IndexMap<JsString, GlobalizedModuleName>,

    typedefs: IndexSet<JsString>,

    module_metadata_map: Arc<ModuleMetadataMap>,
    module_map: Option<Arc<ModuleMap>>,
    global_typed_scope: Option<TypedScope>,
    chunk_output_type: ChunkOutputType,
}

impl Es6RewriteModules {
    /// Creates a new Es6RewriteModules instance which can be used to rewrite ES6 modules to a
    /// concatenable form.
    // port: Es6RewriteModules#Es6RewriteModules(AbstractCompiler,ModuleMetadataMap,ModuleMap,PreprocessorSymbolTable,TypedScope)
    pub fn new(
        compiler: &mut AbstractCompiler,
        module_metadata_map: Option<Arc<ModuleMetadataMap>>,
        module_map: Option<Arc<ModuleMap>>,
        preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
        global_typed_scope: Option<TypedScope>,
    ) -> Self {
        Self::new_with_chunk_output_type(
            compiler,
            module_metadata_map,
            module_map,
            preprocessor_symbol_table,
            global_typed_scope,
            ChunkOutputType::GLOBAL_NAMESPACE,
        )
    }

    /// Creates a new Es6RewriteModules instance which can be used to rewrite ES6 modules to a
    /// concatenable form.
    // port: Es6RewriteModules#Es6RewriteModules(AbstractCompiler,ModuleMetadataMap,ModuleMap,PreprocessorSymbolTable,TypedScope,ChunkOutputType)
    pub fn new_with_chunk_output_type(
        compiler: &mut AbstractCompiler,
        module_metadata_map: Option<Arc<ModuleMetadataMap>>,
        module_map: Option<Arc<ModuleMap>>,
        preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
        global_typed_scope: Option<TypedScope>,
        chunk_output_type: ChunkOutputType,
    ) -> Self {
        let module_metadata_map = check_not_null!(module_metadata_map);
        let ast_factory = compiler.create_ast_factory();
        let unknown_type = compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::UNKNOWN_TYPE);
        Self {
            ast_factory,
            unknown_type,
            preprocessor_symbol_table,
            names_to_inline_by_alias: IndexMap::new(),
            typedefs: IndexSet::new(),
            module_metadata_map,
            module_map,
            global_typed_scope,
            chunk_output_type,
        }
    }

    /// Return whether or not the given script node represents an ES6 module file.
    // port: Es6RewriteModules#isEs6ModuleRoot
    pub fn is_es6_module_root(ast: &Ast, script_node: NodeId) -> bool {
        check_argument!(script_node.is_script(ast), "%s", script_node.to_string(ast));
        if script_node.get_boolean_prop(ast, NodeId::GOOG_MODULE) {
            return false;
        }
        script_node.has_children(ast)
            && script_node
                .get_first_child(ast)
                .unwrap()
                .is_module_body(ast)
    }

    // port: Es6RewriteModules#clearPerFileState
    fn clear_per_file_state(&mut self) {
        self.typedefs = IndexSet::new();
        self.names_to_inline_by_alias = IndexMap::new();
    }

    /// Rust-only: Java's `moduleMap.getModule(ModulePath)` on the non-null module map.
    fn get_module(&self, path: &crate::deps::module_loader::ModulePath) -> Option<Arc<Module>> {
        self.module_map
            .as_ref()
            .expect("java.lang.NullPointerException")
            .get_module_by_path(path)
            .cloned()
    }

    // port: Es6RewriteModules#maybeWarnExternModule
    fn maybe_warn_extern_module(&self, t: &mut NodeTraversal<'_>, n: NodeId, parent: NodeId) {
        check_state!(parent.is_module_body(t));
        if parent.is_from_externs(t)
            && !NodeUtil::is_from_type_summary(t, parent.get_parent(t).unwrap())
        {
            t.report(
                n,
                &transpilation_util::CANNOT_CONVERT_YET,
                &["ES6 modules in externs"],
            );
        }
    }

    // port: Es6RewriteModules#visitImport
    fn visit_import(&mut self, t: &mut NodeTraversal<'_>, import_decl: NodeId, parent: NodeId) {
        check_argument!(parent.is_module_body(t), "%s", parent.to_string(t));
        let import_name = import_decl.get_last_child(t).unwrap().get_string(t);
        let is_namespace_import = import_name.starts_with(&JsString::from("goog:"));
        if is_namespace_import {
            // Allow importing Closure namespace objects (e.g. from goog.provide or goog.module) as
            //   import ... from 'goog:my.ns.Object'.
            let namespace = import_name.substring_from("goog:".len());
            let m = self
                .module_metadata_map
                .get_modules_by_goog_namespace()
                .get(&namespace)
                .cloned();

            match m {
                None => {
                    t.report(
                        import_decl,
                        &MISSING_MODULE_OR_PROVIDE,
                        &[&namespace.to_string_lossy()],
                    );
                }
                Some(m) => {
                    check_state!(m.is_es6_module() || m.is_goog_module() || m.is_goog_provide());
                }
            }
        } else {
            let input = t.get_input().unwrap().clone();
            let path = input.get_path(t.get_compiler());
            let import_name_str = import_name.to_string_lossy();
            let source_file_name = import_decl.get_source_file_name(t);
            let lineno = import_decl.get_lineno(t);
            let charno = import_decl.get_charno(t);
            let mut module_path = path.resolve_js_module(
                &import_name_str,
                source_file_name.as_deref(),
                lineno,
                charno,
            );
            // The module loader reported to the compiler (its ErrorHandler) during the resolution.
            t.get_compiler().flush_module_errors();
            if module_path.is_none() {
                // The module loader issues an error
                // Fall back to assuming the module is a file path
                module_path = Some(path.resolve_module_as_path(&import_name_str));
            }

            let last_child = import_decl.get_last_child(t).unwrap();
            self.maybe_add_imported_file_reference_to_symbol_table(
                t.get_compiler(),
                last_child,
                &module_path.unwrap().to_string(),
            );
            // TODO(johnplaisted): Use ModuleMetadata to ensure the path required is CommonJs or
            // ES6 and if not give a better error.
        }

        let mut child = import_decl.get_first_child(t);
        while let Some(c) = child {
            if c.is_import_specs(t) {
                let mut grand_child = c.get_first_child(t);
                while let Some(gc) = grand_child {
                    let source_name = t.get_source_name();
                    let first = gc.get_first_child(t).unwrap();
                    self.maybe_add_alias_to_symbol_table(t, first, source_name.as_deref());
                    check_state!(gc.has_two_children(t));
                    grand_child = gc.get_next(t);
                }
            } else if c.is_import_star(t) {
                // import * as ns from "mod"
                let source_name = t.get_source_name();
                self.maybe_add_alias_to_symbol_table(t, c, source_name.as_deref());
            }
            child = c.get_next(t);
        }

        import_decl.detach(t);
        t.report_code_change();
    }

    // port: Es6RewriteModules#visitExport
    fn visit_export(&mut self, t: &mut NodeTraversal<'_>, export: NodeId, parent: NodeId) {
        check_argument!(parent.is_module_body(t), "%s", parent.to_string(t));
        if export.get_boolean_prop(t, NodeId::EXPORT_DEFAULT) {
            // export default

            // If the thing being exported is a class or function that has a name,
            // extract it from the export statement, so that it can be referenced
            // from within the module.
            //
            //   export default class X {} -> class X {}; ... moduleName.default = X;
            //   export default function X() {} -> function X() {}; ... moduleName.default = X;
            //
            // Otherwise, create a local variable for it and export that.
            //
            //   export default 'someExpression'
            //     ->
            //   var $jscompDefaultExport = 'someExpression';
            //   ...
            //   moduleName.default = $jscompDefaultExport;
            let child = export.get_first_child(t).unwrap();
            let mut name = None;

            if child.is_function(t) || child.is_class(t) {
                name = NodeUtil::get_name(t, child);
            }

            if name.is_some() {
                let decl = child.detach(t);
                export.replace_with(t, decl);
            } else {
                let removed = export.remove_first_child(t).unwrap();
                let var = self
                    .ast_factory
                    .create_single_var_name_declaration_with_value(
                        t.get_compiler(),
                        ModuleRenaming::DEFAULT_EXPORT_VAR_PREFIX,
                        removed,
                    );
                let info = child.get_jsdoc_info(t);
                var.set_jsdoc_info(t, info);
                child.set_jsdoc_info(t, None);
                var.srcref_tree_if_missing(t, export);
                export.replace_with(t, var);
            }
            t.report_code_change();
        } else if export.get_boolean_prop(t, NodeId::EXPORT_ALL_FROM)
            || export.has_two_children(t)
            || export.get_first_child(t).unwrap().is_export_specs(t)
        {
            //   export * from 'moduleIdentifier';
            //   export {x, y as z} from 'moduleIdentifier';
            //   export {Foo};
            export.detach(t);
            t.report_code_change();
        } else {
            self.visit_export_declaration(t, export);
        }
    }

    // port: Es6RewriteModules#visitExportNameDeclaration
    fn visit_export_name_declaration(&mut self, ast: &mut Ast, declaration: NodeId) {
        if declaration
            .get_jsdoc_info(ast)
            .is_some_and(|info| info.has_typedef_type())
        {
            //    export var Foo;
            //    export let {a, b:[c,d]} = {};
            let typedefs = &mut self.typedefs;
            NodeUtil::visit_lhs_nodes_in_node(ast, declaration, &mut |ast: &mut Ast, lhs| {
                typedefs.insert(lhs.get_string(ast));
            });
        }
    }

    // port: Es6RewriteModules#visitExportDeclaration
    fn visit_export_declaration(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        //    export var Foo;
        //    export function Foo() {}
        // etc.
        let declaration = export.get_first_child(t).unwrap();

        if NodeUtil::is_name_declaration(t, Some(declaration)) {
            self.visit_export_name_declaration(t, declaration);
        }

        let detached = declaration.detach(t);
        export.replace_with(t, detached);
        t.report_code_change();
    }

    // port: Es6RewriteModules#inlineModuleToGlobalScope
    fn inline_module_to_global_scope(&self, ast: &mut Ast, module_node: NodeId) {
        check_state!(module_node.is_module_body(ast));
        let script_node = module_node.get_parent(ast).unwrap();
        module_node.detach(ast);
        let children = module_node.remove_children(ast);
        script_node.add_children_to_front(ast, children);
    }

    // port: Es6RewriteModules#visitScript
    fn visit_script(&mut self, t: &mut NodeTraversal<'_>, script: NodeId) {
        let module_body = script.get_first_child(t).unwrap();
        // TypedScopeCreator sets the module object type on the MODULE_BODY during type checking.
        let module_object_type = AstFactory::type_node(module_body);
        self.inline_module_to_global_scope(t, module_body);

        ClosureRewriteModule::check_and_set_strict_mode_directive(t, script);

        let input = t.get_input().unwrap().clone();
        let path = input.get_path(t.get_compiler());
        let this_module = self
            .get_module(&path)
            .expect("java.lang.NullPointerException");
        let qualified_name = ModuleRenaming::get_global_name(
            t,
            this_module.metadata(),
            /* googNamespace= */ None,
        );
        check_state!(
            qualified_name.is_simple(t),
            "Unexpected qualified name %s",
            qualified_name.join(t).to_string_lossy()
        );
        let module_name = qualified_name.get_root(t);

        let module_var = self.create_exports_object(&module_name, t, script, module_object_type);

        // Rename vars to not conflict in global scope.
        NodeTraversal::traverse(
            t.get_compiler(),
            script,
            &mut RenameGlobalVars {
                outer: self,
                this_module: this_module.clone(),
            },
        );

        // Rename the exports object to something we can reference later.
        let first = module_var.get_first_child(t).unwrap();
        first.set_string(t, module_name);
        module_var.make_non_indexable_recursive(t);
        self.declare_global_variable(first, t);

        // rewriteRequires is here (rather than being part of the main visit() method, because we
        // only want to rewrite the requires if this is an ES6 module. Note that we also want to do
        // this AFTER renaming all module scoped vars in the event that something that is
        // goog.require'd is a global, unqualified name (e.g. if "goog.provide('foo')" exists, we
        // don't want to rewrite "const foo = goog.require('foo')" to "const foo = foo". If we
        // rewrite our module scoped names first then we'll rewrite to
        // "const foo$module$fudge = goog.require('foo')", then to "const foo$module$fudge = foo".
        self.rewrite_requires(t.get_compiler(), script);

        t.report_code_change();
    }

    // port: Es6RewriteModules#createExportsObject
    fn create_exports_object(
        &mut self,
        module_name: &JsString,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        module_object_type: crate::ast_factory::Type,
    ) -> NodeId {
        let module_object = self.ast_factory.create_object_lit_with_type(
            t.get_compiler(),
            module_object_type.clone(),
            &[],
        );
        // Going to get renamed by RenameGlobalVars, so the name we choose here doesn't matter as
        // long as it doesn't collide with an existing variable. (We can't use `moduleName` since
        // then RenameGlobalVars will rename all references to `moduleName` incorrectly). We'll fix
        // the name in visitScript after the global renaming to ensure it has a name that is
        // deterministic from the path.
        //
        // So after this method we'll have:
        // var $jscomp$tmp$exports$module$name = {};
        // module$name.exportName = localName;
        //
        // After RenameGlobalVars:
        // var $jscomp$tmp$exports$module$nameglobalized = {};
        // module$name.exportName = localName$globalized;
        //
        // After visitScript:
        // var module$name = {};
        // module$name.exportName = localName$globalized;
        let module_var = self
            .ast_factory
            .create_single_var_name_declaration_with_value(
                t.get_compiler(),
                "$jscomp$tmp$exports$module$name",
                module_object,
            );
        let first = module_var.get_first_child(t).unwrap();
        first.put_boolean_prop(t, NodeId::MODULE_EXPORT, true);
        // TODO(b/144593112): Stop adding JSDoc when this pass moves to always be after
        // typechecking.
        let mut info_builder = JSDocInfo::builder();
        info_builder.record_constancy();
        module_var.set_jsdoc_info(t, info_builder.build());
        first.set_declared_constant_var(t, true);
        let module_var_srcref = module_var.srcref_tree_if_missing(t, script);
        script.add_child_to_back(t, module_var_srcref);

        let input = t.get_input().unwrap().clone();
        let path = input.get_path(t.get_compiler());
        let this_module = self
            .get_module(&path)
            .expect("java.lang.NullPointerException");

        for (exported_name, binding) in this_module.namespace() {
            let node_for_source_info = binding.source_node();
            let mutated = binding.is_mutated();
            let bound_variable_qualified_name =
                ModuleRenaming::get_global_name_of_binding(t, binding);
            check_state!(
                bound_variable_qualified_name.is_simple(t),
                "unexpected qualified name: %s",
                bound_variable_qualified_name.join(t).to_string_lossy()
            );
            let bound_variable_name = bound_variable_qualified_name.get_root(t);

            let name = self.ast_factory.create_name(
                t.get_compiler(),
                module_name.clone(),
                module_object_type.clone(),
            );
            let get_prop = self.ast_factory.create_get_prop_without_color(
                t.get_compiler(),
                name,
                exported_name.clone(),
            );
            get_prop.put_boolean_prop(t, NodeId::MODULE_EXPORT, true);

            if self.typedefs.contains(exported_name) {
                // /** @typedef {foo} */
                // moduleName.foo;
                let mut builder = JSDocInfo::builder();
                builder.parse_documentation();
                let string = self
                    .ast_factory
                    .create_string(t.get_compiler(), exported_name.clone())
                    .srcref(t, node_for_source_info.unwrap());
                let type_expr = JSTypeExpression::new(
                    string,
                    string_value_of(script.get_source_file_name(t).as_deref()),
                );
                builder.record_typedef(Some(Arc::new(type_expr)));
                let info = builder.build();
                get_prop.set_jsdoc_info(t, info);
                let expr_result = self
                    .ast_factory
                    .expr_result(t.get_compiler(), get_prop)
                    .srcref_tree_if_missing(t, node_for_source_info.unwrap());
                script.add_child_to_back(t, expr_result);
            } else if mutated {
                let global_export_name = self.ast_factory.create_name(
                    t.get_compiler(),
                    bound_variable_name,
                    AstFactory::type_node(get_prop),
                );
                self.add_getter_export(
                    t.get_compiler(),
                    script,
                    node_for_source_info.unwrap(),
                    module_object,
                    exported_name,
                    global_export_name,
                );
                let current_script = t.get_current_script().unwrap();
                NodeUtil::add_feature_to_script(t.get_compiler(), current_script, Feature::GETTER);
            } else {
                // Avoid the extra complexity of using getters when the property isn't mutated.
                // exports.foo = foo;
                let rhs = self.ast_factory.create_name(
                    t.get_compiler(),
                    bound_variable_name,
                    AstFactory::type_node(get_prop),
                );
                let assign = self
                    .ast_factory
                    .create_assign(t.get_compiler(), get_prop, rhs);
                // TODO(b/144593112): Stop adding JSDoc when this pass moves to always be after
                // typechecking
                let mut builder = JSDocInfo::builder();
                builder.parse_documentation();
                builder.record_constancy();
                let info = builder.build();
                assign.set_jsdoc_info(t, info);
                let expr_result = self
                    .ast_factory
                    .expr_result(t.get_compiler(), assign)
                    .srcref_tree_if_missing(t, node_for_source_info.unwrap());
                script.add_child_to_back(t, expr_result);
            }
        }

        module_var
    }

    // port: Es6RewriteModules#addGetterExport
    fn add_getter_export(
        &self,
        compiler: &mut AbstractCompiler,
        script: NodeId,
        for_source_info: NodeId,
        obj_lit: NodeId,
        exported_name: &JsString,
        value: NodeId,
    ) {
        let getter = self
            .ast_factory
            .create_getter_def(compiler, exported_name.clone(), value);
        getter.put_boolean_prop(compiler, NodeId::MODULE_EXPORT, true);
        obj_lit.add_child_to_back(compiler, getter);

        if !self.ast_factory.is_adding_types() {
            // TODO(b/143904518): Remove this code when this pass is permanently moved after type
            // checking
            // Type checker doesn't infer getters so mark the return as unknown.
            // { /** @return {?} */ get foo() { return foo; } }
            let mut builder = JSDocInfo::builder();
            builder.parse_documentation();
            let qmark = compiler
                .new_node(Token::QMARK)
                .srcref(compiler, for_source_info);
            builder.record_return_type(Some(Arc::new(JSTypeExpression::new(
                qmark,
                string_value_of(script.get_source_file_name(compiler).as_deref()),
            ))));
            getter.set_jsdoc_info(compiler, builder.build());
        } else {
            // For a property typed as number, synthesize a type `function(): number`.
            let value_type = value.get_jstype(compiler);
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let function_type =
                registry.create_function_type(ast, check_not_null!(value_type), &[]);
            getter.set_jstype(compiler, Some(function_type));
        }

        getter.srcref_tree_if_missing(compiler, for_source_info);
        let last = getter
            .get_first_child(compiler)
            .unwrap()
            .get_last_child(compiler)
            .unwrap();
        compiler.report_change_to_enclosing_scope(last);
        compiler.report_change_to_enclosing_scope(getter);
    }

    // port: Es6RewriteModules#rewriteRequires
    fn rewrite_requires(&mut self, compiler: &mut AbstractCompiler, script: NodeId) {
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback_post_order(
                |t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>| {
                    if n.is_call(t) {
                        let fn_ = n.get_first_child(t).unwrap();
                        if GOOG_REQUIRE.matches(t, fn_) || GOOG_REQUIRETYPE.matches(t, fn_) {
                            // TODO(tjgq): This will rewrite both type references and code
                            // references. For goog.requireType, the latter are potentially broken
                            // because the symbols aren't guaranteed to be available at run time.
                            // A separate pass needs to be added to detect these incorrect uses of
                            // goog.requireType.
                            self.visit_require_or_get(
                                t,
                                n,
                                parent.unwrap(),
                                /* isRequire= */ true,
                            );
                        } else if GOOG_MODULE_GET.matches(t, fn_) {
                            self.visit_goog_module_get(t, n, parent.unwrap());
                        }
                    }
                },
            )
            .traverse(script);
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback_post_order(
                |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| {
                    let info = n.get_jsdoc_info(t);
                    if let Some(info) = info {
                        for type_node in info.get_type_nodes() {
                            self.inline_aliased_types(t, type_node);
                        }
                    }

                    if n.is_name(t) && self.names_to_inline_by_alias.contains_key(&n.get_string(t))
                    {
                        let scope = t.get_scope();
                        let name = n.get_string(t);
                        let v = scope.get_var(t.get_compiler(), name.clone());
                        if v.is_none_or(|v| v.get_name_node(t.get_compiler()) != Some(n)) {
                            let replacement_name =
                                self.names_to_inline_by_alias.get(&name).unwrap().clone();
                            let replacement = replacement_name
                                .to_qname(t.get_compiler(), &self.ast_factory)
                                .srcref_tree(t, n);
                            n.replace_with(t, replacement);
                        }
                    }
                },
            )
            .traverse(script);
    }

    // port: Es6RewriteModules#inlineAliasedTypes
    fn inline_aliased_types(&self, t: &mut NodeTraversal<'_>, type_node: NodeId) {
        if type_node.is_string_lit(t) {
            let name = type_node.get_string(t);
            let split = dot_splitter_split_to_list(&name, 2);

            // We've already removed the alias.
            let scope = t.get_scope();
            if scope.get_var(t.get_compiler(), split[0].clone()).is_none() {
                let replacement = self.names_to_inline_by_alias.get(&split[0]);
                if let Some(replacement) = replacement {
                    let mut rest = JsString::from("");
                    if split.len() == 2 {
                        rest = JsString::from(".").concat(&split[1]);
                    }
                    type_node.set_original_name(t, Some(name));
                    let new_name = replacement.alias_name().join(t).concat(&rest);
                    type_node.set_string(t, new_name);
                    t.report_code_change();
                }
            }
        }
        let mut child = type_node.get_first_child(t);
        while let Some(c) = child {
            self.inline_aliased_types(t, c);
            child = c.get_next(t);
        }
    }

    // port: Es6RewriteModules#visitGoogModuleGet
    fn visit_goog_module_get(
        &mut self,
        t: &mut NodeTraversal<'_>,
        get_call: NodeId,
        parent: NodeId,
    ) {
        if !get_call.has_two_children(t) || !get_call.get_last_child(t).unwrap().is_string_lit(t) {
            t.report(get_call, &INVALID_GET_NAMESPACE, &[]);
            return;
        }

        // Module has already been turned into a script at this point.
        if t.in_global_hoist_scope() {
            t.report(get_call, &MODULE_USES_GOOG_MODULE_GET, &[]);
            return;
        }

        self.visit_require_or_get(t, get_call, parent, /* isRequire= */ false);
    }

    /// Gets some made-up metadata for the given Closure namespace.
    ///
    /// This is used when the namespace is not part of the input so that this pass can be fault
    /// tolerant and still rewrite to something. Some tools don't care about rewriting correctly
    /// and just want the type information of this module (e.g. clutz).
    // port: Es6RewriteModules#getFallbackMetadataForNamespace
    fn get_fallback_metadata_for_namespace(&self, namespace: &JsString) -> Arc<ModuleMetadata> {
        // Assume a provide'd file to be consistent with goog.module rewriting.
        let mut builder = ModuleMetadata::builder();
        builder
            .module_type(ModuleType::GOOG_PROVIDE)
            .uses_closure(true)
            .is_test_only(false);
        builder.goog_namespaces_builder().add(namespace.clone());
        builder.build()
    }

    // port: Es6RewriteModules#visitRequireOrGet
    fn visit_require_or_get(
        &mut self,
        t: &mut NodeTraversal<'_>,
        require_call: NodeId,
        parent: NodeId,
        is_require: bool,
    ) {
        if !require_call.has_two_children(t)
            || !require_call.get_last_child(t).unwrap().is_string_lit(t)
        {
            t.report(require_call, &INVALID_REQUIRE_NAMESPACE, &[]);
            return;
        }

        // Module has already been turned into a script at this point.
        if is_require && !t.get_scope().is_global(t.get_compiler()) {
            t.report(require_call, &INVALID_CLOSURE_CALL_SCOPE_ERROR, &[]);
            return;
        }

        let namespace = require_call.get_last_child(t).unwrap().get_string(t);

        let grandparent = parent.get_parent(t).unwrap();
        let is_stored_in_declaration = NodeUtil::is_declaration(t, grandparent);

        if is_stored_in_declaration && !grandparent.is_const(t) {
            let error = JSError::make(t, grandparent, &LHS_OF_GOOG_REQUIRE_MUST_BE_CONST, &[]);
            t.get_compiler().report(error);
        }

        let mut m = self
            .module_metadata_map
            .get_modules_by_goog_namespace()
            .get(&namespace)
            .cloned();

        let is_from_fallback_metadata = m.is_none();
        if is_from_fallback_metadata {
            t.report(
                require_call,
                &MISSING_MODULE_OR_PROVIDE,
                &[&namespace.to_string_lossy()],
            );
            m = Some(self.get_fallback_metadata_for_namespace(&namespace));
        }
        let m = m.unwrap();

        if is_stored_in_declaration {
            if is_require {
                let to_detach;

                if parent.is_destructuring_lhs(t) {
                    check_state!(parent.get_first_child(t).unwrap().is_object_pattern(t));
                    to_detach = grandparent;
                    let mut child = parent.get_first_first_child(t);
                    while let Some(c) = child {
                        check_state!(
                            c.is_string_key(t) && c.get_first_child(t).unwrap().is_name(t),
                            "%s",
                            c.to_string(t)
                        );
                        let rep = self
                            .get_global_name_and_type(
                                t.get_compiler(),
                                &m,
                                Some(&namespace),
                                is_from_fallback_metadata,
                            )
                            .getprop(&c.get_string(t));
                        let alias = c.get_first_child(t).unwrap().get_string(t);
                        self.names_to_inline_by_alias.insert(alias, rep);
                        child = c.get_next(t);
                    }
                } else if parent.is_name(t) {
                    let alias = self.get_global_name_and_type(
                        t.get_compiler(),
                        &m,
                        Some(&namespace),
                        is_from_fallback_metadata,
                    );
                    self.names_to_inline_by_alias
                        .insert(parent.get_string(t), alias);

                    to_detach = grandparent;
                } else {
                    check_state!(parent.is_expr_result(t));
                    to_detach = parent;
                }
                to_detach.detach(t);
            } else {
                let name = self.get_global_name_and_type(
                    t.get_compiler(),
                    &m,
                    Some(&namespace),
                    is_from_fallback_metadata,
                );
                let replacement = name
                    .to_qname(t.get_compiler(), &self.ast_factory)
                    .srcref_tree(t, require_call);
                require_call.replace_with(t, replacement);
            }
        } else {
            check_state!(require_call.get_parent(t).unwrap().is_expr_result(t));
            require_call.get_parent(t).unwrap().detach(t);
        }
    }

    /// Looks up information about the globalized name and type of a given module
    ///
    /// `metadata` is required: the metadata for the module or provide being imported;
    /// `goog_namespace` is optional; `is_from_missing_module_or_provide` is whether the metadata
    /// is synthesized fallback metadata.
    // port: Es6RewriteModules#getGlobalNameAndType
    fn get_global_name_and_type(
        &self,
        compiler: &mut AbstractCompiler,
        metadata: &ModuleMetadata,
        goog_namespace: Option<&JsString>,
        is_from_missing_module_or_provide: bool,
    ) -> GlobalizedModuleName {
        if is_from_missing_module_or_provide {
            // The missing namespace presumably does not have a corresponding type defined in the
            // scope. Use the unknownType instead of asking ModuleRenaming to look up the type.
            let global_name = ModuleRenaming::get_global_name(compiler, metadata, goog_namespace);
            return GlobalizedModuleName::create_from_name(global_name, Some(self.unknown_type));
        }
        let scope = match self.global_typed_scope {
            Some(scope) => GlobalTypedScope::Some(compiler, scope),
            None => GlobalTypedScope::None(compiler),
        };
        GlobalizedModuleName::create(scope, metadata, goog_namespace)
    }

    /// Add alias nodes to the symbol table as they going to be removed by rewriter. Example
    /// aliases:
    ///
    /// ```text
    ///   import * as foo from './foo';
    ///   import {doBar} from './bar';
    ///
    ///   console.log(doBar);
    /// ```
    ///
    /// `n` is the alias node. In the example above alias nodes are foo, doBar, and doBar.
    /// `module` is the name of the module currently being processed.
    // port: Es6RewriteModules#maybeAddAliasToSymbolTable
    fn maybe_add_alias_to_symbol_table(&self, ast: &mut Ast, n: NodeId, module: Option<&str>) {
        let Some(preprocessor_symbol_table) = &self.preprocessor_symbol_table else {
            return;
        };
        let mut preprocessor_symbol_table = preprocessor_symbol_table.lock().unwrap();
        n.put_boolean_prop(ast, NodeId::MODULE_ALIAS, true);
        // Alias can be used in js types. Types have node type STRING and not NAME so we have to
        // use their name as string.
        let node_name = if n.is_string_lit(ast) || n.is_import_star(ast) {
            Some(n.get_string(ast))
        } else {
            preprocessor_symbol_table.get_qualified_name(ast, n)
        };
        // We need to include module as part of the name because aliases are local to current
        // module. Aliases with the same name from different module should be completely different
        // entities.
        let name = JsString::from(format!("alias_{}_", string_value_of(module)))
            .concat(&node_name.unwrap_or_else(|| JsString::from("null")));
        preprocessor_symbol_table.add_reference_with_name(n, Some(name));
    }

    /// Add reference to a file that current module imports. Example:
    ///
    /// ```text
    /// import * as qux from '../some/file.js';
    /// ```
    ///
    /// Will add a reference to file.js on the string node `'../some/file.js'`.
    ///
    /// `import_string_node` is the string node from the import statement that references imported
    /// file. In the example above it is the '../some/file.js' STRING node. `imported_file_path` is
    /// the absolute path to the imported file. In the example above it can be
    /// myproject/folder/some/file.js
    // port: Es6RewriteModules#maybeAddImportedFileReferenceToSymbolTable
    fn maybe_add_imported_file_reference_to_symbol_table(
        &self,
        compiler: &mut AbstractCompiler,
        import_string_node: NodeId,
        imported_file_path: &str,
    ) {
        let Some(preprocessor_symbol_table) = &self.preprocessor_symbol_table else {
            return;
        };
        let mut preprocessor_symbol_table = preprocessor_symbol_table.lock().unwrap();

        // If this if the first import that mentions importedFilePath then we need to create a
        // SCRIPT node for the imported file.
        if preprocessor_symbol_table
            .get_slot(&JsString::from(imported_file_path))
            .is_none()
        {
            let script_node = compiler.get_script_node(imported_file_path);
            if let Some(script_node) = script_node {
                preprocessor_symbol_table
                    .add_reference_with_name(script_node, Some(JsString::from(imported_file_path)));
            }
        }

        preprocessor_symbol_table
            .add_reference_with_name(import_string_node, Some(JsString::from(imported_file_path)));
    }

    // port: Es6RewriteModules#declareGlobalVariable
    fn declare_global_variable(&self, n: NodeId, t: &mut NodeTraversal<'_>) {
        check_state!(n.is_name(t));
        if !self.ast_factory.is_adding_types() {
            return;
        }
        let global_typed_scope = check_not_null!(self.global_typed_scope);

        let name = n.get_string(t);
        if global_typed_scope.has_own_slot(t.get_compiler(), name.clone()) {
            let current_script = t.get_current_script().unwrap();
            t.report(
                current_script,
                &ILLEGAL_MODULE_RENAMING_CONFLICT,
                &[&name.to_string_lossy()],
            );
        } else {
            let type_ = check_not_null!(n.get_jstype(t));
            let input = t.get_input().cloned();
            global_typed_scope.declare(t.get_compiler(), name, Some(n), Some(type_), input, false);
        }
    }
}

impl CompilerPass for Es6RewriteModules {
    // port: Es6RewriteModules#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        check_argument!(externs.is_root(compiler), "%s", externs.to_string(compiler));
        check_argument!(root.is_root(compiler), "%s", root.to_string(compiler));
        NodeTraversal::traverse_roots(compiler, self, externs, root);
        // It is unusual to call this NodeUtil method instead of the TranspilationPasses one. This
        // pass is included in the {@code dependency_resolution} BUILD target and does not have
        // access to {@code TranspilationPasses}. Adding that dep produces a cycle in the BUILD dep
        // graph. Regular transpiler passes must use the {@code
        // TranspilationPasses.maybeMarkFeaturesAsTranspiledAway}
        NodeUtil::remove_feature_from_all_scripts(compiler, root, Feature::MODULES);
        // This pass may add getters properties on module objects.
        GatherGetterAndSetterProperties::update(compiler, externs, root);
    }
}

impl Callback for Es6RewriteModules {
    // port: Es6RewriteModules#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(node_traversal) {
            // Trigger creation of the global scope before inserting any synthetic code.
            node_traversal.get_scope();
            RewriteRequiresForEs6Modules {
                outer: self,
                rename_table: RenameTable::new(),
            }
            .rewrite(node_traversal.get_compiler(), n);
            if Self::is_es6_module_root(node_traversal, n) {
                self.clear_per_file_state();
            } else {
                return false;
            }
        }
        true
    }

    // port: Es6RewriteModules#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_import(t) {
            self.maybe_warn_extern_module(t, n, parent.unwrap());
            self.visit_import(t, n, parent.unwrap());
        } else if n.is_export(t) {
            self.maybe_warn_extern_module(t, n, parent.unwrap());
            self.visit_export(t, n, parent.unwrap());
        } else if n.is_script(t) {
            self.visit_script(t, n);
        } else if n.is_call(t) {
            if GOOG_DECLAREMODULEID.matches(t, n.get_first_child(t).unwrap()) {
                n.get_parent(t).unwrap().detach(t);
            }
        } else if n.is_import_meta(t) {
            #[allow(clippy::collapsible_if)]
            if self.chunk_output_type != ChunkOutputType::ES_MODULES {
                // Since import.meta cannot be transpiled, we are only supporting it when the
                // output format is a module. The default output format type is just script.
                t.report(
                    n,
                    &transpilation_util::CANNOT_CONVERT,
                    &["import.meta. Use --chunk_output_type=ES_MODULES to allow passthrough support."],
                );
            }
        }
    }
}

/// Checks for goog.require, goog.requireType, goog.module.get and goog.forwardDeclare calls that
/// are meant to import ES6 modules and rewrites them.
// port: Es6RewriteModules.RewriteRequiresForEs6Modules
struct RewriteRequiresForEs6Modules<'a> {
    outer: &'a Es6RewriteModules,
    // An (s, old, new) entry indicates that occurrences of `old` in scope `s` should be rewritten
    // as `new`. This is used to rewrite namespaces that appear in calls to goog.requireType and
    // goog.forwardDeclare.
    rename_table: RenameTable,
}

impl RewriteRequiresForEs6Modules<'_> {
    // port: Es6RewriteModules.RewriteRequiresForEs6Modules#rewrite
    fn rewrite(&mut self, compiler: &mut AbstractCompiler, script_node: NodeId) {
        self.rename_table = RenameTable::new();
        NodeTraversal::traverse(compiler, script_node, self);

        if !self.rename_table.is_empty() {
            NodeTraversal::traverse(
                compiler,
                script_node,
                &mut Es6RenameTypeReferences::new(&self.rename_table),
            );
        }
    }
}

impl Callback for RewriteRequiresForEs6Modules<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteModules.RewriteRequiresForEs6Modules#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_call(t) {
            return;
        }

        let call_target = n.get_first_child(t).unwrap();
        if !call_target.is_get_prop(t) {
            return;
        }

        let is_require = GOOG_REQUIRE.matches(t, call_target);
        let is_require_type = GOOG_REQUIRETYPE.matches(t, call_target);
        let is_get = GOOG_MODULE_GET.matches(t, call_target);
        let is_forward_declare = GOOG_FORWARDDECLARE.matches(t, call_target);

        if !is_require && !is_require_type && !is_get && !is_forward_declare {
            return;
        }

        if !n.has_two_children(t) || !n.get_last_child(t).unwrap().is_string_lit(t) {
            // Reported in CheckClosureImports, don't report here.

            return;
        }

        let outer = self.outer;
        let name = n.get_last_child(t).unwrap().get_string(t);
        let module_metadata = outer
            .module_metadata_map
            .get_modules_by_goog_namespace()
            .get(&name)
            .cloned();

        let Some(module_metadata) = module_metadata.filter(|m| m.is_es6_module()) else {
            return;
        };

        // TODO(johnplaisted): Once we have an alternative to forwardDeclare / requireType that
        // doesn't require Closure Library warn about those too.
        // TODO(johnplaisted): Once we have import() support warn about goog.module.get.
        if is_require {
            let input = t.get_input().unwrap().clone();
            let path = input.get_path(t.get_compiler()).to_string();
            let current_module_metadata =
                outer.module_metadata_map.get_modules_by_path().get(&path);
            if current_module_metadata.is_some_and(|m| m.is_es6_module()) {
                t.report(n, &SHOULD_IMPORT_ES6_MODULE, &[]);
            }
        }

        if is_get && t.in_global_hoist_scope() {
            return;
        }

        let statement_node = NodeUtil::get_enclosing_statement(t, n).unwrap();
        let import_has_alias = NodeUtil::is_name_declaration(t, Some(statement_node));
        if import_has_alias {
            if statement_node
                .get_first_child(t)
                .unwrap()
                .is_destructuring_lhs(t)
            {
                if is_forward_declare {
                    // const {a, c:b} = goog.forwardDeclare('an.es6.namespace');
                    t.report(n, &INVALID_DESTRUCTURING_FORWARD_DECLARE, &[]);
                    return;
                }
                if is_require_type {
                    if !statement_node.is_const(t) {
                        t.report(statement_node, &REQUIRE_TYPE_FOR_ES6_SHOULD_BE_CONST, &[]);
                        return;
                    }
                    // const {a, c:b} = goog.requireType('an.es6.namespace');
                    let mut child = statement_node
                        .get_first_first_child(t)
                        .unwrap()
                        .get_first_child(t);
                    while let Some(c) = child {
                        check_state!(c.is_string_key(t));
                        check_state!(c.get_first_child(t).unwrap().is_name(t));
                        let scope_root = t.get_scope_root().unwrap();
                        let key = c.get_first_child(t).unwrap().get_string(t);
                        let value =
                            ModuleRenaming::get_global_name(t, &module_metadata, Some(&name))
                                .getprop(c.get_string(t))
                                .join(t);
                        self.rename_table.insert((scope_root, key), value);
                        child = c.get_next(t);
                    }
                } else {
                    // Work around a bug in the type checker where destructuring can create
                    // too many layers of aliases and confuse the type checker. b/112061124.

                    // const {a, c:b} = goog.require('an.es6.namespace');
                    // const a = module$es6.a;
                    // const b = module$es6.c;
                    let mut child = statement_node
                        .get_first_first_child(t)
                        .unwrap()
                        .get_first_child(t);
                    while let Some(c) = child {
                        check_state!(c.is_string_key(t));
                        check_state!(c.get_first_child(t).unwrap().is_name(t));
                        let global_name = outer
                            .get_global_name_and_type(
                                t.get_compiler(),
                                &module_metadata,
                                Some(&name),
                                /* isFromMissingModuleOrProvide= */ false,
                            )
                            .getprop(&c.get_string(t));
                        let qname = global_name.to_qname(t.get_compiler(), &outer.ast_factory);
                        let const_name = c.get_first_child(t).unwrap().get_string(t);
                        let const_node = outer.ast_factory.create_single_const_name_declaration(
                            t.get_compiler(),
                            const_name,
                            qname,
                        );
                        const_node.srcref_tree(t, c);
                        const_node.insert_before(t, statement_node);
                        child = c.get_next(t);
                    }
                }
                statement_node.detach(t);
                t.report_code_change();
            } else if is_forward_declare || is_require_type {
                if !statement_node.is_const(t) {
                    let diagnostic = if is_forward_declare {
                        &FORWARD_DECLARE_FOR_ES6_SHOULD_BE_CONST
                    } else {
                        &REQUIRE_TYPE_FOR_ES6_SHOULD_BE_CONST
                    };
                    t.report(statement_node, diagnostic, &[]);
                    return;
                }
                // const namespace = goog.forwardDeclare('an.es6.namespace');
                // const namespace = goog.requireType('an.es6.namespace');
                let scope_root = t.get_scope_root().unwrap();
                let key = statement_node.get_first_child(t).unwrap().get_string(t);
                let value =
                    ModuleRenaming::get_global_name(t, &module_metadata, Some(&name)).join(t);
                self.rename_table.insert((scope_root, key), value);
                statement_node.detach(t);
                t.report_code_change();
            } else {
                // const module = goog.require('an.es6.namespace');
                // const module = module$es6;
                let root =
                    ModuleRenaming::get_global_name(t, &module_metadata, Some(&name)).get_root(t);
                let replacement = outer
                    .ast_factory
                    .create_name(t.get_compiler(), root, AstFactory::type_node(n))
                    .srcref_tree(t, n);
                n.replace_with(t, replacement);
                t.report_code_change();
            }
        } else {
            if is_forward_declare || is_require_type {
                // goog.forwardDeclare('an.es6.namespace')
                // goog.requireType('an.es6.namespace')
                let scope_root = t.get_scope_root().unwrap();
                let value =
                    ModuleRenaming::get_global_name(t, &module_metadata, Some(&name)).join(t);
                self.rename_table.insert((scope_root, name.clone()), value);
                statement_node.detach(t);
            } else {
                // goog.require('an.es6.namespace')
                if statement_node.is_expr_result(t) && statement_node.get_first_child(t) == Some(n)
                {
                    statement_node.detach(t);
                } else {
                    let root = ModuleRenaming::get_global_name(t, &module_metadata, Some(&name))
                        .get_root(t);
                    let replacement = outer
                        .ast_factory
                        .create_name(t.get_compiler(), root, AstFactory::type_node(n))
                        .srcref_tree(t, n);
                    n.replace_with(t, replacement);
                }
            }
            t.report_code_change();
        }
    }
}

/// Traverses a node tree and
///
/// 1. Appends a suffix to all global variable names defined in this module.
/// 2. Changes references to imported values to access the exported variable.
// port: Es6RewriteModules.RenameGlobalVars
struct RenameGlobalVars<'a> {
    outer: &'a Es6RewriteModules,
    this_module: Arc<Module>,
}

impl RenameGlobalVars<'_> {
    /// Replaces the reference to a given binding. See
    /// `ModuleRenaming#getGlobalNameForJsDoc(ModuleMap, Binding, List)` for a JS Doc version.
    ///
    /// For example:
    ///
    /// ```text
    ///   // bar
    ///   export let baz = {qux: 0};
    /// ```
    ///
    /// ```text
    ///   // foo
    ///   import * as bar from 'bar';
    ///   export {bar};
    /// ```
    ///
    /// ```text
    ///   import * as foo from 'foo';
    ///   use(foo.bar.baz.qux);
    /// ```
    ///
    /// Should call this method with the binding and node for `foo`. In this example any of these
    /// properties could also be modules. This method will replace as much as the GETPROP as it
    /// can with module exported variables. Meaning in the above example this would return
    /// something like "baz$$module$bar.qux", whereas if this method were called for just
    /// "foo.bar" it would return "module$bar", as it refers to a module object itself.
    ///
    /// `n` is the node to replace; `binding` the binding nameNode is a reference to.
    // port: Es6RewriteModules.RenameGlobalVars#replace
    fn replace(&self, compiler: &mut AbstractCompiler, mut n: NodeId, binding: &Binding) -> NodeId {
        check_state!(n.is_name(compiler));
        let outer = self.outer;
        let mut binding = binding.clone();

        while binding.is_module_namespace()
            && binding.metadata().is_es6_module()
            && n.get_parent(compiler).unwrap().is_get_prop(compiler)
        {
            let property_name = n.get_parent(compiler).unwrap().get_string(compiler);
            let m = outer
                .get_module(
                    binding
                        .metadata()
                        .path()
                        .expect("java.lang.NullPointerException"),
                )
                .expect("java.lang.NullPointerException");
            if let Some(b) = m.namespace().get(&property_name) {
                binding = b.clone();
                n = n.get_parent(compiler).unwrap();
            } else {
                // This means someone referenced an invalid export on a module object. This
                // should be an error, so just rewrite and let the type checker complain later.
                // It isn't a super clear error, but we're working on type checking modules soon.
                break;
            }
        }

        let global_name = ModuleRenaming::get_global_name_of_binding(compiler, &binding);
        let new_node;
        if !global_name.is_simple(compiler) {
            let root = global_name.get_root(compiler);
            let joined = global_name.join(compiler).to_string_lossy();
            new_node =
                // we might encounter a name not in the global scope when requiring a missing
                // symbol.
                if outer
                    .global_typed_scope
                    .is_some_and(|scope| scope.has_slot(compiler, root.clone()))
                {
                    outer.ast_factory.create_qname_using_js_type_info(
                        compiler,
                        outer.global_typed_scope,
                        &joined,
                    )
                } else {
                    outer
                        .ast_factory
                        .create_qname_with_unknown_type(compiler, &joined)
                };
        } else {
            // Because this pass does not update the global scope with injected names,
            // t.getScope() will not contain a declaration for this global name. Fortunately, we
            // already have the JSType on the existing node to pass to AstFactory.
            let root = global_name.get_root(compiler);
            new_node = outer
                .ast_factory
                .create_name(compiler, root, AstFactory::type_node(n));
        }

        // For kythe: the new node only represents the last name it replaced, not all the names.
        // e.g. if we rewrite `a.b.c.d.e` to `x.d.e`, then `x` should map to `c`, not `a.b.c`.
        n.replace_with(compiler, new_node);
        new_node.srcref_tree(compiler, n);
        let original_name = n.get_string(compiler);
        new_node.set_original_name(compiler, Some(original_name));
        new_node
    }

    /// Replace type name references. Change short names to fully qualified names with namespace
    /// prefixes. Eg: {Foo} becomes {module$test.Foo}.
    // port: Es6RewriteModules.RenameGlobalVars#fixTypeNode
    fn fix_type_node(&self, t: &mut NodeTraversal<'_>, type_node: NodeId) {
        let outer = self.outer;
        if type_node.is_string_lit(t) {
            let input = t.get_input().unwrap().clone();
            let path = input.get_path(t.get_compiler());
            let this_module = outer
                .get_module(&path)
                .expect("java.lang.NullPointerException");
            let name = type_node.get_string(t);
            let splitted = dot_splitter_split_to_list(&name, 0);
            let base_name = splitted[0].clone();
            let mut rest = JsString::from("");
            if splitted.len() > 1 {
                rest = name.substring_from(base_name.length());
            }
            let scope = t.get_scope();
            let var = scope.get_var(t.get_compiler(), base_name.clone());
            if var.is_some_and(|var| var.is_global(t.get_compiler())) {
                let new_name = ModuleRenaming::get_global_name_of_es_module_local_variable(
                    t,
                    this_module.metadata(),
                    &base_name,
                )
                .join(t)
                .concat(&rest);
                self.maybe_set_new_name(t, type_node, &name, &new_name);
            } else if var.is_none() && this_module.bound_names().contains_key(&base_name) {
                // Imports have been detached, so they won't show up in scope. Thus if we have a
                // variable not in scope that shares the name of an import it is the import.
                let binding = this_module.bound_names().get(&base_name).unwrap();
                let global_name = ModuleRenaming::get_global_name_for_js_doc(
                    t,
                    outer
                        .module_map
                        .as_ref()
                        .expect("java.lang.NullPointerException"),
                    binding,
                    &splitted[1..],
                );
                self.maybe_set_new_name(t, type_node, &name, &global_name);

                if outer.preprocessor_symbol_table.is_some() {
                    // Jsdoc type node is a single STRING node that spans the whole type. For
                    // example STRING node "bar.Foo". ES6 import rewrite replaces only "module"
                    // part of the type: "bar.Foo" => "module$full$path$bar$Foo". We have to
                    // record "bar" as alias.
                    let only_base_name = t.new_string(base_name.clone()).srcref(t, type_node);
                    only_base_name.set_length(t, base_name.length() as i32);
                    let source_name = t.get_source_name();
                    outer.maybe_add_alias_to_symbol_table(
                        t,
                        only_base_name,
                        source_name.as_deref(),
                    );
                }
            }

            type_node.set_original_name(t, Some(name));
        }

        let mut child = type_node.get_first_child(t);
        while let Some(c) = child {
            self.fix_type_node(t, c);
            child = c.get_next(t);
        }
    }

    // port: Es6RewriteModules.RenameGlobalVars#maybeSetNewName
    fn maybe_set_new_name(
        &self,
        t: &mut NodeTraversal<'_>,
        node: NodeId,
        name: &JsString,
        new_name: &JsString,
    ) {
        if name != new_name {
            node.set_string(t, new_name.clone());
            node.set_original_name(t, Some(name.clone()));
            t.report_code_change();
        }
    }
}

impl Callback for RenameGlobalVars<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteModules.RenameGlobalVars#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let info = n.get_jsdoc_info(t);
        if let Some(info) = info {
            for type_node in info.get_type_nodes() {
                self.fix_type_node(t, type_node);
            }
        }

        if n.is_name(t) {
            let name = n.get_string(t);

            let scope = t.get_scope();
            let var = scope.get_var(t.get_compiler(), name.clone());
            if var.is_some_and(|var| var.is_global(t.get_compiler())) {
                // Avoid polluting the global namespace.
                let new_name = ModuleRenaming::get_global_name_of_es_module_local_variable(
                    t,
                    self.this_module.metadata(),
                    &name,
                )
                .join(t);
                n.set_string(t, new_name);
                n.set_original_name(t, Some(name));
                t.report_code_change_at_node(n);
                if NodeUtil::is_declaration_l_value(t, n) {
                    self.outer.declare_global_variable(n, t);
                }
            } else if var.is_none() && self.this_module.bound_names().contains_key(&name) {
                // Imports have been detached, so they won't show up in scope. Thus if we have a
                // variable not in scope that shares the name of an import it is the import.
                let source_name = t.get_source_name();
                self.outer
                    .maybe_add_alias_to_symbol_table(t, n, source_name.as_deref());
                let binding = self.this_module.bound_names().get(&name).unwrap().clone();

                let replacement = self.replace(t.get_compiler(), n, &binding);

                // `n.x()` may become `foo()`
                let parent = parent.unwrap();
                if replacement.is_name(t)
                    && parent.is_call(t)
                    && parent.get_first_child(t) == Some(n)
                    && parent.get_boolean_prop(t, NodeId::FREE_CALL)
                {
                    parent.put_boolean_prop(t, NodeId::FREE_CALL, true);
                }

                if NodeUtil::is_declaration_l_value(t, n) {
                    self.outer.declare_global_variable(n, t);
                }
                t.report_code_change();
            }
        }
    }
}
