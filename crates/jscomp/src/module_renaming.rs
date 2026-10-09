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
//   src/com/google/javascript/jscomp/ModuleRenaming.java.

//! Centralized location for determining how to rename modules.
use crate::{
    abstract_compiler::AbstractCompiler,
    ast_factory::AstFactory,
    ast_factory::AstFactoryContext,
    closure_rewrite_module::ClosureRewriteModule,
    modules::{
        binding::Binding,
        export::Export,
        module_map::ModuleMap,
        module_metadata_map::{ModuleMetadata, ModuleType},
    },
    node_util::NodeUtil,
    typed_scope::TypedScope,
};
use closure_rhino::{
    check_argument, check_state, js_string::JsString, jstype::TypeId, node::Ast, node::NodeId,
    qualified_name::QualifiedName,
};

/// Centralized location for determining how to rename modules.
pub struct ModuleRenaming;

impl ModuleRenaming {
    /// The name of the temporary variable created for the default export before globalization of
    /// the module.
    // port: ModuleRenaming#DEFAULT_EXPORT_VAR_PREFIX
    pub const DEFAULT_EXPORT_VAR_PREFIX: &'static str = "$jscompDefaultExport";

    /// Returns the global name of a variable declared in an ES module.
    // port: ModuleRenaming#getGlobalNameOfEsModuleLocalVariable
    pub fn get_global_name_of_es_module_local_variable(
        ast: &Ast,
        module_metadata: &ModuleMetadata,
        variable_name: &JsString,
    ) -> QualifiedName {
        QualifiedName::of(variable_name.concat(&JsString::from("$$")).concat(
            &Self::get_global_name(ast, module_metadata, /* googNamespace= */ None).join(ast),
        ))
    }

    /// Returns the global name of the anonymous default export for the given module.
    // port: ModuleRenaming#getGlobalNameOfAnonymousDefaultExport
    pub fn get_global_name_of_anonymous_default_export(
        ast: &Ast,
        module_metadata: &ModuleMetadata,
    ) -> QualifiedName {
        Self::get_global_name_of_es_module_local_variable(
            ast,
            module_metadata,
            &JsString::from(Self::DEFAULT_EXPORT_VAR_PREFIX),
        )
    }

    /// Returns the global, qualified name to rewrite any references to this module to.
    ///
    /// `goog_namespace` is the Closure namespace that is being referenced fromEsModule this
    /// module, if any.
    // port: ModuleRenaming#getGlobalName(ModuleMetadata,String)
    pub fn get_global_name(
        ast: &Ast,
        module_metadata: &ModuleMetadata,
        goog_namespace: Option<&JsString>,
    ) -> QualifiedName {
        GlobalizedModuleName::create(GlobalTypedScope::None(ast), module_metadata, goog_namespace)
            .alias_name
    }

    /// Returns the post-transpilation, globalized name of the export.
    // port: ModuleRenaming#getGlobalName(Export)
    pub fn get_global_name_of_export(ast: &Ast, export: &Export) -> QualifiedName {
        if export.module_metadata().is_es6_module() {
            if *export.local_name().unwrap() == Export::DEFAULT_EXPORT_NAME {
                return Self::get_global_name_of_anonymous_default_export(
                    ast,
                    export.module_metadata(),
                );
            }
            return Self::get_global_name_of_es_module_local_variable(
                ast,
                export.module_metadata(),
                export.local_name().unwrap(),
            );
        }
        Self::get_global_name(ast, export.module_metadata(), export.closure_namespace())
            .getprop(export.export_name().unwrap().clone())
    }

    /// Returns the post-transpilation, globalized name of the binding.
    // port: ModuleRenaming#getGlobalName(Binding)
    pub fn get_global_name_of_binding(ast: &Ast, binding: &Binding) -> QualifiedName {
        if binding.is_module_namespace() {
            return Self::get_global_name(ast, binding.metadata(), binding.closure_namespace());
        }
        Self::get_global_name_of_export(ast, binding.originating_export().unwrap())
    }

