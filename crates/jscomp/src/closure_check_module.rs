/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ClosureCheckModule.java,
//   src/com/google/javascript/jscomp/NodeUtil.java.

#![allow(
    clippy::collapsible_match,
    clippy::collapsible_if,
    clippy::if_same_then_else
)] // Keep Java's control flow.
use crate::{
    abstract_compiler::AbstractCompiler,
    closure_primitive_errors::{
        INVALID_DESTRUCTURING_FORWARD_DECLARE, MODULE_USES_GOOG_MODULE_GET,
    },
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    guava_ascii::{is_upper_case, to_lower_case, to_upper_case},
    js_error::JSError,
    modules::module_metadata_map::{ModuleMetadata, ModuleMetadataMap},
    node_traversal::{AbstractModuleCallback, ModuleCallback, NodeTraversal},
    node_util::{NodeUtil, Visitor},
    var::Var,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::{Arc, LazyLock};

// port: ClosureCheckModule#AT_EXPORT_IN_GOOG_MODULE
pub static AT_EXPORT_IN_GOOG_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_AT_EXPORT_IN_GOOG_MODULE",
    "@export has no effect on top-level names in a goog.module. Consider using goog.exportSymbol instead.",
);

// port: ClosureCheckModule#AT_EXPORT_IN_NON_LEGACY_GOOG_MODULE
pub static AT_EXPORT_IN_NON_LEGACY_GOOG_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_AT_EXPORT_IN_NON_LEGACY_GOOG_MODULE",
    "@export is not allowed here in a non-legacy goog.module. Consider using goog.exportSymbol instead.",
);

// port: ClosureCheckModule#GOOG_MODULE_IN_NON_MODULE
pub static GOOG_MODULE_IN_NON_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_IN_NON_MODULE",
    "goog.module() call must be the first statement in a module.",
);

// port: ClosureCheckModule#GOOG_MODULE_MISPLACED
pub static GOOG_MODULE_MISPLACED: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_MISPLACED",
    "goog.module() call must be the first statement in a file.",
);

// port: ClosureCheckModule#DECLARE_LEGACY_NAMESPACE_IN_NON_MODULE
pub static DECLARE_LEGACY_NAMESPACE_IN_NON_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_DECLARE_LEGACY_NAMESPACE_IN_NON_MODULE",
    "goog.module.declareLegacyNamespace may only be called in a goog.module.",
);

// port: ClosureCheckModule#GOOG_MODULE_REFERENCES_THIS
pub static GOOG_MODULE_REFERENCES_THIS: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_REFERENCES_THIS",
    "The body of a goog.module cannot reference 'this'.",
);

// port: ClosureCheckModule#GOOG_MODULE_USES_THROW
pub static GOOG_MODULE_USES_THROW: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_USES_THROW",
    "The body of a goog.module cannot use 'throw'.",
);

// port: ClosureCheckModule#DUPLICATE_NAME_SHORT_REQUIRE
pub static DUPLICATE_NAME_SHORT_REQUIRE: DiagnosticType = DiagnosticType::error(
    "JSC_DUPLICATE_NAME_SHORT_REQUIRE",
    "Found multiple goog.require statements importing identifier ''{0}''.",
);

// port: ClosureCheckModule#INVALID_DESTRUCTURING_REQUIRE
pub static INVALID_DESTRUCTURING_REQUIRE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DESTRUCTURING_REQUIRE",
    "Destructuring goog.require must be a simple object pattern.",
);

// port: ClosureCheckModule#LET_GOOG_REQUIRE
pub static LET_GOOG_REQUIRE: DiagnosticType = DiagnosticType::disabled(
    "JSC_LET_GOOG_REQUIRE",
    "Module imports must be constant. Please use ''const'' instead of ''let''.",
);

// port: ClosureCheckModule#MULTIPLE_MODULES_IN_FILE
pub static MULTIPLE_MODULES_IN_FILE: DiagnosticType = DiagnosticType::error(
    "JSC_MULTIPLE_MODULES_IN_FILE",
    "There should only be a single goog.module() statement per file.",
);

