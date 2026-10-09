/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/FindModuleDependencies.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Find and update any direct dependencies of an input. Used to walk the dependency graph and
//! support a strict depth-first dependency ordering. Marks an input as providing its module name.
//!
//! Discovers dependencies from:
//!
//! - goog.require calls
//! - ES6 import statements
//! - CommonJS require statements
//! - goog.requireDynamic calls
//!
//! The order of dependency references is preserved so that a deterministic depth-first ordering
//! can be achieved.
use crate::{
    compiler::Compiler,
    compiler_input::ModuleType,
    deps::dependency_info::Require,
    es6_rewrite_modules::Es6RewriteModules,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    process_common_js_modules::ProcessCommonJSModules,
    scope::ScopeId,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{check_argument, node::NodeId, qualified_name::QualifiedName, token::Token};
use std::sync::LazyLock;

// port: FindModuleDependencies#GOOG_MODULE
static GOOG_MODULE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.module"));
// port: FindModuleDependencies#GOOG_PROVIDE
static GOOG_PROVIDE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.provide"));

pub struct FindModuleDependencies {
    supports_es6_modules: bool,
    supports_common_js_modules: bool,
    module_type: ModuleType,
    dynamic_import_scope: Option<ScopeId>,
    input_path_by_webpack_id: IndexMap<String, String>,
}

impl FindModuleDependencies {
    // port: FindModuleDependencies#FindModuleDependencies
    pub fn new(
        supports_es6_modules: bool,
        supports_common_js_modules: bool,
        input_path_by_webpack_id: &IndexMap<String, String>,
    ) -> Self {
        Self {
            supports_es6_modules,
            supports_common_js_modules,
            module_type: ModuleType::NONE,
            dynamic_import_scope: None,
            input_path_by_webpack_id: input_path_by_webpack_id.clone(),
        }
    }

    // port: FindModuleDependencies#process
    pub fn process(&mut self, compiler: &mut Compiler, root: NodeId) {
        check_argument!(root.is_script(compiler));
        if Es6RewriteModules::is_es6_module_root(compiler, root) {
            self.module_type = ModuleType::ES6;
        }
        let input_id = root.get_input_id(compiler).unwrap();
        let input = compiler.get_input(&input_id).unwrap().clone();

        // The "goog" namespace isn't always specifically required.
        // The deps parser will pick up any access to a `goog.foo()` call
        // and add "goog" as a dependency. If "goog" is a dependency of the
        // file we add it here to the ordered requires so that it's always
        // first.
        if input.get_requires(compiler).contains(Require::BASE) {
            input.add_ordered_require((**Require::BASE).clone());
        }

        NodeTraversal::traverse(compiler, root, self);

        if self.module_type == ModuleType::ES6 {
            self.convert_to_es6_module(compiler, root, true);
        } else if self.module_type == ModuleType::NONE
            && self
                .input_path_by_webpack_id
                .values()
                .any(|v| *v == input.get_path(compiler).to_string())
        {
            self.module_type = ModuleType::IMPORTED_SCRIPT;
        }

        input.add_provide(input.get_path(compiler).to_module_name());
        input.set_js_module_type(self.module_type);
        input.set_has_full_parse_dependency_info(true);
    }

    /// Adds an es6 module from an import node (import or export statement) to the graph.
    // port: FindModuleDependencies#addEs6ModuleImportToGraph
    fn add_es6_module_import_to_graph(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let module_name = self.get_es6_module_name_from_import_node(t, n);
        let input = t.get_input().unwrap().clone();
        if module_name.starts_with("goog.") {
            input.add_ordered_require((**Require::BASE).clone());
        }
        let raw_path = n.get_last_child(t).unwrap().get_string(t).to_string_lossy();
        input.add_ordered_require(Require::es6_import(&module_name, &raw_path));
    }

    /// Get the module name from an import node (import or export statement).
    // port: FindModuleDependencies#getEs6ModuleNameFromImportNode
    fn get_es6_module_name_from_import_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
    ) -> String {
        let import_name = n.get_last_child(t).unwrap().get_string(t).to_string_lossy();
        let is_namespace_import = import_name.starts_with("goog:");
        if is_namespace_import {
            // Allow importing Closure namespace objects (e.g. from goog.provide or goog.module) as
            //   import ... from 'goog:my.ns.Object'.
            // These are rewritten to plain namespace object accesses.
            import_name["goog:".len()..].to_string()
        } else {
            let input = t.get_input().unwrap().clone();
            let path = input.get_path(t.get_compiler());
            let mut module_path = path.resolve_js_module(
                &import_name,
                n.get_source_file_name(t).as_deref(),
                n.get_lineno(t),
                n.get_charno(t),
            );
            // The module loader reported to the compiler (its ErrorHandler) during the resolution.
            t.get_compiler().flush_module_errors();
            if module_path.is_none() {
                // The module loader issues an error
                // Fall back to assuming the module is a file path
                module_path = Some(path.resolve_module_as_path(&import_name));
            }
            module_path.unwrap().to_module_name()
        }
    }

    // port: FindModuleDependencies#convertToEs6Module
    fn convert_to_es6_module(
        &mut self,
        compiler: &mut Compiler,
        root: NodeId,
        skip_goog_provide_module_check: bool,
    ) -> bool {
        if Es6RewriteModules::is_es6_module_root(compiler, root) {
            return true;
        }
        if !skip_goog_provide_module_check {
            let mut finder = FindGoogProvideOrGoogModule { found: false };
            NodeTraversal::traverse(compiler, root, &mut finder);
            if finder.is_found() {
                return false;
            }
        }
        let module_node = compiler.new_node(Token::MODULE_BODY).srcref(compiler, root);
        let children = root.remove_children(compiler);
        module_node.add_children_to_back(compiler, children);
        root.add_child_to_back(compiler, module_node);
        true
    }
}