    // port: ModuleRenaming#getNameRootType
    fn get_name_root_type(
        qname: &JsString,
        global_typed_scope: &mut GlobalTypedScope<'_>,
    ) -> Option<TypeId> {
        let GlobalTypedScope::Some(compiler, global_typed_scope) = global_typed_scope else {
            return None;
        };
        let root = NodeUtil::get_root_of_qualified_name_string(qname);
        let var = global_typed_scope
            .get_var(compiler, root.clone())
            .unwrap_or_else(|| {
                panic!(
                    "{}",
                    closure_rhino::jscomp_base::guava_format(
                        "missing var for %s",
                        &[root.to_string()]
                    )
                )
            });
        var.get_type(compiler)
    }

    /// Returns the globalized name of a reference to a binding in JS Doc.
    ///
    /// For example:
    ///
    /// ```text
    ///   // bar
    ///   export class Bar {}
    ///
    ///   // foo
    ///   import * as bar from 'bar';
    ///   export {bar};
    ///
    ///   import * as foo from 'foo';
    ///   let /** !foo.bar.Bar */ b;
    /// ```
    ///
    /// Should call this method with the binding for `foo` and a list ("bar", "Bar"). In this
    /// example any of these properties could also be modules. This method will replace as much as
    /// the GETPROP as it can with module exported variables. Meaning in the above example this
    /// would return something like "baz$$module$bar", whereas if this method were called for just
    /// "foo.bar" it would return "module$bar", as it refers to a module object itself.
    // port: ModuleRenaming#getGlobalNameForJsDoc
    pub fn get_global_name_for_js_doc(
        ast: &Ast,
        module_map: &ModuleMap,
        binding: &Binding,
        property_chain: &[JsString],
    ) -> JsString {
        let mut binding = binding.clone();
        let mut prop = 0;
        while binding.is_module_namespace()
            && binding.metadata().is_es6_module()
            && prop < property_chain.len()
        {
            let property_name = &property_chain[prop];
            let m = module_map
                .get_module_by_path(binding.metadata().path().unwrap())
                .unwrap();
            if m.namespace().contains_key(property_name) {
                binding = m.namespace().get(property_name).unwrap().clone();
            } else {
                // This means someone referenced an invalid export on a module object. This should
                // be an error, so just rewrite and let the type checker complain later. It isn't a
                // super clear error, but we're working on type checking modules soon.
                break;
            }
            prop += 1;
        }

        let mut global_name = Self::get_global_name_of_binding(ast, &binding);
        while prop < property_chain.len() {
            global_name = global_name.getprop(property_chain[prop].clone());
            prop += 1;
        }
        global_name.join(ast)
    }
}

/// Stores a fully qualified globalized name of a module or provide + the type of its root node.
///
/// For goog.provides and legacy goog.modules, the fully qualified name is just the namespace of
/// the module "a.b.c.d". For ES modules, it's deterministically generated based on the module
/// path, e.g. "module$path$to$my$dir". For non-legacy goog.modules, it's deterministically
/// generated based on the module name, e.g. $module$exports$a$b$c$d".
///
/// Used because in some cases, the root of the qualified name will not be present in any scopes
/// until a subsequent run of `ClosureRewriteModule`. It's useful to store the type of Closure
/// module exports `module$exports$my$goog$module` separately.
///
/// `root_name_type` is the type of the root of `alias_name`, as it may not always exist in the
/// scope yet.
// port: ModuleRenaming.GlobalizedModuleName
#[derive(Clone, Debug)]
pub struct GlobalizedModuleName {
    alias_name: QualifiedName,
    root_name_type: Option<TypeId>,
}

impl GlobalizedModuleName {
    // port: ModuleRenaming.GlobalizedModuleName#aliasName
    pub fn alias_name(&self) -> &QualifiedName {
        &self.alias_name
    }

    // port: ModuleRenaming.GlobalizedModuleName#rootNameType
    pub fn root_name_type(&self) -> Option<TypeId> {
        self.root_name_type
    }

    /// Creates a GETPROP chain with type information representing this name
    // port: ModuleRenaming.GlobalizedModuleName#toQname
    pub fn to_qname<C: AstFactoryContext + ?Sized>(
        &self,
        cx: &mut C,
        ast_factory: &AstFactory,
    ) -> NodeId {
        let (root, is_simple, components) = {
            let ast: &Ast = cx.get_type_registry_and_ast_mut().1;
            (
                self.alias_name().get_root(ast),
                self.alias_name().is_simple(ast),
                self.alias_name().components(ast),
            )
        };
        let root_name =
            ast_factory.create_name(cx, root, AstFactory::type_jstype(self.root_name_type()));
        if is_simple {
            return root_name;
        }
        // Iterables.skip(this.aliasName().components(), 1)
        let rest: Vec<String> = components.iter().skip(1).map(ToString::to_string).collect();
        let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
        ast_factory.create_get_props_without_colors(cx, root_name, &rest)
    }

