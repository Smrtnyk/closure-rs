/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/TypedScopeCreator.java.

//! Creates the symbol table of types. TypedScopeCreator builds the TypedScopes of a program: it
//! declares the names of each scope with their declared types, builds the function, class and enum
//! types from their declarations, and declares named types in the type registry.
// Preserve Java branch structure.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::if_same_then_else,
    clippy::needless_late_init,
    clippy::unnecessary_unwrap
)]

use crate::abstract_compiler::AbstractCompiler;
use crate::abstract_scope::AbstractScopeHandle;
use crate::closure_coding_convention::ClosureCodingConvention;
use crate::coding_convention::CodingConvention;
use crate::compiler_input::CompilerInput;
use crate::destructured_target::{DestructuredTarget, PatternTypeSupplier};
use crate::diagnostic_type::DiagnosticType;
use crate::function_type_builder::{AstFunctionContents, FunctionContents, FunctionTypeBuilder};
use crate::js_error::JSError;
use crate::memoized_scope_creator::MemoizedScopeCreator;
use crate::module_import_resolver::ModuleImportResolver;
use crate::modules::export::Export;
use crate::modules::module::Module;
use crate::modules::module_map::ModuleMap;
use crate::modules::module_metadata_map::{ModuleMetadataMap, ModuleType};
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::process_closure_provides_and_requires::{
    ProcessClosureProvidesAndRequires, ProvidedName,
};
use crate::scope::as_static_scope;
use crate::scope_creator::ASSERT_NO_SCOPES_CREATED;
use crate::scoped_name::{ScopedName, Simple};
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use crate::type_check::{MULTIPLE_VAR_DEF, TypeCheck};
use crate::type_validator::TypeValidator;
use crate::typed_scope::TypedScope;
use crate::typed_var::TypedVar;
use closure_jstype::{
    JSTypeNative, TypeId,
    known_symbol_type::KnownSymbolType,
    object_type,
    prelude::*,
    property::{Property, PropertyKey},
    rhino::js_type_expression::JSTypeExpressionExt,
    rhino::nominal_type_builder::NominalTypeBuilder,
    template_type_map::TemplateTypeMap,
    template_type_replacer::TemplateTypeReplacer,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    input_id::InputId,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{NodeId, Prop},
    qualified_name::QualifiedName,
    token::Token,
};
use std::rc::Rc;
use std::sync::Arc;

// port: TypedScopeCreator#MALFORMED_TYPEDEF
pub static MALFORMED_TYPEDEF: DiagnosticType = DiagnosticType::warning(
    "JSC_MALFORMED_TYPEDEF",
    "Typedef for {0} does not have any type information",
);

// port: TypedScopeCreator#ENUM_INITIALIZER
pub static ENUM_INITIALIZER: DiagnosticType = DiagnosticType::warning(
    "JSC_ENUM_INITIALIZER_NOT_ENUM",
    "enum initializer must be an object literal or an enum",
);

// port: TypedScopeCreator#INVALID_ENUM_KEY
pub static INVALID_ENUM_KEY: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_ENUM_KEY",
    "enum key must be a string or numeric literal",
);

// port: TypedScopeCreator#CTOR_INITIALIZER
pub static CTOR_INITIALIZER: DiagnosticType = DiagnosticType::warning(
    "JSC_CTOR_INITIALIZER_NOT_CTOR",
    "Constructor {0} must be initialized at declaration",
);

// port: TypedScopeCreator#IFACE_INITIALIZER
pub static IFACE_INITIALIZER: DiagnosticType = DiagnosticType::warning(
    "JSC_IFACE_INITIALIZER_NOT_IFACE",
    "Interface {0} must be initialized at declaration",
);

// port: TypedScopeCreator#CONSTRUCTOR_EXPECTED
pub static CONSTRUCTOR_EXPECTED: DiagnosticType = DiagnosticType::warning(
    "JSC_REFLECT_CONSTRUCTOR_EXPECTED",
    "Constructor expected as first argument",
);

// port: TypedScopeCreator#UNKNOWN_LENDS
pub static UNKNOWN_LENDS: DiagnosticType = DiagnosticType::warning(
    "JSC_UNKNOWN_LENDS",
    "Variable {0} not declared before @lends annotation.",
);

// port: TypedScopeCreator#LENDS_ON_NON_OBJECT
pub static LENDS_ON_NON_OBJECT: DiagnosticType = DiagnosticType::warning(
    "JSC_LENDS_ON_NON_OBJECT",
    "May only lend properties to object types. {0} has type {1}.",
);

// port: TypedScopeCreator#INCOMPATIBLE_ALIAS_ANNOTATION
pub static INCOMPATIBLE_ALIAS_ANNOTATION: DiagnosticType = DiagnosticType::warning(
    "JSC_INCOMPATIBLE_ALIAS_ANNOTATION",
    "Annotation {0} on {1} incompatible with aliased type.",
);

// port: TypedScopeCreator#DYNAMIC_EXTENDS_WITHOUT_JSDOC
pub static DYNAMIC_EXTENDS_WITHOUT_JSDOC: DiagnosticType = DiagnosticType::warning(
    "JSC_DYNAMIC_EXTENDS_WITHOUT_JSDOC",
    "The right-hand side of an extends clause must be a qualified name, or else @extends must be specified in JSDoc",
);

// port: TypedScopeCreator.Stage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    BUILDING,
    FROZEN,
}

/// Tracks information about a weakly ordered import, such as goog.requireType or
/// goog.forwardDeclare
///
/// TypedScopeCreator traverses through all files once in AST order, so when visiting a
/// goog.requireType() call, the required module may not have its scope populated yet. Thus we
/// create a 'WeakModuleImport' object and revisit the import after TypedScopeCreator has traversed
/// all files.
// port: TypedScopeCreator.WeakModuleImport
struct WeakModuleImport {
    module_local_node: NodeId, // the NAME or IMPORT_STAR representing the assignee
    scoped_import: Simple,     // the name (possibly qualified) & scope being imported
    local_module_scope: TypedScope, // the scope containing the import
}

impl WeakModuleImport {
    // port: TypedScopeCreator.WeakModuleImport#WeakModuleImport
    fn new(
        compiler: &AbstractCompiler,
        module_local_node: NodeId,
        scoped_import: Simple,
        local_module_scope: TypedScope,
    ) -> Self {
        check_state!(
            module_local_node.is_name(compiler) || module_local_node.is_import_star(compiler),
            "%s",
            module_local_node.to_string(compiler)
        );
        Self {
            module_local_node,
            scoped_import,
            local_module_scope,
        }
    }

    // port: TypedScopeCreator.WeakModuleImport#resolve
    fn resolve(&self, creator: &TypedScopeCreator, compiler: &mut AbstractCompiler) {
        // resolvedScope may be null if this is importing a nonexistent module
        let resolved_scope = self
            .scoped_import
            .get_scope_root(compiler)
            .and_then(|root| creator.memoized.get(&root).copied());
        let required_var = if let Some(resolved_scope) = resolved_scope {
            let name = self.scoped_import.get_name(compiler);
            resolved_scope.get_slot(compiler, name)
        } else {
            None
        };

        let type_;
        let is_inferred;
        if let Some(required_var) = required_var {
            type_ = required_var.get_type(compiler);
            is_inferred = required_var.is_type_inferred(compiler);
        } else if let Some(resolved_scope) = resolved_scope {
            // Some imports might not exist as fully qualified names in the given scope, but are
            // still resolvable via properties. This is always true for "provideAlreadyProvided"
            // names.
            let name = self.scoped_import.get_name(compiler);
            let ns_type = resolved_scope.get_type_through_namespace(compiler, name);
            type_ = Some(if ns_type.is_some() {
                ns_type.unwrap()
            } else {
                creator.unknown_type
            });
            is_inferred = ns_type.is_none();
        } else {
            // if resolvedScope is null, this code is importing a module that doesn't actually
            // exist and we'll error elsewhere.
            type_ = Some(creator.unknown_type);
            is_inferred = true;
        }
        let local_name = self.module_local_node.get_string(compiler);
        let input = NodeUtil::get_input_id(compiler, self.module_local_node)
            .and_then(|input_id| compiler.get_input(&input_id).cloned());
        self.local_module_scope.declare(
            compiler,
            local_name,
            Some(self.module_local_node),
            type_,
            input,
            is_inferred,
        );
        if required_var.is_some() {
            let name_node = required_var
                .unwrap()
                .get_name_node(compiler)
                .expect("NullPointerException");
            let typedef_type = name_node.get_typedef_type_prop(compiler);
            if typedef_type.is_some() {
                // Propagate the 'typedef type' from the module export to this variable. Otherwise
                // NamedTypes pointing to the imported name fail to resolve.
                let typedef_type = typedef_type.unwrap();
                self.module_local_node
                    .set_typedef_type_prop(compiler, Some(typedef_type));
                let local_module_scope =
                    self.local_module_scope.as_static_typed_scope_arc(compiler);
                let local_name = self.module_local_node.get_string(compiler);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                reg.declare_type(ast, Some(&*local_module_scope), local_name, typedef_type);
            }
        }
    }
}

/// Stores the type and qualified name for a destructuring rvalue.
// port: TypedScopeCreator.RValueInfo
struct RValueInfo {
    type_: Option<TypeId>,
    qualified_name: Option<QualifiedName>,
}

impl RValueInfo {
    // port: TypedScopeCreator.RValueInfo#RValueInfo
    fn new(type_: Option<TypeId>, qualified_name: Option<QualifiedName>) -> Self {
        Self {
            type_,
            qualified_name,
        }
    }

    // port: TypedScopeCreator.RValueInfo#empty
    fn empty() -> Self {
        Self::new(None, None)
    }
}

/// Rust-only: the `Supplier<RValueInfo>` lambdas of TypedScopeCreator's destructuring
/// declarations, as data (they call back into the scope builder, which a stored Rust closure
/// cannot borrow). `AbstractScopeBuilder#get_r_value_info` runs one.
enum RValueInfoSupplier {
    // defineVarChild: () -> value != null ? new RValueInfo(getDeclaredRValueType(null, value),
    // value.getQualifiedNameObject()) : new RValueInfo(unknownType, null)
    VarChildValue(Option<NodeId>),
    // defineDestructuringPatternInVarDeclaration: () -> inferTypeForDestructuredTarget(target,
    // patternTypeSupplier)
    DestructuredTarget {
        target: Rc<DestructuredTarget>,
        pattern_type_supplier: Rc<RValueInfoSupplier>,
    },
}

/// Creates the symbol table of types.
///
/// The Java fields `compiler`, `validator` and `typeRegistry` are not stored (DESIGN.md section
/// 6): every method that used them takes the compiler. `typeParsingErrorReporter` is the
/// registry's own reporter, which `JSType#resolve` uses (DESIGN.md section 8).
pub struct TypedScopeCreator {
    validator: Arc<TypeValidator>,
    coding_convention: Arc<dyn CodingConvention + Send + Sync>,
    module_map: Option<Arc<ModuleMap>>,
    // Java reads this field only in the constructor.
    #[allow(dead_code)]
    metadata_map: Arc<ModuleMetadataMap>,
    module_import_resolver: ModuleImportResolver,
    process_closure_primitives: bool,
    memoized: IndexMap<NodeId, TypedScope>,

    // Maps from scope root to declared variable names. Populated by FirstOrderFunctionAnalyzer to
    // reserve names before the TypedScope is populated.
    reserved_names_for_scope: IndexMap<NodeId, Vec<JsString>>,

    // Set of functions with non-empty returns, for passing to FunctionTypeBuilder.
    functions_with_non_empty_returns: IndexSet<NodeId>,
    // Includes both simple and qualified names.
    escaped_var_names: IndexSet<Simple>,
    // Count of how many times each variable is assigned, for marking effectively final.
    // (Java HashMultiset: element -> count; only add, count and remove are used.)
    assigned_var_names: IndexMap<Simple, i32>,

    // For convenience
    unknown_type: TypeId,

    // All names imported through goog.requireType. Resolve these after all scopes are created.
    weak_imports: Vec<WeakModuleImport>,

    unresolved_nodes: Vec<NodeId>,

    // Set of NAME, GETPROP, and STRING_KEY lvalues which should be treated as const declarations
    // when assigned. Treat simple names in this list as if they were declared `const`. E.g. treat
    // `exports = class {};` as `const exports = class {};`. Treat GETPROP and STRING_KEY nodes as
    // if they were annotated @const.
    undeclared_names_for_closure: IndexSet<NodeId>,

    // Maps EXPR_RESULT nodes from goog.provides to all implicitly provided names from the call
    provided_names_from_call: IndexMap<NodeId, Vec<ProvidedName>>,

    stage: Stage,
}

impl TypedScopeCreator {
    // port: TypedScopeCreator#TypedScopeCreator(AbstractCompiler)
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        // Java: this(compiler, compiler.getCodingConvention()). Compiler#getCodingConvention is
        // the options' convention, or else the compiler's default ClosureCodingConvention (a
        // stateless value, so a fresh instance is the same convention).
        let coding_convention = compiler
            .get_options()
            .get_coding_convention()
            .clone()
            .unwrap_or_else(|| Arc::new(ClosureCodingConvention::new()));
        Self::new_with_coding_convention(compiler, coding_convention)
    }

    // port: TypedScopeCreator#TypedScopeCreator(AbstractCompiler,CodingConvention)
    pub fn new_with_coding_convention(
        compiler: &mut AbstractCompiler,
        coding_convention: Arc<dyn CodingConvention + Send + Sync>,
    ) -> Self {
        let validator = compiler.get_type_validator();
        let unknown_type = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            reg.get_native_object_type(JSTypeNative::UNKNOWN_TYPE)
        };
        let metadata_map = if compiler.get_module_metadata_map().is_some() {
            compiler.get_module_metadata_map().unwrap().clone()
        } else {
            Arc::new(ModuleMetadataMap::new(
                IndexMap::<_, _>::default(),
                IndexMap::<_, _>::default(),
            ))
        };
        let module_map = compiler.get_module_map().cloned();
        let module_import_resolver = ModuleImportResolver::new(module_map.clone());
        let process_closure_primitives = !metadata_map.get_modules_by_goog_namespace().is_empty();
        let mut creator = Self {
            validator,
            coding_convention,
            module_map,
            metadata_map,
            module_import_resolver,
            process_closure_primitives,
            memoized: IndexMap::<_, _>::default(),
            reserved_names_for_scope: IndexMap::<_, _>::default(),
            functions_with_non_empty_returns: IndexSet::<_>::default(),
            escaped_var_names: IndexSet::<_>::default(),
            assigned_var_names: IndexMap::<_, _>::default(),
            unknown_type,
            weak_imports: Vec::new(),
            unresolved_nodes: Vec::new(),
            undeclared_names_for_closure: IndexSet::<_>::default(),
            provided_names_from_call: IndexMap::<_, _>::default(),
            stage: Stage::BUILDING,
        };

        // Reset state to empty collections.
        creator.clear_common_state();
        creator
    }

    // port: TypedScopeCreator#report
    fn report(&self, compiler: &mut AbstractCompiler, error: JSError) {
        compiler.report(error);
    }

    // port: TypedScopeCreator#getReferences
    pub fn get_references(&self, var: TypedVar) -> Vec<TypedVar> {
        vec![var]
    }

    // port: TypedScopeCreator#getScope
    pub fn get_scope(&self, compiler: &AbstractCompiler, var: TypedVar) -> TypedScope {
        var.get_scope(compiler)
    }

    // port: TypedScopeCreator#getAllSymbols
    pub fn get_all_symbols(&self, compiler: &AbstractCompiler) -> Vec<TypedVar> {
        let mut vars = Vec::new();
        for s in self.memoized.values() {
            vars.extend(s.get_all_symbols(compiler));
        }
        vars
    }

    /// Returns a function mapping a scope root node to a {@link TypedScope}.
    ///
    /// This method mostly exists in lieu of an interface representing root node -> scope.
    // port: TypedScopeCreator#getNodeToScopeMapper
    pub fn get_node_to_scope_mapper(&self) -> impl Fn(Option<NodeId>) -> Option<TypedScope> + '_ {
        // Java's `memoized::get` maps a null key to null.
        |n| n.and_then(|n| self.memoized.get(&n).copied())
    }

    // port: TypedScopeCreator#getAllMemoizedScopes
    pub fn get_all_memoized_scopes(&self) -> Vec<TypedScope> {
        // Return scopes in reverse order of creation so that IIFEs will
        // come before the global scope.
        self.memoized.values().rev().copied().collect()
    }

    /// Create a scope if it doesn't already exist, looking up in the map for the parent scope.
    // port: TypedScopeCreator#createScope(Node)
    pub fn create_scope_for_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> TypedScope {
        let s = self.memoized.get(&n).copied();
        if s.is_some() {
            s.unwrap()
        } else {
            let parent_root = NodeUtil::get_enclosing_scope_root(
                compiler,
                n.get_parent(compiler).expect("NullPointerException"),
            )
            .expect("NullPointerException");
            let parent = self.create_scope_for_node(compiler, parent_root);
            self.create_scope(compiler, n, Some(parent))
        }
    }

    /// Creates a scope with all types declared. Declares newly discovered types and type
    /// properties in the type registry.
    // port: TypedScopeCreator#createScope(Node,AbstractScope)
    pub fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        parent: Option<TypedScope>,
    ) -> TypedScope {
        // checkArgument(parent == null || parent instanceof TypedScope): the Rust parameter is
        // already a TypedScope.
        let typed_parent = parent;

        let scope = self.memoized.get(&root).copied();
        if let Some(scope) = scope {
            check_state!(typed_parent == scope.get_parent(compiler));
            scope
        } else {
            let scope = self.create_scope_internal(compiler, root, typed_parent);
            self.memoized.insert(root, scope);
            scope
        }
    }

    // port: TypedScopeCreator#createScopeInternal
    fn create_scope_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        typed_parent: Option<TypedScope>,
    ) -> TypedScope {
        check_state!(
            self.stage == Stage::BUILDING,
            "Cannot create scope for %s after TypedScopeCreator is frozen",
            root.to_string(compiler)
        );
        // Constructing the global scope is very different than constructing
        // inner scopes, because only global scopes can contain named classes that
        // show up in the type registry.
        let new_scope;

        let module = ModuleImportResolver::get_module_from_scope_root(
            self.module_map.as_deref(),
            compiler,
            root,
        );
        if typed_parent.is_none() {
            check_state!(root.is_root(compiler), "%s", root.to_string(compiler));
            let externs_root = root
                .get_first_child(compiler)
                .expect("NullPointerException");
            let js_root = root
                .get_second_child(compiler)
                .expect("NullPointerException");
            check_state!(
                externs_root.is_root(compiler),
                "%s",
                externs_root.to_string(compiler)
            );
            check_state!(js_root.is_root(compiler), "%s", js_root.to_string(compiler));
            let global_this = {
                let (reg, _) = compiler.get_type_registry_and_ast();
                reg.get_native_object_type(JSTypeNative::GLOBAL_THIS)
            };

            // Mark the main root, the externs root, and the src root
            // with the global this type.
            root.set_jstype(compiler, Some(global_this));
            externs_root.set_jstype(compiler, Some(global_this));
            js_root.set_jstype(compiler, Some(global_this));

            // Find all the classes in the global scope.
            new_scope = self.create_initial_scope(compiler, root);
        } else {
            // Because JSTypeRegistry#getType looks up the scope in which a root of a qualified
            // name is declared, pre-populate this TypedScope with all qualified name roots. This
            // prevents type resolution from accidentally returning a type from an outer scope that
            // is shadowed.
            let mut reserved_names = IndexSet::<_>::default();
            // Java HashMap#remove; the map is never iterated, so the O(1) swap_remove keeps
            // the output (shift_remove is O(n) per call).
            reserved_names.extend(
                self.reserved_names_for_scope
                    .swap_remove(&root)
                    .unwrap_or_default(),
            );
            if module.is_some() && module.as_ref().unwrap().metadata().is_goog_module() {
                // TypedScopeCreator treats default export assignments, like `exports = class {};`,
                // as declarations. However, the untyped scope only contains an implicit slot for
                // `exports`.
                reserved_names.insert(JsString::from("exports"));
            } else if root.is_function(compiler)
                && NodeUtil::is_bundled_goog_module_call(
                    compiler,
                    root.get_parent(compiler).expect("NullPointerException"),
                )
            {
                // Pretend that 'exports' is declared in the block of goog.loadModule
                // functions, not the function scope. See the above comment for why.
                reserved_names.shift_remove(&JsString::from("exports"));
            }

            new_scope = TypedScope::new_with_reserved_names(
                compiler,
                typed_parent.unwrap(),
                root,
                &reserved_names,
                module.clone(),
            );
        }

        let scope_builder;
        if root.is_function(compiler) {
            scope_builder = ScopeBuilderKind::FunctionScopeBuilder;
        } else if root.is_class(compiler)
            || root.is_member_field_def(compiler)
            || root.is_computed_field_def(compiler)
        {
            scope_builder = ScopeBuilderKind::ClassScopeBuilder;
        } else {
            scope_builder = ScopeBuilderKind::NormalScopeBuilder;
            if root.is_module_body(compiler) {
                // Store the module scope on the CompilerInput object so it can be found and used
                // for program analysis purposes.
                // DefaultPassConfig is responsible for removing it when it is no longer needed.
                let input_id = check_not_null!(NodeUtil::get_input_id(compiler, root));
                let compiler_input = check_not_null!(compiler.get_input(&input_id));
                compiler_input.set_typed_scope(Some(new_scope));
            }
        }
        self.build_scope(compiler, scope_builder, new_scope);
        if module.is_some() && module.as_ref().unwrap().metadata().is_es6_module() {
            // Declare an implicit variable representing the namespace of this module, then add a
            // property for each exported name to that variable's type.
            let namespace_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                reg.create_anonymous_object_type(ast, None)
            };
            let input = NodeUtil::get_input_id(compiler, root)
                .and_then(|input_id| compiler.get_input(&input_id).cloned());
            new_scope.declare(
                compiler,
                Export::NAMESPACE,
                Some(root), // Use the given MODULE_BODY as the 'declaration node' for lack of a
                // better option.
                Some(namespace_type),
                input,
                /* inferred= */ false,
            );

            // Store the module object type on the MODULE_BODY.
            // The Es6RewriteModules will retrieve it from there for use when it creates a global
            // for the module object.
            root.set_jstype(compiler, Some(namespace_type));
            let node_to_scope_mapper = self.get_node_to_scope_mapper();
            self.module_import_resolver.update_es_module_namespace_type(
                compiler,
                &node_to_scope_mapper,
                namespace_type,
                module.as_ref().unwrap(),
                new_scope,
            );
        }

        new_scope
    }

    /// Adds nodes representing goog.module exports to a list, to treat them as @const.
    ///
    /// This method handles the following styles of exports:
    ///
    /// - {@code exports = class {}} adds the NAME node `exports`
    /// - {@code exports = {Foo};} adds the NAME node `exports` and the STRING_KEY node `Foo`
    /// - {@code exports.Foo = Foo;} adds the GETPROP node `exports.Foo`
    // port: TypedScopeCreator#markGoogModuleExportsAsConst
    fn mark_goog_module_exports_as_const(
        &mut self,
        compiler: &AbstractCompiler,
        module_body: NodeId,
    ) {
        // TODO(lharker): Use the source nodes from the Bindings once we no longer rewrite before
        // typechecking. This is not feasible currently because a few places will rewrite exports =
        // ...
        let mut statement = module_body.get_first_child(compiler);
        while let Some(stmt) = statement {
            statement = stmt.get_next(compiler);
            if !NodeUtil::is_expr_assign(compiler, stmt) {
                continue;
            }
            let lhs = stmt.get_first_first_child(compiler).unwrap();
            if lhs.matches_name(compiler, "exports") {
                self.undeclared_names_for_closure.insert(lhs);
                // If this is full of named exports, add all the string key nodes.
                if NodeUtil::is_named_exports_literal(compiler, lhs.get_next(compiler).unwrap()) {
                    let mut key = lhs.get_next(compiler).unwrap().get_first_child(compiler);
                    while let Some(k) = key {
                        self.undeclared_names_for_closure.insert(k);
                        key = k.get_next(compiler);
                    }
                }
            } else if lhs.is_get_prop(compiler)
                && lhs
                    .get_first_child(compiler)
                    .unwrap()
                    .matches_name(compiler, "exports")
            {
                self.undeclared_names_for_closure.insert(lhs);
            }
        }
    }

    /// Gathers all namespaces created by goog.provide and any definitions in code.
    ///
    /// This method does not actually declare anything in the scope. In order to accurately report
    /// redefinition warnings, wait to declare implicit names until the actual goog.provide call.
    ///
    /// @param root The global ROOT or a SCRIPT
    // port: TypedScopeCreator#gatherAllProvides
    fn gather_all_provides(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        if !self.process_closure_primitives {
            return;
        }

        let externs = root.get_first_child(compiler).unwrap();
        let js = root.get_second_child(compiler).unwrap();
        let mut provides_and_requires = ProcessClosureProvidesAndRequires::new(
            compiler, /* preserveGoogProvidesAndRequires= */ true,
        );
        let provided_names = provides_and_requires
            .collect_provided_names(compiler, externs, js)
            .values()
            .cloned()
            .collect::<Vec<_>>();

        for name in provided_names {
            if name.has_implicit_initialization() {
                continue;
            }
            if name.get_candidate_definition().is_some() {
                // This name will be defined eventually in the source code.
                let first_definition_node = name.get_candidate_definition().unwrap();
                if NodeUtil::is_expr_assign(compiler, first_definition_node)
                    && first_definition_node
                        .get_first_first_child(compiler)
                        .unwrap()
                        .is_name(compiler)
                {
                    // Treat assignments of provided names as declarations.
                    self.undeclared_names_for_closure.insert(
                        first_definition_node
                            .get_first_first_child(compiler)
                            .unwrap(),
                    );
                }
            } else if name.get_first_provide_call().is_some()
                && NodeUtil::is_expr_call(compiler, name.get_first_provide_call().unwrap())
                && !name.is_from_legacy_module()
            {
                // This name is implicitly created by a goog.provide call; declare it in the scope
                // once reaching the provide call. The exception is legacy goog.modules, which are
                // declared once leaving the module.
                self.provided_names_from_call
                    .entry(name.get_first_provide_call().unwrap())
                    .or_default()
                    .push(name.clone());
            }

            if name.is_explicitly_provided() && !name.is_from_legacy_module() {
                let (reg, _) = compiler.get_type_registry_and_ast();
                reg.register_legacy_closure_namespace(name.get_namespace().clone());
            }
        }
    }

    /// Create the outermost scope. This scope contains native binding such as {@code Object},
    /// {@code Date}, etc.
    ///
    /// @param root The global ROOT node
    // port: TypedScopeCreator#createInitialScope
    pub fn create_initial_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> TypedScope {
        check_argument!(root.is_root(compiler), "%s", root.to_string(compiler));

        // Gather global information used in typed scope creation. Use a memoized scope creator
        // because scope-building takes a nontrivial amount of time.
        let mut scope_creator = MemoizedScopeCreator::new(Box::new(SyntacticScopeCreator::new()));

        let externs = root.get_first_child(compiler).unwrap();
        let js = root.get_last_child(compiler).unwrap();
        let mut first_order_function_analyzer = FirstOrderFunctionAnalyzer::new(self);
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(&mut first_order_function_analyzer)
            .set_scope_creator(&mut scope_creator)
            .traverse_roots(externs, js);

        let mut identify_enums_and_typedefs = IdentifyEnumsAndTypedefsAsNonNullable::new();
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(&mut identify_enums_and_typedefs)
            .set_scope_creator(&mut scope_creator)
            .traverse(root);

        let s = TypedScope::create_global_scope(compiler, root);
        self.declare_native_function_type(compiler, s, JSTypeNative::ARRAY_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::BIGINT_OBJECT_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::BOOLEAN_OBJECT_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::DATE_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::FUNCTION_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::GENERATOR_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::ITERABLE_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::ITERATOR_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::NUMBER_OBJECT_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::OBJECT_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::REGEXP_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::STRING_OBJECT_FUNCTION_TYPE);
        self.declare_native_function_type(compiler, s, JSTypeNative::SYMBOL_OBJECT_FUNCTION_TYPE);
        self.declare_native_value_type(compiler, s, "undefined", JSTypeNative::VOID_TYPE);
        self.add_well_known_symbols(compiler, s);

        self.gather_all_provides(compiler, root);

        // Memoize the global scope so that module scope creation can access it. See
        // AbstractScopeBuilder#shouldTraverse - modules are traversed early, as if they were always
        // executed when control flow reaches the module body.
        self.memoized.insert(root, s);

        s
    }

    // port: TypedScopeCreator#addWellKnownSymbols
    fn add_well_known_symbols(&self, compiler: &mut AbstractCompiler, scope: TypedScope) {
        for symbol in WELL_KNOWN_SYMBOLS {
            let type_ = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                KnownSymbolType::new(reg, ast, symbol)
            };
            Self::declare_native_type(compiler, scope, symbol, type_);
        }
    }

    // port: TypedScopeCreator#declareNativeFunctionType
    fn declare_native_function_type(
        &self,
        compiler: &mut AbstractCompiler,
        scope: TypedScope,
        t_id: JSTypeNative,
    ) {
        let (t, instance_name) = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            let t = reg.get_native_function_type(t_id);
            let instance_name = t.get_instance_type(reg).unwrap().get_reference_name(reg);
            (t, instance_name)
        };
        Self::declare_native_type(compiler, scope, instance_name.unwrap(), t);
        let (prototype, prototype_name) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let prototype = t.get_prototype(reg, ast);
            (prototype, prototype.get_reference_name(reg))
        };
        Self::declare_native_type(compiler, scope, prototype_name.unwrap(), prototype);
    }

    // port: TypedScopeCreator#declareNativeValueType
    fn declare_native_value_type(
        &self,
        compiler: &mut AbstractCompiler,
        scope: TypedScope,
        name: &str,
        t_id: JSTypeNative,
    ) {
        let t = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            reg.get_native_type(t_id)
        };
        Self::declare_native_type(compiler, scope, name, t);
    }

    // port: TypedScopeCreator#declareNativeType
    fn declare_native_type(
        compiler: &mut AbstractCompiler,
        scope: TypedScope,
        name: impl Into<JsString>,
        t: TypeId,
    ) {
        scope.declare(compiler, name, None, Some(t), None, false);
    }

    /// Set the type for a node now
    ///
    /// If the type is unresolved, enqueue it to be updated with a resolved type later.
    // port: TypedScopeCreator#setDeferredType
    pub fn set_deferred_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        type_: TypeId,
    ) {
        // Other parts of this pass may read the not-yet-resolved type off the node.
        // (like when we set the LHS of an assign with a typed RHS function.)
        node.set_jstype(compiler, Some(type_));
        let (reg, _) = compiler.get_type_registry_and_ast();
        if !type_.is_resolved(reg) {
            self.unresolved_nodes.push(node);
        }
    }

    /// Needs to run pre-type-resolution to handle weak module imports
    // port: TypedScopeCreator#resolveWeakImportsPreResolution
    pub fn resolve_weak_imports_pre_resolution(&self, compiler: &mut AbstractCompiler) {
        // Declare goog.module type requires in scope.
        for weak_import in &self.weak_imports {
            weak_import.resolve(self, compiler);
        }
    }

    /// Performs some final work to resolve remaining types
    ///
    /// After this call, calling this.createScope will crash if passed a scope root that hasn't
    /// already been visited.
    // port: TypedScopeCreator#finishAndFreeze
    pub fn finish_and_freeze(&mut self, compiler: &mut AbstractCompiler) {
        // Resolve types and attach them to nodes.
        for &node in &self.unresolved_nodes {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let resolved = node
                .get_jstype(ast)
                .expect("NullPointerException")
                .resolve(reg, ast);
            node.set_jstype(compiler, Some(resolved));
        }

        // Resolve types and attach them to scope slots.
        for scope in self.get_all_memoized_scopes() {
            for var in scope.get_var_iterable(compiler) {
                var.resolve_type(compiler);
            }
            scope.validate_completely_built(compiler);
        }

        // free up memory. we no longer need this state now that all scopes have been visited
        self.clear_common_state();
        self.stage = Stage::FROZEN;

        // we must keep the 'memoized' set of TypedScopes around, since later passes will use it.
    }

    // port: TypedScopeCreator#clearCommonState
    fn clear_common_state(&mut self) {
        self.reserved_names_for_scope = IndexMap::<_, _>::default();
        self.functions_with_non_empty_returns = IndexSet::<_>::default();
        self.escaped_var_names = IndexSet::<_>::default();
        self.assigned_var_names = IndexMap::<_, _>::default();
        self.weak_imports = Vec::new();
        self.unresolved_nodes = Vec::new();
        self.undeclared_names_for_closure = IndexSet::<_>::default();
        self.provided_names_from_call = IndexMap::<_, _>::default();
    }

    // port: TypedScopeCreator#getNativeType
    fn get_native_type(
        &self,
        compiler: &mut AbstractCompiler,
        native_type: JSTypeNative,
    ) -> TypeId {
        let (reg, _) = compiler.get_type_registry_and_ast();
        reg.get_native_type(native_type)
    }
}

