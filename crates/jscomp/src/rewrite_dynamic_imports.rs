/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteDynamicImports.java.

//! Rewrite dynamic import expressions to account for bundling and module rewriting. Since dynamic
//! imports cannot be fully polyfilled, optionally support replacing the expression with a function
//! call (alias function) which indicates an external polyfill is utilized.
//!
//! If the import specifier is a string literal and the module resolver recognizes the target, the
//! pass retargets the specifier to the correct output chunk.
//!
//! Unlike other "rewrite" passes for which the DefaultPassConfig checks the output language level
//! using `options.needsTranspilationOf(feature)` before adding a pass, it does not check the
//! language level before adding this particular pass. Even when supported by the output level, we
//! still invoke this pass if `options.shouldAllowDynamicImport()` is set. The only difference is
//! that for `ES2020`, the dynamic imports syntax can pass through if aliasing is not requested,
//! while for `< ES2020` they must get rewritten using the alias call.
//!
//! TODO: b/291319705 For the above reason, the pass should be better named as
//! MaybeRewriteDynamicImports.
use crate::{
    abstract_compiler::AbstractCompiler,
    ast_factory::AstFactory,
    compiler_options::ChunkOutputType,
    compiler_pass::CompilerPass,
    convert_chunks_to_es_modules::{DYNAMIC_IMPORT_CALLBACK_FN, UNABLE_TO_COMPUTE_RELATIVE_PATH},
    deps::module_loader::ModuleLoader,
    diagnostic_type::DiagnosticType,
    js_chunk::JSChunk,
    js_error::JSError,
    module_renaming::{GlobalTypedScope, GlobalizedModuleName},
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    var::VarId,
};
use closure_jstype::{js_type_native::JSTypeNative, prelude::*};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_argument, check_not_null, check_state, js_string::JsString, node::NodeId, token::Token,
};

// port: RewriteDynamicImports#DYNAMIC_IMPORT_ALIASING_REQUIRED
pub static DYNAMIC_IMPORT_ALIASING_REQUIRED: DiagnosticType = DiagnosticType::warning(
    "JSC_DYNAMIC_IMPORT_ALIASING_REQUIRED",
    "Dynamic import expressions should be aliased for language level. Use the --dynamic_import_alias flag.",
);

// port: RewriteDynamicImports#DYNAMIC_IMPORT_INVALID_ALIAS
pub static DYNAMIC_IMPORT_INVALID_ALIAS: DiagnosticType = DiagnosticType::error(
    "JSC_DYNAMIC_IMPORT_INVALID_ALIAS",
    "Dynamic import alias is not a valid name",
);

// port: RewriteDynamicImports
pub struct RewriteDynamicImports {
    ast_factory: AstFactory,
    alias: Option<JsString>,
    requires_aliasing: bool,
    should_wrap_dynamic_import_callbacks: bool,
    dynamic_imports_removed: bool,
    wrapped_dynamic_import_callback: bool,
}