// port: ClosureCheckModule#ONE_REQUIRE_PER_DECLARATION
pub static ONE_REQUIRE_PER_DECLARATION: DiagnosticType = DiagnosticType::error(
    "JSC_ONE_REQUIRE_PER_DECLARATION",
    "There may only be one goog.require() per var/let/const declaration.",
);

// port: ClosureCheckModule#INCORRECT_SHORTNAME_CAPITALIZATION
pub static INCORRECT_SHORTNAME_CAPITALIZATION: DiagnosticType = DiagnosticType::disabled(
    "JSC_INCORRECT_SHORTNAME_CAPITALIZATION",
    "The capitalization of short name {0} is incorrect; it should be {1}.",
);

// port: ClosureCheckModule#EXPORT_NOT_AT_MODULE_SCOPE
pub static EXPORT_NOT_AT_MODULE_SCOPE: DiagnosticType = DiagnosticType::error(
    "JSC_EXPORT_NOT_AT_MODULE_SCOPE",
    "Exports must be at the top-level of a module",
);

// port: ClosureCheckModule#EXPORT_NOT_A_STATEMENT
pub static EXPORT_NOT_A_STATEMENT: DiagnosticType = DiagnosticType::error(
    "JSC_EXPORT_NOT_A_STATEMENT",
    "Exports should be a statement.",
);

// port: ClosureCheckModule#EXPORT_REPEATED_ERROR
pub static EXPORT_REPEATED_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_EXPORT_REPEATED_ERROR",
    "Name cannot be exported multiple times. Previous export on line {0}.",
);

// port: ClosureCheckModule#REFERENCE_TO_MODULE_GLOBAL_NAME
pub static REFERENCE_TO_MODULE_GLOBAL_NAME: DiagnosticType = DiagnosticType::error(
    "JSC_REFERENCE_TO_MODULE_GLOBAL_NAME",
    "References to the global name of a module are not allowed. Perhaps you meant exports?",
);

// port: ClosureCheckModule#REFERENCE_TO_FULLY_QUALIFIED_IMPORT_NAME
pub static REFERENCE_TO_FULLY_QUALIFIED_IMPORT_NAME: DiagnosticType = DiagnosticType::disabled(
    "JSC_REFERENCE_TO_FULLY_QUALIFIED_IMPORT_NAME",
    "Reference to fully qualified import name ''{0}''. Imports in goog.module should use the return value of goog.require / goog.forwardDeclare instead.",
);

// port: ClosureCheckModule#REFERENCE_TO_SHORT_IMPORT_BY_LONG_NAME_INCLUDING_SHORT_NAME
pub static REFERENCE_TO_SHORT_IMPORT_BY_LONG_NAME_INCLUDING_SHORT_NAME: DiagnosticType =
    DiagnosticType::disabled(
        "JSC_REFERENCE_TO_SHORT_IMPORT_BY_LONG_NAME_INCLUDING_SHORT_NAME",
        "Reference to fully qualified import name ''{0}''. Please use the short name ''{1}'' instead.",
    );

// port: ClosureCheckModule#USE_OF_GOOG_PROVIDE
pub static USE_OF_GOOG_PROVIDE: DiagnosticType = DiagnosticType::disabled(
    "JSC_USE_OF_GOOG_PROVIDE",
    "goog.provide is deprecated in favor of goog.module.",
);

// port: ClosureCheckModule#REQUIRE_NOT_AT_TOP_LEVEL
pub static REQUIRE_NOT_AT_TOP_LEVEL: DiagnosticType = DiagnosticType::error(
    "JSC_REQUIRE_NOT_AT_TOP_LEVEL",
    "goog.require() must be called at file scope.",
);

// port: ClosureCheckModule#LEGACY_NAMESPACE_NOT_AT_TOP_LEVEL
pub static LEGACY_NAMESPACE_NOT_AT_TOP_LEVEL: DiagnosticType = DiagnosticType::error(
    "JSC_LEGACY_NAMESPACE_NOT_AT_TOP_LEVEL",
    "goog.module.declareLegacyNamespace() does not return a value",
);

