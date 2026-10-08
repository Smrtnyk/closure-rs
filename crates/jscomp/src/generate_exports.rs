/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/GenerateExports.java.

//! Port of `GenerateExports.java`.
#![allow(clippy::collapsible_if, clippy::if_same_then_else)] // Keep Java's control flow.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::find_exportable_nodes::FindExportableNodes;
use crate::js_error::JSError;
use crate::node_traversal::NodeTraversal;
use crate::node_util::NodeUtil;
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use closure_jstype::JSTypeNative;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::static_source_file::StaticSourceFile;
use closure_rhino::{check_argument, check_not_null};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

const PROTOTYPE_PROPERTY: &str = "prototype";

// port: GenerateExports#MISSING_EXPORT_CONVENTION
pub static MISSING_EXPORT_CONVENTION: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_EXPORT_CONVENTION",
    "@export cannot be used without defining a exportProperty and exportSymbol function in the coding convention",
);

// port: GenerateExports#MISSING_GOOG_FOR_EXPORT
pub static MISSING_GOOG_FOR_EXPORT: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_EXPORT_SYMBOL_DEFINITION",
    "@export cannot be used without including closure/base.js or other definition of {0} and {1}",
);

/// Generates goog.exportSymbol/goog.exportProperty for the @export annotation.
pub struct GenerateExports {
    export_symbol_function: Option<JsString>,
    export_property_function: Option<JsString>,
    allow_non_global_exports: bool,
    exported_variables: IndexSet<JsString>,
}

impl GenerateExports {
    /// Creates a new generate exports compiler pass.
    ///
    /// The `export_symbol_function` and `export_property_function` are optional, but the compiler
    /// will report an error if they are not passed *and* this pass finds @export annotations. They
    /// are allowed to be optional if no code uses the @export annotation.
    // port: GenerateExports#GenerateExports
    pub fn new(
        _compiler: &AbstractCompiler,
        allow_non_global_exports: bool,
        export_symbol_function: Option<JsString>,
        export_property_function: Option<JsString>,
    ) -> Self {
        Self {
            allow_non_global_exports,
            export_symbol_function,
            export_property_function,
            exported_variables: IndexSet::new(),
        }
    }

    // port: GenerateExports#getExportedVariableNames
    pub fn get_exported_variable_names(&self) -> &IndexSet<JsString> {
        &self.exported_variables
    }

    /// Validate that a) the user has configured methods to call to export symbols and b) those
    /// methods are included in this binary
    ///
    /// Returns whether to continue trying to export methods
    // port: GenerateExports#validateExportMethodsIncluded
    fn validate_export_methods_included(
        &self,
        compiler: &mut AbstractCompiler,
        exports: &IndexMap<JsString, NodeId>,
        es6_exports: &IndexMap<NodeId, JsString>,
        root: NodeId,
    ) -> bool {
        // Pick an arbitrary @export to report the warning on.
        let error_location = if !exports.is_empty() {
            *exports.values().next().unwrap()
        } else {
            *es6_exports.keys().next().unwrap()
        };

        let (Some(export_symbol_function), Some(export_property_function)) = (
            self.export_symbol_function.as_ref(),
            self.export_property_function.as_ref(),
        ) else {
            compiler.report(JSError::make(
                compiler,
                error_location,
                &MISSING_EXPORT_CONVENTION,
                &[],
            ));
            // don't try to rewrite @export since there's nothing we can rewrite them to.
            return false;
        };

        let root_parent = root.get_parent(compiler).unwrap();
        if !self.includes_export_methods(compiler, root_parent) {
            let symbol = export_symbol_function.to_string_lossy();
            let property = export_property_function.to_string_lossy();
            compiler.report(JSError::make(
                compiler,
                error_location,
                &MISSING_GOOG_FOR_EXPORT,
                &[&symbol, &property],
            ));
            self.declare_export_methods_in_externs(compiler, error_location);
            return true;
        }
        true
    }

    /// In order to use @export, the user must have included the defintiion of the property and
    /// symbol export methods in their binary.
    ///
    /// In real code these methods are typically goog.exportProperty and goog.exportSymbol.
    // port: GenerateExports#includesExportMethods
    fn includes_export_methods(&self, compiler: &mut AbstractCompiler, root: NodeId) -> bool {
        let global_scope = SyntacticScopeCreator::new().create_scope(compiler, root, None);
        let export_property_root = NodeUtil::get_root_of_qualified_name_string(
            self.export_property_function.as_ref().unwrap(),
        );
        let export_symbol_root = NodeUtil::get_root_of_qualified_name_string(
            self.export_symbol_function.as_ref().unwrap(),
        );
        global_scope.has_slot(compiler, &export_property_root)
            && global_scope.has_slot(compiler, &export_symbol_root)
    }