/// The concrete AbstractScopeBuilder subclass that createScopeInternal instantiates.
// The variants keep the Java class names.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeBuilderKind {
    NormalScopeBuilder,
    FunctionScopeBuilder,
    ClassScopeBuilder,
}

impl TypedScopeCreator {
    // Rust-only: `new XScopeBuilder(newScope).build()` in createScopeInternal.
    // port: TypedScopeCreator.NormalScopeBuilder#NormalScopeBuilder
    // port: TypedScopeCreator.FunctionScopeBuilder#FunctionScopeBuilder
    // port: TypedScopeCreator.ClassScopeBuilder#ClassScopeBuilder
    fn build_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        kind: ScopeBuilderKind,
        scope: TypedScope,
    ) {
        AbstractScopeBuilder::new(self, compiler, kind, scope).build(compiler);
    }
}

/// A deferred AbstractScopeBuilder action (Java: a `Runnable` lambda in `deferredActions`).
// Rust-only: the two lambdas TypedScopeCreator defers, as data the builder runs later.
#[derive(Clone)]
enum DeferredAction {
    // () -> defineObjectLiteral(n)
    DefineObjectLiteral(NodeId),
    // () -> resolveStubDeclaration(n, isExtern, ownerName)
    ResolveStubDeclaration {
        n: NodeId,
        is_extern: bool,
        owner_name: JsString,
    },
}

/// The builder of one TypedScope (Java's abstract inner class; the concrete subclass is `kind`).
// port: TypedScopeCreator.AbstractScopeBuilder
struct AbstractScopeBuilder<'a> {
    // Rust-only: the enclosing TypedScopeCreator instance of the Java inner class.
    creator: &'a mut TypedScopeCreator,
    // Rust-only: which subclass (NormalScopeBuilder, FunctionScopeBuilder, ClassScopeBuilder).
    kind: ScopeBuilderKind,

    /// The scope that we're building.
    current_scope: TypedScope,

    /// The current hoist scope.
    current_hoist_scope: TypedScope,

    /// The current source file that we're in.
    source_name: Option<String>,

    /// The InputId of the current node.
    input_id: Option<Arc<InputId>>,

    /// Some actions need to be deferred, such as analyzing object literals with lends
    /// annotations, or resolving type-less stubs. These actions are added to this map, keyed by
    /// the node that should be waited for before running.
    // Java: a LinkedHashMultimap; its entries in global insertion order (values() runs them in
    // that order), with the multimap operations in the `*_deferred_action*` helpers.
    deferred_actions: Vec<(NodeId, DeferredAction)>,
}

impl<'a> AbstractScopeBuilder<'a> {
    // port: TypedScopeCreator.AbstractScopeBuilder#AbstractScopeBuilder
    fn new(
        creator: &'a mut TypedScopeCreator,
        compiler: &AbstractCompiler,
        kind: ScopeBuilderKind,
        scope: TypedScope,
    ) -> Self {
        Self {
            creator,
            kind,
            current_scope: scope,
            current_hoist_scope: scope
                .get_closest_hoist_scope(compiler)
                .expect("NullPointerException"),
            source_name: None,
            input_id: None,
            deferred_actions: Vec::new(),
        }
    }

