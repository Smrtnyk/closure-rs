/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ConvertChunksToESModules.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

// Identity keys are immutable even though the associated Java objects are mutable.
#![allow(clippy::mutable_key_type)]
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_input::CompilerInput,
    compiler_pass::CompilerPass,
    deps::module_loader::ModuleLoader,
    diagnostic_type::DiagnosticType,
    js_chunk::JSChunk,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_state,
    ir::IR,
    js_string::JsString,
    node::{NodeId, ObjectProp, Prop},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportType {
    STATIC,
    DYNAMIC,
}

/// Finds all references to global symbols in a different output chunk and add ES Module imports
/// and exports for them.
pub struct ConvertChunksToESModules {
    cross_chunk_exports: IndexMap<JSChunk, IndexSet<JsString>>,
    cross_chunk_imports: IndexMap<JSChunk, IndexMap<JSChunk, IndexSet<JsString>>>,
    dynamic_import_callbacks: Vec<NodeId>,
}

// port: ConvertChunksToESModules#DYNAMIC_IMPORT_CALLBACK_FN
pub const DYNAMIC_IMPORT_CALLBACK_FN: &str = "jscomp$DynamicImportCallback";

// port: ConvertChunksToESModules#ASSIGNMENT_TO_IMPORT
pub static ASSIGNMENT_TO_IMPORT: DiagnosticType = DiagnosticType::error(
    "JSC_IMPORT_ASSIGN",
    "Imported symbol \"{0}\" in chunk \"{1}\" cannot be assigned (defined in \"{2}\")",
);

// port: ConvertChunksToESModules#UNABLE_TO_COMPUTE_RELATIVE_PATH
pub static UNABLE_TO_COMPUTE_RELATIVE_PATH: DiagnosticType = DiagnosticType::error(
    "JSC_UNABLE_TO_COMPUTE_RELATIVE_PATH",
    "Unable to compute relative import path from \"{0}\" to \"{1}\"",
);

// port: ConvertChunksToESModules#UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK
pub static UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK: DiagnosticType = DiagnosticType::error(
    "JSC_UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK",
    "Dynamic import callback encountered wih an invalid format.{0}",
);

impl ConvertChunksToESModules {
    pub const DYNAMIC_IMPORT_CALLBACK_FN: &'static str = DYNAMIC_IMPORT_CALLBACK_FN;

    // port: ConvertChunksToESModules#ConvertChunksToESModules
    pub fn new() -> Self {
        Self {
            cross_chunk_exports: IndexMap::new(),
            cross_chunk_imports: IndexMap::new(),
            dynamic_import_callbacks: Vec::new(),
        }
    }

    // port: ConvertChunksToESModules#convertChunkSourcesToModules
    #[allow(clippy::unnecessary_unwrap)] // Retain Java control flow.
    fn convert_chunk_sources_to_modules(&mut self, compiler: &mut AbstractCompiler) {
        let chunks = compiler
            .get_chunk_graph()
            .unwrap()
            .get_all_chunks()
            .to_vec();
        for chunk in chunks {
            if chunk.get_inputs().is_empty() {
                continue;
            }

            let mut first_input: Option<CompilerInput> = None;
            for input in chunk.get_inputs() {
                let ast_root = input.get_ast_root(compiler);
                let mut script_features = NodeUtil::get_feature_set_of_script(compiler, ast_root)
                    .expect("NullPointerException");
                check_state!(!script_features.contains(FeatureSet::ES2015_MODULES));
                if first_input.is_none() {
                    first_input = Some(input.clone());
                    script_features = script_features.union(FeatureSet::ES2015_MODULES);
                    ast_root.put_prop(
                        compiler,
                        Prop::FEATURE_SET,
                        Some(ObjectProp::Opaque(Arc::new(script_features))),
                    );
                    let module_body = compiler.new_node(Token::MODULE_BODY);
                    module_body.srcref(compiler, ast_root);
                    let children = ast_root.remove_children(compiler);
                    module_body.add_children_to_front(compiler, children);
                    ast_root.add_child_to_front(compiler, module_body);
                    compiler.report_change_to_enclosing_scope(module_body);
                } else {
                    let first_input_ast_root = first_input.as_ref().unwrap().get_ast_root(compiler);
                    let first_input_script_features =
                        NodeUtil::get_feature_set_of_script(compiler, first_input_ast_root)
                            .expect("NullPointerException");
                    let combined_feature_set = first_input_script_features.union(
                        NodeUtil::get_feature_set_of_script(compiler, ast_root)
                            .expect("NullPointerException"),
                    );
                    ast_root.put_prop(
                        compiler,
                        Prop::FEATURE_SET,
                        Some(ObjectProp::Opaque(Arc::new(combined_feature_set))),
                    );
                    let module_body = first_input_ast_root.get_first_child(compiler);
                    check_state!(module_body.is_some_and(|m| m.is_module_body(compiler)));
                    let module_body = module_body.unwrap();
                    let children = ast_root.remove_children(compiler);
                    module_body.add_children_to_back(compiler, children);
                    compiler.report_change_to_enclosing_scope(first_input_ast_root);
                    compiler.report_change_to_change_scope(ast_root);
                }
            }
        }
    }