impl RewriteDynamicImports {
    /// `compiler` The compiler
    // port: RewriteDynamicImports#RewriteDynamicImports
    pub fn new(
        compiler: &mut AbstractCompiler,
        alias: Option<&str>,
        chunk_output_type: ChunkOutputType,
    ) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            alias: alias.map(JsString::from),
            requires_aliasing: !compiler
                .get_options()
                .get_output_feature_set()
                .contains(Feature::DYNAMIC_IMPORT),
            should_wrap_dynamic_import_callbacks: chunk_output_type == ChunkOutputType::ES_MODULES,
            dynamic_imports_removed: false,
            wrapped_dynamic_import_callback: false,
        }
    }

    // port: RewriteDynamicImports#aliasIsValid
    fn alias_is_valid(&self, compiler: &AbstractCompiler) -> bool {
        self.alias.as_ref().is_some_and(|alias| {
            *alias == *"import"
                || NodeUtil::is_valid_qualified_name_features(
                    compiler.get_allowable_features(),
                    alias,
                )
        })
    }

    /// Replace a dynamic import expression with a promise resolving to the rewritten module
    /// namespace.
    ///
    /// Before `import('./foo.js');` After `Promise.resolve(module$foo);`
    // port: RewriteDynamicImports#replaceDynamicImportWithPromise
    fn replace_dynamic_import_with_promise(
        &mut self,
        t: &mut NodeTraversal<'_>,
        dynamic_import: NodeId,
        target_module_ns: VarId,
    ) {
        let compiler = t.get_compiler();
        let namespace = TranspilationNamespace::get(compiler);
        let promise_dot_resolve =
            self.ast_factory
                .create_qname(compiler, &namespace, "Promise.resolve");
        let promise_resolve_call = self.ast_factory.create_call(
            compiler,
            promise_dot_resolve,
            AstFactory::type_node(dynamic_import),
            &[],
        );
        let is_expression_used = NodeUtil::is_expression_result_used(compiler, dynamic_import);
        if is_expression_used {
            let module_namespace_node =
                Self::create_module_namespace_node(compiler, target_module_ns);
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let dynamic_import_type = dynamic_import.get_jstype(ast);
            let module_namespace_type = match dynamic_import_type {
                Some(ty) if ty.is_templatized_type(registry) => {
                    let templatized_type = ty;
                    templatized_type.get_template_types(registry).unwrap()[0]
                }
                _ => registry.get_native_type(JSTypeNative::UNKNOWN_TYPE),
            };
            let promise_type = registry.get_native_object_type(JSTypeNative::PROMISE_TYPE);
            let templatized =
                registry.create_templatized_type(ast, promise_type, &[module_namespace_type]);
            let all_type = registry.get_native_type(JSTypeNative::ALL_TYPE);
            let function_type = registry.create_function_type(ast, templatized, &[all_type]);
            promise_dot_resolve.set_jstype(ast, Some(function_type));
            promise_resolve_call.add_child_to_back(ast, module_namespace_node);
            promise_resolve_call.copy_type_from(ast, dynamic_import);
        } else {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let promise_type = registry.get_native_object_type(JSTypeNative::PROMISE_TYPE);
            let void_type = registry.get_native_type(JSTypeNative::VOID_TYPE);
            let templatized = registry.create_templatized_type(ast, promise_type, &[void_type]);
            let all_type = registry.get_native_type(JSTypeNative::ALL_TYPE);
            let function_type = registry.create_function_type(ast, templatized, &[all_type]);
            promise_dot_resolve.set_jstype(ast, Some(function_type));
            let promise_type = registry.get_native_object_type(JSTypeNative::PROMISE_TYPE);
            let void_type = registry.get_native_type(JSTypeNative::VOID_TYPE);
            let templatized = registry.create_templatized_type(ast, promise_type, &[void_type]);
            promise_resolve_call.set_jstype(ast, Some(templatized));
        }
        promise_resolve_call.srcref_tree(t, dynamic_import);
        let parent = dynamic_import.get_parent(t).unwrap();
        dynamic_import.replace_with(t, promise_resolve_call);
        t.report_code_change_at_node(parent);
        self.dynamic_imports_removed = true;
    }

    // port: RewriteDynamicImports#getChunkFileName
    fn get_chunk_file_name(chunk: &JSChunk) -> String {
        format!("{}.js", chunk.get_name())
    }

    /// Rewrite the dynamic import specifier to the output chunk path
    ///
    /// Before `import('./foo.js');` After `import('./chunk0.js');`
    // port: RewriteDynamicImports#retargetImportSpecifier
    fn retarget_import_specifier(
        &mut self,
        t: &mut NodeTraversal<'_>,
        dynamic_import: NodeId,
        target_chunk: &JSChunk,
    ) {
        let importing_chunk = t.get_input().unwrap().get_chunk();
        let importing_chunk_filename = Self::get_chunk_file_name(&check_not_null!(importing_chunk));
        let target_chunk_filename = Self::get_chunk_file_name(target_chunk);
        let retargeted_specifier = match ModuleLoader::try_relative_path_from(
            &importing_chunk_filename,
            &target_chunk_filename,
        ) {
            Ok(retargeted_specifier) => retargeted_specifier,
            Err(_) => {
                let error = JSError::make(
                    t,
                    dynamic_import,
                    &UNABLE_TO_COMPUTE_RELATIVE_PATH,
                    &[&importing_chunk_filename, &target_chunk_filename],
                );
                t.get_compiler().report(error);
                return;
            }
        };
        let original_specifier_node = dynamic_import.get_first_child(t).unwrap();
        let new_specifier = self
            .ast_factory
            .create_string(t.get_compiler(), retargeted_specifier.as_str())
            .srcref(t, original_specifier_node);
        original_specifier_node.replace_with(t, new_specifier);
        t.report_code_change_at_node(new_specifier);
    }

    /// Since a dynamic import expression is a promise resolving to the namespace export type, add
    /// a ".then()" call after it and resolve to the rewritten module namespace.
    ///
    /// Before `import('./foo.js');` After `import('./foo.js').then(() => module$foo);`
    // port: RewriteDynamicImports#addChainedThen
    fn add_chained_then(
        &mut self,
        t: &mut NodeTraversal<'_>,
        dynamic_import: NodeId,
        target_module_ns: VarId,
    ) {
        let compiler = t.get_compiler();
        let import_parent = dynamic_import.get_parent(compiler).unwrap();
        let placeholder = closure_rhino::ir::IR::empty(compiler);
        dynamic_import.replace_with(compiler, placeholder);
        let module_namespace_node = Self::create_module_namespace_node(compiler, target_module_ns);
        let callback_fn = self
            .ast_factory
            .create_zero_arg_arrow_function_for_expression(compiler, module_namespace_node);
        {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let promise_type = registry.get_native_object_type(JSTypeNative::PROMISE_TYPE);
            let unknown_type = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
            let templatized = registry.create_templatized_type(ast, promise_type, &[unknown_type]);
            let all_type = registry.get_native_type(JSTypeNative::ALL_TYPE);
            let all_fn = registry.create_function_type(ast, all_type, &[]);
            let function_type = registry.create_function_type(ast, templatized, &[all_fn]);
            callback_fn.set_jstype(ast, Some(function_type));
        }
        let mut then_argument = callback_fn;
        if self.should_wrap_dynamic_import_callbacks {
            let wrapping_type = {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                let callback_type = callback_fn.get_jstype(ast).unwrap();
                registry.create_function_type(ast, callback_type, &[callback_type])
            };
            let wrapping_function = self.ast_factory.create_name(
                compiler,
                DYNAMIC_IMPORT_CALLBACK_FN,
                AstFactory::type_jstype(Some(wrapping_type)),
            );
            then_argument = self.ast_factory.create_call(
                compiler,
                wrapping_function,
                AstFactory::type_node(callback_fn),
                &[callback_fn],
            );
            self.wrapped_dynamic_import_callback = true;
        }
        let then_getprop =
            self.ast_factory
                .create_get_prop_with_unknown_type(compiler, dynamic_import, "then");
        let import_then_call = self.ast_factory.create_call(
            compiler,
            then_getprop,
            AstFactory::type_node(dynamic_import),
            &[then_argument],
        );
        import_then_call.srcref_tree_if_missing(compiler, dynamic_import);
        if dynamic_import.get_jstype(compiler).is_some() {
            import_then_call.copy_type_from(compiler, dynamic_import);
        }
        placeholder.replace_with(compiler, import_then_call);
        let current_script = t.get_current_script().unwrap();
        let compiler = t.get_compiler();
        NodeUtil::add_feature_to_script(compiler, current_script, Feature::ARROW_FUNCTIONS);
        compiler.report_change_to_change_scope(callback_fn);
        compiler.report_change_to_enclosing_scope(import_parent);
    }

    /// Replace a dynamic import expression with a function call to the specified alias.
    ///
    /// Before `import('./foo.js');` After `aliasedName('./foo.js');`
    // port: RewriteDynamicImports#aliasDynamicImport
    fn alias_dynamic_import(&mut self, t: &mut NodeTraversal<'_>, dynamic_import: NodeId) {
        let alias = check_not_null!(self.alias.as_ref()).to_string_lossy();
        let compiler = t.get_compiler();
        let alias_node = self
            .ast_factory
            .create_qname_with_unknown_type(compiler, &alias);
        alias_node.set_original_name(compiler, Some("import".into()));
        let module_specifier = dynamic_import.remove_first_child(compiler).unwrap();
        let import_alias_call = self
            .ast_factory
            .create_call(
                compiler,
                alias_node,
                AstFactory::type_node(dynamic_import),
                &[module_specifier],
            )
            .srcref_tree_if_missing(compiler, dynamic_import);
        if dynamic_import.get_jstype(compiler).is_some() {
            import_alias_call.copy_type_from(compiler, dynamic_import);
        }
        dynamic_import.replace_with(compiler, import_alias_call);
        t.report_code_change_at_node(import_alias_call);
        self.dynamic_imports_removed = true;
    }

    /// For a given module, return a reference to the module namespace export
    // port: RewriteDynamicImports#createModuleNamespaceNode
    fn create_module_namespace_node(compiler: &mut AbstractCompiler, module_var: VarId) -> NodeId {
        let module_var_node = module_var.get_node(compiler).unwrap();
        let module_namespace = module_var_node.clone_node(compiler);
        module_namespace.copy_type_from(compiler, module_var_node);
        module_namespace
    }

    /// For a given module, return a reference to the module namespace export
    // port: RewriteDynamicImports#injectWrappingFunctionExtern
    fn inject_wrapping_function_extern(&mut self, compiler: &mut AbstractCompiler) {
        let function_type = {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let template_t = registry.create_template_type(ast, "T");
            registry.create_function_type(ast, template_t, &[template_t])
        };
        let params = self
            .ast_factory
            .create_param_list(compiler, &["importCallback"]);
        let body = self.ast_factory.create_block(compiler, &[]);
        let wrapping_function_definition = self.ast_factory.create_function(
            compiler,
            DYNAMIC_IMPORT_CALLBACK_FN,
            params,
            body,
            AstFactory::type_jstype(Some(function_type)),
        );

        let externs_input = compiler.get_synthesized_externs_input().clone();
        let externs_root = externs_input.get_ast_root(compiler);
        wrapping_function_definition.srcref_tree(compiler, externs_root);
        externs_root.add_child_to_back(compiler, wrapping_function_definition);
        compiler.report_change_to_enclosing_scope(wrapping_function_definition);
    }
}