    /// Returns the current compiler input.
    // port: TypedScopeCreator.AbstractScopeBuilder#getCompilerInput
    fn get_compiler_input(&self, compiler: &AbstractCompiler) -> Option<CompilerInput> {
        compiler.get_input(self.input_id.as_deref()?).cloned()
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#getModule
    fn get_module(&self, compiler: &AbstractCompiler) -> Option<Arc<Module>> {
        self.current_scope.get_module(compiler)
    }

    /// Traverse the scope root and build it.
    // port: TypedScopeCreator.AbstractScopeBuilder#build
    fn build(&mut self, compiler: &mut AbstractCompiler) {
        let root = self.current_scope.get_root_node(compiler);
        self.initialize_module_scope(compiler, root);

        let current_scope = self.current_scope;
        let mut scope_creator = ASSERT_NO_SCOPES_CREATED;
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .set_scope_creator(&mut scope_creator)
            .traverse_at_scope(current_scope);

        self.finish_declaring_goog_module(compiler);
    }

    /// Builds the beginning of a module-scope. This can be an ES module or a goog.module.
    // port: TypedScopeCreator.AbstractScopeBuilder#initializeModuleScope
    fn initialize_module_scope(&mut self, compiler: &mut AbstractCompiler, module_body: NodeId) {
        if self.get_module(compiler).is_none() {
            return;
        }
        let module = self.get_module(compiler).unwrap();
        let module_type = module.metadata().module_type();
        match module_type {
            ModuleType::LEGACY_GOOG_MODULE | ModuleType::GOOG_MODULE => {
                self.declare_exports_in_goog_module_scope(compiler, &module, module_body);
                self.creator
                    .mark_goog_module_exports_as_const(compiler, module_body);
            }
            ModuleType::ES6_MODULE => {
                let input = NodeUtil::get_input_id(compiler, module_body)
                    .and_then(|input_id| compiler.get_input(&input_id).cloned());
                let unresolved_imports = {
                    let creator = &*self.creator;
                    let node_to_scope_mapper = creator.get_node_to_scope_mapper();
                    creator.module_import_resolver.declare_es_module_imports(
                        compiler,
                        &node_to_scope_mapper,
                        &module,
                        self.current_scope,
                        input,
                    )
                };
                let current_scope = self.current_scope;
                for (key, value) in unresolved_imports {
                    let weak_import = WeakModuleImport::new(compiler, key, value, current_scope);
                    self.creator.weak_imports.push(weak_import);
                }
            }
            _ => {
                // This should have been a build-breaking DUPLICATE_NAMESPACE_AND_MODULE error in
                // GatherModuleMetadata
                panic!(
                    "IllegalStateException: Unexpected module type {:?} in module {:?}",
                    module_type, module
                );
            }
        }
    }

    /// Ensures that the name `exports` is declared in goog.module scope.
    ///
    /// If a goog.module explicitly assigns to exports, we want to treat that assignment inside the
    /// scope as if it were a declaration: `const exports = ...`. This method only handles cases
    /// where we want to treat exports as implicitly declared.
    // port: TypedScopeCreator.AbstractScopeBuilder#declareExportsInGoogModuleScope
    fn declare_exports_in_goog_module_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        goog_module: &Module,
        module_body: NodeId,
    ) {
        let mut statement = module_body.get_first_child(compiler);
        while let Some(stmt) = statement {
            statement = stmt.get_next(compiler);
            if !NodeUtil::is_expr_assign(compiler, stmt) {
                continue;
            }
            let lhs = stmt.get_first_first_child(compiler).unwrap();
            if lhs.matches_name(compiler, "exports") {
                return; // found a direct assignment `exports = [...]`
            }
        }
        let root = goog_module
            .metadata()
            .root_node()
            .expect("NullPointerException");
        // Synthesize an object literal namespace 'exports'
        let exports_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            reg.create_anonymous_object_type(ast, None)
        };
        SlotDefiner::new()
            .for_declaration_node(root.get_first_child(compiler))
            .for_variable_name("exports")
            .with_type(Some(exports_type))
            .allow_later_type_inference(false)
            .in_scope(self.current_scope)
            .define_slot(self, compiler);
    }

    /// Declares goog.module.declareLegacyNamespace() names in the global scope and annotates the
    /// AST with type information about module exports.
    ///
    /// For example, given `goog.module('a.b.Foo'); goog.module.declareLegacyNamespace();
    /// exports = class Foo {};` This function is responsible for declaring a global name `a.b.Foo`
    /// and giving it the type from `exports = class Foo {};`
    // port: TypedScopeCreator.AbstractScopeBuilder#finishDeclaringGoogModule
    fn finish_declaring_goog_module(&mut self, compiler: &mut AbstractCompiler) {
        if self.get_module(compiler).is_none()
            || !self
                .get_module(compiler)
                .unwrap()
                .metadata()
                .is_goog_module()
        {
            return;
        }
        let module = self.get_module(compiler).unwrap();
        let exports_var = check_not_null!(self.current_scope.get_slot(compiler, "exports"));
        let closure_namespace = module
            .closure_namespace()
            .cloned()
            .expect("NullPointerException");
        let exports_name_node = exports_var.get_name_node(compiler);
        let exports_type = exports_var.get_type(compiler);

        if module.metadata().is_legacy_goog_module() {
            {
                let (reg, _) = compiler.get_type_registry_and_ast();
                reg.register_legacy_closure_namespace(closure_namespace.clone());
            }
            let module_namespace = QualifiedName::of(closure_namespace.clone());
            let global_scope = self.current_scope.get_global_scope(compiler);
            SlotDefiner::new()
                .in_scope(global_scope)
                .for_declaration_node(exports_name_node)
                .for_variable_name(module_namespace.join(compiler))
                .with_type(exports_type)
                .allow_later_type_inference(exports_var.is_type_inferred(compiler))
                .for_goog_provided_name()
                .define_slot(self, compiler);

            if !module_namespace.is_simple(compiler) && !exports_var.is_type_inferred(compiler) {
                let owner = module_namespace.get_owner(compiler).unwrap();
                let parent_type = lookup_qualified_name(compiler, global_scope, &owner);
                let parent_object_type = {
                    let (reg, _) = compiler.get_type_registry_and_ast();
                    parent_type.and_then(|parent_type| parent_type.to_maybe_object_type(reg))
                };
                if parent_type.is_some() && parent_object_type.is_some() {
                    // Declare the namespace on the parent name, propagating any JSDoc annotations
                    // on "exports = Foo".
                    let type_doc = NodeUtil::get_best_jsdoc_info(
                        compiler,
                        exports_name_node.expect("NullPointerException"),
                    );
                    let component = module_namespace.get_component(compiler);
                    self.declare_property_if_namespace_type(
                        compiler,
                        parent_object_type.unwrap(),
                        exports_name_node.expect("NullPointerException"),
                        PropertyKey::String(component),
                        exports_type,
                        exports_name_node.expect("NullPointerException"),
                        type_doc,
                    );
                }
            }
            let current_scope = self.current_scope;
            self.declare_alias_type_if_rvalue_is_aliasable_with_name(
                compiler,
                Some(closure_namespace),
                exports_name_node, // Pretend that 'exports = '... is the lvalue node.
                Some(QualifiedName::of("exports")),
                exports_type,
                current_scope,
                global_scope,
            );
        } else {
            let (reg, _) = compiler.get_type_registry_and_ast();
            reg.register_non_legacy_closure_namespace(
                closure_namespace,
                exports_name_node,
                exports_type,
            );
        }
        // Store the type of the namespace on the AST for the convenience of later passes that
        // want to access it.
        let root_node = self.current_scope.get_root_node(compiler);
        if root_node.is_module_body(compiler) {
            root_node.set_jstype(compiler, exports_type);
        } else {
            // For goog.loadModule, give the `exports` parameter the correct type.
            check_state!(
                root_node.is_block(compiler),
                "%s",
                root_node.to_string(compiler)
            );
            let fn_ = root_node
                .get_parent(compiler)
                .expect("NullPointerException");
            let param_list = NodeUtil::get_function_parameters(compiler, fn_);
            param_list
                .get_only_child(compiler)
                .set_jstype(compiler, exports_type);
        }
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#inCurrentScope
    fn in_current_scope(&self, t: &mut NodeTraversal<'_>) -> bool {
        t.get_scope_root() == Some(self.current_scope.get_root_node(t.get_compiler()))
    }

    /// Called by shouldTraverse on nodes after ensuring the inputId is set.
    // port: TypedScopeCreator.AbstractScopeBuilder#visitPreorder
    fn visit_preorder(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // Rust-only: the subclass override (Java virtual dispatch).
        match self.kind {
            ScopeBuilderKind::NormalScopeBuilder => self.normal_visit_preorder(t, n, parent),
            ScopeBuilderKind::FunctionScopeBuilder => self.function_visit_preorder(t, n, parent),
            ScopeBuilderKind::ClassScopeBuilder => self.class_visit_preorder(t, n, parent),
        }
    }

    /// Called by visit on nodes after updating the inputId.
    // port: TypedScopeCreator.AbstractScopeBuilder#visitPostorder
    fn visit_postorder(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // Rust-only: the subclass override (Java virtual dispatch); FunctionScopeBuilder keeps the
        // empty base method.
        match self.kind {
            ScopeBuilderKind::NormalScopeBuilder => self.normal_visit_postorder(t, n, parent),
            ScopeBuilderKind::FunctionScopeBuilder => {}
            ScopeBuilderKind::ClassScopeBuilder => self.class_visit_postorder(t, n, parent),
        }
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#attachLiteralTypes
    fn attach_literal_types(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::NULL => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::NULL_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            Token::VOID => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::VOID_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            Token::STRINGLIT | Token::TEMPLATELIT_STRING => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::STRING_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            Token::NUMBER => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::NUMBER_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            Token::BIGINT => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::BIGINT_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            Token::TRUE | Token::FALSE => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::BOOLEAN_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            Token::REGEXP => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::REGEXP_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            Token::OBJECTLIT => {
                let info = n.get_jsdoc_info(compiler);
                if info.is_some() && info.unwrap().has_lends_name() {
                    // Defer analyzing object literals with a @lends annotation until we
                    // reach the root of the statement they're defined in.
                    //
                    // This ensures that if there are any @lends annotations on the object
                    // literals, the type on the @lends annotation resolves correctly.
                    //
                    // For more information, see
                    // http://blickly.github.io/closure-compiler-issues/#314
                    let statement = NodeUtil::get_enclosing_statement(compiler, n)
                        .expect("NullPointerException");
                    self.put_deferred_action(statement, DeferredAction::DefineObjectLiteral(n));
                } else {
                    self.define_object_literal(compiler, n);
                }
            }
            Token::CLASS => {
                // NOTE(sdh): We can't handle function nodes here because they need special
                // behavior to deal with hoisting.  But since classes aren't hoisted, and may need
                // to be handled in such places as default method initializers (i.e. in a
                // FunctionScope) or class extends clauses (technically part of the ClassScope, but
                // visited instead by the NormalScope), they can be handled consistently in all
                // scopes.
                self.define_class_literal(compiler, n);
            }
            // NOTE(johnlenz): If we ever support Array tuples,
            // we will need to handle them here as we do object literals
            // above.
            Token::ARRAYLIT => {
                let type_ = self
                    .creator
                    .get_native_type(compiler, JSTypeNative::ARRAY_TYPE);
                n.set_jstype(compiler, Some(type_));
            }
            _ => {}
        }
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#defineObjectLiteral
    fn define_object_literal(&mut self, compiler: &mut AbstractCompiler, object_lit: NodeId) {
        // Handle the @lends annotation.
        let mut type_: Option<TypeId> = None;
        let mut info = object_lit.get_jsdoc_info(compiler);
        if info.is_some() && info.as_ref().unwrap().has_lends_name() {
            let lends_name = info
                .as_ref()
                .unwrap()
                .get_lends_name()
                .unwrap()
                .get_root()
                .get_string(compiler);
            let lends_var = self.current_scope.get_var(compiler, lends_name.clone());
            if lends_var.is_none() {
                let error = JSError::make(
                    compiler,
                    object_lit,
                    &UNKNOWN_LENDS,
                    &[&lends_name.to_string()],
                );
                self.creator.report(compiler, error);
            } else {
                type_ = lends_var.unwrap().get_type(compiler);
                if type_.is_none() {
                    type_ = Some(self.creator.unknown_type);
                }
                let is_subtype = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let object_type = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
                    type_.unwrap().is_subtype_of(reg, ast, object_type)
                };
                if !is_subtype {
                    let type_string = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        type_.unwrap().to_string(reg, ast)
                    };
                    let error = JSError::make(
                        compiler,
                        object_lit,
                        &LENDS_ON_NON_OBJECT,
                        &[&lends_name.to_string(), &type_string],
                    );
                    self.creator.report(compiler, error);
                    type_ = None;
                } else {
                    object_lit.set_jstype(compiler, type_);
                }
            }
        }

        info = NodeUtil::get_best_jsdoc_info(compiler, object_lit);
        let create_enum_type = info.is_some() && info.as_ref().unwrap().has_enum_parameter_type();
        if create_enum_type {
            let l_value = NodeUtil::get_best_l_value(compiler, object_lit);
            let l_value_name = NodeUtil::get_best_l_value_name(compiler, l_value);
            type_ = Some(self.create_enum_type_from_nodes(
                compiler,
                Some(object_lit),
                l_value_name,
                l_value,
                info.clone().unwrap(),
            ));
        }

        if type_.is_none() {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            type_ = Some(reg.create_anonymous_object_type(ast, info));
        }

        self.creator
            .set_deferred_type(compiler, object_lit, type_.unwrap());

        // If this is an enum, the properties were already taken care of above.
        let obj_lit_type = {
            let js_type = object_lit.get_jstype(compiler);
            let (reg, _) = compiler.get_type_registry_and_ast();
            object_type::cast(reg, js_type)
        };
        self.process_object_lit_properties(compiler, object_lit, obj_lit_type, !create_enum_type);
    }

    /// Process an object literal and all the types on it.
    ///
    /// @param objLit The OBJECTLIT node.
    /// @param objLitType The type of the OBJECTLIT node. This might be a named type, because of
    ///     the lends annotation.
    /// @param declareOnOwner If true, declare properties on the objLitType as well. If false, the
    ///     caller should take care of this.
    // port: TypedScopeCreator.AbstractScopeBuilder#processObjectLitProperties
    fn process_object_lit_properties(
        &mut self,
        compiler: &mut AbstractCompiler,
        obj_lit: NodeId,
        obj_lit_type: Option<TypeId>,
        declare_on_owner: bool,
    ) {
        let mut key_node_cursor = obj_lit.get_first_child(compiler);
        while let Some(key_node) = key_node_cursor {
            key_node_cursor = key_node.get_next(compiler);
            if key_node.is_spread(compiler) {
                // Don't try defining computed or spread properties on an object. Note that for
                // spread type inference will try to determine the properties and types. We cannot
                // do it here as we don't have all the type information of the spread object.
                continue;
            }
            let member_name: PropertyKey;
            let value: Option<NodeId>;
            let qualified_name: Option<JsString>;
            if key_node.is_computed_prop(compiler) {
                let symbol = extract_known_symbol_key(
                    compiler,
                    self.current_scope,
                    key_node.get_first_child(compiler).unwrap(),
                );
                if symbol.is_none() {
                    continue;
                }
                qualified_name = None;
                member_name = PropertyKey::Symbol(symbol.unwrap());
                value = key_node.get_second_child(compiler);
            } else {
                value = key_node.get_first_child(compiler);
                member_name = PropertyKey::String(NodeUtil::get_object_or_class_lit_key_name(
                    compiler, key_node,
                ));
                qualified_name = NodeUtil::get_best_l_value_name(compiler, Some(key_node));
            }
            let info = key_node.get_jsdoc_info(compiler);
            let value_type = self.get_declared_type(compiler, info, key_node, value, None);
            let key_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                // objLitType.isEnumType(): Java dereferences objLitType here.
                let obj_lit_type = obj_lit_type.expect("NullPointerException");
                if obj_lit_type.is_enum_type(reg) {
                    Some(
                        obj_lit_type
                            .to_maybe_enum_type(reg)
                            .unwrap()
                            .get_elements_type(reg),
                    )
                } else {
                    TypeCheck::get_object_lit_key_type_from_value_type(
                        reg, ast, key_node, value_type,
                    )
                }
            };

            // Try to declare this property in the current scope if it
            // has an authoritative name.
            if qualified_name.is_some() {
                let scope = self.get_l_value_root_scope(compiler, Some(key_node));
                SlotDefiner::new()
                    .for_declaration_node(Some(key_node))
                    .for_variable_name(qualified_name.unwrap())
                    .in_scope(scope)
                    .with_type(key_type)
                    .allow_later_type_inference(key_type.is_none())
                    .define_slot(self, compiler);
            } else if key_type.is_some() {
                self.creator
                    .set_deferred_type(compiler, key_node, key_type.unwrap());
            }

            if key_type.is_some() && obj_lit_type.is_some() && declare_on_owner {
                // Declare this property on its object literal.
                let (reg, ast) = compiler.get_type_registry_and_ast();
                obj_lit_type.unwrap().define_declared_property(
                    reg,
                    ast,
                    member_name,
                    key_type.unwrap(),
                    Some(key_node),
                );
            }
        }
    }

    /// Returns the type specified in a JSDoc annotation near a GETPROP, NAME, object literal
    /// member, or class field.
    ///
    /// Extracts type information from the {@code @type} tag.
    // port: TypedScopeCreator.AbstractScopeBuilder#getDeclaredTypeInAnnotation
    fn get_declared_type_in_annotation(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        info: &JSDocInfo,
    ) -> TypeId {
        check_argument!(info.has_type(), "%s", format!("{info:?}"));
        let owner_type_keys = self.find_owner_type_keys(compiler, Some(node));

        let current_scope_view = self.current_scope.as_static_typed_scope_arc(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let template_scope = if !owner_type_keys.is_empty() {
            reg.create_scope_with_templates(current_scope_view, owner_type_keys)
        } else {
            current_scope_view
        };
        info.get_type()
            .unwrap()
            .evaluate(reg, ast, Some(template_scope))
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#findOwnerTypeKeys
    fn find_owner_type_keys(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: Option<NodeId>,
    ) -> Vec<TypeId> {
        if node.is_some()
            && !node.unwrap().is_static_member(compiler)
            && node
                .unwrap()
                .get_parent(compiler)
                .expect("NullPointerException")
                .is_class_members(compiler)
        {
            let class_type = node
                .unwrap()
                .get_grandparent(compiler)
                .expect("NullPointerException")
                .get_jstype(compiler);
            if class_type.is_some() {
                let (reg, _) = compiler.get_type_registry_and_ast();
                return class_type
                    .unwrap()
                    .to_maybe_function_type(reg)
                    .expect("NullPointerException")
                    .get_template_type_map(reg)
                    .get_template_keys()
                    .to_vec();
            }
            return Vec::new();
        }
        let owner_node = NodeUtil::get_best_l_value_owner(compiler, node);
        let owner_name = NodeUtil::get_best_l_value_name(compiler, owner_node);
        if owner_name.is_some() {
            let owner_var = self.current_scope.get_var(compiler, owner_name.unwrap());
            if owner_var.is_some() {
                let owner_var_type = owner_var.unwrap().get_type(compiler);
                let (reg, _) = compiler.get_type_registry_and_ast();
                let owner_type =
                    self.get_prototype_owner_type(reg, object_type::cast(reg, owner_var_type));
                if owner_type.is_some() {
                    return owner_type
                        .unwrap()
                        .get_template_type_map(reg)
                        .get_template_keys()
                        .to_vec();
                }
            }
        }
        Vec::new()
    }

    /// Asserts that it's OK to define this node's name. The node should have a source name and be
    /// of the specified type.
    // port: TypedScopeCreator.AbstractScopeBuilder#assertDefinitionNode
    fn assert_definition_node(&self, compiler: &AbstractCompiler, n: NodeId, type_: Token) {
        check_state!(self.source_name.is_some());
        check_state!(n.get_token(compiler) == type_, "%s", n.to_string(compiler));
    }

    /// Defines a catch parameter.
    // port: TypedScopeCreator.AbstractScopeBuilder#defineCatch
    fn define_catch(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.assert_definition_node(compiler, n, Token::CATCH);
        // Though almost certainly a terrible idea, it is possible to do destructuring in
        // the catch declaration.
        // e.g. `} catch ({message, errno}) {`
        NodeUtil::visit_lhs_nodes_in_node(compiler, n, &mut |compiler, catch_name| {
            let info = catch_name.get_jsdoc_info(compiler);
            let type_ = self.get_declared_type(compiler, info, catch_name, None, None);
            let current_scope = self.current_scope;
            SlotDefiner::new()
                .for_declaration_node(Some(catch_name))
                .for_variable_name(catch_name.get_string(compiler))
                .in_scope(current_scope)
                .with_type(type_)
                .allow_later_type_inference(type_.is_none())
                .define_slot(self, compiler);
        });
    }

    /// Defines an assignment to a name as if it were an actual `var` declaration.
    // port: TypedScopeCreator.AbstractScopeBuilder#defineAssignAsIfVarDeclaration
    fn define_assign_as_if_var_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        assignment: NodeId,
    ) {
        let info = assignment.get_jsdoc_info(compiler);
        let name = assignment
            .get_first_child(compiler)
            .expect("NullPointerException");
        check_argument!(name.is_name(compiler), "%s", name.to_string(compiler));
        let rvalue = assignment.get_second_child(compiler);
        let scope = self
            .current_scope
            .get_closest_hoist_scope(compiler)
            .expect("NullPointerException");
        self.define_name(compiler, name, rvalue, scope, info);
    }

    /// Defines a variable declared with `var`, `let`, or `const`.
    // port: TypedScopeCreator.AbstractScopeBuilder#defineVars
    fn define_vars(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        check_state!(self.source_name.is_some());
        check_state!(NodeUtil::is_name_declaration(compiler, Some(n)));
        let info = n.get_jsdoc_info(compiler);
        // `var` declarations are hoisted, but `let` and `const` are not.
        let scope = if n.is_var(compiler) {
            self.current_hoist_scope
        } else {
            self.current_scope
        };

        if n.has_more_than_one_child(compiler) && info.is_some() {
            let error = JSError::make(compiler, n, &MULTIPLE_VAR_DEF, &[]);
            self.creator.report(compiler, error);
        }

        let mut child_cursor = n.get_first_child(compiler);
        while let Some(child) = child_cursor {
            child_cursor = child.get_next(compiler);
            self.define_var_child(compiler, info.clone(), child, scope);
        }
        if n.has_one_child(compiler) && {
            let only_child = n.get_only_child(compiler);
            let jsdoc = n.get_jsdoc_info(compiler);
            self.is_valid_typedef_declaration(compiler, only_child, jsdoc)
        } {
            let only_child = n.get_only_child(compiler);
            let jsdoc = n.get_jsdoc_info(compiler);
            self.declare_typedef_type(compiler, only_child, jsdoc);
        }
    }

    /// Defines a variable declared with `var`, `let`, or `const`.
    // port: TypedScopeCreator.AbstractScopeBuilder#defineVarChild
    fn define_var_child(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut declaration_info: Option<Arc<JSDocInfo>>,
        child: NodeId,
        scope: TypedScope,
    ) {
        if child.is_name(compiler) {
            if declaration_info.is_none() {
                declaration_info = child.get_jsdoc_info(compiler);
                // TODO(bradfordcsmith): Report an error if both the declaration node and the name
                //     itself have JSDoc.
            }
            let first_child = child.get_first_child(compiler);
            self.define_name(compiler, child, first_child, scope, declaration_info);
        } else {
            check_state!(
                child.is_destructuring_lhs(compiler),
                "%s",
                child.to_string(compiler)
            );
            let pattern = child
                .get_first_child(compiler)
                .expect("NullPointerException");
            let value = child.get_second_child(compiler);

            if ModuleImportResolver::is_goog_module_dependency_call(compiler, value) {
                // Define destructuring names here, since goog.require destructuring patterns can
                // only have one level and require some special handling.
                let default_import = self
                    .creator
                    .module_import_resolver
                    .get_closure_namespace_type_from_call(compiler, value.unwrap());
                let mut key_cursor = pattern.get_first_child(compiler);
                while let Some(key) = key_cursor {
                    key_cursor = key.get_next(compiler);
                    let key_first_child =
                        key.get_first_child(compiler).expect("NullPointerException");
                    let key_string = key.get_string(compiler);
                    self.define_module_import(
                        compiler,
                        key_first_child,
                        default_import.clone(),
                        Some(key_string),
                        scope,
                    );
                }
                return;
            }

            self.define_destructuring_pattern_in_var_declaration(
                compiler,
                pattern,
                scope,
                // Note that value will be null if we are in an enhanced for loop
                //   for (const {x, y} of data) {
                Rc::new(RValueInfoSupplier::VarChildValue(value)),
            );
        }
    }

    /// Rust-only: `patternTypeSupplier.get()` (Java's `Supplier<RValueInfo>` lambdas, evaluated
    /// again on every call like Guava's plain Supplier).
    fn get_r_value_info(
        &mut self,
        compiler: &mut AbstractCompiler,
        supplier: &RValueInfoSupplier,
    ) -> RValueInfo {
        match supplier {
            RValueInfoSupplier::VarChildValue(value) => {
                // value != null
                //     ? new RValueInfo(getDeclaredRValueType(null, value), value.getQualifiedNameObject())
                //     : new RValueInfo(unknownType, null)
                if value.is_some() {
                    let type_ = self.get_declared_r_value_type(compiler, None, value.unwrap());
                    RValueInfo::new(type_, value.unwrap().get_qualified_name_object(compiler))
                } else {
                    RValueInfo::new(
                        Some(self.creator.unknown_type),
                        /* qualifiedName= */ None,
                    )
                }
            }
            RValueInfoSupplier::DestructuredTarget {
                target,
                pattern_type_supplier,
            } => self.infer_type_for_destructured_target(compiler, target, pattern_type_supplier),
        }
    }

    /// Returns information about the qualified name and type of the target, if it exists.
    ///
    /// Never returns null, but will return an RValueInfo with null `type` and `qualifiedName`
    /// slots.
    // port: TypedScopeCreator.AbstractScopeBuilder#inferTypeForDestructuredTarget
    fn infer_type_for_destructured_target(
        &mut self,
        compiler: &mut AbstractCompiler,
        target: &DestructuredTarget,
        pattern_type_supplier: &RValueInfoSupplier,
    ) -> RValueInfo {
        // Currently we only do type inference for string key nodes in object patterns here, to
        // handle aliasing types. e.g
        //   const {Foo} = ns;
        // TypeInference takes care of the rest.
        // Note that although DestructuredTarget includes logic for inferring types, we don't use
        // it here because we only do some very limited type inference during TypedScopeCreation,
        // and only return a non-null type here if we are accessing a declared property on a known
        // type.
        if !target.has_string_key(compiler) || target.has_default_value() {
            return RValueInfo::empty();
        }
        let rvalue = self.get_r_value_info(compiler, pattern_type_supplier);
        let pattern_type = rvalue.type_;
        let property_name = target
            .get_string_key(compiler)
            .unwrap()
            .get_string(compiler);
        let qname = if rvalue.qualified_name.is_some() {
            Some(
                rvalue
                    .qualified_name
                    .as_ref()
                    .unwrap()
                    .getprop(property_name.clone()),
            )
        } else {
            None
        };
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if pattern_type.is_none() || pattern_type.unwrap().is_unknown_type(reg, ast) {
            return RValueInfo::new(None, qname);
        }
        if pattern_type
            .unwrap()
            .has_property(reg, ast, property_name.clone())
        {
            let type_ = pattern_type
                .unwrap()
                .find_property_type(reg, ast, property_name);
            return RValueInfo::new(type_, qname);
        }
        RValueInfo::new(None, qname)
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#defineDestructuringPatternInVarDeclaration
    fn define_destructuring_pattern_in_var_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
        scope: TypedScope,
        pattern_type_supplier: Rc<RValueInfoSupplier>,
    ) {
        // Java: `() -> patternTypeSupplier.get().type`. TypedScopeCreator never asks these targets
        // to infer a type (it reads only hasStringKey, hasDefaultValue, getStringKey and getNode),
        // so the Java supplier is never run on this path; the Rust supplier, which cannot reach
        // this builder, says so if it ever is.
        let target_pattern_type: PatternTypeSupplier = Rc::new(|_, _| {
            panic!("TypedScopeCreator's destructuring targets never infer their pattern type")
        });
        for target in DestructuredTarget::create_all_non_empty_targets_in_pattern_with_supplier(
            compiler,
            target_pattern_type,
            pattern,
        ) {
            let target = Rc::new(target);
            let type_supplier = Rc::new(RValueInfoSupplier::DestructuredTarget {
                target: Rc::clone(&target),
                pattern_type_supplier: Rc::clone(&pattern_type_supplier),
            });

            if target.get_node().is_destructuring_pattern(compiler) {
                self.define_destructuring_pattern_in_var_declaration(
                    compiler,
                    target.get_node(),
                    scope,
                    type_supplier,
                );
            } else {
                let name = target.get_node();
                check_state!(
                    name.is_name(compiler),
                    "This method is only for declaring variables: %s",
                    name.to_string(compiler)
                );

                // variable's type
                let info = name.get_jsdoc_info(compiler);
                let mut type_ = self.get_declared_type(
                    compiler,
                    info,
                    name,
                    /* rValue= */ None,
                    Some(&type_supplier),
                );
                if type_.is_none() {
                    // The variable's type will be inferred.
                    type_ = if name.is_from_externs(compiler) {
                        Some(self.creator.unknown_type)
                    } else {
                        None
                    };
                }
                SlotDefiner::new()
                    .for_declaration_node(Some(name))
                    .for_variable_name(name.get_string(compiler))
                    .in_scope(scope)
                    .with_type(type_)
                    .allow_later_type_inference(type_.is_none())
                    .define_slot(self, compiler);
            }
        }
    }

    /// Defines a class literal. Handles any of the following cases:
    ///
    /// - Class declarations: `class Foo { ... }`
    /// - Class assignments: `foo.Bar = class { ... }`
    /// - Bleeding names: `foo.Bar = class Baz { ... }`
    /// - Properties: `{foo: class { ... }}`
    /// - Callbacks: `foo(class { ... })`
    // port: TypedScopeCreator.AbstractScopeBuilder#defineClassLiteral
    fn define_class_literal(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.assert_definition_node(compiler, n, Token::CLASS);

        // Determine the name and JSDocInfo and l-value for the class.
        // Any of these may be null.
        let l_value = NodeUtil::get_best_l_value(compiler, n);
        let info = NodeUtil::get_best_jsdoc_info(compiler, n);
        let class_name = NodeUtil::get_best_l_value_name(compiler, l_value);
        let class_type_identifier = self.get_best_type_name(compiler, l_value, class_name.clone());

        // Create the type and assign it on the CLASS node.
        let class_type = self.create_class_type_from_nodes(
            compiler,
            n,
            class_type_identifier,
            class_name.clone(),
            info,
            l_value,
        );
        self.creator.set_deferred_type(compiler, n, class_type);

        // Declare this symbol in the current scope iff it's a class
        // declaration. Otherwise, the declaration will happen in other
        // code paths.
        if NodeUtil::is_class_declaration(compiler, n) {
            let class_name = check_not_null!(class_name);
            let current_scope = self.current_scope;
            SlotDefiner::new()
                .for_declaration_node(n.get_first_child(compiler))
                .for_variable_name(class_name)
                .in_scope(current_scope)
                .with_type(Some(class_type))
                .allow_later_type_inference(false)
                .define_slot(self, compiler);
        }
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#getBestTypeName
    fn get_best_type_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        lvalue: Option<NodeId>,
        syntactic_lvalue_name: Option<JsString>,
    ) -> Option<JsString> {
        let syntactic_lvalue_name = syntactic_lvalue_name?;
        if self.is_goog_module_exports(compiler, lvalue) {
            let closure_namespace = self
                .get_module(compiler)
                .expect("NullPointerException")
                .closure_namespace()
                .cloned()
                .expect("NullPointerException");
            return Some(
                syntactic_lvalue_name.replace(&JsString::from("exports"), &closure_namespace),
            );
        }
        Some(syntactic_lvalue_name)
    }

    /// Defines a function literal.
    // port: TypedScopeCreator.AbstractScopeBuilder#defineFunctionLiteral
    fn define_function_literal(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.assert_definition_node(compiler, n, Token::FUNCTION);

        // Determine the name and JSDocInfo and l-value for the function.
        // Any of these may be null.
        let l_value = NodeUtil::get_best_l_value(compiler, n);
        let info = NodeUtil::get_best_jsdoc_info(compiler, n);
        let function_name = NodeUtil::get_best_l_value_name(compiler, l_value);
        let function_type = self.create_function_type_from_nodes(
            compiler,
            Some(n),
            function_name.clone(),
            info,
            l_value,
        );

        // Assigning the function type to the function node
        self.creator.set_deferred_type(compiler, n, function_type);

        // Declare this symbol in the current scope iff it's a function
        // declaration. Otherwise, the declaration will happen in other
        // code paths.
        if NodeUtil::is_function_declaration(compiler, n) {
            let current_scope = self.current_scope;
            SlotDefiner::new()
                .for_declaration_node(n.get_first_child(compiler))
                .for_variable_name(function_name.expect("NullPointerException"))
                .in_scope(current_scope)
                .with_type(Some(function_type))
                .allow_later_type_inference(false)
                .define_slot(self, compiler);
        }
    }

    /// Defines a variable based on the {@link Token#NAME} node passed.
    ///
    /// @param name The {@link Token#NAME} node.
    /// @param value Optionally, the value assigned to the name node.
    /// @param info the {@link JSDocInfo} information relating to this {@code name} node.
    // port: TypedScopeCreator.AbstractScopeBuilder#defineName
    fn define_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        name: NodeId,
        value: Option<NodeId>,
        scope: TypedScope,
        info: Option<Arc<JSDocInfo>>,
    ) {
        if ModuleImportResolver::is_goog_module_dependency_call(compiler, value) {
            let imported_module_object = self
                .creator
                .module_import_resolver
                .get_closure_namespace_type_from_call(compiler, value.unwrap());
            self.define_module_import(compiler, name, imported_module_object, None, scope);
            return;
        }
        let mut type_ = self.get_declared_type(
            compiler, info, name, value, /* declaredRValueTypeSupplier= */ None,
        );
        if type_.is_none() {
            // The variable's type will be inferred.
            type_ = if name.is_from_externs(compiler) {
                Some(self.creator.unknown_type)
            } else {
                None
            };
        }
        SlotDefiner::new()
            .for_declaration_node(Some(name))
            .for_variable_name(name.get_string(compiler))
            .in_scope(scope)
            .with_type(type_)
            .allow_later_type_inference(type_.is_none())
            .define_slot(self, compiler);
    }

    /// @param localNameNode The name node of the LHS of the import being defined
    /// @param importedModuleObject The root node of the scope in which the type being imported was
    ///     defined, along with the local name of the overall module object inside the scope where
    ///     it was defined. Or null if no module with that name exists.
    /// @param optionalProperty The property name of the locally imported type on the module
    ///     object, if destructuring-style importing was used. Or null if this is a namespace
    ///     import.
    /// @param scopeToDeclareIn The scope in which localNameNode is defined
    // port: TypedScopeCreator.AbstractScopeBuilder#defineModuleImport
    fn define_module_import(
        &mut self,
        compiler: &mut AbstractCompiler,
        local_name_node: NodeId,
        imported_module_object: Option<Simple>,
        optional_property: Option<JsString>,
        scope_to_declare_in: TypedScope,
    ) {
        if imported_module_object.is_none() {
            // We could not find the module defining this import. If the compiler is assuming
            // forward declared types (or if explicitly forward declared), create a NamedType so it
            // resolves to NoResolvedType. Otherwise, fall back to unknownType to avoid spurious
            // type check errors.
            let fallback_type = if compiler
                .get_options()
                .assume_forward_declared_for_missing_types()
            {
                let current_scope_view = self.current_scope.as_static_typed_scope_arc(compiler);
                let reference = local_name_node.get_string(compiler);
                // Java passes a null source file name through; closure-jstype's createNamedType
                // takes a String.
                let source_name = local_name_node
                    .get_source_file_name(compiler)
                    .unwrap_or_default();
                let lineno = local_name_node.get_lineno(compiler);
                let charno = local_name_node.get_charno(compiler);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                reg.create_named_type(
                    ast,
                    Some(current_scope_view),
                    reference,
                    source_name,
                    lineno,
                    charno,
                )
            } else {
                self.creator.unknown_type
            };
            let allow_later_type_inference = {
                let assume_forward_declared = compiler
                    .get_options()
                    .assume_forward_declared_for_missing_types();
                let (reg, ast) = compiler.get_type_registry_and_ast();
                fallback_type.is_unknown_type(reg, ast) && !assume_forward_declared
            };
            SlotDefiner::new()
                .for_declaration_node(Some(local_name_node))
                .for_variable_name(local_name_node.get_string(compiler))
                .in_scope(scope_to_declare_in)
                .with_type(Some(fallback_type))
                .allow_later_type_inference(allow_later_type_inference)
                .define_slot(self, compiler);
            return;
        }
        let imported_module_object = imported_module_object.unwrap();

        let exported_name: Simple = if optional_property.is_none() {
            imported_module_object
        } else {
            // If this is a destucuring-style import, find its type.
            <dyn ScopedName>::of(
                imported_module_object
                    .get_name(compiler)
                    .concat(&JsString::from("."))
                    .concat(&optional_property.unwrap()),
                imported_module_object.get_scope_root(compiler),
            )
        };
        // Try getting the actual scope. The scope will be null in the following cases:
        //   - Someone has required a module that does not exist at all.
        //   - Someone has requireType'd or forwardDeclare'd a module that exists, but does not
        //     have an associated scope yet.
        let export_scope = if exported_name.get_scope_root(compiler).is_some() {
            self.creator
                .memoized
                .get(&exported_name.get_scope_root(compiler).unwrap())
                .copied()
        } else {
            None
        };

        // The scope is null for modules that were not visited yet.
        if export_scope.is_some() {
            let type_ = lookup_qualified_name(
                compiler,
                export_scope.unwrap(),
                &QualifiedName::of(exported_name.get_name(compiler)),
            );

            // The type is null if either the name is inferred or this is an early ref.
            if type_.is_some() {
                self.declare_alias_type_if_rvalue_is_aliasable(
                    compiler,
                    local_name_node,
                    Some(QualifiedName::of(exported_name.get_name(compiler))),
                    type_,
                    export_scope.unwrap(),
                );

                SlotDefiner::new()
                    .for_declaration_node(Some(local_name_node))
                    .for_variable_name(local_name_node.get_string(compiler))
                    .in_scope(scope_to_declare_in)
                    .with_type(type_)
                    .allow_later_type_inference(type_.is_none())
                    .define_slot(self, compiler);
                return;
            }
        }
        // Defer defining this name until after we have visited the entire AST.
        let weak_import = WeakModuleImport::new(
            compiler,
            local_name_node,
            exported_name,
            scope_to_declare_in,
        );
        self.creator.weak_imports.push(weak_import);
    }

    /// If a variable is assigned a function literal in the global scope, make that a declared
    /// type (even if there's no doc info). There's only one exception to this rule: if the return
    /// type is inferred, and we're in a local scope, we should assume the whole function is
    /// inferred.
    // port: TypedScopeCreator.AbstractScopeBuilder#shouldUseFunctionLiteralType
    fn should_use_function_literal_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        type_: Option<TypeId>,
        info: Option<&JSDocInfo>,
        l_value: Option<NodeId>,
    ) -> bool {
        if info.is_some() {
            return true;
        }
        if l_value.is_some() && NodeUtil::may_be_object_lit_key(compiler, l_value.unwrap()) {
            return false;
        }
        // TODO(johnlenz): consider unifying global and local behavior
        self.is_l_value_rooted_in_global_scope(compiler, l_value) || {
            let (reg, _) = compiler.get_type_registry_and_ast();
            !type_
                .expect("NullPointerException")
                .is_return_type_inferred(reg)
        }
    }

    /// Creates a new class type from the given class literal. This function does not need to
    /// worry about stubs and aliases because they are handled by createFunctionTypeFromNodes
    /// instead.
    // port: TypedScopeCreator.AbstractScopeBuilder#createClassTypeFromNodes
    fn create_class_type_from_nodes(
        &mut self,
        compiler: &mut AbstractCompiler,
        clazz: NodeId,
        name: Option<JsString>,
        syntactic_name: Option<JsString>,
        info: Option<Arc<JSDocInfo>>,
        lvalue_node: Option<NodeId>,
    ) -> TypeId {
        check_argument!(clazz.is_class(compiler), "%s", clazz.to_string(compiler));

        let mut builder = FunctionTypeBuilder::new(name, compiler, clazz, self.current_scope);
        builder
            .using_class_syntax()
            .set_syntactic_function_name(syntactic_name)
            .set_contents(Some(Arc::new(AstFunctionContents::new(clazz))));
        let declaration_scope = if lvalue_node.is_some() {
            self.get_l_value_root_scope(compiler, lvalue_node)
        } else {
            self.current_scope
        };
        builder
            .set_declaration_scope(declaration_scope)
            .infer_kind(compiler, info.as_deref())
            .infer_template_type_name(compiler, info.as_deref(), None);

        let extends_clause = clazz
            .get_second_child(compiler)
            .expect("NullPointerException");

        // Look at the extends clause and/or JSDoc info to find a super class.  Use generics from
        // the JSDoc to supplement the extends type when available.
        let base_type = self.find_super_class_from_nodes(compiler, extends_clause, info.as_deref());
        builder.infer_inheritance(compiler, info.as_deref(), base_type);

        // Look for an explicit constructor.
        let mut constructor =
            NodeUtil::get_es6_class_constructor_member_function_def(compiler, clazz);
        if constructor.is_some() {
            constructor = Some(constructor.unwrap().get_only_child(compiler)); // We want the FUNCTION, not the member.
        }

        if constructor.is_some() {
            // Note: constructor should have the following structure:
            //   MEMBER_FUNCTION_DEF [jsdoc_info]
            //     FUNCTION
            //       NAME
            //       PARAM_LIST ...
            //       BLOCK ...
            // NodeUtil.getFirstPropMatchingKey returns the FUNCTION node.
            let ctor_info = NodeUtil::get_best_jsdoc_info(compiler, constructor.unwrap());
            let args_parent = constructor
                .unwrap()
                .get_second_child(compiler)
                .expect("NullPointerException");
            builder.infer_constructor_parameters(compiler, args_parent, ctor_info.as_deref());
        } else if extends_clause.is_empty(compiler) {
            // No explicit constructor and no superclass: constructor is no-args.
            builder.infer_implicit_constructor_parameters(Vec::new());
        } else {
            // No explicit constructor, but we have a superclass.  If we know its type, then copy
            // its constructor arguments (and templates).  If not, make the constructor arguments
            // unknown.
            // TODO(sdh): consider allowing attaching constructor @param annotations somewhere
            // else?
            let (reg, _) = compiler.get_type_registry_and_ast();
            let extends_ctor = if base_type.is_some() {
                base_type.unwrap().get_constructor(reg)
            } else {
                None
            };
            if extends_ctor.is_some() {
                // Known superclass: copy the parameters node.
                let parameters = extends_ctor.unwrap().get_parameters(reg);
                builder.infer_implicit_constructor_parameters(parameters);
            } else {
                // Unresolveable extends clause: suppress typechecking.
                let unknown_type = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
                let parameters = reg.create_parameters_with_var_args(&[unknown_type]);
                builder.infer_implicit_constructor_parameters(parameters);
            }
        }

        // TODO(sdh): Handle template parameters.  The constructor should store all parameters,
        // while the instance type should only have the class-level parameters?

        // Add the type for the "constructor" property.
        let class_type = builder.build_and_register(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if class_type.is_constructor(reg) {
            // NOTE: This logic is similar to the goog.inherits handling in
            // ClosureCodingConvention#applySubclassRelationship. If this logic is modified
            // it is likely that code needs to be modified as well.

            // Notice that constructor functions do not need to be covariant on the superclass.
            // So if G extends F, new G() and new F() can accept completely different argument
            // types, but G.prototype.constructor needs to be covariant on F.prototype.constructor.
            // To get around this, we just turn off type-checking on arguments and return types
            // of G.prototype.constructor.

            // NOTE: For final classes we could do better here and retain the parameter types.

            let qmark_ctor = class_type.forget_parameter_and_return_types(reg, ast);
            let class_prototype_type = class_type.get_prototype_property(reg, ast);
            class_prototype_type.define_declared_property(
                reg,
                ast,
                "constructor",
                qmark_ctor,
                constructor,
            );
        }
        if class_type.has_instance_type(reg) {
            let class_prototype = class_type
                .get_slot(reg, ast, "prototype")
                .expect("NullPointerException");
            // SymbolTable users expect the class prototype and actual class to have the same
            // declaration node.
            let node = if lvalue_node.is_some() {
                lvalue_node
            } else {
                class_prototype.get_node(reg)
            };
            class_prototype.set_node(reg, node);
        }

        class_type
    }

    /// Look at the {@code extends} clause to find the instance type being extended. Returns {@code
    /// null} if there is no such clause, and unknown if the type cannot be determined.
    // port: TypedScopeCreator.AbstractScopeBuilder#findSuperClassFromNodes
    fn find_super_class_from_nodes(
        &mut self,
        compiler: &mut AbstractCompiler,
        extends_node: NodeId,
        info: Option<&JSDocInfo>,
    ) -> Option<TypeId> {
        if extends_node.is_empty(compiler) {
            // No extends clause: return null.
            return None;
        }
        let mut ctor_type = extends_node.get_jstype(compiler);
        if ctor_type.is_none() {
            if extends_node.is_qualified_name(compiler) {
                let superclass = extends_node
                    .get_qualified_name(compiler)
                    .expect("NullPointerException");
                // Look up qualified names in the scope (types won't be set on the AST until
                // inference).
                ctor_type = lookup_qualified_name(
                    compiler,
                    self.current_scope,
                    &QualifiedName::of(superclass.clone()),
                );
                if ctor_type.is_none() {
                    let var = self.current_scope.get_var(compiler, superclass.clone());
                    ctor_type = if var.is_some() {
                        var.unwrap().get_type(compiler)
                    } else {
                        None
                    };
                }
                // If that doesn't work, then fall back on the registry
                if ctor_type.is_none() {
                    let current_scope_view = self.current_scope.as_static_typed_scope_arc(compiler);
                    // Java passes a null source file name through; closure-jstype's getType takes
                    // a String.
                    let source_name = extends_node
                        .get_source_file_name(compiler)
                        .unwrap_or_default();
                    let lineno = extends_node.get_lineno(compiler);
                    let charno = extends_node.get_charno(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let type_ = reg.get_type_with_location(
                        ast,
                        Some(current_scope_view),
                        superclass,
                        source_name,
                        lineno,
                        charno,
                    );
                    return object_type::cast(reg, Some(type_));
                }
            } else if self.is_goog_module_get_property(compiler, extends_node) {
                ctor_type = self.get_ctor_for_goog_module_get(compiler, extends_node);
            } else {
                // Anything TypedScopeCreator can infer has already been read off the AST.  This
                // is likely a CALL or GETELEM, which are unknown until TypeInference.  Instead,
                // ignore it for now, require an @extends tag in the JSDoc, and verify correctness
                // in TypeCheck.
                if info.is_none() || !info.unwrap().has_base_type() {
                    let error =
                        JSError::make(compiler, extends_node, &DYNAMIC_EXTENDS_WITHOUT_JSDOC, &[]);
                    self.creator.report(compiler, error);
                }
            }
        }

        if ctor_type.is_some() {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let ctor_type = ctor_type.unwrap();
            if ctor_type.is_constructor(reg) || ctor_type.is_interface(reg) {
                return ctor_type
                    .to_maybe_function_type(reg)
                    .unwrap()
                    .get_instance_type(reg);
            } else if ctor_type.is_unknown_type(reg, ast) || ctor_type.is_no_resolved_type(reg) {
                // The constructor could have an unknown type for cases where it is dynamically
                // created or passed in from elsewhere.
                // e.g. with a mixin pattern
                // function mixinSomething(ctor) {
                //   return class extends ctor { ... };
                // }
                // In that case consider the super class instance type to be unknown.
                // On the other hand, the constructor can be a NoResolvedType if running in Clutz's
                // partial compilation mode.
                return ctor_type.to_maybe_object_type(reg);
            }
        }

        // We couldn't determine the type, so for TypedScope creation purposes we will treat it as
        // if there were no extends clause.  TypeCheck will give a more precise error later.
        None
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#isGoogModuleGetProperty
    fn is_goog_module_get_property(
        &self,
        compiler: &AbstractCompiler,
        extends_clause: NodeId,
    ) -> bool {
        if extends_clause.is_call(compiler) {
            ModuleImportResolver::is_goog_module_dependency_call(compiler, Some(extends_clause))
        } else if extends_clause.is_get_prop(compiler) {
            self.is_goog_module_get_property(
                compiler,
                extends_clause
                    .get_first_child(compiler)
                    .expect("NullPointerException"),
            )
        } else {
            false
        }
    }

    /// Looks up the type of a goog.module.get property access
    ///
    /// @param extendsNode `goog.module.get('x');` or a getprop `goog.module.get('x').y.z
    // port: TypedScopeCreator.AbstractScopeBuilder#getCtorForGoogModuleGet
    fn get_ctor_for_goog_module_get(
        &mut self,
        compiler: &mut AbstractCompiler,
        extends_node: NodeId,
    ) -> Option<TypeId> {
        let mut call = extends_node;
        let mut properties: Vec<JsString> = Vec::new();
        while !call.is_call(compiler) {
            properties.insert(0, call.get_string(compiler));
            call = call
                .get_first_child(compiler)
                .expect("NullPointerException");
        }
        let extends_call = self
            .creator
            .module_import_resolver
            .get_closure_namespace_type_from_call(compiler, call);
        if extends_call.is_none()
            || !extends_call
                .as_ref()
                .unwrap()
                .get_scope_root(compiler)
                .is_some_and(|root| self.creator.memoized.contains_key(&root))
        {
            // Handle invalid goog.module.get calls
            return None;
        }
        let extends_call = extends_call.unwrap();
        let module_scope = self.creator.memoized[&extends_call.get_scope_root(compiler).unwrap()];
        properties.insert(0, extends_call.get_name(compiler));
        // Joiner.on('.').join(properties)
        let mut joined = JsString::from("");
        for (i, property) in properties.iter().enumerate() {
            if i > 0 {
                joined = joined.concat(&JsString::from("."));
            }
            joined = joined.concat(property);
        }
        let superclass_name = QualifiedName::of(joined);
        lookup_qualified_name(compiler, module_scope, &superclass_name)
    }

    /// Creates a new function type, based on the given nodes.
    ///
    /// This handles two cases that are semantically very different, but are not mutually
    /// exclusive: - A function literal that needs a type attached to it (called from
    /// defineClassLiteral with a non-null FUNCTION node for rValue). - An assignment expression
    /// with function-type info in the JsDoc (called from getDeclaredType on a stub (rValue ==
    /// null) or alias (rValue is a qualified name).
    ///
    /// All parameters are optional, and we will do the best we can to create a function type.
    ///
    /// This function will always create a function type, so only call it if you're sure that's
    /// what you want.
    ///
    /// @param rValue The function node.
    /// @param name the function's name
    /// @param info the {@link JSDocInfo} attached to the function definition
    /// @param lvalueNode The node where this function is being assigned. For example, {@code
    ///     A.prototype.foo = ...} would be used to determine that this function is a method of
    ///     A.prototype. May be null to indicate that this is not being assigned to a qualified
    ///     name.
    // port: TypedScopeCreator.AbstractScopeBuilder#createFunctionTypeFromNodes
    fn create_function_type_from_nodes(
        &mut self,
        compiler: &mut AbstractCompiler,
        r_value: Option<NodeId>,
        name: Option<JsString>,
        info: Option<Arc<JSDocInfo>>,
        lvalue_node: Option<NodeId>,
    ) -> TypeId {
        // Check for an alias.
        if r_value.is_some()
            && r_value.unwrap().is_qualified_name(compiler)
            && lvalue_node.is_some()
        {
            let qualified_name = r_value
                .unwrap()
                .get_qualified_name(compiler)
                .expect("NullPointerException");
            let var = self.current_scope.get_var(compiler, qualified_name);
            let var_type = var.and_then(|var| var.get_type(compiler));
            let is_function_type = {
                let (reg, _) = compiler.get_type_registry_and_ast();
                var_type.is_some_and(|var_type| var_type.is_function_type(reg))
            };
            if var.is_some() && var_type.is_some() && is_function_type {
                let (aliased_type, is_ctor_or_iface) = {
                    let (reg, _) = compiler.get_type_registry_and_ast();
                    let aliased_type = var_type.unwrap().to_maybe_function_type(reg).unwrap();
                    (
                        aliased_type,
                        aliased_type.is_constructor(reg) || aliased_type.is_interface(reg),
                    )
                };
                if is_ctor_or_iface {
                    // TODO(nick): Remove this. This should already be handled by normal type
                    // resolution.
                    if name.is_some() {
                        let current_scope_view = self.current_scope.as_static_typed_scope(compiler);
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        let instance_type = aliased_type
                            .get_instance_type(reg)
                            .expect("NullPointerException");
                        reg.declare_type(
                            ast,
                            Some(current_scope_view),
                            name.clone().unwrap(),
                            instance_type,
                        );
                    }
                    self.check_function_alias_annotations(
                        compiler,
                        lvalue_node.unwrap(),
                        aliased_type,
                        info.as_deref(),
                    );
                    return aliased_type;
                }
            }
        }

        // No alias: look for an explicit @type in JSDocInfo.
        if info.is_some() && info.as_ref().unwrap().has_type() {
            let current_scope_view = self.current_scope.as_static_typed_scope_arc(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let mut type_ = info.as_ref().unwrap().get_type().unwrap().evaluate(
                reg,
                ast,
                Some(current_scope_view),
            );

            // Known to be not null since we have the FUNCTION token there.
            type_ = type_.restrict_by_not_null_or_undefined(reg, ast);
            if type_.is_function_type(reg) {
                let function_type = type_.to_maybe_function_type(reg).unwrap();
                function_type.set_jsdoc_info(reg, info.clone());
                return function_type;
            }
        }

        // No alias or explicit @type, so look for a function literal, or @param/@return.
        let error_root = if r_value.is_none() {
            lvalue_node
        } else {
            r_value
        };
        let is_fn_literal = r_value.is_some() && r_value.unwrap().is_function(compiler);
        let fn_root = if is_fn_literal { r_value } else { None };
        let parameters_node = if is_fn_literal {
            r_value.unwrap().get_second_child(compiler)
        } else {
            None
        };

        // If this function is being assigned as a property on a type, try finding the owner type
        // and the property name.
        // This is easy to do for class members because the owner type is on the CLASS node and
        // the property name is the MEMBER_FUNCTION_DEF/GETTER_DEF/SETTER_DEF string.
        // For other functions, we rely on NodeUtil.getBestLValueOwner.
        let class_root = if lvalue_node.is_some()
            && lvalue_node
                .unwrap()
                .get_parent(compiler)
                .expect("NullPointerException")
                .is_class_members(compiler)
        {
            lvalue_node.unwrap().get_grandparent(compiler)
        } else {
            None
        };
        let owner_node = NodeUtil::get_best_l_value_owner(compiler, lvalue_node);

        let mut owner_type: Option<TypeId> = None;
        let mut prop_name: Option<JsString> = None;
        if class_root.is_some() {
            // Static members are owned by the constructor, non-statics are owned by the prototype.
            let class_root_type = class_root.unwrap().get_jstype(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            // JSType.toMaybeFunctionType(classRoot.getJSType())
            owner_type = class_root_type.and_then(|t| t.to_maybe_function_type(reg));
            if !lvalue_node.unwrap().is_static_member(ast) && owner_type.is_some() {
                owner_type = Some(owner_type.unwrap().get_prototype(reg, ast));
            }
            prop_name = if lvalue_node.unwrap().is_computed_prop(ast) {
                None
            } else {
                Some(lvalue_node.unwrap().get_string(ast))
            };
        } else {
            let owner_name = NodeUtil::get_best_l_value_name(compiler, owner_node);
            if owner_name.is_some() {
                let looked_up = lookup_qualified_name(
                    compiler,
                    self.current_scope,
                    &QualifiedName::of(owner_name.clone().unwrap()),
                );
                let (reg, _) = compiler.get_type_registry_and_ast();
                owner_type = object_type::cast(reg, looked_up);
            }

            if owner_name.is_some() && name.is_some() {
                // TODO(b/111621092): Use the AST rather than manipulating strings here.
                let owner_name = owner_name.unwrap();
                let name = name.as_ref().unwrap();
                check_state!(
                    name.starts_with(&owner_name),
                    "Expected \"%s\" to start with \"%s\"",
                    name.to_string(),
                    owner_name.to_string()
                );
                prop_name = Some(name.substring_from(owner_name.length() + 1));
            }
        }

        let (prototype_owner, prototype_owner_type_map) = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            let prototype_owner = self.get_prototype_owner_type(reg, owner_type);
            let mut prototype_owner_type_map = None;
            if prototype_owner.is_some() && prototype_owner.unwrap().get_type_of_this(reg).is_some()
            {
                prototype_owner_type_map = Some(
                    prototype_owner
                        .unwrap()
                        .get_type_of_this(reg)
                        .unwrap()
                        .get_template_type_map(reg),
                );
            }
            (prototype_owner, prototype_owner_type_map)
        };

        // Find the type of any overridden function.
        let mut overridden_type: Option<TypeId> = None;
        if owner_type.is_some() && prop_name.is_some() {
            // the type of the property this overrides, not necessarily a function.
            let overridden_prop_type = self.find_overridden_property(
                compiler,
                owner_type.unwrap(),
                prop_name.clone().unwrap(),
                prototype_owner_type_map,
            );
            if overridden_prop_type.is_some() {
                // Overridden getters and setters need special handling because we declare
                // getters/setters as simple properties with their respective return/parameter
                // type. This causes a split during inference where left and right sides of a
                // getter/setter declaration will be inferred to have different types; if the left
                // side has type `T`, the right side will be some function type involving `T`.
                let lvalue = lvalue_node.expect("NullPointerException");
                // (side-effect free; read before the getter/setter checks for the borrow checker)
                let overridden_prop_type_is_function_type = {
                    let (reg, _) = compiler.get_type_registry_and_ast();
                    overridden_prop_type.unwrap().is_function_type(reg)
                };
                if lvalue.is_getter_def(compiler) {
                    // Convert `number` to `function(): number`
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    overridden_type =
                        Some(reg.create_function_type(ast, overridden_prop_type.unwrap(), &[]));
                } else if lvalue.is_setter_def(compiler) {
                    // Convert `number` to `function(number): undefined`
                    let void_type = self
                        .creator
                        .get_native_type(compiler, JSTypeNative::VOID_TYPE);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    overridden_type = Some(reg.create_function_type(
                        ast,
                        void_type,
                        &[overridden_prop_type.unwrap()],
                    ));
                } else if overridden_prop_type_is_function_type {
                    // for cases where we override a non-method (e.g. a number) with a method,
                    // don't put the non-method type (e.g. number) on the function.
                    // Instead do some basic inference to create a function type.
                    // we will warn during typechecking for an invalid override, but we don't want
                    // to put a non-function type on this function because that will interfere
                    // with type inference inside the function.
                    let (reg, _) = compiler.get_type_registry_and_ast();
                    overridden_type = overridden_prop_type.unwrap().to_maybe_function_type(reg);
                }
            }
        }

        let mut contents = fn_root.map(AstFunctionContents::new);
        if fn_root.is_some_and(|fn_root| {
            self.creator
                .functions_with_non_empty_returns
                .contains(&fn_root)
        }) {
            contents.as_mut().unwrap().record_non_empty_return();
        }

        let best_type_name = self.get_best_type_name(compiler, lvalue_node, name.clone());
        let mut builder = FunctionTypeBuilder::new(
            best_type_name,
            compiler,
            error_root.expect("NullPointerException"),
            self.current_scope,
        );
        builder
            .set_syntactic_function_name(name)
            .set_contents(contents.map(|contents| Arc::new(contents) as Arc<dyn FunctionContents>));
        let declaration_scope = if lvalue_node.is_some() {
            self.get_l_value_root_scope(compiler, lvalue_node)
        } else {
            self.current_scope
        };
        builder
            .set_declaration_scope(declaration_scope)
            .infer_from_overridden_function(compiler, overridden_type, parameters_node)
            .infer_kind(compiler, info.as_deref())
            .infer_closure_primitive(info.as_deref())
            .infer_template_type_name(compiler, info.as_deref(), prototype_owner)
            .infer_inheritance(compiler, info.as_deref(), None);

        if info.is_none() || !info.as_ref().unwrap().has_return_type() {
            // when there is no @return annotation, look for inline return type declaration
            if r_value.is_some()
                && r_value.unwrap().is_function(compiler)
                && r_value.unwrap().has_children(compiler)
            {
                let name_doc_info = r_value
                    .unwrap()
                    .get_first_child(compiler)
                    .unwrap()
                    .get_jsdoc_info(compiler);
                builder.infer_return_type(compiler, name_doc_info.as_deref(), true);
            }
        } else {
            builder.infer_return_type(compiler, info.as_deref(), false);
        }

        // Infer the context type.
        let mut fallback_receiver_type: Option<TypeId> = None;
        let (reg, _) = compiler.get_type_registry_and_ast();
        if owner_type.is_some()
            && owner_type.unwrap().is_function_prototype_type(reg)
            && owner_type
                .unwrap()
                .get_owner_function(reg)
                .expect("NullPointerException")
                .has_instance_type(reg)
        {
            fallback_receiver_type = owner_type
                .unwrap()
                .get_owner_function(reg)
                .unwrap()
                .get_instance_type(reg);
        } else if owner_type.is_some()
            && owner_type.unwrap().is_function_type(reg)
            && owner_type
                .unwrap()
                .to_maybe_function_type(reg)
                .unwrap()
                .has_instance_type(reg)
            && lvalue_node.is_some()
            && lvalue_node.unwrap().is_static_member(compiler)
        {
            // Limit this case to members of ctors and interfaces decalared using `static`. Most
            // namespaces, like object literals, are assumed to declare free functions, so we
            // exclude them. Additionally, methods *assigned* to a ctor, especially an ES5 ctor,
            // were never designed with static polymorphism in mind, so excluding them preserves
            // their assumptions.
            fallback_receiver_type = owner_type;
        } else if owner_node.is_some() && owner_node.unwrap().is_this(compiler) {
            fallback_receiver_type = self.current_scope.get_type_of_this(compiler);
        }

        let fn_type = builder
            .infer_this_type_with_type(compiler, info.as_deref(), fallback_receiver_type)
            .infer_parameter_types(compiler, parameters_node, info.as_deref())
            .build_and_register(compiler);

        // Do some additional validation for constructors and interfaces.
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if fn_type.has_instance_type(reg) && lvalue_node.is_some() {
            let prototype_slot = fn_type
                .get_slot(reg, ast, "prototype")
                .expect("NullPointerException");

            // We want to make sure that the function and its prototype are declared at the same
            // node. This consistency is helpful to users of SymbolTable, because everything gets
            // declared at the same place.
            prototype_slot.set_node(reg, lvalue_node);
        }
        fn_type
    }

    /// Checks that the annotations in {@code info} are compatible with the aliased {@code type}.
    /// Any errors will be reported at {@code n}, which should be the qualified name node.
    // port: TypedScopeCreator.AbstractScopeBuilder#checkFunctionAliasAnnotations
    fn check_function_alias_annotations(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        type_: TypeId,
        info: Option<&JSDocInfo>,
    ) {
        let Some(info) = info else {
            return;
        };
        let mut annotation: Option<&str> = None;
        let (reg, _) = compiler.get_type_registry_and_ast();
        if info.uses_implicit_match() {
            if !type_.is_structural_interface(reg) {
                annotation = Some("@record");
            }
        } else if info.is_interface() {
            if !type_.is_interface(reg) {
                annotation = Some("@interface");
            }
        } else if info.is_constructor() && !type_.is_constructor(reg) {
            annotation = Some("@constructor");
        }
        // TODO(sdh): consider checking @template, @param, @return, and/or @this.
        if annotation.is_some()
            // TODO(sdh): Remove this extra check once TypeScript stops passing us duplicate
            // conflicting externs.  In particular, TS considers everything an interface, but
            // Closure externs mark most things as @constructor.  The load order is not always the
            // same, so the error can show up in either the generated TS externs file or in our own
            // extern.
            && (!n.is_from_externs(compiler) || annotation.unwrap() == "@record")
        {
            let qualified_name = n
                .get_qualified_name(compiler)
                .map(|name| name.to_string())
                .unwrap_or_else(|| "null".to_string());
            let error = JSError::make(
                compiler,
                n,
                &INCOMPATIBLE_ALIAS_ANNOTATION,
                &[annotation.unwrap(), &qualified_name],
            );
            self.creator.report(compiler, error);
        }
    }

    /// Find the property that's being overridden on this type, if any.
    ///
    /// Said property could be a method, field, getter, or setter. We don't distinguish between
    /// these when looking up a property type.
    // port: TypedScopeCreator.AbstractScopeBuilder#findOverriddenProperty
    fn find_overridden_property(
        &mut self,
        compiler: &mut AbstractCompiler,
        owner_type: TypeId,
        prop_name: JsString,
        type_map: Option<Arc<TemplateTypeMap>>,
    ) -> Option<TypeId> {
        let mut result: Option<TypeId> = None;
        let (reg, ast) = compiler.get_type_registry_and_ast();

        // First, check to see if the property is implemented
        // on a superclass.
        let mut prop_type = Some(owner_type.get_property_type(reg, ast, prop_name.clone()));
        if prop_type.is_some() && !prop_type.unwrap().is_unknown_type(reg, ast) {
            result = prop_type;
        } else {
            // If it's not, then check to see if it's implemented
            // on an implemented interface.
            for iface in owner_type.get_ctor_implemented_interfaces(reg, ast) {
                prop_type = Some(iface.get_property_type(reg, ast, prop_name.clone()));
                if prop_type.is_some() && !prop_type.unwrap().is_unknown_type(reg, ast) {
                    result = prop_type;
                    break;
                }
            }
        }

        if result.is_some() && type_map.is_some() && !type_map.as_ref().unwrap().is_empty() {
            let mut replacer = TemplateTypeReplacer::for_partial_replacement(type_map.unwrap());
            result = Some(result.unwrap().visit(reg, ast, &mut replacer));
        }

        result
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#getPrototypeOwnerType
    fn get_prototype_owner_type(
        &self,
        reg: &JSTypeRegistry,
        owner_type: Option<TypeId>,
    ) -> Option<TypeId> {
        if owner_type.is_some() && owner_type.unwrap().is_function_prototype_type(reg) {
            return owner_type.unwrap().get_owner_function(reg);
        }
        None
    }
}

impl Callback for AbstractScopeBuilder<'_> {
    // port: TypedScopeCreator.AbstractScopeBuilder#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        self.input_id = t.get_input_id();

        let compiler = t.get_compiler();
        if n.is_function(compiler)
            || n.is_script(compiler)
            || (parent.is_none() && self.input_id.is_some())
        {
            check_not_null!(self.input_id.as_ref());
            self.source_name = NodeUtil::get_source_name(compiler, n);
        }
        if parent.is_none() || self.in_current_scope(t) {
            self.visit_preorder(t, n, parent);
            return true;
        } else if n.is_module_body(t.get_compiler()) {
            // Visit modules pre-order. While this doesn't exactly match execution semantics, it
            // does match how the compiler rewrites modules into the global scope.
            let current_scope = self.current_scope;
            self.creator
                .create_scope(t.get_compiler(), n, Some(current_scope));
        } else if NodeUtil::is_bundled_goog_module_scope_root(t.get_compiler(), n) {
            let current_scope = self.current_scope;
            let function_scope =
                self.creator
                    .create_scope(t.get_compiler(), parent.unwrap(), Some(current_scope));
            self.creator
                .create_scope(t.get_compiler(), n, Some(function_scope));
        }
        false
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let is_scope_root = parent.is_none();
        self.input_id = t.get_input_id();
        if !is_scope_root || !n.is_class(t.get_compiler()) {
            // Do not type literals or visit class scope roots
            self.attach_literal_types(t.get_compiler(), n);
            self.visit_postorder(t, n, parent);
        }
        if !is_scope_root && self.deferred_actions_contains_key(n) {
            // streams are expensive, only make if needed
            for action in self.remove_all_deferred_actions(n) {
                self.run_deferred_action(t.get_compiler(), action);
            }
        } else if is_scope_root && !self.deferred_actions.is_empty() {
            // Run *all* remaining deferred actions, in case any were missed.
            let actions: Vec<DeferredAction> = self
                .deferred_actions
                .iter()
                .map(|(_, action)| action.clone())
                .collect();
            for action in actions {
                self.run_deferred_action(t.get_compiler(), action);
            }
        }
    }
}

impl AbstractScopeBuilder<'_> {
    // Rust-only: `deferredActions.put(key, runnable)` (LinkedHashMultimap: entries keep their
    // global insertion order).
    fn put_deferred_action(&mut self, key: NodeId, action: DeferredAction) {
        self.deferred_actions.push((key, action));
    }

    // Rust-only: `deferredActions.containsKey(key)`.
    fn deferred_actions_contains_key(&self, key: NodeId) -> bool {
        self.deferred_actions.iter().any(|(k, _)| *k == key)
    }

    // Rust-only: `deferredActions.removeAll(key)`: the key's values in insertion order.
    fn remove_all_deferred_actions(&mut self, key: NodeId) -> Vec<DeferredAction> {
        let mut removed = Vec::new();
        let mut kept = Vec::new();
        for (k, action) in std::mem::take(&mut self.deferred_actions) {
            if k == key {
                removed.push(action);
            } else {
                kept.push((k, action));
            }
        }
        self.deferred_actions = kept;
        removed
    }

    // Rust-only: `Runnable#run` of a deferred lambda.
    fn run_deferred_action(&mut self, compiler: &mut AbstractCompiler, action: DeferredAction) {
        match action {
            DeferredAction::DefineObjectLiteral(n) => self.define_object_literal(compiler, n),
            DeferredAction::ResolveStubDeclaration {
                n,
                is_extern,
                owner_name,
            } => self.resolve_stub_declaration(compiler, n, is_extern, owner_name),
        }
    }
}

impl AbstractScopeBuilder<'_> {
    /// Creates a new enum type, based on the given nodes.
    ///
    /// This handles two cases that are semantically very different, but are not mutually
    /// exclusive: - An object literal that needs an enum type attached to it. - An assignment
    /// expression with an enum tag in the JsDoc.
    ///
    /// This function will always create an enum type, so only call it if you're sure that's what
    /// you want.
    ///
    /// @param rValue The right-hand side of the enum, or null if none.
    /// @param lValue The left-hand side of the enum.
    /// @param name The qualified name of the enum
    /// @param info The {@link JSDocInfo} attached to the enum definition.
    // port: TypedScopeCreator.AbstractScopeBuilder#createEnumTypeFromNodes
    fn create_enum_type_from_nodes(
        &mut self,
        compiler: &mut AbstractCompiler,
        r_value: Option<NodeId>,
        name: Option<JsString>,
        l_value: Option<NodeId>,
        info: Arc<JSDocInfo>,
    ) -> TypeId {
        // checkNotNull(info): `info` is not nullable here.
        check_state!(info.has_enum_parameter_type());
        check_state!(
            l_value.is_some() || r_value.is_some(),
            "An enum initializer should come from either an lvalue or rvalue"
        );

        let mut enum_type: Option<TypeId> = None;
        if r_value.is_some() && r_value.unwrap().is_qualified_name(compiler) {
            // Handle an aliased enum. Note that putting @enum on an enum alias is optional. If the
            // rValue is not an enum, then this assignment errors during TypeCheck.
            let qualified_name = r_value
                .unwrap()
                .get_qualified_name(compiler)
                .expect("NullPointerException");
            let var = self.current_scope.get_var(compiler, qualified_name);
            let var_type = var.and_then(|var| var.get_type(compiler));
            let (reg, _) = compiler.get_type_registry_and_ast();
            if var.is_some() && var_type.is_some() && var_type.unwrap().is_enum_type(reg) {
                enum_type = var_type.unwrap().to_maybe_enum_type(reg);
            }
        }

        if enum_type.is_none() {
            let best_name = self.get_best_type_name(compiler, l_value, name.clone());
            let goog_module_id =
                TypedScopeCreator::containing_goog_module_id_of(compiler, self.current_scope);
            let current_scope_view = self.current_scope.as_static_typed_scope_arc(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let element_type = info
                .get_enum_parameter_type()
                .expect("NullPointerException")
                .evaluate(reg, ast, Some(current_scope_view));
            let built = closure_jstype::enum_type::builder()
                .set_name_nullable(best_name)
                .set_goog_module_id_nullable(goog_module_id)
                .set_source(r_value)
                .set_element_type(element_type)
                .build(reg, ast);
            enum_type = Some(built);

            if r_value.is_some() && r_value.unwrap().is_object_lit(compiler) {
                // collect enum elements
                let mut key = r_value.unwrap().get_first_child(compiler);
                while key.is_some() {
                    if key.unwrap().is_computed_prop(compiler) {
                        let error = JSError::make(compiler, key.unwrap(), &INVALID_ENUM_KEY, &[]);
                        self.creator.report(compiler, error);
                        key = key.unwrap().get_next(compiler);
                        continue;
                    }
                    let key_name = key.unwrap().get_string(compiler);
                    // Preconditions.checkNotNull(keyName, "Invalid enum key: %s", key): a key's
                    // string is never null here.
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    built.define_element(reg, ast, key_name, key);
                    key = key.unwrap().get_next(compiler);
                }
            }
        }
        let enum_type = enum_type.unwrap();

        if name.is_some() {
            let current_scope_view = self.current_scope.as_static_typed_scope(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let elements_type = enum_type.get_elements_type(reg);
            reg.declare_type(ast, Some(current_scope_view), name.unwrap(), elements_type);
        }

        if r_value.is_none()
            || !(r_value.unwrap().is_object_lit(compiler)
                || r_value.unwrap().is_qualified_name(compiler))
        {
            let error = JSError::make(
                compiler,
                if l_value.is_some() {
                    l_value.unwrap()
                } else {
                    r_value.unwrap()
                },
                &ENUM_INITIALIZER,
                &[],
            );
            self.creator.report(compiler, error);
        }
        enum_type
    }
}

impl AbstractScopeBuilder<'_> {
    /// Check if the given node is a property of a name in the global scope.
    // port: TypedScopeCreator.AbstractScopeBuilder#isLValueRootedInGlobalScope
    fn is_l_value_rooted_in_global_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: Option<NodeId>,
    ) -> bool {
        self.get_l_value_root_scope(compiler, n).is_global(compiler)
    }

    /// Return the scope for the name of the given node.
    // port: TypedScopeCreator.AbstractScopeBuilder#getLValueRootScope
    fn get_l_value_root_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: Option<NodeId>,
    ) -> TypedScope {
        let root = NodeUtil::get_best_l_value_root(compiler, n);
        if root.is_some() {
            let root = root.unwrap();
            if root.is_name(compiler) {
                let name_parent = root.get_parent(compiler).expect("NullPointerException");
                match name_parent.get_token(compiler) {
                    Token::VAR => {
                        return self.current_hoist_scope;
                    }
                    Token::LET
                    | Token::CONST
                    | Token::CLASS
                    | Token::FUNCTION
                    | Token::PARAM_LIST
                    | Token::CATCH => {
                        return self.current_scope;
                    }
                    Token::ITER_REST | Token::OBJECT_REST => {
                        // TODO(bradfordcsmith): Handle array destructuring REST
                        check_state!(
                            name_parent
                                .get_parent(compiler)
                                .expect("NullPointerException")
                                .is_param_list(compiler),
                            "%s",
                            name_parent.to_string(compiler)
                        );
                        return self.current_scope;
                    }
                    _ => {
                        if self.is_goog_module_exports(compiler, Some(root)) {
                            // Ensure that 'exports = class {}' in a goog.module returns the module scope.
                            return self.current_scope;
                        }
                        let root_name = root.get_string(compiler);
                        let var = self.current_scope.get_var(compiler, root_name);
                        if var.is_some() {
                            return var.unwrap().get_scope(compiler);
                        }
                    }
                }
            } else if root.is_this(compiler) || root.is_super(compiler) {
                // We want the enclosing function scope, or the global scope if not in a function.
                return self.current_hoist_scope.get_scope_of_this(compiler);
            }
        }
        self.current_hoist_scope.get_global_scope(compiler)
    }

    /// Look for a type declaration on a property assignment (in an ASSIGN or an object literal
    /// key).
    ///
    /// @param info The doc info for this property.
    /// @param lValue The l-value node.
    /// @param rValue The node that {@code n} is being initialized to, or {@code null} if this is a
    ///     stub declaration.
    /// @param declaredRValueTypeSupplier A supplier for the declared type of the rvalue, used for
    ///     destructuring declarations where we have to do additional work on the rvalue.
    // port: TypedScopeCreator.AbstractScopeBuilder#getDeclaredType
    fn get_declared_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<Arc<JSDocInfo>>,
        l_value: NodeId,
        r_value: Option<NodeId>,
        declared_r_value_type_supplier: Option<&RValueInfoSupplier>,
    ) -> Option<TypeId> {
        if info.is_some() && info.as_ref().unwrap().has_type() {
            let type_ =
                self.get_declared_type_in_annotation(compiler, l_value, info.as_ref().unwrap());

            let is_symbol = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                type_.is_symbol(reg, ast)
            };
            if is_symbol
                && l_value.is_from_externs(compiler)
                && l_value.is_get_prop(compiler)
                && l_value
                    .get_first_child(compiler)
                    .expect("NullPointerException")
                    .matches_name(compiler, "Symbol")
                && self.current_scope.is_global(compiler)
            {
                // Create a unique subtype type for this name.
                // Currently we only look specifically for extern properties on the global Symbol object
                // itself - so
                //   /** @const {symbol} */
                //   Symbol.iterator;
                // but not:
                //   /** @const {symbol} */
                //   const x = Symbol();
                // We could consider relaxing that in the future & also defining known symbols from
                // source code.
                let qualified_name = l_value
                    .get_qualified_name(compiler)
                    .expect("NullPointerException");
                let (reg, ast) = compiler.get_type_registry_and_ast();
                return Some(KnownSymbolType::new(reg, ast, qualified_name));
            }
            return Some(type_);
        } else if r_value.is_some() && r_value.unwrap().is_function(compiler) && {
            let fn_type = {
                let js_type = r_value.unwrap().get_jstype(compiler);
                let (reg, _) = compiler.get_type_registry_and_ast();
                js_type.and_then(|t| t.to_maybe_function_type(reg))
            };
            self.should_use_function_literal_type(compiler, fn_type, info.as_deref(), Some(l_value))
        } {
            return r_value.unwrap().get_jstype(compiler);
        } else if r_value.is_some() && r_value.unwrap().is_class(compiler) {
            return r_value.unwrap().get_jstype(compiler);
        } else if info.is_some() {
            if info.as_ref().unwrap().has_enum_parameter_type() {
                if r_value.is_some() && r_value.unwrap().is_object_lit(compiler) {
                    return r_value.unwrap().get_jstype(compiler);
                } else {
                    let qualified_name = l_value.get_qualified_name(compiler);
                    return Some(self.create_enum_type_from_nodes(
                        compiler,
                        r_value,
                        qualified_name,
                        Some(l_value),
                        info.clone().unwrap(),
                    ));
                }
            } else if info.as_ref().unwrap().is_constructor_or_interface() {
                let qualified_name = l_value.get_qualified_name(compiler);
                let fn_type = self.create_function_type_from_nodes(
                    compiler,
                    r_value,
                    qualified_name,
                    info.clone(),
                    Some(l_value),
                );
                if r_value.is_none() && !l_value.is_from_externs(compiler) {
                    let is_constructor = {
                        let (reg, _) = compiler.get_type_registry_and_ast();
                        fn_type.is_constructor(reg)
                    };
                    let qualified_name = l_value
                        .get_qualified_name(compiler)
                        .map(|name| name.to_string_lossy())
                        .unwrap_or_else(|| "null".to_string());
                    let error = JSError::make(
                        compiler,
                        l_value,
                        if is_constructor {
                            &CTOR_INITIALIZER
                        } else {
                            &IFACE_INITIALIZER
                        },
                        &[&qualified_name],
                    );
                    self.creator.report(compiler, error);
                }
                return Some(fn_type);
            }
        }

        // Check if this is constant and if it has a known type.
        if (l_value.is_export(compiler) && l_value.get_boolean_prop(compiler, Prop::EXPORT_DEFAULT))
            || NodeUtil::is_constant_declaration(compiler, info.as_deref(), l_value)
            || self.is_goog_module_exports(compiler, Some(l_value))
        {
            if r_value.is_some() {
                let r_value_type =
                    self.get_declared_r_value_type(compiler, Some(l_value), r_value.unwrap());
                let r_value_qualified_name = r_value.unwrap().get_qualified_name_object(compiler);
                let current_scope = self.current_scope;
                self.declare_alias_type_if_rvalue_is_aliasable(
                    compiler,
                    l_value,
                    r_value_qualified_name,
                    r_value_type,
                    current_scope,
                );
                if r_value_type.is_some() {
                    return r_value_type;
                }
            } else if declared_r_value_type_supplier.is_some() {
                // RValueInfo is never null here (the suppliers never return null).
                let rvalue_info =
                    self.get_r_value_info(compiler, declared_r_value_type_supplier.unwrap());
                let current_scope = self.current_scope;
                self.declare_alias_type_if_rvalue_is_aliasable(
                    compiler,
                    l_value,
                    rvalue_info.qualified_name.clone(),
                    rvalue_info.type_,
                    current_scope,
                );
                if rvalue_info.type_.is_some() {
                    return rvalue_info.type_;
                }
            }
        }

        if r_value.is_some() && r_value.unwrap().is_assign(compiler) {
            // Handle nested assignments. For example, TypeScript generates code like this:
            //   var Foo_1;
            //   let Foo = Foo_1 = class Foo {}
            //   Foo = Foo_1 = tslib_1.decorate(..., Foo);
            let second = r_value.unwrap().get_second_child(compiler);
            return self.get_declared_type(compiler, info, l_value, second, None);
        }

        if info.is_some()
            && FunctionTypeBuilder::is_function_type_declaration(info.as_ref().unwrap())
        {
            let fn_name = l_value.get_qualified_name(compiler);
            return Some(self.create_function_type_from_nodes(
                compiler,
                None,
                fn_name,
                info,
                Some(l_value),
            ));
        }

        if self.is_valid_typedef_declaration(compiler, l_value, info) {
            return Some(
                self.creator
                    .get_native_type(compiler, JSTypeNative::NO_TYPE),
            );
        }

        None
    }
}