    /// Returns a copy of this name with the given `property` appended to the `alias_name`
    // port: ModuleRenaming.GlobalizedModuleName#getprop
    pub fn getprop(&self, property: &JsString) -> GlobalizedModuleName {
        check_argument!(!property.is_empty() && property.index_of(".") < 0);
        GlobalizedModuleName::create_from_name(
            self.alias_name().getprop(property.clone()),
            self.root_name_type(),
        )
    }

    // port: ModuleRenaming.GlobalizedModuleName#create(QualifiedName,JSType)
    pub fn create_from_name(
        alias_name: QualifiedName,
        root_name_type: Option<TypeId>,
    ) -> GlobalizedModuleName {
        GlobalizedModuleName {
            alias_name,
            root_name_type,
        }
    }

    /// Returns the global, qualified name to rewrite any references to this module to, along with
    /// the type of the root of the module if `global_typed_scope` is not null.
    ///
    /// `goog_namespace` is the Closure namespace that is being referenced fromEsModule this
    /// module, if any; `global_typed_scope` is a global scope expected to contain the types of
    /// goog.provided names and rewritten ES6 module names.
    // port: ModuleRenaming.GlobalizedModuleName#create(ModuleMetadata,String,TypedScope)
    pub fn create(
        mut global_typed_scope: GlobalTypedScope<'_>,
        module_metadata: &ModuleMetadata,
        goog_namespace: Option<&JsString>,
    ) -> GlobalizedModuleName {
        check_state!(
            goog_namespace.is_none_or(|ns| module_metadata.goog_namespaces().contains(ns))
        );
        match module_metadata.module_type() {
            ModuleType::GOOG_MODULE => {
                // The exported type is stored on the MODULE_BODY node.
                let module_body = module_metadata
                    .root_node()
                    .unwrap()
                    .get_first_child(global_typed_scope.ast())
                    .unwrap();
                return GlobalizedModuleName {
                    alias_name: QualifiedName::of(
                        ClosureRewriteModule::get_binary_module_namespace(goog_namespace.unwrap()),
                    ),
                    root_name_type: module_body.get_jstype(global_typed_scope.ast()),
                };
            }
            ModuleType::GOOG_PROVIDE | ModuleType::LEGACY_GOOG_MODULE => {
                let goog_namespace = goog_namespace.unwrap();
                return GlobalizedModuleName {
                    alias_name: QualifiedName::of(goog_namespace.clone()),
                    root_name_type: ModuleRenaming::get_name_root_type(
                        goog_namespace,
                        &mut global_typed_scope,
                    ),
                };
            }
            ModuleType::ES6_MODULE | ModuleType::COMMON_JS => {
                let module_name = JsString::from(module_metadata.path().unwrap().to_module_name());
                return GlobalizedModuleName {
                    alias_name: QualifiedName::of(module_name.clone()),
                    root_name_type: ModuleRenaming::get_name_root_type(
                        &module_name,
                        &mut global_typed_scope,
                    ),
                };
            }
            ModuleType::SCRIPT => {}
        }
        panic!(
            "java.lang.IllegalStateException: Unexpected module type: {}",
            module_metadata.module_type().name()
        );
    }
}

/// Rust-only: Java's `@Nullable TypedScope globalTypedScope` argument of
/// `GlobalizedModuleName#create`. A TypedScope's vars live in the compiler, so a non-null scope
/// comes with the compiler (which also owns the AST); a null scope comes with the AST alone.
pub enum GlobalTypedScope<'a> {
    /// `globalTypedScope == null`.
    None(&'a Ast),
    /// A global TypedScope expected to contain the types of goog.provided names and rewritten ES6
    /// module names.
    Some(&'a mut AbstractCompiler, TypedScope),
}

impl GlobalTypedScope<'_> {
    fn ast(&self) -> &Ast {
        match self {
            GlobalTypedScope::None(ast) => ast,
            GlobalTypedScope::Some(compiler, _) => &compiler.ast,
        }
    }
}