    /// Add a synthesized externs declaration to prevent crashes later on in VarCheck.
    // port: GenerateExports#declareExportMethodsInExterns
    fn declare_export_methods_in_externs(&self, compiler: &mut AbstractCompiler, srcref: NodeId) {
        let export_property_root = NodeUtil::get_root_of_qualified_name_string(
            self.export_property_function.as_ref().unwrap(),
        );
        let export_symbol_root = NodeUtil::get_root_of_qualified_name_string(
            self.export_symbol_function.as_ref().unwrap(),
        );

        if export_property_root == export_symbol_root {
            Self::declare_synthetic_externs_var(compiler, export_property_root, srcref);
        } else {
            Self::declare_synthetic_externs_var(compiler, export_property_root, srcref);
            Self::declare_synthetic_externs_var(compiler, export_symbol_root, srcref);
        }
    }

    // port: GenerateExports#declareSyntheticExternsVar
    fn declare_synthetic_externs_var(
        compiler: &mut AbstractCompiler,
        name: JsString,
        srcref: NodeId,
    ) {
        let synthetic_input = compiler.get_synthesized_externs_input().clone();
        let synthetic_var_root = synthetic_input.get_ast_root(compiler);

        let name = IR::name(compiler, name);
        let var_declaration = IR::var(compiler, name).srcref_tree(compiler, srcref);
        let source_file: Arc<dyn StaticSourceFile> = compiler
            .get_synthesized_externs_input()
            .get_source_file_arc();
        var_declaration.set_static_source_file(compiler, Some(source_file));
        synthetic_var_root.add_child_to_back(compiler, var_declaration);
        compiler.report_change_to_enclosing_scope(var_declaration);
    }

