/*
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
//   src/com/google/javascript/jscomp/ModuleImportResolver.java.

//! Resolves module requires into `TypedVar`s.
//!
//! Java's resolver keeps `nodeToScopeMapper` (TypedScopeCreator's `memoized::get`) and the
//! registry as fields. In Rust the registry lives in the compiler (DESIGN §8) and the mapper
//! cannot be a field (it borrows the TypedScopeCreator that owns this resolver), so the methods
//! that use them take the compiler and the mapper per call. Java's mapper is a
//! `Function<Node, TypedScope>` applied to possibly-null scope roots, hence `Option<NodeId>`.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_input::CompilerInput,
    compiler_input_provider::CompilerInputProvider,
    modules::{
        binding::Binding,
        export::Export,
        module::Module,
        module_map::ModuleMap,
        module_metadata_map::{ModuleMetadata, ModuleType},
    },
    node_util::NodeUtil,
    scoped_name::{ScopedName, Simple},
    typed_scope::TypedScope,
};
use closure_jstype::{JSTypeNative, TypeId, object_type::ObjectType};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    node::{Ast, NodeId, Prop},
    qualified_name::QualifiedName,
};
use std::sync::{Arc, LazyLock};

/// Rust-only: Java's `Function<Node, TypedScope> nodeToScopeMapper`.
pub type NodeToScopeMapper<'a> = &'a dyn Fn(Option<NodeId>) -> Option<TypedScope>;

// port: ModuleImportResolver#GOOG
const GOOG: &str = "goog";
// port: ModuleImportResolver#GOOG_DEPENDENCY_CALLS
const GOOG_DEPENDENCY_CALLS: [&str; 4] =
    ["require", "requireType", "forwardDeclare", "requireDynamic"];
// port: ModuleImportResolver#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));

/// Resolves module requires into `TypedVar`s.
///
/// Currently this only supports goog.modules, but can be extended for ES modules.
pub struct ModuleImportResolver {
    module_map: Option<Arc<ModuleMap>>,
}

impl ModuleImportResolver {
    // port: ModuleImportResolver#ModuleImportResolver
    pub fn new(module_map: Option<Arc<ModuleMap>>) -> Self {
        Self { module_map }
    }

    /// Returns whether this is a CALL node for goog.require, goog.requireType,
    /// goog.requireDynamic, goog.forwardDeclare, or goog.module.get.
    ///
    /// This method does not verify that the call is actually in a valid location. For example,
    /// this method does not verify that goog.require calls are at the top-level. That is left to
    /// the caller.
    // port: ModuleImportResolver#isGoogModuleDependencyCall
    pub fn is_goog_module_dependency_call(
        compiler: &AbstractCompiler,
        value: Option<NodeId>,
    ) -> bool {
        let ast = &compiler.ast;
        let Some(value) = value else {
            return false;
        };
        if !value.is_call(ast)
            || !value.has_two_children(ast)
            || !value.get_second_child(ast).unwrap().is_string_lit(ast)
        {
            return false;
        }
        let callee = value.get_first_child(ast).unwrap();
        if !callee.is_get_prop(ast) {
            return false;
        }
        let owner = callee.get_first_child(ast).unwrap();
        (owner.is_name(ast)
            && owner.get_string(ast) == GOOG
            && GOOG_DEPENDENCY_CALLS
                .iter()
                .any(|call| callee.get_string(ast) == *call))
            || GOOG_MODULE_GET.matches(ast, callee)
    }

    /// Attempts to look up the type of a Closure namespace from a require call
    ///
    /// This returns null if the given `ModuleMap` is null, if the required module does not exist,
    /// or if support is missing for the type of required `Module`. Currently only requires of
    /// goog.modules, goog.provides, and ES module with goog.declareModuleId are supported.
    ///
    /// `goog_require`: a CALL node representing some kind of Closure require.
    // port: ModuleImportResolver#getClosureNamespaceTypeFromCall
    pub fn get_closure_namespace_type_from_call(
        &self,
        compiler: &AbstractCompiler,
        goog_require: NodeId,
    ) -> Option<Simple> {
        // TODO(b/124919359): make sure all tests have generated a ModuleMap
        self.module_map.as_ref()?;
        let ast = &compiler.ast;
        let module_id = goog_require.get_second_child(ast).unwrap().get_string(ast);
        self.get_scoped_name_for_closure_namespace(ast, &module_id)
    }

    /// Attempts to look up the type of a Closure namespace from a require call
    ///
    /// `module_id`: a Closure namespace, such as "foo.bar"
    // port: ModuleImportResolver#getScopedNameForClosureNamespace
    fn get_scoped_name_for_closure_namespace(
        &self,
        ast: &Ast,
        module_id: &JsString,
    ) -> Option<Simple> {
        let module = self
            .module_map
            .as_ref()
            .unwrap()
            .get_closure_module(module_id)?;
        match module.metadata().module_type() {
            ModuleType::GOOG_PROVIDE => {
                // Expect this to be a global variable
                let provide = module.metadata().root_node();
                match provide {
                    Some(provide) if provide.is_script(ast) => Some(<dyn ScopedName>::of(
                        module_id.clone(),
                        provide.get_grandparent(ast),
                    )),
                    // Unknown module requires default to 'goog provides', but we don't want to
                    // type them.
                    _ => None,
                }
            }
            ModuleType::GOOG_MODULE | ModuleType::LEGACY_GOOG_MODULE => {
                // TODO(b/124919359): Fix getGoogModuleScopeRoot to never return null.
                let scope_root = Self::get_goog_module_scope_root(ast, Some(module));
                scope_root.map(|scope_root| <dyn ScopedName>::of("exports", Some(scope_root)))
            }
            ModuleType::ES6_MODULE => {
                let module_body = module.metadata().root_node().unwrap().get_first_child(ast); // SCRIPT -> MODULE_BODY
                Some(<dyn ScopedName>::of(Export::NAMESPACE, module_body))
            }
            ModuleType::COMMON_JS => {
                panic!("IllegalStateException: Type checking CommonJs modules not yet supported")
            }
            ModuleType::SCRIPT => {
                panic!("IllegalStateException: Cannot import a name from a SCRIPT")
            }
        }
    }

    /// Returns the corresponding scope root Node from a goog.module.
    // port: ModuleImportResolver#getGoogModuleScopeRoot
    fn get_goog_module_scope_root(ast: &Ast, module: Option<&Arc<Module>>) -> Option<NodeId> {
        let module = module.unwrap();
        if !module.metadata().is_goog_module() {
            panic!("IllegalArgumentException: {:?}", module.metadata());
        }
        let script_node = module.metadata().root_node().unwrap();

        if script_node.is_script(ast)
            && script_node.has_one_child(ast)
            && script_node.get_only_child(ast).is_module_body(ast)
        {
            // The module root node should be a SCRIPT, whose first child is a MODULE_BODY.
            // The map is keyed off a MODULE_BODY node for a goog.module,
            // which is the only child of our SCRIPT node.
            return Some(script_node.get_only_child(ast));
        } else if script_node.is_call(ast) {
            // This is a goog.loadModule call, and the scope is keyed off the FUNCTION node's BLOCK
            // in:
            //   goog.loadModule(function(exports)
            let function_literal = script_node.get_second_child(ast).unwrap();
            return Some(NodeUtil::get_function_body(ast, function_literal));
        }
        // TODO(b/124919359): this case should not happen, but is triggering on goog.require calls
        // in rewritten modules with preserveClosurePrimitives enabled.
        None
    }

    /// Declares/updates the type of all bindings imported into the ES module scope
    ///
    /// Returns a map from local nodes to ScopedNames for which `node_to_scope_mapper` couldn't
    /// find a scope, despite the original module existing. This is expected to happen for
    /// circular references if not all module scopes are created and the caller should handle
    /// declaring these names later, e.g. in TypedScopeCreator.
    // port: ModuleImportResolver#declareEsModuleImports
    pub fn declare_es_module_imports(
        &self,
        compiler: &mut AbstractCompiler,
        node_to_scope_mapper: NodeToScopeMapper<'_>,
        module: &Module,
        scope: TypedScope,
        module_input: Option<CompilerInput>,
    ) -> IndexMap<NodeId, Simple> {
        if !module.metadata().is_es6_module() {
            panic!("IllegalArgumentException: {:?}", module.metadata());
        }
        if !scope.is_module_scope(compiler) {
            panic!("IllegalArgumentException: {}", scope.to_string(compiler));
        }
        let mut missing_names: IndexMap<NodeId, Simple> = IndexMap::<_, _>::default();
        for (local_name, binding) in module.bound_names() {
            if !binding.is_created_by_es_import() {
                continue;
            }
            // ES imports fall into two categories:
            //  - namespace imports. These correspond to an object type containing all named
            //    exports.
            //  - named imports. These always correspond, eventually, to a name local to a module.
            //    Note that we include imports of an `export default` in this case and map them to
            //    a pseudo-variable named *default*.
            let export = self.get_scoped_name_from_es_binding(compiler, binding);
            let source_node = binding.source_node().unwrap();
            let mod_scope = node_to_scope_mapper(export.get_scope_root(compiler));
            let Some(mod_scope) = mod_scope else {
                if source_node.get_string(&compiler.ast) != *local_name {
                    panic!("IllegalStateException: {source_node:?}");
                }
                // ImmutableMap.Builder#buildOrThrow rejects duplicate keys.
                let previous = missing_names.insert(source_node, export);
                check_argument!(previous.is_none(), "Multiple entries with same key");
                continue;
            };

            let original_var = mod_scope
                .get_var(compiler, export.get_name(compiler))
                .unwrap();
            let import_type = original_var.get_type(compiler);
            scope.declare(
                compiler,
                local_name.clone(),
                Some(source_node),
                import_type,
                module_input.clone(),
                /* inferred= */ original_var.is_type_inferred(compiler),
            );

            // Non-namespace imports may be typedefs; if so, propagate the typedef prop onto the
            // export and import bindings, if not already there.
            if !binding.is_module_namespace()
                && source_node.get_typedef_type_prop(&compiler.ast).is_none()
            {
                let typedef_type = original_var
                    .get_name_node(compiler)
                    .unwrap()
                    .get_typedef_type_prop(&compiler.ast);
                if let Some(typedef_type) = typedef_type {
                    source_node.set_typedef_type_prop(&mut compiler.ast, Some(typedef_type));
                    let static_scope = scope.as_static_typed_scope(compiler);
                    let (registry, ast) = compiler.get_type_registry_and_ast();
                    registry.declare_type(
                        ast,
                        Some(static_scope),
                        local_name.clone(),
                        typedef_type,
                    );
                }
            }
        }
        missing_names
    }

    /// Declares or updates the type of properties representing exported names from ES module
    ///
    /// When the given object type does not have existing properties corresponding to exported
    /// names, this method adds new properties to the object type. If the object type already has
    /// properties, this method will ignore declared properties and update the type of inferred
    /// properties.
    ///
    /// The additional properties will be inferred (instead of declared) if and only if
    /// `TypedVar#isTypeInferred()` is true for the original exported name.
    ///
    /// We create this type to support 'import *' and goog.requires of this module. Note: we could
    /// lazily initialize this type if always creating it hurts performance.
    ///
    /// `namespace`: An object type which may already have properties representing exported
    /// names. `scope`: The scope rooted at the given module.
    // port: ModuleImportResolver#updateEsModuleNamespaceType
    pub fn update_es_module_namespace_type(
        &self,
        compiler: &mut AbstractCompiler,
        node_to_scope_mapper: NodeToScopeMapper<'_>,
        namespace: TypeId,
        module: &Module,
        scope: TypedScope,
    ) {
        if !module.metadata().is_es6_module() {
            panic!("IllegalArgumentException: {:?}", module.metadata());
        }
        if !scope.is_module_scope(compiler) {
            panic!("IllegalArgumentException: {}", scope.to_string(compiler));
        }

        for (export_key, binding) in module.namespace() {
            {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                if namespace.is_property_type_declared(registry, ast, export_key.clone()) {
                    // Cannot change the type of a declared property after it is added to the
                    // ObjectType.
                    continue;
                }
            }

            // e.g. 'x' in `export let x;` or `export {x};`
            let binding_source_node = binding.source_node();
            let export = self.get_scoped_name_from_es_binding(compiler, binding);
            let export_scope_root = export.get_scope_root(compiler);
            let original_scope = if export_scope_root == Some(scope.get_root_node(compiler)) {
                Some(scope)
            } else {
                node_to_scope_mapper(export_scope_root)
            };
            let Some(original_scope) = original_scope else {
                // Exporting an import from an invalid module load or early reference.
                let (registry, ast) = compiler.get_type_registry_and_ast();
                let unknown = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
                namespace.define_inferred_property(
                    registry,
                    ast,
                    export_key.clone(),
                    unknown,
                    binding_source_node,
                );
                Self::update_ast_for_export(
                    ast,
                    binding_source_node.unwrap(),
                    unknown,
                    /* typedefType= */ None,
                );
                continue;
            };

            let original_name = original_scope
                .get_slot(compiler, export.get_name(compiler))
                .unwrap();
            let mut export_type = original_name.get_type(compiler);
            let is_type_inferred = original_name.is_type_inferred(compiler);
            let typedef_type = original_name
                .get_name_node(compiler)
                .unwrap()
                .get_typedef_type_prop(&compiler.ast);
            let (registry, ast) = compiler.get_type_registry_and_ast();
            if export_type.is_none() {
                export_type = Some(registry.get_native_type(JSTypeNative::NO_TYPE));
            }
            let export_type = export_type.unwrap();
            if is_type_inferred {
                // NB: this method may be either adding a new inferred property or updating the
                // type of an existing inferred property.
                namespace.define_inferred_property(
                    registry,
                    ast,
                    export_key.clone(),
                    export_type,
                    binding_source_node,
                );
            } else {
                namespace.define_declared_property(
                    registry,
                    ast,
                    export_key.clone(),
                    export_type,
                    binding_source_node,
                );
            }

            Self::update_ast_for_export(
                ast,
                binding_source_node.unwrap(),
                export_type,
                typedef_type,
            );
        }
    }

    // port: ModuleImportResolver#updateAstForExport
    fn update_ast_for_export(
        ast: &mut Ast,
        binding_source_node: NodeId,
        export_type: TypeId,
        typedef_type: Option<TypeId>,
    ) {
        binding_source_node.set_jstype(ast, Some(export_type));
        binding_source_node.set_typedef_type_prop(ast, typedef_type);
        if binding_source_node
            .get_parent(ast)
            .unwrap()
            .is_export_spec(ast)
        {
            // given  `export {x as y} from './a/b'` update the y NAME node
            binding_source_node
                .get_next(ast)
                .unwrap()
                .set_jstype(ast, Some(export_type));
        }
    }

    /// Given a Binding from an ES module, return the name and scope of the bound name.
    // port: ModuleImportResolver#getScopedNameFromEsBinding
    fn get_scoped_name_from_es_binding(
        &self,
        compiler: &AbstractCompiler,
        binding: &Binding,
    ) -> Simple {
        let ast = &compiler.ast;
        // NB: If the original export was an `export default` then the local name is *default*.
        // We've already declared a dummy variable named `*default*` in the scope.
        // Java's binding.boundName() is null for an import of a name from a SCRIPT; the SCRIPT
        // branch below passes it on unchanged.
        let name: Option<JsString> = if binding.is_module_namespace() {
            Some(JsString::from(Export::NAMESPACE))
        } else {
            binding.bound_name().cloned()
        };
        let original_metadata: &Arc<ModuleMetadata> = if binding.is_module_namespace() {
            binding.metadata()
        } else {
            binding.originating_export().unwrap().module_metadata()
        };
        match original_metadata.module_type() {
            ModuleType::ES6_MODULE => {
                let script_node = original_metadata.root_node();
                // Imports of nonexistent modules have a null 'root node'. Imports of names from
                // scripts are meaningless.
                if !script_node.is_none_or(|s| s.is_script(ast)) {
                    panic!("IllegalStateException: {script_node:?}");
                }
                <dyn ScopedName>::of_nullable(name, script_node.map(|s| s.get_only_child(ast)))
            }
            ModuleType::GOOG_MODULE | ModuleType::GOOG_PROVIDE | ModuleType::LEGACY_GOOG_MODULE => {
                let closure_namespace = binding.closure_namespace().unwrap();
                let closure_module_object =
                    self.get_scoped_name_for_closure_namespace(ast, closure_namespace);
                let Some(closure_module_object) = closure_module_object else {
                    // This is an error, but one that should be reported elsewhere. assume the
                    // module is a goog.provide for legacy compatibility.
                    return <dyn ScopedName>::of(closure_namespace.clone(), None);
                };
                if binding.is_module_namespace() {
                    return closure_module_object;
                }
                let qualified = closure_module_object
                    .get_name(compiler)
                    .concat(&JsString::from("."))
                    .concat(binding.originating_export().unwrap().export_name().unwrap());
                <dyn ScopedName>::of(qualified, closure_module_object.get_scope_root(compiler))
            }
            ModuleType::SCRIPT => {
                // Importing SCRIPTs should not allow you to look up names in scope.
                // we also don't really support CommonJs.
                <dyn ScopedName>::of_nullable(name, None)
            }
            ModuleType::COMMON_JS => {
                panic!("IllegalStateException: Typechecking CommonJS modules is not supported")
            }
        }
    }

    /// Returns the `Module` corresponding to this scope root, or null if not a module root.
    // port: ModuleImportResolver#getModuleFromScopeRoot
    pub fn get_module_from_scope_root(
        module_map: Option<&ModuleMap>,
        compiler: &AbstractCompiler,
        module_body: NodeId,
    ) -> Option<Arc<Module>> {
        let ast = &compiler.ast;
        if Self::is_goog_module_body(ast, module_body) {
            let goog_module_call = module_body.get_first_child(ast).unwrap();
            let namespace = goog_module_call
                .get_first_child(ast)
                .unwrap()
                .get_second_child(ast)
                .unwrap()
                .get_string(ast);
            return module_map.unwrap().get_closure_module(&namespace).cloned();
        } else if module_body.is_module_body(ast) {
            let script_node = module_body.get_parent(ast).unwrap();
            let input_id = script_node.get_input_id(ast).unwrap();
            let input = check_not_null!(CompilerInputProvider::get_input(compiler, &input_id));
            let module = module_map
                .unwrap()
                .get_module_by_path(&input.get_path(compiler))
                .unwrap();
            // TODO(b/131418081): Also cover CommonJS modules.
            check_state!(
                module.metadata().is_es6_module(),
                "Typechecking of non-goog- and non-es-modules not supported"
            );
            return Some(Arc::clone(module));
        }
        None
    }

    // port: ModuleImportResolver#isGoogModuleBody
    fn is_goog_module_body(ast: &Ast, module_body: NodeId) -> bool {
        if module_body.is_module_body(ast) {
            return module_body
                .get_parent(ast)
                .unwrap()
                .get_boolean_prop(ast, Prop::GOOG_MODULE);
        } else if module_body.is_block(ast) {
            return module_body.get_parent(ast).unwrap().is_function(ast)
                && NodeUtil::is_bundled_goog_module_call(
                    ast,
                    module_body.get_grandparent(ast).unwrap(),
                );
        }
        false
    }
}
