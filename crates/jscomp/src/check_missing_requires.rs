/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckMissingRequires.java.

//! Checks that references to Closure namespaces (goog.provide, goog.module and
//! goog.module.declareLegacyNamespace) are satisfied by a goog.require or goog.requireType in the
//! referencing file.
#![allow(clippy::collapsible_if)] // Keep Java's control flow.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    modules::module_metadata_map::{ModuleMetadata, ModuleMetadataMap},
    node_traversal::{AbstractModuleCallback, ModuleCallback, NodeTraversal},
    node_util::{GoogRequire, NodeUtil},
    var::VarId,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_state,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::{Arc, LazyLock};

// port: CheckMissingRequires#MISSING_REQUIRE
pub static MISSING_REQUIRE: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_REQUIRE",
    "''{0}'' references a fully qualified namespace, which is disallowed by the style guide.\nPlease add a goog.require, assign or destructure it into an alias, and use the alias instead.\nNamespace defined in {1}.",
);

// port: CheckMissingRequires#MISSING_REQUIRE_TYPE
pub static MISSING_REQUIRE_TYPE: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_REQUIRE_TYPE",
    "''{0}'' references a fully qualified namespace, which is disallowed by the style guide.\nPlease add a goog.requireType, assign or destructure it into an alias, and use the alias instead.\nNamespace defined in {1}.",
);

// port: CheckMissingRequires#INCORRECT_NAMESPACE_ALIAS_REQUIRE
pub static INCORRECT_NAMESPACE_ALIAS_REQUIRE: DiagnosticType = DiagnosticType::disabled(
    "JSC_INCORRECT_NAMESPACE_ALIAS_REQUIRE",
    "''{0}'' is its own namespace.\nPlease add a separate goog.require and use that alias instead.",
);

// port: CheckMissingRequires#INCORRECT_NAMESPACE_ALIAS_REQUIRE_TYPE
pub static INCORRECT_NAMESPACE_ALIAS_REQUIRE_TYPE: DiagnosticType = DiagnosticType::disabled(
    "JSC_INCORRECT_NAMESPACE_ALIAS_REQUIRE_TYPE",
    "''{0}'' is its own namespace.\nPlease add a separate goog.requireType and use that alias instead.",
);

// port: CheckMissingRequires#INDIRECT_NAMESPACE_REF_REQUIRE
pub static INDIRECT_NAMESPACE_REF_REQUIRE: DiagnosticType = DiagnosticType::disabled(
    "JSC_INDIRECT_NAMESPACE_REF_REQUIRE",
    "''{0}'' should have its own goog.require.\nPlease add a separate goog.require and use that alias instead.",
);

// port: CheckMissingRequires#INDIRECT_NAMESPACE_REF_REQUIRE_TYPE
pub static INDIRECT_NAMESPACE_REF_REQUIRE_TYPE: DiagnosticType = DiagnosticType::disabled(
    "JSC_INDIRECT_NAMESPACE_REF_REQUIRE_TYPE",
    "''{0}'' should have its own goog.requireType.\nPlease add a separate goog.requireType and use that alias instead.",
);

// port: CheckMissingRequires#MISSING_REQUIRE_IN_PROVIDES_FILE
pub static MISSING_REQUIRE_IN_PROVIDES_FILE: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_REQUIRE_IN_PROVIDES_FILE",
    "''{0}'' references a namespace which was not required by this file.\nPlease add a goog.require.\nNamespace defined in {1}.",
);

// port: CheckMissingRequires#MISSING_REQUIRE_TYPE_IN_PROVIDES_FILE
pub static MISSING_REQUIRE_TYPE_IN_PROVIDES_FILE: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_REQUIRE_TYPE_IN_PROVIDES_FILE",
    "''{0}'' references a namespace which was not required by this file.\nPlease add a goog.requireType.\nNamespace defined in {1}.",
);

// port: CheckMissingRequires#NON_LEGACY_GOOG_MODULE_REFERENCE
pub static NON_LEGACY_GOOG_MODULE_REFERENCE: DiagnosticType = DiagnosticType::error(
    "JSC_NON_LEGACY_GOOG_MODULE_REFERENCE",
    "''{0}'' references the name of a module without goog.declareLegacyNamespace(), which is not actually defined. Use goog.module.get() instead.",
);

// port: CheckMissingRequires#MISSING_REQUIRE_IN_GOOG_SCOPE
pub static MISSING_REQUIRE_IN_GOOG_SCOPE: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_REQUIRE_IN_GOOG_SCOPE",
    "''{0}'' is its own namespace. Please add a goog.require and reference that alias.",
);