    /// Add export statements to chunks
    // port: ConvertChunksToESModules#addExportStatements
    fn add_export_statements(&mut self, compiler: &mut AbstractCompiler) {
        for (chunk, names) in &self.cross_chunk_exports {
            let first_input = chunk.get_first();
            let module_body = first_input.get_ast_root(compiler).get_first_child(compiler);
            check_state!(module_body.is_some_and(|m| m.is_module_body(compiler)));
            let module_body = module_body.unwrap();
            let export_specs = compiler.new_node(Token::EXPORT_SPECS);
            for name in names {
                let export_spec = compiler.new_node(Token::EXPORT_SPEC);
                let n1 = IR::name(compiler, name.clone());
                export_spec.add_child_to_front(compiler, n1);
                let n2 = IR::name(compiler, name.clone());
                export_spec.add_child_to_front(compiler, n2);
                export_spec.put_int_prop(compiler, Prop::IS_SHORTHAND_PROPERTY, 1);
                export_specs.add_child_to_back(compiler, export_spec);
            }
            let imports_by_chunk = self.cross_chunk_imports.get(chunk);

            // Force the chunk to parse as a module by adding an empty export spec when no actual
            // static imports or exports exist
            if export_specs.has_children(compiler)
                || imports_by_chunk.is_none()
                || imports_by_chunk.unwrap().is_empty()
            {
                let export = IR::export(compiler, export_specs).srcref_tree(compiler, module_body);
                module_body.add_child_to_back(compiler, export);
                compiler.report_change_to_enclosing_scope(module_body);
            }
        }
    }

    // port: ConvertChunksToESModules#getChunkName
    fn get_chunk_name(chunk: &JSChunk) -> String {
        format!("{}.js", chunk.get_name())
    }

    /// Add import statements to chunks
    // port: ConvertChunksToESModules#addImportStatements
    fn add_import_statements(&mut self, compiler: &mut AbstractCompiler) {
        for (importing_chunk, imports) in &self.cross_chunk_imports {
            let mut import_statements: Vec<NodeId> = Vec::new();
            let first_input = importing_chunk.get_first();
            let module_body = first_input.get_ast_root(compiler).get_first_child(compiler);
            check_state!(module_body.is_some_and(|m| m.is_module_body(compiler)));
            let module_body = module_body.unwrap();

            // For each distinct chunk where a referenced symbol is defined, create an import
            // statement referencing the names.
            for (exporting_chunk, names) in imports {
                let import_specs = compiler.new_node(Token::IMPORT_SPECS);
                for name in names {
                    let import_spec = compiler.new_node(Token::IMPORT_SPEC);
                    let n1 = IR::name(compiler, name.clone());
                    import_spec.add_child_to_front(compiler, n1);
                    let n2 = IR::name(compiler, name.clone());
                    import_spec.add_child_to_front(compiler, n2);
                    import_spec.put_int_prop(compiler, Prop::IS_SHORTHAND_PROPERTY, 1);
                    import_specs.add_child_to_back(compiler, import_spec);
                }
                let import_statement = compiler.new_node(Token::IMPORT);
                let mut import_path = Self::get_chunk_name(exporting_chunk);
                match ModuleLoader::try_relative_path_from(
                    &Self::get_chunk_name(importing_chunk),
                    &Self::get_chunk_name(exporting_chunk),
                ) {
                    Ok(path) => import_path = path,
                    Err(_e) => {
                        compiler.report(JSError::make(
                            compiler,
                            module_body,
                            &UNABLE_TO_COMPUTE_RELATIVE_PATH,
                            &[
                                &Self::get_chunk_name(importing_chunk),
                                &Self::get_chunk_name(exporting_chunk),
                            ],
                        ));
                    }
                }
                let path_node = IR::string(compiler, import_path);
                import_statement.add_child_to_front(compiler, path_node);
                if import_specs.has_children(compiler) {
                    import_statement.add_child_to_front(compiler, import_specs);
                } else {
                    // Empty import of a dependent chunk for side effects
                    // import './chunk.js'
                    let empty = IR::empty(compiler);
                    import_statement.add_child_to_front(compiler, empty);
                }
                let empty = IR::empty(compiler);
                import_statement.add_child_to_front(compiler, empty);

                import_statement.srcref_tree(compiler, module_body);
                import_statements.insert(0, import_statement);
            }
            for import_statement in import_statements {
                module_body.add_child_to_front(compiler, import_statement);
            }
            compiler.report_change_to_enclosing_scope(module_body);
        }
    }