impl AbstractScopeBuilder<'_> {
    /// For a const alias, like `const alias = other.name`, this may declare `alias` as a type name,
    /// depending on what other.name is defined to be.
    ///
    /// This method recognizes three kinds of type aliases: @typedefs, @constructor/@interface
    /// types, and @enums.
    ///
    /// Given any of those three types, this method redeclares the aliasing name in the
    /// typeRegistry. For @typedefs and global @enums, this method also marks the qualified name
    /// referring to the type as non-nullable by default.
    // port: TypedScopeCreator.AbstractScopeBuilder#declareAliasTypeIfRvalueIsAliasable
    fn declare_alias_type_if_rvalue_is_aliasable(
        &mut self,
        compiler: &mut AbstractCompiler,
        l_value: NodeId,
        r_value: Option<QualifiedName>,
        r_value_type: Option<TypeId>,
        r_value_lookup_scope: TypedScope,
    ) {
        let l_value_name = l_value.get_qualified_name(compiler);
        let current_scope = self.current_scope;
        self.declare_alias_type_if_rvalue_is_aliasable_with_name(
            compiler,
            l_value_name,
            Some(l_value),
            r_value,
            r_value_type,
            r_value_lookup_scope,
            current_scope,
        );
    }

    /// For a const alias, like `const alias = other.name`, this may declare `alias` as a type name,
    /// depending on what other.name is defined to be.
    ///
    /// NOTE: in most cases, call the version with fewer arguments. This version only exists to
    /// handle goog.declareLegacyNamespace, which is strange compared to normal type aliasing
    /// because (1) there's no GETPROP node representing the lvalue and (2) the type is declared in
    /// the global scope, not the current module-local scope.
    ///
    /// @param lValueName the fully qualified lValue name, if any. If null, all this method will do
    ///     is propagate the @typedef Node annotation to actualLvalueNode.
    /// @param aliasDeclarationScope The scope in which to declare the alias name. In most cases,
    ///     this should just be the {@link #currentScope}.
    // port: TypedScopeCreator.AbstractScopeBuilder#declareAliasTypeIfRvalueIsAliasable
    #[allow(clippy::too_many_arguments)]
    fn declare_alias_type_if_rvalue_is_aliasable_with_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        l_value_name: Option<JsString>,
        actual_lvalue_node: Option<NodeId>,
        r_value: Option<QualifiedName>,
        r_value_type: Option<TypeId>,
        r_value_lookup_scope: TypedScope,
        alias_declaration_scope: TypedScope,
    ) {
        // NOTE: this allows some strange patterns such allowing instance properties
        // to be aliases of constructors, and then creating a local alias of that to be
        // used as a type name.  Consider restricting this.

        let Some(r_value) = r_value else {
            return;
        };

        // Look for a @typedef annotation on the definition node
        let definition_node = self.get_definition_node(compiler, &r_value, r_value_lookup_scope);
        if definition_node.is_some() {
            let typedef_type = definition_node.unwrap().get_typedef_type_prop(compiler);
            if typedef_type.is_some() {
                // Propagate typedef type to typedef aliases.
                actual_lvalue_node
                    .expect("NullPointerException")
                    .set_typedef_type_prop(compiler, typedef_type);
                if l_value_name.is_some() {
                    let alias_declaration_scope_view =
                        alias_declaration_scope.as_static_typed_scope(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    reg.identify_non_nullable_name(
                        ast,
                        Some(alias_declaration_scope_view),
                        l_value_name.clone().unwrap(),
                    );
                    reg.declare_type(
                        ast,
                        Some(alias_declaration_scope_view),
                        l_value_name.unwrap(),
                        typedef_type.unwrap(),
                    );
                }
                return;
            }
        }

        let Some(l_value_name) = l_value_name else {
            return;
        };

        let alias_declaration_scope_view = alias_declaration_scope.as_static_typed_scope(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        // Check if the provided rValueType indicates that we should declare this type
        // Note that we only look for enums and constructors/interfaces here: this step cannot work
        // for @typedefs. The 'type' of the TypedVar representing a @typedef'd name is the None type,
        // not the @typedef'd type.
        if r_value_type.is_some()
            && r_value_type.unwrap().is_function_type(reg)
            && r_value_type
                .unwrap()
                .to_maybe_function_type(reg)
                .unwrap()
                .has_instance_type(reg)
        {
            // Look for @constructor/@interface by checking if the RHS has an instance type
            let function_type = r_value_type.unwrap().to_maybe_function_type(reg).unwrap();
            let instance_type = function_type
                .get_instance_type(reg)
                .expect("NullPointerException");
            reg.declare_type(
                ast,
                Some(alias_declaration_scope_view),
                l_value_name,
                instance_type,
            );
            return;
        }

        if r_value_type.is_some() && r_value_type.unwrap().is_enum_type(reg) {
            // Look for cases where the rValue is an Enum namespace
            let elements_type = r_value_type
                .unwrap()
                .to_maybe_enum_type(reg)
                .unwrap()
                .get_elements_type(reg);
            reg.declare_type(
                ast,
                Some(alias_declaration_scope_view),
                l_value_name.clone(),
                elements_type,
            );
            reg.identify_non_nullable_name(ast, Some(alias_declaration_scope_view), l_value_name);
        }
    }

    /// Whether this lvalue is either `exports`, `exports.x`, or a string key in `exports = {x}`.
    // port: TypedScopeCreator.AbstractScopeBuilder#isGoogModuleExports
    fn is_goog_module_exports(
        &mut self,
        compiler: &mut AbstractCompiler,
        l_value: Option<NodeId>,
    ) -> bool {
        if self.get_module(compiler).is_none() || l_value.is_none() {
            return false;
        }
        let l_value = l_value.unwrap();
        if self.creator.undeclared_names_for_closure.contains(&l_value) {
            // includes "exports" and "exports.x"
            return true;
        }
        // ASSIGN
        //   NAME "exports"
        //   OBJECT_LIT
        //     STRING_KEY "x"
        //     [value]
        l_value.is_string_key(compiler)
            && l_value
                .get_parent(compiler)
                .expect("NullPointerException")
                .is_object_lit(compiler)
            && l_value
                .get_grandparent(compiler)
                .expect("NullPointerException")
                .is_assign(compiler)
            && l_value
                .get_parent(compiler)
                .unwrap()
                .get_previous(compiler)
                .expect("NullPointerException")
                .matches_name(compiler, "exports")
            && self.creator.undeclared_names_for_closure.contains(
                &l_value
                    .get_parent(compiler)
                    .unwrap()
                    .get_previous(compiler)
                    .unwrap(),
            )
    }

    /// Returns the AST node associated with the definition, if any.
    // port: TypedScopeCreator.AbstractScopeBuilder#getDefinitionNode
    fn get_definition_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        qname: &QualifiedName,
        scope: TypedScope,
    ) -> Option<NodeId> {
        if qname.is_simple(compiler) {
            let component = qname.get_component(compiler);
            let var = scope.get_var(compiler, component);
            return if var.is_some() {
                var.unwrap().get_name_node(compiler)
            } else {
                None
            };
        }
        let owner = qname.get_owner(compiler).expect("NullPointerException");
        let owner_type = lookup_qualified_name(compiler, scope, &owner);
        let component = qname.get_component(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let parent = object_type::cast(reg, owner_type);
        if parent.is_some() {
            parent.unwrap().get_property_def_site(reg, ast, component)
        } else {
            None
        }
    }

    /// Check for common idioms of a typed R-value assigned to a const L-value.
    ///
    /// Normally, we would only want this sort of propagation to happen under type inference. But
    /// we want a declared const to be nameable in a type annotation, so we need to figure out the
    /// type before we try to resolve the annotation.
    ///
    /// @param lValue is the lvalue node if this is a simple assignment, null for destructuring
    // port: TypedScopeCreator.AbstractScopeBuilder#getDeclaredRValueType
    fn get_declared_r_value_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        l_value: Option<NodeId>,
        r_value: NodeId,
    ) -> Option<TypeId> {
        // If rValue has a type-cast, we use the type in the type-cast.
        let r_value_info = r_value.get_jsdoc_info(compiler);
        if r_value.is_cast(compiler)
            && r_value_info.is_some()
            && r_value_info.as_ref().unwrap().has_type()
        {
            let current_scope_view = self.current_scope.as_static_typed_scope_arc(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            return Some(r_value_info.unwrap().get_type().unwrap().evaluate(
                reg,
                ast,
                Some(current_scope_view),
            ));
        }

        // Check if the type has already been computed during scope-creation.
        // This is mostly useful for literals like BOOLEAN, NUMBER, STRING, and
        // OBJECT_LITERAL
        let mut type_ = r_value.get_jstype(compiler);
        if type_.is_some() && {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            !type_.unwrap().is_unknown_type(reg, ast)
        } {
            return type_;
        }

        // If rValue is a name, try looking it up in the current scope.
        if r_value.is_qualified_name(compiler) {
            let qname = r_value
                .get_qualified_name_object(compiler)
                .expect("NullPointerException");
            return lookup_qualified_name(compiler, self.current_scope, &qname);
        }

        // Check for simple invariant operations, such as "!x" or "+x" or "''+x"
        if NodeUtil::is_boolean_result(compiler, r_value) {
            return Some(
                self.creator
                    .get_native_type(compiler, JSTypeNative::BOOLEAN_TYPE),
            );
        }

        if NodeUtil::is_numeric_result(compiler, r_value) {
            return Some(
                self.creator
                    .get_native_type(compiler, JSTypeNative::NUMBER_TYPE),
            );
        }

        if NodeUtil::is_big_int_result(compiler, r_value) {
            return Some(
                self.creator
                    .get_native_type(compiler, JSTypeNative::BIGINT_TYPE),
            );
        }

        if NodeUtil::is_string_result(compiler, r_value) {
            return Some(
                self.creator
                    .get_native_type(compiler, JSTypeNative::STRING_TYPE),
            );
        }

        if r_value.is_new(compiler)
            && r_value
                .get_first_child(compiler)
                .expect("NullPointerException")
                .is_qualified_name(compiler)
        {
            let qname = r_value
                .get_first_child(compiler)
                .unwrap()
                .get_qualified_name_object(compiler)
                .expect("NullPointerException");
            let target_type = lookup_qualified_name(compiler, self.current_scope, &qname);
            if target_type.is_some() {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let fn_type = target_type
                    .unwrap()
                    .restrict_by_not_null_or_undefined(reg, ast)
                    .to_maybe_function_type(reg);
                if fn_type.is_some() && fn_type.unwrap().has_instance_type(reg) {
                    return fn_type.unwrap().get_instance_type(reg);
                }
            }
        }

        if r_value.is_call(compiler)
            && r_value
                .get_first_child(compiler)
                .expect("NullPointerException")
                .matches_name(compiler, "Symbol")
        {
            // Type calls like `Symbol('foo')`
            let symbol_var = self.current_scope.get_var(compiler, "Symbol");
            if symbol_var.is_some() && symbol_var.unwrap().get_scope(compiler).is_global(compiler) {
                let (reg, _) = compiler.get_type_registry_and_ast();
                return Some(reg.get_native_type(JSTypeNative::SYMBOL_TYPE));
            }
        }

        // Check for a very specific JS idiom:
        // var x = x || TYPE;
        // This is used by Closure's base namespace for esoteric
        // reasons, so we only really care about that case.
        if r_value.is_or(compiler) {
            let first_clause = r_value
                .get_first_child(compiler)
                .expect("NullPointerException");
            let second_clause = first_clause
                .get_next(compiler)
                .expect("NullPointerException");
            let names_match = first_clause.is_name(compiler)
                && l_value.is_some()
                && l_value.unwrap().is_name(compiler)
                && first_clause.get_string(compiler) == l_value.unwrap().get_string(compiler);
            if names_match {
                type_ = second_clause.get_jstype(compiler);
                if type_.is_some() && {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    !type_.unwrap().is_unknown_type(reg, ast)
                } {
                    return type_;
                }
            }
        }

        None
    }
}

impl AbstractScopeBuilder<'_> {
    /// Look for class-defining calls. Because JS has no 'native' syntax for defining classes, this
    /// is often very coding-convention dependent and business-logic heavy.
    // port: TypedScopeCreator.AbstractScopeBuilder#checkForClassDefiningCalls
    fn check_for_class_defining_calls(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let coding_convention = Arc::clone(&self.creator.coding_convention);
        let relationship = coding_convention.get_classes_defined_by_call(compiler, n);
        if relationship.is_some() {
            let relationship = relationship.unwrap();
            let superclass_type = lookup_qualified_name(
                compiler,
                self.current_scope,
                &QualifiedName::of(relationship.superclass_name.clone()),
            );
            let super_class = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                TypeValidator::get_instance_of_ctor(reg, ast, superclass_type)
            };
            let subclass_type = lookup_qualified_name(
                compiler,
                self.current_scope,
                &QualifiedName::of(relationship.subclass_name.clone()),
            );
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let sub_class = TypeValidator::get_instance_of_ctor(reg, ast, subclass_type);
            if super_class.is_some() && sub_class.is_some() {
                // superCtor and subCtor might be structural constructors
                // (like {function(new:Object)}) so we need to resolve them back
                // to the original ctor objects.
                let super_ctor = super_class.unwrap().get_constructor(reg);
                let sub_ctor = sub_class.unwrap().get_constructor(reg);
                if super_ctor.is_some() && sub_ctor.is_some() {
                    let parent = NominalTypeBuilder::new(
                        reg,
                        ast,
                        super_ctor.unwrap(),
                        super_class.unwrap(),
                    );
                    let child =
                        NominalTypeBuilder::new(reg, ast, sub_ctor.unwrap(), sub_class.unwrap());
                    coding_convention.apply_subclass_relationship(
                        ast,
                        reg,
                        &parent,
                        &child,
                        relationship.r#type,
                    );
                }
            }
        }

        let singleton_getter_class_name =
            coding_convention.get_singleton_getter_class_name(compiler, n);
        if singleton_getter_class_name.is_some() {
            let current_scope_view = self.current_scope.as_static_typed_scope(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let found = reg.get_type(
                ast,
                Some(current_scope_view),
                singleton_getter_class_name.unwrap(),
            );
            let object_type = object_type::cast(reg, found);
            if object_type.is_some() {
                let function_type = object_type.unwrap().get_constructor(reg);

                if function_type.is_some() {
                    let getter_type = reg.create_function_type(ast, object_type.unwrap(), &[]);
                    let class_type = NominalTypeBuilder::new(
                        reg,
                        ast,
                        function_type.unwrap(),
                        object_type.unwrap(),
                    );
                    coding_convention.apply_singleton_getter(ast, reg, &class_type, getter_type);
                }
            }
        }

        let object_literal_cast = coding_convention.get_object_literal_cast(compiler, n);
        if object_literal_cast.is_some() {
            let object_literal_cast = object_literal_cast.unwrap();
            if object_literal_cast.diagnostic_type.is_none() {
                let current_scope_view = self.current_scope.as_static_typed_scope(compiler);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                // typeRegistry.getType(scope, null) would throw in Java's map lookup; the type
                // name is set whenever there is no diagnostic.
                let found = reg.get_type(
                    ast,
                    Some(current_scope_view),
                    object_literal_cast
                        .type_name
                        .clone()
                        .expect("NullPointerException"),
                );
                let type_ = object_type::cast(reg, found);
                if type_.is_some() && type_.unwrap().get_constructor(reg).is_some() {
                    let object_node = object_literal_cast
                        .object_node
                        .expect("NullPointerException");
                    self.creator
                        .set_deferred_type(compiler, object_node, type_.unwrap());
                    object_node.put_boolean_prop(compiler, Prop::REFLECTED_OBJECT, true);
                } else {
                    let error = JSError::make(compiler, n, &CONSTRUCTOR_EXPECTED, &[]);
                    self.creator.report(compiler, error);
                }
            } else {
                let error = JSError::make(
                    compiler,
                    n,
                    object_literal_cast.diagnostic_type.unwrap(),
                    &[],
                );
                self.creator.report(compiler, error);
            }
        }
    }

    /// Declare a computed property on its owner type
    ///
    /// @param info The doc info for this property.
    /// @param n A top-level GETELEM node (it should not be contained inside another GETPROP).
    /// @param rhsValue The node that {@code n} is being initialized to, or {@code null} if this is
    ///     a stub declaration.
    // port: TypedScopeCreator.AbstractScopeBuilder#maybeDeclareGetElem
    fn maybe_declare_get_elem(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<Arc<JSDocInfo>>,
        n: NodeId,
        rhs_value: Option<NodeId>,
    ) {
        check_argument!(n.is_get_elem(compiler), "%s", n.to_string(compiler));
        let owner_node = n.get_first_child(compiler).expect("NullPointerException");
        let key = n.get_second_child(compiler).expect("NullPointerException");
        let key_type = extract_known_symbol_key(compiler, self.current_scope, key);
        if key_type.is_none() {
            return;
        }
        let value_type = self.get_declared_type(compiler, info, n, rhs_value, None);
        if value_type.is_none() {
            return;
        }
        let owner_name = owner_node.get_qualified_name(compiler);
        let owner_type = self.get_object_slot(compiler, owner_name);
        if owner_type.is_none() {
            return;
        }
        self.declare_property_if_namespace_type(
            compiler,
            owner_type.unwrap(),
            owner_node,
            PropertyKey::Symbol(key_type.unwrap()),
            value_type,
            n,
            /* jsdocInfo= */ None,
        );
    }

    /// Declare the symbol for a qualified name in the current scope.
    ///
    /// @param info The doc info for this property.
    /// @param n A top-level GETPROP node (it should not be contained inside another GETPROP).
    /// @param parent The parent of {@code n}.
    /// @param rhsValue The node that {@code n} is being initialized to, or {@code null} if this is
    ///     a stub declaration.
    // port: TypedScopeCreator.AbstractScopeBuilder#maybeDeclareQualifiedName
    fn maybe_declare_qualified_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        info: Option<Arc<JSDocInfo>>,
        n: NodeId,
        parent: NodeId,
        rhs_value: Option<NodeId>,
    ) {
        let is_typedef = self.is_valid_typedef_declaration(t.get_compiler(), n, info.clone());
        if is_typedef {
            self.declare_typedef_type(t.get_compiler(), n, info.clone());
        }

        let compiler = t.get_compiler();
        let owner_node = n.get_first_child(compiler).expect("NullPointerException");
        let owner_name = owner_node.get_qualified_name(compiler);
        let q_name = n.get_qualified_name(compiler);
        let prop_name = n.get_string(compiler);
        check_argument!(q_name.is_some() && owner_name.is_some());
        let q_name = q_name.unwrap();
        let owner_name = owner_name.unwrap();

        // Precedence of type information on GETPROPs:
        // 1) @type annotation / @enum annotation
        // 2) ASSIGN to FUNCTION literal
        // 3) @param/@return annotation (with no function literal)
        // 4) ASSIGN to something marked @const
        // 5) ASSIGN to anything else
        //
        // 1, 3, and 4 are declarations, 5 is inferred, and 2 is a declaration iff
        // the function has JsDoc or has not been declared before.
        //
        // FUNCTION literals are special because TypedScopeCreator is very smart
        // about getting as much type information as possible for them.

        // Determining type for #1 + #2 + #3 + #4
        let mut value_type = self.get_declared_type(compiler, info.clone(), n, rhs_value, None);
        if value_type.is_none() && rhs_value.is_some() {
            // Determining type for #5
            value_type = rhs_value.unwrap().get_jstype(compiler);
        }

        // Function prototypes are special.
        // It's a common JS idiom to do:
        // F.prototype = { ... };
        // So if F does not have an explicitly declared super type,
        // allow F.prototype to be redefined arbitrarily.
        if prop_name == "prototype" {
            let q_var = self.current_scope.get_var(compiler, q_name.clone());
            if q_var.is_some() {
                let q_var = q_var.unwrap();
                // If the programmer has declared that F inherits from Super,
                // and they assign F.prototype to an object literal,
                // then they are responsible for making sure that the object literal's
                // implicit prototype is set up appropriately. We just obey
                // the @extends tag.
                let q_var_declared_type = q_var.get_type(compiler);
                let q_var_type = {
                    let (reg, _) = compiler.get_type_registry_and_ast();
                    object_type::cast(reg, q_var_declared_type)
                };
                if q_var_type.is_some()
                    && rhs_value.is_some()
                    && rhs_value.unwrap().is_object_lit(compiler)
                {
                    let rhs_type = rhs_value.unwrap().get_jstype(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let implicit_prototype = q_var_type.unwrap().get_implicit_prototype(reg, ast);
                    reg.reset_implicit_prototype(
                        rhs_type.expect("NullPointerException"),
                        implicit_prototype,
                    );
                } else if !q_var.is_type_inferred(compiler) {
                    // If the programmer has declared that F inherits from Super,
                    // and they assign F.prototype to some arbitrary expression,
                    // there's not much we can do. We just ignore the expression,
                    // and hope they've annotated their code in a way to tell us
                    // what props are going to be on that prototype.
                    return;
                }

                q_var.get_scope(compiler).undeclare(compiler, q_var);
            }
        }

        if value_type.is_none() {
            if parent.is_expr_result(compiler) {
                // t is mutable so make sure to capture the current state before the lambda.
                let is_extern = t.get_input().is_some_and(|input| input.is_extern());
                let compiler = t.get_compiler();
                let root = self.current_scope.get_root_node(compiler);
                self.put_deferred_action(
                    root,
                    DeferredAction::ResolveStubDeclaration {
                        n,
                        is_extern,
                        owner_name,
                    },
                );
            }

            return;
        }

        let inferred = self.is_qualified_name_inferred(
            compiler,
            Some(q_name.clone()),
            n,
            info,
            rhs_value,
            value_type.unwrap(),
        );
        if !inferred {
            let owner_type = self.get_object_slot(compiler, Some(owner_name));
            if owner_type.is_some() {
                self.declare_property_if_namespace_type(
                    compiler,
                    owner_type.unwrap(),
                    owner_node,
                    PropertyKey::String(prop_name),
                    value_type,
                    n,
                    /* jsdocInfo= */ None,
                );
            }

            // this is a memory optimization: we don't need to declare .prototype props in the scope.
            // NOTE: in theory we could possibly extend this to more kinds of declarations and only
            // declare simple (non-qualified) names in the scope, but that seems to cause a lot more
            // issues with older code.
            let declare_in_scope = owner_type.is_none() || {
                let (reg, _) = compiler.get_type_registry_and_ast();
                !owner_type.unwrap().is_function_prototype_type(reg)
            };

            // If the property is already declared, the error will be caught when we try to declare it
            // in the current scope.
            if declare_in_scope {
                let scope = self.get_l_value_root_scope(compiler, Some(n));
                SlotDefiner::new()
                    .for_declaration_node(Some(n))
                    .for_variable_name(q_name)
                    .in_scope(scope)
                    .with_type(value_type)
                    .allow_later_type_inference(inferred)
                    .define_slot(self, compiler);
            }
        }
    }

    /// Determines whether a qualified name is inferred. NOTE(nicksantos): Determining whether a
    /// property is declared or not is really really obnoxious.
    ///
    /// The problem is that there are two (equally valid) coding styles:
    ///
    /// (function() { /* The authoritative definition of goog.bar. / goog.bar = function() {};
    /// })();
    ///
    /// function f() { goog.bar(); /* Reset goog.bar to a no-op. / goog.bar = function() {}; }
    ///
    /// In a dynamic language with first-class functions, it's very difficult to know which one
    /// the user intended without looking at lots of contextual information (the second example
    /// demonstrates a small case of this, but there are some really pathological cases as well).
    ///
    /// The current algorithm checks if either the declaration has JsDoc type information,
    /// or @const with a known type, or a function literal with a name we haven't seen before.
    // port: TypedScopeCreator.AbstractScopeBuilder#isQualifiedNameInferred
    fn is_qualified_name_inferred(
        &mut self,
        compiler: &mut AbstractCompiler,
        q_name: Option<JsString>,
        n: NodeId,
        info: Option<Arc<JSDocInfo>>,
        rhs_value: Option<NodeId>,
        value_type: TypeId,
    ) -> bool {
        // Prototypes of constructors and interfaces are always declared.
        let prototype_suffix = JsString::from(".prototype");
        if q_name.is_some() && q_name.as_ref().unwrap().ends_with(&prototype_suffix) {
            let q_name_ref = q_name.as_ref().unwrap();
            let class_name =
                q_name_ref.substring(0, q_name_ref.last_index_of(&prototype_suffix) as usize);
            let slot = self.current_scope.get_var(compiler, class_name);
            let class_type = if slot.is_none() {
                None
            } else {
                slot.unwrap().get_type(compiler)
            };
            let (reg, _) = compiler.get_type_registry_and_ast();
            if class_type.is_some()
                && (class_type.unwrap().is_constructor(reg)
                    || class_type.unwrap().is_interface(reg))
            {
                return false;
            }
        }
        // treat "foo = bar = <VAL>" the same as "foo = <VAL>"
        let rhs_value = Self::unwrap_if_assign(compiler, rhs_value);

        // If the jsdoc or RHS specifies a concrete type, it's not inferred.
        if (info.is_some()
            && (info.as_ref().unwrap().has_type()
                || info.as_ref().unwrap().has_enum_parameter_type()
                || self.is_valid_typedef_declaration(compiler, n, info.clone())
                || FunctionTypeBuilder::is_function_type_declaration(info.as_ref().unwrap())
                || (rhs_value.is_some() && rhs_value.unwrap().is_function(compiler))))
            || self.is_typed_constant_declaration(compiler, info.as_deref(), n, Some(value_type))
        {
            return false;
        }

        // At this point, we're pretty sure it's inferred, since there's neither
        // useful jsdoc info, nor a useful const or doc'd function RHS.  But
        // there's still one case where it may still not be: if the RHS is a
        // class or function that is not
        //   (1) a scoped qualified name (i.e. this.b.c or super.b.c),
        //   (2) already declared in a scope,
        //   (3) assigned in a conditional block, or
        //   (4) escaped to a closure,
        // then we treat it as if it is declared, rather than inferred.
        // Stubs and other values are always considered inferred at this point.
        if rhs_value.is_none()
            || (!rhs_value.unwrap().is_function(compiler) && !rhs_value.unwrap().is_class(compiler))
        {
            return true;
        }

        // "Scoped" qualified names (e.g. this.b.c or super.d) are inferred.
        if !n.is_unscoped_qualified_name(compiler) {
            return true;
        }

        // If this qname is already declared then treat this definition as inferred.
        let owner_scope = self.get_l_value_root_scope(compiler, Some(n));
        // ownerScope is never null (getLValueRootScope returns a scope).
        if owner_scope.has_own_slot(compiler, q_name.as_ref().expect("NullPointerException")) {
            return true;
        }

        // Check if this is in a conditional block.
        // Functions assigned in conditional blocks are inferred.
        if self.has_control_structure_ancestor(
            compiler,
            n.get_parent(compiler).expect("NullPointerException"),
        ) {
            return true;
        }

        // Check if this is assigned in an inner scope.
        // Functions assigned in inner scopes are inferred.
        let owner_root = owner_scope.get_root_node(compiler);
        if self
            .creator
            .escaped_var_names
            .contains(&<dyn ScopedName>::of(q_name.unwrap(), Some(owner_root)))
        {
            return true;
        }

        false
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#unwrapIfAssign
    fn unwrap_if_assign(
        compiler: &AbstractCompiler,
        maybe_assign: Option<NodeId>,
    ) -> Option<NodeId> {
        if maybe_assign.is_none() || !maybe_assign.unwrap().is_assign(compiler) {
            return maybe_assign;
        }
        Self::unwrap_if_assign(compiler, maybe_assign.unwrap().get_second_child(compiler))
    }

    /// Given a `goog.provide()` or legacy `goog.module()` call and implicit ProvidedName, declares
    /// the name in the global scope.
    // port: TypedScopeCreator.AbstractScopeBuilder#declareProvidedNs
    fn declare_provided_ns(
        &mut self,
        compiler: &mut AbstractCompiler,
        provide_call: NodeId,
        provided_name: &ProvidedName,
    ) {
        // Redefine this name if we haven't already added a provide definition.
        // Note: in some cases, this will cause a redefinition error.
        let anonymous_object_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            reg.create_anonymous_object_type(ast, None)
        };
        let global_scope = self.current_scope.get_global_scope(compiler);
        SlotDefiner::new()
            .in_scope(global_scope)
            .allow_later_type_inference(false)
            .for_variable_name(provided_name.get_namespace().clone())
            .for_declaration_node(Some(provide_call))
            .with_type(Some(anonymous_object_type))
            .for_goog_provided_name()
            .define_slot(self, compiler);

        let namespace = QualifiedName::of(provided_name.get_namespace().clone());
        if !namespace.is_simple(compiler) {
            let owner = namespace.get_owner(compiler).expect("NullPointerException");
            let owner_type = lookup_qualified_name(compiler, self.current_scope, &owner);
            let component = namespace.get_component(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if owner_type.is_some() && owner_type.unwrap().is_object_type(reg, ast) {
                owner_type
                    .unwrap()
                    .to_maybe_object_type(reg)
                    .unwrap()
                    .define_declared_property(
                        reg,
                        ast,
                        component,
                        anonymous_object_type,
                        Some(provide_call),
                    );
            }
        }
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#isTypedConstantDeclaration
    fn is_typed_constant_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
        n: NodeId,
        value_type: Option<TypeId>,
    ) -> bool {
        (NodeUtil::is_constant_declaration(compiler, info, n)
            || self.is_goog_module_exports(compiler, Some(n)))
            && value_type.is_some()
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#hasControlStructureAncestor
    fn has_control_structure_ancestor(&self, compiler: &AbstractCompiler, mut n: NodeId) -> bool {
        while !(n.is_script(compiler) || n.is_function(compiler)) {
            if NodeUtil::is_control_structure(compiler, n) {
                return true;
            }
            n = n.get_parent(compiler).expect("NullPointerException");
        }
        false
    }

    /// Find the ObjectType associated with the given slot.
    ///
    /// @param slotName The name of the slot to find the type in.
    /// @return An object type, or null if this slot does not contain an object.
    // port: TypedScopeCreator.AbstractScopeBuilder#getObjectSlot
    fn get_object_slot(
        &mut self,
        compiler: &mut AbstractCompiler,
        slot_name: Option<JsString>,
    ) -> Option<TypeId> {
        // currentScope.getVar(null) throws in Java's map lookup.
        let owner_var = self
            .current_scope
            .get_var(compiler, slot_name.expect("NullPointerException"));
        if owner_var.is_some() {
            let owner_var_type = owner_var.unwrap().get_type(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let restricted = if owner_var_type.is_none() {
                None
            } else {
                Some(
                    owner_var_type
                        .unwrap()
                        .restrict_by_not_null_or_undefined(reg, ast),
                )
            };
            return object_type::cast(reg, restricted);
        }
        None
    }

    /// When a class has a stub for a property, and the property exists on a super interface, use
    /// that type.
    // port: TypedScopeCreator.AbstractScopeBuilder#getInheritedInterfacePropertyType
    fn get_inherited_interface_property_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        obj: Option<TypeId>,
        prop_name: &JsString,
    ) -> Option<TypeId> {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if obj.is_some() && obj.unwrap().is_function_prototype_type(reg) {
            let f = obj
                .unwrap()
                .get_owner_function(reg)
                .expect("NullPointerException");
            for i in f.get_implemented_interfaces(reg, ast) {
                if i.has_property(reg, ast, prop_name.clone()) {
                    return Some(i.get_property_type(reg, ast, prop_name.clone()));
                }
            }
        }
        None
    }

    /// Resolve any type-less stub declarations to unknown types if we could not find types for them
    /// during traversal. This method is only called as a deferred action after the root node is
    /// visted.
    // port: TypedScopeCreator.AbstractScopeBuilder#resolveStubDeclaration
    fn resolve_stub_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_extern: bool,
        owner_name: JsString,
    ) {
        let q_name = n
            .get_qualified_name(compiler)
            .expect("NullPointerException");
        let prop_name = n.get_string(compiler);

        // TODO(b/111216910): should this be getLValueRoot(n).hasOwnSlot(qName)?
        if self.current_scope.has_own_slot(compiler, &q_name) {
            return;
        }

        // If we see a stub property, make sure to register this property
        // in the type registry.
        let owner_type = self.get_object_slot(compiler, Some(owner_name));
        let inherited_type =
            self.get_inherited_interface_property_type(compiler, owner_type, &prop_name);
        let stub_type = if inherited_type.is_none() {
            self.creator.unknown_type
        } else {
            inherited_type.unwrap()
        };
        let scope = self.get_l_value_root_scope(compiler, Some(n));
        SlotDefiner::new()
            .for_declaration_node(Some(n))
            .read_variable_name_from_declaration_node(compiler)
            .in_scope(scope)
            .with_type(Some(stub_type))
            .allow_later_type_inference(true)
            .define_slot(self, compiler);

        let (reg, ast) = compiler.get_type_registry_and_ast();
        if owner_type.is_some()
            && (is_extern || owner_type.unwrap().is_function_prototype_type(reg))
        {
            // If this is a stub for a prototype, just declare it
            // as an unknown type. These are seen often in externs.
            owner_type
                .unwrap()
                .define_inferred_property(reg, ast, prop_name, stub_type, Some(n));
        } else {
            reg.register_property_on_type(
                ast,
                prop_name,
                if owner_type.is_none() {
                    stub_type
                } else {
                    owner_type.unwrap()
                },
            );
        }
    }

    /// Returns whether this is a valid declaration of a @typedef.
    ///
    /// @param candidate A qualified name node.
    /// @param info JSDoc comments.
    // port: TypedScopeCreator.AbstractScopeBuilder#isValidTypedefDeclaration
    fn is_valid_typedef_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        candidate: NodeId,
        info: Option<Arc<JSDocInfo>>,
    ) -> bool {
        if info.is_none() || !info.as_ref().unwrap().has_typedef_type() {
            return false;
        }
        // `isUnscopedQualifiedName` excludes `this` and `super` properties.
        candidate.is_unscoped_qualified_name(compiler)
            && !NodeUtil::is_prototype_property(compiler, candidate)
    }

    /// Declares a typedef'd name in the {@link JSTypeRegistry}.
    // port: TypedScopeCreator.AbstractScopeBuilder#declareTypedefType
    fn declare_typedef_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        candidate: NodeId,
        info: Option<Arc<JSDocInfo>>,
    ) {
        let typedef = candidate
            .get_qualified_name(compiler)
            .expect("NullPointerException");

        // TODO(nicksantos|user): This is a terrible, terrible hack
        // to bail out on recursive typedefs. We'll eventually need
        // to handle these properly.
        let current_scope_view = self.current_scope.as_static_typed_scope(compiler);
        let current_scope_view_arc = self.current_scope.as_static_typed_scope_arc(compiler);
        let unknown_type = self.creator.unknown_type;
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.declare_type(ast, Some(current_scope_view), typedef.clone(), unknown_type);

        // JSTypeExpression#evaluate never returns null, so Java's MALFORMED_TYPEDEF branch
        // (realType == null) cannot be taken.
        let real_type = info
            .expect("NullPointerException")
            .get_typedef_type()
            .expect("NullPointerException")
            .evaluate(reg, ast, Some(current_scope_view_arc));
        candidate.set_typedef_type_prop(compiler, Some(real_type));

        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.overwrite_declared_type(ast, Some(current_scope_view), typedef, real_type);
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#declarePropertyIfNamespaceType
    #[allow(clippy::too_many_arguments)]
    fn declare_property_if_namespace_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        owner_type: TypeId,
        owner_node: NodeId,
        prop_name: PropertyKey,
        value_type: Option<TypeId>,
        declaration_node: NodeId,
        jsdoc_info: Option<Arc<JSDocInfo>>,
    ) {
        let compiler_input = self.get_compiler_input(compiler);
        let owner_is_this = owner_node.is_this(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        // Only declare this as an official property if it has not been
        // declared yet.
        if owner_type.has_own_property(reg, ast, prop_name.clone())
            && !owner_type.is_property_type_inferred(reg, ast, prop_name.clone())
        {
            return;
        }
        // Define the property if any of the following are true:
        //   (1) it's a non-native extern type. Native types are excluded here because we don't
        //       want externs of the form "/** @type {!Object} */ var api = {}; api.foo;" to
        //       cause a property "foo" to be declared on Object.
        //   (2) it's a non-instance type. This primarily covers static properties on
        //       constructors (which are FunctionTypes, not InstanceTypes).
        //   (3) it's an assignment to 'this', which covers instance properties assigned in
        //       constructors or other methods.
        let is_non_native_extern = compiler_input.is_some()
            && compiler_input.as_ref().unwrap().is_extern()
            && !owner_type.is_native_object_type(reg);
        if is_non_native_extern || !owner_type.is_instance_type(reg) || owner_is_this {
            // If the property is undeclared or inferred, declare it now.
            owner_type.define_declared_property(
                reg,
                ast,
                prop_name.clone(),
                value_type.expect("NullPointerException"),
                Some(declaration_node),
            );
            owner_type.set_property_jsdoc_info(reg, ast, prop_name, jsdoc_info);
        }
    }
}