impl CompilerPass for RewriteDynamicImports {
    // port: RewriteDynamicImports#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.dynamic_imports_removed = false;
        check_argument!(externs.is_root(compiler), "%s", externs.to_string(compiler));
        check_argument!(root.is_root(compiler), "%s", root.to_string(compiler));

        NodeTraversal::traverse(compiler, root, self);
        if self.wrapped_dynamic_import_callback {
            self.inject_wrapping_function_extern(compiler);
        }

        if self.dynamic_imports_removed {
            compiler.mark_feature_not_allowed(Feature::DYNAMIC_IMPORT);

            // This pass removes dynamic import, but adds arrow functions.
            // It is unusual to call this NodeUtil method instead of the TranspilationPasses one.
            // This pass runs as a stage1 checks pass and does not have access to {@code
            // TranspilationPasses}. Adding that dep produces a cycle in the BUILD dep graph.
            // Regular transpiler passes must use the {@code
            // TranspilationPassses.maybeMarkFeaturesAsTranspiledAway}.
            NodeUtil::remove_feature_from_all_scripts(compiler, root, Feature::DYNAMIC_IMPORT);
            if self.requires_aliasing && self.alias_is_valid(compiler) {
                NodeTraversal::traverse(
                    compiler,
                    externs,
                    &mut AliasInjectingTraversal { outer: self },
                );
            }
        }
    }
}