// port: CheckMissingRequires#MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE
pub static MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE ",
    "''{0}'' is its own namespace. Please add a goog.requireType and reference that alias.",
);

// port: CheckMissingRequires#MISSING_REQUIRE_FOR_GOOG_MODULE_GET
pub static MISSING_REQUIRE_FOR_GOOG_MODULE_GET: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_REQUIRE_FOR_GOOG_MODULE_GET",
    "''{0}'' references a namespace which was not required by this file.\nPlease add a goog.require.",
);

/// Checks that references to Closure namespaces are satisfied by a require.
pub struct CheckMissingRequires {
    module_metadata_map: Arc<ModuleMetadataMap>,
    /// The set of template parameter names found so far in the file currently being checked.
    template_param_names: IndexSet<JsString>,
    /// The mapping from Closure namespace into the module that provides it.
    module_by_namespace: IndexMap<JsString, Arc<ModuleMetadata>>,
    /// Tracks how many "control flow scopes" we've entered, starting at 0
    ///
    /// Where a "control flow scope" is here defined as any scope /except/ for either the global
    /// control flow scope, or an IIFE or goog.scope body within the global control flow scope.
    control_flow_scope_depth: i32,
}

// port: CheckMissingRequires#GOOG_SCOPE
static GOOG_SCOPE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.scope"));

impl CheckMissingRequires {
    // port: CheckMissingRequires#CheckMissingRequires
    pub fn new(_compiler: &AbstractCompiler, module_metadata_map: Arc<ModuleMetadataMap>) -> Self {
        let module_by_namespace = module_metadata_map.get_modules_by_goog_namespace().clone();
        Self {
            module_metadata_map,
            template_param_names: IndexSet::<_>::default(),
            module_by_namespace,
            control_flow_scope_depth: 0,
        }
    }