// port: ClosureCheckModule#LEGACY_NAMESPACE_NOT_AFTER_GOOG_MODULE
pub static LEGACY_NAMESPACE_NOT_AFTER_GOOG_MODULE: DiagnosticType = DiagnosticType::error(
    "JSC_LEGACY_NAMESPACE_NOT_AT_TOP_LEVEL",
    "goog.module.declareLegacyNamespace() must be immediately after the goog.module('...'); call",
);

// port: ClosureCheckModule#LEGACY_NAMESPACE_ARGUMENT
pub static LEGACY_NAMESPACE_ARGUMENT: DiagnosticType = DiagnosticType::error(
    "JSC_LEGACY_NAMESPACE_ARGUMENT",
    "goog.module.declareLegacyNamespace() takes no arguments",
);

// port: ClosureCheckModule#GOOG_MODULE
static GOOG_MODULE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.module"));
// port: ClosureCheckModule#GOOG_PROVIDE
static GOOG_PROVIDE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.provide"));
// port: ClosureCheckModule#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: ClosureCheckModule#GOOG_REQUIRE_TYPE
static GOOG_REQUIRE_TYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
// port: ClosureCheckModule#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));
// port: ClosureCheckModule#GOOG_FORWARD_DECLARE
static GOOG_FORWARD_DECLARE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.forwardDeclare"));
// port: ClosureCheckModule#GOOG_MODULE_DECLARE_LEGACY_NAMESPACE
static GOOG_MODULE_DECLARE_LEGACY_NAMESPACE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.declareLegacyNamespace"));

// port: ClosureCheckModule.ModuleInfo
struct ModuleInfo {
    // Name of the module in question (i.e. the argument to goog.module)
    name: JsString,
    // Mapping from fully qualified goog.required names to the import LHS node.
    // For standalone goog.require()s the value is the EXPR_RESULT node.
    // (Java's LinkedHashMap allows the null key a non-string first argument produces.)
    imports_by_long_required_name: IndexMap<Option<JsString>, NodeId>,
    // Module-local short names for goog.required symbols.
    short_import_names: IndexSet<JsString>,
    // A map from the export names "exports.name" to the nodes of those named exports.
    // The default export is keyed with just "exports"
    export_nodes_by_name: IndexMap<JsString, NodeId>,
}

impl ModuleInfo {
    // port: ClosureCheckModule.ModuleInfo#ModuleInfo
    fn new(module_name: JsString) -> Self {
        Self {
            name: module_name,
            imports_by_long_required_name: IndexMap::<_, _>::default(),
            short_import_names: IndexSet::<_>::default(),
            export_nodes_by_name: IndexMap::<_, _>::default(),
        }
    }
}

/// Checks that goog.module() is used correctly.
///
/// Note that this file only does checks that can be done per-file. Whole program checks happen
/// during goog.module rewriting, in `ClosureRewriteModule`.
pub struct ClosureCheckModule {
    module_metadata_map: Arc<ModuleMetadataMap>,
    current_module_info: Option<ModuleInfo>,
}

impl ClosureCheckModule {
    // port: ClosureCheckModule#ClosureCheckModule
    pub fn new(_compiler: &AbstractCompiler, module_metadata_map: Arc<ModuleMetadataMap>) -> Self {
        Self {
            module_metadata_map,
            current_module_info: None,
        }
    }

    fn current_module_info(&mut self) -> &mut ModuleInfo {
        self.current_module_info.as_mut().unwrap()
    }

    // port: ClosureCheckModule#checkJSDoc
    fn check_js_doc(&mut self, t: &mut NodeTraversal<'_>, js_doc: &JSDocInfo) {
        for type_node in js_doc.get_type_nodes() {
            self.check_type_expression(t, type_node);
        }
    }