    // port: GenerateExports#addExtern
    fn add_extern(&self, compiler: &mut AbstractCompiler, export: &JsString) {
        let object_prototype = NodeUtil::new_qname(compiler, "Object.prototype");
        let obj_ctor = compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::OBJECT_FUNCTION_TYPE);
        object_prototype
            .get_first_child(compiler)
            .unwrap()
            .set_jstype(compiler, Some(obj_ctor));
        let getprop = IR::getprop(compiler, object_prototype, export.clone());
        let propstmt = IR::expr_result(compiler, getprop);
        let externs_root = Self::get_synthesized_externs_root(compiler);
        propstmt.srcref_tree(compiler, externs_root);
        propstmt.set_original_name(compiler, Some(export.clone()));
        let externs_root = Self::get_synthesized_externs_root(compiler);
        externs_root.add_child_to_back(compiler, propstmt);
        compiler.report_change_to_enclosing_scope(propstmt);
    }

    // port: GenerateExports#recordExportSymbol
    fn record_export_symbol(&mut self, qname: &JsString) {
        let dot = qname.index_of_char(u16::from(b'.'));
        if dot == -1 {
            self.exported_variables.insert(qname.clone());
        } else {
            self.exported_variables
                .insert(qname.substring(0, dot as usize));
        }
    }

    // port: GenerateExports#addExportForEs6Method
    fn add_export_for_es6_method(
        &mut self,
        compiler: &mut AbstractCompiler,
        member_function: NodeId,
        owner_name: &JsString,
    ) {
        // We always export ES6 member methods as properties.
        check_argument!(
            member_function.is_member_function_def(compiler),
            "%s",
            member_function.to_string(compiler)
        );
        check_argument!(!owner_name.is_empty(), "%s", owner_name.to_string_lossy());
        let full_export = owner_name
            .concat(&JsString::from("."))
            .concat(&member_function.get_string(compiler));
        let property_name = member_function.get_string(compiler);
        self.add_export_property_call(
            compiler,
            Some(owner_name.clone()),
            member_function,
            full_export,
            property_name,
        );
    }

    /// Emits a call to either goog.exportProperty or goog.exportSymbol.
    ///
    /// Attempts to optimize by creating a property export instead of a symbol export, because
    /// property exports are significantly simpler/faster.
    ///
    /// `export` The fully qualified name of the object we want to export; `context` The node on
    /// which the @export annotation was found
    // port: GenerateExports#addExportMethod
    fn add_export_method(
        &mut self,
        compiler: &mut AbstractCompiler,
        exports: &IndexMap<JsString, NodeId>,
        export: &JsString,
        context: NodeId,
    ) {
        // We can export as a property if any of the following conditions holds:
        // a) ES6 class members, which the above `addExportForEs6Method` handles
        // b) this is a property on a name which is also being exported
        // c) this is a prototype property
        let mut method_owner_name: Option<JsString> = None; // the object this method is on, null for exported names.
        let mut is_es5_style_prototype_assignment = false; // If this is a prototype property
        let mut property_name: Option<JsString> = None;

        let child = context.get_first_child(compiler);
        if child.is_some_and(|child| child.is_get_prop(compiler)) {
            // e.g. `/** @export */ a.prototype.b = obj;`
            let node = context.get_first_child(compiler).unwrap(); // e.g. get `a.prototype.b`
            let owner_node = node.get_first_child(compiler).unwrap(); // e.g. get `a.prototype`
            method_owner_name = owner_node.get_qualified_name(compiler); // e.g. get the string "a.prototype"
            if owner_node.is_get_prop(compiler)
                && owner_node.get_string(compiler) == PROTOTYPE_PROPERTY
            {
                // e.g. true if ownerNode is `a.prototype`
                // false if this export were `/** @export */ a.b = obj;` instead
                is_es5_style_prototype_assignment = true;
            }
            property_name = Some(node.get_string(compiler));
        }

        let mut use_export_symbol = true;
        if is_es5_style_prototype_assignment {
            use_export_symbol = false;
        } else if method_owner_name
            .as_ref()
            .is_some_and(|name| exports.contains_key(name))
        {
            use_export_symbol = false;
        }

        if use_export_symbol {
            self.add_export_symbol_call(compiler, export, context);
        } else {
            self.add_export_property_call(
                compiler,
                method_owner_name,
                context,
                export.clone(),
                property_name.unwrap(),
            );
        }
    }

    // port: GenerateExports#addExportPropertyCall
    fn add_export_property_call(
        &self,
        compiler: &mut AbstractCompiler,
        method_owner_name: Option<JsString>,
        context: NodeId,
        export: JsString,
        property_name: JsString,
    ) {
        // JS output: exportProperty(object, publicName, symbol);
        let method_owner_name = check_not_null!(method_owner_name);
        let export_property_function = self.export_property_function.clone().unwrap();
        let target = NodeUtil::new_qname_with_basis(
            compiler,
            export_property_function.clone(),
            context,
            export_property_function.clone(),
        );
        let owner = NodeUtil::new_qname_with_basis(
            compiler,
            method_owner_name,
            context,
            export_property_function.clone(),
        );
        let public_name = IR::string(compiler, property_name);
        let symbol =
            NodeUtil::new_qname_with_basis(compiler, export, context, export_property_function);
        let call = IR::call(compiler, target, &[owner, public_name, symbol]);

        let expression = IR::expr_result(compiler, call).srcref_tree_if_missing(compiler, context);

        Self::add_statement(compiler, context, expression);
    }

    // port: GenerateExports#addExportSymbolCall
    fn add_export_symbol_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        export: &JsString,
        context: NodeId,
    ) {
        // JS output: exportSymbol(publicPath, object);
        self.record_export_symbol(export);

        let export_symbol_function = self.export_symbol_function.clone().unwrap();
        let target = NodeUtil::new_qname_with_basis(
            compiler,
            export_symbol_function,
            context,
            export.clone(),
        );
        let public_path = IR::string(compiler, export.clone());
        let object =
            NodeUtil::new_qname_with_basis(compiler, export.clone(), context, export.clone());
        let call = IR::call(compiler, target, &[public_path, object]);

        let expression = IR::expr_result(compiler, call).srcref_tree_if_missing(compiler, context);

        Self::add_statement(compiler, context, expression);
    }

    // port: GenerateExports#addStatement
    fn add_statement(compiler: &mut AbstractCompiler, context: NodeId, stmt: NodeId) {
        let n = context;
        let mut expr_root = n;
        while !NodeUtil::is_statement_block(compiler, expr_root.get_parent(compiler).unwrap()) {
            expr_root = expr_root.get_parent(compiler).unwrap();
        }

        // It's important that any class-building calls (goog.inherits)
        // come right after the class definition, so move the export after that.
        loop {
            let next = expr_root.get_next(compiler);
            if let Some(next) = next
                && NodeUtil::is_expr_call(compiler, next)
                && compiler
                    .get_coding_convention()
                    .get_classes_defined_by_call(compiler, next.get_first_child(compiler).unwrap())
                    .is_some()
            {
                expr_root = next;
            } else {
                break;
            }
        }

        stmt.insert_after(compiler, expr_root);
        compiler.report_change_to_enclosing_scope(stmt);
    }

    /// Lazily create a "new" externs root for undeclared variables.
    // port: GenerateExports#getSynthesizedExternsRoot
    fn get_synthesized_externs_root(compiler: &mut AbstractCompiler) -> NodeId {
        let input = compiler.get_synthesized_externs_input().clone();
        input.get_ast_root(compiler)
    }
}

impl CompilerPass for GenerateExports {
    // port: GenerateExports#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut find_exportable_nodes =
            FindExportableNodes::new(compiler, self.allow_non_global_exports);
        NodeTraversal::traverse(compiler, root, &mut find_exportable_nodes);
        let exports = find_exportable_nodes.get_exports();
        let es6_exports = find_exportable_nodes.get_es6_class_exports();
        let local_exports = find_exportable_nodes.get_local_exports();

        for export in local_exports {
            self.add_extern(compiler, export);
        }

        if !exports.is_empty() || !es6_exports.is_empty() {
            if !self.validate_export_methods_included(compiler, exports, es6_exports, root) {
                return;
            }
        }

        for (key, value) in es6_exports {
            self.add_export_for_es6_method(compiler, *key, value);
        }

        for (export, context) in exports {
            self.add_export_method(compiler, exports, export, *context);
        }
    }
}