struct FindGoogProvideOrGoogModule {
    found: bool,
}

impl FindGoogProvideOrGoogModule {
    // port: FindModuleDependencies.FindGoogProvideOrGoogModule#isFound
    fn is_found(&self) -> bool {
        self.found
    }
}

impl Callback for FindGoogProvideOrGoogModule {
    // port: FindModuleDependencies.FindGoogProvideOrGoogModule#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if self.found {
            return false;
        }
        // Shallow traversal, since we don't need to inspect within functions or expressions.
        if NodeUtil::is_shallow_statement_tree(node_traversal, parent) {
            if n.is_expr_result(node_traversal) {
                let maybe_get_prop = n.get_first_first_child(node_traversal);
                if let Some(maybe_get_prop) = maybe_get_prop
                    && (GOOG_PROVIDE.matches(node_traversal, maybe_get_prop)
                        || GOOG_MODULE.matches(node_traversal, maybe_get_prop))
                {
                    self.found = true;
                    return false;
                }
            }
            return true;
        }
        false
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

impl Callback for FindModuleDependencies {
    // port: FindModuleDependencies#shouldTraverse
    #[allow(clippy::collapsible_if)] // Retain Java control flow.
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let resolution_mode = t.get_compiler().get_options().get_module_resolution_mode();
        if self.supports_common_js_modules
            && n.is_function(t)
            && ProcessCommonJSModules::is_common_js_dynamic_import_callback(
                t,
                Some(n),
                resolution_mode,
            )
        {
            if self.dynamic_import_scope.is_none() {
                self.dynamic_import_scope = Some(t.get_scope());
            }
        }

