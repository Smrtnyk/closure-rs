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
//   src/com/google/javascript/jscomp/Es6RewriteModulesToCommonJsModules.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Rewrites an ES6 module to a CommonJS-like module for the sake of per-file transpilation +
//! bunlding (e.g. Closure Bundler). Output is not meant to be type checked.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    deps::module_loader::ModulePath,
    es6_rewrite_modules::Es6RewriteModules,
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    scope::ScopeId,
    transpilation_util,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use indexmap::IndexSet;
use std::collections::BTreeMap;

// port: Es6RewriteModulesToCommonJsModules#JSCOMP_DEFAULT_EXPORT
const JSCOMP_DEFAULT_EXPORT: &str = "$$default";
// port: Es6RewriteModulesToCommonJsModules#MODULE
const MODULE: &str = "$$module";
// port: Es6RewriteModulesToCommonJsModules#EXPORTS
const EXPORTS: &str = "$$exports";
// port: Es6RewriteModulesToCommonJsModules#REQUIRE
const REQUIRE: &str = "$$require";

/// Rewrites an ES6 module to a CommonJS-like module for the sake of per-file transpilation +
/// bunlding (e.g. Closure Bundler). Output is not meant to be type checked.
// port: Es6RewriteModulesToCommonJsModules
pub struct Es6RewriteModulesToCommonJsModules;

impl Es6RewriteModulesToCommonJsModules {
    // port: Es6RewriteModulesToCommonJsModules#Es6RewriteModulesToCommonJsModules
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }
}

impl CompilerPass for Es6RewriteModulesToCommonJsModules {
    // port: Es6RewriteModulesToCommonJsModules#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            if Es6RewriteModules::is_es6_module_root(compiler, s) {
                let mut rewriter = Rewriter::new(compiler, s);
                NodeTraversal::traverse(compiler, s, &mut rewriter);
            }
            script = s.get_next(compiler);
        }
        // It is unusual to call this NodeUtil method instead of the TranspilationPasses one. This
        // pass is included in the {@code dependency_resolution} BUILD target and does not have access
        // to {@code TranspilationPasses}. Adding that dep produces a cycle in the BUILD dep graph.
        // Regular transpiler passes must use the {@code
        // TranspilationPasses.maybeMarkFeaturesAsTranspiledAway}
        NodeUtil::remove_feature_from_all_scripts(compiler, root, Feature::MODULES);
        GatherGetterAndSetterProperties::update(compiler, externs, root);
    }
}

// port: Es6RewriteModulesToCommonJsModules.LocalQName
struct LocalQName {
    q_name: JsString,

    /// Node to use for source information. All exported properties are transpiled to ES5 getters
    /// so debuggers will automatically step into these, even with source maps. When stepping in
    /// this node will be displayed in the source map. In general it should either be the export
    /// itself (e.g. export function or class) or the specific name being exported (export specs,
    /// const, etc).
    node_for_source_info: NodeId,
}

impl LocalQName {
    // port: Es6RewriteModulesToCommonJsModules.LocalQName#LocalQName
    fn new(q_name: JsString, node_for_source_info: NodeId) -> Self {
        Self {
            q_name,
            node_for_source_info,
        }
    }
}

/// Normalizes a registered or import path.
///
/// Absolute import paths need to match the registered path exactly. Some
/// `ModuleLoader.ModulePath`s will have a leading slash and some won't. So in order to have
/// everything line up AND preserve schemes (if they exist) then just strip leading /.
///
/// Additionally if any path contains a protocol it will be stripped only the path part will
/// remain. This is done heuristically as we cannot use `java.net.URL` or `java.nio.file.Path` due
/// to GWT. As a result of stripping this cross-domain imports are not compatible with this pass.
// port: Es6RewriteModulesToCommonJsModules#normalizePath
fn normalize_path(path: &str) -> String {
    let mut path = JsString::from(path);
    let index_of_protocol = path.index_of("://");
    if index_of_protocol > -1 {
        path = path.substring_from(index_of_protocol as usize + 3);
        let index_of_slash = path.index_of("/");
        if index_of_slash > -1 {
            path = path.substring_from(index_of_slash as usize + 1);
        }
    } else if path.starts_with("/") {
        path = path.substring_from(1);
    }
    path.to_string_lossy()
}