    // port: ClosureCheckModule#checkTypeExpression
    fn check_type_expression(&mut self, t: &mut NodeTraversal<'_>, type_node: NodeId) {
        // Java visits with an anonymous NodeUtil.Visitor that only reports; the nodes are
        // collected in the same pre-order first so the visitor body can use the traversal.
        struct Collect(Vec<NodeId>);
        impl Visitor for Collect {
            fn visit(&mut self, _ast: &mut Ast, node: NodeId) {
                self.0.push(node);
            }
        }
        let mut collect = Collect(Vec::new());
        NodeUtil::visit_pre_order(t, type_node, &mut collect);
        for node in collect.0 {
            // port: ClosureCheckModule#checkTypeExpression (anonymous NodeUtil.Visitor#visit)
            if !node.is_string_lit(t) {
                continue;
            }

            let mut qname = node.get_string(t);
            let mut next_qname_part: Option<JsString> = None;

            while self.check_improper_reference_to_import(
                t,
                node,
                Some(qname.clone()),
                next_qname_part.clone(),
            ) {
                let last_dot = qname.last_index_of_char(u16::from(b'.'));
                if last_dot < 0 {
                    break;
                }
                next_qname_part = Some(qname.substring_from(last_dot as usize + 1));
                qname = qname.substring(0, last_dot as usize);
            }
        }
    }

    // port: ClosureCheckModule#isLocalVar
    fn is_local_var(compiler: &AbstractCompiler, v: Option<Var>) -> bool {
        let Some(v) = v else {
            return false;
        };
        v.is_local(compiler)
    }