        true
    }

    // port: FindModuleDependencies#visit
    #[allow(clippy::collapsible_if)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let resolution_mode = t.get_compiler().get_options().get_module_resolution_mode();
        if NodeUtil::is_shallow_statement_tree(t, parent) {
            if n.is_expr_result(t) {
                let maybe_get_prop = n.get_first_first_child(t);
                if let Some(maybe_get_prop) = maybe_get_prop
                    && (GOOG_PROVIDE.matches(t, maybe_get_prop)
                        || GOOG_MODULE.matches(t, maybe_get_prop))
                {
                    self.module_type = ModuleType::GOOG;
                    return;
                }
            }
        }

        // goog.requireDynamic()
        if NodeUtil::is_goog_require_dynamic_call(t, n) {
            let namespace = n.get_last_child(t).unwrap().get_string(t).to_string_lossy();
            t.get_input()
                .unwrap()
                .clone()
                .add_require_dynamic_imports(namespace);
        }

        if self.supports_es6_modules && n.is_export(t) {
            self.module_type = ModuleType::ES6;
            if n.get_boolean_prop(t, NodeId::EXPORT_DEFAULT) {
                // export default
            } else if n.has_two_children(t) {
                // export * from 'moduleIdentifier';
                // export {x, y as z} from 'moduleIdentifier';
                self.add_es6_module_import_to_graph(t, n);
            }
        } else if self.supports_es6_modules && n.is_import(t) {
            self.module_type = ModuleType::ES6;
            self.add_es6_module_import_to_graph(t, n);
        } else if self.supports_es6_modules
            && n.get_token(t) == Token::DYNAMIC_IMPORT
            && n.get_first_child(t).unwrap().is_string(t)
        {
            let path = n
                .get_first_child(t)
                .unwrap()
                .get_string(t)
                .to_string_lossy();
            let input = t.get_input().unwrap().clone();
            let module_path = input
                .get_path(t.get_compiler())
                .resolve_js_module_silently(&path);
            if let Some(module_path) = module_path {
                input.add_dynamic_require(module_path.to_module_name());
            }
        } else if self.supports_common_js_modules {
            if self.module_type != ModuleType::GOOG
                && ProcessCommonJSModules::is_common_js_export(t, n, resolution_mode)
            {
                self.module_type = ModuleType::COMMONJS;
            } else if ProcessCommonJSModules::is_common_js_import(t, n, resolution_mode) {
                let path = ProcessCommonJSModules::get_common_js_import_path(t, n, resolution_mode)
                    .to_string_lossy();

                let input = t.get_input().unwrap().clone();
                let module_path = input.get_path(t.get_compiler()).resolve_js_module(
                    &path,
                    n.get_source_file_name(t).as_deref(),
                    n.get_lineno(t),
                    n.get_charno(t),
                );
                // The module loader reported to the compiler (its ErrorHandler) during the resolution.
                t.get_compiler().flush_module_errors();

                if let Some(module_path) = module_path {
                    if self.dynamic_import_scope.is_some()
                        || ProcessCommonJSModules::is_common_js_dynamic_import_callback(
                            t,
                            NodeUtil::get_enclosing_function(t, n),
                            resolution_mode,
                        )
                    {
                        input.add_dynamic_require(module_path.to_module_name());
                    } else {
                        input.add_ordered_require(Require::common_js(
                            &module_path.to_module_name(),
                            &path,
                        ));
                    }
                }
            }

            // TODO(ChadKillingsworth) add require.ensure support
        }

        if let Some(parent) = parent
            && (parent.is_expr_result(t) || !t.in_global_hoist_scope())
            && NodeUtil::is_goog_require_call(t, n)
            && n.get_second_child(t).is_some()
            && n.get_second_child(t).unwrap().is_string_lit(t)
        {
            let namespace = n
                .get_second_child(t)
                .unwrap()
                .get_string(t)
                .to_string_lossy();
            let input = t.get_input().unwrap().clone();
            if namespace.starts_with("goog.") {
                input.add_ordered_require((**Require::BASE).clone());
            }
            input.add_ordered_require(Require::goog_require_symbol(&namespace));
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for FindModuleDependencies {
    // port: FindModuleDependencies#enterScope
    fn enter_scope(&mut self, _t: &mut NodeTraversal<'_>) {}

    // port: FindModuleDependencies#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if Some(t.get_scope()) == self.dynamic_import_scope {
            self.dynamic_import_scope = None;
        }
    }
}