// port: Es6RewriteModulesToCommonJsModules.ModuleRequest
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ModuleRequest {
    specifier: String,
    var_name: JsString,
}

impl ModuleRequest {
    // port: Es6RewriteModulesToCommonJsModules.ModuleRequest#create
    fn create(specifier: String, var_name: JsString) -> Self {
        Self {
            specifier,
            var_name,
        }
    }
}

/// Rewrites a single ES6 module into a CommonJS like module designed to be loaded in the
/// compiler's module runtime.
// port: Es6RewriteModulesToCommonJsModules.Rewriter
struct Rewriter {
    require_insert_spot: Option<NodeId>,
    script: NodeId,
    exported_name_to_local_q_name: BTreeMap<JsString, LocalQName>,
    imports: IndexSet<NodeId>,
    import_requests: IndexSet<ModuleRequest>,
    module_path: ModulePath,
}

impl Callback for Rewriter {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::IMPORT => {
                let input = t.get_input().unwrap().clone();
                let path = input.get_path(t.get_compiler());
                self.visit_import(t, &path, n);
            }
            Token::EXPORT => self.visit_export(t, n, parent.unwrap()),
            Token::SCRIPT => self.visit_script(t, n),
            Token::NAME => self.maybe_rename_imported_value(t, n),
            _ => {}
        }
    }
}

impl Rewriter {
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#Rewriter
    fn new(compiler: &mut AbstractCompiler, script: NodeId) -> Self {
        let input_id = script.get_input_id(compiler).unwrap();
        let input = compiler.get_input(&input_id).unwrap().clone();
        let module_path = input.get_path(compiler);
        Self {
            script,
            require_insert_spot: None,
            // TreeMap because ES6 orders the export key using natural ordering.
            exported_name_to_local_q_name: BTreeMap::new(),
            import_requests: IndexSet::new(),
            imports: IndexSet::new(),
            module_path,
        }
    }