impl Callback for RewriteDynamicImports {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RewriteDynamicImports#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.get_token(t) != Token::DYNAMIC_IMPORT {
            return;
        }

        // If the module specifier is a string, attempt to resolve the module
        let module_map = t.get_compiler().get_module_map().cloned();
        let import_specifier = n.get_first_child(t).unwrap();
        if import_specifier.is_string_lit(t)
            && let Some(module_map) = module_map
        {
            let input = t.get_input().unwrap().clone();
            let path = input.get_path(t.get_compiler());
            let specifier = import_specifier.get_string(t).to_string_lossy();
            let source_file_name = n.get_source_file_name(t);
            let lineno = n.get_lineno(t);
            let charno = n.get_charno(t);
            let target_path =
                path.resolve_js_module(&specifier, source_file_name.as_deref(), lineno, charno);
            // The module loader reported to the compiler (its ErrorHandler) during the resolution.
            t.get_compiler().flush_module_errors();
            let module = target_path
                .as_ref()
                .and_then(|target_path| module_map.get_module_by_path(target_path).cloned());

            let target_module_var_name = module.map(|module| {
                GlobalizedModuleName::create(
                    GlobalTypedScope::None(t.get_compiler()),
                    module.metadata(),
                    None,
                )
                .alias_name()
                .join(t)
            });
            let target_module_ns = target_module_var_name.and_then(|name| {
                let scope = t.get_scope();
                scope.get_var(t.get_compiler(), &name)
            });

            if let Some(target_module_ns) = target_module_ns {
                let target_chunk = target_module_ns
                    .get_input(t.get_compiler())
                    .unwrap()
                    .get_chunk();
                // If the target module is bundled into the same output chunk, replace the import
                // statement with a promise that resolves to the module namespace.
                // No further rewriting occurs for this case.
                if t.get_chunk() == target_chunk {
                    self.replace_dynamic_import_with_promise(t, n, target_module_ns);
                    return;
                } else {
                    // The target output chunk is recognized and different from the current chunk.
                    // Retarget the import specifier path to the output chunk path and rewrite
                    // the import to reference the rewritten global module namespace variable.
                    self.retarget_import_specifier(t, n, &check_not_null!(target_chunk));
                    if NodeUtil::is_expression_result_used(t, n) {
                        self.add_chained_then(t, n, target_module_ns);
                    }
                }
            }
        }
        if self.alias_is_valid(t.get_compiler()) {
            self.alias_dynamic_import(t, n);
        } else if self.alias.is_some() {
            t.report(n, &DYNAMIC_IMPORT_INVALID_ALIAS, &[]);
        } else if self.requires_aliasing {
            t.report(n, &DYNAMIC_IMPORT_ALIASING_REQUIRED, &[]);
        }
    }
}