    /// Find and return the module namespace name node in a dynamic import callback function
    // port: ConvertChunksToESModules#getDynamicImportCallbackModuleNamespace
    pub fn get_dynamic_import_callback_module_namespace(
        compiler: &mut AbstractCompiler,
        call: NodeId,
    ) -> Option<NodeId> {
        check_state!(call.is_call(compiler));
        let callback_fn = NodeUtil::get_argument_for_call_or_new(compiler, call, 0);
        if callback_fn.is_none()
            || !callback_fn.unwrap().is_function(compiler)
            || NodeUtil::get_function_parameters(compiler, callback_fn.unwrap())
                .has_children(compiler)
        {
            compiler.report(JSError::make(
                compiler,
                call,
                &UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK,
                &[" Unable to find valid callback function."],
            ));
            return None;
        }
        let callback_body = NodeUtil::get_function_body(compiler, callback_fn.unwrap());

        // The callback body should have a single statement that returns a name.
        // Support both standard and arrow function semantics
        if callback_body.is_name(compiler) {
            return Some(callback_body);
        } else if callback_body.is_block(compiler)
            && callback_body.has_one_child(compiler)
            && callback_body
                .get_first_child(compiler)
                .unwrap()
                .is_return(compiler)
            && callback_body
                .get_first_child(compiler)
                .unwrap()
                .has_one_child(compiler)
            && callback_body
                .get_first_first_child(compiler)
                .unwrap()
                .is_name(compiler)
        {
            return callback_body.get_first_first_child(compiler);
        }
        compiler.report(JSError::make(
            compiler,
            call,
            &UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK,
            &[" Unable to find valid namespace reference."],
        ));
        None
    }

    // port: ConvertChunksToESModules#rewriteDynamicImportCallbacks
    fn rewrite_dynamic_import_callbacks(&mut self, compiler: &mut AbstractCompiler) {
        let ast_factory = compiler.create_ast_factory();
        let dynamic_import_callbacks = self.dynamic_import_callbacks.clone();
        for dynamic_import_callback in dynamic_import_callbacks {
            check_state!(dynamic_import_callback.is_call(compiler));
            let module_namespace = Self::get_dynamic_import_callback_module_namespace(
                compiler,
                dynamic_import_callback,
            );
            let Some(module_namespace) = module_namespace else {
                continue;
            };
            let callback_fn =
                NodeUtil::get_argument_for_call_or_new(compiler, dynamic_import_callback, 0)
                    .unwrap();
            let callback_param_list = NodeUtil::get_function_parameters(compiler, callback_fn);
            let import_namespace_param = ast_factory
                .create_name_with_unknown_type(compiler, "$")
                .srcref(compiler, module_namespace);
            callback_param_list.add_child_to_front(compiler, import_namespace_param);
            compiler.report_change_to_enclosing_scope(import_namespace_param);

            let receiver = ast_factory
                .create_name_with_unknown_type(compiler, "$")
                .srcref(compiler, module_namespace);
            let namespace_name = module_namespace.get_string(compiler);
            let namespace_getprop =
                ast_factory.create_get_prop_with_unknown_type(compiler, receiver, namespace_name);

            module_namespace.replace_with(compiler, namespace_getprop);
            compiler.report_change_to_enclosing_scope(namespace_getprop);
            let inner_callback =
                NodeUtil::get_argument_for_call_or_new(compiler, dynamic_import_callback, 0)
                    .unwrap()
                    .detach(compiler);
            dynamic_import_callback.replace_with(compiler, inner_callback);
            compiler.report_change_to_enclosing_scope(inner_callback);
        }
    }

    /// Test if a node is a .then callback as inserted by the RewriteDynamicImports pass Only finds
    /// callbacks when wrapped in the specially named extern function injected when the chunk
    /// output type is ES_MODULES
    // port: ConvertChunksToESModules#isDynamicImportCallback
    pub fn is_dynamic_import_callback(ast: &closure_rhino::node::Ast, call: NodeId) -> bool {
        if !call.is_call(ast) {
            return false;
        }
        if NodeUtil::is_call_to(ast, call, DYNAMIC_IMPORT_CALLBACK_FN) {
            return true;
        }
        false
    }