    /// Given an import node gets the name of the var to use for the imported module.
    ///
    /// Example: `import {v} from './foo.js'; use(v);` Can become:
    ///
    /// ```text
    ///   const module$foo = require('./foo.js');
    ///   use(module$foo.v);
    /// ```
    ///
    /// This method would return "module$foo".
    ///
    /// Note that if there is a star import the name will be preserved.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#getVarNameOfImport(Node)
    fn get_var_name_of_import(&self, ast: &Ast, import_decl: NodeId) -> JsString {
        check_state!(import_decl.is_import(ast));
        let second = import_decl.get_second_child(ast).unwrap();
        if second.is_import_star(ast) {
            return second.get_string(ast);
        }
        self.get_var_name_of_import_request(
            &import_decl
                .get_last_child(ast)
                .unwrap()
                .get_string(ast)
                .to_string_lossy(),
        )
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#getVarNameOfImport(String)
    fn get_var_name_of_import_request(&self, import_request: &str) -> JsString {
        JsString::from(
            self.module_path
                .resolve_module_as_path(import_request)
                .to_module_name()
                .as_str(),
        )
    }

    /// Returns the qualified name to use to reference an imported value.
    ///
    /// Used to rename references to imported values within this module.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#getNameOfImportedValue
    fn get_name_of_imported_value(&self, ast: &Ast, name_node: NodeId) -> JsString {
        let mut import_decl = name_node;

        while !import_decl.is_import(ast) {
            import_decl = import_decl.get_parent(ast).unwrap();
        }

        let module_name = self.get_var_name_of_import(ast, import_decl);

        let parent = name_node.get_parent(ast).unwrap();
        if parent.is_import_spec(ast) {
            module_name
                .concat(&".".into())
                .concat(&parent.get_first_child(ast).unwrap().get_string(ast))
        } else if name_node.is_import_star(ast) {
            module_name
        } else {
            check_state!(parent.is_import(ast));
            module_name.concat(&".default".into())
        }
    }

    /// Returns the qualified name to use to reference an imported value if the given node is an
    /// imported name or null if the value is not imported or if it is in the import statement
    /// itself.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#maybeGetNameOfImportedValue
    fn maybe_get_name_of_imported_value(
        &self,
        compiler: &mut AbstractCompiler,
        s: ScopeId,
        name_node: NodeId,
    ) -> Option<JsString> {
        check_state!(name_node.is_name(compiler));
        let var = s.get_var(compiler, name_node.get_string(compiler));

        if let Some(var) = var
            // variables added implicitly to the scope, like arguments, have a null name node
            && let Some(var_name_node) = var.get_name_node(compiler)
            && NodeUtil::is_imported_name(compiler, var_name_node)
            && name_node != var_name_node
        {
            return Some(self.get_name_of_imported_value(compiler, var_name_node));
        }

        None
    }

    /// Renames the given name node if it is an imported value.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#maybeRenameImportedValue
    fn maybe_rename_imported_value(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        check_state!(n.is_name(t));
        let parent = n.get_parent(t).unwrap();

        if parent.is_export(t)
            || parent.is_export_spec(t)
            || parent.is_import(t)
            || parent.is_import_spec(t)
        {
            return;
        }

        let scope = t.get_scope();
        let q_name = self.maybe_get_name_of_imported_value(t.get_compiler(), scope, n);

        if let Some(q_name) = q_name {
            let replacement = NodeUtil::new_qname(t.get_compiler(), q_name);
            n.replace_with(t, replacement);
            t.report_code_change();
        }
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitScript
    fn visit_script(&mut self, t: &mut NodeTraversal<'_>, script: NodeId) {
        check_state!(self.script == script);
        let module_node = script.get_first_child(t).unwrap();
        check_state!(module_node.is_module_body(t));
        module_node.detach(t);
        let children = module_node.remove_children(t);
        script.add_children_to_front(t, children);

        // Order here is important. We want the end result to be:
        //  $jscomp.registerAndLoadModule(function($$require, $$exports, $$module) {
        //   // First to ensure circular deps can see exports of this module before we require them,
        //   // and also so that temporal deadzone is respected.
        //   //<export def>
        //   // Second so the module definition can reference imported modules, and so any require'd
        //   // modules are loaded.
        //   //<requires>
        //   // And finally last is the actual module definition.
        //   //<module def>
        //  }, /* <module path> */, [/* <deps> */]);
        // As a result the calls below are in *inverse* order to what we want above so they can keep
        // adding to the front of the script.
        self.add_require_calls(t.get_compiler());
        self.add_export_def(t.get_compiler());
        self.register_and_load_module(t);
    }

    /// Adds one call to require per imported module.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#addRequireCalls
    fn add_require_calls(&mut self, compiler: &mut AbstractCompiler) {
        if !self.import_requests.is_empty() {
            for &import_decl in &self.imports {
                import_decl.detach(compiler);
            }

            let mut imported_names: IndexSet<JsString> = IndexSet::new();

            for request in &self.import_requests {
                let var_name = request.var_name.clone();
                if imported_names.insert(var_name.clone()) {
                    let require_name = IR::name(compiler, REQUIRE);
                    let specifier = IR::string(compiler, request.specifier.as_str());
                    let require_call = IR::call(compiler, require_name, &[specifier]);
                    require_call.put_boolean_prop(compiler, NodeId::FREE_CALL, true);
                    let name = IR::name(compiler, var_name);
                    let decl = IR::var_with_value(compiler, name, require_call);
                    decl.srcref_tree_if_missing(compiler, self.script);
                    match self.require_insert_spot {
                        None => self.script.add_child_to_front(compiler, decl),
                        Some(spot) => decl.insert_after(compiler, spot),
                    }
                    self.require_insert_spot = Some(decl);
                }
            }
        }
    }

    /// Wraps the entire current module definition in a $jscomp.registerAndLoadModule function.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#registerAndLoadModule
    fn register_and_load_module(&mut self, t: &mut NodeTraversal<'_>) {
        let script = self.script;
        let block = IR::block(t);
        let children = script.remove_children(t);
        block.add_children_to_front(t, children);

        // TODO(b/282006497): Maybe mark this function for strict mode?
        // NOTE: One might be tempted to add `'use strict';` here, but that causes problems.
        // Optimizations and transpilations are not written to look for this statement,
        // and are likely to either remove it or move other statements ahead of it,
        // making it ineffective.
        // The "right" way to mark the method for strict mode would be to apply the
        // USE_STRICT node property to its body, but at the moment that will have no
        // effect because 1) the code printer ignores it and 2) TypedAst doesn't
        // serialize it.

        let fn_name = IR::name(t, "");
        let p_require = IR::name(t, REQUIRE);
        let p_exports = IR::name(t, EXPORTS);
        let p_module = IR::name(t, MODULE);
        let params = IR::param_list(t, &[p_require, p_exports, p_module]);
        let module_function = IR::function(t, fn_name, params, block);

        let shallow_deps = t.new_node(Token::ARRAYLIT);

        for request in &self.import_requests {
            let s = IR::string(t, request.specifier.as_str());
            shallow_deps.add_child_to_back(t, s);
        }

        let input_name = t.get_input().unwrap().get_name().to_string();
        // Resolving this path enables removing module roots from this path.
        let resolved = normalize_path(
            &t.get_compiler()
                .get_module_loader()
                .resolve(&input_name)
                .to_string(),
        );
        let jscomp = IR::name(t, "$jscomp");
        let target = IR::getprop(t, jscomp, "registerAndLoadModule");
        let path_string = IR::string(t, resolved.as_str());
        let call = IR::call(t, target, &[module_function, path_string, shallow_deps]);
        let expr_result = IR::expr_result(t, call);

        let expr_result = expr_result.srcref_tree_if_missing(t, script);
        script.add_child_to_back(t, expr_result);

        t.get_compiler().report_change_to_change_scope(script);
        t.get_compiler()
            .report_change_to_change_scope(module_function);
        t.report_code_change();
    }

    /// Adds exports to the exports object using Object.defineProperties.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#addExportDef
    fn add_export_def(&mut self, compiler: &mut AbstractCompiler) {
        if !self.exported_name_to_local_q_name.is_empty() {
            let define_properties_lit = IR::objectlit(compiler, &[]);

            for (key, value) in &self.exported_name_to_local_q_name {
                Self::add_export(compiler, define_properties_lit, key, value);
            }

            let target = NodeUtil::new_qname(compiler, "Object.defineProperties");
            let exports = IR::name(compiler, EXPORTS);
            let call = IR::call(compiler, target, &[exports, define_properties_lit]);
            let stmt =
                IR::expr_result(compiler, call).srcref_tree_if_missing(compiler, self.script);
            self.script.add_child_to_front(compiler, stmt);
        }
    }

    /// Adds an ES5 getter to the given object literal to use an an export.
    // port: Es6RewriteModulesToCommonJsModules.Rewriter#addExport
    fn add_export(
        compiler: &mut AbstractCompiler,
        define_properties_lit: NodeId,
        exported_name: &JsString,
        local_q_name: &LocalQName,
    ) {
        let exported_value = NodeUtil::new_qname(compiler, local_q_name.q_name.clone());
        let fn_name = IR::name(compiler, "");
        let params = IR::param_list(compiler, &[]);
        let ret = IR::return_node_with_expression(compiler, exported_value);
        let body = IR::block_with_child(compiler, ret);
        let getter_function = IR::function(compiler, fn_name, params, body);
        getter_function.srcref_tree(compiler, local_q_name.node_for_source_info);

        let true_node = IR::true_node(compiler);
        let enumerable = IR::string_key_with_value(compiler, "enumerable", true_node);
        let get = IR::string_key_with_value(compiler, "get", getter_function);
        let obj_lit = IR::objectlit(compiler, &[enumerable, get]);
        let key = IR::string_key_with_value(compiler, exported_name.clone(), obj_lit);
        define_properties_lit.add_child_to_back(compiler, key);

        compiler.report_change_to_change_scope(getter_function);
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitImport
    fn visit_import(&mut self, t: &mut NodeTraversal<'_>, path: &ModulePath, import_decl: NodeId) {
        let specifier = import_decl
            .get_last_child(t)
            .unwrap()
            .get_string(t)
            .to_string_lossy();
        if specifier.contains("://") {
            let error = JSError::make(
                t,
                import_decl,
                &transpilation_util::CANNOT_CONVERT,
                &["Module requests with protocols."],
            );
            t.get_compiler().report(error);
        }

        // Normalize the import path according to the module resolution scheme so that bundles are
        // compatible with the compiler's module loader options.
        let normalized = normalize_path(&path.resolve_module_as_path(&specifier).to_string());
        let var_name = self.get_var_name_of_import(t, import_decl);
        self.import_requests
            .insert(ModuleRequest::create(normalized, var_name));
        self.imports.insert(import_decl);
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitExportDefault
    fn visit_export_default(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        let child = export.get_first_child(t).unwrap();
        let mut name = None;

        if child.is_function(t) || child.is_class(t) {
            name = NodeUtil::get_name(t, child);
        }

        let name = if let Some(name) = name {
            let decl = child.detach(t);
            export.replace_with(t, decl);
            name
        } else {
            let name = JsString::from(JSCOMP_DEFAULT_EXPORT);
            // Default exports are constant in more ways than one. Not only can they not be
            // overwritten but they also act like a const for temporal dead-zone purposes.
            let lhs = IR::name(t, name.clone());
            let value = export.remove_first_child(t).unwrap();
            let var = IR::const_node(t, lhs, value);
            let var = var.srcref_tree_if_missing(t, export);
            export.replace_with(t, var);
            let current_script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(
                t.get_compiler(),
                current_script,
                Feature::CONST_DECLARATIONS,
            );
            name
        };

        self.exported_name_to_local_q_name
            .insert("default".into(), LocalQName::new(name, export));
        t.report_code_change();
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitExportFrom
    fn visit_export_from(&mut self, t: &mut NodeTraversal<'_>, export: NodeId, parent: NodeId) {
        //   export {x, y as z} from 'moduleIdentifier';
        let module_identifier = export.get_last_child(t).unwrap();
        let e1 = IR::empty(t);
        let e2 = IR::empty(t);
        let cloned = module_identifier.clone_node(t);
        let import_node = IR::import_node(t, e1, e2, cloned);
        import_node.srcref(t, export);
        import_node.insert_before(t, export);
        self.visit(t, import_node, Some(parent));

        let module_name =
            self.get_var_name_of_import_request(&module_identifier.get_string(t).to_string_lossy());

        let mut export_spec = export.get_first_first_child(t);
        while let Some(spec) = export_spec {
            self.exported_name_to_local_q_name.insert(
                spec.get_last_child(t).unwrap().get_string(t),
                LocalQName::new(
                    module_name
                        .concat(&".".into())
                        .concat(&spec.get_first_child(t).unwrap().get_string(t)),
                    spec,
                ),
            );
            export_spec = spec.get_next(t);
        }

        export.detach(t);
        t.report_code_change();
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitExportSpecs
    fn visit_export_specs(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        //     export {Foo};
        let mut export_spec = export.get_first_first_child(t);
        while let Some(spec) = export_spec {
            let mut local_name = spec.get_first_child(t).unwrap().get_string(t);
            let scope = t.get_scope();
            let var = scope.get_var(t.get_compiler(), &local_name);
            if let Some(var) = var
                && let var_name_node = var.get_name_node(t.get_compiler()).unwrap()
                && NodeUtil::is_imported_name(t, var_name_node)
            {
                let first = spec.get_first_child(t).unwrap();
                let scope = t.get_scope();
                local_name = check_not_null!(self.maybe_get_name_of_imported_value(
                    t.get_compiler(),
                    scope,
                    first
                ));
            }
            self.exported_name_to_local_q_name.insert(
                spec.get_last_child(t).unwrap().get_string(t),
                LocalQName::new(local_name, spec),
            );
            export_spec = spec.get_next(t);
        }
        export.detach(t);
        t.report_code_change();
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitExportNameDeclaration
    fn visit_export_name_declaration(&mut self, t: &mut NodeTraversal<'_>, declaration: NodeId) {
        //    export var Foo;
        //    export let {a, b:[c,d]} = {};
        NodeUtil::visit_lhs_nodes_in_node(t.get_compiler(), declaration, &mut |c, lhs| {
            self.add_exported_name(c, lhs)
        });
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#addExportedName
    fn add_exported_name(&mut self, ast: &Ast, lhs: NodeId) {
        check_state!(lhs.is_name(ast));
        let name = lhs.get_string(ast);
        self.exported_name_to_local_q_name
            .insert(name.clone(), LocalQName::new(name, lhs));
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitExportDeclaration
    fn visit_export_declaration(&mut self, t: &mut NodeTraversal<'_>, export: NodeId) {
        //    export var Foo;
        //    export function Foo() {}
        // etc.
        let declaration = export.get_first_child(t).unwrap();

        if NodeUtil::is_name_declaration(t, Some(declaration)) {
            self.visit_export_name_declaration(t, declaration);
        } else {
            check_state!(declaration.is_function(t) || declaration.is_class(t));
            let name = declaration.get_first_child(t).unwrap().get_string(t);
            self.exported_name_to_local_q_name
                .insert(name.clone(), LocalQName::new(name, export));
        }

        let detached = declaration.detach(t);
        export.replace_with(t, detached);
        t.report_code_change();
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitExportStar
    fn visit_export_star(&mut self, t: &mut NodeTraversal<'_>, export: NodeId, parent: NodeId) {
        //   export * from 'moduleIdentifier';
        let module_identifier = export.get_last_child(t).unwrap();

        // Make an "import 'spec'" from this export node and then visit it to rewrite to a require().
        let e1 = IR::empty(t);
        let e2 = IR::empty(t);
        let cloned = module_identifier.clone_node(t);
        let import_node = IR::import_node(t, e1, e2, cloned);
        import_node.srcref(t, export);
        import_node.insert_before(t, export);
        self.visit(t, import_node, Some(parent));

        let module_name =
            self.get_var_name_of_import_request(&module_identifier.get_string(t).to_string_lossy());
        let module = IR::name(t, "$$module");
        let target = IR::getprop(t, module, "exportAllFrom");
        let arg = IR::name(t, module_name);
        let call = IR::call(t, target, &[arg]);
        let replacement = IR::expr_result(t, call).srcref_tree(t, export);
        export.replace_with(t, replacement);

        t.report_code_change();
    }

    // port: Es6RewriteModulesToCommonJsModules.Rewriter#visitExport
    fn visit_export(&mut self, t: &mut NodeTraversal<'_>, export: NodeId, parent: NodeId) {
        if export.get_boolean_prop(t, NodeId::EXPORT_DEFAULT) {
            self.visit_export_default(t, export);
        } else if export.get_boolean_prop(t, NodeId::EXPORT_ALL_FROM) {
            self.visit_export_star(t, export, parent);
        } else if export.has_two_children(t) {
            self.visit_export_from(t, export, parent);
        } else if export.get_first_child(t).unwrap().is_export_specs(t) {
            self.visit_export_specs(t, export);
        } else {
            self.visit_export_declaration(t, export);
        }
    }
}