/// A shallow traversal class for the externs to inject the alias.
///
/// Later passes require the names to be defined. For simple name aliases first check for an
/// existing definition. Qualified name aliases check for an existing definition of the root name
/// only.
// port: RewriteDynamicImports.AliasInjectingTraversal
struct AliasInjectingTraversal<'a> {
    outer: &'a RewriteDynamicImports,
}

impl Callback for AliasInjectingTraversal<'_> {
    // port: RewriteDynamicImports.AliasInjectingTraversal#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        check_state!(self.outer.alias_is_valid(t.get_compiler()));
        let alias = self.outer.alias.clone().unwrap();
        let ast_factory = &self.outer.ast_factory;

        // If the extern is a simple name and already declared, there is nothing to do
        if NodeUtil::is_valid_simple_name(&alias) {
            let scope = t.get_scope();
            if scope.get_var(t.get_compiler(), &alias).is_some() {
                return false;
            }
        }

        // Create the type of the import function
        let (promise_templatized_type, string_type) = {
            let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
            let unknown_type = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
            let promise_type = registry.get_native_type(JSTypeNative::PROMISE_TYPE);
            let promise_object_type = promise_type.to_object_type(registry).unwrap();
            let promise_templatized_type =
                registry.create_templatized_type(ast, promise_object_type, &[unknown_type]);
            let string_type = registry.get_native_type(JSTypeNative::STRING_TYPE);
            (promise_templatized_type, string_type)
        };

        let compiler = t.get_compiler();
        let externs_input = compiler.get_synthesized_externs_input().clone();
        let externs_root = externs_input.get_ast_root(compiler);
        // "import" is not a valid JS name and will not parse. However we can manually create it
        // and inject it into the externs.
        if alias == *"import" || NodeUtil::is_valid_simple_name(&alias) {
            let function_type = {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                registry.create_function_type(ast, promise_templatized_type, &[string_type])
            };
            let params = ast_factory.create_param_list(compiler, &["specifier"]);
            let body = ast_factory.create_block(compiler, &[]);
            let alias_node = ast_factory.create_function(
                compiler,
                alias.clone(),
                params,
                body,
                AstFactory::type_jstype(Some(function_type)),
            );
            alias_node.srcref_tree(compiler, externs_root);
            externs_root.add_child_to_back(compiler, alias_node);
        } else {
            let alias_root_name = NodeUtil::get_root_of_qualified_name_string(&alias);
            let scope = t.get_scope();
            let alias_var = scope.get_var(t.get_compiler(), &alias_root_name);
            // If the namespace root is defined, just assume the whole name is properly defined.
            // This may need revisited in the future, but checking if the full qualified name
            // is properly defined is difficult here. As long as the root of the qualified name is
            // defined, other passes seem be ok even with missing properties.
            if alias_var.is_some() {
                return false;
            }
            let compiler = t.get_compiler();
            let object_lit = ast_factory.create_object_lit(compiler, &[]);
            let alias_root_node = ast_factory.create_single_var_name_declaration_with_value(
                compiler,
                alias_root_name,
                object_lit,
            );
            alias_root_node.srcref_tree(compiler, externs_root);
            externs_root.add_child_to_back(compiler, alias_root_node);

            // Define the rest of the parts of the name
            let alias_str = alias.to_string_lossy();
            let mut alias_name_parts = alias_str.split('.').peekable();
            let mut q_name = alias_root_node
                .get_first_child(compiler)
                .unwrap()
                .clone_node(compiler);
            alias_name_parts.next(); // skip over root name
            while let Some(part) = alias_name_parts.next() {
                let receiver = q_name.clone_tree(compiler);
                q_name = ast_factory.create_get_prop_with_unknown_type(compiler, receiver, part);
                let assigned_value = if alias_name_parts.peek().is_none() {
                    let function_type = {
                        let (registry, ast) = compiler.get_type_registry_and_ast();
                        registry.create_function_type(ast, promise_templatized_type, &[string_type])
                    };
                    let params = ast_factory.create_param_list(compiler, &["specifier"]);
                    let body = ast_factory.create_block(compiler, &[]);
                    ast_factory.create_function(
                        compiler,
                        "",
                        params,
                        body,
                        AstFactory::type_jstype(Some(function_type)),
                    )
                } else {
                    ast_factory.create_object_lit(compiler, &[])
                };
                let assign = ast_factory.create_assign(compiler, q_name, assigned_value);
                let expr = ast_factory.expr_result(compiler, assign);
                expr.srcref_tree(compiler, externs_root);
                externs_root.add_child_to_back(compiler, expr);
            }
        }
        t.get_compiler().report_change_to_change_scope(externs_root);
        false
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}