// port: TypedScopeCreator.NormalScopeBuilder
// (NormalScopeBuilder, FunctionScopeBuilder and ClassScopeBuilder have no fields of their own;
// their constructors are `AbstractScopeBuilder::new` with the matching `ScopeBuilderKind`.)
impl AbstractScopeBuilder<'_> {
    // port: TypedScopeCreator.NormalScopeBuilder#visitPreorder
    fn normal_visit_preorder(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        let compiler = t.get_compiler();
        // Create any child block scopes "pre-order" as we see them.
        //
        // This is required because hoisted or qualified names defined in earlier blocks might be
        // referred to later outside the block. This isn't a big deal in most cases since a NamedType
        // will be created and resolved later, but if a NamedType is used for a superclass, we lose a
        // lot of valuable checking. Recursing into child blocks immediately prevents this from being
        // a problem.
        //
        // We don't traverse into CLASSes because we haven't yet have created the class-type on which
        // to assign members. We'll do this on the way back up (post-order) instead, after the
        // class-type has been attached to the AST.
        if parent.is_some() && NodeUtil::creates_block_scope(compiler, n) && !n.is_class(compiler) {
            let current_scope = self.current_scope;
            self.creator.create_scope(compiler, n, Some(current_scope));
        }

        // All other functions (and classes, etc) are handled when we see the actual function node.
        if n.is_function(compiler) {
            self.define_function_literal(compiler, n);
        }
    }

    // port: TypedScopeCreator.NormalScopeBuilder#visitPostorder
    fn normal_visit_postorder(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        match n.get_token(t.get_compiler()) {
            Token::CALL => self.check_for_class_defining_calls(t.get_compiler(), n),
            Token::ASSIGN => {
                // Handle initialization of properties.
                // We only allow qualified name declarations of the form
                //   /** @type {number} */ a.b.c = rhs;
                // or, for symbol-typed properties:
                //   /** @type {number} */ a.b[Symbol.something] = rhs;
                // TODO(b/77597706): Ensure that CheckJSDoc warns for JSDoc on assignments not to
                // qualified names, e.g.
                //   /** @type {number} */ [a.b.c] = someArr;
                let compiler = t.get_compiler();
                let first_child = n.get_first_child(compiler).expect("NullPointerException");
                if first_child.is_get_prop(compiler) && first_child.is_qualified_name(compiler) {
                    let info = n.get_jsdoc_info(compiler);
                    let next = first_child.get_next(compiler);
                    self.maybe_declare_qualified_name(t, info, first_child, n, next);
                } else if first_child.is_get_elem(compiler)
                    && first_child
                        .get_first_child(compiler)
                        .expect("NullPointerException")
                        .is_qualified_name(compiler)
                {
                    let info = n.get_jsdoc_info(compiler);
                    let next = first_child.get_next(compiler);
                    self.maybe_declare_get_elem(compiler, info, first_child, next);
                } else if self
                    .creator
                    .undeclared_names_for_closure
                    .contains(&first_child)
                {
                    self.define_assign_as_if_var_declaration(compiler, n);
                }
            }
            Token::CATCH => self.define_catch(t.get_compiler(), n),
            Token::VAR | Token::LET | Token::CONST => self.define_vars(t.get_compiler(), n),
            Token::GETPROP => {
                // Handle stubbed properties.
                let compiler = t.get_compiler();
                let parent = parent.expect("NullPointerException");
                if parent.is_expr_result(compiler) && n.is_qualified_name(compiler) {
                    let info = n.get_jsdoc_info(compiler);
                    self.maybe_declare_qualified_name(t, info, n, parent, None);
                }
            }
            Token::GETELEM => {
                // Handle stubbed properties.
                let compiler = t.get_compiler();
                if parent
                    .expect("NullPointerException")
                    .is_expr_result(compiler)
                    && n.get_first_child(compiler)
                        .expect("NullPointerException")
                        .is_qualified_name(compiler)
                {
                    let info = n.get_jsdoc_info(compiler);
                    self.maybe_declare_get_elem(compiler, info, n, None);
                }
            }
            Token::CLASS => {
                // Analyse CLASS child-scopes now because later code in this scope may assign
                // properties to these class-types. We want to ensure declarations within the CLASS have
                // priority.
                let current_scope = self.current_scope;
                self.creator
                    .create_scope(t.get_compiler(), n, Some(current_scope));
            }
            Token::EXPR_RESULT => {
                let names = self.creator.provided_names_from_call.get(&n).cloned();
                if names.is_some() {
                    for name in names.unwrap() {
                        self.declare_provided_ns(t.get_compiler(), n, &name);
                    }
                }
            }
            Token::EXPORT => {
                let compiler = t.get_compiler();
                if n.get_boolean_prop(compiler, Prop::EXPORT_DEFAULT) {
                    // Define a dummy var for "export default <someExpr>" so that other utilities have
                    // access to the type.
                    // Problem: according to the debugger, if we have export default SomeName
                    // SomeName does not have a type attached so we don't really export it and can't use it
                    // as a type later. So we need to call getDeclaredType() instead to get the type.
                    let only_child = n.get_only_child(compiler);
                    let declared_type =
                        self.get_declared_type(compiler, None, n, Some(only_child), None);
                    let current_scope = self.current_scope;
                    SlotDefiner::new()
                        .in_scope(current_scope)
                        .for_declaration_node(Some(n))
                        .with_type(declared_type)
                        .for_variable_name(Export::DEFAULT_EXPORT_NAME)
                        .allow_later_type_inference(declared_type.is_none())
                        .define_slot(self, compiler);
                }
            }
            _ => {}
        }
    }
}