    /// Takes a valid "control flow root", as defined by `NodeUtil#isValidCfgRoot`, and returns
    /// whether it creates a new "control flow scope" from the parent scope (the global scope).
    // port: CheckMissingRequires#isInGlobalControlFlowScope
    fn is_in_global_control_flow_scope(&self, ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::SCRIPT | Token::ROOT => true,
            Token::MODULE_BODY | Token::BLOCK =>
            // class static blocks need to create a new cfg root
            {
                false
            }
            Token::FUNCTION => {
                if self.control_flow_scope_depth > 0 {
                    return false;
                }
                // Check for functions invoked immediately within the global scope - so IIFEs or
                // goog.scope callees.
                if Self::is_goog_scope_body(ast, NodeUtil::get_function_body(ast, n)) {
                    return true;
                }
                Self::is_iife(ast, n)
            }
            _ => panic!("Unexpected control flow root: {}", n.to_string(ast)),
        }
    }

    // port: CheckMissingRequires#isIIFE
    fn is_iife(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast)
            && n.get_parent(ast).unwrap().is_call(ast)
            && n.is_first_child_of(ast, n.get_parent(ast))
    }

    // port: CheckMissingRequires#visitNode
    fn visit_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_module: &ModuleMetadata,
    ) {
        let info = n.get_jsdoc_info(t);
        if let Some(info) = info {
            self.visit_js_doc_info(t, current_module, &info);
        }
        if !n.get_parent(t).unwrap().is_get_prop(t) && n.is_qualified_name(t) {
            let qualified_name = n.get_qualified_name_object(t).unwrap();
            let root = qualified_name.get_root(t);
            if root == "this" || root == "super" {
                return;
            }
            self.visit_qualified_name(t, n, current_module, &qualified_name, Strength::CODE);
        }

        if n.is_name(t) && !n.get_string_ref(t).is_empty() {
            self.visit_maybe_declaration(t, n, current_module);
        }

        if n.is_call(t) && self.control_flow_scope_depth == 0 {
            self.visit_maybe_goog_module_get(t, n, current_module);
        }
    }

    // port: CheckMissingRequires#visitJsDocInfo
    fn visit_js_doc_info(
        &mut self,
        t: &mut NodeTraversal<'_>,
        current_module: &ModuleMetadata,
        info: &JSDocInfo,
    ) {
        // Collect template parameter names before checking, so that annotations on the same node
        // that reference the name are excluded from the check.
        self.template_param_names
            .extend(info.get_template_type_names());
        self.template_param_names
            .extend(info.get_type_transformations().into_keys());
        if info.has_type() {
            self.visit_js_doc_expr(
                t,
                current_module,
                &info.get_type().unwrap(),
                Strength::WEAK_TYPE,
            );
        }
        for param in info.get_parameter_names() {
            if info.has_parameter_type(param.clone()) {
                self.visit_js_doc_expr(
                    t,
                    current_module,
                    &info.get_parameter_type(param).unwrap(),
                    Strength::WEAK_TYPE,
                );
            }
        }
        if info.has_return_type() {
            self.visit_js_doc_expr(
                t,
                current_module,
                &info.get_return_type().unwrap(),
                Strength::WEAK_TYPE,
            );
        }
        if info.has_enum_parameter_type() {
            self.visit_js_doc_expr(
                t,
                current_module,
                &info.get_enum_parameter_type().unwrap(),
                Strength::WEAK_TYPE,
            );
        }
        if info.has_typedef_type() {
            self.visit_js_doc_expr(
                t,
                current_module,
                &info.get_typedef_type().unwrap(),
                Strength::WEAK_TYPE,
            );
        }
        if info.has_this_type() {
            self.visit_js_doc_expr(
                t,
                current_module,
                &info.get_this_type().unwrap(),
                Strength::WEAK_TYPE,
            );
        }
        if info.has_base_type() {
            // Note that `@extends` requires a goog.require, not a goog.requireType.
            self.visit_js_doc_expr(
                t,
                current_module,
                &info.get_base_type().unwrap(),
                Strength::IMPLEMENTS_EXTENDS,
            );
        }
        for expr in info.get_extended_interfaces() {
            // Note that `@extends` requires a goog.require, not a goog.requireType.
            self.visit_js_doc_expr(t, current_module, &expr, Strength::IMPLEMENTS_EXTENDS);
        }
        for expr in info.get_implemented_interfaces() {
            // Note that `@implements` requires a goog.require, not a goog.requireType.
            self.visit_js_doc_expr(t, current_module, &expr, Strength::IMPLEMENTS_EXTENDS);
        }
    }

    // port: CheckMissingRequires#visitJsDocExpr
    fn visit_js_doc_expr(
        &mut self,
        t: &mut NodeTraversal<'_>,
        current_module: &ModuleMetadata,
        expr: &JSTypeExpression,
        reference_strength: Strength,
    ) {
        for type_node in expr.get_all_type_nodes(t) {
            let qualified_name = QualifiedName::of(type_node.get_string(t));
            self.visit_qualified_name(
                t,
                type_node,
                current_module,
                &qualified_name,
                reference_strength,
            );
        }
    }

    // port: CheckMissingRequires#isGoogScopeBody
    fn is_goog_scope_body(ast: &Ast, hoist_scope_root: NodeId) -> bool {
        hoist_scope_root.is_block(ast)
            && hoist_scope_root.get_parent(ast).unwrap().is_function(ast)
            && hoist_scope_root.get_grandparent(ast).unwrap().is_call(ast)
            && GOOG_SCOPE.matches(
                ast,
                hoist_scope_root
                    .get_grandparent(ast)
                    .unwrap()
                    .get_first_child(ast)
                    .unwrap(),
            )
    }

    /// Check for invalid goog.module.get calls executed on script load
    ///
    /// We don't check for goog.module.get within function bodies though (except for goog.scope
    /// since that's basically an IIFE) - it's possible that the module will have been loaded by the
    /// time the goog.module.get is called even though there's no strong require. It's up to the
    /// caller to ensure that it's loaded.
    // port: CheckMissingRequires#visitMaybeGoogModuleGet
    fn visit_maybe_goog_module_get(
        &mut self,
        t: &mut NodeTraversal<'_>,
        goog_module_get: NodeId,
        current_file: &ModuleMetadata,
    ) {
        check_state!(goog_module_get.is_call(t));
        if !NodeUtil::is_goog_module_get_call(t, goog_module_get) {
            return;
        }
        let imported_namespace = goog_module_get.get_second_child(t).unwrap().get_string(t);
        let Some(required_file) = self.module_by_namespace.get(&imported_namespace).cloned() else {
            // goog.module.get of a non-existing namespace is an error in another pass.
            return;
        };
        if !Self::has_acceptable_require(
            t,
            current_file,
            &QualifiedName::of(imported_namespace.clone()),
            &required_file,
            Strength::WEAK_TYPE,
        ) {
            t.report(
                goog_module_get,
                &MISSING_REQUIRE_FOR_GOOG_MODULE_GET,
                &[&imported_namespace.to_string()],
            );
        }
    }

    // port: CheckMissingRequires#visitMaybeDeclaration
    fn visit_maybe_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_file: &ModuleMetadata,
    ) {
        if !current_file.is_module() {
            // This check only makes sense in goog.module files
            return;
        }

        // Currently, tsickle can introduce these
        // TODO(b/333952917): Remove this once tsickle is fixed.
        if Self::is_type_script_source(t, n) {
            return;
        }

        check_state!(n.is_name(t));
        let root_name = n.get_string(t);
        let scope = t.get_scope();
        let Some(var) = scope.get_var(t.get_compiler(), root_name) else {
            return;
        };
        let declaration = var.get_declaration(t.get_compiler());
        match declaration {
            Some(declaration) if declaration.get_node(t.get_compiler()) == Some(n) => {}
            _ => {
                // Not a declaration reference
                return;
            }
        }

        if var.get_scope(t.get_compiler()).is_local(t.get_compiler()) {
            let Some(require) = NodeUtil::get_goog_require_info_var(t.get_compiler(), var) else {
                // If it refers to a local variable, it's legal.
                return;
            };

            // Not destructured.
            let Some(property) = require.property() else {
                return;
            };

            let required_file = self.module_by_namespace.get(&require.namespace()).cloned();
            let Some(required_file) =
                required_file.filter(|required_file| required_file.has_legacy_goog_namespaces())
            else {
                // If we are importing from a non-legacy namespace than any property reference is
                // valid, as it is assumed that the property reference is exported intentionally.

                // NOTE There are corner cases if a legacy namespace object is explicitly reexported
                // but (1) it isn't clear what the "correct" behavior is and (2) we don't have the
                // information here in order to validate it as we would need to the object shapes.
                return;
            };

            let reference = require
                .namespace()
                .concat(&JsString::from("."))
                .concat(&property);

            // Do not report references to a namespace provided in the same file, and do not recurse
            // into parent namespaces either.
            if !current_file.goog_namespaces().contains(&reference) {
                // Try to find an alternate file to import from if, if we construct a namespace
                // using the destructured property name.
                let alternate_file = self.module_by_namespace.get(&reference);

                if let Some(alternate_file) = alternate_file
                    && !Arc::ptr_eq(alternate_file, &required_file)
                {
                    // TODO: report on the node that needs to be removed, include the namespace
                    // that needs to be added.
                    let to_report = if require.is_strong_require() {
                        &INCORRECT_NAMESPACE_ALIAS_REQUIRE
                    } else {
                        &INCORRECT_NAMESPACE_ALIAS_REQUIRE_TYPE
                    };
                    t.report(n, to_report, &[&reference.to_string()]);
                }
            }
        }
    }

    // port: CheckMissingRequires#isTypeScriptSource
    fn is_type_script_source(ast: &Ast, n: NodeId) -> bool {
        n.get_static_source_file(ast)
            .unwrap()
            .is_type_script_source()
    }

    // port: CheckMissingRequires#visitQualifiedName
    fn visit_qualified_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_file: &ModuleMetadata,
        qualified_name: &QualifiedName,
        reference_strength: Strength,
    ) {
        let root_name = qualified_name.get_root(t);
        if qualified_name.is_simple(t) {
            if self.template_param_names.contains(&root_name) {
                // This will produce a false negative when the same name is used in both template
                // and non-template capacity in the same file, and a false positive when the
                // `@template` does not precede the reference within the same source file (e.g. an
                // ES5 ctor in a different file).
                return;
            }
        }

        if NodeUtil::is_declaration_l_value(t, n) {
            // NOTE: maybe do something here.
            return;
        }

        let scope = t.get_scope();
        let var = scope.get_var(t.get_compiler(), root_name);
        match var {
            Some(var) if var.get_scope(t.get_compiler()).is_local(t.get_compiler()) => {
                self.check_missing_require_through_short_name(
                    t,
                    n,
                    var,
                    current_file,
                    qualified_name,
                    reference_strength,
                );
            }
            _ => {
                self.check_missing_require_through_fully_qualified_name(
                    t,
                    n,
                    current_file,
                    qualified_name,
                    reference_strength,
                );
            }
        }
    }

    /// Checks if a reference to a local variable should actually trigger a missing require warning.
    ///
    /// This only warns within goog.scope and module bodies, in cases where there's a local variable
    /// that's actually an alias of some import.
    // port: CheckMissingRequires#checkMissingRequireThroughShortName
    fn check_missing_require_through_short_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        local_var: VarId,
        current_file: &ModuleMetadata,
        qualified_name: &QualifiedName,
        reference_strength: Strength,
    ) {
        // TODO(b/333952917): Remove this once tsickle is fixed.
        if Self::is_type_script_source(t, n) {
            return;
        }

        let in_module = current_file.is_module();
        let in_goog_scope = !in_module && {
            let scope_root = local_var.get_scope_root(t.get_compiler());
            Self::is_goog_scope_body(t, scope_root)
        };

        if !in_module && !in_goog_scope {
            // Don't worry about aliases outside of module files and goog.scopes for now.
            return;
        }

        // The qualified name *is* the root name if it is "simple"
        if qualified_name.is_simple(t) {
            // It is explicitly imported and we have already validated the import in
            // `visitMaybeDeclaration`
            return;
        }

        if in_goog_scope {
            self.check_missing_require_through_short_name_in_goog_scope(
                t,
                n,
                local_var,
                current_file,
                qualified_name,
                reference_strength,
            );
        } else {
            let require = NodeUtil::get_goog_require_info_var(t.get_compiler(), local_var);
            self.check_missing_require_through_short_name_in_module(
                t,
                n,
                require.as_ref(),
                current_file,
                qualified_name,
                reference_strength,
            );
        }
    }

    // port: CheckMissingRequires#checkMissingRequireThroughShortNameInGoogScope
    fn check_missing_require_through_short_name_in_goog_scope(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        local_var: VarId,
        current_file: &ModuleMetadata,
        qualified_name: &QualifiedName,
        reference_strength: Strength,
    ) {
        let Some(mut initial_value) = local_var.get_initial_value(t.get_compiler()) else {
            return;
        };
        // There are two cases we have to handle (assume 'foo.bar' is a module or provide, below);
        //  1) a legacy namespace, possibly with extra properties:
        //        const alias = foo.bar;
        //        const alias = foo.bar.baz;
        //  2) a goog.module.get, possibly with extra properties:
        //      const alias = goog.module.get('foo.bar');
        //      const alias = goog.module.get('foo.bar').baz;
        // Ignore any other initial value. This pass isn't designed to check arbitrary chains of
        // import aliases, and we just look at direct aliases of Closure namespaces. For example, we
        // will not find aliases such as:
        //    alias.something = foo.bar;
        // This aligns with the behavior of the ScopedAliases pass goog.scope handling.

        // Case (1) - the initial value is a legacy namespace + potential extra properties
        if initial_value.is_qualified_name(t) {
            let aliased_name = initial_value.get_qualified_name_object(t).unwrap();
            let normalized_qualified_name =
                Self::swap_root_of_qualified_name(t, qualified_name, &aliased_name);

            // Find longest prefix match that is actually a provide/legacy namespace, if any.
            let mut original_import: Option<Arc<ModuleMetadata>> = None;
            let mut sub_name = Some(aliased_name);
            while let Some(current) = sub_name {
                let imported = self.module_by_namespace.get(&current.join(t));
                if let Some(imported) = imported {
                    original_import = Some(imported.clone());
                    break;
                }
                sub_name = current.get_owner(t);
            }
            self.check_normalized_short_name_import(
                t,
                n,
                current_file,
                original_import.as_ref(),
                qualified_name,
                &normalized_qualified_name,
                reference_strength,
            );
            return;
        }

        // Case (2) - the initial value is a goog.module.get + potential extra properties
        while initial_value.is_get_prop(t) {
            initial_value = initial_value.get_first_child(t).unwrap();
        }
        if !NodeUtil::is_goog_module_get_call(t, initial_value) {
            return;
        }
        let imported_namespace = initial_value.get_second_child(t).unwrap().get_string(t);
        let original_import = self.module_by_namespace.get(&imported_namespace).cloned();
        if let Some(original_import) = &original_import
            && !original_import.has_legacy_goog_namespaces()
        {
            // We trust the import from a non-legacy module, as they should not overlap.
            return;
        }
        let mut aliased_name = QualifiedName::of(imported_namespace);
        let mut prop = initial_value.get_parent(t).unwrap();
        while prop.is_get_prop(t) {
            aliased_name = aliased_name.getprop(prop.get_string(t));
            prop = prop.get_parent(t).unwrap();
        }
        let normalized_qualified_name =
            Self::swap_root_of_qualified_name(t, qualified_name, &aliased_name);
        self.check_normalized_short_name_import(
            t,
            n,
            current_file,
            original_import.as_ref(),
            qualified_name,
            &normalized_qualified_name,
            reference_strength,
        );
    }

    // port: CheckMissingRequires#checkMissingRequireThroughShortNameInModule
    fn check_missing_require_through_short_name_in_module(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        require: Option<&GoogRequire>,
        current_file: &ModuleMetadata,
        qualified_name: &QualifiedName,
        reference_strength: Strength,
    ) {
        let Some(require) = require else {
            // It is a local name, not an import.
            // NOTE: this *could be a local alias* of an imported name
            //   but at some point the only real fix is to not use `goog.provide`
            //   or `goog.module.declareLegacyNamespace` because it is always
            //   possible to obfuscate the use of the namespace.
            return;
        };

        let original_required_file = self.module_by_namespace.get(&require.namespace()).cloned();
        let Some(original_required_file) = original_required_file
            .filter(|original_required_file| original_required_file.has_legacy_goog_namespaces())
        else {
            // We trust the import from a non-legacy module, as they should not overlap.
            return;
        };

        // Verify namespace usage
        let normalize_qualified_name =
            Self::normalize_qualified_name_plus_import(t, qualified_name, require);

        self.check_normalized_short_name_import(
            t,
            n,
            current_file,
            Some(&original_required_file),
            qualified_name,
            &normalize_qualified_name,
            reference_strength,
        );
    }

    // port: CheckMissingRequires#checkNormalizedShortNameImport
    #[allow(clippy::too_many_arguments)] // Java's parameter list.
    fn check_normalized_short_name_import(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_file: &ModuleMetadata,
        original_import: Option<&Arc<ModuleMetadata>>,
        original_qualified_name: &QualifiedName,
        normalized_qualified_name: &QualifiedName,
        reference_strength: Strength,
    ) {
        // TESTCASES, where A.B should not be used through A:
        // checked here
        // X  const A = require('A'); ref(A.B);  // error, missing require A.B
        // X  const A = require('A'); const B = require('B');   ref(A.B);  // error, incorrect ref
        // checked in declaration
        // X  const {B} = require('A');  // error bad import
        //

        // Look for the longest prefix match against a provided namespace.
        let mut sub_name = Some(normalized_qualified_name.clone());
        while let Some(current) = sub_name {
            let namespace = current.join(t);
            if self.is_allowed_namespace(current_file, &namespace) {
                return;
            }

            let Some(required_file) = self.module_by_namespace.get(&namespace).cloned() else {
                // Not a known namespace check the parent
                sub_name = current.get_owner(t);
                continue;
            };

            // TODO: report on the node that needs rewritten, include the namespace that needs
            // to be added:
            // Autofix: add the require if necessary, rewrite to be the full namespace, and let
            // fixjs fix
            // it up. Write a different message if the namespace is already imported vs missing.

            if !Self::has_acceptable_require(
                t,
                current_file,
                &current,
                &required_file,
                reference_strength,
            ) || original_import.is_none()
                || !Arc::ptr_eq(original_import.unwrap(), &required_file)
            {
                // Why check `originalImport == null|| originalImport != requiredFile`? in case the
                // imported file does have an acceptable goog.require in this file, but this
                // particular reference is still through the wrong namespace.
                // For example:
                //   const foo = goog.require('my.foo');
                //   const bar = goog.require('my.foo.bar');
                // (...)
                //    use(foo.bar); // should be use(bar);
                let to_report: &'static DiagnosticType = if current_file.is_module() {
                    if reference_strength.is_strong {
                        &INDIRECT_NAMESPACE_REF_REQUIRE
                    } else {
                        &INDIRECT_NAMESPACE_REF_REQUIRE_TYPE
                    }
                } else if reference_strength.is_strong {
                    &MISSING_REQUIRE_IN_GOOG_SCOPE
                } else {
                    &MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE
                };

                let mut error_node = n;
                // Get the right location if it is a code reference.
                let diff = normalized_qualified_name.get_component_count(t)
                    - current.get_component_count(t);
                if n.is_qualified_name(t) {
                    let mut i = 0;
                    while i < diff && error_node.is_get_prop(t) {
                        error_node = error_node.get_first_child(t).unwrap();
                        i += 1;
                    }
                    let root = NodeUtil::get_root_of_qualified_name(t, error_node);
                    t.report_with_range(root, error_node, to_report, &[&namespace.to_string()]);
                } else {
                    // JSDoc reference case
                    // Trim the original qualified name from the source text by the same difference.
                    let mut error_q_name = original_qualified_name.clone();
                    for _ in 0..diff {
                        error_q_name = error_q_name.get_owner(t).unwrap();
                    }

                    let correct_name = error_q_name.join(t);
                    // For JSDoc, the entire qualified name is a single STRINGLIT node.
                    // To report the error on the correct sub-expression, we create a new
                    // temporary node with the correct string and length.
                    let new_error_node = n.clone_node(t);
                    new_error_node.set_string(t, correct_name.clone());
                    new_error_node.set_length(t, correct_name.length() as i32);

                    t.report(new_error_node, to_report, &[&namespace.to_string()]);
                }
            }

            // We found the imported namespace: done
            return;
        }
    }

    /// Checks if a reference to a global qualified name should trigger a missing require warning
    ///
    /// Here, "global qualified name" means that the root object for the name is not defined in
    /// some local module or function scope, but is global.
    // port: CheckMissingRequires#checkMissingRequireThroughFullyQualifiedName
    fn check_missing_require_through_fully_qualified_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_file: &ModuleMetadata,
        qualified_name: &QualifiedName,
        reference_strength: Strength,
    ) {
        // Look for the longest prefix match against a provided namespace.
        let mut sub_name = Some(qualified_name.clone());
        while let Some(current) = sub_name {
            let namespace = current.join(t);
            if self.is_allowed_namespace(current_file, &namespace) {
                return;
            }

            let Some(required_file) = self.module_by_namespace.get(&namespace).cloned() else {
                // Not a known namespace check the parent
                sub_name = current.get_owner(t);
                continue;
            };
            let file_name = required_file.root_node().unwrap().get_source_file_name(t);
            let to_report: &'static DiagnosticType = if current_file.is_module() {
                // In files that represent modules, report a require without an alias the same as a
                // totally missing require.
                if reference_strength.is_strong {
                    &MISSING_REQUIRE
                } else {
                    &MISSING_REQUIRE_TYPE
                }
            } else if !Self::has_acceptable_require(
                t,
                current_file,
                &current,
                &required_file,
                reference_strength,
            ) {
                // In files that aren't modules, report a qualified name reference only if there's
                // no require to satisfy it.
                if reference_strength.is_strong {
                    &MISSING_REQUIRE_IN_PROVIDES_FILE
                } else {
                    &MISSING_REQUIRE_TYPE_IN_PROVIDES_FILE
                }
            } else if !reference_strength.is_type_only && required_file.is_non_legacy_goog_module()
            {
                // The referenced file has a valid goog.require for it, but the reference cannot be
                // in a regular qualified name: that won't be defined at runtime.
                // However, this only applies to references in actual code. Type-only references may
                // still reference the namespace in a script. The Closure type system will still
                // resolve the namespace - it just doesn't have any runtime effect.
                &NON_LEGACY_GOOG_MODULE_REFERENCE
            } else {
                return;
            };

            t.report(
                n,
                to_report,
                &[
                    &namespace.to_string_lossy(),
                    file_name.as_deref().unwrap_or("null"),
                ],
            );
            return;
        }
    }

    /// Given a qualified name with local root name and an import that defines that name,
    /// transform that name so that it is a fully qualified name.
    // port: CheckMissingRequires#normalizeQualifiedNamePlusImport
    fn normalize_qualified_name_plus_import(
        ast: &Ast,
        qualified_name: &QualifiedName,
        imported: &GoogRequire,
    ) -> QualifiedName {
        let mut new_q_name = QualifiedName::of(imported.namespace());

        if let Some(property) = imported.property() {
            new_q_name = new_q_name.getprop(property);
        }
        Self::swap_root_of_qualified_name(ast, qualified_name, &new_q_name)
    }

    /// Replaces the root name of the original qname with the newRoot (which may be a simple or
    /// complex qualified name).
    ///
    /// For example: if original is `oldRoot.a.b.c`, and newRoot is `newRoot.withProperty`,
    /// returns `newRoot.withProperty.a.b.c`.
    // port: CheckMissingRequires#swapRootOfQualifiedName
    fn swap_root_of_qualified_name(
        ast: &Ast,
        original: &QualifiedName,
        new_root: &QualifiedName,
    ) -> QualifiedName {
        let mut new_q_name = new_root.clone();
        let mut components = original.components(ast).into_iter();
        components.next(); // skip the root
        for component in components {
            new_q_name = new_q_name.getprop(component);
        }
        // Verify namespace usage
        new_q_name
    }

    // port: CheckMissingRequires#isAllowedNamespace
    fn is_allowed_namespace(&self, current_file: &ModuleMetadata, namespace: &JsString) -> bool {
        if *namespace == "goog.module" {
            // We must special case `goog.module` because Closure Library provides a namespace with
            // that name, but it's (confusingly) unrelated to the `goog.module` primitive.
            return true;
        }

        // Do not report references to a namespace provided in the same file, and do not recurse
        // into parent namespaces either.
        // TODO(tjgq): Also check for these references.
        if current_file.goog_namespaces().contains(namespace) {
            return true;
        }

        false
    }

    /// Does `rdep` contain an acceptable require for `namespace` from `dep`?
    ///
    /// Any require for a parent of `namespace` from `rdep` onto `dep`, which declares that parent,
    /// is sufficient. This constraint still ensures correct dependency ordering.
    ///
    /// We loosen the check in this way because we want to make it easy to migrate multi-provide
    /// files into modules. That includes deleting obsolete provides and splitting provides into
    /// different files.
    // port: CheckMissingRequires#hasAcceptableRequire
    fn has_acceptable_require(
        ast: &Ast,
        rdep: &ModuleMetadata,
        namespace: &QualifiedName,
        dep: &ModuleMetadata,
        reference_strength: Strength,
    ) -> bool {
        let mut acceptable_requires: IndexSet<&JsString> = rdep
            .strongly_required_goog_namespaces()
            .element_set()
            .collect();
        if !reference_strength.is_strong {
            // Sets.union
            acceptable_requires.extend(rdep.weakly_required_goog_namespaces().element_set());
        }
        // Sets.intersection
        let dep_namespaces: IndexSet<&JsString> = dep.goog_namespaces().element_set().collect();
        acceptable_requires.retain(|namespace| dep_namespaces.contains(namespace));

        let mut parent = Some(namespace.clone());
        while let Some(current) = parent {
            if acceptable_requires.contains(&current.join(ast)) {
                return true;
            }
            parent = current.get_owner(ast);
        }

        false
    }
}