    /// @return true if the reference might be valid, false means an error was reported
    // port: ClosureCheckModule#checkImproperReferenceToImport
    fn check_improper_reference_to_import(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        qname: Option<JsString>,
        next_qname_part: Option<JsString>,
    ) -> bool {
        let Some(qname) = qname else {
            return true;
        };

        let import_lhs = self
            .current_module_info()
            .imports_by_long_required_name
            .get(&Some(qname.clone()))
            .copied();
        let Some(import_lhs) = import_lhs else {
            return true;
        };

        if qname.index_of_char(u16::from(b'.')) < 0 {
            let scope = t.get_scope();
            let var = scope.get_var(t.get_compiler(), &qname);
            if Self::is_local_var(t.get_compiler(), var) {
                return true;
            }
        }

        if import_lhs.is_name(t) {
            let short_name = import_lhs.get_string(t).to_string();
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                n,
                &REFERENCE_TO_SHORT_IMPORT_BY_LONG_NAME_INCLUDING_SHORT_NAME,
                &[&qname.to_string(), &short_name],
            ));
            return false;
        } else if import_lhs.is_destructuring_lhs(t) && next_qname_part.is_some() {
            let next_qname_part = next_qname_part.unwrap();
            let obj_pattern = import_lhs.get_first_child(t).unwrap();
            if obj_pattern.is_object_pattern(t) {
                let mut str_key = obj_pattern.get_first_child(t);
                while let Some(key) = str_key {
                    // const {foo: barFoo} = goog.require('ns.bar');
                    // Should use the short name "barFoo" instead of "ns.bar.foo".
                    if key.is_string_key(t)
                        && key.has_one_child(t)
                        && key.get_first_child(t).unwrap().is_name(t)
                        && key.get_string(t) == next_qname_part
                    {
                        let parent = n.get_parent(t);
                        let target = match parent {
                            Some(parent) if parent.is_get_prop(t) => parent,
                            _ => n,
                        };
                        let long_name = format!("{qname}.{next_qname_part}");
                        let short_name = key.get_first_child(t).unwrap().get_string(t).to_string();
                        let compiler = t.get_compiler();
                        compiler.report(JSError::make(
                            compiler,
                            target,
                            &REFERENCE_TO_SHORT_IMPORT_BY_LONG_NAME_INCLUDING_SHORT_NAME,
                            &[&long_name, &short_name],
                        ));
                        return false;
                    }
                    str_key = key.get_next(t);
                }
            }
        }
        if import_lhs.is_expr_result(t)
            && NodeUtil::is_goog_forward_declare_call(t, import_lhs.get_only_child(t))
        {
            // goog.forwardDeclare does not need to be aliased.
            // This is because goog.forwardDeclares that are aliased have stricter semantics than
            // those that are not aliased: aliased goog.forwardDeclares always need to reference a
            // namespace that's actually defined, while non-aliased forwardDeclares may reference
            // namespaces that do not actually exist.
            return true;
        }

        let compiler = t.get_compiler();
        compiler.report(JSError::make(
            compiler,
            n,
            &REFERENCE_TO_FULLY_QUALIFIED_IMPORT_NAME,
            &[&qname.to_string()],
        ));
        false
    }

    /// Is this the LHS of a goog.module export? i.e. Either "exports" or "exports.name"
    // port: ClosureCheckModule#isExportLhs
    fn is_export_lhs(ast: &Ast, lhs: NodeId) -> bool {
        if !lhs.is_qualified_name(ast) {
            return false;
        }
        lhs.matches_name(ast, "exports")
            || (lhs.is_get_prop(ast)
                && lhs
                    .get_first_child(ast)
                    .unwrap()
                    .matches_name(ast, "exports"))
    }

    // port: ClosureCheckModule#checkModuleExport
    fn check_module_export(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: NodeId) {
        check_argument!(n.is_assign(t));
        let lhs = n.get_first_child(t).unwrap();
        check_state!(Self::is_export_lhs(t, lhs));
        let lhs_qname = lhs.get_qualified_name(t).unwrap();
        // Check multiple exports of the same name
        let previous_definition = self
            .current_module_info()
            .export_nodes_by_name
            .get(&lhs_qname)
            .copied();
        if let Some(previous_definition) = previous_definition {
            let previous_line = previous_definition.get_lineno(t);
            t.report(n, &EXPORT_REPEATED_ERROR, &[&previous_line.to_string()]);
        }
        // Check exports in invalid program position
        let default_export_node = self
            .current_module_info()
            .export_nodes_by_name
            .get(&JsString::from("exports"))
            .copied();
        // If we have never seen an `exports =` default export assignment, or this is the
        // default export, then treat this assignment as an export and do the checks it is well
        // formed.
        if default_export_node.is_none() || lhs.matches_name(t, "exports") {
            self.current_module_info()
                .export_nodes_by_name
                .insert(lhs_qname, lhs);
            if !t.in_module_scope() {
                t.report(n, &EXPORT_NOT_AT_MODULE_SCOPE, &[]);
            } else if !parent.is_expr_result(t) {
                t.report(n, &EXPORT_NOT_A_STATEMENT, &[]);
            }
        }
        // Check @export on a module local name
        if !NodeUtil::is_prototype_property(t, lhs)
            && !NodeUtil::is_legacy_goog_module_file(
                t,
                NodeUtil::get_enclosing_script(t, n).unwrap(),
            )
        {
            let js_doc = n.get_jsdoc_info(t);
            if js_doc.is_some_and(|js_doc| js_doc.is_export()) {
                t.report(n, &AT_EXPORT_IN_NON_LEGACY_GOOG_MODULE, &[]);
            }
        }
    }

    // port: ClosureCheckModule#extractFirstArgumentName
    fn extract_first_argument_name(ast: &Ast, call_node: NodeId) -> Option<JsString> {
        let first_arg = call_node.get_second_child(ast);
        if let Some(first_arg) = first_arg
            && first_arg.is_string_lit(ast)
        {
            return Some(first_arg.get_string(ast));
        }
        None
    }

    // port: ClosureCheckModule#checkRequireCall
    fn check_require_call(&mut self, t: &mut NodeTraversal<'_>, call_node: NodeId, parent: NodeId) {
        check_state!(call_node.is_call(t));
        check_state!(call_node.get_last_child(t).unwrap().is_string_lit(t));
        match parent.get_token(t) {
            Token::EXPR_RESULT => {
                let key = Self::extract_first_argument_name(t, call_node);
                self.current_module_info()
                    .imports_by_long_required_name
                    .entry(key)
                    .or_insert(parent);
            }
            Token::NAME | Token::DESTRUCTURING_LHS => {
                let declaration = parent.get_parent(t).unwrap();
                self.check_short_goog_require_call(t, call_node, declaration);
            }
            Token::AWAIT => {
                let grand_parent_token = parent.get_parent(t).unwrap().get_token(t);
                if grand_parent_token == Token::DESTRUCTURING_LHS
                    || grand_parent_token == Token::NAME
                {
                    let declaration = parent.get_grandparent(t).unwrap();
                    self.check_short_goog_require_call(t, call_node, declaration);
                }
            }
            _ => t.report(call_node, &REQUIRE_NOT_AT_TOP_LEVEL, &[]),
        }
    }

    // port: ClosureCheckModule#checkShortGoogRequireCall
    fn check_short_goog_require_call(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call_node: NodeId,
        declaration: NodeId,
    ) {
        if declaration.is_let(t)
            && !GOOG_FORWARD_DECLARE.matches(t, call_node.get_first_child(t).unwrap())
        {
            t.report(declaration, &LET_GOOG_REQUIRE, &[]);
        }
        if !declaration.has_one_child(t) {
            t.report(declaration, &ONE_REQUIRE_PER_DECLARATION, &[]);
            return;
        }
        let lhs = declaration.get_first_child(t).unwrap();
        if lhs.is_destructuring_lhs(t) {
            if !Self::is_valid_destructuring_import(t, lhs) {
                t.report(declaration, &INVALID_DESTRUCTURING_REQUIRE, &[]);
            }
            if GOOG_FORWARD_DECLARE.matches(t, call_node.get_first_child(t).unwrap()) {
                t.report(lhs, &INVALID_DESTRUCTURING_FORWARD_DECLARE, &[]);
            }
        } else if lhs.is_name(t) {
            let namespace = call_node.get_last_child(t).unwrap().get_string(t);
            Self::check_short_name(t, lhs, &namespace);
        }
        let key = Self::extract_first_argument_name(t, call_node);
        self.current_module_info()
            .imports_by_long_required_name
            .insert(key, lhs);
        // Java reports from the visitLhsNodesInNode consumer; the name nodes are collected in the
        // same order first so the consumer body can use the traversal.
        let mut name_nodes = Vec::new();
        NodeUtil::visit_lhs_nodes_in_node(t.get_compiler(), declaration, &mut |_, name_node| {
            name_nodes.push(name_node)
        });
        for name_node in name_nodes {
            let name = name_node.get_string(t);
            if !self
                .current_module_info()
                .short_import_names
                .insert(name.clone())
            {
                t.report(
                    name_node,
                    &DUPLICATE_NAME_SHORT_REQUIRE,
                    &[&name.to_string()],
                );
            }
        }
    }

    // port: ClosureCheckModule#checkShortName
    fn check_short_name(t: &mut NodeTraversal<'_>, short_name_node: NodeId, namespace: &JsString) {
        let next_qname_part = short_name_node.get_string(t);
        if namespace.starts_with("google3.") {
            // `google3` namespaces don't provide capitalization context for the import name
            return;
        }

        let last_segment =
            namespace.substring_from((namespace.last_index_of_char(u16::from(b'.')) + 1) as usize);
        if next_qname_part == last_segment || last_segment.is_empty() {
            return;
        }

        if is_upper_case(next_qname_part.char_at(0)) != is_upper_case(last_segment.char_at(0)) {
            let new_start_char = if is_upper_case(next_qname_part.char_at(0)) {
                to_lower_case(next_qname_part.char_at(0))
            } else {
                to_upper_case(next_qname_part.char_at(0))
            };
            let corrected_name = JsString::from_units(vec![new_start_char])
                .concat(&next_qname_part.substring_from(1));
            t.report(
                short_name_node,
                &INCORRECT_SHORTNAME_CAPITALIZATION,
                &[&next_qname_part.to_string(), &corrected_name.to_string()],
            );
        }
    }

    // port: ClosureCheckModule#isValidDestructuringImport
    fn is_valid_destructuring_import(ast: &Ast, destructuring_lhs: NodeId) -> bool {
        check_argument!(destructuring_lhs.is_destructuring_lhs(ast));
        let object_pattern = destructuring_lhs.get_first_child(ast).unwrap();
        if !object_pattern.is_object_pattern(ast) {
            return false;
        }
        let mut string_key = object_pattern.get_first_child(ast);
        while let Some(key) = string_key {
            if !key.is_string_key(ast) {
                return false;
            } else if !key.has_children(ast) || !key.get_first_child(ast).unwrap().is_name(ast) {
                return false;
            }
            string_key = key.get_next(ast);
        }
        true
    }

    /// Validates the position of a goog.module.declareLegacyNamespace(); call
    // port: ClosureCheckModule#checkLegacyNamespaceCall
    fn check_legacy_namespace_call(t: &mut NodeTraversal<'_>, call_node: NodeId, parent: NodeId) {
        check_argument!(call_node.is_call(t));
        if call_node.get_child_count(t) > 1 {
            t.report(call_node, &LEGACY_NAMESPACE_ARGUMENT, &[]);
        }
        if !parent.is_expr_result(t) {
            t.report(call_node, &LEGACY_NAMESPACE_NOT_AT_TOP_LEVEL, &[]);
            return;
        }
        let prev = parent.get_previous(t);
        if !prev.is_some_and(|prev| NodeUtil::is_goog_module_call(t, prev)) {
            t.report(call_node, &LEGACY_NAMESPACE_NOT_AFTER_GOOG_MODULE, &[]);
        }
    }

    /// Whether this is the first statement in a module
    // port: ClosureCheckModule#isFirstExpressionInGoogModule
    fn is_first_expression_in_goog_module(ast: &Ast, node: NodeId) -> bool {
        if !node.is_expr_result(ast) || node.get_previous(ast).is_some() {
            return false;
        }
        let statement_block = node.get_parent(ast).unwrap();
        statement_block.is_module_body(ast)
            || NodeUtil::is_bundled_goog_module_scope_root(ast, statement_block)
    }
}