/// Scope builder subclass for function scopes, which only contain bleeding function names and
/// parameter names. The main function body is handled by the NormalScopeBuilder on the function
/// block.
// port: TypedScopeCreator.FunctionScopeBuilder
impl AbstractScopeBuilder<'_> {
    // port: TypedScopeCreator.FunctionScopeBuilder#visitPreorder
    fn function_visit_preorder(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        let compiler = t.get_compiler();
        if parent.is_none() {
            self.handle_function_inputs(compiler);
        } else if n.is_function(compiler) {
            self.define_function_literal(compiler, n);
        }
    }

    /// Handle bleeding functions and function parameters.
    // port: TypedScopeCreator.FunctionScopeBuilder#handleFunctionInputs
    fn handle_function_inputs(&mut self, compiler: &mut AbstractCompiler) {
        // Handle bleeding functions. These are defined as function expressions which have a non-empty
        // name, which we declare in the FUNCTION scope. Function declarations are hoisted and are
        // already declared in the containing scope; ignore those.
        let fn_node = self.current_scope.get_root_node(compiler);
        let fn_name_node = fn_node
            .get_first_child(compiler)
            .expect("NullPointerException");
        let fn_name = fn_name_node.get_string(compiler);
        if !fn_name.is_empty() && NodeUtil::is_function_expression(compiler, fn_node) {
            let current_scope = self.current_scope;
            let fn_type = fn_node.get_jstype(compiler);
            SlotDefiner::new()
                .for_declaration_node(Some(fn_name_node))
                .for_variable_name(fn_name)
                .in_scope(current_scope)
                .with_type(fn_type)
                .allow_later_type_inference(false)
                .define_slot(self, compiler);
        }

        self.declare_parameters(compiler, fn_node);
    }

    /// Declares all of a function's parameters inside the function's scope.
    // port: TypedScopeCreator.FunctionScopeBuilder#declareParameters
    fn declare_parameters(&mut self, compiler: &mut AbstractCompiler, function_node: NodeId) {
        if NodeUtil::is_bundled_goog_module_call(
            compiler,
            function_node
                .get_parent(compiler)
                .expect("NullPointerException"),
        ) {
            // Skip declaring 'exports' for a goog.loadModule(function(exports) {.
            // We pretend that any assignments to 'exports' in the body are actually declarations.
            return;
        }

        let ast_parameters = function_node
            .get_second_child(compiler)
            .expect("NullPointerException");
        let mut iife_argument_node: Option<NodeId> = None;

        if NodeUtil::is_invocation_target(compiler, function_node) {
            iife_argument_node = function_node.get_next(compiler);
        }

        let function_type = {
            let js_type = function_node.get_jstype(compiler);
            let (reg, _) = compiler.get_type_registry_and_ast();
            closure_jstype::js_type::to_maybe_function_type(reg, js_type)
        };
        if function_type.is_some() {
            let function_type = function_type.unwrap();
            let parameters = {
                let (reg, _) = compiler.get_type_registry_and_ast();
                function_type.get_parameters(reg)
            };
            let mut jsdoc_parameters = parameters.into_iter();
            let mut js_doc_parameter = jsdoc_parameters.next();

            let mut ast_parameter = ast_parameters.get_first_child(compiler);
            while ast_parameter.is_some() {
                if iife_argument_node.is_some() && iife_argument_node.unwrap().is_spread(compiler) {
                    // don't try inferring types from spreads in iifes because we don't know how
                    // many items are in the iterable.
                    iife_argument_node = None;
                }
                let declared_type = if js_doc_parameter.is_none() {
                    self.creator.unknown_type
                } else {
                    js_doc_parameter.as_ref().unwrap().get_jstype()
                };
                self.declare_names_in_positional_parameter(
                    compiler,
                    ast_parameter.unwrap(),
                    declared_type,
                    iife_argument_node,
                );
                if js_doc_parameter.is_some() {
                    js_doc_parameter = jsdoc_parameters.next();
                }
                if iife_argument_node.is_some() {
                    iife_argument_node = iife_argument_node.unwrap().get_next(compiler);
                }
                ast_parameter = ast_parameter.unwrap().get_next(compiler);
            }

            self.declare_templates_in_function_body(compiler, function_node, function_type);
        }
    } // end declareParameters

    // port: TypedScopeCreator.FunctionScopeBuilder#declareTemplatesInFunctionBody
    fn declare_templates_in_function_body(
        &mut self,
        compiler: &mut AbstractCompiler,
        function_node: NodeId,
        function_type: TypeId,
    ) {
        // Add template params to the scope.
        // This lets JSTypeRegistry resolve references to template types within the function body
        // scope. (they were already registered as types by FunctionTypeBuilder, but they need to be
        // in a TypedScope to let JSTypeRegistry correctly handle scoping).
        let info = NodeUtil::get_best_jsdoc_info(compiler, function_node);
        let mut template_names: IndexSet<JsString> = IndexSet::<_>::default();

        let is_possible_prototype_method = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            !function_type.has_instance_type(reg)
        } && (info.is_none()
            || !info.as_ref().unwrap().has_this_type());
        if is_possible_prototype_method {
            // If this is a prototype method, add any @template types from the prototype owner.
            // e.g. given
            //   /** @template T */
            //   class Foo {
            //     bar() {}
            //   }
            // We want to declare 'T' in the scope of 'bar'
            let owner_node = NodeUtil::get_best_l_value(compiler, function_node);
            for template_key in self.find_owner_type_keys(compiler, owner_node) {
                let (reg, _) = compiler.get_type_registry_and_ast();
                template_names.insert(
                    template_key
                        .get_reference_name(reg)
                        .expect("NullPointerException"),
                );
            }
        }
        if info.is_some() {
            // Add @template parameters from the JSDoc on this function directly. For example, given
            //   /** @template U */
            //   function foo() {}
            // This declares 'U'.
            template_names.extend(info.as_ref().unwrap().get_template_type_names());
            template_names.extend(
                info.as_ref()
                    .unwrap()
                    .get_type_transformations()
                    .into_keys(),
            );
        }
        if template_names.is_empty() {
            return;
        }
        let input = self.get_compiler_input(compiler);
        let void_type = self
            .creator
            .get_native_type(compiler, JSTypeNative::VOID_TYPE);
        // Declare any template names in the function scope. This means that if someone shadows
        // an outer variable FOO with a @template FOO and refers to FOO inside the method, we
        // will treat it as undefined, rather than the correct type, which could lead to weird
        // errors. Ideally we'd have a "don't use me" type that gives an error at use.
        for name in template_names {
            if !self.current_scope.can_declare(compiler, name.clone()) {
                let source_name = NodeUtil::get_source_name(compiler, function_node);
                let parent = function_node.get_parent(compiler);
                let var = self.current_scope.get_var(compiler, name.clone());
                let validator = self.creator.validator.clone();
                // Java's sourceName parameter is unused by the method body.
                validator.expect_undeclared_variable(
                    compiler,
                    source_name.as_deref().unwrap_or_default(),
                    input.clone(),
                    function_node,
                    parent.expect("NullPointerException"),
                    var.expect("NullPointerException"),
                    &name.to_string_lossy(),
                    Some(void_type),
                );
            }
            self.current_scope.declare(
                compiler,
                name,
                Some(function_node),
                Some(void_type),
                input.clone(),
                /* inferred= */ false,
            );
        }
    }

    /// Declares the name(s) in a positional AST parameter in the scope.
    ///
    /// @param astParameter the positional parameter node
    /// @param declaredParameterType the declared parameter type, or the unknown type if there is
    ///     none
    /// @param iifeArgumentNode the corresponding argument from the iife, if in an iife. e.g. for
    ///     `(function (x) {}(3);` this would be `3`.
    // port: TypedScopeCreator.FunctionScopeBuilder#declareNamesInPositionalParameter
    fn declare_names_in_positional_parameter(
        &mut self,
        compiler: &mut AbstractCompiler,
        ast_parameter: NodeId,
        declared_parameter_type: TypeId,
        iife_argument_node: Option<NodeId>,
    ) {
        let mut param_type = Some(declared_parameter_type);
        let is_inferred = declared_parameter_type == self.creator.unknown_type;

        if iife_argument_node.is_some() && is_inferred {
            let argument_name = iife_argument_node.unwrap().get_qualified_name(compiler);
            let parent_scope = self.current_scope.get_parent(compiler);
            let argument_var = if argument_name.is_none() || parent_scope.is_none() {
                None
            } else {
                parent_scope
                    .unwrap()
                    .get_var(compiler, argument_name.unwrap())
            };
            if argument_var.is_some() && !argument_var.unwrap().is_type_inferred(compiler) {
                param_type = argument_var.unwrap().get_type(compiler);
            }
        }

        if param_type.is_none() {
            param_type = Some(self.creator.unknown_type);
        }
        let param_type = param_type.unwrap();

        match ast_parameter.get_token(compiler) {
            Token::NAME => {
                // function f(x) {}
                self.declare_single_parameter_name(
                    compiler,
                    is_inferred,
                    ast_parameter,
                    Some(param_type),
                )
            }
            Token::ITER_REST => {
                // function f(...x) {}
                // rest parameter is actually an array of the type specified in the JSDoc
                let param = ast_parameter
                    .get_first_child(compiler)
                    .expect("NullPointerException");
                let rest_param_type = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let array_type = reg.get_native_object_type(JSTypeNative::ARRAY_TYPE);
                    reg.create_templatized_type(ast, array_type, &[param_type])
                };
                if param.is_name(compiler) {
                    let first_child = ast_parameter.get_first_child(compiler).unwrap();
                    self.declare_single_parameter_name(
                        compiler,
                        is_inferred,
                        first_child,
                        Some(rest_param_type),
                    );
                } else {
                    // function f(...{length}) {}
                    self.declare_destructuring_parameter(
                        compiler,
                        is_inferred,
                        param,
                        Some(rest_param_type),
                    );
                }
            }
            Token::DEFAULT_VALUE => {
                // function f(x = 3) {} or function f([x] = []) {}
                let actual_param = ast_parameter
                    .get_first_child(compiler)
                    .expect("NullPointerException");
                if actual_param.is_name(compiler) {
                    self.declare_single_parameter_name(
                        compiler,
                        is_inferred,
                        actual_param,
                        Some(param_type),
                    );
                } else {
                    self.declare_destructuring_parameter(
                        compiler,
                        is_inferred,
                        actual_param,
                        Some(param_type),
                    );
                }
            }
            Token::ARRAY_PATTERN | Token::OBJECT_PATTERN => {
                // function f([x]) {}
                // function f({x}) {}
                self.declare_destructuring_parameter(
                    compiler,
                    is_inferred,
                    ast_parameter,
                    Some(param_type),
                )
            }
            _ => panic!(
                "IllegalStateException: Unexpected function parameter node {}",
                ast_parameter.to_string(compiler)
            ),
        }
    }

    /// Declares all names inside a destructuring pattern in a parameter list in the scope if we can
    /// find a non-unknown type for them.
    ///
    /// Unknown typed parameters are always treated as inferred, not declared. TypeInference may
    /// later give them a better inferred type than unknown, but they will never become declared.
    ///
    /// NOTE: currently, there are some less-than-ideal aspects to how we do this. If the pattern
    /// type is an unresolved NamedType, then we can't lookup properties on it (to find the
    /// individual parameter types) until after name resolution. The current state is to defer to
    /// TypeInference to type those parameters, with the drawback that they are 'inferred', not
    /// 'declared', and so any type can be assigned to them. In the future we will just enforce
    /// typing each parameter individually: <a
    /// href="https://github.com/google/closure-compiler/issues/1781">relevant issue</a>
    // port: TypedScopeCreator.FunctionScopeBuilder#declareDestructuringParameter
    fn declare_destructuring_parameter(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut is_inferred: bool,
        pattern: NodeId,
        pattern_type: Option<TypeId>,
    ) {
        for target in DestructuredTarget::create_all_non_empty_targets_in_pattern(
            compiler,
            pattern_type,
            pattern,
        ) {
            let mut parameter_type = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                Some(target.infer_type_without_using_default_value(reg, ast))
            };

            if target.get_node().is_destructuring_pattern(compiler) {
                self.declare_destructuring_parameter(
                    compiler,
                    is_inferred,
                    target.get_node(),
                    parameter_type,
                );
            } else {
                let param_name = target.get_node();
                check_state!(
                    param_name.is_name(compiler),
                    "Expected all parameters to be names, got %s",
                    param_name.to_string(compiler)
                );

                if parameter_type.is_none() || {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    parameter_type.unwrap().is_unknown_type(reg, ast)
                } {
                    let param_js_doc = param_name.get_jsdoc_info(compiler);
                    if param_js_doc.is_some() && param_js_doc.as_ref().unwrap().has_type() {
                        // see if the parameter has its own inline JSDoc, and use that unless we already have
                        // a type from @param JSDoc.
                        // TODO(b/112651122): this should happen inside FunctionTypeBuilder, so that we can
                        // check that calls to the function match the inline JSDoc.
                        // TODO(b/111523967): we should also report a
                        // warning if the inline and non-inline JSDoc conflict.
                        let current_scope_view =
                            self.current_scope.as_static_typed_scope_arc(compiler);
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        parameter_type = Some(reg.evaluate_type_expression(
                            ast,
                            &param_js_doc.unwrap().get_type().unwrap(),
                            Some(current_scope_view),
                        ));
                        is_inferred = false;
                    } else {
                        // note - these parameters may get better types during TypeInference
                        is_inferred = true;
                    }
                }
                self.declare_single_parameter_name(
                    compiler,
                    is_inferred,
                    param_name,
                    parameter_type,
                );
            }
        }
    }

    // port: TypedScopeCreator.FunctionScopeBuilder#declareSingleParameterName
    fn declare_single_parameter_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        is_inferred: bool,
        name: NodeId,
        type_: Option<TypeId>,
    ) {
        let current_scope = self.current_scope;
        SlotDefiner::new()
            .for_declaration_node(Some(name))
            .for_variable_name(name.get_string(compiler))
            .in_scope(current_scope)
            .with_type(type_)
            .allow_later_type_inference(is_inferred)
            .define_slot(self, compiler);
    }
} // end FunctionScopeBuilder