/// Represents the strength of references of an require'd name, i.e. whether references are in
/// code only (strong, not-typeOnly), JsDoc annotations for \@implements or \@extends (strong as
/// well as typeOnly) or in other JsDoc annotations only (weak, typeOnly).
// port: CheckMissingRequires.Strength
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
enum StrengthKind {
    CODE,
    IMPLEMENTS_EXTENDS,
    WEAK_TYPE,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Strength {
    #[allow(dead_code)] // Java's enum constant identity.
    kind: StrengthKind,
    is_strong: bool,
    is_type_only: bool,
}

#[allow(non_upper_case_globals)]
impl Strength {
    // port: CheckMissingRequires.Strength#CODE
    const CODE: Strength = Strength::new(StrengthKind::CODE, true, false);
    // port: CheckMissingRequires.Strength#IMPLEMENTS_EXTENDS
    const IMPLEMENTS_EXTENDS: Strength =
        Strength::new(StrengthKind::IMPLEMENTS_EXTENDS, true, true);
    // port: CheckMissingRequires.Strength#WEAK_TYPE
    const WEAK_TYPE: Strength = Strength::new(StrengthKind::WEAK_TYPE, false, true);

    // port: CheckMissingRequires.Strength#Strength
    const fn new(kind: StrengthKind, is_strong: bool, is_type_only: bool) -> Self {
        Self {
            kind,
            is_strong,
            is_type_only,
        }
    }
}

impl CompilerPass for CheckMissingRequires {
    // port: CheckMissingRequires#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let module_metadata_map = self.module_metadata_map.clone();
        let mut callback = AbstractModuleCallback::new(&module_metadata_map, &mut *self);
        NodeTraversal::traverse(compiler, root, &mut callback);
    }
}

impl ModuleCallback for &mut CheckMissingRequires {
    // port: CheckMissingRequires#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_module: Option<&Arc<ModuleMetadata>>,
        _scope_root: Option<NodeId>,
    ) -> bool {
        if NodeUtil::is_valid_cfg_root(t, n) && !self.is_in_global_control_flow_scope(t, n) {
            self.control_flow_scope_depth += 1;
        }
        let Some(current_module) = current_module else {
            return true;
        };

        // Traverse nodes in preorder to collect `@template` parameter names before their use.
        self.visit_node(t, n, current_module);
        true
    }

    // port: CheckMissingRequires#visit
    fn visit(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        current_module: Option<&Arc<ModuleMetadata>>,
        _scope_root: Option<NodeId>,
    ) {
        if NodeUtil::is_valid_cfg_root(t, n) && !self.is_in_global_control_flow_scope(t, n) {
            self.control_flow_scope_depth -= 1;
        }
        if let Some(current_module) = current_module
            && Some(n) == current_module.root_node()
        {
            // For this pass, template parameter names are only meaningful inside the file
            // defining them.
            self.template_param_names.clear();
        }
    }
}