    /// Test if a node is a .then callback as inserted by the RewriteDynamicImports pass
    // port: ConvertChunksToESModules#visitCallAndTraverse
    fn visit_call_and_traverse(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) -> bool {
        check_state!(call.is_call(t));
        if !Self::is_dynamic_import_callback(t, call) {
            return true;
        }

        let module_namespace =
            Self::get_dynamic_import_callback_module_namespace(t.get_compiler(), call);
        let Some(module_namespace) = module_namespace else {
            return true;
        };
        let is_valid_module_namespace = self.visit_name(t, module_namespace, ImportType::DYNAMIC);
        if is_valid_module_namespace {
            self.dynamic_import_callbacks.push(call);
        } else {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                call,
                &UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK,
                &[" Unable to find valid namespace reference."],
            ));
        }
        false
    }

    // port: ConvertChunksToESModules#visitName
    fn visit_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        name_node: NodeId,
        import_type: ImportType,
    ) -> bool {
        check_state!(name_node.is_name(t));
        let name = name_node.get_string(t);

        if name.is_empty() {
            return false;
        }

        let s = t.get_scope();
        let v = s.get_var(t.get_compiler(), name.clone());
        let Some(v) = v else {
            return false;
        };
        if !v.is_global(t.get_compiler()) {
            return false;
        }
        let input = v.get_input(t.get_compiler());
        let Some(input) = input else {
            return false;
        };
        let defining_chunk = input.get_chunk();
        let referencing_chunk = t.get_chunk();

        if defining_chunk != referencing_chunk {
            let defining_chunk = defining_chunk.expect("NullPointerException");
            let referencing_chunk = referencing_chunk.expect("NullPointerException");
            if NodeUtil::is_lhs_of_assign(t, name_node) {
                let name_string = name_node.get_string(t).to_string();
                t.report(
                    name_node,
                    &ASSIGNMENT_TO_IMPORT,
                    &[
                        &name_string,
                        &Self::get_chunk_name(&referencing_chunk),
                        &Self::get_chunk_name(&defining_chunk),
                    ],
                );
            }

            // Mark the chunk where the name is declared as needing an export for this name
            let names_to_export = self
                .cross_chunk_exports
                .entry(defining_chunk.clone())
                .or_default();
            names_to_export.insert(name.clone());

            // Add an import for this name to this chunk from the source chunk
            let names_to_import_by_chunk = self
                .cross_chunk_imports
                .entry(referencing_chunk)
                .or_default();
            if import_type == ImportType::STATIC {
                let imports_for_chunk = names_to_import_by_chunk.entry(defining_chunk).or_default();
                imports_for_chunk.insert(name);
            }
        }
        true
    }
}

impl Default for ConvertChunksToESModules {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for ConvertChunksToESModules {
    // port: ConvertChunksToESModules#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // Find global names that are used in more than one chunk. Those that
        // are have to have import and export statements added.
        NodeTraversal::traverse(
            compiler,
            root,
            &mut FindCrossChunkReferences { outer: self },
        );

        // Force every output chunk to parse as an ES Module. If a chunk has no imports and
        // no exports, add an empty export list to generate an empty export statement:
        // example: export {};
        let chunks = compiler
            .get_chunk_graph()
            .unwrap()
            .get_all_chunks()
            .to_vec();
        for chunk in chunks {
            if !self.cross_chunk_exports.contains_key(&chunk)
                && !self.cross_chunk_imports.contains_key(&chunk)
                && !chunk.get_inputs().is_empty()
            {
                self.cross_chunk_exports.insert(chunk, IndexSet::new());
            }
        }

        self.convert_chunk_sources_to_modules(compiler);
        self.add_export_statements(compiler);
        self.add_import_statements(compiler);
        self.rewrite_dynamic_import_callbacks(compiler);

        NodeUtil::add_feature_to_all_scripts(compiler, root, Feature::MODULES);
    }
}

/// Find names in a chunk that are defined in a different chunk.
struct FindCrossChunkReferences<'a> {
    outer: &'a mut ConvertChunksToESModules,
}

impl FindCrossChunkReferences<'_> {
    // port: ConvertChunksToESModules.FindCrossChunkReferences#visitScript
    fn visit_script(&mut self, t: &mut NodeTraversal<'_>, script: NodeId) {
        check_state!(script.is_script(t));
        let chunk = t.get_chunk().expect("NullPointerException");
        let chunk_dependencies = chunk.get_dependencies();

        self.outer
            .cross_chunk_exports
            .entry(chunk.clone())
            .or_default();

        // Ensure every chunk dependency is explicitly listed with an import
        // Dependent chunks may have side effects even if there isn't an explicit name reference
        if !chunk_dependencies.is_empty() {
            let names_to_import_by_chunk = self.outer.cross_chunk_imports.entry(chunk).or_default();
            for dependency in chunk_dependencies {
                names_to_import_by_chunk.entry(dependency).or_default();
            }
        }
    }
}

impl Callback for FindCrossChunkReferences<'_> {
    // port: ConvertChunksToESModules.FindCrossChunkReferences#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            self.visit_script(t, n);
            return true;
        } else if n.is_call(t) {
            return self.outer.visit_call_and_traverse(t, n);
        } else if n.is_name(t) {
            self.outer.visit_name(t, n, ImportType::STATIC);
            return true;
        }
        true
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}