/// Scope builder subclass for class scopes (which only contain a bleeding class name), member
/// field def scopes, and RHS computed field def scopes (the latter two of which have `this` and
/// `super` properties). Methods are handled by FunctionScopeBuilder and NormalScopeBuilder for the
/// bodies.
// port: TypedScopeCreator.ClassScopeBuilder
impl AbstractScopeBuilder<'_> {
    // port: TypedScopeCreator.ClassScopeBuilder#visitPreorder
    fn class_visit_preorder(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) {
        let compiler = t.get_compiler();
        // These are not descended into, so must be done preorder
        if !n.is_function(compiler) {
            return;
        }

        if NodeUtil::is_es6_constructor(compiler, n) {
            // Constructor has already been analyzed, so pull that here.
            let root_type = self
                .current_scope
                .get_root_node(compiler)
                .get_jstype(compiler)
                .expect("NullPointerException");
            self.creator.set_deferred_type(compiler, n, root_type);
        } else {
            self.define_function_literal(compiler, n);
        }
    }

    // port: TypedScopeCreator.ClassScopeBuilder#visitPostorder
    fn class_visit_postorder(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        let compiler = t.get_compiler();
        // Java compares `parent == currentScope.getRootNode()` (false for null) and dereferences
        // parent only where the branches below do (MEMBER_FIELD_DEF gets no parent).
        if n.is_name(compiler)
            && parent == Some(self.current_scope.get_root_node(compiler))
            && NodeUtil::is_class_expression(compiler, parent.expect("NullPointerException"))
        {
            let parent = parent.expect("NullPointerException");
            // Declare bleeding class name in scope.  Pull the type off the AST.
            check_state!(!n.get_string_ref(compiler).is_empty()); // anonymous classes have EMPTY nodes, not NAME
            let current_scope = self.current_scope;
            let parent_type = parent.get_jstype(compiler);
            SlotDefiner::new()
                .for_declaration_node(Some(n))
                .read_variable_name_from_declaration_node(compiler)
                .in_scope(current_scope)
                .with_type(parent_type)
                .allow_later_type_inference(false)
                .define_slot(self, compiler);
        } else if NodeUtil::is_es6_constructor_member_function_def(compiler, n) {
            // Ignore "constructor" since it has special handling in `createClassTypeFromNodes()`.
        } else if n.is_member_function_def(compiler)
            && parent
                .expect("NullPointerException")
                .is_class_members(compiler)
        {
            self.define_member_function(compiler, n);
        } else if n.is_member_field_def(compiler) {
            // public fields are roots of their own scope so the parent doesn't get passed into
            // visitPostorder
            self.define_member_field(compiler, n);
        } else if (n.is_getter_def(compiler) || n.is_setter_def(compiler))
            && parent
                .expect("NullPointerException")
                .is_class_members(compiler)
        {
            self.define_getter_setter(compiler, n);
        } else if n.is_computed_field_def(compiler)
            || (n.is_computed_prop(compiler)
                && n.get_boolean_prop(compiler, Prop::COMPUTED_PROP_METHOD)
                && parent
                    .expect("NullPointerException")
                    .is_class_members(compiler))
        {
            self.define_computed_member_field(compiler, n);
        }
    }

    // port: TypedScopeCreator.ClassScopeBuilder#defineMemberFunction
    fn define_member_function(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let owner_type = self.determine_owner_type_for_class_member(compiler, n);
        let name = n.get_string(compiler);
        let fn_type = n
            .get_last_child(compiler)
            .expect("NullPointerException")
            .get_jstype(compiler)
            .expect("NullPointerException");
        let (reg, ast) = compiler.get_type_registry_and_ast();
        owner_type.define_declared_property(reg, ast, name, fn_type, Some(n));
    }

    // port: TypedScopeCreator.ClassScopeBuilder#defineMemberField
    fn define_member_field(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let rhs = n.get_last_child(compiler);
        let info = n.get_jsdoc_info(compiler);
        let mut declared_type = self.get_declared_type(compiler, info, n, rhs, None);
        // When there's no JSDoc type declaration for a field:
        // If we can infer a type other than UNKNOWN for an initial value, use that as the field type
        // Otherwise, set the field's type to the ALL type ('*')
        if declared_type.is_none() {
            let rhs_type = if rhs.is_none() {
                None
            } else {
                self.get_declared_r_value_type(compiler, None, rhs.unwrap())
            };
            declared_type = if rhs_type.is_some() && {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                !rhs_type.unwrap().is_unknown_type(reg, ast)
            } {
                rhs_type
            } else {
                Some(
                    self.creator
                        .get_native_type(compiler, JSTypeNative::ALL_TYPE),
                )
            };
        }
        let declared_type = declared_type.unwrap();

        let owner_type = self.determine_owner_type_for_class_member(compiler, n);
        let prop_name = n.get_string(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        owner_type.define_declared_property(reg, ast, prop_name, declared_type, Some(n));
        n.set_jstype(compiler, Some(declared_type));
    }

    // port: TypedScopeCreator.ClassScopeBuilder#defineComputedMemberField
    fn define_computed_member_field(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let key = n.get_first_child(compiler).expect("NullPointerException");
        if !key.is_qualified_name(compiler) {
            return;
        }
        let key_qname = key
            .get_qualified_name_object(compiler)
            .expect("NullPointerException");
        let key_type = lookup_qualified_name(compiler, self.current_scope, &key_qname);
        if key_type.is_none() || {
            let (reg, _) = compiler.get_type_registry_and_ast();
            !key_type.unwrap().is_known_symbol_value_type(reg)
        } {
            return;
        }
        let rhs = n.get_last_child(compiler);
        let info = n.get_jsdoc_info(compiler);
        let mut declared_type = self.get_declared_type(compiler, info, n, rhs, None);
        // When there's no JSDoc type declaration for a field:
        // If we can infer a type other than UNKNOWN for an initial value, use that as the field type
        // Otherwise, set the field's type to the ALL type ('*')
        if declared_type.is_none() {
            let rhs_type = if rhs.is_none() {
                None
            } else {
                self.get_declared_r_value_type(compiler, None, rhs.unwrap())
            };
            declared_type = if rhs_type.is_some() && {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                !rhs_type.unwrap().is_unknown_type(reg, ast)
            } {
                rhs_type
            } else {
                Some(
                    self.creator
                        .get_native_type(compiler, JSTypeNative::ALL_TYPE),
                )
            };
        }
        let declared_type = declared_type.unwrap();

        let owner_type = self.determine_owner_type_for_class_member(compiler, n);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let symbol = key_type
            .unwrap()
            .to_maybe_known_symbol_type(reg)
            .expect("NullPointerException");
        owner_type.define_declared_property(
            reg,
            ast,
            PropertyKey::Symbol(symbol),
            declared_type,
            Some(n),
        );
        n.set_jstype(compiler, Some(declared_type));
    }

    // port: TypedScopeCreator.ClassScopeBuilder#defineGetterSetter
    fn define_getter_setter(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let name = n.get_string(compiler);
        let token = n.get_token(compiler);
        let method_js_type = n
            .get_last_child(compiler)
            .expect("NullPointerException")
            .get_jstype(compiler)
            .expect("NullPointerException");
        let unknown_type = self.creator.unknown_type;
        let tree = if matches!(token, Token::GETTER_DEF | Token::SETTER_DEF) {
            String::new()
        } else {
            n.to_string_tree(compiler)
        };
        let (reg, _) = compiler.get_type_registry_and_ast();
        let method_type = method_js_type
            .to_maybe_function_type(reg)
            .expect("NullPointerException");

        let property_type = match token {
            Token::GETTER_DEF => {
                // TODO(sdh): consider only falling back on unknown if the function body is empty?
                // But
                // we need to not report a conflicting type error if there's different unknowns.
                if method_type.is_return_type_inferred(reg) {
                    unknown_type
                } else {
                    method_type.get_return_type(reg)
                }
            }
            Token::SETTER_DEF => {
                let parameters = method_type.get_parameters(reg);
                if parameters.is_empty() {
                    unknown_type
                } else {
                    parameters[0].get_jstype()
                }
            }
            _ => panic!("AssertionError: {tree}"),
        };

        // TODO(b/116797078): correctly model getters/setters and stop treating this as a normal
        // property.
        let owner_type = self.determine_owner_type_for_class_member(compiler, n);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        owner_type.define_declared_property(reg, ast, name, property_type, Some(n));
    }

    /// Returns the owner type for a class member function, getter, or setter.
    ///
    /// For a member on class C, this is either `C` for a static member or `C.prototype` for a
    /// nonstatic member.
    // port: TypedScopeCreator.ClassScopeBuilder#determineOwnerTypeForClassMember
    fn determine_owner_type_for_class_member(
        &mut self,
        compiler: &mut AbstractCompiler,
        member: NodeId,
    ) -> TypeId {
        // MEMBER_FUNCTION_DEF -> CLASS_MEMBERS -> CLASS or
        // MEMBER_FIELD_DEF -> CLASS_MEMBERS -> CLASS or
        // GETTER_DEF -> CLASS_MEMBERS -> CLASS or
        // SETTER_DEF -> CLASS_MEMBERS -> CLASS
        let owner_node = member
            .get_grandparent(compiler)
            .expect("NullPointerException");
        check_state!(owner_node.is_class(compiler));
        let owner_js_type = owner_node
            .get_jstype(compiler)
            .expect("NullPointerException");
        let is_static_member = member.is_static_member(compiler);
        let is_member_field_def = member.is_member_field_def(compiler);
        let is_prototype_member = member.is_member_function_def(compiler)
            || member.is_getter_def(compiler)
            || member.is_setter_def(compiler)
            || member.is_computed_prop(compiler)
            || member.is_computed_field_def(compiler);
        let owner_type = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            owner_js_type
                .to_maybe_function_type(reg)
                .expect("NullPointerException")
        };
        if is_static_member {
            owner_type
        } else if is_member_field_def {
            let (reg, _) = compiler.get_type_registry_and_ast();
            owner_type
                .get_instance_type(reg)
                .expect("NullPointerException")
        } else {
            // Java formats `member` only when the check fails (check_state! is lazy).
            check_state!(is_prototype_member, "%s", member.to_string(compiler));
            let (reg, ast) = compiler.get_type_registry_and_ast();
            owner_type.get_prototype(reg, ast)
        }
    }
} // end ClassScopeBuilder