impl CompilerPass for ClosureCheckModule {
    // port: ClosureCheckModule#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let module_metadata_map = self.module_metadata_map.clone();
        let mut callback = AbstractModuleCallback::new(&module_metadata_map, &mut *self);
        NodeTraversal::traverse(compiler, root, &mut callback);
    }
}

impl ModuleCallback for &mut ClosureCheckModule {
    // port: ClosureCheckModule#enterModule
    fn enter_module(
        &mut self,
        _compiler: &mut AbstractCompiler,
        current_module: &Arc<ModuleMetadata>,
        _module_scope_root: NodeId,
    ) {
        if !current_module.is_goog_module() {
            return;
        }

        check_state!(self.current_module_info.is_none());
        let goog_namespaces = current_module.goog_namespaces();
        check_state!(!goog_namespaces.is_empty());
        self.current_module_info = Some(ModuleInfo::new(
            goog_namespaces
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| JsString::from("")),
        ));
    }

    // port: ClosureCheckModule#exitModule
    fn exit_module(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _current_module: &Arc<ModuleMetadata>,
        _module_scope_root: NodeId,
    ) {
        self.current_module_info = None;
    }

    // port: ClosureCheckModule#visit
    fn visit(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _current_module: Option<&Arc<ModuleMetadata>>,
        _module_scope_root: Option<NodeId>,
    ) {
        let parent = n.get_parent(t);
        if self.current_module_info.is_none() {
            if NodeUtil::is_call_to_qualified_name(t, n, &GOOG_MODULE) {
                t.report(n, &GOOG_MODULE_IN_NON_MODULE, &[]);
            } else if NodeUtil::is_goog_module_declare_legacy_namespace_call(t, n) {
                t.report(n, &DECLARE_LEGACY_NAMESPACE_IN_NON_MODULE, &[]);
            } else if NodeUtil::is_call_to_qualified_name(t, n, &GOOG_PROVIDE) {
                // This error is reported here, rather than in a provide-specific pass, because
                // it must be reported prior to ClosureRewriteModule converting legacy
                // goog.modules into goog.provides.
                t.report(n, &USE_OF_GOOG_PROVIDE, &[]);
            }
            return;
        }
        let js_doc = n.get_jsdoc_info(t);
        if let Some(js_doc) = js_doc {
            self.check_js_doc(t, &js_doc);
        }
        match n.get_token(t) {
            Token::CALL => {
                let callee = n.get_first_child(t).unwrap();
                if GOOG_MODULE.matches(t, callee) {
                    let module_name = self.current_module_info().name.clone();
                    if Some(module_name) != ClosureCheckModule::extract_first_argument_name(t, n) {
                        t.report(n, &MULTIPLE_MODULES_IN_FILE, &[]);
                    } else if !ClosureCheckModule::is_first_expression_in_goog_module(
                        t,
                        parent.unwrap(),
                    ) {
                        t.report(n, &GOOG_MODULE_MISPLACED, &[]);
                    }
                } else if GOOG_REQUIRE.matches(t, callee)
                    || GOOG_REQUIRE_TYPE.matches(t, callee)
                    || GOOG_FORWARD_DECLARE.matches(t, callee)
                {
                    self.check_require_call(t, n, parent.unwrap());
                } else if GOOG_MODULE_GET.matches(t, callee) && t.in_module_hoist_scope() {
                    t.report(n, &MODULE_USES_GOOG_MODULE_GET, &[]);
                } else if GOOG_MODULE_DECLARE_LEGACY_NAMESPACE.matches(t, callee) {
                    ClosureCheckModule::check_legacy_namespace_call(t, n, parent.unwrap());
                }
            }
            Token::ASSIGN => {
                if ClosureCheckModule::is_export_lhs(t, n.get_first_child(t).unwrap()) {
                    self.check_module_export(t, n, parent.unwrap());
                }
            }
            token @ (Token::CLASS | Token::FUNCTION | Token::VAR | Token::LET | Token::CONST) => {
                if matches!(token, Token::CLASS | Token::FUNCTION) && !NodeUtil::is_statement(t, n)
                {
                    return;
                }
                // fallthrough
                if t.in_module_hoist_scope()
                    && (n.is_class(t) || NodeUtil::get_enclosing_class(t, n).is_none())
                    && NodeUtil::get_enclosing_type(t, n, Token::OBJECTLIT).is_none()
                {
                    let jsdoc = NodeUtil::get_best_jsdoc_info(t, n);
                    if jsdoc.is_some_and(|jsdoc| jsdoc.is_export()) {
                        t.report(n, &AT_EXPORT_IN_GOOG_MODULE, &[]);
                    }
                }
            }
            Token::THIS => {
                if t.in_module_hoist_scope() {
                    t.report(n, &GOOG_MODULE_REFERENCES_THIS, &[]);
                }
            }
            Token::THROW => {
                if t.in_module_hoist_scope() {
                    t.report(n, &GOOG_MODULE_USES_THROW, &[]);
                }
            }
            Token::NAME | Token::GETPROP => {
                let module_name = self.current_module_info().name.clone();
                if n.matches_qualified_name(t, module_name) {
                    if n.is_get_prop(t) {
                        // This warning makes sense on NAME nodes as well,
                        // but it would require a cleanup to land.
                        t.report(n, &REFERENCE_TO_MODULE_GLOBAL_NAME, &[]);
                    }
                } else if !parent.unwrap().is_get_prop(t) && n.is_qualified_name(t) {
                    let mut node = n;

                    loop {
                        let qname = node.get_qualified_name(t);
                        let node_parent = node.get_parent(t).unwrap();
                        let next_qname_part = if node_parent.is_get_prop(t) {
                            Some(node_parent.get_string(t))
                        } else {
                            None
                        };
                        if !self.check_improper_reference_to_import(t, node, qname, next_qname_part)
                        {
                            break;
                        }
                        if node.is_name(t) || !node.has_children(t) {
                            // Don't get the value of a declaration (i.e. `var name = 2`)
                            return;
                        }
                        node = node.get_only_child(t);
                    }
                }
            }
            _ => {}
        }
    }
}