/// Responsible for defining typed variable "slots".
// port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner
struct SlotDefiner {
    declaration_node: Option<NodeId>,
    variable_name: Option<JsString>,
    scope: Option<TypedScope>,
    // default is no type and a type may be inferred later
    type_: Option<TypeId>,
    allow_later_type_inference: bool,
    for_goog_provided_name: bool,
    // TODO(bradfordcsmith): Once all the logic needed for ES_2017 features has been added,
    //     make the API to this class more restrictive to avoid accidental misuse.
    //     e.g. There will probably always be a declarationNode, so make it a constructor
    //     parameter.
}

impl SlotDefiner {
    // Rust-only: Java's field initializers (`new SlotDefiner()`).
    fn new() -> Self {
        Self {
            declaration_node: None,
            variable_name: None,
            scope: None,
            type_: None,
            allow_later_type_inference: true,
            for_goog_provided_name: false,
        }
    }

    /// @param declarationNode the defining NAME or GETPROP or object literal key node.
    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#forDeclarationNode
    fn for_declaration_node(mut self, declaration_node: Option<NodeId>) -> Self {
        self.declaration_node = declaration_node;
        self
    }

    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#readVariableNameFromDeclarationNode
    fn read_variable_name_from_declaration_node(mut self, compiler: &AbstractCompiler) -> Self {
        // Only qualified name nodes can use this method to get the variable name
        // Object literal keys will have to compute their names themselves.
        // TODO(bradfordcsmith): Clean up these checks of the parent.
        let declaration_node = self.declaration_node.expect("NullPointerException");
        let parent = declaration_node.get_parent(compiler);
        if declaration_node.is_name(compiler) {
            let parent = parent.expect("NullPointerException");
            check_argument!(
                parent.is_function(compiler)
                    || parent.is_class(compiler)
                    || NodeUtil::is_name_declaration(compiler, Some(parent))
                    || parent.is_param_list(compiler)
                    || (parent.is_rest(compiler)
                        && parent
                            .get_parent(compiler)
                            .expect("NullPointerException")
                            .is_param_list(compiler))
                    || parent.is_catch(compiler)
            );
        } else {
            check_argument!(
                declaration_node.is_get_prop(compiler)
                    && (parent.expect("NullPointerException").is_assign(compiler)
                        || parent.unwrap().is_expr_result(compiler))
            );
        }
        self.variable_name = declaration_node.get_qualified_name(compiler);
        self
    }

    // TODO(bradfordcsmith): maybe change to withVariableName(). Need to make these names more
    //     consistent.
    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#forVariableName
    fn for_variable_name(mut self, variable_name: impl Into<JsString>) -> Self {
        self.variable_name = Some(variable_name.into());
        self
    }

    /// Sets the scope in which the variable should be declared.
    ///
    /// If the given name is a qualified name, this scope should be the scope in which the root of
    /// the name is (or will later be) declared.
    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#inScope
    fn in_scope(mut self, scope: TypedScope) -> Self {
        self.scope = Some(scope);
        self
    }

    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#withType
    fn with_type(mut self, type_: Option<TypeId>) -> Self {
        self.type_ = type_;
        self
    }

    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#allowLaterTypeInference
    fn allow_later_type_inference(mut self, allow_later_type_inference: bool) -> Self {
        self.allow_later_type_inference = allow_later_type_inference;
        self
    }

    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#forGoogProvidedName
    fn for_goog_provided_name(mut self) -> Self {
        self.for_goog_provided_name = true;
        self
    }

    /// Define the slot and do related work.
    ///
    /// At minimum the declaration node and variable name must have been set.
    // port: TypedScopeCreator.AbstractScopeBuilder.SlotDefiner#defineSlot
    fn define_slot(self, builder: &mut AbstractScopeBuilder<'_>, compiler: &mut AbstractCompiler) {
        let declaration_node = check_not_null!(self.declaration_node, "declarationNode not set");
        check_state!(
            declaration_node.is_name(compiler)
                || declaration_node.is_get_prop(compiler)
                || NodeUtil::may_be_object_lit_key(compiler, declaration_node)
                || declaration_node.is_module_body(compiler)
                || declaration_node.is_export(compiler)
                || (self.for_goog_provided_name
                    && NodeUtil::is_expr_call(compiler, declaration_node)),
            "declaration node must be an lvalue or goog.provide call, found %s",
            declaration_node.to_string(compiler)
        );
        let variable_name = check_not_null!(self.variable_name.clone(), "variableName not set");
        check_state!(
            self.allow_later_type_inference || self.type_.is_some(),
            "null type but inference not allowed"
        );
        check_state!(!variable_name.is_empty());
        let scope = check_not_null!(self.scope);

        let parent = declaration_node.get_parent(compiler);

        let mut scope_to_declare_in = scope;

        let is_global_var =
            declaration_node.is_name(compiler) && scope_to_declare_in.is_global(compiler);
        let should_declare_on_global_this = (is_global_var
            && (parent.expect("NullPointerException").is_var(compiler)
                || parent.unwrap().is_function(compiler)))
            || (self.for_goog_provided_name && variable_name.index_of_char(u16::from(b'.')) < 0);

        // TODO(sdh): Remove this special case.  It is required to reproduce the original
        // non-block-scoped behavior, which is depended on in several places including
        // https://github.com/angular/tsickle/issues/761.  But it's more correct to always
        // declare on the owner scope.  Once all the bugs are fixed, this should be removed.
        // We may be able to get by with checking a "declared" function's source for jsdoc.
        if scope_to_declare_in != builder.current_hoist_scope
            && scope_to_declare_in.is_global(compiler)
            && scope_to_declare_in.has_own_slot(compiler, variable_name.clone())
            && !self.for_goog_provided_name
        {
            scope_to_declare_in = builder.current_hoist_scope;
        }

        // The input may be null if we are working with a AST snippet. So read
        // the extern info from the node.

        // declared in closest scope?
        let input = builder.get_compiler_input(compiler);
        if !scope_to_declare_in.can_declare(compiler, variable_name.clone()) {
            let old_var = scope_to_declare_in.get_var(compiler, variable_name.clone());
            let validator = builder.creator.validator.clone();
            // Java's sourceName parameter is unused by the method body.
            validator.expect_undeclared_variable(
                compiler,
                builder.source_name.as_deref().unwrap_or_default(),
                input.clone(),
                declaration_node,
                parent.expect("NullPointerException"),
                old_var.expect("NullPointerException"),
                &variable_name.to_string_lossy(),
                self.type_,
            );
        } else {
            if self.type_.is_some() {
                builder
                    .creator
                    .set_deferred_type(compiler, declaration_node, self.type_.unwrap());
            }

            builder.declare(
                compiler,
                scope_to_declare_in,
                variable_name.clone(),
                Some(declaration_node),
                self.type_,
                input.clone(),
                self.allow_later_type_inference,
            );
        }

        // We need to do some additional work for constructors and interfaces.
        let fn_type = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            self.type_.and_then(|t| t.to_maybe_function_type(reg))
        };
        if fn_type.is_some()
            // We don't want to look at empty function types.
            && !{
                let (reg, _) = compiler.get_type_registry_and_ast();
                self.type_.unwrap().is_empty_type(reg)
            }
        {
            // We want to make sure that when we declare a new instance type
            // (with @constructor) that there's actually a ctor for it.
            // This doesn't apply to structural constructors (like
            // function(new:Array). Checking the constructed type against
            // the variable name is a sufficient check for this.
            let is_constructor_or_interface = {
                let (reg, _) = compiler.get_type_registry_and_ast();
                fn_type.unwrap().is_constructor(reg) || fn_type.unwrap().is_interface(reg)
            };
            if is_constructor_or_interface {
                builder.finish_constructor_definition(
                    compiler,
                    declaration_node,
                    &variable_name,
                    fn_type.unwrap(),
                    scope_to_declare_in,
                    input,
                );
            }
        }

        if should_declare_on_global_this {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let global_this = reg.get_native_object_type(JSTypeNative::GLOBAL_THIS);
            if self.allow_later_type_inference {
                let type_ = if self.type_.is_none() {
                    reg.get_native_type(JSTypeNative::NO_TYPE)
                } else {
                    self.type_.unwrap()
                };
                global_this.define_inferred_property(
                    reg,
                    ast,
                    variable_name.clone(),
                    type_,
                    Some(declaration_node),
                );
            } else {
                global_this.define_declared_property(
                    reg,
                    ast,
                    variable_name.clone(),
                    self.type_.expect("NullPointerException"),
                    Some(declaration_node),
                );
            }
        }

        let (reg, ast) = compiler.get_type_registry_and_ast();
        if is_global_var
            && variable_name == "Window"
            && self.type_.is_some()
            && self.type_.unwrap().is_function_type(reg)
            && self.type_.unwrap().is_constructor(reg)
        {
            let global_this_ctor = reg
                .get_native_object_type(JSTypeNative::GLOBAL_THIS)
                .get_constructor(reg)
                .expect("NullPointerException");
            global_this_ctor
                .get_instance_type(reg)
                .expect("NullPointerException")
                .clear_cached_values(reg);
            let prototype = global_this_ctor.get_prototype(reg, ast);
            let instance_type = self
                .type_
                .unwrap()
                .to_maybe_function_type(reg)
                .unwrap()
                .get_instance_type(reg);
            reg.reset_implicit_prototype(prototype, instance_type);
        }
    }
}

impl AbstractScopeBuilder<'_> {
    /// Declares a variable with the given {@code name} and {@code type} on the given {@code
    /// scope}, returning the newly-declared {@link TypedVar}. Additionally checks the {@link
    /// #escapedVarNames} and {@link #assignedVarNames} maps (which were populated during the {@link
    /// FirstOrderFunctionAnalyzer} and marks the result as escaped or assigned exactly once if
    /// appropriate.
    // port: TypedScopeCreator.AbstractScopeBuilder#declare
    #[allow(clippy::too_many_arguments)]
    fn declare(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope: TypedScope,
        name: JsString,
        n: Option<NodeId>,
        type_: Option<TypeId>,
        input: Option<CompilerInput>,
        inferred: bool,
    ) -> TypedVar {
        let var = scope.declare(compiler, name.clone(), n, type_, input, inferred);
        let scoped_name = <dyn ScopedName>::of(name, Some(scope.get_root_node(compiler)));
        if self.creator.escaped_var_names.contains(&scoped_name) {
            var.mark_escaped(compiler);
        }
        if self
            .creator
            .assigned_var_names
            .get(&scoped_name)
            .copied()
            .unwrap_or(0)
            == 1
        {
            var.mark_assigned_exactly_once(compiler);
        }
        // assignedVarNames.remove(scopedName): Multiset#remove removes one occurrence.
        if let Some(count) = self.creator.assigned_var_names.get_mut(&scoped_name) {
            *count -= 1;
            if *count == 0 {
                // Never iterated: O(1) swap_remove instead of Java-order shift_remove.
                self.creator.assigned_var_names.swap_remove(&scoped_name);
            }
        } // free up memory
        var
    }

    // port: TypedScopeCreator.AbstractScopeBuilder#finishConstructorDefinition
    fn finish_constructor_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        declaration_node: NodeId,
        variable_name: &JsString,
        fn_type: TypeId,
        scope_to_declare_in: TypedScope,
        input: Option<CompilerInput>,
    ) {
        // Declare var.prototype in the scope chain.
        let (super_class_ctor, prototype_slot) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let super_class_ctor = fn_type.get_super_class_constructor(reg, ast);
            let prototype_slot = fn_type.get_slot(reg, ast, "prototype");
            (super_class_ctor, prototype_slot)
        };

        let prototype_name = variable_name.concat(&JsString::from(".prototype"));

        // There are some rare cases where the prototype will already
        // be declared. See TypedScopeCreatorTest#testBogusPrototypeInit.
        // Fortunately, other warnings will complain if this happens.
        let prototype_var = scope_to_declare_in.get_var(compiler, prototype_name.clone());
        if prototype_var.is_some()
            && prototype_var.unwrap().get_scope(compiler) == scope_to_declare_in
        {
            scope_to_declare_in.undeclare(compiler, prototype_var.unwrap());
        }

        let (prototype_type, inferred) = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let prototype_type = prototype_slot.expect("NullPointerException").get_type(reg);
            // declared iff there's an explicit supertype
            let inferred = super_class_ctor.is_none() || {
                let object_type = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
                super_class_ctor
                    .unwrap()
                    .get_instance_type(reg)
                    .expect("NullPointerException")
                    .equals(reg, ast, object_type)
            };
            (prototype_type, inferred)
        };
        scope_to_declare_in.declare(
            compiler,
            prototype_name,
            Some(declaration_node),
            Some(prototype_type),
            input,
            inferred,
        );
    }
}

/// Does a first-order function analysis that just looks at simple things like what variables are
/// escaped, and whether 'this' is used.
///
/// The syntactic scopes created in this traversal are also stored for later use.
// port: TypedScopeCreator.FirstOrderFunctionAnalyzer
struct FirstOrderFunctionAnalyzer<'a> {
    creator: &'a mut TypedScopeCreator,
}

impl<'a> FirstOrderFunctionAnalyzer<'a> {
    // Rust-only: Java's inner class reads the enclosing TypedScopeCreator's fields.
    fn new(creator: &'a mut TypedScopeCreator) -> Self {
        Self { creator }
    }
}

impl ScopedCallback for FirstOrderFunctionAnalyzer<'_> {
    // port: TypedScopeCreator.FirstOrderFunctionAnalyzer#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let root = scope.get_root_node(compiler);
        for symbol in scope.get_var_iterable(compiler) {
            self.creator
                .reserved_names_for_scope
                .entry(root)
                .or_default()
                .push(symbol.get_name(compiler));
        }
    }

    // port: NodeTraversal.AbstractScopedCallback#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}

impl Callback for FirstOrderFunctionAnalyzer<'_> {
    // port: NodeTraversal.AbstractScopedCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: TypedScopeCreator.FirstOrderFunctionAnalyzer#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if t.in_global_scope() {
            // The first-order function analyzer looks at two types of variables:
            //
            // 1) Local variables that are assigned in inner scopes ("escaped vars")
            //
            // 2) Local variables that are assigned more than once.
            //
            // We treat all global variables as escaped by default, so there's
            // no reason to do this extra computation for them.
            return;
        }

        // (Scope) t.getClosestContainerScope()
        let container_scope = match t.get_closest_container_scope() {
            Some(AbstractScopeHandle::Untyped(scope)) => scope,
            Some(AbstractScopeHandle::Typed(_)) => {
                panic!("ClassCastException: TypedScope cannot be cast to Scope")
            }
            None => panic!("NullPointerException"),
        };

        // Record function with returns or arrow functions without bodies
        if (n.is_return(t) && n.has_children(t))
            || NodeUtil::is_blockless_arrow_function_result(t, n)
        {
            let root = container_scope.get_root_node(t.get_compiler());
            self.creator.functions_with_non_empty_returns.insert(root);
        }

        // Be careful of bleeding functions, which create variables
        // in the inner scope, not the scope where the name appears.
        if n.is_name(t) && NodeUtil::is_l_value(t, n) && !NodeUtil::is_bleeding_function_name(t, n)
        {
            let name = n.get_string(t);
            let scope = t.get_scope();
            let compiler = t.get_compiler();
            let var = scope.get_var(compiler, name.clone());
            // TODO(sdh): consider checking hasSameHoistScope instead of container scope here and
            // below. This will detect function(a) { a.foo = bar } as an escaped qualified name,
            // which seems like the right thing to do (but could possibly break things?)
            // Doing so will allow removing the warning on TypeCheckTest#testIssue1024b.
            if let Some(var) = var {
                let owner_scope = var.get_scope(compiler);
                if owner_scope.is_local(compiler) {
                    let scoped_name =
                        <dyn ScopedName>::of(name, Some(owner_scope.get_root_node(compiler)));
                    *self
                        .creator
                        .assigned_var_names
                        .entry(scoped_name.clone())
                        .or_insert(0) += 1;
                    if !container_scope.has_same_container_scope(compiler, owner_scope) {
                        self.creator.escaped_var_names.insert(scoped_name);
                    }
                }
            }
        } else if n.is_get_prop(t) && n.is_unscoped_qualified_name(t) && NodeUtil::is_l_value(t, n)
        {
            let name = NodeUtil::get_root_of_qualified_name(t, n).get_string(t);
            let scope = t.get_scope();
            let compiler = t.get_compiler();
            let var = scope.get_var(compiler, name);
            if let Some(var) = var {
                let owner_scope = var.get_scope(compiler);
                if owner_scope.is_local(compiler)
                    && !container_scope.has_same_container_scope(compiler, owner_scope)
                {
                    self.creator.escaped_var_names.insert(<dyn ScopedName>::of(
                        n.get_qualified_name(compiler)
                            .expect("NullPointerException"),
                        Some(owner_scope.get_root_node(compiler)),
                    ));
                }
            }
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

/// Symbols that are defined in the language spec.
///
/// NOTE: adding a symbol to this list is necessary only to make it available in Clutz output. By
/// default, JSCompiler will note any Symbol.* references in externs whether or not they are in
/// this list, but Clutz runs without including all externs so relies on this list to know what is
/// / is not a well-known symbol.
// port: TypedScopeCreator#WELL_KNOWN_SYMBOLS
const WELL_KNOWN_SYMBOLS: [&str; 12] = [
    // go/keep-sorted start
    "Symbol.asyncDispose",
    "Symbol.asyncIterator",
    "Symbol.dispose",
    "Symbol.hasInstance",
    "Symbol.isConcatSpreadable",
    "Symbol.iterator",
    "Symbol.match",
    "Symbol.replace",
    "Symbol.species",
    "Symbol.toPrimitive",
    "Symbol.toStringTag",
    "Symbol.unscopables",
    // go/keep-sorted end
];

/// Adds all enums and typedefs to the registry's list of non-nullable types.
///
/// The Java `registry` field is reached through the compiler (DESIGN.md section 6).
// port: TypedScopeCreator.IdentifyEnumsAndTypedefsAsNonNullable
struct IdentifyEnumsAndTypedefsAsNonNullable;

impl IdentifyEnumsAndTypedefsAsNonNullable {
    // port: TypedScopeCreator.IdentifyEnumsAndTypedefsAsNonNullable#IdentifyEnumsAndTypedefsAsNonNullable
    fn new() -> Self {
        Self
    }

    // port: TypedScopeCreator.IdentifyEnumsAndTypedefsAsNonNullable#identifyEnumOrTypedefDeclaration
    fn identify_enum_or_typedef_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        name_node: NodeId,
        rvalue: Option<NodeId>,
        info: Option<&JSDocInfo>,
    ) {
        if !name_node.is_qualified_name(t) {
            return;
        }
        if info.is_some() && info.unwrap().has_enum_parameter_type() {
            let scope = t.get_scope();
            let qualified_name = name_node.get_qualified_name(t).unwrap();
            let compiler = t.get_compiler();
            let scope = as_static_scope(compiler, scope);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            reg.identify_non_nullable_name(ast, Some(&*scope), qualified_name);
        } else if info.is_some() && info.unwrap().has_typedef_type() {
            let scope = t.get_scope();
            let qualified_name = name_node.get_qualified_name(t).unwrap();
            let compiler = t.get_compiler();
            let scope = as_static_scope(compiler, scope);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            reg.identify_non_nullable_name(ast, Some(&*scope), qualified_name);
        } else if rvalue.is_some()
            && rvalue.unwrap().is_qualified_name(t)
            && {
                let scope = t.get_scope();
                let rvalue_name = rvalue.unwrap().get_qualified_name(t).unwrap();
                let compiler = t.get_compiler();
                let scope = as_static_scope(compiler, scope);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                reg.is_non_nullable_name(ast, Some(&*scope), rvalue_name)
            }
            && NodeUtil::is_constant_declaration(t, info, name_node)
        {
            let scope = t.get_scope();
            let qualified_name = name_node.get_qualified_name(t).unwrap();
            let compiler = t.get_compiler();
            let scope = as_static_scope(compiler, scope);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            reg.identify_non_nullable_name(ast, Some(&*scope), qualified_name);
        }
    }
}

impl Callback for IdentifyEnumsAndTypedefsAsNonNullable {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: TypedScopeCreator.IdentifyEnumsAndTypedefsAsNonNullable#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, node: NodeId, _parent: Option<NodeId>) {
        use closure_rhino::token::Token;
        match node.get_token(t) {
            Token::LET | Token::CONST | Token::VAR => {
                let mut child = node.get_first_child(t);
                while let Some(c) = child {
                    // TODO(b/116853368): make this work for destructuring aliases as well.
                    let rvalue = c.get_first_child(t);
                    let info = NodeUtil::get_best_jsdoc_info(t, c);
                    self.identify_enum_or_typedef_declaration(t, c, rvalue, info.as_deref());
                    child = c.get_next(t);
                }
            }
            Token::EXPR_RESULT => {
                let first_child = node.get_first_child(t).unwrap();
                if first_child.is_assign(t) {
                    let assign = first_child;
                    let lhs = assign.get_first_child(t).unwrap();
                    let rhs = assign.get_second_child(t);
                    let info = assign.get_jsdoc_info(t);
                    self.identify_enum_or_typedef_declaration(t, lhs, rhs, info.as_deref());
                } else if first_child.is_get_prop(t) {
                    let info = first_child.get_jsdoc_info(t);
                    self.identify_enum_or_typedef_declaration(
                        t,
                        first_child,
                        /* rvalue= */ None,
                        info.as_deref(),
                    );
                }
            }
            _ => {}
        }
    }
}

impl TypedScopeCreator {
    // port: TypedScopeCreator#containingGoogModuleIdOf
    pub(crate) fn containing_goog_module_id_of(
        compiler: &AbstractCompiler,
        scope: TypedScope,
    ) -> Option<JsString> {
        let Some(module) = scope.get_module(compiler) else {
            let parent = scope.get_parent(compiler);
            return parent.and_then(|parent| Self::containing_goog_module_id_of(compiler, parent));
        };

        // Stop recursing once we've hit a module scope.
        let metadata = module.metadata();
        check_state!(metadata.is_module(), "%s", metadata.to_string(compiler));

        /*
         * This module may not have a goog.module/goog.declareModuleId. Also don't crash if it's
         * malformed with multiple module IDs.
         */
        metadata.goog_namespaces().iter().next().cloned()
    }
}

// port: TypedScopeCreator#extractKnownSymbolKey
fn extract_known_symbol_key(
    compiler: &mut AbstractCompiler,
    current_scope: TypedScope,
    key: NodeId,
) -> Option<TypeId> {
    if !key.is_qualified_name(compiler) {
        return None;
    }
    let qname = key.get_qualified_name_object(compiler).unwrap();
    let type_ = lookup_qualified_name(compiler, current_scope, &qname);
    let (reg, _) = compiler.get_type_registry_and_ast();
    if type_.is_some() {
        type_.unwrap().to_maybe_known_symbol_type(reg)
    } else {
        None
    }
}

// StaticTypedScope#lookupQualifiedName, the default method TypedScope inherits from Closure's
// Rhino-derived StaticTypedScope (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "typed_scope_creator_rhino.rs"]
mod rhino;
use rhino::lookup_qualified_name;
